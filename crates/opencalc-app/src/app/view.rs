//! Commands about how the worksheet is looked at and which windows are open: zoom,
//! grid and snap, page and draft view, and the Units, Page setup, Shortcuts and
//! About windows.

use super::actions::{zoom_by, zoom_to};
use super::dispatch::Effect;
use super::state::AppState;
use crate::commands::CommandId;

/// Run a view or window command. `None` means it is not one of these.
pub(super) fn dispatch_view(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    use CommandId::*;
    let mut fx = Vec::new();
    match command {
        ViewZoomIn => zoom_by(state, 1.25),
        ViewZoomOut => zoom_by(state, 1.0 / 1.25),
        ViewZoom100 => zoom_to(state, 1.0),
        ZoomTo(percent) => zoom_to(state, f32::from(percent) / 100.0),
        ViewZoomFit => {
            let size = state.viewport.size();
            if let Some(i) = state.active {
                let sheet = &mut state.sheets[i];
                if let Some(bounds) = sheet.bounds() {
                    sheet.view.fit(bounds, size, 24.0);
                }
            }
        }
        ViewGrid => state.show_grid = !state.show_grid,
        ViewSnap => state.snap = !state.snap,
        ViewFunctions => state.show_functions = !state.show_functions,
        ViewResetLayout => {
            state.show_functions = false;
            state.layout_gen += 1;
            fx.push(Effect::ResetDockWidths);
        }
        UnitsDefine | UnitsManage => state.units_open = true,
        UnitsClose => state.units_open = false,
        DocPageSetup => state.page_setup_open = true,
        DocPageSetupClose => {
            state.page_setup_open = false;
            if let Some(i) = state.active {
                state.sheets[i].seal();
            }
        }
        ViewPageView => state.draft = false,
        ViewDraftView => state.draft = true,
        _ => return None,
    }
    Some(fx)
}
