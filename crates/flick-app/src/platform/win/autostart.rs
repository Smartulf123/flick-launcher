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

/// Turns start-with-Windows on at the very first run, and keeps an existing
/// entry pointing at this exe if Flick has moved.
pub fn sync(first_run: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let result = if first_run {
        log::info!("first run: starting with Windows from {}", exe.display());
        FLICK.enable(&exe)
    } else if FLICK.is_enabled() && !FLICK.points_to(&exe) {
        log::info!("start-with-Windows now points to {}", exe.display());
        FLICK.enable(&exe)
    } else {
        Ok(())
    };
    if let Err(e) = result {
        log::error!("could not update start-with-Windows: {e}");
    }
}

/// The tray menu's name for this setting.
pub const LABEL: &str = "Start with Windows";

pub fn is_enabled() -> bool {
    FLICK.is_enabled()
}

/// The tray checkbox: flip the registry entry.
pub fn toggle() {
    let result = if FLICK.is_enabled() {
        log::info!("start with Windows: off");
        FLICK.disable()
    } else {
        log::info!("start with Windows: on");
        std::env::current_exe().and_then(|exe| FLICK.enable(&exe))
    };
    if let Err(e) = result {
        log::error!("could not change start-with-Windows: {e}");
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
