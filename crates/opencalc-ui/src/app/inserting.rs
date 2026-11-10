//! Insert Matrix and Insert Range: a template written into the math region being
//! edited (replacing the selection), or into a new math region at the insertion
//! point. The first value is left selected, so typing replaces it.

use opencalc_core::document::RegionKind;
use opencalc_core::edit::Edit;

use super::dispatch::{apply_canvas, Effect};
use super::state::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;

/// Most rows or columns a matrix can be inserted with.
pub const MAX_SIDE: u8 = 10;

/// Most cells a matrix can be inserted with: an expression holds at most 128
/// symbols, and each cell takes two (a number and a separator).
pub const MAX_CELLS: u8 = 60;

/// `[0, 0; 0, 0]` for 2 by 2: zeros, so it evaluates as it stands, and where the
/// first one is (a character range) so it can be selected.
pub fn matrix_source(rows: u8, cols: u8) -> (String, (usize, usize)) {
    let rows = rows.clamp(1, MAX_SIDE);
    let cols = cols.clamp(1, MAX_SIDE).min(MAX_CELLS / rows);
    let row = vec!["0"; usize::from(cols)].join(", ");
    let text = format!("[{}]", vec![row; usize::from(rows)].join("; "));
    (text, (1, 2))
}

/// The range template: the numbers 1 to 10, the first selected.
fn range_source() -> (String, (usize, usize)) {
    ("1..10".to_owned(), (0, 1))
}

/// A solve block that works as it stands (it finds 2), the name selected.
fn solve_block_source() -> (String, (usize, usize)) {
    (
        "answer := given\n    x := 1\n    x ^ 2 == 4\nfind(x)".to_owned(),
        (0, 6),
    )
}

/// A program that works as it stands, the name selected.
fn program_source() -> (String, (usize, usize)) {
    (
        "double(x) := program\n    y \u{2190} x * 2\n    return y".to_owned(),
        (0, 6),
    )
}

/// Write `text` in, with the characters `select` (relative to its start) selected.
fn insert_source(state: &mut AppState, text: &str, select: (usize, usize)) -> Vec<Effect> {
    let Some(i) = state.active else {
        return Vec::new();
    };
    let canvas = &mut state.sheets[i].canvas;
    if let (Some(id), Some(field)) = (canvas.editing, canvas.field.as_ref()) {
        let mut after = field.clone();
        let start = after.selection().0;
        after.insert(text);
        after.select(start + select.0, start + select.1);
        let source = after.text();
        canvas.field = Some(after);
        return apply_canvas(
            state,
            i,
            vec![CanvasCmd::Edit(Edit::SetMathSource { id, source })],
        );
    }
    let sheet = &mut state.sheets[i];
    sheet.seal();
    sheet.insert(RegionKind::math(text));
    sheet.seal();
    sheet.canvas.preselect = Some(select);
    vec![Effect::Recalculate]
}

/// Run an Insert Matrix or Insert Range command. `None` if it is neither.
pub fn dispatch_insert(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    match command {
        CommandId::InsertMatrix => state.matrix_open = true,
        CommandId::MatrixClose => state.matrix_open = false,
        CommandId::InsertMatrixSize(rows, cols) => {
            state.matrix_open = false;
            let (text, select) = matrix_source(rows, cols);
            return Some(insert_source(state, &text, select));
        }
        CommandId::InsertSolveBlock => {
            let (text, select) = solve_block_source();
            return Some(insert_source(state, &text, select));
        }
        CommandId::InsertProgram => {
            let (text, select) = program_source();
            return Some(insert_source(state, &text, select));
        }
        CommandId::InsertRange => {
            let (text, select) = range_source();
            return Some(insert_source(state, &text, select));
        }
        _ => return None,
    }
    Some(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::parser::parse;

    #[test]
    fn a_matrix_template_parses_and_has_the_size_asked_for() {
        assert_eq!(matrix_source(2, 2).0, "[0, 0; 0, 0]");
        assert_eq!(matrix_source(1, 3).0, "[0, 0, 0]");
        assert_eq!(matrix_source(3, 1).0, "[0; 0; 0]");
        for (r, c) in [
            (1, 1),
            (2, 3),
            (10, 10),
            (0, 0),
            (200, 200),
            (6, 10),
            (10, 6),
        ] {
            let (text, (a, b)) = matrix_source(r, c);
            let parsed = parse(&text);
            assert!(
                !matches!(parsed, opencalc_core::expr::Expr::Error(_)),
                "{text}: {parsed:?}"
            );
            assert_eq!(&text[a..b], "0", "the first cell is the one selected");
        }
        // Sizes are held to what the dialog offers.
        assert_eq!(matrix_source(0, 0).0, "[0]");
        assert_eq!(
            matrix_source(200, 1).0.matches(';').count(),
            usize::from(MAX_SIDE) - 1
        );
    }
}

#[cfg(test)]
mod block_tests {
    use super::*;
    use opencalc_core::eval::{evaluate, RegionId};
    use opencalc_core::parser::parse;

    /// Evaluate the template, then a region that reads what it defines.
    fn value_of(template: &str, then: &str) -> f64 {
        let out = evaluate(&[(RegionId(1), parse(template)), (RegionId(2), parse(then))]);
        out.values[&RegionId(2)]
            .as_ref()
            .unwrap_or_else(|e| panic!("{template:?}: {e}"))
            .as_real()
            .unwrap()
            .si
    }

    #[test]
    fn the_solve_block_and_program_templates_work_as_they_stand() {
        let (solve, (a, b)) = solve_block_source();
        assert_eq!(&solve[a..b], "answer");
        assert!((value_of(&solve, "answer =") - 2.0).abs() < 1e-7);
        let (program, (a, b)) = program_source();
        assert_eq!(&program[a..b], "double");
        assert_eq!(value_of(&program, "double(21)"), 42.0);
    }
}
