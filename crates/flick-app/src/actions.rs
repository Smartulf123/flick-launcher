//! Carries out a result's action with the program Windows has set as default.

use flick_core::mode::Action;

/// Opens URLs in the default browser and files with their associated app.
/// Returns without waiting for the program to start.
pub fn run_action(action: &Action) -> std::io::Result<()> {
    match action {
        Action::OpenUrl(url) => open::that_detached(url),
        Action::OpenPath(path) => open::that_detached(path),
        Action::Launch(target) => crate::platform::apps::launch(target),
        Action::None => Ok(()),
    }
}
