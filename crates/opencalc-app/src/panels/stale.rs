//! A notice over the top of the page while its results are out of date (manual
//! calculation, edited since the last calculation). Never exported or printed.

use eframe::egui::{self, Align2, Area, Context, Frame, Id, Order, RichText, Stroke, Vec2};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::icons;
use crate::theme::{metrics, type_size};

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.stale_notice() {
        return;
    }
    let theme = state.theme;
    let at = state.viewport.center_top() + Vec2::new(0.0, 12.0);
    Area::new(Id::new("stale-banner"))
        .order(Order::Foreground)
        .fixed_pos(at)
        .pivot(Align2::CENTER_TOP)
        .show(ctx, |ui| {
            Frame::new()
                .fill(theme.surface)
                .stroke(Stroke::new(1.0_f32, theme.warning))
                .corner_radius(metrics::WINDOW_RADIUS)
                .inner_margin(egui::Margin::symmetric(12, 6))
                .shadow(ctx.style().visuals.popup_shadow)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icons::WARNING).color(theme.warning));
                        ui.label(
                            RichText::new("Results are out of date")
                                .size(type_size::BODY)
                                .color(theme.text),
                        );
                        if ui.button("Recalculate").clicked() {
                            out.push(CommandId::CalcAll);
                        }
                    });
                });
        });
}
