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

pub mod message;
#[cfg(any(unix, test))]
pub mod unix_paths;

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::{
    BarWindow, IconCache, autostart, configure, instance, paths, show_native_menu, with_tray_icon,
};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{
    BarWindow, IconCache, autostart, configure, instance, show_native_menu, with_tray_icon,
};
#[cfg(unix)]
pub use unix_paths as paths;
