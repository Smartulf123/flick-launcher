//! `config.toml`: hotkey, default mode, file-search roots and appearance.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::index::SharedIndex;
use crate::modes::suggest::Suggestions;
use crate::modes::{FilesMode, WebSearchMode};
use crate::registry::ModeRegistry;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Global hotkey, e.g. `"Alt+Space"` or `"Ctrl+Space"`.
    pub hotkey: String,
    /// Mode shown when the bar opens: `"google"`, `"youtube"` or `"files"`.
    pub default_mode: String,
    pub max_results: usize,
    /// Show Google and YouTube suggestions while typing. Sends what you
    /// type to Google, as a browser search box does.
    pub suggestions: bool,
    pub files: FilesConfig,
    pub appearance: Appearance,
}

/// How the bar looks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// `"builtin:<id>"` (a picture inside Flick), `"none"`, or a path to a picture.
    pub background: String,
    /// How dark the cards are over the picture, from 0 (clear) to 1 (solid).
    pub tint: f32,
}

/// The built-in picture used when nothing else is chosen.
pub const DEFAULT_BUILTIN: &str = "great-wave";

impl Default for Appearance {
    fn default() -> Self {
        Self {
            background: format!("builtin:{DEFAULT_BUILTIN}"),
            tint: 0.55,
        }
    }
}

impl Appearance {
    pub fn clamped_tint(&self) -> f32 {
        self.tint.clamp(0.0, 1.0)
    }
}

/// Where the background picture comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundSource {
    /// A picture inside Flick, by id (e.g. `"great-wave"`).
    Builtin(String),
    None,
    File(PathBuf),
}

impl BackgroundSource {
    pub fn from_setting(setting: &str) -> Self {
        match setting.trim() {
            "" | "none" => Self::None,
            // Older versions followed the desktop wallpaper.
            "wallpaper" => Self::Builtin(DEFAULT_BUILTIN.into()),
            other => match other.strip_prefix("builtin:") {
                Some(id) => Self::Builtin(id.into()),
                None => Self::File(PathBuf::from(other)),
            },
        }
    }

    pub fn to_setting(&self) -> String {
        match self {
            Self::Builtin(id) => format!("builtin:{id}"),
            Self::None => "none".into(),
            Self::File(path) => path.display().to_string(),
        }
    }
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

/// The hotkey a new settings file starts with. Linux gets Ctrl+Alt+Space,
/// because GNOME and KDE use Alt+Space for the window menu.
pub fn default_hotkey() -> &'static str {
    if cfg!(target_os = "linux") {
        "Ctrl+Alt+Space"
    } else {
        "Alt+Space"
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey().into(),
            default_mode: "google".into(),
            max_results: 8,
            suggestions: true,
            files: FilesConfig::default(),
            appearance: Appearance::default(),
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
            (config.clone(), save(path, &config).err())
        }
        Err(e) => (Config::default(), Some(e.into())),
    }
}

/// Writes the whole config, creating its folder if needed. Comments a user
/// added by hand are not kept.
pub fn save(path: &Path, config: &Config) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(config).map_err(std::io::Error::other)?;
    fs::write(path, text)?;
    Ok(())
}

/// Registers the built-in modes in Tab order (Google, YouTube, Files) and
/// returns the index of `default_mode`, or 0 when it names no mode.
pub fn build_registry(config: &Config, index: SharedIndex) -> (ModeRegistry, usize) {
    build_registry_with(config, index, None)
}

/// Like `build_registry`, giving the web modes suggestions when
/// `config.suggestions` is on and a source is given.
pub fn build_registry_with(
    config: &Config,
    index: SharedIndex,
    suggestions: Option<Suggestions>,
) -> (ModeRegistry, usize) {
    let suggestions = suggestions.filter(|_| config.suggestions);
    let web = |mode: WebSearchMode| match &suggestions {
        Some(s) => mode.with_suggestions(s.clone()),
        None => mode,
    };
    let mut registry = ModeRegistry::new();
    registry.register(Box::new(web(WebSearchMode::google())));
    registry.register(Box::new(web(WebSearchMode::youtube())));
    registry.register(Box::new(FilesMode::new(index)));
    let default = registry.index_of(&config.default_mode).unwrap_or(0);
    (registry, default)
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

    use crate::modes::suggest::{Engine, SuggestionCache, SuggestionSource, Suggestions};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Engine>>);
    impl SuggestionSource for Recorder {
        fn request(&self, engine: Engine, _query: &str) {
            self.0.lock().unwrap().push(engine);
        }
    }

    fn suggestions(recorder: &Arc<Recorder>) -> Suggestions {
        Suggestions {
            cache: SuggestionCache::default(),
            source: recorder.clone(),
        }
    }

    #[test]
    fn suggestions_are_on_by_default() {
        assert!(Config::default().suggestions);
    }

    #[test]
    fn web_modes_get_suggestions_when_on() {
        let recorder = Arc::new(Recorder::default());
        let (r, _) = build_registry_with(
            &Config::default(),
            Default::default(),
            Some(suggestions(&recorder)),
        );
        r.mode(0).query("rust", 8);
        r.mode(1).query("lofi", 8);
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [Engine::Google, Engine::YouTube]
        );
    }

    #[test]
    fn suggestions_off_in_config_means_no_requests() {
        let recorder = Arc::new(Recorder::default());
        let config = Config {
            suggestions: false,
            ..Config::default()
        };
        let (r, _) = build_registry_with(&config, Default::default(), Some(suggestions(&recorder)));
        r.mode(0).query("rust", 8);
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[test]
    fn builtin_backgrounds_round_trip() {
        let s = BackgroundSource::from_setting("builtin:starry-night");
        assert_eq!(s, BackgroundSource::Builtin("starry-night".into()));
        assert_eq!(s.to_setting(), "builtin:starry-night");
    }

    #[test]
    fn old_wallpaper_setting_becomes_the_default_builtin() {
        assert_eq!(
            BackgroundSource::from_setting("wallpaper"),
            BackgroundSource::Builtin("great-wave".into())
        );
    }

    #[test]
    fn none_and_paths_still_work() {
        assert_eq!(
            BackgroundSource::from_setting("none"),
            BackgroundSource::None
        );
        assert_eq!(BackgroundSource::from_setting(""), BackgroundSource::None);
        assert_eq!(
            BackgroundSource::from_setting(r"C:\Pics\a.jpg"),
            BackgroundSource::File(r"C:\Pics\a.jpg".into())
        );
    }

    #[test]
    fn appearance_defaults() {
        let a = Config::default().appearance;
        assert_eq!(
            (a.background.as_str(), a.tint),
            ("builtin:great-wave", 0.55)
        );
    }

    #[test]
    fn old_config_with_theme_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "hotkey = \"Ctrl+Space\"
[appearance]
theme = \"dracula\"
background = \"wallpaper\"
",
        )
        .unwrap();
        let (c, err) = load_or_create(&path);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(c.hotkey, "Ctrl+Space");
        assert_eq!(
            BackgroundSource::from_setting(&c.appearance.background),
            BackgroundSource::Builtin("great-wave".into())
        );
    }

    #[test]
    fn tint_is_kept_between_zero_and_one() {
        let mut c = Config::default();
        c.appearance.tint = 3.0;
        assert_eq!(c.appearance.clamped_tint(), 1.0);
        c.appearance.tint = -1.0;
        assert_eq!(c.appearance.clamped_tint(), 0.0);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut c = Config::default();
        c.appearance.background = r"C:\Pics\hinata.jpg".into();
        save(&path, &c).unwrap();
        let (loaded, err) = load_or_create(&path);
        assert!(err.is_none());
        assert_eq!(loaded, c);
    }

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!(
            (c.hotkey.as_str(), c.default_mode.as_str(), c.max_results),
            (default_hotkey(), "google", 8)
        );
        assert_eq!(
            c.files.exclude_dir_names,
            [".git", "node_modules", "target", "$RECYCLE.BIN"]
        );
        assert!(c.files.roots.is_empty());
        assert_eq!(c.files.reindex_minutes, 30);
    }

    #[test]
    fn default_hotkey_suits_each_system() {
        // Alt+Space opens the window menu on GNOME and KDE.
        let expected = if cfg!(target_os = "linux") {
            "Ctrl+Alt+Space"
        } else {
            "Alt+Space"
        };
        assert_eq!(default_hotkey(), expected);
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
    }

    #[test]
    fn old_keywords_section_is_ignored() {
        // Config files from before prefixes were removed still load.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "max_results = 5\n\n[keywords]\ngoogle = \"g\"\n").unwrap();
        let (c, err) = load_or_create(&path);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(c.max_results, 5);
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
        assert_eq!(build_registry(&c, Default::default()).1, 0);
    }

    #[test]
    fn registry_has_google_youtube_files_in_order() {
        let (r, default) = build_registry(&Config::default(), Default::default());
        let ids: Vec<_> = (0..r.len()).map(|i| r.mode(i).id()).collect();
        assert_eq!(ids, ["google", "youtube", "files"]);
        assert_eq!(default, 0);
    }
}
