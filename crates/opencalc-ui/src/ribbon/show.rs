//! Laying out and drawing the ribbon.

use eframe::egui::{
    self, popup::popup_below_widget, Align2, CornerRadius, FontId, Id, PopupCloseBehavior, Rect,
    Sense, Ui, Vec2,
};

use super::widgets::{self as w, Look};
use super::{Cell, Column, CommandSource, Item, Panel, Ribbon, RibbonOutput, RibbonState, Split};
use crate::commands::CommandId;
use crate::theme::{metrics, type_size, Theme};

/// Everything a draw call needs, so helpers take one argument.
pub(super) struct Cx<'a> {
    pub(super) theme: &'a Theme,
    source: &'a dyn CommandSource,
    pub(super) state: &'a mut RibbonState,
    out: &'a mut RibbonOutput,
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
        for (i, tab) in self.tabs.iter().enumerate() {
            let active = i == state.active_tab;
            let width = w::text_width(ui, tab.title, type_size::TAB) + 24.0;
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(width, metrics::TAB_STRIP), Sense::click());
            if active {
                let top = CornerRadius {
                    nw: 4,
                    ne: 4,
                    sw: 0,
                    se: 0,
                };
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
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            for (pi, panel) in tab.panels.iter().enumerate() {
                panel_ui(ui, &mut cx, panel, (tab_index, pi));
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(1.0, w::LARGE_H + 16.0), Sense::hover());
                ui.painter().rect_filled(rect, 0.0, theme.base);
            }
        });
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

fn small_item(ui: &mut Ui, cx: &mut Cx, item: &Item, label: Option<&str>, width: f32) {
    let look = cx.look(item.command);
    let resp = w::small_button(ui, cx.theme, look, Some(item.icon), label, width);
    if resp.on_hover_text(cx.tip(item)).clicked() {
        cx.press(item, look);
    }
}

/// Width of a run-time menu: fixed, so the ribbon does not shift as the selection
/// changes what the menu reads.
const MENU_W: f32 = 112.0;

fn cell_width(cell: &Cell) -> f32 {
    match cell {
        Cell::Menu { .. } => MENU_W,
        Cell::Icon(_) => w::SMALL_H,
    }
}

fn split_label<'a>(split: &'a Split, pick: Option<&'a Item>) -> &'a str {
    split
        .label
        .or(pick.map(|p| p.label))
        .or(split.items.first().map(|i| i.label))
        .unwrap_or("")
}

fn rows_ui(ui: &mut Ui, cx: &mut Cx, rows: &[Vec<Cell>]) {
    let widths: Vec<Vec<f32>> = rows
        .iter()
        .map(|r| r.iter().map(cell_width).collect())
        .collect();
    let col_w = widths
        .iter()
        .map(|r| r.iter().sum::<f32>() + 2.0 * r.len().saturating_sub(1) as f32)
        .fold(0.0, f32::max);
    for (ri, row) in rows.iter().enumerate() {
        // Every row is exactly one small-button high, whatever it holds.
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        ui.allocate_ui_with_layout(Vec2::new(col_w, w::SMALL_H), layout, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
            let used: f32 =
                widths[ri].iter().sum::<f32>() + 2.0 * row.len().saturating_sub(1) as f32;
            // The last labelled cell takes the spare width so stacked rows line up.
            let last_labelled = row.iter().rposition(|c| matches!(c, Cell::Menu { .. }));
            for (ci, cell) in row.iter().enumerate() {
                let extra = if Some(ci) == last_labelled {
                    (col_w - used).max(0.0)
                } else {
                    0.0
                };
                let width = widths[ri][ci] + extra;
                match cell {
                    Cell::Icon(item) => small_item(ui, cx, item, None, width),
                    Cell::Menu { menu, first } => menu_cell(ui, cx, menu, first, width),
                }
            }
        });
    }
}

/// The item a split currently runs: the last pick, else the first.
fn current_pick<'a>(cx: &Cx, split: &'a Split) -> Option<&'a Item> {
    let first = split.items.first()?;
    let picked = cx.state.split_picks.get(&first.command);
    Some(
        picked
            .and_then(|c| split.items.iter().find(|i| i.command == *c))
            .unwrap_or(first),
    )
}

/// The drop-down list of a split. Returns true if the popup is open.
fn split_list(ui: &mut Ui, cx: &mut Cx, split: &Split, anchor: &egui::Response, popup: Id) {
    let Some(first) = split.items.first() else {
        return;
    };
    popup_below_widget(ui, popup, anchor, PopupCloseBehavior::CloseOnClick, |ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
        let width = split
            .items
            .iter()
            .map(|i| w::small_width(ui, true, Some(i.label)))
            .fold(120.0, f32::max);
        for item in &split.items {
            let look = cx.look(item.command);
            let resp =
                w::small_button(ui, cx.theme, look, Some(item.icon), Some(item.label), width);
            if resp.on_hover_text(cx.tip(item)).clicked() && look.enabled {
                cx.state.split_picks.insert(first.command, item.command);
                cx.press(item, look);
            }
        }
    });
}

fn large_split(ui: &mut Ui, cx: &mut Cx, split: &Split, id: ((usize, usize), usize)) {
    let Some(pick) = current_pick(cx, split) else {
        return;
    };
    let look = cx.look(pick.command);
    let label = split_label(split, Some(pick));
    let width = w::large_width(ui, label);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, w::LARGE_H), Sense::hover());
    let face = Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + 40.0));
    let list = Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + 40.0), rect.max);
    let base = Id::new(("ribbon-large-split", id));
    let face_resp = ui.interact(face, base.with("face"), look.sense());
    let list_resp = ui.interact(list, base.with("list-btn"), Sense::click());
    w::paint_state(ui, face, &face_resp, cx.theme, look);
    w::paint_state(
        ui,
        list,
        &list_resp,
        cx.theme,
        Look {
            enabled: true,
            active: false,
        },
    );
    w::paint_large(ui, rect, cx.theme, look, pick.icon, label, true);
    if face_resp.clone().on_hover_text(cx.tip(pick)).clicked() {
        cx.press(pick, look);
    }
    let popup = base.with("list");
    if list_resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    split_list(ui, cx, split, &list_resp, popup);
}

/// A drop-down filled from the command source: its face reads what is chosen now
/// (say the unit a selected result shows), and the list offers `first` and then
/// whatever the source lists. With nothing to list it is dimmed.
fn menu_cell(ui: &mut Ui, cx: &mut Cx, menu: &Item, first: &Item, width: f32) {
    let (items, face) = cx.source.menu(menu.command);
    let look = Look {
        enabled: face.is_some() || !items.is_empty(),
        active: false,
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, w::SMALL_H), Sense::hover());
    let base = Id::new(("ribbon-menu", menu.command));
    let resp = ui.interact(rect, base, look.sense());
    w::paint_state(ui, rect, &resp, cx.theme, look);
    let (body, arrow) = rect.split_left_right_at_x(rect.max.x - w::ARROW_W);
    let text = face.clone().unwrap_or_else(|| menu.label.to_owned());
    w::paint_small(ui, body, cx.theme, look, Some(menu.icon), Some(&text));
    w::paint_arrow(ui, arrow, cx.theme, look);
    let resp = resp.on_hover_text(menu.label);
    let popup = base.with("list");
    if resp.clicked() && look.enabled {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    popup_below_widget(ui, popup, &resp, PopupCloseBehavior::CloseOnClick, |ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
        let list: Vec<Item> = std::iter::once(Item::of(first.command))
            .chain(items.iter().map(|c| Item::of(*c)))
            .collect();
        let width = list
            .iter()
            .map(|i| w::small_width(ui, true, Some(i.label)))
            .fold(width, f32::max);
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                for item in &list {
                    // The unit showing now is marked.
                    let look = Look {
                        enabled: cx.look(item.command).enabled,
                        active: face.as_deref() == Some(item.label)
                            && item.command != first.command,
                    };
                    let resp = w::small_button(
                        ui,
                        cx.theme,
                        look,
                        Some(item.icon),
                        Some(item.label),
                        width,
                    );
                    if resp.clicked() && look.enabled {
                        cx.press(item, look);
                    }
                }
            });
    });
}
