//! Region properties (right dock): counts, warnings, errors and the selection.

use eframe::egui::{self, RichText, Ui};
use opencalc_core::document::{MathRegion, PlotSpec, RegionId, RegionKind, MAX_SERIES};
use opencalc_core::edit::Edit;
use opencalc_core::eval::{EvalError, Output, Warning};
use opencalc_core::expr::Expr;
use opencalc_core::value::Value;

use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::icons;
use crate::theme::type_size;

pub fn show(ui: &mut Ui, state: &AppState, out: &mut Vec<CanvasCmd>) {
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
                if let RegionKind::Plot(spec) = &r.kind {
                    plot_editor(ui, state, *id, spec, out);
                }
            }
        }
        many => {
            ui.add_space(8.0);
            ui.label(format!("{} regions selected", many.len()));
        }
    }
}

/// One text field of the plot editor. Each change is one edit; losing focus ends
/// the run, so a whole field is one undo step (see `History`).
fn field(
    ui: &mut Ui,
    state: &AppState,
    id: RegionId,
    out: &mut Vec<CanvasCmd>,
    label: &str,
    text: &str,
    make: &dyn Fn(&str) -> PlotSpec,
) {
    ui.label(
        RichText::new(label)
            .size(type_size::SMALL)
            .color(state.theme.text_weak),
    );
    let mut buffer = text.to_owned();
    let response = ui.add(egui::TextEdit::singleline(&mut buffer).desired_width(f32::INFINITY));
    if response.changed() {
        out.push(CanvasCmd::Edit(Edit::SetPlot {
            id,
            spec: make(&buffer),
        }));
    }
    if response.lost_focus() {
        out.push(CanvasCmd::Seal);
    }
}

/// Fields for what a plot shows. Each change is an edit of the whole spec, built
/// from the current one, so the panel keeps no state of its own.
fn plot_editor(
    ui: &mut Ui,
    state: &AppState,
    id: RegionId,
    spec: &PlotSpec,
    out: &mut Vec<CanvasCmd>,
) {
    let theme = state.theme;
    ui.add_space(8.0);
    ui.label(
        RichText::new("PLOT")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    field(ui, state, id, out, "Title", &spec.title, &|t| {
        spec.clone().with_title(t)
    });
    field(ui, state, id, out, "x values", spec.x.source(), &|t| {
        spec.clone().with_x(t)
    });
    for (k, series) in spec.series.iter().enumerate() {
        field(
            ui,
            state,
            id,
            out,
            &format!("Series {}", k + 1),
            series.source(),
            &|t| spec.clone().with_series(k, t),
        );
    }
    ui.horizontal(|ui| {
        if spec.series.len() < MAX_SERIES && ui.button("Add series").clicked() {
            let next = spec.series.len();
            out.push(CanvasCmd::Edit(Edit::SetPlot {
                id,
                spec: spec.clone().with_series(next, ""),
            }));
            out.push(CanvasCmd::Seal);
        }
        if spec.series.len() > 1 && ui.button("Remove last").clicked() {
            let last = spec.series.len() - 1;
            out.push(CanvasCmd::Edit(Edit::SetPlot {
                id,
                spec: spec.clone().without_series(last),
            }));
            out.push(CanvasCmd::Seal);
        }
    });
    field(ui, state, id, out, "x label", &spec.x_label, &|t| {
        spec.clone().with_labels(t, &spec.y_label)
    });
    field(ui, state, id, out, "y label", &spec.y_label, &|t| {
        spec.clone().with_labels(&spec.x_label, t)
    });
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
