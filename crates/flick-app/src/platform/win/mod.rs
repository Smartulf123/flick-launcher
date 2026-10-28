//! Windows: Win32 calls, the registry and the shell.

pub mod apps;
pub mod autostart;
pub mod instance;
mod menu;
pub mod paths;
mod shell_icons;
mod window;

pub use menu::show_native_menu;
pub use shell_icons::IconCache;
pub use window::BarWindow;

/// Windows tray icons keep their colors.
pub fn with_tray_icon(
    icon: tray_icon::Icon,
    builder: tray_icon::TrayIconBuilder,
) -> tray_icon::TrayIconBuilder {
    builder.with_icon(icon)
}

/// Windows always lets Flick grab its hotkey.
pub fn system_shortcut() -> Option<super::SystemShortcut> {
    None
}

/// Never needed on Windows; nothing to do.
pub fn apply_system_shortcut(_shortcut: super::SystemShortcut, _text: &str) -> anyhow::Result<()> {
    Ok(())
}

/// Nothing to change on Windows.
pub fn configure(_options: &mut eframe::NativeOptions) {}
