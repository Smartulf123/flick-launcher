//! Where the window opens and how tall the result list may get, so the
//! bar never runs off the bottom of the screen.

use eframe::egui::{Pos2, Vec2, pos2};

/// How far down the screen the window opens, like Spotlight and look.
const TOP_FRACTION: f32 = 0.22;
/// Space kept free under the window.
const BOTTOM_MARGIN: f32 = 24.0;
/// Height of everything except the list: search card, gaps, hint line.
const OVERHEAD: f32 = 170.0;
/// The list never gets shorter than this (about three rows); it scrolls.
pub const MIN_LIST_HEIGHT: f32 = 140.0;

/// Top-left corner for a window `width` wide on a `monitor`-sized screen.
pub fn initial_position(monitor: Vec2, width: f32) -> Pos2 {
    pos2(
        ((monitor.x - width) / 2.0).round(),
        (monitor.y * TOP_FRACTION).round(),
    )
}

/// The tallest the result list may be with the window's top at `window_top`.
pub fn list_max_height(monitor_height: f32, window_top: f32) -> f32 {
    (monitor_height - window_top - BOTTOM_MARGIN - OVERHEAD).max(MIN_LIST_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    #[test]
    fn opens_centered_a_bit_above_the_middle() {
        assert_eq!(
            initial_position(vec2(1920.0, 1080.0), 640.0),
            pos2(640.0, 238.0)
        );
        assert_eq!(
            initial_position(vec2(1280.0, 720.0), 640.0),
            pos2(320.0, 158.0)
        );
    }

    #[test]
    fn list_gets_the_room_below_the_window() {
        // 1080 high, window top at 238: 1080 - 238 - 24 margin - 170 for the rest.
        assert_eq!(list_max_height(1080.0, 238.0), 648.0);
        assert_eq!(list_max_height(720.0, 158.0), 368.0);
    }

    #[test]
    fn list_keeps_a_few_rows_when_dragged_low() {
        assert_eq!(list_max_height(720.0, 650.0), MIN_LIST_HEIGHT);
    }
}
