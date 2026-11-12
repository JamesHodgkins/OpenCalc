//! Pointer input on the canvas: panning and zooming, the drawing tool, and the
//! press / drag / release gestures that select, move, resize and marquee regions.

use eframe::egui::{self, PointerButton, Pos2, Rect, Ui};
use opencalc_core::document::{Region, RegionId, RegionKind, ShapeKind, ShapeSpec};

pub(super) mod drag;

use super::adapt::rect_of;
use super::hit::{click_selection, hit_stack, hit_test, pick_under};
use super::{grip, resize, shape, unit_slot, CanvasCmd, CanvasUi, CanvasView, Drag, Scene};
use drag::{begin_drag, continue_drag, end_drag};

/// Wheel pans, Ctrl+wheel (or pinch) zooms about the pointer, middle-drag pans.
pub(super) fn navigate(ui: &Ui, resp: &egui::Response, origin: Pos2, view: &mut CanvasView) {
    if resp.hovered() {
        let zoom = ui.input(|i| i.zoom_delta());
        if (zoom - 1.0).abs() > f32::EPSILON {
            if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                view.zoom_about(p, origin, zoom);
            }
        } else {
            view.offset += ui.input(|i| i.smooth_scroll_delta);
        }
    }
    if resp.dragged_by(PointerButton::Middle) {
        view.offset += resp.drag_delta();
    }
}

/// Pixels from a handle's centre that still count as on it.
const HANDLE_REACH: f32 = 7.0;
/// Handle square, in pixels.
pub(super) const HANDLE_SIZE: f32 = 8.0;

/// Whether the region has a size of its own to pull (typeset regions are sized by
/// their content) and is free to change.
pub(super) fn resizable(r: &Region) -> bool {
    !r.locked && !r.kind.is_auto_sized()
}

/// Whether the region shows the eight resize handles. A line does not: its two
/// ends are grips instead.
pub(super) fn has_handles(r: &Region) -> bool {
    resizable(r) && !matches!(&r.kind, RegionKind::Shape(s) if s.kind == ShapeKind::Line)
}

/// The one selected region that shows grips (and, unless a line, resize handles).
fn gripped<'a>(regions: &'a [Region], st: &CanvasUi) -> Option<&'a Region> {
    let [id] = st.selection.as_slice() else {
        return None;
    };
    regions.iter().find(|r| r.id == *id && resizable(r))
}

/// The one selected region that shows resize handles, if there is one.
fn handled<'a>(regions: &'a [Region], st: &CanvasUi) -> Option<&'a Region> {
    gripped(regions, st).filter(|r| has_handles(r))
}

/// The drawing tool: drag out a box for a shape. A click without a drag makes the
/// shape at its usual size with its corner at the click. Escape puts the tool away.
fn draw_with_pen(
    ui: &Ui,
    resp: &egui::Response,
    scene: &Scene,
    kind: ShapeKind,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let Scene {
        env, view, origin, ..
    } = *scene;
    let mods = ui.input(|i| i.modifiers);
    let grid = (env.snap && !mods.alt).then_some(st.snapper);
    let (square, centre) = (mods.shift, mods.ctrl || mods.command);
    let to_doc = |p: Pos2| view.screen_to_doc(p, origin);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.pen = None;
        st.drag = None;
        return;
    }
    if resp.drag_started_by(PointerButton::Primary) {
        cmds.push(CanvasCmd::Seal);
        if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
            let start = to_doc(press);
            st.drag = Some(Drag::Draw {
                start,
                current: start,
            });
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let (Some(Drag::Draw { current, .. }), Some(p)) =
            (&mut st.drag, resp.interact_pointer_pos())
        {
            *current = to_doc(p);
        }
    }
    let finished = if resp.drag_stopped_by(PointerButton::Primary) {
        match st.drag.take() {
            Some(Drag::Draw { start, current }) => Some((start, current)),
            _ => None,
        }
    } else if resp.clicked() {
        resp.interact_pointer_pos().map(|p| (to_doc(p), to_doc(p)))
    } else {
        None
    };
    if let Some((start, end)) = finished {
        let (pos, size, flip) = if kind == ShapeKind::Line {
            let (from, to) = shape::drawn_line(start, end, grid, square, centre);
            let (pos, size, flip_x, flip_y) = ShapeSpec::line_box(from, to);
            (pos, size, (flip_x, flip_y))
        } else {
            let (pos, size) = shape::drawn_box(kind, start, end, grid, square, centre);
            (pos, size, (false, false))
        };
        cmds.push(CanvasCmd::Seal);
        cmds.push(CanvasCmd::DrawShape {
            kind,
            pos,
            size,
            flip,
        });
        cmds.push(CanvasCmd::Seal);
        st.pen = None;
        st.drag = None;
    }
}

/// The pointer's gestures for one frame on the canvas.
pub(super) fn pointer(
    ui: &Ui,
    resp: &egui::Response,
    scene: &Scene,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    if let Some(kind) = st.pen {
        draw_with_pen(ui, resp, scene, kind, st, cmds);
        return;
    }
    if st.drag.is_none() && resp.hovered() {
        let hover = ui.input(|i| i.pointer.hover_pos());
        if let Some((_, h, _)) = hover.and_then(|p| handle_under(scene, st, p)) {
            ui.ctx().set_cursor_icon(h.cursor());
        } else if hover.is_some_and(|p| grip_under(scene, st, p).is_some()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    if resp.drag_started_by(PointerButton::Primary) {
        cmds.push(CanvasCmd::Seal);
        if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
            begin_drag(ui, scene, st, press);
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        continue_drag(ui, resp, scene, st, cmds);
    }
    if resp.drag_stopped_by(PointerButton::Primary) {
        cmds.push(CanvasCmd::Seal);
        end_drag(scene, st);
    }
    if let Some(pos) = resp.interact_pointer_pos() {
        click(ui, resp, scene, st, cmds, pos);
    }
    if resp.secondary_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            right_click(scene, st, cmds, pos);
        }
    }
}

/// A right click, before its menu opens: it acts on what is under the pointer. A
/// region that is not selected becomes the selection (one already in the selection
/// keeps the whole selection); blank page clears it and puts the insertion point
/// there, so the menu's Paste and Insert go where the click was.
fn right_click(scene: &Scene, st: &mut CanvasUi, cmds: &mut Vec<CanvasCmd>, pos: Pos2) {
    let doc = scene.view.screen_to_doc(pos, scene.origin);
    match hit_test(scene.regions, &st.sizes, doc) {
        Some(id) if st.editing == Some(id) => {}
        Some(id) => {
            cmds.push(CanvasCmd::Seal);
            st.editing = None;
            if !st.selection.contains(&id) {
                st.selection = vec![id];
            }
        }
        None => {
            cmds.push(CanvasCmd::Seal);
            st.editing = None;
            st.selection.clear();
            st.insert_at = st.snapper.snap(Pos2::new(doc.x.max(0.0), doc.y.max(0.0)));
            st.armed = true;
        }
    }
}

/// The resize handle under the screen point `p`, with the region it belongs to and
/// that region's box.
fn handle_under(scene: &Scene, st: &CanvasUi, p: Pos2) -> Option<(RegionId, resize::Handle, Rect)> {
    let r = handled(scene.regions, st)?;
    let screen = scene
        .view
        .rect_to_screen(rect_of(r, &st.sizes), scene.origin);
    Some((
        r.id,
        resize::handle_at(screen, p, HANDLE_REACH)?,
        rect_of(r, &st.sizes),
    ))
}

/// The grip under the screen point `p`, with the region it belongs to.
fn grip_under(scene: &Scene, st: &CanvasUi, p: Pos2) -> Option<(RegionId, grip::Grip)> {
    let r = gripped(scene.regions, st)?;
    let screen = scene
        .view
        .rect_to_screen(rect_of(r, &st.sizes), scene.origin);
    Some((r.id, grip::grip_at(r, screen, scene.view.zoom, p)?))
}

/// Clicks and double clicks: the unit slot, editing a region, selecting.
fn click(
    ui: &Ui,
    resp: &egui::Response,
    scene: &Scene,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
    pos: Pos2,
) {
    let Scene {
        regions,
        view,
        origin,
        ..
    } = *scene;
    let mods = ui.input(|i| i.modifiers);
    let extend = mods.shift || mods.command;
    let to_doc = |p: Pos2| view.screen_to_doc(p, origin);
    // A click on the unit after a result opens it for typing.
    if resp.clicked() || resp.double_clicked() {
        if let Some(id) = unit_slot::hit(ui.ctx(), regions, pos) {
            cmds.push(CanvasCmd::Seal);
            st.selection = vec![id];
            st.editing = None;
            st.armed = false;
            st.unit_edit = unit_slot::begin(ui.ctx(), regions, id);
            return;
        }
    }
    if resp.double_clicked() {
        cmds.push(CanvasCmd::Seal);
        if let Some(id) = hit_test(regions, &st.sizes, to_doc(pos)) {
            if regions.iter().any(|r| r.id == id && r.kind.is_editable()) {
                st.selection = vec![id];
                st.editing = Some(id);
                st.focus_edit = true;
            }
        }
    } else if resp.clicked() {
        cmds.push(CanvasCmd::Seal);
        let doc = to_doc(pos);
        // Alt+click picks what lies under the topmost region there, and again goes
        // further down: a way to reach a shape that is covered.
        let hit = if mods.alt && !extend {
            pick_under(&hit_stack(regions, &st.sizes, doc), &st.selection)
        } else {
            hit_test(regions, &st.sizes, doc)
        };
        st.selection = click_selection(&st.selection, hit, extend);
        if hit != st.editing {
            st.editing = None;
        }
        st.armed = hit.is_none();
        if hit.is_none() {
            st.insert_at = st.snapper.snap(doc);
        }
    }
}
