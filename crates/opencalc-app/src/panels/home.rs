//! The Home page, shown when no worksheet is open: a header, ways to start, the
//! recent worksheets, and (in a wide window) a few tips at the side. It is laid out
//! like OpenDraft's, so the family looks alike.

use eframe::egui::{self, Frame, RichText, Stroke, Ui};

use super::Commands;
use crate::app::AppState;
use crate::theme::{metrics, type_size, Theme};

mod layout;
mod recent;
mod start;

use layout::{layout, Layout, COLUMN_GAP};

/// A small upper-case heading above a section.
fn section_label(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
}

pub fn show(ui: &mut Ui, state: &AppState, out: &mut Commands) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            let shown = layout(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.add_space(shown.margin());
                ui.vertical(|ui| {
                    ui.add_space(24.0);
                    header(ui, state, shown.main());
                    ui.add_space(20.0);
                    start::section(ui, &state.theme, shown.main(), out);
                    ui.add_space(12.0);
                    recent::section(ui, state, shown.main(), out);
                    ui.add_space(24.0);
                });
                if let Layout::Wide { side, .. } = shown {
                    ui.add_space(COLUMN_GAP);
                    ui.vertical(|ui| {
                        ui.add_space(24.0 + 64.0);
                        ui.set_width(side);
                        tips(ui, &state.theme, side);
                    });
                }
            });
        });
}

fn header(ui: &mut Ui, state: &AppState, width: f32) {
    let theme = state.theme;
    ui.set_width(width);
    ui.horizontal(|ui| {
        ui.add(crate::logo::image(ui.ctx(), 40.0));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new("OpenCalc").size(26.0).color(theme.text));
            ui.label(
                RichText::new(format!(
                    "Engineering worksheets \u{b7} version {}",
                    env!("CARGO_PKG_VERSION")
                ))
                .color(theme.text_weak),
            );
        });
    });
    ui.add_space(6.0);
    ui.label(
        RichText::new("Drop an .ocalc file on the window to open it.")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
}

/// What to try first: a card at the side.
fn tips(ui: &mut Ui, theme: &Theme, width: f32) {
    section_label(ui, theme, "Getting started");
    ui.add_space(6.0);
    Frame::new()
        .fill(theme.surface)
        .stroke(Stroke::new(1.0_f32, theme.border))
        .corner_radius(metrics::WINDOW_RADIUS)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(width - 26.0);
            for (title, text) in TIPS {
                ui.label(RichText::new(title).color(theme.text));
                ui.label(
                    RichText::new(text)
                        .size(type_size::SMALL)
                        .color(theme.text_weak),
                );
                ui.add_space(8.0);
            }
        });
}

const TIPS: [(&str, &str); 4] = [
    (
        "Click and type",
        "Click anywhere on the page and start typing: it becomes a calculation.",
    ),
    (
        "Define, then calculate",
        "Write F := 10 kN to define a value and F * 2 m to use it. Units are part of the number.",
    ),
    (
        "Right-click for more",
        "Cut, copy, arrange and insert are in the menu; Ctrl+D duplicates, the arrow keys nudge.",
    ),
    (
        "Check your work",
        "Errors and warnings are counted in the status bar; click them to see the list.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::PaletteKey;

    /// Draw the page in a window `width` wide.
    fn draw(state: &AppState, width: f32) -> Vec<crate::commands::CommandId> {
        let ctx = egui::Context::default();
        crate::fonts::install(&ctx);
        let mut out = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 700.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| show(ui, state, &mut out));
            });
        }
        out
    }

    #[test]
    fn the_page_draws_at_every_width_with_and_without_recent_files() {
        let mut state = AppState::new(PaletteKey::Neutral);
        for width in [360.0, 640.0, 900.0, 1280.0, 2400.0] {
            assert!(
                draw(&state, width).is_empty(),
                "nothing is clicked by drawing"
            );
        }
        state.recent = vec![
            "C:/nowhere/beam.ocalc".into(),
            "C:/nowhere/starter.ocalct".into(),
            "D:/also/gone.ocalc".into(),
        ];
        for width in [360.0, 640.0, 900.0, 1280.0, 2400.0] {
            assert!(draw(&state, width).is_empty());
        }
    }
}
