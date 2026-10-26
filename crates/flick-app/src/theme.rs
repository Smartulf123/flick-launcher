//! Colors, text sizes and the small bits of text the search bar shows.
//!
//! A dark palette in the spirit of look's default Kanagawa theme.

use eframe::egui::{self, Color32, FontId, TextStyle};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x28);
pub const BORDER: Color32 = Color32::from_rgb(0x36, 0x36, 0x46);
pub const TEXT: Color32 = Color32::from_rgb(0xdc, 0xd7, 0xba);
pub const MUTED: Color32 = Color32::from_rgb(0x72, 0x71, 0x69);
pub const ACCENT: Color32 = Color32::from_rgb(0x7e, 0x9c, 0xd8);
/// Background of the selected row and the active mode chip.
pub const SELECTED: Color32 = Color32::from_rgb(0x2d, 0x4f, 0x67);

/// Sets the dark palette and text sizes once at startup.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = BACKGROUND;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = SELECTED;
    visuals.selection.stroke.color = ACCENT;
    visuals.text_cursor.stroke.color = ACCENT;

    // Both the light and dark styles, so a light Windows theme changes nothing.
    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(20.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(13.0)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
    });
}

/// Grey text shown in the empty search box.
pub fn placeholder(mode_name: &str) -> String {
    match mode_name {
        "Files" => "Search files and folders".to_owned(),
        name => format!("Search {name}"),
    }
}

/// The hint line under the results.
pub fn footer(result_count: usize, typing: bool) -> String {
    const GAP: &str = "    ";
    match (typing, result_count) {
        (false, _) => format!("Tab switch mode{GAP}Esc hide"),
        (true, 0) => format!("No results{GAP}Tab switch mode{GAP}Esc hide"),
        (true, n) => {
            let noun = if n == 1 { "result" } else { "results" };
            format!("{n} {noun}{GAP}Up/Down choose{GAP}Enter open{GAP}Esc hide")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_names_the_mode() {
        assert_eq!(placeholder("Google"), "Search Google");
        assert_eq!(placeholder("YouTube"), "Search YouTube");
        assert_eq!(placeholder("Files"), "Search files and folders");
    }

    #[test]
    fn footer_teaches_keys_before_typing() {
        assert_eq!(footer(0, false), "Tab switch mode    Esc hide");
    }

    #[test]
    fn footer_counts_results_while_typing() {
        assert_eq!(
            footer(3, true),
            "3 results    Up/Down choose    Enter open    Esc hide"
        );
        assert_eq!(
            footer(1, true),
            "1 result    Up/Down choose    Enter open    Esc hide"
        );
        assert_eq!(footer(0, true), "No results    Tab switch mode    Esc hide");
    }
}
