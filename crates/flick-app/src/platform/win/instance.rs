//! Makes sure only one Flick runs at a time.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::HSTRING;

use crate::platform::message::Message;

const MUTEX_NAME: &str = r"Local\FlickLauncher";

/// Holds the named mutex; the next Flick may start once this is dropped.
pub struct Guard(HANDLE);

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateMutexW and is closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// `None` when another Flick is already running. On Windows a second
/// Flick just exits, so `on_message` is never called.
pub fn acquire(on_message: impl Fn(Message) + Send + 'static) -> Option<Guard> {
    drop(on_message);
    acquire_named(MUTEX_NAME)
}

/// Windows has no `flick --toggle`; nothing is sent.
pub fn send(msg: Message) -> bool {
    let _ = msg;
    false
}

fn acquire_named(name: &str) -> Option<Guard> {
    // SAFETY: plain Win32 call; the name outlives the call.
    unsafe {
        let handle = CreateMutexW(None, false, &HSTRING::from(name)).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(Guard(handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_fails_while_first_is_held() {
        let name = r"Local\FlickLauncherTest-single";
        let first = acquire_named(name);
        assert!(first.is_some());
        assert!(acquire_named(name).is_none());
        drop(first);
        assert!(acquire_named(name).is_some());
    }
}
