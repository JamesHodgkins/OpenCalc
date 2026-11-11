//! The Label window: a short name for one region. What is typed goes out as
//! `SetLabel` edits (rule R4), so it is undoable; the text box keeps its own copy
//! while it is open, so spaces typed at the end are not trimmed away under the
//! cursor.

use eframe::egui::{self, Align2, Context, Id, Key};
use opencalc_core::document::MAX_LABEL_CHARS;
use opencalc_core::edit::Edit;

use super::Commands;
use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;

/// What the window remembers while it is open: the text, for which region, and
/// whether it has taken the keyboard yet.
#[derive(Clone, Default)]
struct Form {
    region: Option<opencalc_core::document::RegionId>,
    text: String,
}

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands, edits: &mut Vec<CanvasCmd>) {
    let id = Id::new("label-form");
    let Some(region) = state.label_for else {
        ctx.data_mut(|d| d.remove::<Form>(id));
        return;
    };
    let Some(current) = state
        .active_sheet()
        .and_then(|s| s.doc.region(region))
        .map(|r| r.label.clone())
    else {
        out.push(CommandId::LabelClose);
        return;
    };
    let mut form: Form = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let fresh = form.region != Some(region);
    if fresh {
        form = Form {
            region: Some(region),
            text: current,
        };
    }
    let mut open = true;
    egui::Window::new("Label")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(super::default_pos(ctx, Align2::CENTER_CENTER, [0.0, 0.0]))
        .show(ctx, |ui| {
            ui.label("A short name to find this region by.");
            let before = form.text.clone();
            let field = ui.add(
                egui::TextEdit::singleline(&mut form.text)
                    .char_limit(MAX_LABEL_CHARS)
                    .hint_text("Label")
                    .desired_width(260.0),
            );
            if fresh {
                field.request_focus();
            }
            if form.text != before {
                edits.push(CanvasCmd::Edit(Edit::SetLabel {
                    id: region,
                    label: form.text.clone(),
                }));
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.button("Done").clicked() || enter {
                out.push(CommandId::LabelClose);
            }
        });
    if !open {
        out.push(CommandId::LabelClose);
    }
    ctx.data_mut(|d| d.insert_temp(id, form));
}
