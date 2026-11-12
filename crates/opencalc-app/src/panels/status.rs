//! Status bar: the page, the calculation indicator, and at the
//! right the zoom slider. Grid, snap, units and automatic calculation live on the
//! ribbon; errors show on the regions themselves.

use eframe::egui::{self, RichText, Ui, Vec2};

use super::Commands;
use crate::app::AppState;
use crate::canvas::view::{MAX_ZOOM, MIN_ZOOM};
use crate::commands::CommandId;
use crate::ribbon::widgets::{self as w, Look};
use crate::theme::type_size;

pub fn show(ui: &mut Ui, state: &AppState, out: &mut Commands) {
    let theme = state.theme;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
        if state.active.is_some() {
            let (page, pages) = state.page;
            ui.label(
                RichText::new(format!("Page {page} of {pages}"))
                    .color(theme.text_weak)
                    .size(type_size::BODY),
            );
        }
        if state.stale_notice() {
            // Numbers that no longer match the inputs: say so where it cannot be
            // missed, and let a click put it right.
            let text = RichText::new(format!(
                "{}  Results out of date. Recalculate (Ctrl+F9)",
                crate::icons::WARNING
            ))
            .color(theme.warning)
            .strong()
            .size(type_size::BODY);
            if ui
                .add(egui::Button::new(text).frame(false))
                .on_hover_text("Calculate the worksheet now")
                .clicked()
            {
                out.push(CommandId::CalcAll);
            }
        } else {
            ui.label(
                RichText::new(state.calc_label())
                    .color(theme.text_weak)
                    .size(type_size::BODY),
            );
        }
        if state.active_sheet().is_some_and(|s| s.canvas.pen.is_some()) {
            ui.label(
                RichText::new(
                    "Drag to draw. Shift: equal sides. Ctrl: from the centre. Esc: cancel.",
                )
                .color(theme.text)
                .size(type_size::BODY),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.active.is_none() {
                return;
            }
            let percent = state.zoom_percent();
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            // Right to left: the slider is placed last, so the view buttons sit left of it.
            let reset = egui::Button::new(format!("{percent:.0}%")).frame(false);
            if ui.add(reset).on_hover_text("Reset zoom to 100%").clicked() {
                out.push(CommandId::ViewZoom100);
            }
            // The slider is on a log scale so each step is the same ratio.
            let mut value = percent.clamp(MIN_ZOOM * 100.0, MAX_ZOOM * 100.0);
            ui.spacing_mut().slider_width = 140.0;
            let slider = egui::Slider::new(&mut value, MIN_ZOOM * 100.0..=MAX_ZOOM * 100.0)
                .logarithmic(true)
                .show_value(false);
            if ui.add(slider).on_hover_text("Zoom").changed() {
                out.push(CommandId::ZoomTo(value.round() as u16));
            }
            ui.add_space(8.0);
            // Right to left, so Draft View is placed first and Page View lands left of it.
            for (command, on) in [
                (CommandId::ViewDraftView, state.draft),
                (CommandId::ViewPageView, !state.draft),
            ] {
                let look = Look {
                    enabled: true,
                    active: on,
                };
                let width = w::small_width(ui, true, Some(command.label()));
                let label = Some(command.label());
                let button = w::small_button(ui, &theme, look, Some(command.icon()), label, width);
                if button.clicked() {
                    out.push(command);
                }
            }
        });
    });
}
