//! Where Flick keeps its files, and which folders it indexes by default.

use std::path::PathBuf;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Pictures, KNOWN_FOLDER_FLAG,
    SHGetKnownFolderPath,
};
use windows::core::GUID;

fn flick_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("flick")
}

/// `%APPDATA%\flick\config.toml`
pub fn config_path() -> PathBuf {
    flick_dir().join("config.toml")
}

/// `%APPDATA%\flick\flick.log`
pub fn log_path() -> PathBuf {
    flick_dir().join("flick.log")
}

/// `%APPDATA%\flick\backgrounds`, where pictures picked online are saved.
pub fn backgrounds_dir() -> PathBuf {
    flick_dir().join("backgrounds")
}

/// Desktop, Documents and Downloads as Windows reports them, so folders
/// redirected to OneDrive are found. Folders that cannot be resolved are left out.
pub fn default_roots() -> Vec<PathBuf> {
    [FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads]
        .iter()
        .filter_map(known_folder)
        .collect()
}

/// Your Pictures folder (OneDrive-aware), where "Choose a picture..." opens.
pub fn pictures_dir() -> PathBuf {
    known_folder(&FOLDERID_Pictures).unwrap_or_else(|| {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_default()
    })
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    // SAFETY: SHGetKnownFolderPath allocates the string; we copy it and free it once.
    unsafe {
        let raw = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), None).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0 as *const _));
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_live_in_appdata_flick() {
        let appdata = PathBuf::from(std::env::var_os("APPDATA").unwrap());
        assert_eq!(config_path(), appdata.join("flick").join("config.toml"));
        assert_eq!(log_path(), appdata.join("flick").join("flick.log"));
        assert_eq!(backgrounds_dir(), appdata.join("flick").join("backgrounds"));
    }

    #[test]
    fn pictures_dir_exists() {
        assert!(pictures_dir().is_dir(), "{}", pictures_dir().display());
    }

    #[test]
    fn default_roots_are_existing_known_folders() {
        let roots = default_roots();
        assert_eq!(roots.len(), 3, "{roots:?}");
        for root in &roots {
            assert!(root.is_dir(), "{} is not a folder", root.display());
        }
        assert!(roots[2].ends_with("Downloads"), "{roots:?}");
    }
}
