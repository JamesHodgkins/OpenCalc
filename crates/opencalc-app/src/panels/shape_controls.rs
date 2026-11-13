//! The shape section of the Properties panel: kind, corner radius or sides, fill,
//! outline, width, line style and arrowheads of the selected shape, and its draw
//! order, lock and delete. Like every panel it reads the document and only asks: each change
//! is an edit sent back as a canvas command.

use eframe::egui::{self, Id, RichText, Ui};
use opencalc_core::document::{
    Arrowhead, Dash, RegionId, RegionKind, ShapeKind, ShapeSpec, MAX_RADIUS, MAX_SIDES, MAX_STROKE,
    MIN_SIDES,
};
use opencalc_core::edit::Edit;

use super::colour_picker;
use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::theme::{type_size, Theme};

/// The single selected shape region, if that is what is selected.
pub fn selected_shape(state: &AppState) -> Option<(RegionId, &ShapeSpec)> {
    let sheet = state.active_sheet()?;
    let [id] = sheet.canvas.selection.as_slice() else {
        return None;
    };
    match &sheet.doc.region(*id)?.kind {
        RegionKind::Shape(spec) => Some((*id, spec)),
        _ => None,
    }
}

/// The settings a shape has, one row of the panel each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeControl {
    Kind,
    Fill,
    Outline,
    Width,
    Line,
    Corners,
    StartHead,
    EndHead,
}

impl ShapeControl {
    fn label(self) -> &'static str {
        match self {
            ShapeControl::Kind => "Shape",
            ShapeControl::Fill => "Fill",
            ShapeControl::Outline => "Outline",
            ShapeControl::Width => "Width",
            ShapeControl::Line => "Line",
            ShapeControl::Corners => "Corners",
            ShapeControl::StartHead => "Start",
            ShapeControl::EndHead => "End",
        }
    }
}

const KINDS: [ShapeKind; 6] = [
    ShapeKind::Rectangle,
    ShapeKind::Ellipse,
    ShapeKind::Triangle,
    ShapeKind::Diamond,
    ShapeKind::Polygon,
    ShapeKind::Line,
];

fn dash_label(d: Dash) -> &'static str {
    match d {
        Dash::Solid => "Solid",
        Dash::Dashed => "Dashed",
        Dash::Dotted => "Dotted",
    }
}

fn colour_id(id: RegionId, what: &'static str) -> Id {
    Id::new(("shape-colour", id.0, what))
}

/// Draws the shape controls for the selected shape.
struct ShapeControls<'a> {
    state: &'a AppState,
    out: &'a mut Vec<CanvasCmd>,
}

impl ShapeControls<'_> {
    /// Send the new settings. `done` ends the undo step (a click or a finished
    /// drag); while a value is still being dragged the edits merge.
    fn send(&mut self, id: RegionId, current: &ShapeSpec, next: ShapeSpec, done: bool) {
        if next != *current {
            self.out.push(CanvasCmd::Edit(Edit::SetShape {
                id,
                spec: Box::new(next),
            }));
        }
        if done {
            self.out.push(CanvasCmd::Seal);
        }
    }
}

impl ShapeControls<'_> {
    /// The label before a control; the row that sets a rectangle's corners sets a
    /// polygon's sides.
    fn label(&self, control: ShapeControl) -> &'static str {
        match (control, selected_shape(self.state)) {
            (ShapeControl::Corners, Some((_, s))) if s.kind == ShapeKind::Polygon => "Sides",
            _ => control.label(),
        }
    }

    fn control(&mut self, ui: &mut Ui, theme: &Theme, control: ShapeControl) {
        let Some((id, spec)) = selected_shape(self.state) else {
            return;
        };
        let spec = spec.clone();
        let mut next = spec.clone();
        let mut done = true;
        let mut changed = false;
        match control {
            ShapeControl::Kind => {
                egui::ComboBox::from_id_salt("shape-kind")
                    .selected_text(spec.kind.label())
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for k in KINDS {
                            if ui.selectable_label(spec.kind == k, k.label()).clicked() {
                                next.kind = k;
                                changed = true;
                            }
                        }
                    });
            }
            ShapeControl::Fill => {
                let a = theme.accent.to_srgba_unmultiplied();
                let auto = [a[0], a[1], a[2], 48];
                // A line has no inside to fill.
                let picked = ui
                    .add_enabled_ui(spec.kind != ShapeKind::Line, |ui| {
                        colour_picker::button(
                            ui,
                            colour_id(id, "fill"),
                            spec.fill,
                            auto,
                            "No fill",
                            theme,
                        )
                    })
                    .inner;
                if let Some(p) = picked {
                    next.fill = p.colour;
                    done = p.done;
                    changed = true;
                }
            }
            ShapeControl::Outline => {
                let auto = theme.text.to_srgba_unmultiplied();
                if let Some(p) = colour_picker::button(
                    ui,
                    colour_id(id, "stroke"),
                    spec.stroke,
                    auto,
                    "No outline",
                    theme,
                ) {
                    next.stroke = p.colour;
                    done = p.done;
                    changed = true;
                }
            }
            ShapeControl::Width => {
                let mut v = spec.stroke_width;
                let r = ui.add(
                    egui::DragValue::new(&mut v)
                        .range(0.0..=MAX_STROKE)
                        .speed(0.1)
                        .max_decimals(1),
                );
                changed = r.changed() || r.drag_stopped() || r.lost_focus();
                done = !r.dragged();
                next.stroke_width = v;
            }
            ShapeControl::Line => {
                egui::ComboBox::from_id_salt("shape-dash")
                    .selected_text(dash_label(spec.dash))
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for d in [Dash::Solid, Dash::Dashed, Dash::Dotted] {
                            if ui.selectable_label(spec.dash == d, dash_label(d)).clicked() {
                                next.dash = d;
                                changed = true;
                            }
                        }
                    });
            }
            ShapeControl::StartHead | ShapeControl::EndHead => {
                let start = control == ShapeControl::StartHead;
                let current = if start {
                    spec.arrow_start
                } else {
                    spec.arrow_end
                };
                ui.add_enabled_ui(spec.kind == ShapeKind::Line, |ui| {
                    egui::ComboBox::from_id_salt(if start { "head-start" } else { "head-end" })
                        .selected_text(current.label())
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for a in Arrowhead::ALL {
                                if ui.selectable_label(current == a, a.label()).clicked() {
                                    if start {
                                        next.arrow_start = a;
                                    } else {
                                        next.arrow_end = a;
                                    }
                                    changed = true;
                                }
                            }
                        });
                });
            }
            ShapeControl::Corners if spec.kind == ShapeKind::Polygon => {
                let mut v = spec.sides;
                let r = ui.add(egui::DragValue::new(&mut v).range(MIN_SIDES..=MAX_SIDES));
                changed = r.changed() || r.drag_stopped() || r.lost_focus();
                done = !r.dragged();
                next.sides = v;
            }
            ShapeControl::Corners => {
                let rounded = spec.kind == ShapeKind::Rectangle;
                let mut v = spec.corner_radius;
                let r = ui.add_enabled(
                    rounded,
                    egui::DragValue::new(&mut v)
                        .range(0.0..=MAX_RADIUS)
                        .speed(0.5)
                        .max_decimals(1),
                );
                changed = r.changed() || r.drag_stopped() || r.lost_focus();
                done = !r.dragged();
                next.corner_radius = v;
            }
        }
        if changed {
            self.send(id, &spec, next, done);
        }
    }
}

/// The settings of the selected shape, one labelled row each, then what can be done
/// to it as a region (draw order, lock, delete).
pub fn body(ui: &mut Ui, state: &AppState, cmds: &mut Vec<CommandId>, out: &mut Vec<CanvasCmd>) {
    let Some((id, spec)) = selected_shape(state) else {
        return;
    };
    let theme = state.theme;
    let kind = spec.kind;
    let mut host = ShapeControls { state, out };
    egui::Grid::new("shape-settings")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            let mut rows = vec![
                ShapeControl::Kind,
                ShapeControl::Corners,
                ShapeControl::Fill,
                ShapeControl::Outline,
                ShapeControl::Width,
                ShapeControl::Line,
            ];
            if kind == ShapeKind::Line {
                rows.extend([ShapeControl::StartHead, ShapeControl::EndHead]);
            }
            for control in rows {
                ui.label(
                    RichText::new(host.label(control))
                        .size(type_size::SMALL)
                        .color(theme.text_weak),
                );
                host.control(ui, &theme, control);
                ui.end_row();
            }
        });
    ui.add_space(12.0);
    ui.label(
        RichText::new("ARRANGE")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    let locked = state
        .active_sheet()
        .and_then(|s| s.doc.region(id))
        .is_some_and(|r| r.locked);
    ui.horizontal_wrapped(|ui| {
        for command in [
            CommandId::ArrangeToFront,
            CommandId::ArrangeForward,
            CommandId::ArrangeBackward,
            CommandId::ArrangeToBack,
            CommandId::RegionLock,
            CommandId::RegionDelete,
        ] {
            let button = egui::Button::new(command.label())
                .selected(command == CommandId::RegionLock && locked);
            if ui.add(button).clicked() {
                cmds.push(command);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::PaletteKey;
    use opencalc_core::document::Point;

    #[test]
    fn a_change_is_an_edit_and_a_seal_and_no_change_is_only_a_seal() {
        let mut state = AppState::with_sample(PaletteKey::Neutral);
        let i = state.active.unwrap();
        let id = state.sheets[i].add(RegionKind::shape(ShapeKind::Ellipse), Point::new(5.0, 5.0));
        let spec = ShapeSpec::of(ShapeKind::Ellipse);
        let wider = ShapeSpec {
            stroke_width: 6.0,
            ..spec.clone()
        };
        let mut out = Vec::new();
        let mut host = ShapeControls {
            state: &state,
            out: &mut out,
        };
        host.send(id, &spec, wider.clone(), true);
        host.send(id, &spec, spec.clone(), true);
        host.send(id, &spec, wider.clone(), false);
        assert_eq!(
            out,
            vec![
                CanvasCmd::Edit(Edit::SetShape {
                    id,
                    spec: Box::new(wider.clone())
                }),
                CanvasCmd::Seal,
                CanvasCmd::Seal,
                CanvasCmd::Edit(Edit::SetShape {
                    id,
                    spec: Box::new(wider)
                }),
            ],
            "a drag in progress sends edits without sealing"
        );
    }
}
