//! What a second `flick` tells the running one: one line, `toggle` or
//! `show`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// `flick --toggle`: show the bar if hidden, hide it if shown.
    Toggle,
    /// Flick was started again while running: show the bar.
    Show,
}

// Written and read by the macOS and Linux socket; Windows sends nothing.
#[cfg_attr(windows, allow(dead_code))]
impl Message {
    pub fn parse(line: &str) -> Option<Message> {
        match line.trim() {
            "toggle" => Some(Message::Toggle),
            "show" => Some(Message::Show),
            _ => None,
        }
    }

    pub fn as_line(self) -> &'static str {
        match self {
            Message::Toggle => "toggle",
            Message::Show => "show",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_two_messages() {
        assert_eq!(Message::parse("toggle\n"), Some(Message::Toggle));
        assert_eq!(Message::parse("show"), Some(Message::Show));
    }

    #[test]
    fn anything_else_is_ignored() {
        assert_eq!(Message::parse("Toggle"), None);
        assert_eq!(Message::parse(""), None);
        assert_eq!(Message::parse("hide"), None);
    }

    #[test]
    fn written_lines_read_back() {
        for msg in [Message::Toggle, Message::Show] {
            assert_eq!(Message::parse(msg.as_line()), Some(msg));
        }
    }
}
