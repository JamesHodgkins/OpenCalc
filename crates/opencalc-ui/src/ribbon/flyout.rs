//! The expanded panel of the ribbon: an icon-only grid in a foreground area.

use eframe::egui::{self, CornerRadius, Id, Order, Ui, Vec2};

use super::show::Cx;
use super::widgets as w;
use super::Panel;
use crate::theme::{metrics, type_size};

/// The expanded panel: an icon-only grid in a foreground area under the panel.
pub(super) fn flyout(
    ui: &mut Ui,
    cx: &mut Cx,
    panel: &Panel,
    key: (usize, usize),
    arrow: &egui::Response,
) {
    let ctx = ui.ctx().clone();
    let pos = arrow.rect.left_bottom() + Vec2::new(-80.0, 4.0);
    let frame = egui::Frame::popup(&ctx.style())
        .fill(cx.theme.surface)
        .corner_radius(CornerRadius::same(metrics::RADIUS))
        .inner_margin(4.0);
    let area = egui::Area::new(Id::new(("ribbon-flyout", key)))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(&ctx, |ui| {
            frame.show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(2.0);
                ui.set_max_width(26.0 * 8.0 + 14.0);
                ui.horizontal_wrapped(|ui| {
                    for item in &panel.extra {
                        let look = cx.look(item.command);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), look.sense());
                        w::paint_state(ui, rect, &resp, cx.theme, look);
                        w::paint_small(ui, rect, cx.theme, look, Some(item.icon), None);
                        if resp.on_hover_text(cx.tip(item)).clicked() {
                            cx.press(item, look);
                            cx.state.open_panel = None;
                        }
                    }
                });
                ui.label(
                    egui::RichText::new(panel.title)
                        .size(type_size::RIBBON_TITLE)
                        .color(cx.theme.text_weak),
                );
            });
        });
    let outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !area.response.rect.contains(p) && !arrow.rect.contains(p))
    });
    if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cx.state.open_panel = None;
    }
}
