//! One Flick at a time on macOS and Linux. Not wired yet: every start
//! runs, and nothing is sent.

use crate::platform::message::Message;

pub struct Guard;

pub fn acquire(_on_message: impl Fn(Message) + Send + 'static) -> Option<Guard> {
    Some(Guard)
}

pub fn send(msg: Message) -> bool {
    log::info!("no running Flick to send {:?} to", msg.as_line());
    false
}
