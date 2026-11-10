//! The unit of a result, changed where it is shown.
//!
//! As in Mathcad and SMath, the unit after `=` is a slot: click it and type the
//! unit the result should read in (`N/mm^2`), Enter to accept. It is display only.
//! A slot that is hovered or selected shows a faint outline so it can be found.
//!
//! Painting records where each slot is (screen space, from the last paint) in egui's
//! memory; the pointer and the editor read that back, so layout stays in one place.

use eframe::egui::{
    self, Align2, Color32, Context, FontId, Id, Key, Pos2, Rect, Stroke, StrokeKind, TextEdit, Ui,
    Vec2,
};
use opencalc_core::document::{Region, RegionId, RegionKind};
use opencalc_core::edit::Edit;
use opencalc_core::unit_define::define_unit;

use super::{CanvasCmd, CanvasUi, Env};
use crate::theme::type_size;

/// Where a result's unit was drawn, and the unit it shows.
#[derive(Clone)]
struct Slot {
    rect: Rect,
    label: String,
    /// The result's font size on screen, so the box types at the same size.
    size: f32,
}

/// The unit being typed into.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitEdit {
    pub id: RegionId,
    pub text: String,
    started: bool,
}

fn key(id: RegionId) -> Id {
    Id::new(("unit-slot", id.0))
}

/// Forget a region's slot (it shows no unit, or is being edited).
pub(super) fn forget(ctx: &Context, id: RegionId) {
    ctx.data_mut(|d| d.remove::<Slot>(key(id)));
}

/// Record a slot and draw its hint: outlined while hovered or selected.
pub(super) fn paint(
    ui: &Ui,
    painter: &egui::Painter,
    env: &Env,
    r: &Region,
    rect: Rect,
    label: String,
    size: f32,
) {
    let hover = ui.input(|i| i.pointer.hover_pos());
    let hovered = hover.is_some_and(|p| rect.expand(3.0).contains(p));
    if hovered && !r.locked {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    if hovered && !r.locked {
        let line = Stroke::new(1.0_f32, env.paper.text_weak);
        painter.rect_stroke(rect.expand(3.0), 3.0, line, StrokeKind::Middle);
    }
    ui.ctx()
        .data_mut(|d| d.insert_temp(key(r.id), Slot { rect, label, size }));
}

/// The region whose unit slot is at `pos`, if any.
pub(super) fn hit(ctx: &Context, regions: &[Region], pos: Pos2) -> Option<RegionId> {
    regions.iter().find_map(|r| {
        let slot: Slot = ctx.data(|d| d.get_temp(key(r.id)))?;
        (!r.locked && slot.rect.expand(3.0).contains(pos)).then_some(r.id)
    })
}

/// Start typing a unit into the slot of `id`.
pub(super) fn begin(ctx: &Context, regions: &[Region], id: RegionId) -> Option<UnitEdit> {
    let slot: Slot = ctx.data(|d| d.get_temp(key(id)))?;
    let custom = regions
        .iter()
        .find(|r| r.id == id)
        .and_then(|r| match &r.kind {
            RegionKind::Math(m) => m.format().custom_unit().map(str::to_owned),
            _ => None,
        });
    Some(UnitEdit {
        id,
        text: custom.unwrap_or(slot.label),
        started: false,
    })
}

/// Why `text` cannot be the unit of a result of dimension `dim`, or `None` if it can.
fn problem(text: &str, dim: Option<opencalc_core::units::Dimension>) -> Option<String> {
    let def = match define_unit("shown", text, &[], None) {
        Ok(d) => d,
        Err(e) => return Some(e.to_string()),
    };
    match dim {
        Some(d) if d != def.dimension => Some("that is a different kind of unit".to_owned()),
        _ => None,
    }
}

fn typing_key() -> Id {
    Id::new("unit-typing")
}

/// What is being typed into the unit slot of `id`, if it is open. Painting makes
/// room for it in the result, so the region grows with the text and the box never
/// covers the number or runs past the region.
pub(super) fn typing(ctx: &Context, id: RegionId) -> Option<String> {
    ctx.data(|d| d.get_temp::<(RegionId, String)>(typing_key()))
        .filter(|(r, _)| *r == id)
        .map(|(_, t)| t)
}

/// The unit box while a unit is being typed, and the text painting makes room for.
pub(super) fn edit(
    ui: &mut Ui,
    env: &Env,
    regions: &[Region],
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    edit_box(ui, env, regions, st, cmds);
    let now = st.unit_edit.as_ref().map(|u| (u.id, u.text.clone()));
    let before: Option<(RegionId, String)> = ui.ctx().data(|d| d.get_temp(typing_key()));
    if now != before {
        ui.ctx().data_mut(|d| match now {
            Some(v) => d.insert_temp(typing_key(), v),
            None => d.remove::<(RegionId, String)>(typing_key()),
        });
        // The region is laid out from this text: lay it out again now.
        ui.ctx().request_repaint();
    }
}

fn edit_box(
    ui: &mut Ui,
    env: &Env,
    regions: &[Region],
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let Some(mut ue) = st.unit_edit.take() else {
        return;
    };
    let slot: Option<Slot> = ui.ctx().data(|d| d.get_temp(key(ue.id)));
    let region = regions.iter().find(|r| r.id == ue.id);
    let (Some(slot), Some(region), None) = (slot, region, st.editing) else {
        return;
    };
    let dim = match env.output.values.get(&ue.id) {
        Some(Ok(opencalc_core::value::Value::Scalar(s))) => Some(s.dim),
        _ => None,
    };
    let bad = if ue.text.trim().is_empty() {
        None
    } else {
        problem(&ue.text, dim)
    };
    // The result has made room for the text (see `typing`): type exactly there,
    // with a thin outline around it.
    let rect = slot.rect;
    let outline = if bad.is_some() {
        env.theme.error
    } else {
        env.theme.accent
    };
    ui.painter().rect_stroke(
        rect.expand2(Vec2::new(2.0, 1.0)),
        2.0,
        Stroke::new(1.0_f32, outline),
        StrokeKind::Outside,
    );
    let id = Id::new(("unit-edit", ue.id.0));
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            // The sheet's colours, not the window's: no dark box, a soft selection.
            let v = ui.visuals_mut();
            v.selection.bg_fill = env.theme.accent.gamma_multiply(0.3);
            v.selection.stroke = Stroke::NONE;
            v.extreme_bg_color = Color32::TRANSPARENT;
            let colour = if bad.is_some() {
                env.theme.error
            } else {
                env.paper.text
            };
            ui.add(
                TextEdit::singleline(&mut ue.text)
                    .id(id)
                    .frame(false)
                    .margin(egui::Margin::ZERO)
                    .vertical_align(egui::Align::Center)
                    .font(FontId::proportional(slot.size))
                    .text_color(colour)
                    .desired_width(rect.width()),
            )
        })
        .inner;
    if !ue.started {
        resp.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
            let len = ue.text.chars().count();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                )));
            state.store(ui.ctx(), id);
        }
        ue.started = true;
    }
    if let Some(message) = &bad {
        ui.painter().text(
            rect.left_bottom() + Vec2::new(0.0, 4.0),
            Align2::LEFT_TOP,
            message,
            FontId::proportional(type_size::SMALL),
            env.theme.error,
        );
    }
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let clicked_away = resp.lost_focus() && !enter && !escape;
    if escape || (clicked_away && bad.is_some()) {
        return;
    }
    if enter && bad.is_some() {
        // Keep it open so the mistake can be fixed.
        resp.request_focus();
        st.unit_edit = Some(ue);
        return;
    }
    if enter || clicked_away {
        if let RegionKind::Math(m) = &region.kind {
            let text = ue.text.trim();
            let format = m
                .format()
                .with_custom_unit((!text.is_empty()).then(|| text.to_owned()));
            cmds.push(CanvasCmd::Seal);
            cmds.push(CanvasCmd::Edit(Edit::SetResultFormat { id: ue.id, format }));
            cmds.push(CanvasCmd::Seal);
        }
        return;
    }
    st.unit_edit = Some(ue);
}

/// Where the middle of a region's unit slot is on screen, for tests.
#[cfg(test)]
pub(crate) fn centre_of(ctx: &Context, id: RegionId) -> Option<Pos2> {
    let slot: Slot = ctx.data(|d| d.get_temp(key(id)))?;
    Some(slot.rect.center())
}
