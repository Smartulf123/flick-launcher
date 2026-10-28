//! macOS and Linux: installed apps (`.app` bundles, `.desktop` files).

use std::process::Command;

use flick_core::apps::{self, AppEntry};

/// The `.app` bundles (macOS) or `.desktop` apps (Linux) on this computer.
pub fn scan() -> Vec<AppEntry> {
    let home = dirs::home_dir().unwrap_or_default();
    #[cfg(target_os = "macos")]
    return apps::app_bundles(&apps::mac_app_dirs(&home));
    #[cfg(not(target_os = "macos"))]
    {
        let data_dirs = std::env::var("XDG_DATA_DIRS").ok();
        apps::desktop_apps(&apps::linux_app_dirs(&home, data_dirs.as_deref()))
    }
}

/// The command that starts the app at `path`: `open` on macOS,
/// `gio launch` (GLib) on Linux.
pub fn launch_command(path: &str) -> Command {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(not(target_os = "macos"))]
    let mut command = {
        let mut gio = Command::new("gio");
        gio.arg("launch");
        gio
    };
    command.arg(path);
    command
}

/// Starts the app without waiting for it; a thread reaps the helper.
pub fn launch(path: &str) -> std::io::Result<()> {
    let mut child = launch_command(path).spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}
