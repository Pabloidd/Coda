use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rand::seq::SliceRandom;

use crate::config::Config;
use crate::library::Track;

#[derive(Debug, Clone)]
pub struct Playlist {
    pub name: String,
    #[allow(dead_code)]
    pub path: Option<PathBuf>,
    pub tracks: Vec<Track>,
}

impl Playlist {
    pub fn temporary(tracks: Vec<Track>) -> Self {
        Self {
            name: "queue".into(),
            path: None,
            tracks,
        }
    }

    pub fn from_m3u(path: &Path, library: &[Track]) -> Result<Self> {
        let file = fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut tracks = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let track_path = PathBuf::from(line);
            if let Some(t) = library.iter().find(|t| t.path == track_path) {
                tracks.push(t.clone());
            } else if track_path.exists() {
                tracks.push(Track {
                    title: track_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    artist: "Unknown".into(),
                    album: String::new(),
                    path: track_path,
                    duration: None,
                });
            }
        }

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
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let safe = safe.trim();
        if safe.is_empty() {
            bail!("empty playlist name");
        }

        let path = Config::playlists_dir()?.join(format!("{safe}.m3u"));
        let mut file = fs::File::create(&path)?;
        writeln!(file, "#EXTM3U")?;
        for track in &self.tracks {
            writeln!(file, "{}", track.path.display())?;
        }
        Ok(path)
    }
}

#[derive(Debug, Default)]
pub struct Queue {
    pub tracks: Vec<Track>,
    pub index: Option<usize>,
    pub shuffle: bool,
    order: Vec<usize>,
}

impl Queue {
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.index = None;
        self.order.clear();
    }

    pub fn replace(&mut self, tracks: Vec<Track>) {
        self.tracks = tracks;
        self.index = if self.tracks.is_empty() {
            None
        } else {
            Some(0)
        };
        self.rebuild_order();
    }

    pub fn append(&mut self, track: Track) {
        self.tracks.push(track);
        if self.index.is_none() && !self.tracks.is_empty() {
            self.index = Some(0);
        }
        self.rebuild_order();
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
        let current_path = self.current().map(|t| t.path.clone());
        self.rebuild_order();
        if let Some(path) = current_path {
            if let Some(pos) = self
                .order
                .iter()
                .position(|&i| self.tracks.get(i).map(|t| &t.path) == Some(&path))
            {
                // keep logical index pointing to same track via order position
                let _ = pos;
            }
            if let Some(i) = self.tracks.iter().position(|t| t.path == path) {
                self.index = Some(i);
            }
        }
    }

    fn rebuild_order(&mut self) {
        let mut order: Vec<usize> = (0..self.tracks.len()).collect();
        if self.shuffle && order.len() > 1 {
            let mut rng = rand::rng();
            // keep current track first if possible
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

    pub fn current(&self) -> Option<&Track> {
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

    pub fn advance(&mut self, repeat_one: bool, repeat_all: bool) -> Option<&Track> {
        self.index = self.next_index(repeat_one, repeat_all);
        self.current()
    }

    pub fn retreat(&mut self) -> Option<&Track> {
        self.index = self.prev_index();
        self.current()
    }

    pub fn play_at(&mut self, index: usize) -> Option<&Track> {
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
}

pub fn list_saved_playlists() -> Result<Vec<PathBuf>> {
    let dir = Config::playlists_dir()?;
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("m3u"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    Ok(paths)
}

pub fn delete_playlist(path: &Path) -> Result<()> {
    fs::remove_file(path).with_context(|| format!("delete {}", path.display()))
}
