//! The Options window: application settings. For now the colour theme; the
//! default unit system, number format and shortcuts arrive with Milestone 3.

use eframe::egui::{self, Context, RichText};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::theme::{type_size, PaletteKey};

/// The themes on offer: the palette, its command and its name.
pub const THEMES: [(PaletteKey, CommandId); 2] = [
    (PaletteKey::Neutral, CommandId::ThemeNeutral),
    (PaletteKey::Slate, CommandId::ThemeSlate),
];

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.options_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Options")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(
                RichText::new("THEME")
                    .size(type_size::SMALL)
                    .color(state.theme.text_weak),
            );
            ui.label(
                RichText::new("The colour of the application. Worksheet pages stay white.")
                    .size(type_size::SMALL)
                    .color(state.theme.text_weak),
            );
            for (palette, command) in THEMES {
                if ui
                    .selectable_label(state.palette == palette, command.label())
                    .clicked()
                {
                    out.push(command);
                }
            }
            // Only where the system lets a program do this for itself.
            let (supported, registered) = state.file_type;
            if supported {
                ui.add_space(8.0);
                let weak = |text: &str| {
                    RichText::new(text)
                        .size(type_size::SMALL)
                        .color(state.theme.text_weak)
                };
                ui.label(weak("WORKSHEET FILES"));
                if registered {
                    ui.label(weak("Double-clicking an .ocx file opens it in OpenCalc."));
                    if ui.button(CommandId::FileTypeRemove.label()).clicked() {
                        out.push(CommandId::FileTypeRemove);
                    }
                } else {
                    ui.label(weak(
                        "Make double-clicking an .ocx file open it here, with its icon.",
                    ));
                    if ui.button(CommandId::FileTypeRegister.label()).clicked() {
                        out.push(CommandId::FileTypeRegister);
                    }
                }
            }
        });
    if !open {
        out.push(CommandId::AppOptionsClose);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_has_exactly_one_theme_command() {
        for key in [PaletteKey::Neutral, PaletteKey::Slate] {
            assert_eq!(THEMES.iter().filter(|(p, _)| *p == key).count(), 1);
        }
    }
}
