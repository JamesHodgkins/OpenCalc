//! The autocomplete list drawn under the caret.

use eframe::egui::{self, Id, Order, Pos2, Rect, RichText, Ui};

use super::complete::Candidate;
use crate::canvas::Env;

/// Draw the list at `at`. Returns its rectangle and the entry clicked, if any.
pub(super) fn show(
    ui: &Ui,
    id: Id,
    at: Pos2,
    list: &[Candidate],
    pick: usize,
    env: &Env,
) -> (Rect, Option<usize>) {
    let mut clicked = None;
    let area = egui::Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .fill(env.theme.surface)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, c) in list.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let selected = i == pick.min(list.len() - 1);
                            let label = RichText::new(&c.label).monospace();
                            if ui.selectable_label(selected, label).clicked() {
                                clicked = Some(i);
                            }
                            ui.label(RichText::new(&c.detail).small().color(env.theme.text_weak));
                        });
                    }
                });
        });
    (area.response.rect, clicked)
}
