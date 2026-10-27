//! Start at login on macOS (a LaunchAgent) and Linux (an autostart
//! `.desktop` file): what the file says, and turning it on and off. The
//! file is the only record of the setting.

use std::io;
use std::path::{Path, PathBuf};

/// The LaunchAgent's label, also the bundle identifier.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const LABEL_ID: &str = "io.github.smartulf123.flick";

/// The program to start: inside an AppImage the running binary lives on a
/// temporary mount, so the AppImage file itself (`$APPIMAGE`) is used.
pub fn self_command(appimage: Option<&str>, exe: &Path) -> PathBuf {
    match appimage {
        Some(file) if !file.is_empty() => PathBuf::from(file),
        _ => exe.to_path_buf(),
    }
}

/// `~/.config/autostart/flick.desktop`. The path is quoted for the
/// Desktop Entry `Exec` key: `"`, `` ` ``, `$` and `\` get a backslash,
/// and `%` (a field code) is doubled.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn desktop_entry(exe: &Path) -> String {
    let mut quoted = String::new();
    for c in exe.to_string_lossy().chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    format!(
        "[Desktop Entry]\nType=Application\nName=Flick\nComment=Keyboard launcher\nExec=\"{quoted}\"\nX-GNOME-Autostart-enabled=true\n"
    )
}

/// `~/Library/LaunchAgents/io.github.smartulf123.flick.plist`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn launch_agent_plist(exe: &Path) -> String {
    let exe = exe
        .to_string_lossy()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL_ID}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#
    )
}

/// A start-at-login file and how to write it.
pub struct Entry {
    pub file: PathBuf,
    pub contents: fn(&Path) -> String,
}

impl Entry {
    pub fn is_enabled(&self) -> bool {
        self.file.is_file()
    }

    /// Whether the file exists and starts exactly `exe`.
    pub fn points_to(&self, exe: &Path) -> bool {
        std::fs::read_to_string(&self.file).is_ok_and(|text| text == (self.contents)(exe))
    }

    pub fn enable(&self, exe: &Path) -> io::Result<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.file, (self.contents)(exe))
    }

    /// Removes the file; a missing file is fine.
    pub fn disable(&self) -> io::Result<()> {
        match std::fs::remove_file(&self.file) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn an_appimage_starts_from_its_own_file_not_its_mount() {
        let exe = Path::new("/tmp/.mount_x/usr/bin/flick");
        assert_eq!(
            self_command(Some("/home/a/Flick.AppImage"), exe),
            PathBuf::from("/home/a/Flick.AppImage")
        );
        assert_eq!(self_command(Some(""), exe), exe);
        assert_eq!(self_command(None, exe), exe);
    }

    #[test]
    fn desktop_entry_quotes_the_program_path() {
        let entry = desktop_entry(Path::new("/home/a b/flick"));
        assert!(entry.starts_with("[Desktop Entry]\n"), "{entry}");
        assert!(entry.contains("\nExec=\"/home/a b/flick\"\n"), "{entry}");
        assert!(
            entry.contains("\nX-GNOME-Autostart-enabled=true\n"),
            "{entry}"
        );
    }

    #[test]
    fn desktop_entry_escapes_shell_and_field_characters() {
        let entry = desktop_entry(Path::new("/home/$me/100%/fl\"ick"));
        assert!(
            entry.contains("Exec=\"/home/\\$me/100%%/fl\\\"ick\""),
            "{entry}"
        );
    }

    #[test]
    fn launch_agent_runs_flick_at_load() {
        let plist = launch_agent_plist(Path::new(
            "/Applications/Flick & Co.app/Contents/MacOS/flick",
        ));
        assert!(
            plist.contains("<string>io.github.smartulf123.flick</string>"),
            "{plist}"
        );
        assert!(
            plist
                .contains("<string>/Applications/Flick &amp; Co.app/Contents/MacOS/flick</string>"),
            "{plist}"
        );
        assert!(
            plist.contains("<key>RunAtLoad</key>\n    <true/>"),
            "{plist}"
        );
    }

    #[test]
    fn entry_turns_on_and_off() {
        let dir = tempfile::tempdir().unwrap();
        let entry = Entry {
            file: dir.path().join("autostart").join("flick.desktop"),
            contents: desktop_entry,
        };
        let exe = Path::new("/opt/flick/flick");
        assert!(!entry.is_enabled());
        entry.enable(exe).unwrap();
        assert!(entry.is_enabled());
        assert!(entry.points_to(exe));
        assert!(!entry.points_to(Path::new("/elsewhere/flick")));
        entry.disable().unwrap();
        entry.disable().unwrap();
        assert!(!entry.is_enabled());
    }
}
