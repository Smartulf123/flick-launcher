//! Start at login on macOS and Linux. Not wired yet: the tray item shows,
//! unticked, and does nothing.

/// The tray menu's name for this setting.
pub const LABEL: &str = if cfg!(target_os = "macos") {
    "Open at Login"
} else {
    "Start at login"
};

pub fn sync(_first_run: bool) {}

pub fn toggle() {
    log::warn!("{LABEL} is not available yet");
}

pub fn is_enabled() -> bool {
    false
}
