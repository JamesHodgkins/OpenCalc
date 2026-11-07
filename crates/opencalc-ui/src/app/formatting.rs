//! The Format commands: how the selected math regions show their results.
//!
//! Display only. These change a region's `ResultFormat`, as one undoable step, and
//! never a value, so nothing is recalculated.

use opencalc_core::document::{Notation, RegionId, RegionKind, ResultFormat};
use opencalc_core::edit::Edit;

use super::dispatch::Effect;
use super::state::AppState;
use crate::commands::CommandId;

/// The selected math regions and their formats, in layout order.
pub fn selected_formats(state: &AppState) -> Vec<(RegionId, ResultFormat)> {
    let Some(sheet) = state.active_sheet() else {
        return Vec::new();
    };
    sheet
        .doc
        .regions()
        .iter()
        .filter(|r| sheet.canvas.selection.contains(&r.id))
        .filter_map(|r| match &r.kind {
            RegionKind::Math(m) => Some((r.id, m.format())),
            _ => None,
        })
        .collect()
}

/// Whether every selected math region shows `notation` (and there is one).
pub fn all_in(state: &AppState, notation: Notation) -> bool {
    let formats = selected_formats(state);
    !formats.is_empty() && formats.iter().all(|(_, f)| f.notation() == notation)
}

/// Run a Format command. `None` if `command` is not one of them.
pub fn dispatch_format(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    use CommandId::*;
    let choose: Box<dyn Fn(ResultFormat) -> ResultFormat> = match command {
        FormatDecimalsMore => Box::new(ResultFormat::more_decimals),
        FormatDecimalsLess => Box::new(ResultFormat::fewer_decimals),
        FormatScientific | FormatEngineering => {
            let notation = if command == FormatScientific {
                Notation::Scientific
            } else {
                Notation::Engineering
            };
            // Pressing it again, with everything already so, puts it back.
            let target = if all_in(state, notation) {
                Notation::General
            } else {
                notation
            };
            Box::new(move |f| f.with_notation(target))
        }
        _ => return None,
    };
    let formats = selected_formats(state);
    let edits: Vec<Edit> = formats
        .into_iter()
        .map(|(id, f)| Edit::SetResultFormat {
            id,
            format: choose(f),
        })
        .collect();
    if let (Some(i), false) = (state.active, edits.is_empty()) {
        let sheet = &mut state.sheets[i];
        sheet.seal();
        sheet.edit(Edit::Batch(edits));
        sheet.seal();
    }
    Some(Vec::new())
}
