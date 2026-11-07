//! The Units window: the worksheet's own units, and a form to define one.
//!
//! A unit is defined by what it equals (`hand` = `4 in`), which is evaluated by
//! core, so nobody works out a conversion factor by hand. The form's text lives in
//! egui's memory (it is not document state); every change to the list goes out as
//! an `Edit::SetCustomUnits`, so it is undoable like any other edit.

use eframe::egui::{self, Context, Id, RichText, Ui};
use opencalc_core::edit::Edit;
use opencalc_core::unit_define::define_unit;
use opencalc_core::units::{builtin_symbols, builtin_unit, Quantity, UnitDef};

use super::Commands;
use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::format::format_quantity;
use crate::theme::type_size;
use opencalc_core::registry::UnitSystem;

/// What is typed in the form.
#[derive(Clone, Default)]
pub(crate) struct Form {
    pub symbol: String,
    pub definition: String,
    /// The unit being edited, by position, or `None` when adding.
    pub editing: Option<usize>,
    pub started: bool,
}

/// Where the form's text is kept between frames.
pub(crate) fn form_id() -> Id {
    Id::new("units-form")
}

/// The list with `def` added, or put in place of the unit at `editing`.
fn with_unit(units: &[UnitDef], editing: Option<usize>, def: UnitDef) -> Vec<UnitDef> {
    let mut list = units.to_vec();
    match editing {
        Some(i) if i < list.len() => list[i] = def,
        _ => list.push(def),
    }
    list
}

/// The list without the unit at `index`.
fn without_unit(units: &[UnitDef], index: usize) -> Vec<UnitDef> {
    let mut list = units.to_vec();
    if index < list.len() {
        list.remove(index);
    }
    list
}

/// A unit's value in SI, as text: `0.1016 m`.
fn meaning(def: &UnitDef) -> String {
    let q = Quantity {
        si: def.to_si_scale,
        dim: def.dimension,
    };
    format_quantity(q, UnitSystem::Si)
}

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands, edits: &mut Vec<CanvasCmd>) {
    if !state.units_open {
        return;
    }
    let Some(sheet) = state.active_sheet() else {
        out.push(CommandId::UnitsClose);
        return;
    };
    let units = sheet.doc.custom_units();
    let id = form_id();
    let mut form: Form = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let mut open = true;
    egui::Window::new("Units")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            let weak = state.theme.text_weak;
            heading(ui, "YOUR UNITS", weak);
            list(ui, units, &mut form, edits);
            ui.add_space(6.0);
            heading(
                ui,
                if form.editing.is_some() {
                    "EDIT UNIT"
                } else {
                    "DEFINE A UNIT"
                },
                weak,
            );
            editor(ui, state, units, &mut form, edits);
            ui.add_space(6.0);
            egui::CollapsingHeader::new(
                RichText::new("BUILT-IN UNITS")
                    .size(type_size::SMALL)
                    .color(weak),
            )
            .show(ui, |ui| builtin_list(ui, weak));
        });
    if !open {
        out.push(CommandId::UnitsClose);
        form = Form::default();
    }
    ctx.data_mut(|d| d.insert_temp(id, form));
}

fn heading(ui: &mut Ui, text: &str, colour: egui::Color32) {
    ui.label(RichText::new(text).size(type_size::SMALL).color(colour));
}

/// One edit that replaces the list, as its own undo step.
fn replace(edits: &mut Vec<CanvasCmd>, list: Vec<UnitDef>) {
    edits.push(CanvasCmd::Seal);
    edits.push(CanvasCmd::Edit(Edit::SetCustomUnits(list)));
    edits.push(CanvasCmd::Seal);
}

fn list(ui: &mut Ui, units: &[UnitDef], form: &mut Form, edits: &mut Vec<CanvasCmd>) {
    if units.is_empty() {
        ui.label("None yet. Define one below, then use it in a worksheet like any unit.");
        return;
    }
    egui::Grid::new("units-list")
        .num_columns(4)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (i, def) in units.iter().enumerate() {
                ui.label(RichText::new(&def.symbol).strong());
                let shown = if def.definition.is_empty() {
                    meaning(def)
                } else {
                    format!("{}  ({})", def.definition, meaning(def))
                };
                ui.label(format!("= {shown}"));
                if ui.small_button("Edit").clicked() {
                    *form = Form {
                        symbol: def.symbol.clone(),
                        definition: if def.definition.is_empty() {
                            meaning(def)
                        } else {
                            def.definition.clone()
                        },
                        editing: Some(i),
                        started: true,
                    };
                }
                if ui
                    .small_button("Delete")
                    .on_hover_text("Undo restores it")
                    .clicked()
                {
                    replace(edits, without_unit(units, i));
                    *form = Form::default();
                }
                ui.end_row();
            }
        });
}

fn editor(
    ui: &mut Ui,
    state: &AppState,
    units: &[UnitDef],
    form: &mut Form,
    edits: &mut Vec<CanvasCmd>,
) {
    let (symbol_box, definition_box) = ui
        .horizontal(|ui| {
            let a = ui.add(
                egui::TextEdit::singleline(&mut form.symbol)
                    .hint_text("name, e.g. hand")
                    .desired_width(120.0),
            );
            ui.label("=");
            let b = ui.add(
                egui::TextEdit::singleline(&mut form.definition)
                    .hint_text("what it equals, e.g. 4 in")
                    .desired_width(190.0),
            );
            (a, b)
        })
        .inner;
    if !form.started {
        symbol_box.request_focus();
        form.started = true;
    }
    let result = if form.symbol.trim().is_empty() && form.definition.trim().is_empty() {
        None
    } else {
        Some(define_unit(
            &form.symbol,
            &form.definition,
            units,
            form.editing,
        ))
    };
    match &result {
        Some(Ok(def)) => {
            ui.label(
                RichText::new(format!("1 {} = {}", def.symbol, meaning(def)))
                    .color(state.theme.text_weak),
            );
        }
        Some(Err(e)) => {
            ui.label(RichText::new(e.to_string()).color(state.theme.accent));
        }
        None => {}
    }
    ui.horizontal(|ui| {
        let ready = matches!(result, Some(Ok(_)));
        let label = if form.editing.is_some() {
            "Save"
        } else {
            "Add"
        };
        let add = ui.add_enabled(ready, egui::Button::new(label));
        let enter = (symbol_box.lost_focus() || definition_box.lost_focus())
            && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (add.clicked() || enter) && ready {
            if let Some(Ok(def)) = result {
                replace(edits, with_unit(units, form.editing, def));
                *form = Form::default();
            }
        }
        if form.editing.is_some() && ui.button("Cancel").clicked() {
            *form = Form::default();
        }
    });
}

fn builtin_list(ui: &mut Ui, weak: egui::Color32) {
    egui::Grid::new("units-builtin")
        .num_columns(2)
        .spacing([16.0, 2.0])
        .show(ui, |ui| {
            for symbol in builtin_symbols() {
                if let Some(def) = builtin_unit(symbol) {
                    ui.label(symbol);
                    ui.label(RichText::new(format!("= {}", meaning(&def))).color(weak));
                    ui.end_row();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(symbol: &str, definition: &str) -> UnitDef {
        define_unit(symbol, definition, &[], None).unwrap()
    }

    #[test]
    fn adding_appends_and_editing_replaces_in_place() {
        let a = unit("a", "1 m");
        let b = unit("b", "2 m");
        let list = with_unit(&[], None, a.clone());
        let list = with_unit(&list, None, b.clone());
        assert_eq!(list, [a.clone(), b.clone()]);
        let c = unit("c", "3 m");
        assert_eq!(with_unit(&list, Some(0), c.clone()), [c.clone(), b.clone()]);
        // A stale position adds rather than panics.
        assert_eq!(
            with_unit(&list, Some(9), c.clone()),
            [a.clone(), b.clone(), c]
        );
        assert_eq!(without_unit(&list, 0), [b]);
        assert_eq!(without_unit(&list, 7), list, "out of range removes nothing");
    }

    #[test]
    fn what_a_unit_means_reads_in_si() {
        assert_eq!(meaning(&unit("hand", "4 in")), "101.6 mm");
    }
}
