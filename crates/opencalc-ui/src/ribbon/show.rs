//! Laying out and drawing the ribbon.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Sense, Ui, Vec2};

use super::cells::{large_split, rows_ui, small_item, CONTROL_LABEL_W, CONTROL_MIN_W, CONTROL_W};
use super::widgets::{self as w, Look};
use super::{
    Column, CommandSource, Context, ControlHost, Item, Panel, Ribbon, RibbonOutput, RibbonState,
};
use crate::commands::CommandId;
use crate::theme::{metrics, type_size, Theme};

/// The pale red gradient of a contextual tab, `(top, bottom)`: open, hovered, and
/// resting. Light at the top, a shade deeper toward the ribbon.
const CONTEXT_TAB: (Color32, Color32) = (
    Color32::from_rgb(255, 224, 220),
    Color32::from_rgb(238, 160, 152),
);
const CONTEXT_TAB_HOVER: (Color32, Color32) = (
    Color32::from_rgb(252, 230, 227),
    Color32::from_rgb(240, 186, 180),
);
const CONTEXT_TAB_IDLE: (Color32, Color32) = (
    Color32::from_rgb(246, 220, 217),
    Color32::from_rgb(230, 176, 170),
);

/// Points round the top-rounded tab shape, from the bottom left, clockwise:
/// a convex polygon, so a fan of triangles fills it.
fn tab_outline(rect: Rect, radius: f32) -> Vec<egui::Pos2> {
    const STEPS: usize = 6;
    let r = radius.min(rect.width() / 2.0).min(rect.height());
    let mut pts = vec![rect.left_bottom()];
    for k in 0..=STEPS {
        let a = std::f32::consts::FRAC_PI_2 * k as f32 / STEPS as f32;
        pts.push(egui::pos2(
            rect.left() + r - r * a.cos(),
            rect.top() + r - r * a.sin(),
        ));
    }
    for k in 0..=STEPS {
        let a = std::f32::consts::FRAC_PI_2 * k as f32 / STEPS as f32;
        pts.push(egui::pos2(
            rect.right() - r + r * a.sin(),
            rect.top() + r - r * a.cos(),
        ));
    }
    pts.push(rect.right_bottom());
    pts
}

/// Fill a tab with a vertical gradient.
fn gradient_tab(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    (top, bottom): (Color32, Color32),
) {
    let mut mesh = egui::Mesh::default();
    for p in tab_outline(rect, radius) {
        let t = ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        mesh.colored_vertex(p, top.lerp_to_gamma(bottom, t));
    }
    let n = mesh.vertices.len() as u32;
    for k in 1..n - 1 {
        mesh.add_triangle(0, k, k + 1);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Everything a draw call needs, so helpers take one argument.
pub(super) struct Cx<'a> {
    pub(super) theme: &'a Theme,
    pub(super) source: &'a dyn CommandSource,
    pub(super) state: &'a mut RibbonState,
    pub(super) out: &'a mut RibbonOutput,
    pub(super) host: &'a mut dyn ControlHost,
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
    pub fn tab_strip(
        &self,
        ui: &mut Ui,
        theme: &Theme,
        state: &mut RibbonState,
        contexts: &[Context],
    ) {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        if let Some(wanted) = state.focus.take().filter(|c| contexts.contains(c)) {
            if let Some(i) = self.tabs.iter().position(|t| t.context == Some(wanted)) {
                state.active_tab = i;
                state.open_panel = None;
            }
        }
        let current = self.current(state.active_tab, contexts);
        // A tab whose context went away stays gone: reselecting the thing later
        // must not jump back to it.
        state.active_tab = current;
        for i in self.visible(contexts) {
            let tab = &self.tabs[i];
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
            if tab.context.is_some() {
                // A contextual tab is a pale red gradient with black text, a shade
                // deeper while it is the one open.
                let fill = if active {
                    CONTEXT_TAB
                } else if resp.hovered() {
                    CONTEXT_TAB_HOVER
                } else {
                    CONTEXT_TAB_IDLE
                };
                gradient_tab(ui.painter(), rect, 4.0, fill);
            } else if active {
                ui.painter().rect_filled(rect, top, theme.raised);
            }
            let colour = if tab.context.is_some() {
                Color32::BLACK
            } else if active || resp.hovered() {
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
        contexts: &[Context],
        host: &mut dyn ControlHost,
    ) -> RibbonOutput {
        let mut out = RibbonOutput::default();
        let tab_index = self.current(state.active_tab, contexts);
        let Some(tab) = self.tabs.get(tab_index) else {
            return out;
        };
        let mut cx = Cx {
            theme,
            source,
            state,
            out: &mut out,
            host,
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
        Column::Controls(list) => {
            for control in list {
                let layout = egui::Layout::left_to_right(egui::Align::Center);
                ui.allocate_ui_with_layout(Vec2::new(CONTROL_W, w::SMALL_H), layout, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    // A column is never narrower than this, so a narrow widget (a
                    // colour button) still leaves a gap before the next column.
                    ui.set_min_width(CONTROL_MIN_W);
                    // Left-aligned, so the labels of a column line up.
                    ui.allocate_ui_with_layout(
                        Vec2::new(CONTROL_LABEL_W, w::SMALL_H),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            // A cell shrinks to its content unless told otherwise;
                            // without this a longer label pushes its widget right.
                            ui.set_min_size(Vec2::new(CONTROL_LABEL_W, w::SMALL_H));
                            ui.add(egui::Label::new(
                                egui::RichText::new(control.label())
                                    .size(type_size::SMALL)
                                    .color(cx.theme.text_weak),
                            ));
                        },
                    );
                    cx.host.control(ui, cx.theme, *control);
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_shape_stays_inside_its_box_with_square_feet_and_round_shoulders() {
        let rect = Rect::from_min_size(egui::pos2(10.0, 5.0), Vec2::new(90.0, 25.0));
        let pts = tab_outline(rect, 4.0);
        assert!(pts.iter().all(|p| rect.expand(0.01).contains(*p)));
        assert_eq!(pts[0], rect.left_bottom());
        assert_eq!(*pts.last().unwrap(), rect.right_bottom());
        assert!(
            !pts.contains(&rect.left_top()) && !pts.contains(&rect.right_top()),
            "the top corners are rounded off"
        );
    }

    #[test]
    fn the_gradient_is_light_above_and_deeper_below() {
        for (top, bottom) in [CONTEXT_TAB, CONTEXT_TAB_HOVER, CONTEXT_TAB_IDLE] {
            assert!(top.g() > bottom.g() && top.b() > bottom.b());
            assert!(top.r() >= 240, "still a pale red");
        }
    }
}
