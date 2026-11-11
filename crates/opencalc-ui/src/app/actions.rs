//! The small state changes that `dispatch` hands off to: page setup, zoom, drawing tools, inserts.

use opencalc_core::document::{PageSetup, RegionKind, ShapeKind};
use opencalc_core::edit::Edit;
use opencalc_core::registry::UnitSystem;

use super::dispatch::{apply_canvas, with_eval_check, Effect};
use super::state::AppState;
use crate::canvas::CanvasCmd;

/// Show results in another unit system. Display only, so no recalculation; it
/// is an edit so it can be undone.
pub(super) fn set_unit_system(state: &mut AppState, system: UnitSystem) {
    if let Some(i) = state.active {
        state.sheets[i].edit(Edit::SetUnitSystem(system));
        state.sheets[i].seal();
    }
}

/// Change the active worksheet's page setup. One-off choices (paper, orientation)
/// are their own undo steps; margin changes merge while the box is being dragged.
pub(super) fn set_page(state: &mut AppState, own_step: bool, change: impl FnOnce(&mut PageSetup)) {
    if let Some(i) = state.active {
        let sheet = &mut state.sheets[i];
        let mut page = sheet.doc.page();
        change(&mut page);
        if own_step {
            sheet.seal();
        }
        sheet.edit(Edit::SetPageSetup(page));
        if own_step {
            sheet.seal();
        }
    }
}

/// Push overlapping regions apart (one undo step).
pub(super) fn separate_regions(state: &mut AppState) {
    let Some(i) = state.active else { return };
    let sheet = &mut state.sheets[i];
    match crate::canvas::separate::separate(
        sheet.doc.regions(),
        &sheet.canvas.sizes,
        sheet.canvas.grid.step(),
    ) {
        None => state.message(
            "Separate regions",
            format!(
                "There are too many regions to separate (more than {}).",
                crate::canvas::separate::MAX_REGIONS
            ),
        ),
        Some(mut moves) => {
            // A locked region stays where it is.
            moves.retain(|(id, _)| sheet.doc.region(*id).is_some_and(|r| !r.locked));
            if moves.is_empty() {
                return;
            }
            sheet.seal();
            sheet.edit(Edit::MoveRegions { moves });
            sheet.seal();
        }
    }
}

/// Put `symbol` into the math region being edited, at the caret (replacing the
/// selection), as an ordinary edit of its source. The editor picks it up.
pub(super) fn insert_symbol(state: &mut AppState, sheet: usize, symbol: char) -> Vec<Effect> {
    let canvas = &state.sheets[sheet].canvas;
    let (Some(id), Some(field)) = (canvas.editing, canvas.field.as_ref()) else {
        return Vec::new();
    };
    let mut after = field.clone();
    after.insert(&symbol.to_string());
    let edit = Edit::SetMathSource {
        id,
        source: after.text(),
    };
    apply_canvas(state, sheet, vec![CanvasCmd::Edit(edit)])
}

pub(super) fn insert(state: &mut AppState, fx: &mut Vec<Effect>, kind: RegionKind) {
    with_eval_check(state, fx, |s| {
        s.insert(kind);
    });
}

/// Pick up the drawing tool: the next drag on the canvas draws this shape.
pub(super) fn arm_pen(state: &mut AppState, kind: ShapeKind) {
    if let Some(i) = state.active {
        let canvas = &mut state.sheets[i].canvas;
        canvas.pen = Some(kind);
        canvas.editing = None;
        canvas.field = None;
    }
}

/// Set the zoom of the active worksheet (clamped by the view), about the centre.
pub(super) fn zoom_to(state: &mut AppState, zoom: f32) {
    let (centre, origin) = (state.viewport.center(), state.viewport.min);
    if let Some(i) = state.active {
        state.sheets[i].view.set_zoom_about(centre, origin, zoom);
    }
}

pub(super) fn zoom_by(state: &mut AppState, factor: f32) {
    let (centre, origin) = (state.viewport.center(), state.viewport.min);
    if let Some(i) = state.active {
        state.sheets[i].view.zoom_about(centre, origin, factor);
    }
}
