//! The Names and Units browser (left dock).

use eframe::egui::{RichText, Ui};
use opencalc_core::document::RegionKind;
use opencalc_core::expr::{AssignKind, Expr};

use crate::app::AppState;
use crate::format::format_value;
use crate::theme::type_size;

pub fn show(ui: &mut Ui, state: &AppState) {
    let theme = state.theme;
    ui.label(
        RichText::new("NAMES")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    let Some(sheet) = state.active_sheet() else {
        ui.label(RichText::new("No worksheet open").color(theme.text_weak));
        return;
    };
    let units = sheet.doc.unit_system();
    let mut any = false;
    for r in sheet.doc.regions() {
        let RegionKind::Math(m) = &r.kind else {
            continue;
        };
        let Expr::Assign {
            kind: AssignKind::Define,
            target,
            ..
        } = m.expr()
        else {
            continue;
        };
        any = true;
        let value = match state.output.values.get(&r.id) {
            Some(Ok(q)) => format_value(q, units),
            Some(Err(_)) => "error".to_owned(),
            None => "\u{2026}".to_owned(),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(target).color(theme.text));
            ui.label(
                RichText::new(value)
                    .size(type_size::SMALL)
                    .color(theme.text_weak),
            );
        });
    }
    if !any {
        ui.label(RichText::new("No definitions yet").color(theme.text_weak));
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new("UNITS")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    ui.label(
        RichText::new("mm cm m km in ft\ng kg lb  s\nN kN  Pa kPa MPa")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
}
