//! Small icons drawn with egui's painter, so the app ships no icon font or images.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};
use flick_core::mode::ResultKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Search,
    Web,
    /// Brand-style mark in Google's colors (drawn, not the logo file).
    Google,
    /// Brand-style red play button (drawn, not the logo file).
    YouTube,
    File,
    Folder,
    Info,
    /// An installed program: four tiles.
    App,
    /// A gear, for the settings button.
    Settings,
}

const GOOGLE_BLUE: Color32 = Color32::from_rgb(0x42, 0x85, 0xf4);
const GOOGLE_GREEN: Color32 = Color32::from_rgb(0x34, 0xa8, 0x53);
const GOOGLE_YELLOW: Color32 = Color32::from_rgb(0xfb, 0xbc, 0x05);
const GOOGLE_RED: Color32 = Color32::from_rgb(0xea, 0x43, 0x35);
const YOUTUBE_RED: Color32 = Color32::from_rgb(0xff, 0x00, 0x00);

/// A heart in `rect`: filled red when `filled`, else a white outline.
pub fn paint_heart(painter: &Painter, rect: Rect, filled: bool) {
    let s = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), vec2(s, s));
    let at = |x: f32, y: f32| pos2(r.left() + x * s, r.top() + y * s);
    if filled {
        let red = Color32::from_rgb(0xe5, 0x48, 0x4d);
        painter.circle_filled(at(0.33, 0.38), s * 0.2, red);
        painter.circle_filled(at(0.67, 0.38), s * 0.2, red);
        painter.add(Shape::convex_polygon(
            vec![at(0.14, 0.45), at(0.86, 0.45), at(0.5, 0.86)],
            red,
            Stroke::NONE,
        ));
    } else {
        // The classic heart curve, scaled into the square.
        let points: Vec<Pos2> = (0..48)
            .map(|i| {
                let t = i as f32 / 48.0 * std::f32::consts::TAU;
                let x = 16.0 * t.sin().powi(3);
                let y = 13.0 * t.cos()
                    - 5.0 * (2.0 * t).cos()
                    - 2.0 * (3.0 * t).cos()
                    - (4.0 * t).cos();
                at(0.5 + x / 40.0, 0.48 - y / 40.0)
            })
            .collect();
        painter.add(Shape::closed_line(
            points,
            Stroke::new(s * 0.08, Color32::WHITE),
        ));
    }
}

/// Points along a circle from `start` to `end` degrees (0 = right, clockwise).
fn arc(center: Pos2, radius: f32, start: f32, end: f32) -> Vec<Pos2> {
    let steps = ((end - start).abs() / 6.0).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|i| {
            let deg = start + (end - start) * i as f32 / steps as f32;
            let rad = deg.to_radians();
            pos2(center.x + rad.cos() * radius, center.y + rad.sin() * radius)
        })
        .collect()
}

impl Icon {
    pub fn for_kind(kind: ResultKind) -> Self {
        match kind {
            ResultKind::Web => Icon::Web,
            ResultKind::File => Icon::File,
            ResultKind::Folder => Icon::Folder,
            ResultKind::App => Icon::App,
            ResultKind::Info => Icon::Info,
        }
    }

    /// The chip icon for a mode, by its display name.
    pub fn for_mode(name: &str) -> Self {
        match name {
            "Google" => Icon::Google,
            "YouTube" => Icon::YouTube,
            "Files" => Icon::Folder,
            "Apps" => Icon::App,
            _ => Icon::Web,
        }
    }

    /// The mark for a web result, by the site it opens.
    pub fn for_url(url: &str) -> Self {
        if url.contains("youtube.com") {
            Icon::YouTube
        } else if url.contains("google.") {
            Icon::Google
        } else {
            Icon::Web
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
            Icon::YouTube => {
                let body = Rect::from_min_max(at(0.04, 0.18), at(0.96, 0.82));
                painter.rect_filled(body, s * 0.18, YOUTUBE_RED);
                painter.add(Shape::convex_polygon(
                    vec![at(0.40, 0.34), at(0.68, 0.50), at(0.40, 0.66)],
                    Color32::WHITE,
                    Stroke::NONE,
                ));
            }
            Icon::Google => {
                // A "G" as four colored arcs plus the blue bar; the opening
                // sits between -40 and 0 degrees (upper right).
                let c = r.center();
                let radius = s * 0.34;
                let ring = Stroke::new(s * 0.16, GOOGLE_RED);
                for (start, end, colour) in [
                    (200.0, 320.0, GOOGLE_RED),
                    (135.0, 200.0, GOOGLE_YELLOW),
                    (45.0, 135.0, GOOGLE_GREEN),
                    (0.0, 45.0, GOOGLE_BLUE),
                ] {
                    painter.add(Shape::line(
                        arc(c, radius, start, end),
                        Stroke::new(ring.width, colour),
                    ));
                }
                painter.line_segment(
                    [pos2(c.x, c.y), pos2(c.x + radius + ring.width / 2.0, c.y)],
                    Stroke::new(ring.width, GOOGLE_BLUE),
                );
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
            Icon::Settings => {
                let c = r.center();
                for i in 0..8 {
                    let a = (i as f32 * 45.0).to_radians();
                    let dir = vec2(a.cos(), a.sin());
                    painter.line_segment(
                        [c + dir * s * 0.30, c + dir * s * 0.46],
                        Stroke::new(s * 0.14, color),
                    );
                }
                painter.circle_stroke(c, s * 0.30, stroke);
                painter.circle_stroke(c, s * 0.11, stroke);
            }
            Icon::App => {
                for (x, y) in [(0.12, 0.12), (0.54, 0.12), (0.12, 0.54), (0.54, 0.54)] {
                    let tile = Rect::from_min_size(at(x, y), vec2(s * 0.34, s * 0.34));
                    painter.rect_stroke(tile, s * 0.08, stroke, StrokeKind::Inside);
                }
            }
            Icon::Info => {
                painter.circle_stroke(r.center(), s * 0.40, stroke);
                painter.line_segment([at(0.50, 0.44), at(0.50, 0.72)], stroke);
                painter.circle_filled(at(0.50, 0.30), stroke.width * 0.8, color);
            }
        }
    }
}

/// The type label for a file the system gave no name for: "Folder", the
/// extension in capitals ("PDF"), or "File".
pub fn kind_label(path: &std::path::Path, is_dir: bool) -> String {
    if is_dir {
        return "Folder".to_owned();
    }
    path.extension()
        .map(|ext| ext.to_string_lossy().to_uppercase())
        .filter(|ext| !ext.is_empty())
        .unwrap_or_else(|| "File".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flick_core::mode::ResultKind;
    use std::path::Path;

    #[test]
    fn files_without_a_system_type_name_show_their_extension() {
        assert_eq!(kind_label(Path::new("C:/a/report.pdf"), false), "PDF");
        assert_eq!(kind_label(Path::new("C:/a/Makefile"), false), "File");
        assert_eq!(kind_label(Path::new("C:/a/photos"), true), "Folder");
        assert_eq!(kind_label(Path::new("C:/a/old.photos.d"), true), "Folder");
    }

    #[test]
    fn result_kinds_get_their_icon() {
        assert_eq!(Icon::for_kind(ResultKind::Folder), Icon::Folder);
        assert_eq!(Icon::for_kind(ResultKind::File), Icon::File);
        assert_eq!(Icon::for_kind(ResultKind::Web), Icon::Web);
        assert_eq!(Icon::for_kind(ResultKind::Info), Icon::Info);
        assert_eq!(Icon::for_kind(ResultKind::App), Icon::App);
    }

    #[test]
    fn modes_get_their_icon() {
        assert_eq!(Icon::for_mode("Google"), Icon::Google);
        assert_eq!(Icon::for_mode("YouTube"), Icon::YouTube);
        assert_eq!(Icon::for_mode("Files"), Icon::Folder);
        assert_eq!(Icon::for_mode("Apps"), Icon::App);
        // Every other engine is a website.
        assert_eq!(Icon::for_mode("GitHub"), Icon::Web);
    }

    #[test]
    fn web_results_get_the_site_mark() {
        assert_eq!(
            Icon::for_url("https://www.youtube.com/results?search_query=x"),
            Icon::YouTube
        );
        assert_eq!(
            Icon::for_url("https://www.google.com/search?q=x"),
            Icon::Google
        );
        assert_eq!(Icon::for_url("https://example.com/"), Icon::Web);
    }
}
