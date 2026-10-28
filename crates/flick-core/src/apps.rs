//! Installed programs: the list Apps mode searches, and the rules for
//! building it. Listing apps is up to each system (`flick-app`'s platform
//! layer); what counts as an app is decided here, so it is tested everywhere.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// One program that can be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    /// What the person searches and sees, e.g. "Spotify".
    pub name: String,
    /// What the system starts: a Windows `AppsFolder` parsing name, a
    /// macOS `.app` path or a Linux `.desktop` file path.
    pub launch: String,
}

/// The app list shared between the UI thread and the scanning thread.
/// `None` until the first scan finishes.
pub type SharedApps = Arc<RwLock<Option<Vec<AppEntry>>>>;

/// Windows' Start list as (display name, parsing name) pairs, without
/// uninstallers, documents and repeated names.
pub fn windows_apps(items: impl IntoIterator<Item = (String, String)>) -> Vec<AppEntry> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|(name, launch)| {
            !name.to_lowercase().contains("uninstall")
                && is_program(launch)
                && seen.insert(name.to_lowercase())
        })
        .map(|(name, launch)| AppEntry { name, launch })
        .collect()
}

/// Store app ids are programs; so are paths to `exe`, `msc` and
/// `appref-ms` files. Other paths (readme files, manuals) and web links
/// are not.
fn is_program(launch: &str) -> bool {
    let lower = launch.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return false;
    }
    if !lower.contains('\\') {
        return true;
    }
    let file = lower.rsplit('\\').next().unwrap_or_default();
    match file.rsplit_once('.') {
        Some((_, ext)) => ["exe", "msc", "appref-ms"].contains(&ext),
        None => true,
    }
}

/// The app's name if a `.desktop` file describes a visible application.
pub fn parse_desktop_entry(text: &str) -> Option<String> {
    let mut in_entry = false;
    let (mut kind, mut name, mut exec, mut hidden) = (None, None, false, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| in_entry) else {
            continue;
        };
        match (key.trim(), value.trim()) {
            ("Type", v) => kind = kind.or(Some(v)),
            ("Name", v) if !v.is_empty() => name = name.or(Some(v)),
            ("Exec", v) => exec |= !v.is_empty(),
            ("NoDisplay" | "Hidden", "true") => hidden = true,
            _ => {}
        }
    }
    (kind == Some("Application") && exec && !hidden)
        .then_some(name?)
        .map(str::to_owned)
}

/// Apps from the `.desktop` files in `dirs`. A file hides one with the
/// same file name in a later folder; unreadable files are skipped.
pub fn desktop_apps(dirs: &[PathBuf]) -> Vec<AppEntry> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for dir in dirs {
        for path in children(dir) {
            let is_desktop = path.extension().is_some_and(|e| e == "desktop");
            let Some(file) = path.file_name().filter(|_| is_desktop) else {
                continue;
            };
            // Even a hidden entry hides the same app in later folders.
            if !seen.insert(file.to_owned()) {
                continue;
            }
            let name = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| parse_desktop_entry(&text));
            if let Some(name) = name {
                apps.push(AppEntry {
                    name,
                    launch: path.to_string_lossy().into_owned(),
                });
            }
        }
    }
    apps
}

/// The entries of `dir`, sorted; none if it cannot be read.
fn children(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.sort();
    paths
}

/// The `.app` bundles directly inside each of `dirs`.
pub fn app_bundles(dirs: &[PathBuf]) -> Vec<AppEntry> {
    dirs.iter()
        .flat_map(|dir| children(dir))
        .filter(|path| {
            path.is_dir()
                && path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("app"))
        })
        .filter_map(|path| {
            Some(AppEntry {
                name: path.file_stem()?.to_string_lossy().into_owned(),
                launch: path.to_string_lossy().into_owned(),
            })
        })
        .collect()
}

/// Where Linux desktops keep `.desktop` files, most personal first.
pub fn linux_app_dirs(home: &Path, xdg_data_dirs: Option<&str>) -> Vec<PathBuf> {
    let data_dirs = xdg_data_dirs
        .filter(|d| !d.trim().is_empty())
        .unwrap_or("/usr/local/share:/usr/share");
    let mut dirs = vec![home.join(".local/share/applications")];
    dirs.extend(
        data_dirs
            .split(':')
            .filter(|d| !d.is_empty())
            .map(|d| Path::new(d).join("applications")),
    );
    dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs.push("/var/lib/snapd/desktop/applications".into());
    dirs
}

/// Where macOS keeps apps.
pub fn mac_app_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        "/Applications",
        "/Applications/Utilities",
        "/System/Applications",
        "/System/Applications/Utilities",
    ]
    .map(PathBuf::from)
    .to_vec();
    dirs.push(home.join("Applications"));
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    fn names(apps: &[AppEntry]) -> Vec<&str> {
        apps.iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn windows_filter_drops_uninstallers_documents_and_repeats() {
        let apps = windows_apps(pairs(&[
            (
                "Calculator",
                "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
            ),
            ("Tool", r"C:\x\tool.exe"),
            ("Readme", r"C:\x\readme.txt"),
            ("Uninstall Foo", r"C:\x\unins000.exe"),
            (
                "Notepad",
                r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\notepad.exe",
            ),
            ("notepad", "Other.Notepad!App"),
            ("Services", r"C:\Windows\system32\services.msc"),
            ("Help site", "https://example.com/help"),
            (
                "Manual",
                r"{F38BF404-1D43-42F2-9305-67DE0B28FC23}\Foo\manual.pdf",
            ),
        ]));
        assert_eq!(names(&apps), ["Calculator", "Tool", "Notepad", "Services"]);
        assert_eq!(
            apps[0].launch,
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        );
    }

    const FIREFOX: &str = "[Desktop Entry]\nType=Application\nName=Firefox\nName[nb]=Firefox nett\nExec=firefox %u\n\n[Desktop Action new-window]\nName=New Window\nExec=firefox --new-window\n";

    #[test]
    fn desktop_entries_that_count() {
        assert_eq!(parse_desktop_entry(FIREFOX).as_deref(), Some("Firefox"));
        let with =
            |extra: &str| format!("[Desktop Entry]\nType=Application\nName=A\nExec=a\n{extra}");
        assert_eq!(parse_desktop_entry(&with("NoDisplay=true")), None);
        assert_eq!(parse_desktop_entry(&with("Hidden=true")), None);
        assert_eq!(
            parse_desktop_entry(&with("NoDisplay=false")).as_deref(),
            Some("A")
        );
        assert_eq!(
            parse_desktop_entry("[Desktop Entry]\nType=Link\nName=A\nURL=https://a.com\n"),
            None
        );
        assert_eq!(
            parse_desktop_entry("[Desktop Entry]\nType=Application\nName=A\n"),
            None
        );
        // An action group's Name never replaces the entry's.
        assert_eq!(
            parse_desktop_entry("[Desktop Action x]\nName=Wrong\n[Desktop Entry]\nType=Application\nName=Right\nExec=r\n").as_deref(),
            Some("Right")
        );
        assert_eq!(parse_desktop_entry("# just a comment\n"), None);
    }

    #[test]
    fn earlier_folders_hide_later_ones_and_bad_files_are_skipped() {
        let mine = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        let entry =
            |name: &str| format!("[Desktop Entry]\nType=Application\nName={name}\nExec=x\n");
        fs::write(mine.path().join("firefox.desktop"), entry("My Firefox")).unwrap();
        fs::write(system.path().join("firefox.desktop"), entry("Firefox")).unwrap();
        fs::write(system.path().join("gimp.desktop"), entry("GIMP")).unwrap();
        fs::write(system.path().join("broken.desktop"), [0xff, 0xfe, 0x00]).unwrap();
        fs::write(system.path().join("notes.txt"), entry("Not an app")).unwrap();
        let dirs = [
            mine.path().to_path_buf(),
            PathBuf::from("/no/such/folder"),
            system.path().to_path_buf(),
        ];
        let mut apps = desktop_apps(&dirs);
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(names(&apps), ["GIMP", "My Firefox"]);
        assert_eq!(
            PathBuf::from(&apps[1].launch),
            mine.path().join("firefox.desktop")
        );
    }

    #[test]
    fn app_bundles_are_found_one_level_deep() {
        let apps_dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(apps_dir.path().join("Safari.app/Contents")).unwrap();
        fs::create_dir_all(apps_dir.path().join("Utilities/Terminal.app")).unwrap();
        fs::write(apps_dir.path().join("notes.txt"), "x").unwrap();
        let found = app_bundles(&[apps_dir.path().to_path_buf(), PathBuf::from("/no/such")]);
        assert_eq!(names(&found), ["Safari"]);
        assert_eq!(
            PathBuf::from(&found[0].launch),
            apps_dir.path().join("Safari.app")
        );
    }

    #[test]
    fn folder_lists_follow_the_spec() {
        let home = Path::new("/home/kim");
        assert_eq!(
            linux_app_dirs(home, None),
            [
                "/home/kim/.local/share/applications",
                "/usr/local/share/applications",
                "/usr/share/applications",
                "/home/kim/.local/share/flatpak/exports/share/applications",
                "/var/lib/flatpak/exports/share/applications",
                "/var/lib/snapd/desktop/applications",
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            linux_app_dirs(home, Some("/opt/share:")),
            [
                "/home/kim/.local/share/applications",
                "/opt/share/applications",
                "/home/kim/.local/share/flatpak/exports/share/applications",
                "/var/lib/flatpak/exports/share/applications",
                "/var/lib/snapd/desktop/applications",
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            mac_app_dirs(Path::new("/Users/kim")),
            [
                "/Applications",
                "/Applications/Utilities",
                "/System/Applications",
                "/System/Applications/Utilities",
                "/Users/kim/Applications",
            ]
            .map(PathBuf::from)
        );
    }
}
