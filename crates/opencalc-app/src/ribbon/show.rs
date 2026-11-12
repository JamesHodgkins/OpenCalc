//! Laying out and drawing the ribbon.

use eframe::egui::{self, Align2, CornerRadius, FontId, Sense, Ui, Vec2};

use super::cells::{large_split, rows_ui, small_item};
use super::widgets::{self as w, Look};
use super::{Column, CommandSource, Item, Panel, Ribbon, RibbonOutput, RibbonState};
use crate::commands::CommandId;
use crate::theme::{metrics, type_size, Theme};

/// Everything a draw call needs, so helpers take one argument.
pub(super) struct Cx<'a> {
    pub(super) theme: &'a Theme,
    pub(super) source: &'a dyn CommandSource,
    pub(super) state: &'a mut RibbonState,
    pub(super) out: &'a mut RibbonOutput,
}

impl Cx<'_> {
    pub(super) fn look(&self, command: CommandId) -> Look {
        Look {
            enabled: self.source.is_enabled(command),
            active: self.source.is_active(command),
        }
    }

    pub(super) fn tip(&self, item: &Item) -> String {
        match self.source.shortcut_text(item.command) {
            Some(s) => format!("{} ({s})", item.label),
            None => item.label.to_owned(),
        }
    }

    pub(super) fn press(&mut self, item: &Item, look: Look) {
        if look.enabled {
            self.out.clicked = Some(item.command);
        }
    }
}

impl Ribbon {
    /// The row of tab titles. The active tab takes the ribbon body's tone.
    pub fn tab_strip(&self, ui: &mut Ui, theme: &Theme, state: &mut RibbonState) {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        let current = state.active_tab.min(self.tabs.len().saturating_sub(1));
        state.active_tab = current;
        for (i, tab) in self.tabs.iter().enumerate() {
            let active = i == current;
            let width = w::text_width(ui, tab.title, type_size::TAB) + 24.0;
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(width, metrics::TAB_STRIP), Sense::click());
            let top = CornerRadius {
                nw: 4,
                ne: 4,
                sw: 0,
                se: 0,
            };
            if active {
                ui.painter().rect_filled(rect, top, theme.raised);
            }
            let colour = if active || resp.hovered() {
                theme.text
            } else {
                theme.text_weak
            };
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                tab.title,
                FontId::proportional(type_size::TAB),
                colour,
            );
            if resp.clicked() {
                state.active_tab = i;
                state.open_panel = None;
            }
        }
    }

    /// Draw the active tab's panels. Returns the clicked command, if any.
    pub fn show(
        &self,
        ui: &mut Ui,
        theme: &Theme,
        state: &mut RibbonState,
        source: &dyn CommandSource,
    ) -> RibbonOutput {
        let mut out = RibbonOutput::default();
        let tab_index = state.active_tab.min(self.tabs.len().saturating_sub(1));
        let Some(tab) = self.tabs.get(tab_index) else {
            return out;
        };
        let mut cx = Cx {
            theme,
            source,
            state,
            out: &mut out,
        };
        let mut _used = 0.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            for (pi, panel) in tab.panels.iter().enumerate() {
                panel_ui(ui, &mut cx, panel, (tab_index, pi));
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(1.0, w::LARGE_H + 16.0), Sense::hover());
                ui.painter().rect_filled(rect, 0.0, theme.base);
            }
            _used = ui.min_rect().width();
        });
        #[cfg(test)]
        {
            out.width = _used;
        }
        out
    }
}

fn panel_ui(ui: &mut Ui, cx: &mut Cx, panel: &Panel, key: (usize, usize)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        let body = ui.horizontal_top(|ui| {
            for (ci, col) in panel.columns.iter().enumerate() {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                    ui.set_min_height(w::LARGE_H);
                    ui.set_max_height(w::LARGE_H);
                    column_ui(ui, cx, col, (key, ci));
                });
            }
        });
        // The big Greek button opens the panel's flyout instead of running a command.
        if cx
            .out
            .clicked
            .take_if(|c| *c == CommandId::InsertGreek)
            .is_some()
        {
            cx.state.open_panel = (cx.state.open_panel != Some(key)).then_some(key);
        }
        title_row(ui, cx, panel, key, body.response.rect.width());
    });
}

/// Height of a panel's title row: the same on every panel.
pub const TITLE_H: f32 = 16.0;

/// The panel title, centred under the panel (with the flyout arrow, if any).
fn title_row(ui: &mut Ui, cx: &mut Cx, panel: &Panel, key: (usize, usize), width: f32) {
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(Vec2::new(width, TITLE_H), layout, |ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        let label = w::text_width(ui, panel.title, type_size::RIBBON_TITLE);
        let arrow_w = if panel.extra.is_empty() { 0.0 } else { 14.0 };
        ui.add_space(((width - label - arrow_w) / 2.0).max(0.0));
        ui.label(
            egui::RichText::new(panel.title)
                .size(type_size::RIBBON_TITLE)
                .color(cx.theme.text_weak),
        );
        if panel.extra.is_empty() {
            return;
        }
        let look = Look {
            enabled: true,
            active: cx.state.open_panel == Some(key),
        };
        let (rect, arrow) = ui.allocate_exact_size(Vec2::new(12.0, TITLE_H), look.sense());
        w::paint_state(ui, rect, &arrow, cx.theme, look);
        w::paint_arrow(ui, arrow.rect, cx.theme, look);
        if arrow.clicked() {
            cx.state.open_panel = if look.active { None } else { Some(key) };
        }
        if look.active {
            super::flyout::flyout(ui, cx, panel, key, &arrow);
        }
    });
}

fn column_ui(ui: &mut Ui, cx: &mut Cx, col: &Column, id: ((usize, usize), usize)) {
    match col {
        Column::Large(item) => {
            let look = cx.look(item.command);
            let width = w::large_width(ui, item.label);
            let resp = w::large_button(ui, cx.theme, look, item.icon, item.label, width);
            if resp.on_hover_text(cx.tip(item)).clicked() {
                cx.press(item, look);
            }
        }
        Column::LargeSplit(split) => large_split(ui, cx, split, id),
        Column::Stack(items) => {
            let width = items
                .iter()
                .map(|i| w::small_width(ui, true, Some(i.label)))
                .fold(0.0, f32::max);
            for item in items {
                small_item(ui, cx, item, Some(item.label), width);
            }
        }
        Column::Rows(rows) => rows_ui(ui, cx, rows),
    }
}
