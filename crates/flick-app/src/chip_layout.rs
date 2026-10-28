//! Where the engine chips sit on the bar, and how they move while one is
//! dragged: the others slide apart to open a gap where it will land.

use eframe::egui::{Pos2, Rect, pos2, vec2};

/// Height of a chip (and of the "+" chip).
pub const HEIGHT: f32 = 26.0;
/// Space between chips, across and down.
pub const GAP: f32 = 6.0;

/// Top-left corners for items of `widths`, left to right from (0, 0),
/// starting a new row when one would pass `max_width`.
pub fn place(widths: &[f32], max_width: f32) -> Vec<Pos2> {
    let mut at = pos2(0.0, 0.0);
    widths
        .iter()
        .map(|&width| {
            if at.x > 0.0 && at.x + width > max_width {
                at = pos2(0.0, at.y + HEIGHT + GAP);
            }
            let here = at;
            at.x += width + GAP;
            here
        })
        .collect()
}

/// Where the chip being dragged lands when let go at `pointer`: after
/// every other chip that comes before the pointer in reading order.
/// `others` are the other chips' places, in bar order.
pub fn drop_index(others: &[Rect], pointer: Pos2) -> usize {
    let half_row = (HEIGHT + GAP) / 2.0;
    others
        .iter()
        .filter(|r| {
            let c = r.center();
            c.y < pointer.y - half_row || ((c.y - pointer.y).abs() <= half_row && c.x < pointer.x)
        })
        .count()
}

/// Every chip's place (in bar order) while chip `from` is dragged to `to`,
/// followed by the place of the item after the chips (the "+" chip).
pub fn places_while_dragging(
    widths: &[f32],
    trailing: f32,
    drag: Option<(usize, usize)>,
    max_width: f32,
) -> Vec<Pos2> {
    let mut order: Vec<usize> = (0..widths.len()).collect();
    if let Some((from, to)) = drag {
        let moved = order.remove(from);
        order.insert(to.min(order.len()), moved);
    }
    let mut in_order: Vec<f32> = order.iter().map(|&i| widths[i]).collect();
    in_order.push(trailing);
    let placed = place(&in_order, max_width);
    let mut places = vec![Pos2::ZERO; widths.len()];
    for (slot, &chip) in order.iter().enumerate() {
        places[chip] = placed[slot];
    }
    places.push(placed[widths.len()]);
    places
}

/// One frame of easing from `pos` toward `target`, `dt` seconds long;
/// lands exactly once within half a pixel.
pub fn ease(pos: Pos2, target: Pos2, dt: f32) -> Pos2 {
    /// How fast chips catch up: about 90% of the way in an eighth of a second.
    const SPEED: f32 = 18.0;
    let next = pos + (target - pos) * (1.0 - (-dt * SPEED).exp());
    if next.distance(target) < 0.5 {
        target
    } else {
        next
    }
}

/// What letting go of an engine carried from the "+" list does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drop {
    /// Takes the place of the chip at this position.
    Replace(usize),
    /// Goes in at this position (the bar has room).
    Insert(usize),
}

/// The drop for `pointer`, given the bar's chips at rest and the `row` they
/// sit in. On a chip it swaps that chip, except near its edges while the bar
/// has `room`, where it goes in beside it; between chips it goes in (with
/// room) or swaps the nearest chip in that row (without); off the row, nothing.
pub fn drop_for(chips: &[Rect], row: Rect, pointer: Pos2, room: bool) -> Option<Drop> {
    if !row.expand(GAP).contains(pointer) {
        return None;
    }
    if let Some(i) = chips.iter().position(|r| r.contains(pointer)) {
        let (r, edge) = (chips[i], chips[i].width() * 0.2);
        return Some(if room && pointer.x < r.left() + edge {
            Drop::Insert(i)
        } else if room && pointer.x > r.right() - edge {
            Drop::Insert(i + 1)
        } else {
            Drop::Replace(i)
        });
    }
    if room {
        return Some(Drop::Insert(drop_index(chips, pointer)));
    }
    let across = |r: &Rect| (r.left() - pointer.x).max(pointer.x - r.right()).max(0.0);
    chips
        .iter()
        .enumerate()
        .filter(|(_, r)| (r.center().y - pointer.y).abs() <= (HEIGHT + GAP) / 2.0)
        .min_by(|(_, a), (_, b)| across(a).total_cmp(&across(b)))
        .map(|(i, _)| Drop::Replace(i))
}

/// The rectangle of an item placed at `at` with `width`.
pub fn rect(at: Pos2, width: f32) -> Rect {
    Rect::from_min_size(at, vec2(width, HEIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_go_left_to_right_and_wrap() {
        assert_eq!(
            place(&[50.0, 60.0, 70.0], 180.0),
            [pos2(0.0, 0.0), pos2(56.0, 0.0), pos2(0.0, HEIGHT + GAP)]
        );
        // One too wide for the row still gets a row of its own.
        assert_eq!(place(&[300.0], 200.0), [pos2(0.0, 0.0)]);
        assert!(place(&[], 200.0).is_empty());
    }

    #[test]
    fn the_drop_place_follows_the_pointer_in_reading_order() {
        let others: Vec<Rect> = place(&[50.0, 50.0, 50.0], 120.0)
            .into_iter()
            .map(|at| rect(at, 50.0))
            .collect();
        // Rows: [0, 1] then [2].
        assert_eq!(drop_index(&others, pos2(-10.0, 10.0)), 0);
        assert_eq!(drop_index(&others, pos2(30.0, 10.0)), 1);
        assert_eq!(drop_index(&others, pos2(200.0, 10.0)), 2);
        assert_eq!(drop_index(&others, pos2(0.0, HEIGHT + GAP + 10.0)), 2);
        assert_eq!(drop_index(&others, pos2(40.0, HEIGHT + GAP + 10.0)), 3);
        assert_eq!(drop_index(&others, pos2(0.0, 500.0)), 3);
    }

    #[test]
    fn dragging_opens_a_gap_where_the_chip_will_land() {
        let widths = [40.0, 50.0, 60.0];
        // Not dragging: chips, then the "+".
        assert_eq!(
            places_while_dragging(&widths, 26.0, None, 500.0),
            [
                pos2(0.0, 0.0),
                pos2(46.0, 0.0),
                pos2(102.0, 0.0),
                pos2(168.0, 0.0)
            ]
        );
        // Chip 0 dragged to the end: 1 and 2 slide left, 0's place is last.
        assert_eq!(
            places_while_dragging(&widths, 26.0, Some((0, 2)), 500.0),
            [
                pos2(122.0, 0.0),
                pos2(0.0, 0.0),
                pos2(56.0, 0.0),
                pos2(168.0, 0.0)
            ]
        );
    }

    fn bar(widths: &[f32]) -> Vec<Rect> {
        place(widths, 500.0)
            .into_iter()
            .zip(widths)
            .map(|(at, &w)| rect(at, w))
            .collect()
    }

    #[test]
    fn a_carried_engine_swaps_the_chip_it_is_let_go_on() {
        // Chips at x 0-100, 106-206, 212-312.
        let chips = bar(&[100.0, 100.0, 100.0]);
        let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, HEIGHT));
        let at = |x: f32| pos2(x, 13.0);
        assert_eq!(
            drop_for(&chips, row, at(50.0), false),
            Some(Drop::Replace(0))
        );
        assert_eq!(
            drop_for(&chips, row, at(160.0), true),
            Some(Drop::Replace(1))
        );
        // Near a chip's edge, with room: in beside it.
        assert_eq!(
            drop_for(&chips, row, at(110.0), true),
            Some(Drop::Insert(1))
        );
        assert_eq!(
            drop_for(&chips, row, at(305.0), true),
            Some(Drop::Insert(3))
        );
        // Between or after chips: in, with room; else the nearest chip.
        assert_eq!(
            drop_for(&chips, row, at(400.0), true),
            Some(Drop::Insert(3))
        );
        assert_eq!(
            drop_for(&chips, row, at(400.0), false),
            Some(Drop::Replace(2))
        );
        assert_eq!(
            drop_for(&chips, row, at(103.0), false),
            Some(Drop::Replace(0))
        );
        // Off the row: nothing.
        assert_eq!(drop_for(&chips, row, pos2(50.0, 200.0), false), None);
        assert_eq!(drop_for(&[], row, at(50.0), true), Some(Drop::Insert(0)));
    }

    #[test]
    fn easing_moves_part_way_then_lands() {
        let (from, to) = (pos2(0.0, 0.0), pos2(100.0, 0.0));
        let step = ease(from, to, 1.0 / 60.0);
        assert!(step.x > 0.0 && step.x < 100.0, "{step:?}");
        assert!(ease(step, to, 1.0 / 60.0).x > step.x);
        assert_eq!(ease(pos2(99.8, 0.0), to, 1.0 / 60.0), to);
        assert_eq!(ease(from, to, 10.0), to);
    }
}
