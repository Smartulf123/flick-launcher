//! Windows: Win32 calls, the registry and the shell.

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

/// Nothing to change on Windows.
pub fn configure(_options: &mut eframe::NativeOptions) {}
