//! Start at login on macOS (a LaunchAgent) and Linux (an autostart
//! `.desktop` file), as Windows does with its Run key: on at the first run,
//! kept pointing at Flick if it moves, flipped by the tray checkbox.

use std::path::PathBuf;

use crate::platform::unix_autostart::{Entry, self_command};

/// The tray menu's name for this setting.
pub const LABEL: &str = if cfg!(target_os = "macos") {
    "Open at Login"
} else {
    "Start at login"
};

#[cfg(target_os = "macos")]
fn entry() -> Option<Entry> {
    use crate::platform::unix_autostart::{LABEL_ID, launch_agent_plist};
    let file = dirs::home_dir()?
        .join("Library/LaunchAgents")
        .join(format!("{LABEL_ID}.plist"));
    Some(Entry {
        file,
        contents: launch_agent_plist,
    })
}

#[cfg(not(target_os = "macos"))]
fn entry() -> Option<Entry> {
    use crate::platform::unix_autostart::desktop_entry;
    Some(Entry {
        file: dirs::config_dir()?.join("autostart/flick.desktop"),
        contents: desktop_entry,
    })
}

/// This program, as the system should start it.
fn program() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(self_command(
        std::env::var("APPIMAGE").ok().as_deref(),
        &exe,
    ))
}

/// Turns start-at-login on at the very first run, and keeps an existing
/// entry pointing at this program if Flick has moved.
pub fn sync(first_run: bool) {
    let (Some(entry), Some(program)) = (entry(), program()) else {
        return;
    };
    let result = if first_run {
        log::info!("first run: {LABEL} from {}", program.display());
        entry.enable(&program)
    } else if entry.is_enabled() && !entry.points_to(&program) {
        log::info!("{LABEL} now points to {}", program.display());
        entry.enable(&program)
    } else {
        Ok(())
    };
    if let Err(e) = result {
        log::error!("could not update {LABEL}: {e}");
    }
}

/// The tray checkbox: write or remove the file.
pub fn toggle() {
    let Some(entry) = entry() else {
        log::error!("{LABEL}: no home folder");
        return;
    };
    let result = if entry.is_enabled() {
        log::info!("{LABEL}: off");
        entry.disable()
    } else {
        log::info!("{LABEL}: on");
        match program() {
            Some(program) => entry.enable(&program),
            None => Ok(()),
        }
    };
    if let Err(e) = result {
        log::error!("could not change {LABEL}: {e}");
    }
}

pub fn is_enabled() -> bool {
    entry().is_some_and(|entry| entry.is_enabled())
}
