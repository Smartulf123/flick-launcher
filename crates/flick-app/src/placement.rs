//! Where the window opens, and keeping it on the screen as the result list
//! grows: the window slides up rather than the list scrolling.

use eframe::egui::{Pos2, Vec2, pos2};

/// How far down the screen the window opens, like Spotlight and look.
const TOP_FRACTION: f32 = 0.22;
/// Space kept free under the window.
const BOTTOM_MARGIN: f32 = 16.0;

/// Top-left corner for a window `width` wide on a `monitor`-sized screen.
pub fn initial_position(monitor: Vec2, width: f32) -> Pos2 {
    pos2(
        ((monitor.x - width) / 2.0).round(),
        (monitor.y * TOP_FRACTION).round(),
    )
}

/// The new top edge if a window at `top` that is `height` tall would run
/// past the bottom of a `monitor_height` screen; `None` if it already fits.
/// Never moves the window above the top of the screen.
pub fn keep_on_screen(top: f32, height: f32, monitor_height: f32) -> Option<f32> {
    let bottom_limit = monitor_height - BOTTOM_MARGIN;
    (top + height > bottom_limit).then(|| (bottom_limit - height).max(0.0).round())
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
    fn a_window_that_fits_stays_put() {
        assert_eq!(keep_on_screen(158.0, 500.0, 720.0), None);
    }

    #[test]
    fn a_window_that_runs_off_the_bottom_slides_up() {
        // 158 + 620 = 778 > 720 - 16: move the top to 720 - 16 - 620 = 84.
        assert_eq!(keep_on_screen(158.0, 620.0, 720.0), Some(84.0));
    }

    #[test]
    fn never_above_the_top_of_the_screen() {
        assert_eq!(keep_on_screen(300.0, 900.0, 720.0), Some(0.0));
    }
}
