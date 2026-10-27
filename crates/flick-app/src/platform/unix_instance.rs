//! One Flick at a time on macOS and Linux: the first one holds a lock file
//! and listens on a Unix socket next to it; later ones (including
//! `flick --toggle`) send it one line and exit.

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
