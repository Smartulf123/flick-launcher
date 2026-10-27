//! Flick's colors (Kanagawa), text sizes and the small bits of text the
//! search bar shows.

use eframe::egui::{self, Color32, FontId, TextStyle};

/// The colors Flick draws with.
#[derive(Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    /// Background of the selected row and the active mode chip.
    pub selected: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Kanagawa, Flick's one look (the default theme of look, too).
pub const KANAGAWA: Palette = Palette {
    background: rgb(0x1f1f28),
    border: rgb(0x363646),
    text: rgb(0xdcd7ba),
    muted: rgb(0x727169),
    accent: rgb(0x7e9cd8),
    selected: rgb(0x2d4f67),
};

/// A card's fill over the background picture: the theme background with
/// `tint` (0 to 1) as its opacity.
pub fn card_fill(palette: &Palette, tint: f32) -> Color32 {
    let [r, g, b, _] = palette.background.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (tint.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Behind the mode chips and the gear: nearly solid, so their text stays
/// readable over bright pictures while the rest of the card shows through.
pub fn chip_fill(palette: &Palette) -> Color32 {
    card_fill(palette, 0.85)
}

/// Sets the palette and text sizes. Called at startup and on theme change.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.background;
    visuals.extreme_bg_color = palette.background;
    visuals.override_text_color = Some(palette.text);
    visuals.selection.bg_fill = palette.selected;
    visuals.selection.stroke.color = palette.accent;
    visuals.text_cursor.stroke.color = palette.accent;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_fill_uses_tint_as_alpha() {
        let p = &KANAGAWA;
        let fill = card_fill(p, 0.55);
        assert_eq!(fill.a(), 140);
        assert_eq!(card_fill(p, 1.0), p.background);
    }

    #[test]
    fn chips_are_backed_darker_than_any_usual_card() {
        // The chips and gear keep a near-solid backing, so their text reads
        // on bright pictures even when the card itself is see-through.
        let fill = chip_fill(&KANAGAWA);
        assert!(fill.a() >= 215, "{}", fill.a());
        assert!(fill.a() > card_fill(&KANAGAWA, 0.55).a());
        assert_eq!(fill.to_opaque(), KANAGAWA.background);
    }

    #[test]
    fn placeholder_names_the_mode() {
        assert_eq!(placeholder("Google"), "Search Google");
        assert_eq!(placeholder("YouTube"), "Search YouTube");
        assert_eq!(placeholder("Files"), "Search files and folders");
    }
}
