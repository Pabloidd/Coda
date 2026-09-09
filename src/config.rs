use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub music_dir: PathBuf,
    pub lang: Lang,
    pub volume: f32,
    pub repeat: RepeatMode,
    /// After the user dismisses the first-run keymap overlay.
    #[serde(default)]
    pub help_seen: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_dir: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Music"),
            lang: Lang::from_env(),
            volume: 0.7,
            repeat: RepeatMode::Off,
            help_seen: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    One,
    All,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("no config dir")?
            .join("coda");
        fs::create_dir_all(&dir)?;
        Ok(dir.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let text = fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&text)?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn data_dir() -> Result<PathBuf> {
        let dir = dirs::data_local_dir()
            .context("no data dir")?
            .join("coda");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn playlists_dir() -> Result<PathBuf> {
        let dir = Self::data_dir()?.join("playlists");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
