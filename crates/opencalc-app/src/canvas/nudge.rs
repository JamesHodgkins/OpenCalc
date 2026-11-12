//! Arrow keys move the selected regions: one grid step (or one unit with snap off),
//! more with Shift, a single unit with Alt. Selection wins over the insertion
//! crosshair, which only moves when nothing is selected.

use eframe::egui::{Key, Pos2, Ui};
use opencalc_core::document::{Region, RegionId};
use opencalc_core::edit::Edit;

use super::adapt::point;
use super::view::Snapper;
use super::{CanvasCmd, CanvasUi, Env};

/// Steps taken by one press with Shift held, in grid steps (or units, snap off: 10).
const SHIFT_STEPS: u32 = 5;
const SHIFT_UNITS: f32 = 10.0;

/// New positions for `origins` after one press toward `dir` (each of -1, 0, 1).
/// With a grid, the first region lands on the next grid line in that direction
/// (from wherever it is, on the grid or not) and the rest keep their offsets from
/// it, so a group keeps its shape. Without one, every region moves by whole units.
/// Nothing is moved off the top or left of the page.
pub fn nudged(
    origins: &[(RegionId, Pos2)],
    dir: (i8, i8),
    big: bool,
    grid: Option<Snapper>,
) -> Vec<(RegionId, Pos2)> {
    let Some(&(_, lead)) = origins.first() else {
        return Vec::new();
    };
    let mut delta = match grid {
        Some(g) => g.step_toward(lead, dir, if big { SHIFT_STEPS } else { 1 }) - lead,
        None => {
            let unit = if big { SHIFT_UNITS } else { 1.0 };
            eframe::egui::vec2(f32::from(dir.0) * unit, f32::from(dir.1) * unit)
        }
    };
    let min_x = origins
        .iter()
        .map(|(_, p)| p.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = origins
        .iter()
        .map(|(_, p)| p.y)
        .fold(f32::INFINITY, f32::min);
    delta.x = delta.x.max(-min_x);
    delta.y = delta.y.max(-min_y);
    origins.iter().map(|&(id, p)| (id, p + delta)).collect()
}

/// Enter or F2 on a lone selected region that can be edited starts editing it (the
/// way a double click does), with the caret at the end.
pub(super) fn edit_on_enter(
    ui: &Ui,
    regions: &[Region],
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let [id] = st.selection.as_slice() else {
        return;
    };
    let key = ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::F2));
    if !key || ui.ctx().wants_keyboard_input() {
        return;
    }
    let editable = regions
        .iter()
        .any(|r| r.id == *id && r.kind.is_editable() && !r.locked);
    if editable {
        cmds.push(CanvasCmd::Seal);
        st.editing = Some(*id);
        st.focus_edit = true;
        st.enter_at_start = false;
        st.armed = false;
    }
}

fn pressed(ui: &Ui, key: Key) -> i8 {
    i8::from(ui.input(|i| i.key_pressed(key)))
}

/// Move the selection if an arrow was pressed this frame. Holding the key is one
/// undo step: it is sealed on the first frame with no arrow pressed.
pub(super) fn nudge(
    ui: &Ui,
    env: &Env,
    regions: &[Region],
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let (mods, wants_keys) = (ui.input(|i| i.modifiers), ui.ctx().wants_keyboard_input());
    let dir = if wants_keys || mods.command || mods.ctrl {
        (0, 0)
    } else {
        (
            pressed(ui, Key::ArrowRight) - pressed(ui, Key::ArrowLeft),
            pressed(ui, Key::ArrowDown) - pressed(ui, Key::ArrowUp),
        )
    };
    if dir == (0, 0) {
        if st.nudging {
            st.nudging = false;
            cmds.push(CanvasCmd::Seal);
        }
        return;
    }
    // Locked regions stay where they are.
    let origins: Vec<(RegionId, Pos2)> = st
        .selection
        .iter()
        .filter_map(|id| regions.iter().find(|r| r.id == *id))
        .filter(|r| !r.locked)
        .map(|r| (r.id, Pos2::new(r.pos.x, r.pos.y)))
        .collect();
    let grid = (env.snap && !mods.alt).then_some(st.snapper);
    let moves: Vec<_> = nudged(&origins, dir, mods.shift, grid)
        .into_iter()
        .map(|(id, p)| (id, point(p)))
        .collect();
    if moves.is_empty() {
        return;
    }
    if !st.nudging {
        st.nudging = true;
        cmds.push(CanvasCmd::Seal);
    }
    cmds.push(CanvasCmd::Edit(Edit::MoveRegions { moves }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> RegionId {
        RegionId(n)
    }

    #[test]
    fn on_the_grid_a_press_moves_one_step_and_shift_five() {
        let g = Some(Snapper::plain(12.0));
        let at = [(id(1), Pos2::new(48.0, 60.0))];
        assert_eq!(nudged(&at, (1, 0), false, g)[0].1, Pos2::new(60.0, 60.0));
        assert_eq!(nudged(&at, (0, 1), true, g)[0].1, Pos2::new(48.0, 120.0));
        assert_eq!(nudged(&at, (-1, 0), false, g)[0].1, Pos2::new(36.0, 60.0));
    }

    #[test]
    fn off_the_grid_the_first_press_lands_on_the_next_line() {
        let g = Some(Snapper::plain(12.0));
        let at = [(id(1), Pos2::new(50.0, 61.0))];
        assert_eq!(nudged(&at, (1, 0), false, g)[0].1, Pos2::new(60.0, 61.0));
        assert_eq!(nudged(&at, (-1, 0), false, g)[0].1, Pos2::new(48.0, 61.0));
        assert_eq!(nudged(&at, (0, -1), false, g)[0].1, Pos2::new(50.0, 60.0));
    }

    #[test]
    fn without_a_grid_it_is_one_unit_and_shift_ten() {
        let at = [(id(1), Pos2::new(50.0, 61.0))];
        assert_eq!(nudged(&at, (1, 0), false, None)[0].1, Pos2::new(51.0, 61.0));
        assert_eq!(nudged(&at, (0, -1), true, None)[0].1, Pos2::new(50.0, 51.0));
    }

    #[test]
    fn a_group_keeps_its_shape_and_stops_at_the_page_edge() {
        let g = Some(Snapper::plain(12.0));
        let pair = [
            (id(1), Pos2::new(24.0, 30.0)),
            (id(2), Pos2::new(5.0, 90.0)),
        ];
        let out = nudged(&pair, (1, 0), false, g);
        assert_eq!(out[1].1 - out[0].1, pair[1].1 - pair[0].1);
        // Pushed left past the page edge: the leftmost region stops at 0, and the
        // other keeps its distance from it.
        let out = nudged(&pair, (-1, 0), true, g);
        assert_eq!((out[0].1.x, out[1].1.x), (19.0, 0.0));
        assert!(nudged(&[], (1, 0), false, g).is_empty());
    }

    #[test]
    fn the_grid_is_the_pages_not_the_documents() {
        // A lattice that starts at a margin of 7.5: lines at 7.5, 19.5, 31.5, ...
        let g = Some(Snapper {
            step: 12.0,
            margin: 7.5,
            stride: f32::INFINITY,
        });
        let at = [(id(1), Pos2::new(19.5, 7.5))];
        assert_eq!(nudged(&at, (1, 1), false, g)[0].1, Pos2::new(31.5, 19.5));
    }
}
