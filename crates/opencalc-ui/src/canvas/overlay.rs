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

/// Click-and-type: after a click on blank page, the first printable text starts a
/// math region at the insertion point. Does nothing while another widget (a search
/// box, say) wants the keyboard.
pub(super) fn type_to_create(ui: &Ui, st: &mut CanvasUi, cmds: &mut Vec<CanvasCmd>) {
    if !st.armed || ui.ctx().wants_keyboard_input() {
        return;
    }
    let typed: String = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                eframe::egui::Event::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    });
    let typed: String = typed.chars().filter(|c| !c.is_control()).collect();
    if typed.trim().is_empty() {
        return;
    }
    st.armed = false;
    cmds.push(CanvasCmd::Seal);
    cmds.push(CanvasCmd::TypeAhead(typed));
}

/// Arrow keys on the blank page move the crosshair one grid step. Does nothing
/// while a region is edited or another widget wants the keyboard.
pub(super) fn move_crosshair(ui: &Ui, regions: &[Region], st: &mut CanvasUi) {
    if ui.ctx().wants_keyboard_input() {
        return;
    }
    let step = |k: Key| ui.input(|i| i.key_pressed(k)) as i8;
    let dx = step(Key::ArrowRight) - step(Key::ArrowLeft);
    let dy = step(Key::ArrowDown) - step(Key::ArrowUp);
    if dx == 0 && dy == 0 {
        return;
    }
    let g = super::view::GRID;
    let at = st.insert_at + Vec2::new(f32::from(dx) * g, f32::from(dy) * g);
    st.insert_at = super::view::snap(Pos2::new(at.x.max(0.0), at.y.max(0.0)), g);
    st.selection.clear();
    st.armed = true;
    // Moving onto an existing region hands control to it: the crosshair gives way
    // to that region's caret, which enters on the side the crosshair came from.
    let hit = super::hit::hit_test(regions, &st.sizes, st.insert_at)
        .filter(|id| regions.iter().any(|r| r.id == *id && r.kind.is_editable()));
    if let Some(id) = hit {
        st.selection = vec![id];
        st.editing = Some(id);
        st.focus_edit = true;
        st.armed = false;
        st.enter_at_start = dx > 0 || dy > 0;
    }
}

/// An arrow press ran off the edge of the region being edited: put the crosshair
/// just beyond that edge, in the direction pressed.
pub(super) fn leave_toward(st: &mut CanvasUi, region: &Region, (dx, dy): (i8, i8)) {
    let r = rect_of(region, &st.sizes);
    let gap = 12.0;
    // Leave along the region's middle line (sideways) or just inside its left edge
    // (up and down), so that snapping to the grid cannot land the crosshair off the
    // row or column of a region that is not itself on the grid.
    let (cx, cy) = (r.min.x + gap, (r.min.y + r.max.y) / 2.0);
    let at = match (dx, dy) {
        (-1, _) => Pos2::new((r.min.x - gap).max(0.0), cy),
        (1, _) => Pos2::new(r.max.x + gap, cy),
        (_, -1) => Pos2::new(cx, (r.min.y - gap).max(0.0)),
        _ => Pos2::new(cx, r.max.y + gap),
    };
    st.insert_at = super::view::snap(at, super::view::GRID);
    st.selection.clear();
    st.armed = true;
}

/// Enter ends an input and readies the next: the insertion point drops just below
/// the finished region, nothing stays selected, and typing starts a new math region.
pub(super) fn next_input_below(st: &mut CanvasUi, region: &Region) {
    let r = rect_of(region, &st.sizes);
    st.insert_at = super::view::snap(Pos2::new(r.min.x, r.max.y + 12.0), super::view::GRID);
    st.selection.clear();
    st.armed = true;
}
