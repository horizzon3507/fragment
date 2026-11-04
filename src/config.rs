use std::path::PathBuf;

use anyhow::Result;

use crate::game::LEVELS;

pub const DEFAULT_ROUNDS: u32 = 5;

#[derive(Debug, Clone)]
pub struct Config {
    pub music_dir: PathBuf,
    pub rounds: u32,
    pub level_secs: Vec<u64>,
}

impl Config {
    /// Resolve configuration. CLI flags take precedence over the XDG
    /// config file, which takes precedence over defaults.
    pub fn load(dir: Option<PathBuf>, rounds: Option<u32>) -> Result<Self> {
        let mut cfg = Self::defaults();
        if let Some(dir) = dir {
            cfg.music_dir = dir;
        }
        if let Some(rounds) = rounds {
            cfg.rounds = rounds;
        }
        Ok(cfg)
    }

    pub fn defaults() -> Self {
        let music_dir = directories::UserDirs::new()
            .and_then(|u| u.audio_dir().map(PathBuf::from))
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default()
                    .join("Music")
            });
        Self {
            music_dir,
            rounds: DEFAULT_ROUNDS,
            level_secs: LEVELS.to_vec(),
        }
    }
}
