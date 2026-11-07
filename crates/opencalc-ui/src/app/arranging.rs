//! Align Horizontally, Align Vertically and Lock for the selected regions.

use opencalc_core::edit::Edit;

use super::dispatch::Effect;
use super::state::AppState;
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
