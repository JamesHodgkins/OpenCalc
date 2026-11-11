//! Pointer input on the canvas: panning and zooming, the drawing tool, and the
//! press / drag / release gestures that select, move, resize and marquee regions.

use eframe::egui::{self, PointerButton, Pos2, Rect, Ui, Vec2};
use opencalc_core::document::{Region, RegionId, RegionKind, ShapeKind, ShapeSpec, Size};
use opencalc_core::edit::Edit;

use super::adapt::{self, point, rect_of};
use super::hit::{click_selection, drag_positions, hit_stack, hit_test, in_marquee, pick_under};
use super::view::snap;
use super::{grip, magnet, resize, shape, unit_slot, CanvasCmd, CanvasUi, CanvasView, Drag, Scene};

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
    let grid = (env.snap && !mods.alt).then_some(st.grid.step());
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

/// A press that starts a drag: on a handle it resizes, on a region it moves the
/// selection, on empty canvas it starts a marquee.
fn begin_drag(ui: &Ui, scene: &Scene, st: &mut CanvasUi, press: Pos2) {
    let Scene {
        regions,
        view,
        origin,
        ..
    } = *scene;
    let mods = ui.input(|i| i.modifiers);
    let extend = mods.shift || mods.command;
    let doc = view.screen_to_doc(press, origin);
    let grabbed = handle_under(scene, st, press);
    let gripped = grip_under(scene, st, press);
    match hit_test(regions, &st.sizes, doc) {
        _ if gripped.is_some() => {
            st.drag = gripped.map(|(id, grip)| Drag::Grip {
                id,
                grip,
                fixed: regions
                    .iter()
                    .find(|r| r.id == id)
                    .map_or(Pos2::ZERO, |r| grip::fixed_end(grip, r)),
            });
        }
        _ if grabbed.is_some() => {
            st.drag = grabbed.map(|(id, handle, orig)| Drag::Resize { id, handle, orig });
        }
        Some(id) => {
            if !st.selection.contains(&id) {
                st.selection = click_selection(&st.selection, Some(id), extend);
            }
            let origins: Vec<_> = regions
                .iter()
                .filter(|r| st.selection.contains(&r.id) && !r.locked)
                .map(|r| (r.id, adapt::pos(r.pos)))
                .collect();
            st.drag = Some(Drag::Move {
                origins,
                total: Vec2::ZERO,
            });
        }
        None => {
            let base = if extend {
                st.selection.clone()
            } else {
                Vec::new()
            };
            st.selection = base.clone();
            st.drag = Some(Drag::Marquee {
                start: doc,
                current: doc,
                base,
            });
        }
    }
}

/// A drag in progress: move, resize or grow the marquee.
fn continue_drag(
    ui: &Ui,
    resp: &egui::Response,
    scene: &Scene,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let Scene {
        env, view, origin, ..
    } = *scene;
    let mods = ui.input(|i| i.modifiers);
    let to_doc = |p: Pos2| view.screen_to_doc(p, origin);
    let delta = resp.drag_delta() / view.zoom;
    let pos = resp.interact_pointer_pos();
    st.guides.clear();
    let mut guides = Vec::new();
    let reach = magnet::REACH_PX / view.zoom;
    match &mut st.drag {
        Some(Drag::Move { origins, total }) => {
            *total += delta;
            let snapping = env.snap && !mods.alt;
            let grid = snapping.then_some(st.grid.step());
            let mut moved = drag_positions(origins, *total, grid);
            if snapping {
                // The group's box where the pointer has put it; its edges are pulled
                // onto other regions' edges, which beat the grid on that axis.
                let ids: Vec<RegionId> = origins.iter().map(|(id, _)| *id).collect();
                let boxes = origins.iter().filter_map(|(id, at)| {
                    let r = scene.regions.iter().find(|r| r.id == *id)?;
                    Some(Rect::from_min_size(
                        *at + *total,
                        rect_of(r, &st.sizes).size(),
                    ))
                });
                if let Some(group) = boxes.reduce(|a, b| a.union(b)) {
                    let t = magnet::targets(scene.regions, &ids, &st.sizes);
                    let s = magnet::snap_rect(group, &t, magnet::REACH_PX / view.zoom);
                    if let (Some(&(_, lead)), Some(&(_, gridded))) =
                        (origins.first(), moved.first())
                    {
                        let on_grid = gridded - lead;
                        let delta = Vec2::new(
                            s.dx.map_or(on_grid.x, |d| total.x + d),
                            s.dy.map_or(on_grid.y, |d| total.y + d),
                        );
                        moved = origins.iter().map(|&(id, at)| (id, at + delta)).collect();
                    }
                    guides = s.guides;
                }
            }
            let moves = moved.into_iter().map(|(id, p)| (id, point(p))).collect();
            cmds.push(CanvasCmd::Edit(Edit::MoveRegions { moves }));
        }
        Some(Drag::Marquee { current, .. }) => {
            if let Some(p) = pos {
                *current = to_doc(p);
            }
        }
        Some(Drag::Resize { id, handle, orig }) => {
            if let Some(p) = pos {
                let snapping = env.snap && !mods.alt;
                let grid = snapping.then_some(st.grid.step());
                let keep_ratio = mods.shift && handle.is_corner();
                let mut at = to_doc(p);
                let mut grid_for_box = grid;
                if snapping && !keep_ratio {
                    // The pulled edge goes onto another region's edge if one is near,
                    // else onto the grid.
                    if let Some(g) = grid {
                        at = snap(at, g);
                    }
                    let (on_x, on_y) = handle.pulls();
                    let t = magnet::targets(scene.regions, &[*id], &st.sizes);
                    let s = magnet::snap_point(to_doc(p), &t, reach, on_x, on_y);
                    at = Pos2::new(
                        s.dx.map_or(at.x, |d| to_doc(p).x + d),
                        s.dy.map_or(at.y, |d| to_doc(p).y + d),
                    );
                    guides = s.guides;
                    grid_for_box = None;
                }
                let b = resize::resized(
                    *orig,
                    *handle,
                    at,
                    grid_for_box,
                    mods.shift,
                    mods.ctrl || mods.command,
                );
                cmds.push(CanvasCmd::Edit(Edit::SetRegionBox {
                    id: *id,
                    pos: point(b.min),
                    size: Size::new(b.width(), b.height()),
                }));
            }
        }
        Some(Drag::Grip { id, grip, fixed }) => {
            let region = scene.regions.iter().find(|r| r.id == *id);
            if let (Some(p), Some(region)) = (pos, region) {
                if let RegionKind::Shape(spec) = &region.kind {
                    if let Some(is_start) = grip.line_end() {
                        let snapping = env.snap && !mods.alt;
                        let grid = snapping.then_some(st.grid.step());
                        let t = magnet::targets(scene.regions, &[*id], &st.sizes);
                        let (moved, g) = grip::line_end_dragged(
                            *fixed,
                            to_doc(p),
                            grid,
                            mods.shift,
                            snapping.then_some((&t, reach)),
                        );
                        guides = g;
                        let (from, to) = if is_start {
                            (moved, *fixed)
                        } else {
                            (*fixed, moved)
                        };
                        cmds.push(CanvasCmd::Edit(Edit::SetLineEnds {
                            id: *id,
                            from: point(from),
                            to: point(to),
                        }));
                    } else {
                        let screen = view.rect_to_screen(rect_of(region, &st.sizes), scene.origin);
                        let next = grip::dragged(*grip, spec, screen, view.zoom, p);
                        if next != **spec {
                            cmds.push(CanvasCmd::Edit(Edit::SetShape {
                                id: *id,
                                spec: Box::new(next),
                            }));
                        }
                    }
                }
            }
        }
        Some(Drag::Draw { .. }) | None => {}
    }
    st.guides = guides;
}

/// The release that ends a drag: a marquee selects what it covers.
fn end_drag(scene: &Scene, st: &mut CanvasUi) {
    st.guides.clear();
    if let Some(Drag::Marquee {
        start,
        current,
        base,
    }) = st.drag.take()
    {
        let mut sel = base;
        for id in in_marquee(scene.regions, &st.sizes, Rect::from_two_pos(start, current)) {
            if !sel.contains(&id) {
                sel.push(id);
            }
        }
        st.selection = sel;
    }
    st.drag = None;
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
            st.insert_at = snap(doc, st.grid.step());
        }
    }
}
