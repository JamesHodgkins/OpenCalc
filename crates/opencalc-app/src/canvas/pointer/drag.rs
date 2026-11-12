//! The press, drag and release of the primary button: begin a move, resize,
//! grip pull or marquee, follow it, and end it.

use eframe::egui::{self, Pos2, Rect, Ui, Vec2};
use opencalc_core::document::{RegionId, RegionKind, Size};
use opencalc_core::edit::Edit;

use super::{grip_under, handle_under};
use crate::canvas::adapt::{self, point, rect_of};
use crate::canvas::hit::{click_selection, drag_positions, hit_test, in_marquee};
use crate::canvas::view::snap;
use crate::canvas::{grip, magnet, resize, CanvasCmd, CanvasUi, Scene};

/// What the primary button is doing between a press and its release.
pub(in crate::canvas) enum Drag {
    Move {
        origins: Vec<(RegionId, Pos2)>,
        total: Vec2,
    },
    /// A handle of the selected region being pulled; `orig` is its box when the
    /// drag began (document space), so the result does not accumulate error.
    Resize {
        id: RegionId,
        handle: resize::Handle,
        orig: Rect,
    },
    /// A grip of the selected shape being pulled (the corner radius of a rectangle).
    /// `fixed` is the end of a line that stays put (document space).
    Grip {
        id: RegionId,
        grip: grip::Grip,
        fixed: Pos2,
    },
    /// A shape being drawn with the drawing tool.
    Draw { start: Pos2, current: Pos2 },
    Marquee {
        start: Pos2,
        current: Pos2,
        /// Selection to keep when the marquee extends rather than replaces it.
        base: Vec<RegionId>,
    },
}

/// A press that starts a drag: on a handle it resizes, on a region it moves the
/// selection, on empty canvas it starts a marquee.
pub(super) fn begin_drag(ui: &Ui, scene: &Scene, st: &mut CanvasUi, press: Pos2) {
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
pub(super) fn continue_drag(
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
pub(super) fn end_drag(scene: &Scene, st: &mut CanvasUi) {
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
