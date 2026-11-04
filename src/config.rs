use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::game::LEVELS;

pub const DEFAULT_ROUNDS: u32 = 5;

#[derive(Debug, Clone)]
pub struct Config {
    pub music_dir: PathBuf,
    pub rounds: u32,
    pub level_secs: Vec<u64>,
}

/// Optional keys of `config.toml`. Unknown keys are ignored so files
/// written for newer versions keep loading.
#[derive(Debug, Deserialize)]
struct FileConfig {
    music_dir: Option<PathBuf>,
    rounds: Option<u32>,
}

impl Config {
    /// Resolve configuration. CLI flags take precedence over the XDG
    /// config file, which takes precedence over defaults.
    pub fn load(dir: Option<PathBuf>, rounds: Option<u32>) -> Result<Self> {
        let mut cfg = Self::defaults();
        if let Some(file) = read_file_config()? {
            cfg.apply_file(file);
        }
        if let Some(dir) = dir {
            cfg.music_dir = expand_tilde(&dir);
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

    fn apply_file(&mut self, file: FileConfig) {
        if let Some(dir) = file.music_dir {
            self.music_dir = expand_tilde(&dir);
        }
        if let Some(rounds) = file.rounds {
            self.rounds = rounds.clamp(1, 99);
        }
    }
}

/// Path of the optional config file:
/// `$XDG_CONFIG_HOME/fragment/config.toml` (Linux:
/// `~/.config/fragment/config.toml`). `None` when the platform config
/// directory cannot be resolved.
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "fragment")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// Read `config.toml` when it exists. A missing file is `Ok(None)`; a
/// file that cannot be read or parsed is an error naming its path.
fn read_file_config() -> Result<Option<FileConfig>> {
    let Some(path) = config_path() else {
        return Ok(None);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read config file {}", path.display()));
        }
    };
    toml::from_str(&text)
        .with_context(|| format!("failed to parse config file {}", path.display()))
        .map(Some)
}

/// Expand a leading `~` or `~/` against `$HOME`. Other paths, including
/// `~name`-style prefixes, are returned unchanged.
fn expand_tilde(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    let rest = match text.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => rest.trim_start_matches('/'),
        _ => return path.to_path_buf(),
    };
    let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) else {
        return path.to_path_buf();
    };
    let mut expanded = PathBuf::from(home);
    if !rest.is_empty() {
        expanded.push(rest);
    }
    expanded
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Config, FileConfig, config_path, expand_tilde};

    #[test]
    fn config_path_is_fragment_config_toml() {
        let Some(path) = config_path() else {
            return;
        };
        assert_eq!(path.file_name().unwrap(), "config.toml");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "fragment");
    }

    #[test]
    fn file_config_keys_are_optional_and_unknown_keys_ignored() {
        let file: FileConfig = toml::from_str(
            r#"
            music_dir = "~/Music"
            rounds = 10
            future_key = "ignored"
            "#,
        )
        .unwrap();
        assert_eq!(file.music_dir, Some(PathBuf::from("~/Music")));
        assert_eq!(file.rounds, Some(10));

        let empty: FileConfig = toml::from_str("").unwrap();
        assert!(empty.music_dir.is_none());
        assert!(empty.rounds.is_none());
    }

    #[test]
    fn file_config_rejects_wrong_types() {
        assert!(toml::from_str::<FileConfig>(r#"rounds = "ten""#).is_err());
    }

    #[test]
    fn file_rounds_clamp_into_cli_range() {
        for (raw, want) in [(0, 1), (10, 10), (150, 99)] {
            let mut cfg = Config::defaults();
            cfg.apply_file(FileConfig {
                music_dir: None,
                rounds: Some(raw),
            });
            assert_eq!(cfg.rounds, want);
        }
    }

    #[test]
    fn expand_tilde_uses_home_for_leading_tilde_only() {
        let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) else {
            return;
        };
        let home = PathBuf::from(home);
        assert_eq!(expand_tilde(Path::new("~")), home);
        assert_eq!(expand_tilde(Path::new("~/Music")), home.join("Music"));
        assert_eq!(expand_tilde(Path::new("~/a/b")), home.join("a").join("b"));
        assert_eq!(
            expand_tilde(Path::new("/abs/dir")),
            PathBuf::from("/abs/dir")
        );
        assert_eq!(expand_tilde(Path::new("rel/dir")), PathBuf::from("rel/dir"));
        assert_eq!(
            expand_tilde(Path::new("~other/x")),
            PathBuf::from("~other/x")
        );
    }
}
