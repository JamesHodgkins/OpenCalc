//! Status bar, after Mathcad's: the page, the calculation indicator, and at the
//! right the zoom slider. Grid, snap, units and automatic calculation live on the
//! ribbon; errors show on the regions themselves.

use eframe::egui::{self, RichText, Ui, Vec2};

use super::Commands;
use crate::app::AppState;
use crate::canvas::view::{MAX_ZOOM, MIN_ZOOM};
use crate::commands::CommandId;
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
        ui.label(
            RichText::new(state.calc_label())
                .color(theme.text_weak)
                .size(type_size::BODY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.active.is_none() {
                return;
            }
            let percent = state.zoom_percent();
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
        });
    });
}
