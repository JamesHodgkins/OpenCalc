//! The Format commands: how the selected math regions show their results.
//!
//! Display only. These change a region's `ResultFormat`, as one undoable step, and
//! never a value, so nothing is recalculated.

use opencalc_core::document::{
    DisplayUnit, FontStyle, Notation, RegionId, RegionKind, ResultFormat,
};
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
        .filter(|r| sheet.canvas.selection.contains(&r.id) && !r.locked)
        .filter_map(|r| match &r.kind {
            RegionKind::Math(m) => Some((r.id, m.format())),
            _ => None,
        })
        .collect()
}

/// The selected text regions and their styles, in layout order.
pub fn selected_styles(state: &AppState) -> Vec<(RegionId, FontStyle)> {
    let Some(sheet) = state.active_sheet() else {
        return Vec::new();
    };
    sheet
        .doc
        .regions()
        .iter()
        .filter(|r| sheet.canvas.selection.contains(&r.id) && !r.locked)
        .filter(|r| matches!(r.kind, RegionKind::Text { .. }))
        .map(|r| (r.id, r.style))
        .collect()
}

/// Whether every selected text region has `flag` on (and there is one).
pub fn all_styled(state: &AppState, flag: fn(&FontStyle) -> bool) -> bool {
    let styles = selected_styles(state);
    !styles.is_empty() && styles.iter().all(|(_, s)| flag(s))
}

/// Bold and italic, as the toggles switched on for the selected text regions.
pub fn style_commands(state: &AppState) -> Vec<CommandId> {
    let mut on = Vec::new();
    if all_styled(state, |s| s.bold) {
        on.push(CommandId::FormatBold);
    }
    if all_styled(state, |s| s.italic) {
        on.push(CommandId::FormatItalic);
    }
    on
}

/// Bold or italic: on for every selected text region, or off if all had it on.
fn toggle_style(state: &mut AppState, bold: bool) {
    let flag: fn(&FontStyle) -> bool = if bold { |s| s.bold } else { |s| s.italic };
    let on = !all_styled(state, flag);
    let edits: Vec<Edit> = selected_styles(state)
        .into_iter()
        .map(|(id, mut style)| {
            if bold {
                style.bold = on;
            } else {
                style.italic = on;
            }
            Edit::SetFontStyle { id, style }
        })
        .collect();
    if let (Some(i), false) = (state.active, edits.is_empty()) {
        let sheet = &mut state.sheets[i];
        sheet.seal();
        sheet.edit(Edit::Batch(edits));
        sheet.seal();
    }
}

/// Whether every selected math region shows `notation` (and there is one).
pub fn all_in(state: &AppState, notation: Notation) -> bool {
    let formats = selected_formats(state);
    !formats.is_empty() && formats.iter().all(|(_, f)| f.notation() == notation)
}

fn unit_choice(unit: Option<DisplayUnit>) -> Box<dyn Fn(ResultFormat) -> ResultFormat> {
    Box::new(move |f| f.with_unit(unit))
}

/// The display units that every selected math region has chosen.
pub fn units_in_use(state: &AppState) -> Vec<CommandId> {
    let formats = selected_formats(state);
    let Some((_, first)) = formats.first() else {
        return Vec::new();
    };
    let same = formats.iter().all(|(_, f)| f.unit() == first.unit());
    let command = match first.unit() {
        Some(DisplayUnit::Metre) => CommandId::UnitsDisplayM,
        Some(DisplayUnit::Millimetre) => CommandId::UnitsDisplayMm,
        Some(DisplayUnit::Inch) => CommandId::UnitsDisplayIn,
        Some(DisplayUnit::Newton) => CommandId::UnitsDisplayN,
        Some(DisplayUnit::Kilonewton) => CommandId::UnitsDisplayKn,
        None => return Vec::new(),
    };
    if same {
        vec![command]
    } else {
        Vec::new()
    }
}

/// Run a Format command. `None` if `command` is not one of them.
pub fn dispatch_format(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    use CommandId::*;
    if matches!(command, FormatBold | FormatItalic) {
        toggle_style(state, command == FormatBold);
        return Some(Vec::new());
    }
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
        UnitsDisplayM => unit_choice(Some(DisplayUnit::Metre)),
        UnitsDisplayMm => unit_choice(Some(DisplayUnit::Millimetre)),
        UnitsDisplayIn => unit_choice(Some(DisplayUnit::Inch)),
        UnitsDisplayN => unit_choice(Some(DisplayUnit::Newton)),
        UnitsDisplayKn => unit_choice(Some(DisplayUnit::Kilonewton)),
        UnitsDisplayReset => unit_choice(None),
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
