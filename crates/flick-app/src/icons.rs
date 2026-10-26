//! Small icons drawn with egui's painter, so the app ships no icon font or images.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};
use flick_core::mode::ResultKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Search,
    Web,
    Video,
    File,
    Folder,
    Info,
}

impl Icon {
    pub fn for_kind(kind: ResultKind) -> Self {
        match kind {
            ResultKind::Web => Icon::Web,
            ResultKind::File => Icon::File,
            ResultKind::Folder => Icon::Folder,
            ResultKind::Info => Icon::Info,
        }
    }

    /// The chip icon for a mode, by its display name.
    pub fn for_mode(name: &str) -> Self {
        match name {
            "Google" => Icon::Web,
            "YouTube" => Icon::Video,
            "Files" => Icon::Folder,
            _ => Icon::Search,
        }
    }

    /// Draws the icon inside `rect` (best square) with line color `color`.
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let s = rect.width().min(rect.height());
        let r = Rect::from_center_size(rect.center(), vec2(s, s));
        let at = |x: f32, y: f32| pos2(r.left() + x * s, r.top() + y * s);
        let stroke = Stroke::new((s / 10.0).max(1.2), color);
        match self {
            Icon::Search => {
                painter.circle_stroke(at(0.42, 0.42), s * 0.26, stroke);
                painter.line_segment([at(0.62, 0.62), at(0.88, 0.88)], stroke);
            }
            Icon::Web => {
                let c = r.center();
                painter.circle_stroke(c, s * 0.40, stroke);
                painter.line_segment([at(0.10, 0.50), at(0.90, 0.50)], stroke);
                // The meridian: an ellipse approximated by a thin polygon outline.
                let meridian: Vec<Pos2> = (0..24)
                    .map(|i| {
                        let t = i as f32 / 24.0 * std::f32::consts::TAU;
                        pos2(c.x + t.cos() * s * 0.17, c.y + t.sin() * s * 0.40)
                    })
                    .collect();
                painter.add(Shape::closed_line(meridian, stroke));
            }
            Icon::Video => {
                let body = Rect::from_min_max(at(0.08, 0.22), at(0.92, 0.78));
                painter.rect_stroke(body, s * 0.14, stroke, StrokeKind::Middle);
                painter.add(Shape::convex_polygon(
                    vec![at(0.42, 0.36), at(0.66, 0.50), at(0.42, 0.64)],
                    color,
                    Stroke::NONE,
                ));
            }
            Icon::Folder => {
                painter.add(Shape::closed_line(
                    vec![
                        at(0.08, 0.26),
                        at(0.38, 0.26),
                        at(0.46, 0.34),
                        at(0.92, 0.34),
                        at(0.92, 0.80),
                        at(0.08, 0.80),
                    ],
                    stroke,
                ));
            }
            Icon::File => {
                painter.add(Shape::closed_line(
                    vec![
                        at(0.22, 0.10),
                        at(0.58, 0.10),
                        at(0.80, 0.32),
                        at(0.80, 0.90),
                        at(0.22, 0.90),
                    ],
                    stroke,
                ));
                painter.line_segment([at(0.58, 0.10), at(0.58, 0.32)], stroke);
                painter.line_segment([at(0.58, 0.32), at(0.80, 0.32)], stroke);
            }
            Icon::Info => {
                painter.circle_stroke(r.center(), s * 0.40, stroke);
                painter.line_segment([at(0.50, 0.44), at(0.50, 0.72)], stroke);
                painter.circle_filled(at(0.50, 0.30), stroke.width * 0.8, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flick_core::mode::ResultKind;

    #[test]
    fn result_kinds_get_their_icon() {
        assert_eq!(Icon::for_kind(ResultKind::Folder), Icon::Folder);
        assert_eq!(Icon::for_kind(ResultKind::File), Icon::File);
        assert_eq!(Icon::for_kind(ResultKind::Web), Icon::Web);
        assert_eq!(Icon::for_kind(ResultKind::Info), Icon::Info);
    }

    #[test]
    fn modes_get_their_icon() {
        assert_eq!(Icon::for_mode("Google"), Icon::Web);
        assert_eq!(Icon::for_mode("YouTube"), Icon::Video);
        assert_eq!(Icon::for_mode("Files"), Icon::Folder);
        assert_eq!(Icon::for_mode("Something new"), Icon::Search);
    }
}
