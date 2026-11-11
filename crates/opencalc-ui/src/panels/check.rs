//! The Check Worksheet window: what is wrong, one entry each. Clicking an entry
//! goes to that region. It reads the findings and asks for changes by command (R4).

use eframe::egui::{self, Align2, Context, RichText};
use opencalc_core::check::Severity;

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::icons;

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.check.open {
        return;
    }
    if state.active.is_none() {
        out.push(CommandId::CheckClose);
        return;
    }
    let theme = &state.theme;
    let mut open = true;
    egui::Window::new("Check Worksheet")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .pivot(Align2::RIGHT_BOTTOM)
        .default_pos(super::default_pos(
            ctx,
            Align2::RIGHT_BOTTOM,
            [-16.0, -40.0],
        ))
        .show(ctx, |ui| {
            let findings = &state.check.findings;
            if findings.is_empty() {
                ui.label(format!("{} No problems found.", icons::CHECK));
                return;
            }
            let errors = findings
                .iter()
                .filter(|f| f.severity == Severity::Error)
                .count();
            let summary = format!("{errors} error(s), {} warning(s)", findings.len() - errors);
            ui.label(RichText::new(summary).color(theme.text_weak));
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (i, f) in findings.iter().enumerate().take(500) {
                        let colour = match f.severity {
                            Severity::Error => theme.error,
                            Severity::Warning => theme.warning,
                        };
                        // A labelled region is named by its label, the source after it.
                        let source = f.source.as_deref().unwrap_or("");
                        let what = if f.label.is_empty() {
                            source.to_owned()
                        } else {
                            format!("{}  {source}", f.label)
                        };
                        let line = format!("{}  {what}\n{}", icons::WARNING, f.message);
                        let button = egui::Button::new(RichText::new(line).color(colour));
                        if ui.add(button.frame(false)).clicked() {
                            out.push(CommandId::GoToFinding(i as u16));
                        }
                    }
                });
        });
    if !open {
        out.push(CommandId::CheckClose);
    }
}
