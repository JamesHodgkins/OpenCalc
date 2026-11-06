//! The in-place editor for the region being edited: the math editor for math
//! regions, a text box for text regions.

use eframe::egui::{Align2, FontId, Id, Key, Painter, Pos2, Rect, Stroke, TextEdit, Ui, Vec2};
use opencalc_core::document::{Region, RegionKind};
use opencalc_core::edit::Edit;

use super::adapt::rect_of;
use super::{editor, CanvasCmd, CanvasUi, CanvasView, Env};

/// The in-place editor for the region being edited.
pub(super) fn edit_overlay(
    ui: &mut Ui,
    env: &Env,
    regions: &[Region],
    view: &CanvasView,
    origin: Pos2,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let Some(id) = st.editing else { return };
    let Some(r) = regions.iter().find(|r| r.id == id) else {
        st.editing = None;
        return;
    };
    let screen = view.rect_to_screen(rect_of(r, &st.sizes), origin);
    let zoom = view.zoom;
    let widget_id = Id::new(("region", id.0));
    let (resp, change) = match &r.kind {
        RegionKind::Math(_) => {
            let cx = editor::Ctx {
                env,
                regions,
                view,
                origin,
            };
            editor::math(ui, &cx, r, st, cmds);
            return;
        }
        RegionKind::Text { text } => {
            let mut buf = text.clone();
            let rect = Rect::from_min_size(
                screen.min,
                Vec2::new(r.size.w * zoom, screen.height().max(40.0 * zoom)),
            );
            let edit = TextEdit::multiline(&mut buf)
                .id(widget_id)
                .font(FontId::proportional(14.0 * zoom))
                .desired_width(rect.width());
            let resp = ui.put(rect, edit);
            let change = resp.changed().then_some(Edit::SetText { id, text: buf });
            (resp, change)
        }
        _ => {
            st.editing = None;
            return;
        }
    };
    cmds.extend(change.map(CanvasCmd::Edit));
    if st.focus_edit {
        resp.request_focus();
        st.focus_edit = false;
    } else if resp.lost_focus() || ui.input(|i| i.key_pressed(Key::Escape)) {
        st.editing = None;
        cmds.push(CanvasCmd::Seal);
    }
}

/// The worksheet insertion point: a crosshair where the next region goes. Drawn only
/// when no region is being edited, so it never competes with the text caret.
pub(super) fn crosshair(
    painter: &Painter,
    view: &CanvasView,
    origin: Pos2,
    st: &CanvasUi,
    env: &Env,
) {
    let c = view.doc_to_screen(st.insert_at, origin);
    let arm = 7.0;
    let stroke = Stroke::new(1.0_f32, env.theme.accent);
    painter.line_segment([c - Vec2::new(arm, 0.0), c + Vec2::new(arm, 0.0)], stroke);
    painter.line_segment([c - Vec2::new(0.0, arm), c + Vec2::new(0.0, arm)], stroke);
}

/// Tell the user that a paste or insertion was cut short by the editor's size limit.
pub(super) fn clip_notice(painter: &Painter, origin: Pos2, height: f32, zoom: f32, env: &Env) {
    painter.text(
        origin + Vec2::new(0.0, height + 6.0 * zoom),
        Align2::LEFT_TOP,
        "Input was cut off: the expression is at its size limit.",
        FontId::proportional(12.0 * zoom),
        env.theme.warning,
    );
}
