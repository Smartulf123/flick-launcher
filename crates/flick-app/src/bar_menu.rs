//! The gear menu drawn inside the bar, for systems without a native pop-up
//! (macOS and Linux). Same choices as the Windows pop-up in `tray.rs`.

use eframe::egui::{self, Align, Layout, RichText};

use crate::theme::Palette;
use crate::tray::{BackgroundChoice, TrayCommand};

pub const ITEMS: [(&str, TrayCommand); 4] = [
    (
        "Browse backgrounds...",
        TrayCommand::Background(BackgroundChoice::Browse),
    ),
    (
        "Choose your own picture...",
        TrayCommand::Background(BackgroundChoice::ChooseFile),
    ),
    ("None", TrayCommand::Background(BackgroundChoice::None)),
    ("Change hotkey...", TrayCommand::ChangeHotkey),
];

/// The menu's rows, full width. Returns the clicked choice.
pub fn rows(ui: &mut egui::Ui, p: &Palette) -> Option<TrayCommand> {
    let mut picked = None;
    ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
        for (label, command) in ITEMS {
            let row = egui::Button::new(RichText::new(label).color(p.text))
                .frame(false)
                .min_size(egui::vec2(0.0, 30.0));
            if ui
                .add(row)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                picked = Some(command);
            }
        }
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_the_background_choices_and_change_hotkey() {
        let labels: Vec<_> = ITEMS.iter().map(|(label, _)| *label).collect();
        assert_eq!(
            labels,
            [
                "Browse backgrounds...",
                "Choose your own picture...",
                "None",
                "Change hotkey..."
            ]
        );
        assert_eq!(ITEMS[3].1, TrayCommand::ChangeHotkey);
    }
}
