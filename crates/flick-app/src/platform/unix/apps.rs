//! macOS and Linux: installed apps (`.app` bundles, `.desktop` files).

use std::process::Command;

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
