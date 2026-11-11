//! The Find and Replace window. It reads the find state and asks for changes:
//! what is typed goes out as [`FindCmd`]s, the buttons as commands (rule R4).

use eframe::egui::{self, Align2, Context, Key, RichText, Ui};

use super::Commands;
use crate::app::finding::FindCmd;
use crate::app::AppState;
use crate::commands::CommandId;

/// What the count beside the query says.
pub fn count_text(state: &AppState) -> String {
    let find = &state.find;
    if find.query.is_empty() {
        String::new()
    } else if find.matches.is_empty() {
        "No matches".to_owned()
    } else {
        let more = if find.matches.len() >= opencalc_core::find::MAX_MATCHES {
            "+"
        } else {
            ""
        };
        format!("{} of {}{more}", find.current + 1, find.matches.len())
    }
}

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands, typed: &mut Vec<FindCmd>) {
    if !state.find.open {
        return;
    }
    if state.active.is_none() {
        out.push(CommandId::FindClose);
        return;
    }
    let mut open = true;
    egui::Window::new("Find")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::RIGHT_BOTTOM)
        .default_pos(super::default_pos(
            ctx,
            Align2::RIGHT_BOTTOM,
            [-16.0, -40.0],
        ))
        .show(ctx, |ui| body(ui, state, out, typed));
    if !open {
        out.push(CommandId::FindClose);
    }
}

fn body(ui: &mut Ui, state: &AppState, out: &mut Commands, typed: &mut Vec<FindCmd>) {
    let find = &state.find;
    let weak = state.theme.text_weak;
    let mut query = find.query.clone();
    let row = ui.horizontal(|ui| {
        let r = ui.add(
            egui::TextEdit::singleline(&mut query)
                .hint_text("Find")
                .desired_width(200.0),
        );
        ui.label(RichText::new(count_text(state)).color(weak));
        r
    });
    let query_box = row.inner;
    if find.focus {
        query_box.request_focus();
        typed.push(FindCmd::FocusTaken);
    }
    if query != find.query {
        typed.push(FindCmd::Query(query));
    }
    if query_box.lost_focus() {
        let (enter, shift, escape) = ui.input(|i| {
            (
                i.key_pressed(Key::Enter),
                i.modifiers.shift,
                i.key_pressed(Key::Escape),
            )
        });
        if enter {
            out.push(if shift {
                CommandId::FindPrev
            } else {
                CommandId::FindNext
            });
            query_box.request_focus();
        } else if escape {
            out.push(CommandId::FindClose);
        }
    }
    if find.replace {
        let mut replacement = find.replacement.clone();
        ui.add(
            egui::TextEdit::singleline(&mut replacement)
                .hint_text("Replace with")
                .desired_width(200.0),
        );
        if replacement != find.replacement {
            typed.push(FindCmd::Replacement(replacement));
        }
    }
    ui.horizontal(|ui| {
        let mut case = find.options.match_case;
        if ui.checkbox(&mut case, "Match case").changed() {
            typed.push(FindCmd::MatchCase(case));
        }
        let mut word = find.options.whole_word;
        if ui.checkbox(&mut word, "Whole word").changed() {
            typed.push(FindCmd::WholeWord(word));
        }
    });
    let any = !find.matches.is_empty();
    ui.horizontal(|ui| {
        for command in [CommandId::FindPrev, CommandId::FindNext] {
            let label = if command == CommandId::FindNext {
                "Next"
            } else {
                "Previous"
            };
            if ui.add_enabled(any, egui::Button::new(label)).clicked() {
                out.push(command);
            }
        }
        if find.replace {
            for command in [CommandId::FindReplaceOne, CommandId::FindReplaceAll] {
                if ui
                    .add_enabled(any, egui::Button::new(command.label()))
                    .clicked()
                {
                    out.push(command);
                }
            }
        } else if ui.button("Replace\u{2026}").clicked() {
            out.push(CommandId::EditReplace);
        }
    });
}
