use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::Result;
use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use walkdir::WalkDir;

const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wma", "aiff", "aif",
];

const CACHE_MAGIC: &[u8] = b"CODA1";
/// Skip oversized embedded art so one track cannot pin megabytes of RAM or disk.
const MAX_COVER_BYTES: usize = 1_500_000;

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: Arc<str>,
    pub artist: Arc<str>,
    pub album: Arc<str>,
    pub duration: Option<Duration>,
}

impl Track {
    pub fn display_name(&self) -> String {
        if self.artist.is_empty() || self.artist.as_ref() == "Unknown" {
            self.title.to_string()
        } else {
            format!("{} — {}", self.artist, self.title)
        }
    }

    /// `needle` is already lowercased. `buf` is reused so a library search does not allocate per field.
    pub fn matches_lower(&self, needle: &str, buf: &mut String) -> bool {
        if needle.is_empty() {
            return true;
        }
        contains_ci(&self.title, needle, buf)
            || contains_ci(&self.artist, needle, buf)
            || contains_ci(&self.album, needle, buf)
            || self
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| contains_ci(name, needle, buf))
    }
}

#[derive(Debug, Default)]
pub struct Library {
    pub tracks: Vec<Arc<Track>>,
}

impl Library {
    pub fn scan(root: &Path) -> Result<Self> {
        scan(root, false)
    }

    /// Ignore the on-disk tag cache and read every file again.
    pub fn rescan(root: &Path) -> Result<Self> {
        scan(root, true)
    }
}

struct Stamp {
    mtime: u64,
    len: u64,
}

struct Cached {
    mtime: u64,
    len: u64,
    duration_ms: u32,
    title: Arc<str>,
    artist: Arc<str>,
    album: Arc<str>,
}

fn scan(root: &Path, force: bool) -> Result<Library> {
    let cache_path = cache_file();
    let mut cache = if force {
        HashMap::new()
    } else {
        cache_path
            .as_ref()
            .map(|p| read_cache(p))
            .unwrap_or_default()
    };
    let had_cache = !cache.is_empty();

    let mut intern: HashMap<Arc<str>, ()> = HashMap::new();
    let mut built: Vec<Arc<Track>> = Vec::new();
    let mut stamps: Vec<Stamp> = Vec::new();
    let mut seen: HashSetIno = HashSetIno::default();
    let mut dirty = force || cache_path.is_some() && !had_cache;

    if root.exists() {
        let mut dirs_seen: HashMap<(u64, u64), ()> = HashMap::new();
        let walker = WalkDir::new(root).follow_links(true).into_iter();
        let walker = walker.filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            match entry.metadata() {
                Ok(meta) => dirs_seen.insert((meta.dev(), meta.ino()), ()).is_none(),
                Err(_) => true,
            }
        });

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let ino = (meta.dev(), meta.ino());
            if !seen.insert(ino) {
                continue;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let len = meta.len();

            let track = if let Some(hit) = cache.remove(path) {
                if hit.mtime == mtime && hit.len == len {
                    Arc::new(Track {
                        path: path.to_path_buf(),
                        title: share(&mut intern, &hit.title),
                        artist: share(&mut intern, &hit.artist),
                        album: share(&mut intern, &hit.album),
                        duration: duration_from_ms(hit.duration_ms),
                    })
                } else {
                    dirty = true;
                    read_track(path, &mut intern)
                }
            } else {
                dirty = true;
                read_track(path, &mut intern)
            };
            stamps.push(Stamp { mtime, len });
            built.push(track);
        }
    }

    if !cache.is_empty() {
        dirty = true;
    }
    drop(cache);
    drop(intern);
    Ok(finish_scan(built, stamps, dirty, cache_path))
}

fn finish_scan(
    tracks: Vec<Arc<Track>>,
    stamps: Vec<Stamp>,
    dirty: bool,
    cache_path: Option<PathBuf>,
) -> Library {
    let mut rows: Vec<(Arc<Track>, Stamp)> = tracks.into_iter().zip(stamps).collect();
    rows.sort_by(|a, b| cmp_track(&a.0, &b.0));
    let mut tracks = Vec::with_capacity(rows.len());
    let mut stamps = Vec::with_capacity(rows.len());
    for (track, stamp) in rows {
        tracks.push(track);
        stamps.push(stamp);
    }
    if dirty {
        if let Some(path) = cache_path {
            let _ = write_cache(&path, &tracks, &stamps);
        }
    }
    Library { tracks }
}

fn cmp_track(a: &Track, b: &Track) -> std::cmp::Ordering {
    cmp_ci(&a.artist, &b.artist)
        .then_with(|| cmp_ci(&a.album, &b.album))
        .then_with(|| cmp_ci(&a.title, &b.title))
}

/// Case-insensitive compare without allocating lowered copies.
fn cmp_ci(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ia = a.chars().flat_map(|c| c.to_lowercase());
    let mut ib = b.chars().flat_map(|c| c.to_lowercase());
    loop {
        match (ia.next(), ib.next()) {
            (Some(x), Some(y)) => {
                let ord = x.cmp(&y);
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            (None, None) => return std::cmp::Ordering::Equal,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (None, Some(_)) => return std::cmp::Ordering::Less,
        }
    }
}

#[derive(Default)]
struct HashSetIno {
    inner: HashMap<(u64, u64), ()>,
}

impl HashSetIno {
    fn insert(&mut self, key: (u64, u64)) -> bool {
        self.inner.insert(key, ()).is_none()
    }
}

fn read_track(path: &Path, intern: &mut HashMap<Arc<str>, ()>) -> Arc<Track> {
    let fallback_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    let mut title = fallback_title.to_string();
    let mut artist = String::from("Unknown");
    let mut album = String::new();
    let mut duration = None;

    let options = ParseOptions::new().read_cover_art(false);
    if let Ok(tagged) = Probe::open(path).and_then(|p| p.options(options).read()) {
        duration = Some(tagged.properties().duration());
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(t) = tag.title() {
                title = t.to_string();
            }
            if let Some(a) = tag.artist() {
                artist = a.to_string();
            }
            if let Some(a) = tag.album() {
                album = a.to_string();
            }
        }
    }

    Arc::new(Track {
        path: path.to_path_buf(),
        title: share(intern, &title),
        artist: share(intern, &artist),
        album: share(intern, &album),
        duration,
    })
}

fn share(intern: &mut HashMap<Arc<str>, ()>, value: &str) -> Arc<str> {
    if let Some((existing, _)) = intern.get_key_value(value) {
        return Arc::clone(existing);
    }
    let shared: Arc<str> = Arc::from(value);
    intern.insert(Arc::clone(&shared), ());
    shared
}

/// Read embedded cover art for the current track only. Library scan never does this.
pub fn read_cover(path: &Path) -> Option<(Vec<u8>, &'static str)> {
    let options = ParseOptions::new()
        .read_properties(false)
        .read_cover_art(true);
    let tagged = Probe::open(path).ok()?.options(options).read().ok()?;
    let tag = if let Some(tag) = tagged.primary_tag() {
        tag
    } else if let Some(tag) = tagged.first_tag() {
        tag
    } else {
        return None;
    };
    let pics = tag.pictures();
    let pic = pics
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| {
            pics.iter()
                .find(|p| !matches!(p.pic_type(), PictureType::Icon | PictureType::OtherIcon))
        })
        .or_else(|| pics.first())?;
    if pic.data().len() > MAX_COVER_BYTES || pic.data().is_empty() {
        return None;
    }
    let ext: &'static str = match pic.mime_type().and_then(|mime| mime.ext()) {
        Some("jpg") => "jpg",
        Some("png") => "png",
        Some("gif") => "gif",
        Some("bmp") => "bmp",
        Some("tif") => "tif",
        _ => "img",
    };
    Some((pic.data().to_vec(), ext))
}

fn cache_file() -> Option<PathBuf> {
    let dir = dirs::cache_dir()?.join("coda");
    fs::create_dir_all(&dir).ok()?;
    Some(dir.join("library.bin"))
}

fn duration_from_ms(ms: u32) -> Option<Duration> {
    if ms == u32::MAX {
        None
    } else {
        Some(Duration::from_millis(u64::from(ms)))
    }
}

fn duration_to_ms(duration: Option<Duration>) -> u32 {
    match duration {
        Some(d) => u32::try_from(d.as_millis()).unwrap_or(u32::MAX - 1),
        None => u32::MAX,
    }
}

fn read_cache(path: &Path) -> HashMap<PathBuf, Cached> {
    let Ok(mut file) = File::open(path) else {
        return HashMap::new();
    };
    let mut magic = [0u8; 5];
    if file.read_exact(&mut magic).is_err() || magic != CACHE_MAGIC {
        return HashMap::new();
    }
    let mut count_buf = [0u8; 4];
    if file.read_exact(&mut count_buf).is_err() {
        return HashMap::new();
    }
    let count = u32::from_le_bytes(count_buf) as usize;
    let mut map = HashMap::with_capacity(count.min(100_000));
    for _ in 0..count {
        let Ok(rec) = read_record(&mut file) else {
            return HashMap::new();
        };
        map.insert(rec.0, rec.1);
    }
    map
}

fn read_record(file: &mut File) -> Result<(PathBuf, Cached), ()> {
    let mtime = read_u64(file)?;
    let len = read_u64(file)?;
    let duration_ms = read_u32(file)?;
    let path = read_string(file)?;
    let title = read_string(file)?;
    let artist = read_string(file)?;
    let album = read_string(file)?;
    Ok((
        PathBuf::from(path.as_ref()),
        Cached {
            mtime,
            len,
            duration_ms,
            title,
            artist,
            album,
        },
    ))
}

fn read_u64(file: &mut File) -> Result<u64, ()> {
    let mut buf = [0u8; 8];
    file.read_exact(&mut buf).map_err(|_| ())?;
    Ok(u64::from_le_bytes(buf))
}

fn read_u32(file: &mut File) -> Result<u32, ()> {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf).map_err(|_| ())?;
    Ok(u32::from_le_bytes(buf))
}

fn read_string(file: &mut File) -> Result<Arc<str>, ()> {
    let len = read_u32(file)? as usize;
    if len > 64 * 1024 {
        return Err(());
    }
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).map_err(|_| ())?;
    let text = String::from_utf8(buf).map_err(|_| ())?;
    Ok(Arc::from(text))
}

fn write_cache(path: &Path, tracks: &[Arc<Track>], stamps: &[Stamp]) -> Result<()> {
    let tmp = path.with_extension("bin.tmp");
    {
        let mut file = File::create(&tmp)?;
        file.write_all(CACHE_MAGIC)?;
        let n = u32::try_from(tracks.len()).unwrap_or(u32::MAX);
        file.write_all(&n.to_le_bytes())?;
        for (track, stamp) in tracks.iter().zip(stamps.iter()) {
            file.write_all(&stamp.mtime.to_le_bytes())?;
            file.write_all(&stamp.len.to_le_bytes())?;
            file.write_all(&duration_to_ms(track.duration).to_le_bytes())?;
            write_str(&mut file, &track.path.to_string_lossy())?;
            write_str(&mut file, &track.title)?;
            write_str(&mut file, &track.artist)?;
            write_str(&mut file, &track.album)?;
        }
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn write_str(file: &mut File, value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    file.write_all(&len.to_le_bytes())?;
    file.write_all(&bytes[..len as usize])?;
    Ok(())
}

fn contains_ci(hay: &str, needle_lower: &str, buf: &mut String) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if hay.is_ascii() && needle_lower.is_ascii() {
        return hay
            .as_bytes()
            .windows(needle_lower.len())
            .any(|window| window.eq_ignore_ascii_case(needle_lower.as_bytes()));
    }
    buf.clear();
    buf.extend(hay.chars().flat_map(|c| c.to_lowercase()));
    buf.contains(needle_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyrillic_filter_reuses_buffer() {
        let track = Track {
            path: PathBuf::from("/music/kino.mp3"),
            title: Arc::from("Группа крови"),
            artist: Arc::from("Кино"),
            album: Arc::from("Группа крови"),
            duration: None,
        };
        let mut buf = String::new();
        assert!(track.matches_lower("кино", &mut buf));
        assert!(!track.matches_lower("наутилус", &mut buf));
        assert!(buf.capacity() > 0);
    }

    #[test]
    fn cache_roundtrip_keeps_unicode() {
        let dir = std::env::temp_dir().join(format!("coda-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.bin");
        let track = Arc::new(Track {
            path: PathBuf::from("/home/pabloid/Музыка/вечер.mp3"),
            title: Arc::from("Вечер"),
            artist: Arc::from("Кино"),
            album: Arc::from(""),
            duration: Some(Duration::from_millis(1500)),
        });
        let stamps = [Stamp { mtime: 42, len: 99 }];
        write_cache(&path, std::slice::from_ref(&track), &stamps).unwrap();
        let map = read_cache(&path);
        let got = map
            .get(Path::new("/home/pabloid/Музыка/вечер.mp3"))
            .unwrap();
        assert_eq!(got.title.as_ref(), "Вечер");
        assert_eq!(got.artist.as_ref(), "Кино");
        assert_eq!(got.mtime, 42);
        assert_eq!(got.len, 99);
        assert_eq!(
            duration_from_ms(got.duration_ms),
            Some(Duration::from_millis(1500))
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
