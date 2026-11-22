//! The window's drop-down selectors (patch, ports).

use eframe::egui;

/// One line in a drop-down.
pub enum Item<'a, T> {
    /// A choice: its label, whether it is the current one, and what picking
    /// it gives.
    Choice(&'a str, bool, T),
    /// A line between groups of choices.
    Separator,
}

/// Shows `combo` listing `items`; returns what the choice clicked this frame
/// gives.
pub fn show<'a, T>(
    ui: &mut egui::Ui,
    combo: egui::ComboBox,
    items: impl IntoIterator<Item = Item<'a, T>>,
) -> Option<T> {
    let mut picked = None;
    combo.show_ui(ui, |ui| {
        for item in items {
            match item {
                Item::Choice(label, current, value) => {
                    if ui.selectable_label(current, label).clicked() {
                        picked = Some(value);
                    }
                }
                Item::Separator => {
                    ui.separator();
                }
            }
        }
    });
    picked
}
