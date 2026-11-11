//! The Format commands: how the selected math regions show their results.
//!
//! Display only. These change a region's `ResultFormat`, as one undoable step, and
//! never a value, so nothing is recalculated.

use opencalc_core::document::{
    FontStyle, Notation, RegionId, RegionKind, ResultFormat, Run, RunStyle,
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

/// The single selected text region, with the characters of it that are marked
/// (`start..end`, remembered from the editor) or, with none marked, the whole text.
/// `marked` says which.
pub struct TextTarget {
    pub id: RegionId,
    pub start: usize,
    pub end: usize,
    pub marked: bool,
    pub chars: usize,
    pub runs: Vec<Run>,
}

pub fn text_target(state: &AppState) -> Option<TextTarget> {
    let sheet = state.active_sheet()?;
    let [id] = sheet.canvas.selection.as_slice() else {
        return None;
    };
    let region = sheet.doc.region(*id)?;
    let RegionKind::Text { text } = &region.kind else {
        return None;
    };
    if region.locked {
        return None;
    }
    let chars = text.chars().count();
    let marked = sheet
        .canvas
        .text_sel
        .filter(|(sel, a, b)| sel == id && a < b && *b <= chars);
    let (start, end) = marked.map_or((0, chars), |(_, a, b)| (a, b));
    Some(TextTarget {
        id: *id,
        start,
        end,
        marked: marked.is_some(),
        chars,
        runs: region.runs.clone(),
    })
}

impl TextTarget {
    /// Whether every character of the target has `flag` in its run style.
    pub fn all_have(&self, flag: fn(&RunStyle) -> bool) -> bool {
        opencalc_core::document::runs_all_have(&self.runs, self.chars, self.start, self.end, flag)
    }

    /// The runs after `change` is applied to the target.
    pub fn restyled(&self, change: impl Fn(&mut RunStyle)) -> Vec<Run> {
        opencalc_core::document::restyle_runs(&self.runs, self.chars, self.start, self.end, change)
    }
}

/// Apply `change` to the marked characters (or the whole text), as one undo step.
pub fn restyle_text(state: &mut AppState, change: impl Fn(&mut RunStyle)) {
    let Some(target) = text_target(state) else {
        return;
    };
    let runs = target.restyled(change);
    if let Some(i) = state.active {
        let sheet = &mut state.sheets[i];
        sheet.seal();
        sheet.edit(Edit::SetRuns {
            id: target.id,
            runs,
        });
        sheet.seal();
    }
}

/// Bold, italic and underline, as the toggles switched on: for the marked
/// characters when some are marked, else for the selected text regions.
pub fn style_commands(state: &AppState) -> Vec<CommandId> {
    if let Some(t) = text_target(state).filter(|t| t.marked) {
        let mut on = Vec::new();
        if t.all_have(|s| s.bold) {
            on.push(CommandId::FormatBold);
        }
        if t.all_have(|s| s.italic) {
            on.push(CommandId::FormatItalic);
        }
        if t.all_have(|s| s.underline) {
            on.push(CommandId::FormatUnderline);
        }
        if all_bordered(state) {
            on.push(CommandId::FormatBorder);
        }
        return on;
    }
    let mut on = whole_region_style_commands(state);
    if text_target(state).is_some_and(|t| t.chars > 0 && t.all_have(|s| s.underline)) {
        on.push(CommandId::FormatUnderline);
    }
    on
}

/// Bold and italic, as the toggles switched on for the selected text regions.
fn whole_region_style_commands(state: &AppState) -> Vec<CommandId> {
    let mut on = Vec::new();
    if all_styled(state, |s| s.bold) {
        on.push(CommandId::FormatBold);
    }
    if all_styled(state, |s| s.italic) {
        on.push(CommandId::FormatItalic);
    }
    if all_bordered(state) {
        on.push(CommandId::FormatBorder);
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

/// The Result unit menu for the selection: the units the selected results can be
/// shown in (all of one kind, else none), and the unit the first one shows now.
pub fn unit_menu(state: &AppState) -> (Vec<CommandId>, Option<String>) {
    use opencalc_core::value::Value;
    let Some(sheet) = state.active_sheet() else {
        return (Vec::new(), None);
    };
    let system = sheet.doc.unit_system();
    let mut dim = None;
    let mut face = None;
    for (id, format) in selected_formats(state) {
        let Some(Ok(v @ Value::Scalar(s))) = state.output.values.get(&id) else {
            continue;
        };
        if s.dim.is_dimensionless() {
            continue;
        }
        if dim.is_some_and(|d| d != s.dim) {
            // Results of different kinds: only Automatic applies to them all.
            return (Vec::new(), Some("Mixed".to_owned()));
        }
        dim = Some(s.dim);
        if face.is_none() {
            face = crate::format::scalar_parts(v, system, &format)
                .map(|(_, unit)| crate::format::pretty_unit(&unit));
        }
    }
    let choices = dim
        .map(crate::format::unit_choices)
        .unwrap_or_default()
        .into_iter()
        .map(CommandId::UnitsShowIn)
        .collect();
    (choices, face)
}

/// The selected regions that are not locked, with whether each has a border.
fn selected_borders(state: &AppState) -> Vec<(RegionId, bool)> {
    let Some(sheet) = state.active_sheet() else {
        return Vec::new();
    };
    sheet
        .doc
        .regions()
        .iter()
        .filter(|r| sheet.canvas.selection.contains(&r.id) && !r.locked)
        .map(|r| (r.id, r.border))
        .collect()
}

/// Whether every selected region has a border (and there is one).
pub fn all_bordered(state: &AppState) -> bool {
    let b = selected_borders(state);
    !b.is_empty() && b.iter().all(|(_, on)| *on)
}

/// Border on for every selected region, or off if all had it on.
fn toggle_border(state: &mut AppState) {
    let on = !all_bordered(state);
    let edits: Vec<Edit> = selected_borders(state)
        .into_iter()
        .map(|(id, _)| Edit::SetBorder { id, border: on })
        .collect();
    if let (Some(i), false) = (state.active, edits.is_empty()) {
        let sheet = &mut state.sheets[i];
        sheet.seal();
        sheet.edit(Edit::Batch(edits));
        sheet.seal();
    }
}

/// Run a Format command. `None` if `command` is not one of them.
pub fn dispatch_format(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    use CommandId::*;
    if command == FormatBorder {
        toggle_border(state);
        return Some(Vec::new());
    }
    if command == FormatUnderline {
        if let Some(t) = text_target(state) {
            let on = !t.all_have(|s| s.underline);
            restyle_text(state, |s| s.underline = on);
        }
        return Some(Vec::new());
    }
    if matches!(command, FormatBold | FormatItalic) {
        // With characters marked in a text region, the style goes on those alone.
        if let Some(t) = text_target(state).filter(|t| t.marked) {
            if command == FormatBold {
                let on = !t.all_have(|s| s.bold);
                restyle_text(state, |s| s.bold = on);
            } else {
                let on = !t.all_have(|s| s.italic);
                restyle_text(state, |s| s.italic = on);
            }
            return Some(Vec::new());
        }
        toggle_style(state, command == FormatBold);
        return Some(Vec::new());
    }
    let choose: Box<dyn Fn(ResultFormat) -> ResultFormat> = match command {
        FormatDecimalsMore => Box::new(|f| f.more_decimals()),
        FormatDecimalsLess => Box::new(|f| f.fewer_decimals()),
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
        UnitsDisplayReset => Box::new(|f| f.with_unit(None)),
        UnitsShowIn(unit) => Box::new(move |f| f.with_custom_unit(Some(unit.to_owned()))),
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
