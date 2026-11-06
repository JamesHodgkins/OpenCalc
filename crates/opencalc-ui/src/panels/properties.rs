//! Region properties (right dock): counts, warnings, errors and the selection.

use eframe::egui::{RichText, Ui};
use opencalc_core::document::RegionKind;
use opencalc_core::eval::Warning;

use crate::app::AppState;
use crate::icons;
use crate::theme::type_size;

pub fn show(ui: &mut Ui, state: &AppState) {
    let theme = state.theme;
    ui.label(
        RichText::new("REGION PROPERTIES")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    let Some(sheet) = state.active_sheet() else {
        ui.label(RichText::new("No worksheet open").color(theme.text_weak));
        return;
    };
    ui.label(format!("{} regions", sheet.doc.regions().len()));
    ui.label("Unit system: SI");
    ui.add_space(8.0);
    for w in &state.output.warnings {
        let Warning::Redefinition { name, regions } = w;
        ui.colored_label(
            theme.warning,
            format!(
                "{} '{name}' defined {} times",
                icons::WARNING,
                regions.len()
            ),
        );
    }
    for r in sheet.doc.regions() {
        if let (Some(Err(e)), RegionKind::Math(m)) = (state.output.values.get(&r.id), &r.kind) {
            ui.colored_label(
                theme.error,
                format!("{} {}: {e}", icons::WARNING, m.source()),
            );
        }
    }
    match sheet.canvas.selection.as_slice() {
        [] => {}
        [id] => {
            if let Some(r) = sheet.doc.region(*id) {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("SELECTED")
                        .size(type_size::SMALL)
                        .color(theme.text_weak),
                );
                ui.label(format!("{} region", r.kind.label()));
                ui.label(format!("x {:.0}  y {:.0}", r.pos.x, r.pos.y));
                ui.label(format!("{:.0} \u{d7} {:.0}", r.size.w, r.size.h));
            }
        }
        many => {
            ui.add_space(8.0);
            ui.label(format!("{} regions selected", many.len()));
        }
    }
}
