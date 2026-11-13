//! Small controls shared by the panels.

use eframe::egui::{self, Ui};

/// A drop-down list of choices.
pub fn combo<T: Copy + PartialEq>(ui: &mut Ui, salt: &str, value: &mut T, options: &[(T, String)]) {
    let shown = options
        .iter()
        .find(|(o, _)| o == value)
        .map_or(String::new(), |(_, l)| l.clone());
    egui::ComboBox::from_id_salt(salt)
        .width(140.0)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (option, label) in options {
                ui.selectable_value(value, *option, label.as_str());
            }
        });
}
