//! Align Horizontally and Align Vertically for the selected regions.

use opencalc_core::edit::Edit;

use super::dispatch::Effect;
use super::state::AppState;
use crate::canvas::align::align;
use crate::commands::CommandId;

/// Run an alignment command. `None` if `command` is not one.
pub fn dispatch_arrange(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    let row = match command {
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
