//! `config.toml`: hotkey, default mode, action keywords and file-search roots.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::index::SharedIndex;
use crate::modes::{FilesMode, WebSearchMode};
use crate::registry::{ModeRegistry, RegistryError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Global hotkey, e.g. `"Alt+Space"` or `"Ctrl+Space"`.
    pub hotkey: String,
    /// Mode shown when the bar opens: `"google"`, `"youtube"` or `"files"`.
    pub default_mode: String,
    pub max_results: usize,
    pub keywords: Keywords,
    pub files: FilesConfig,
}

/// Action keywords: type `<keyword><space>` to jump to a mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keywords {
    pub google: String,
    pub youtube: String,
    pub files: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesConfig {
    /// Folders to index. Empty means Desktop, Documents and Downloads.
    pub roots: Vec<PathBuf>,
    /// Folder names that are skipped with everything inside them.
    pub exclude_dir_names: Vec<String>,
    pub reindex_minutes: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".into(),
            default_mode: "google".into(),
            max_results: 8,
            keywords: Keywords::default(),
            files: FilesConfig::default(),
        }
    }
}

impl Default for Keywords {
    fn default() -> Self {
        Self {
            google: "g".into(),
            youtube: "yt".into(),
            files: "f".into(),
        }
    }
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            exclude_dir_names: [".git", "node_modules", "target", "$RECYCLE.BIN"]
                .map(String::from)
                .to_vec(),
            reindex_minutes: 30,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read or write the config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("config file is not valid TOML: {0}")]
    Parse(#[from] toml::de::Error),
}

/// Reads the config, creating it with defaults when missing. Never fails:
/// on any error the defaults are returned together with the error to log.
pub fn load_or_create(path: &Path) -> (Config, Option<ConfigError>) {
    match fs::read_to_string(path) {
        Ok(text) => match toml::from_str(&text) {
            Ok(config) => (config, None),
            Err(e) => (Config::default(), Some(e.into())),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let config = Config::default();
            (config.clone(), write_default(path, &config).err())
        }
        Err(e) => (Config::default(), Some(e.into())),
    }
}

fn write_default(path: &Path, config: &Config) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(config).map_err(std::io::Error::other)?;
    fs::write(path, text)?;
    Ok(())
}

/// Registers the built-in modes in Tab order (Google, YouTube, Files) and
/// returns the index of `default_mode`, or 0 when it names no mode.
pub fn build_registry(
    config: &Config,
    index: SharedIndex,
) -> Result<(ModeRegistry, usize), RegistryError> {
    let mut registry = ModeRegistry::new();
    registry.register(&config.keywords.google, Box::new(WebSearchMode::google()))?;
    registry.register(&config.keywords.youtube, Box::new(WebSearchMode::youtube()))?;
    registry.register(&config.keywords.files, Box::new(FilesMode::new(index)))?;
    let default = registry.index_of(&config.default_mode).unwrap_or(0);
    Ok((registry, default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn missing_file_is_created_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("flick/config.toml");
        let (c, err) = load_or_create(&path);
        assert!(err.is_none());
        assert_eq!(c, Config::default());
        assert!(path.exists());
    }

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!(
            (c.hotkey.as_str(), c.default_mode.as_str(), c.max_results),
            ("Alt+Space", "google", 8)
        );
        assert_eq!(
            (
                c.keywords.google.as_str(),
                c.keywords.youtube.as_str(),
                c.keywords.files.as_str()
            ),
            ("g", "yt", "f")
        );
        assert_eq!(
            c.files.exclude_dir_names,
            [".git", "node_modules", "target", "$RECYCLE.BIN"]
        );
        assert!(c.files.roots.is_empty());
        assert_eq!(c.files.reindex_minutes, 30);
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "hotkey = \"Ctrl+Space\"\n").unwrap();
        let (c, err) = load_or_create(&path);
        assert!(err.is_none());
        assert_eq!(c.hotkey, "Ctrl+Space");
        assert_eq!(c.max_results, 8);
        assert_eq!(c.keywords.youtube, "yt");
    }

    #[test]
    fn broken_file_gives_defaults_and_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "hotkey = ").unwrap();
        let (c, err) = load_or_create(&path);
        assert_eq!(c, Config::default());
        assert!(matches!(err, Some(ConfigError::Parse(_))));
        assert_eq!(fs::read_to_string(&path).unwrap(), "hotkey = ");
    }

    #[test]
    fn unknown_default_mode_falls_back_to_first() {
        let c = Config {
            default_mode: "bing".into(),
            ..Config::default()
        };
        assert_eq!(build_registry(&c, Default::default()).unwrap().1, 0);
    }

    #[test]
    fn registry_uses_configured_keywords() {
        let mut c = Config::default();
        c.keywords.youtube = "y".into();
        let (r, _) = build_registry(&c, Default::default()).unwrap();
        assert_eq!(r.parse("y cats", 0).mode, 1);
    }
}
