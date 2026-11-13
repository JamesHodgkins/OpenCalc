//! The text panel: appears in the Properties side panel while
//! one text region is selected. It holds bold, italic, underline and the text
//! colours. They act on the characters marked in the editor (remembered after a
//! click moves the focus here) or, with none marked, on the whole text.

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Ui, Vec2};
use opencalc_core::document::{RunStyle, TEXT_COLOURS};
use opencalc_core::edit::Edit;

use crate::app::formatting::text_target;
use crate::app::AppState;
use crate::canvas::text_style::COLOURS;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::theme::type_size;

/// Whether a text region is selected, so there are text settings to show.
pub fn wanted(state: &AppState) -> bool {
    text_target(state).is_some()
}

/// The text settings of the selected text region.
pub fn body(ui: &mut Ui, state: &AppState, cmds: &mut Vec<CommandId>, out: &mut Vec<CanvasCmd>) {
    let Some(target) = text_target(state) else {
        return;
    };
    let on = crate::app::formatting::style_commands(state);
    ui.label(
        RichText::new(if target.marked {
            "Applies to the marked characters"
        } else {
            "Applies to all the text"
        })
        .size(type_size::SMALL)
        .color(state.theme.text_weak),
    );
    ui.horizontal(|ui| {
        for (command, label) in [
            (CommandId::FormatBold, "B"),
            (CommandId::FormatItalic, "I"),
            (CommandId::FormatUnderline, "U"),
        ] {
            let text = match command {
                CommandId::FormatBold => RichText::new(label).strong(),
                CommandId::FormatItalic => RichText::new(label).italics(),
                _ => RichText::new(label).underline(),
            };
            if ui
                .add(egui::Button::new(text).selected(on.contains(&command)))
                .clicked()
            {
                cmds.push(command);
            }
        }
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new("Colour")
            .size(type_size::SMALL)
            .color(state.theme.text_weak),
    );
    ui.horizontal_wrapped(|ui| {
        let mut choose = |colour: Option<u8>| {
            out.push(CanvasCmd::Edit(Edit::SetRuns {
                id: target.id,
                runs: target.restyled(|s: &mut RunStyle| s.colour = colour),
            }));
            out.push(CanvasCmd::Seal);
        };
        if ui.button("Default").clicked() {
            choose(None);
        }
        for k in 0..TEXT_COLOURS {
            let (name, colour) = COLOURS[usize::from(k)];
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
            ui.painter().rect_filled(rect, 3.0, colour);
            ui.painter().rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0_f32, Color32::from_black_alpha(80)),
                egui::StrokeKind::Inside,
            );
            if response.on_hover_text(name).clicked() {
                choose(Some(k));
            }
        }
    });
}
