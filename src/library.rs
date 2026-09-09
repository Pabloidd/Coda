use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use walkdir::WalkDir;

const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wma", "aiff", "aif",
];

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<Duration>,
}

impl Track {
    pub fn display_name(&self) -> String {
        if self.artist.is_empty() || self.artist == "Unknown" {
            self.title.clone()
        } else {
            format!("{} — {}", self.artist, self.title)
        }
    }

    pub fn matches_filter(&self, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        let q = q.to_lowercase();
        self.title.to_lowercase().contains(&q)
            || self.artist.to_lowercase().contains(&q)
            || self.album.to_lowercase().contains(&q)
            || self
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase().contains(&q))
                .unwrap_or(false)
    }
}

#[derive(Debug, Default)]
pub struct Library {
    pub tracks: Vec<Track>,
    #[allow(dead_code)]
    pub scanned_at: Option<SystemTime>,
}

impl Library {
    pub fn scan(root: &Path) -> Result<Self> {
        let mut tracks = Vec::new();
        if !root.exists() {
            return Ok(Self {
                tracks,
                scanned_at: Some(SystemTime::now()),
            });
        }

        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                continue;
            }
            if let Some(track) = read_track(path) {
                tracks.push(track);
            }
        }

        tracks.sort_by(|a, b| {
            a.artist
                .to_lowercase()
                .cmp(&b.artist.to_lowercase())
                .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase()))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });

        Ok(Self {
            tracks,
            scanned_at: Some(SystemTime::now()),
        })
    }

    pub fn filtered<'a>(&'a self, query: &str) -> Vec<&'a Track> {
        self.tracks
            .iter()
            .filter(|t| t.matches_filter(query))
            .collect()
    }
}

fn read_track(path: &Path) -> Option<Track> {
    let fallback_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let mut title = fallback_title.clone();
    let mut artist = String::from("Unknown");
    let mut album = String::new();
    let mut duration = None;

    if let Ok(tagged) = Probe::open(path).and_then(|p| p.read()) {
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

    Some(Track {
        path: path.to_path_buf(),
        title,
        artist,
        album,
        duration,
    })
}
