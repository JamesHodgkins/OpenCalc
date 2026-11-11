//! A region left empty is removed when editing ends: clearing an input and moving
//! on deletes it, rather than leaving a blank box behind.

use opencalc_core::document::{Region, RegionKind};
use opencalc_core::edit::Edit;

use super::{CanvasCmd, CanvasUi};

/// Call once a frame, after the editor and the pointer have run. If the region that
/// was being edited last frame no longer is, and holds only blanks (counting the
/// edits still waiting in `cmds`), queue its deletion as one more undo step.
pub(super) fn remove_if_empty(st: &mut CanvasUi, regions: &[Region], cmds: &mut Vec<CanvasCmd>) {
    let was = std::mem::replace(&mut st.last_editing, st.editing);
    let Some(id) = was.filter(|id| st.editing != Some(*id)) else {
        return;
    };
    let Some(region) = regions.iter().find(|r| r.id == id) else {
        return;
    };
    // What the region will hold once the waiting edits are applied.
    let mut text = match &region.kind {
        RegionKind::Math(m) => m.source().to_owned(),
        RegionKind::Text { text } => text.clone(),
        _ => return,
    };
    for cmd in cmds.iter() {
        match cmd {
            CanvasCmd::Edit(Edit::SetMathSource { id: i, source }) if *i == id => {
                text.clone_from(source);
            }
            CanvasCmd::Edit(Edit::SetText { id: i, text: t }) if *i == id => text.clone_from(t),
            _ => {}
        }
    }
    if text.trim().is_empty() {
        st.selection.retain(|s| *s != id);
        cmds.push(CanvasCmd::Seal);
        cmds.push(CanvasCmd::Edit(Edit::DeleteRegions { ids: vec![id] }));
        cmds.push(CanvasCmd::Seal);
    }
}
