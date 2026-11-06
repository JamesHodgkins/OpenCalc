//! Status bar: icon toggles in a fixed order (grid, snap, live recalculation,
//! unit system, warnings, zoom), then the calculation status at the right.

use eframe::egui::{self, RichText, Ui, Vec2};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::icons;
use crate::ribbon::widgets::{self as w, Look};
use crate::theme::type_size;

pub fn show(ui: &mut Ui, state: &AppState, out: &mut Commands) {
    let theme = state.theme;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
        let toggles = [
            (CommandId::ViewGrid, state.show_grid, "Show grid"),
            (CommandId::ViewSnap, state.snap, "Snap to grid"),
            (
                CommandId::ViewLive,
                state.live_recalc,
                "Recalculate as you type",
            ),
        ];
        for (command, on, tip) in toggles {
            let label = match command {
                CommandId::ViewLive => "Live",
                other => other.label(),
            };
            let width = w::small_width(ui, true, Some(label));
            let look = Look {
                enabled: true,
                active: on,
            };
            let button =
                w::small_button(ui, &theme, look, Some(command.icon()), Some(label), width);
            if button.on_hover_text(tip).clicked() {
                out.push(command);
            }
        }
        // Shown but disabled: per-worksheet unit systems are not built yet (TD-09).
        // A control that looks live but does nothing is worse than an honest stub.
        let units = Look {
            enabled: false,
            active: false,
        };
        let width = w::small_width(ui, true, Some("SI"));
        w::small_button(ui, &theme, units, Some(icons::RULER), Some("SI"), width)
            .on_hover_text("Unit systems are not available yet. All values are SI internally.");

        let n = state.output.warnings.len();
        let colour = if n > 0 {
            theme.warning
        } else {
            theme.text_weak
        };
        ui.label(
            RichText::new(format!("{} {n}", icons::WARNING))
                .color(colour)
                .size(type_size::BODY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let zoom = egui::Button::new(format!("{:.0}%", state.zoom_percent())).frame(false);
            if ui.add(zoom).on_hover_text("Reset zoom").clicked() {
                out.push(CommandId::ViewZoom100);
            }
            ui.label(
                RichText::new(state.status_text())
                    .size(type_size::SMALL)
                    .color(theme.text_weak),
            );
        });
    });
}
