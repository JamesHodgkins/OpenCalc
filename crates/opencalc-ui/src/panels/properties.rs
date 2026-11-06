//! Region properties (right dock): counts, warnings, errors and the selection.

use eframe::egui::{RichText, Ui};
use opencalc_core::document::{MathRegion, RegionId, RegionKind};
use opencalc_core::eval::{EvalError, Output, Warning};
use opencalc_core::expr::Expr;
use opencalc_core::value::Value;

use crate::app::AppState;
use crate::icons;
use crate::theme::type_size;

pub fn show(ui: &mut Ui, state: &AppState) {
    let theme = state.theme;
    ui.label(
        RichText::new("REGION PROPERTIES")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    let Some(sheet) = state.active_sheet() else {
        ui.label(RichText::new("No worksheet open").color(theme.text_weak));
        return;
    };
    ui.label(format!("{} regions", sheet.doc.regions().len()));
    ui.label(format!("Unit system: {}", sheet.doc.unit_system().label()));
    ui.add_space(8.0);
    for w in &state.output.warnings {
        let Warning::Redefinition { name, regions } = w;
        ui.colored_label(
            theme.warning,
            format!(
                "{} '{name}' defined {} times",
                icons::WARNING,
                regions.len()
            ),
        );
    }
    for r in sheet.doc.regions() {
        let RegionKind::Math(m) = &r.kind else {
            continue;
        };
        if let Some(problem) = diagnose(m, state.output.values.get(&r.id), &state.output, r.id) {
            ui.colored_label(theme.error, format!("{} {}", icons::WARNING, m.source()));
            ui.label(
                RichText::new(problem)
                    .size(type_size::SMALL)
                    .color(theme.text_weak),
            );
        }
    }
    match sheet.canvas.selection.as_slice() {
        [] => {}
        [id] => {
            if let Some(r) = sheet.doc.region(*id) {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("SELECTED")
                        .size(type_size::SMALL)
                        .color(theme.text_weak),
                );
                ui.label(format!("{} region", r.kind.label()));
                ui.label(format!("x {:.0}  y {:.0}", r.pos.x, r.pos.y));
                ui.label(format!("{:.0} \u{d7} {:.0}", r.size.w, r.size.h));
            }
        }
        many => {
            ui.add_space(8.0);
            ui.label(format!("{} regions selected", many.len()));
        }
    }
}

/// What is wrong with a math region and where, or `None` if nothing is. A parse
/// error names the characters it points at; an evaluation error names the part
/// of the expression that failed.
fn diagnose(
    m: &MathRegion,
    result: Option<&Result<Value, EvalError>>,
    output: &Output,
    id: RegionId,
) -> Option<String> {
    if let Expr::Error(e) = m.expr() {
        let chars: Vec<char> = m.source().chars().collect();
        let end = e.end.min(chars.len());
        let shown: String = chars[e.start.min(end)..end].iter().collect();
        return Some(if shown.is_empty() {
            format!("{} at the end", e.message)
        } else {
            format!("{} at \u{201c}{shown}\u{201d}", e.message)
        });
    }
    let Some(Err(error)) = result else {
        return None;
    };
    let part = output
        .error_paths
        .get(&id)
        .and_then(|path| m.expr().at_path(path));
    Some(match part {
        Some(part) => format!("{error}, in \u{201c}{part}\u{201d}"),
        None => error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::eval::evaluate;

    fn diagnosis(source: &str) -> Option<String> {
        let m = MathRegion::new(source);
        let id = RegionId(1);
        let out = evaluate(&[(id, m.expr().clone())]);
        diagnose(&m, out.values.get(&id), &out, id)
    }

    #[test]
    fn a_working_region_has_no_diagnosis() {
        assert_eq!(diagnosis("x := 1 m + 2 m"), None);
    }

    #[test]
    fn a_parse_error_quotes_the_characters_it_points_at() {
        assert_eq!(
            diagnosis("1 + * 2").as_deref(),
            Some("unexpected '*' at \u{201c}*\u{201d}")
        );
        assert_eq!(
            diagnosis("(1 + 2").as_deref(),
            Some("missing ')' at the end")
        );
    }

    #[test]
    fn an_evaluation_error_quotes_the_failing_part() {
        let text = diagnosis("y := 2 * sqrt(1 / 0)").unwrap();
        assert!(text.ends_with("in \u{201c}1 / 0\u{201d}"), "{text}");
        let text = diagnosis("z := 3 m + 1 s").unwrap();
        assert!(text.contains("in \u{201c}3 m + 1 s\u{201d}"), "{text}");
    }
}
