use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rand::seq::SliceRandom;
use rand::Rng;

use crate::config::Config;
use crate::library::Track;

#[derive(Debug, Clone)]
pub struct Playlist {
    pub name: String,
    #[allow(dead_code)]
    pub path: Option<PathBuf>,
    pub tracks: Vec<Arc<Track>>,
}

impl Playlist {
    pub fn temporary(tracks: Vec<Arc<Track>>) -> Self {
        Self {
            name: "queue".into(),
            path: None,
            tracks,
        }
    }

    pub fn from_m3u(path: &Path, library: &[Arc<Track>]) -> Result<Self> {
        let tracks = read_paths(path, library, &index_library(library))?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("playlist")
            .to_string();
        Ok(Self {
            name,
            path: Some(path.to_path_buf()),
            tracks,
        })
    }

    pub fn save_m3u(&self, name: &str) -> Result<PathBuf> {
        let safe = sanitize_playlist_name(name)?;
        let path = Config::playlists_dir()?.join(format!("{safe}.m3u"));
        let mut file = File::create(&path)?;
        writeln!(file, "#EXTM3U")?;
        for track in &self.tracks {
            writeln!(file, "{}", track.path.display())?;
        }
        Ok(path)
    }
}

/// Keep letters from any alphabet. Drop separators so a name cannot escape the playlist directory.
pub fn sanitize_playlist_name(name: &str) -> Result<String> {
    let mut safe = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_control() || matches!(c, '/' | '\\' | '\0') {
            continue;
        }
        safe.push(c);
    }
    let safe = safe.trim().trim_matches('.').trim();
    if safe.is_empty() || safe == ".." {
        bail!("empty playlist name");
    }
    Ok(safe.to_string())
}

#[derive(Debug, Default)]
pub struct Queue {
    pub tracks: Vec<Arc<Track>>,
    pub index: Option<usize>,
    pub shuffle: bool,
    order: Vec<usize>,
}

impl Queue {
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.index = None;
        self.order.clear();
        self.tracks.shrink_to_fit();
        self.order.shrink_to_fit();
    }

    pub fn replace(&mut self, tracks: Vec<Arc<Track>>) {
        self.tracks = tracks;
        self.index = if self.tracks.is_empty() {
            None
        } else {
            Some(0)
        };
        self.rebuild_order();
    }

    /// Append without reshuffling tracks that are already queued.
    pub fn append(&mut self, track: Arc<Track>) {
        let new_i = self.tracks.len();
        self.tracks.push(track);
        if self.index.is_none() {
            self.index = Some(0);
        }
        if self.order.len() != new_i {
            self.rebuild_order();
            return;
        }
        if self.shuffle && new_i > 0 {
            let mut rng = rand::rng();
            let cur_pos = self
                .index
                .and_then(|cur| self.order.iter().position(|&i| i == cur))
                .unwrap_or(0);
            let after = (cur_pos + 1).min(self.order.len());
            let at = if after >= self.order.len() {
                self.order.len()
            } else {
                rng.random_range(after..=self.order.len())
            };
            self.order.insert(at, new_i);
        } else {
            self.order.push(new_i);
        }
    }

    pub fn remove_at(&mut self, index: usize) -> Option<Arc<Track>> {
        if index >= self.tracks.len() {
            return None;
        }
        let removed = self.tracks.remove(index);
        self.order.retain(|&i| i != index);
        for slot in &mut self.order {
            if *slot > index {
                *slot -= 1;
            }
        }
        self.index = match self.index {
            Some(i) if i == index => {
                if self.tracks.is_empty() {
                    None
                } else {
                    Some(index.min(self.tracks.len() - 1))
                }
            }
            Some(i) if i > index => Some(i - 1),
            other => other,
        };
        Some(removed)
    }

    pub fn move_track(&mut self, from: usize, delta: isize) -> Option<usize> {
        if self.tracks.is_empty() || from >= self.tracks.len() || delta == 0 {
            return None;
        }
        let last = self.tracks.len() - 1;
        let to = (from as isize + delta).clamp(0, last as isize) as usize;
        if to == from {
            return None;
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        for slot in &mut self.order {
            *slot = remap_index(*slot, from, to);
        }
        if let Some(i) = self.index {
            self.index = Some(remap_index(i, from, to));
        }
        Some(to)
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
        let current = self.index;
        self.rebuild_order();
        self.index = current.or_else(|| self.order.first().copied());
    }

    fn rebuild_order(&mut self) {
        let mut order: Vec<usize> = (0..self.tracks.len()).collect();
        if self.shuffle && order.len() > 1 {
            let mut rng = rand::rng();
            if let Some(cur) = self.index {
                order.retain(|&i| i != cur);
                order.shuffle(&mut rng);
                order.insert(0, cur);
            } else {
                order.shuffle(&mut rng);
            }
        }
        self.order = order;
    }

    pub fn current(&self) -> Option<&Arc<Track>> {
        self.index.and_then(|i| self.tracks.get(i))
    }

    pub fn next_index(&self, repeat_one: bool, repeat_all: bool) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        if repeat_one {
            return self.index;
        }
        let Some(cur) = self.index else {
            return self.order.first().copied();
        };
        let pos = self.order.iter().position(|&i| i == cur).unwrap_or(0);
        if pos + 1 < self.order.len() {
            Some(self.order[pos + 1])
        } else if repeat_all {
            self.order.first().copied()
        } else {
            None
        }
    }

    pub fn prev_index(&self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        let Some(cur) = self.index else {
            return self.order.first().copied();
        };
        let pos = self.order.iter().position(|&i| i == cur).unwrap_or(0);
        if pos > 0 {
            Some(self.order[pos - 1])
        } else {
            Some(self.order[pos])
        }
    }

    pub fn advance(&mut self, repeat_one: bool, repeat_all: bool) -> Option<&Arc<Track>> {
        self.index = self.next_index(repeat_one, repeat_all);
        self.current()
    }

    pub fn retreat(&mut self) -> Option<&Arc<Track>> {
        self.index = self.prev_index();
        self.current()
    }

    pub fn play_at(&mut self, index: usize) -> Option<&Arc<Track>> {
        if index < self.tracks.len() {
            self.index = Some(index);
            if self.shuffle {
                self.rebuild_order();
            }
            self.current()
        } else {
            None
        }
    }

    /// Set the current track without rebuilding the shuffle order.
    pub fn select_index(&mut self, index: usize) -> Option<&Arc<Track>> {
        if index < self.tracks.len() {
            self.index = Some(index);
            self.current()
        } else {
            None
        }
    }

    pub fn play_len(&self) -> usize {
        if self.order.len() == self.tracks.len() {
            self.order.len()
        } else {
            self.tracks.len()
        }
    }

    pub fn track_index_at_play(&self, pos: usize) -> Option<usize> {
        if self.order.len() == self.tracks.len() {
            self.order.get(pos).copied()
        } else if pos < self.tracks.len() {
            Some(pos)
        } else {
            None
        }
    }

    pub fn track_at_play(&self, pos: usize) -> Option<&Arc<Track>> {
        self.track_index_at_play(pos)
            .and_then(|index| self.tracks.get(index))
    }

    pub fn play_pos_of_index(&self, index: usize) -> usize {
        self.order.iter().position(|&i| i == index).unwrap_or(0)
    }

    /// Move a row in playback order. With shuffle off, the stored list follows.
    pub fn move_play_pos(&mut self, from_pos: usize, delta: isize) -> Option<usize> {
        let len = self.play_len();
        if len == 0 || from_pos >= len || delta == 0 {
            return None;
        }
        let to_pos = (from_pos as isize + delta).clamp(0, len as isize - 1) as usize;
        if to_pos == from_pos {
            return None;
        }
        if self.order.len() != self.tracks.len() {
            self.rebuild_order();
        }
        let track_index = self.order.remove(from_pos);
        self.order.insert(to_pos, track_index);
        if !self.shuffle {
            let current = self.index.and_then(|i| self.tracks.get(i).cloned());
            let tracks: Vec<_> = self
                .order
                .iter()
                .filter_map(|&i| self.tracks.get(i).cloned())
                .collect();
            self.tracks = tracks;
            self.order = (0..self.tracks.len()).collect();
            self.index = current
                .as_ref()
                .and_then(|cur| self.tracks.iter().position(|t| Arc::ptr_eq(t, cur)));
        }
        Some(to_pos)
    }
}

fn remap_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        return to;
    }
    if from < to {
        if index > from && index <= to {
            index - 1
        } else {
            index
        }
    } else if index >= to && index < from {
        index + 1
    } else {
        index
    }
}

fn index_library(library: &[Arc<Track>]) -> HashMap<&Path, usize> {
    let mut by_path = HashMap::with_capacity(library.len());
    for (index, track) in library.iter().enumerate() {
        by_path.insert(track.path.as_path(), index);
    }
    by_path
}

fn read_paths(
    path: &Path,
    library: &[Arc<Track>],
    by_path: &HashMap<&Path, usize>,
) -> Result<Vec<Arc<Track>>> {
    let file = File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut tracks = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let track_path = Path::new(line);
        if let Some(index) = by_path.get(track_path) {
            tracks.push(Arc::clone(&library[*index]));
        } else if track_path.exists() {
            tracks.push(placeholder_track(track_path.to_path_buf()));
        }
    }
    Ok(tracks)
}

fn placeholder_track(path: PathBuf) -> Arc<Track> {
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");
    Arc::new(Track {
        title: Arc::from(title),
        artist: Arc::from("Unknown"),
        album: Arc::from(""),
        path,
        duration: None,
    })
}

pub fn list_saved_playlists() -> Result<Vec<PathBuf>> {
    let dir = Config::playlists_dir()?;
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("m3u"))
        })
        .collect();
    paths.sort();
    Ok(paths)
}

pub fn delete_playlist(path: &Path) -> Result<()> {
    fs::remove_file(path).with_context(|| format!("delete {}", path.display()))
}

fn session_path() -> Result<PathBuf> {
    Ok(Config::data_dir()?.join("session.txt"))
}

/// Remember the queue without copying the whole library when it is still a slice of it.
pub fn save_session(library: &[Arc<Track>], queue: &Queue, pos: Duration) -> Result<()> {
    let path = session_path()?;
    if queue.tracks.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    let index = queue.index.unwrap_or(0);
    let pos_ms = u64::try_from(pos.as_millis()).unwrap_or(0);
    let mut body = String::new();
    if let Some((start, len)) = library_range(&queue.tracks, library) {
        body.push_str(&format!("#CODA range {start} {len} {index} {pos_ms}\n"));
    } else {
        body.push_str(&format!("#CODA list {index} {pos_ms}\n"));
        for track in &queue.tracks {
            body.push_str(&track.path.display().to_string());
            body.push('\n');
        }
    }
    let tmp = path.with_extension("txt.tmp");
    fs::write(&tmp, body)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn load_session(library: &[Arc<Track>]) -> Option<(Vec<Arc<Track>>, usize, Duration)> {
    let path = session_path().ok()?;
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header = lines.next()?.trim();
    let mut parts = header.split_whitespace();
    if parts.next() != Some("#CODA") {
        return None;
    }
    match parts.next() {
        Some("range") => {
            let start: usize = parts.next()?.parse().ok()?;
            let len: usize = parts.next()?.parse().ok()?;
            let index: usize = parts.next()?.parse().ok()?;
            let pos_ms: u64 = parts.next()?.parse().ok()?;
            if start.checked_add(len)? > library.len() || len == 0 {
                return None;
            }
            let tracks = library[start..start + len].to_vec();
            let index = index.min(tracks.len() - 1);
            Some((tracks, index, Duration::from_millis(pos_ms)))
        }
        Some("list") => {
            let index: usize = parts.next()?.parse().ok()?;
            let pos_ms: u64 = parts.next()?.parse().ok()?;
            let by_path = index_library(library);
            let mut tracks = Vec::new();
            for line in lines {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let track_path = Path::new(line);
                if let Some(index) = by_path.get(track_path) {
                    tracks.push(Arc::clone(&library[*index]));
                } else if track_path.exists() {
                    tracks.push(placeholder_track(track_path.to_path_buf()));
                }
            }
            if tracks.is_empty() {
                return None;
            }
            let index = index.min(tracks.len() - 1);
            Some((tracks, index, Duration::from_millis(pos_ms)))
        }
        _ => None,
    }
}

fn library_range(queue: &[Arc<Track>], library: &[Arc<Track>]) -> Option<(usize, usize)> {
    if queue.is_empty() || queue.len() > library.len() {
        return None;
    }
    let start = library
        .iter()
        .position(|track| Arc::ptr_eq(track, &queue[0]))?;
    if start + queue.len() > library.len() {
        return None;
    }
    let same = queue
        .iter()
        .zip(&library[start..])
        .all(|(queued, lib)| Arc::ptr_eq(queued, lib));
    same.then_some((start, queue.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(name: &str) -> Arc<Track> {
        Arc::new(Track {
            path: PathBuf::from(name),
            title: Arc::from(name),
            artist: Arc::from("a"),
            album: Arc::from(""),
            duration: None,
        })
    }

    #[test]
    fn playlist_name_keeps_cyrillic() {
        assert_eq!(sanitize_playlist_name("вечер").unwrap(), "вечер");
        assert_eq!(sanitize_playlist_name("a/b\\c").unwrap(), "abc");
        assert!(sanitize_playlist_name("...").is_err());
    }

    #[test]
    fn append_does_not_reshuffle_existing_order() {
        let mut queue = Queue::default();
        queue.replace(vec![track("1"), track("2"), track("3")]);
        queue.set_shuffle(true);
        queue.play_at(1);
        let before = queue.order.clone();
        queue.append(track("4"));
        let kept: Vec<_> = queue.order.iter().copied().filter(|&i| i != 3).collect();
        assert_eq!(kept, before);
    }

    #[test]
    fn move_play_pos_follows_the_row_you_see() {
        let mut queue = Queue::default();
        queue.replace(vec![track("a"), track("b"), track("c")]);
        let to = queue.move_play_pos(0, 2).unwrap();
        assert_eq!(to, 2);
        assert_eq!(queue.track_at_play(0).unwrap().title.as_ref(), "b");
        assert_eq!(queue.track_at_play(2).unwrap().title.as_ref(), "a");
        assert_eq!(queue.index, Some(2));
    }

    #[test]
    fn move_track_keeps_now_playing() {
        let mut queue = Queue::default();
        queue.replace(vec![track("a"), track("b"), track("c")]);
        queue.play_at(0);
        let to = queue.move_track(0, 2).unwrap();
        assert_eq!(to, 2);
        assert_eq!(queue.index, Some(2));
        assert_eq!(queue.current().unwrap().title.as_ref(), "a");
    }
}
