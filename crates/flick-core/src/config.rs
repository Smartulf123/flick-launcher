//! `config.toml`: hotkey, default mode, file-search roots and appearance.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::apps::SharedApps;
use crate::catalog::{self, CustomEngine, Target};
use crate::index::SharedIndex;
use crate::modes::suggest::Suggestions;
use crate::modes::{AppsMode, FilesMode, WebSearchMode};
use crate::registry::{ModeRegistry, Modes};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Global hotkey, e.g. `"Alt+Space"` or `"Ctrl+Space"`.
    pub hotkey: String,
    /// Engine the bar opens in, by id (e.g. `"google"`); the first engine
    /// on the bar when it is not one of them.
    pub default_mode: String,
    pub max_results: usize,
    /// Show suggestions while typing (Google, YouTube, Wikipedia,
    /// DuckDuckGo). Sends what you type to that engine, as a browser
    /// search box does.
    pub suggestions: bool,
    /// The engines on the bar, by id, in Tab order (1 to 5). Use `chips()`
    /// for the cleaned list.
    pub engines: Vec<String>,
    pub files: FilesConfig,
    pub appearance: Appearance,
    /// Engines people added themselves.
    pub custom_engines: Vec<CustomEngine>,
}

/// The bar holds at least one engine and at most five.
pub const MIN_ENGINES: usize = 1;
pub const MAX_ENGINES: usize = 5;

/// The engines a new settings file starts with.
pub const DEFAULT_ENGINES: [&str; 5] = ["google", "youtube", "files", "apps", "wikipedia"];

/// Why an engine could not be added to or removed from the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineChange {
    /// Already five engines.
    Full,
    /// It is the only engine left.
    LastOne,
    /// No engine has that id.
    Unknown,
    AlreadyThere,
}

impl Config {
    /// Whether `id` names a catalog engine or one of the custom engines.
    pub fn knows_engine(&self, id: &str) -> bool {
        catalog::find(id).is_some() || self.custom_engines.iter().any(|c| c.id() == id)
    }

    /// The search address (with `{query}`) of a website engine, built-in
    /// or custom; `None` for Files, Apps and unknown ids.
    pub fn engine_url(&self, id: &str) -> Option<String> {
        if let Some(spec) = catalog::find(id) {
            return match spec.target {
                Target::Web { url, .. } => Some(url.to_owned()),
                Target::Files | Target::Apps => None,
            };
        }
        self.custom_engines
            .iter()
            .find(|c| c.id() == id)
            .map(|c| c.url.clone())
    }

    /// The bar's engines: unknown ids and repeats dropped, at most five;
    /// the default list if nothing is left.
    pub fn chips(&self) -> Vec<String> {
        let mut chips: Vec<String> = Vec::new();
        for id in &self.engines {
            if self.knows_engine(id) && !chips.contains(id) && chips.len() < MAX_ENGINES {
                chips.push(id.clone());
            }
        }
        if chips.is_empty() {
            chips = DEFAULT_ENGINES.map(String::from).to_vec();
        }
        chips
    }

    /// Puts engine `id` at the end of the bar.
    pub fn add_engine(&mut self, id: &str) -> Result<(), EngineChange> {
        let mut chips = self.chips();
        if !self.knows_engine(id) {
            return Err(EngineChange::Unknown);
        }
        if chips.iter().any(|c| c == id) {
            return Err(EngineChange::AlreadyThere);
        }
        if chips.len() >= MAX_ENGINES {
            return Err(EngineChange::Full);
        }
        chips.push(id.to_owned());
        self.engines = chips;
        Ok(())
    }

    /// Takes engine `id` off the bar (it stays in the "+" list).
    pub fn remove_engine(&mut self, id: &str) -> Result<(), EngineChange> {
        let mut chips = self.chips();
        let Some(at) = chips.iter().position(|c| c == id) else {
            return Err(EngineChange::Unknown);
        };
        if chips.len() <= MIN_ENGINES {
            return Err(EngineChange::LastOne);
        }
        chips.remove(at);
        self.engines = chips;
        Ok(())
    }

    /// Moves the engine at position `from` on the bar to position `to`.
    pub fn move_engine(&mut self, from: usize, to: usize) {
        let mut chips = self.chips();
        if from < chips.len() {
            let id = chips.remove(from);
            chips.insert(to.min(chips.len()), id);
        }
        self.engines = chips;
    }

    /// Adds a checked custom engine, and puts it on the bar if there is room.
    pub fn add_custom(&mut self, engine: CustomEngine) {
        let id = engine.id();
        self.custom_engines.push(engine);
        let _ = self.add_engine(&id);
    }

    /// Deletes a custom engine and takes it off the bar.
    pub fn remove_custom(&mut self, id: &str) {
        let chips: Vec<String> = self.chips().into_iter().filter(|c| c != id).collect();
        self.custom_engines.retain(|c| c.id() != id);
        self.engines = chips;
    }
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
            engines: DEFAULT_ENGINES.map(String::from).to_vec(),
            files: FilesConfig::default(),
            appearance: Appearance::default(),
            custom_engines: Vec::new(),
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

/// Registers every engine (the catalog, then custom engines) and works
/// out the bar's engines and the engine the bar opens in.
pub fn build_modes(config: &Config, index: SharedIndex, apps: SharedApps) -> Modes {
    build_modes_with(config, index, apps, None)
}

/// Like `build_modes`, giving engines that have type-ahead their
/// suggestions when `config.suggestions` is on and a source is given.
pub fn build_modes_with(
    config: &Config,
    index: SharedIndex,
    apps: SharedApps,
    suggestions: Option<Suggestions>,
) -> Modes {
    let suggestions = suggestions.filter(|_| config.suggestions);
    let mut registry = ModeRegistry::new();
    for spec in catalog::CATALOG {
        match (spec.target, WebSearchMode::from_spec(spec)) {
            (Target::Files, _) => registry.register(Box::new(FilesMode::new(index.clone()))),
            (Target::Apps, _) => registry.register(Box::new(AppsMode::new(apps.clone()))),
            (_, Some(mode)) => registry.register(Box::new(match &suggestions {
                Some(s) => mode.with_suggestions(s.clone()),
                None => mode,
            })),
            (_, None) => unreachable!("every catalog engine is a website, Files or Apps"),
        }
    }
    for custom in &config.custom_engines {
        registry.register(Box::new(WebSearchMode::custom(custom)));
    }
    let chips: Vec<usize> = config
        .chips()
        .iter()
        .filter_map(|id| registry.index_of(id))
        .collect();
    let default = registry
        .index_of(&config.default_mode)
        .filter(|i| chips.contains(i))
        .unwrap_or(chips[0]);
    Modes {
        registry,
        chips,
        default,
    }
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
        let m = build_modes_with(
            &Config::default(),
            Default::default(),
            Default::default(),
            Some(suggestions(&recorder)),
        );
        for &chip in &m.chips {
            m.registry.mode(chip).query("rust", 8);
        }
        assert_eq!(
            *recorder.0.lock().unwrap(),
            [Engine::Google, Engine::YouTube, Engine::Wikipedia]
        );
    }

    #[test]
    fn suggestions_off_in_config_means_no_requests() {
        let recorder = Arc::new(Recorder::default());
        let config = Config {
            suggestions: false,
            ..Config::default()
        };
        let m = build_modes_with(
            &config,
            Default::default(),
            Default::default(),
            Some(suggestions(&recorder)),
        );
        m.registry.mode(m.default).query("rust", 8);
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
    fn default_mode_off_the_bar_falls_back_to_the_first_engine() {
        let c = Config {
            default_mode: "bing".into(),
            ..Config::default()
        };
        let m = build_modes(&c, Default::default(), Default::default());
        assert_eq!(m.default, m.chips[0]);
        assert_eq!(m.registry.mode(m.default).id(), "google");
    }

    fn chip_ids(m: &Modes) -> Vec<&str> {
        m.chips.iter().map(|&i| m.registry.mode(i).id()).collect()
    }

    #[test]
    fn every_engine_is_registered_and_the_default_five_are_on_the_bar() {
        let m = build_modes(&Config::default(), Default::default(), Default::default());
        assert_eq!(m.registry.len(), catalog::CATALOG.len());
        assert_eq!(
            chip_ids(&m),
            ["google", "youtube", "files", "apps", "wikipedia"]
        );
        assert_eq!(m.registry.mode(m.default).id(), "google");
    }

    fn crates() -> CustomEngine {
        CustomEngine {
            name: "Crates".into(),
            url: "https://crates.io/search?q={query}".into(),
        }
    }

    #[test]
    fn custom_engines_are_registered_after_the_catalog() {
        let mut c = Config::default();
        c.remove_engine("wikipedia").unwrap();
        c.add_custom(crates());
        let m = build_modes(&c, Default::default(), Default::default());
        assert_eq!(m.registry.len(), catalog::CATALOG.len() + 1);
        assert_eq!(chip_ids(&m).last(), Some(&"custom:crates"));
    }

    #[test]
    fn engines_know_their_search_address() {
        let mut c = Config::default();
        c.add_custom(crates());
        assert_eq!(
            c.engine_url("github").as_deref(),
            Some("https://github.com/search?q={query}")
        );
        assert_eq!(
            c.engine_url("custom:crates").as_deref(),
            Some("https://crates.io/search?q={query}")
        );
        assert_eq!(c.engine_url("files"), None);
        assert_eq!(c.engine_url("apps"), None);
        assert_eq!(c.engine_url("nope"), None);
    }

    #[test]
    fn the_bar_list_is_cleaned() {
        let c = Config {
            engines: [
                "github", "nope", "github", "bing", "x", "maps", "mdn", "reddit", "ebay",
            ]
            .map(String::from)
            .to_vec(),
            ..Config::default()
        };
        assert_eq!(c.chips(), ["github", "bing", "x", "maps", "mdn"]);
        let empty = Config {
            engines: vec!["nope".into()],
            ..Config::default()
        };
        assert_eq!(empty.chips(), DEFAULT_ENGINES);
    }

    #[test]
    fn a_settings_file_from_before_engines_gets_the_default_five() {
        let c: Config = toml::from_str("hotkey = \"Alt+Space\"\n").unwrap();
        assert_eq!(c.chips(), DEFAULT_ENGINES);
    }

    #[test]
    fn adding_respects_the_limits() {
        let mut c = Config::default();
        assert_eq!(c.add_engine("google"), Err(EngineChange::AlreadyThere));
        assert_eq!(c.add_engine("nope"), Err(EngineChange::Unknown));
        // The default five fill the bar.
        assert_eq!(c.add_engine("github"), Err(EngineChange::Full));
        c.remove_engine("wikipedia").unwrap();
        assert_eq!(c.add_engine("github"), Ok(()));
        assert_eq!(c.chips().last().map(String::as_str), Some("github"));
        assert_eq!(c.add_engine("bing"), Err(EngineChange::Full));
    }

    #[test]
    fn removing_keeps_at_least_one() {
        let mut c = Config {
            engines: vec!["google".into(), "files".into()],
            ..Config::default()
        };
        assert_eq!(c.remove_engine("google"), Ok(()));
        assert_eq!(c.chips(), ["files"]);
        assert_eq!(c.remove_engine("files"), Err(EngineChange::LastOne));
        assert_eq!(c.remove_engine("bing"), Err(EngineChange::Unknown));
    }

    #[test]
    fn moving_reorders_the_bar() {
        let mut c = Config::default();
        c.move_engine(0, 2);
        assert_eq!(
            c.chips(),
            ["youtube", "files", "google", "apps", "wikipedia"]
        );
        c.move_engine(4, 0);
        assert_eq!(
            c.chips(),
            ["wikipedia", "youtube", "files", "google", "apps"]
        );
        c.move_engine(1, 99);
        assert_eq!(
            c.chips(),
            ["wikipedia", "files", "google", "apps", "youtube"]
        );
    }

    #[test]
    fn custom_engines_join_the_bar_when_there_is_room_and_leave_with_delete() {
        let mut c = Config::default();
        c.remove_engine("wikipedia").unwrap();
        c.add_custom(crates());
        assert_eq!(c.chips().len(), 5);
        let mut second = crates();
        second.name = "Crates 2".into();
        c.add_custom(second);
        assert_eq!(c.custom_engines.len(), 2);
        assert!(!c.chips().contains(&"custom:crates-2".to_owned()));
        c.remove_custom("custom:crates");
        assert_eq!(c.custom_engines.len(), 1);
        assert_eq!(c.chips(), DEFAULT_ENGINES[..4]);
    }

    #[test]
    fn engines_and_custom_engines_survive_saving() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut c = Config::default();
        c.add_custom(crates());
        c.move_engine(4, 0);
        save(&path, &c).unwrap();
        let (back, err) = load_or_create(&path);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(back, c);
    }
}
