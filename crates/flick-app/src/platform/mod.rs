//! Everything that differs between Windows, macOS and Linux. The rest of
//! Flick calls only what this module exports:
//!
//! - `paths`: where settings, favorites and the cache live, and which
//!   folders are searched by default.
//! - `autostart`: starting at login (`LABEL`, `sync`, `toggle`, `is_enabled`).
//! - `instance`: one Flick at a time (`acquire`, `send`).
//! - `BarWindow`: showing, hiding and placing the bar.
//! - `IconCache`: file icons and type names, where the system has them.
//! - `show_native_menu`: the gear menu as a system pop-up, where possible.
//! - `configure`: system-specific window options.
//! - `with_tray_icon`: sets the tray icon (a template image on macOS).
//! - `system_shortcut`, `apply_system_shortcut`: where the desktop, not
//!   Flick, catches the hotkey (Linux on Wayland).

#[cfg(any(target_os = "linux", test))]
mod gnome;
pub mod message;
#[cfg(any(unix, test))]
mod unix_autostart;
#[cfg(unix)]
mod unix_instance;
#[cfg(any(unix, test))]
pub mod unix_paths;

/// Where apps cannot grab a global key (Linux on Wayland), the desktop
/// runs `flick --toggle` on a key instead.
// Only Linux creates these; elsewhere Flick always grabs the key itself.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemShortcut {
    /// Flick writes GNOME's custom shortcut itself.
    Gnome,
    /// The person binds `flick --toggle` in their desktop's settings.
    Manual,
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::{
    BarWindow, IconCache, apply_system_shortcut, autostart, configure, instance, paths,
    show_native_menu, system_shortcut, with_tray_icon,
};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{
    BarWindow, IconCache, apply_system_shortcut, autostart, configure, instance, show_native_menu,
    system_shortcut, with_tray_icon,
};
#[cfg(unix)]
pub use unix_paths as paths;
