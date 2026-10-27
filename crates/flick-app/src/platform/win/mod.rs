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
