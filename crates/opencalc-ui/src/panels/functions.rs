//! The Functions window: the built-in functions, searchable and grouped. Clicking
//! one adds it to the math region being edited. Optional, closed by default.

use eframe::egui::{self, RichText, Ui};
use opencalc_core::document::RegionKind;
use opencalc_core::edit::Edit;
use opencalc_core::functions::{all, category, FunctionDef};

use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::theme::type_size;

/// Group order in the window.
const GROUPS: [&str; 7] = [
    "Powers and logarithms",
    "Trigonometry",
    "Rounding and integers",
    "Complex numbers",
    "Vectors and matrices",
    "Statistics",
    "Number theory",
];

/// Whether `f` matches what was typed in the search box: every word must appear in
/// the function's name, signature or summary, ignoring case.
pub fn matches(f: &FunctionDef, query: &str) -> bool {
    let haystack = format!("{} {} {}", f.name, f.signature, f.summary).to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| haystack.contains(word))
}

/// The source text after adding `function` to `source`: its name and an opening
/// bracket, at the end. (When the editor has a caret this will insert at it.)
pub fn with_function(source: &str, function: &FunctionDef) -> String {
    format!("{source}{}(", function.name)
}

pub fn show(ui: &mut Ui, state: &AppState, out: &mut Vec<CanvasCmd>) {
    let theme = state.theme;
    ui.label(
        RichText::new("FUNCTIONS")
            .size(type_size::SMALL)
            .color(theme.text_weak),
    );
    let search_id = ui.id().with("function-search");
    let mut query: String = ui
        .data_mut(|d| d.get_temp::<String>(search_id))
        .unwrap_or_default();
    ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text("Search functions")
            .desired_width(f32::INFINITY),
    );
    ui.data_mut(|d| d.insert_temp(search_id, query.clone()));

    // The math region being edited, if any: where a click inserts.
    let target = state.active_sheet().and_then(|s| {
        let id = s.canvas.editing?;
        match &s.doc.region(id)?.kind {
            RegionKind::Math(m) => Some((id, m.source().to_owned())),
            _ => None,
        }
    });
    if target.is_none() {
        ui.label(
            RichText::new("Double-click a math region, then choose a function to add it.")
                .size(type_size::SMALL)
                .color(theme.text_weak),
        );
    }
    ui.add_space(4.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut any = false;
        for group in GROUPS {
            let items: Vec<&FunctionDef> = all()
                .filter(|f| category(f.name) == group && matches(f, &query))
                .collect();
            if items.is_empty() {
                continue;
            }
            any = true;
            ui.add_space(4.0);
            ui.label(
                RichText::new(group)
                    .size(type_size::SMALL)
                    .color(theme.text_weak),
            );
            for f in items {
                let row = ui
                    .add_enabled(
                        target.is_some(),
                        egui::Button::new(RichText::new(f.signature).monospace())
                            .frame(false)
                            .wrap_mode(egui::TextWrapMode::Truncate),
                    )
                    .on_hover_text(f.summary)
                    .on_disabled_hover_text(f.summary);
                if row.clicked() {
                    if let Some((id, source)) = &target {
                        out.push(CanvasCmd::Edit(Edit::SetMathSource {
                            id: *id,
                            source: with_function(source, f),
                        }));
                        out.push(CanvasCmd::Seal);
                    }
                }
                ui.label(
                    RichText::new(f.summary)
                        .size(type_size::SMALL)
                        .color(theme.text_weak),
                );
            }
        }
        if !any {
            ui.label(RichText::new("No function matches.").color(theme.text_weak));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::functions::lookup;

    fn f(name: &str) -> &'static FunctionDef {
        lookup(name).expect("a built-in")
    }

    #[test]
    fn search_matches_name_signature_and_summary_ignoring_case() {
        assert!(matches(f("sqrt"), "SQRT"));
        assert!(matches(f("sqrt"), "square root"));
        assert!(matches(f("det"), "determinant matrix"));
        assert!(matches(f("sum"), ""), "an empty search shows everything");
        assert!(!matches(f("sin"), "matrix"));
        assert!(!matches(f("sin"), "sine zebra"), "every word must match");
    }

    #[test]
    fn every_function_is_in_a_listed_group_and_findable_by_its_name() {
        for def in all() {
            assert!(GROUPS.contains(&category(def.name)), "{}", def.name);
            assert!(matches(def, def.name), "{}", def.name);
        }
    }

    #[test]
    fn choosing_a_function_adds_its_name_and_an_opening_bracket() {
        assert_eq!(with_function("", f("sin")), "sin(");
        assert_eq!(with_function("y := ", f("sqrt")), "y := sqrt(");
    }
}
