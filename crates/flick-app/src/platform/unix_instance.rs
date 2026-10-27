//! One Flick at a time on macOS and Linux: the first one holds a lock file
//! and listens on a Unix socket next to it; later ones (including
//! `flick --toggle`) send it one line and exit.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::platform::message::Message;

const LOCK: &str = "flick.lock";
const SOCKET: &str = "flick.sock";

/// Held by the running Flick: the lock, and the socket file to remove on exit.
pub struct Guard {
    _lock: Option<File>,
    socket: Option<PathBuf>,
}

impl Guard {
    /// A guard that holds nothing, for when the lock folder is unusable:
    /// Flick still runs, just without the one-at-a-time check.
    pub fn detached() -> Self {
        Guard {
            _lock: None,
            socket: None,
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(socket) = &self.socket {
            let _ = std::fs::remove_file(socket);
        }
    }
}

/// Takes the lock in `dir` (created private to this user). `Ok(None)` when
/// another Flick holds it. The winner listens on `dir/flick.sock` and calls
/// `on_message` for every line a later Flick sends.
pub fn acquire_in(
    dir: &Path,
    on_message: impl Fn(Message) + Send + 'static,
) -> io::Result<Option<Guard>> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
        Err(std::fs::TryLockError::Error(e)) => return Err(e),
    }
    // We hold the lock, so a socket file left here is from a crash.
    let socket = dir.join(SOCKET);
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    std::thread::Builder::new()
        .name("flick-socket".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut line = String::new();
                if BufReader::new(stream).read_line(&mut line).is_ok()
                    && let Some(msg) = Message::parse(&line)
                {
                    on_message(msg);
                }
            }
        })?;
    Ok(Some(Guard {
        _lock: Some(lock),
        socket: Some(socket),
    }))
}

/// Sends `msg` to the Flick running with `dir`; `false` if none listens.
pub fn send_in(dir: &Path, msg: Message) -> bool {
    UnixStream::connect(dir.join(SOCKET))
        .and_then(|mut stream| writeln!(stream, "{}", msg.as_line()))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn second_acquire_in_the_same_dir_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let first = acquire_in(dir.path(), |_| {}).unwrap();
        assert!(first.is_some());
        assert!(acquire_in(dir.path(), |_| {}).unwrap().is_none());
        drop(first);
        assert!(acquire_in(dir.path(), |_| {}).unwrap().is_some());
    }

    #[test]
    fn send_reaches_the_first_instance() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let _guard = acquire_in(dir.path(), move |msg| {
            let _ = tx.send(msg);
        })
        .unwrap();
        assert!(send_in(dir.path(), Message::Toggle));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(Message::Toggle));
    }

    #[test]
    fn a_leftover_socket_does_not_block_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("flick.sock"), b"").unwrap();
        let (tx, rx) = mpsc::channel();
        let guard = acquire_in(dir.path(), move |msg| {
            let _ = tx.send(msg);
        })
        .unwrap();
        assert!(guard.is_some());
        assert!(send_in(dir.path(), Message::Show));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(Message::Show));
    }

    #[test]
    fn send_without_a_running_flick_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!send_in(dir.path(), Message::Toggle));
    }
}
