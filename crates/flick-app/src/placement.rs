//! Where the window opens, and keeping it on the screen as the result list
//! grows: the window slides up rather than the list scrolling.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

/// Where a window of `size` should go to be as close to `wanted` as
/// possible while staying inside `work` (a monitor's usable area). A window
/// too big for `work` starts at its top-left corner.
pub fn place_in(work: Rect, size: Vec2, wanted: Pos2) -> Pos2 {
    let axis = |want: f32, low: f32, high: f32, len: f32| want.min(high - len).max(low);
    pos2(
        axis(wanted.x, work.left(), work.right(), size.x),
        axis(wanted.y, work.top(), work.bottom(), size.y),
    )
}

/// `size`, shrunk so it fits inside `work` with `margin` on every side.
pub fn fit_size(work: Rect, size: Vec2, margin: f32) -> Vec2 {
    vec2(
        size.x.min(work.width() - 2.0 * margin),
        size.y.min(work.height() - 2.0 * margin),
    )
}

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

    fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        Rect::from_min_max(pos2(left, top), pos2(right, bottom))
    }

    #[test]
    fn a_fitting_window_stays_where_wanted() {
        let work = rect(0.0, 0.0, 1920.0, 1040.0);
        assert_eq!(
            place_in(work, vec2(960.0, 640.0), pos2(480.0, 200.0)),
            pos2(480.0, 200.0)
        );
    }

    #[test]
    fn a_monitor_left_of_the_main_one_is_respected() {
        let work = rect(-1920.0, 0.0, 0.0, 1040.0);
        assert_eq!(
            place_in(work, vec2(960.0, 640.0), pos2(-1200.0, 200.0)),
            pos2(-1200.0, 200.0)
        );
    }

    #[test]
    fn right_and_bottom_edges_pull_the_window_back_in() {
        let work = rect(0.0, 0.0, 1280.0, 680.0);
        assert_eq!(
            place_in(work, vec2(960.0, 640.0), pos2(500.0, 100.0)),
            pos2(320.0, 40.0)
        );
    }

    #[test]
    fn a_window_bigger_than_the_screen_starts_at_its_top_left() {
        let work = rect(100.0, 50.0, 1100.0, 650.0);
        assert_eq!(
            place_in(work, vec2(1400.0, 900.0), pos2(400.0, 300.0)),
            pos2(100.0, 50.0)
        );
    }

    #[test]
    fn gallery_shrinks_to_fit_a_small_screen() {
        // 1366 x 768 at 125%: about 1093 x 614 points of work area.
        let work = rect(0.0, 0.0, 1093.0, 614.0);
        assert_eq!(fit_size(work, vec2(960.0, 640.0), 16.0), vec2(960.0, 582.0));
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
