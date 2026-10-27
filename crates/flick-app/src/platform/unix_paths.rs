//! Folders on macOS and Linux, from the system's own settings (`dirs`).
//!
//! | | macOS | Linux |
//! |---|---|---|
//! | settings, log | `~/Library/Application Support/flick` | `$XDG_CONFIG_HOME/flick` |
//! | favorites | `~/Library/Application Support/flick/backgrounds` | `$XDG_DATA_HOME/flick/backgrounds` |
//! | preview cache | `~/Library/Caches/flick` | `$XDG_CACHE_HOME/flick` |

use std::path::PathBuf;

/// `dir/flick`, or a `flick` folder in the temp dir if the system names none.
#[cfg(unix)]
fn flick_in(dir: Option<PathBuf>) -> PathBuf {
    dir.unwrap_or_else(std::env::temp_dir).join("flick")
}

#[cfg(unix)]
pub fn config_path() -> PathBuf {
    flick_in(dirs::config_dir()).join("config.toml")
}

#[cfg(unix)]
pub fn log_path() -> PathBuf {
    flick_in(dirs::config_dir()).join("flick.log")
}

/// Favorites. On macOS the data folder is Application Support, next to
/// the settings, as the spec asks.
#[cfg(unix)]
pub fn backgrounds_dir() -> PathBuf {
    flick_in(dirs::data_dir()).join("backgrounds")
}

#[cfg(unix)]
pub fn cache_dir() -> PathBuf {
    flick_in(dirs::cache_dir())
}

/// Desktop, Documents and Downloads, as the system names them (Linux
/// reads `user-dirs.dirs`, so translated folder names work).
#[cfg(unix)]
pub fn default_roots() -> Vec<PathBuf> {
    default_roots_from([
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
    ])
}

/// Where "Choose your own picture..." opens.
#[cfg(unix)]
pub fn pictures_dir() -> PathBuf {
    dirs::picture_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_default()
}

/// The candidates that are existing folders, in order.
pub fn default_roots_from(candidates: [Option<PathBuf>; 3]) -> Vec<PathBuf> {
    candidates
        .into_iter()
        .flatten()
        .filter(|dir| dir.is_dir())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_roots_keep_existing_folders_in_order() {
        let home = tempfile::tempdir().unwrap();
        let (desktop, documents, downloads) = (
            home.path().join("Desktop"),
            home.path().join("Documents"),
            home.path().join("Downloads"),
        );
        std::fs::create_dir(&desktop).unwrap();
        std::fs::create_dir(&downloads).unwrap();
        assert_eq!(
            default_roots_from([
                Some(desktop.clone()),
                Some(documents),
                Some(downloads.clone())
            ]),
            [desktop, downloads]
        );
    }

    #[test]
    fn no_known_folders_means_no_roots() {
        assert!(default_roots_from([None, None, None]).is_empty());
    }
}
