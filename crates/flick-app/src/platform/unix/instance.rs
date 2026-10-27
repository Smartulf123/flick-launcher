//! One Flick at a time on macOS and Linux, through a lock file and a
//! socket in a folder private to this user.

use std::path::PathBuf;

use crate::platform::message::Message;
use crate::platform::unix_instance::{self, Guard};

/// Linux: the session's runtime folder (private, cleared at logout).
/// macOS, or without one: Flick's cache folder.
fn socket_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    if let Some(run) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(run).join("flick");
    }
    crate::platform::paths::cache_dir()
}

/// `None` when another Flick is already running. If the lock folder
/// cannot be used, Flick still runs, without the one-at-a-time check.
pub fn acquire(on_message: impl Fn(Message) + Send + 'static) -> Option<Guard> {
    let dir = socket_dir();
    match unix_instance::acquire_in(&dir, on_message) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("flick: no single-instance check ({}): {e}", dir.display());
            Some(Guard::detached())
        }
    }
}

/// Tells the running Flick; `false` if there is none.
pub fn send(msg: Message) -> bool {
    unix_instance::send_in(&socket_dir(), msg)
}
