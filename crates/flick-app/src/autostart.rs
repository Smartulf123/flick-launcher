//! Start with Windows: Flick's entry in the per-user Run key.
//!
//! The registry entry is the only record of the setting.

use std::io;
use std::path::Path;

use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

/// A named value under a key in HKEY_CURRENT_USER.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub key: &'static str,
    pub value: &'static str,
}

/// Flick's real entry: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Flick`.
pub const FLICK: Entry = Entry {
    key: r"Software\Microsoft\Windows\CurrentVersion\Run",
    value: "Flick",
};

/// The command Windows runs at login: the exe path in quotes.
pub fn run_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

impl Entry {
    /// The registered command, or `None` when start-with-Windows is off.
    pub fn registered(self) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(self.key, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(self.value))
            .ok()
    }

    pub fn is_enabled(self) -> bool {
        self.registered().is_some()
    }

    /// Whether the entry exists and starts exactly `exe`.
    pub fn points_to(self, exe: &Path) -> bool {
        self.registered().as_deref() == Some(run_command(exe).as_str())
    }

    pub fn enable(self, exe: &Path) -> io::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(self.key)?;
        key.set_value(self.value, &run_command(exe))
    }

    pub fn disable(self) -> io::Result<()> {
        let key =
            match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(self.key, KEY_WRITE) {
                Ok(key) => key,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e),
            };
        match key.delete_value(self.value) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    const TEST_KEY: &str = r"Software\FlickTest\Run";

    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(r"Software\FlickTest");
        }
    }

    #[test]
    fn command_quotes_the_exe_path() {
        assert_eq!(
            run_command(Path::new(r"C:\Program Files\Flick\flick.exe")),
            r#""C:\Program Files\Flick\flick.exe""#
        );
    }

    #[test]
    fn enable_disable_round_trip() {
        let _cleanup = Cleanup;
        let entry = Entry {
            key: TEST_KEY,
            value: "Flick",
        };
        let exe = Path::new(r"C:\Tools\flick.exe");

        assert_eq!(entry.registered(), None);
        entry.enable(exe).unwrap();
        assert_eq!(
            entry.registered().as_deref(),
            Some(r#""C:\Tools\flick.exe""#)
        );
        assert!(entry.points_to(exe));
        assert!(!entry.points_to(Path::new(r"C:\Other\flick.exe")));

        entry.disable().unwrap();
        assert_eq!(entry.registered(), None);
        entry.disable().unwrap(); // already off: still fine
    }
}
