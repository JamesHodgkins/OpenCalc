//! Align Horizontally, Align Vertically and Lock for the selected regions.

use opencalc_core::document::Arrange;
use opencalc_core::edit::Edit;

use super::dispatch::Effect;
use super::state::AppState;
use crate::canvas::adapt::rect_of;
use crate::canvas::align::align;
use crate::commands::CommandId;

/// Whether every selected region is locked (and there is one).
pub fn all_locked(state: &AppState) -> bool {
    state.active_sheet().is_some_and(|s| {
        let chosen: Vec<_> = s
            .doc
            .regions()
            .iter()
            .filter(|r| s.canvas.selection.contains(&r.id))
            .collect();
        !chosen.is_empty() && chosen.iter().all(|r| r.locked)
    })
}

/// Lock the selected regions, or unlock them if they all are. One undo step.
fn toggle_lock(state: &mut AppState) {
    let lock = !all_locked(state);
    let Some(i) = state.active else { return };
    let sheet = &mut state.sheets[i];
    let edits: Vec<Edit> = sheet
        .doc
        .regions()
        .iter()
        .filter(|r| sheet.canvas.selection.contains(&r.id) && r.locked != lock)
        .map(|r| Edit::SetLocked {
            id: r.id,
            locked: lock,
        })
        .collect();
    if !edits.is_empty() {
        sheet.seal();
        sheet.edit(Edit::Batch(edits));
        sheet.seal();
    }
}

/// Open the Label window for the first selected region. The region is no longer
/// being edited, so the text box can have the keyboard.
fn open_label(state: &mut AppState) {
    let Some(i) = state.active else { return };
    let canvas = &mut state.sheets[i].canvas;
    let Some(id) = canvas.selection.first().copied() else {
        return;
    };
    canvas.editing = None;
    canvas.field = None;
    state.label_for = Some(id);
}

/// Change the draw order of the selected regions (one undo step). A locked
/// region stays where it is in the order. Forward and backward step past the next
/// region the selection overlaps on the page.
fn restack(state: &mut AppState, how: Arrange) -> Vec<Effect> {
    let Some(i) = state.active else {
        return Vec::new();
    };
    let sheet = &mut state.sheets[i];
    let regions = sheet.doc.regions();
    let chosen: Vec<_> = sheet
        .canvas
        .selection
        .iter()
        .copied()
        .filter(|id| regions.iter().any(|r| r.id == *id && !r.locked))
        .collect();
    let rect = |id| {
        regions
            .iter()
            .find(|r| r.id == id)
            .map(|r| rect_of(r, &sheet.canvas.sizes))
    };
    let moves = sheet.doc.arrange(
        &chosen,
        how,
        &|a, b| matches!((rect(a), rect(b)), (Some(a), Some(b)) if a.intersects(b)),
    );
    if !moves.is_empty() {
        sheet.seal();
        sheet.edit(Edit::SetZ { moves });
        sheet.seal();
    }
    Vec::new()
}

/// Run an arrange command. `None` if `command` is not one.
pub fn dispatch_arrange(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    let row = match command {
        CommandId::RegionLock => {
            toggle_lock(state);
            return Some(Vec::new());
        }
        CommandId::RegionTag => {
            open_label(state);
            return Some(Vec::new());
        }
        CommandId::LabelClose => {
            state.label_for = None;
            if let Some(i) = state.active {
                state.sheets[i].seal();
            }
            return Some(Vec::new());
        }
        CommandId::ArrangeToFront => return Some(restack(state, Arrange::ToFront)),
        CommandId::ArrangeForward => return Some(restack(state, Arrange::Forward)),
        CommandId::ArrangeBackward => return Some(restack(state, Arrange::Backward)),
        CommandId::ArrangeToBack => return Some(restack(state, Arrange::ToBack)),
        CommandId::ArrangeAlignH => true,
        CommandId::ArrangeAlignV => false,
        _ => return None,
    };
    if let Some(i) = state.active {
        let sheet = &mut state.sheets[i];
        let moves = align(sheet.doc.regions(), &sheet.canvas.selection, row);
        if !moves.is_empty() {
            sheet.seal();
            sheet.edit(Edit::MoveRegions { moves });
            sheet.seal();
        }
    }
    Some(Vec::new())
}
