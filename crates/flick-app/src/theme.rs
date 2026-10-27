//! Theme presets, text sizes and the small bits of text the search bar shows.
//!
//! The presets use the published colors of six popular dark themes, the
//! same family look ships.

use eframe::egui::{self, Color32, FontId, TextStyle};

/// One theme's colors.
#[derive(Debug, PartialEq, Eq)]
pub struct Palette {
    /// Config value, e.g. `"tokyo-night"`.
    pub id: &'static str,
    /// Name in the tray menu.
    pub name: &'static str,
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

pub const PRESETS: [Palette; 6] = [
    Palette {
        id: "kanagawa",
        name: "Kanagawa",
        background: rgb(0x1f1f28),
        border: rgb(0x363646),
        text: rgb(0xdcd7ba),
        muted: rgb(0x727169),
        accent: rgb(0x7e9cd8),
        selected: rgb(0x2d4f67),
    },
    Palette {
        id: "tokyo-night",
        name: "Tokyo Night",
        background: rgb(0x1a1b26),
        border: rgb(0x292e42),
        text: rgb(0xc0caf5),
        muted: rgb(0x565f89),
        accent: rgb(0x7aa2f7),
        selected: rgb(0x283457),
    },
    Palette {
        id: "rose-pine",
        name: "Rose Pine",
        background: rgb(0x191724),
        border: rgb(0x26233a),
        text: rgb(0xe0def4),
        muted: rgb(0x6e6a86),
        accent: rgb(0xc4a7e7),
        selected: rgb(0x403d52),
    },
    Palette {
        id: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        background: rgb(0x1e1e2e),
        border: rgb(0x313244),
        text: rgb(0xcdd6f4),
        muted: rgb(0x6c7086),
        accent: rgb(0xcba6f7),
        selected: rgb(0x45475a),
    },
    Palette {
        id: "gruvbox",
        name: "Gruvbox",
        background: rgb(0x282828),
        border: rgb(0x3c3836),
        text: rgb(0xebdbb2),
        muted: rgb(0x928374),
        accent: rgb(0xfabd2f),
        selected: rgb(0x504945),
    },
    Palette {
        id: "dracula",
        name: "Dracula",
        background: rgb(0x282a36),
        border: rgb(0x44475a),
        text: rgb(0xf8f8f2),
        muted: rgb(0x6272a4),
        accent: rgb(0xbd93f9),
        selected: rgb(0x44475a),
    },
];

/// The preset with this id, or Kanagawa when there is none.
pub fn palette(id: &str) -> &'static Palette {
    PRESETS.iter().find(|p| p.id == id).unwrap_or(&PRESETS[0])
}

pub fn is_known(id: &str) -> bool {
    PRESETS.iter().any(|p| p.id == id)
}

/// A card's fill over the background picture: the theme background with
/// `tint` (0 to 1) as its opacity.
pub fn card_fill(palette: &Palette, tint: f32) -> Color32 {
    let [r, g, b, _] = palette.background.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (tint.clamp(0.0, 1.0) * 255.0) as u8)
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
    fn presets_in_menu_order() {
        let ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "kanagawa",
                "tokyo-night",
                "rose-pine",
                "catppuccin-mocha",
                "gruvbox",
                "dracula"
            ]
        );
    }

    #[test]
    fn unknown_theme_falls_back_to_kanagawa() {
        assert_eq!(palette("dracula").name, "Dracula");
        assert_eq!(palette("no-such-theme").id, "kanagawa");
        assert!(is_known("rose-pine"));
        assert!(!is_known("no-such-theme"));
    }

    #[test]
    fn card_fill_uses_tint_as_alpha() {
        let p = palette("kanagawa");
        let fill = card_fill(p, 0.55);
        assert_eq!(fill.a(), 140);
        assert_eq!(card_fill(p, 1.0), p.background);
    }

    #[test]
    fn placeholder_names_the_mode() {
        assert_eq!(placeholder("Google"), "Search Google");
        assert_eq!(placeholder("YouTube"), "Search YouTube");
        assert_eq!(placeholder("Files"), "Search files and folders");
    }
}
