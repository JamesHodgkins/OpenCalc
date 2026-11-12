//! The Options window: the application settings (`crate::settings`).
//!
//! A list of sections on the left, the chosen section's settings on the right.
//! Each setting is a row: its name, the control, and a line saying what it does.
//! Changes apply at once and are kept between sessions; there is no OK or Cancel.
//!
//! Like every panel it only reads state. A changed setting comes back as a whole
//! new [`Settings`] value; the theme, grid display, snap and file commands go out
//! as the commands that already do those things elsewhere.

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Context, CornerRadius, FontId, Id, Layout, Rect, RichText,
    Sense, Stroke, StrokeKind, Ui,
};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::icons;
use crate::settings::Settings;
use crate::theme::{metrics, type_size, PaletteKey, Theme};

/// The themes on offer: the palette and its command.
pub const THEMES: [(PaletteKey, CommandId); 2] = [
    (PaletteKey::Neutral, CommandId::ThemeNeutral),
    (PaletteKey::Slate, CommandId::ThemeSlate),
];

mod sections;

const NAV_WIDTH: f32 = 160.0;
const PANE_WIDTH: f32 = 430.0;
const HEIGHT: f32 = 360.0;
/// Width of the column of setting names.
const NAME_WIDTH: f32 = 140.0;
const ROW: f32 = 30.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    General,
    Worksheet,
    NewWorksheets,
    Files,
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::General,
        Section::Worksheet,
        Section::NewWorksheets,
        Section::Files,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Worksheet => "Worksheet",
            Section::NewWorksheets => "New worksheets",
            Section::Files => "Files",
        }
    }

    fn icon(self) -> char {
        match self {
            Section::General => icons::SETTINGS,
            Section::Worksheet => icons::GRID,
            Section::NewWorksheets => icons::FILE_NEW,
            Section::Files => icons::SAVE,
        }
    }
}

/// Which section is showing: view state, kept in egui's memory.
fn section_id() -> Id {
    Id::new("options-section")
}

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands, change: &mut Option<Settings>) {
    if !state.options_open {
        return;
    }
    let mut open = true;
    let mut section: Section = ctx.data(|d| d.get_temp(section_id()).unwrap_or_default());
    egui::Window::new("Options")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(super::default_pos(ctx, Align2::CENTER_CENTER, [0.0, 0.0]))
        .show(ctx, |ui| {
            let t = &state.theme;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(NAV_WIDTH, HEIGHT),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_min_size(vec2(NAV_WIDTH, HEIGHT));
                        for s in Section::ALL {
                            if nav_row(ui, t, s, s == section) {
                                section = s;
                            }
                        }
                    },
                );
                // A one-pixel divider between the list and the page.
                let (line, _) = ui.allocate_exact_size(vec2(1.0, HEIGHT), Sense::hover());
                ui.painter().rect_filled(line, 0.0, t.border);
                ui.add_space(12.0);
                ui.allocate_ui_with_layout(
                    vec2(PANE_WIDTH, HEIGHT),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_min_size(vec2(PANE_WIDTH, HEIGHT));
                        ui.label(
                            RichText::new(section.label())
                                .size(type_size::HEADING)
                                .color(t.text),
                        );
                        ui.add_space(10.0);
                        let mut next = state.settings;
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| match section {
                                Section::General => sections::general(ui, state, &mut next, out),
                                Section::Worksheet => {
                                    sections::worksheet(ui, state, &mut next, out)
                                }
                                Section::NewWorksheets => {
                                    sections::new_worksheets(ui, t, &mut next)
                                }
                                Section::Files => sections::files(ui, state, &mut next, out),
                            });
                        if next != state.settings {
                            *change = Some(next);
                        }
                    },
                );
            });
            ui.add_space(4.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button(CommandId::AppOptionsDefaults.label())
                    .on_hover_text("Put every setting back as it was when OpenCalc was installed")
                    .clicked()
                {
                    out.push(CommandId::AppOptionsDefaults);
                }
                ui.label(
                    RichText::new("Changes apply at once and are kept for next time.")
                        .size(type_size::SMALL)
                        .color(t.text_weak),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        out.push(CommandId::AppOptionsClose);
                    }
                });
            });
        });
    ctx.data_mut(|d| d.insert_temp(section_id(), section));
    if !open {
        out.push(CommandId::AppOptionsClose);
    }
}

/// One section in the list. Returns whether it was chosen.
fn nav_row(ui: &mut Ui, t: &Theme, section: Section, current: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(NAV_WIDTH, metrics::CONTROL_HEIGHT + 4.0),
        Sense::click(),
    );
    let painter = ui.painter();
    let inner = rect.shrink2(vec2(0.0, 1.0));
    let r = CornerRadius::same(metrics::RADIUS);
    if current {
        painter.rect_filled(inner, r, t.raised);
        let bar =
            Rect::from_center_size(pos2(inner.left() + 1.5, inner.center().y), vec2(3.0, 16.0));
        painter.rect_filled(bar, CornerRadius::same(2), t.text_weak);
    } else if resp.hovered() {
        painter.rect_filled(inner, r, t.raised_high);
        painter.rect_stroke(inner, r, Stroke::new(1.0_f32, t.border), StrokeKind::Inside);
    }
    if resp.has_focus() {
        painter.rect_stroke(
            inner.expand(2.0),
            r,
            Stroke::new(2.0_f32, t.selection),
            StrokeKind::Outside,
        );
    }
    let colour = if current { t.text } else { t.text_weak };
    let cy = inner.center().y;
    painter.text(
        pos2(inner.left() + 20.0, cy),
        Align2::CENTER_CENTER,
        section.icon(),
        FontId::proportional(15.0),
        colour,
    );
    painter.text(
        pos2(inner.left() + 36.0, cy),
        Align2::LEFT_CENTER,
        section.label(),
        FontId::proportional(type_size::BODY),
        colour,
    );
    resp.clicked()
}

/// A setting: its name on the left, the control and a line about it on the right.
pub(super) fn row(ui: &mut Ui, t: &Theme, name: &str, help: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(NAME_WIDTH, ROW - 6.0), Sense::hover());
        ui.painter().text(
            pos2(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(type_size::BODY),
            t.text,
        );
        ui.vertical(|ui| {
            ui.set_width(PANE_WIDTH - NAME_WIDTH - 24.0);
            ui.horizontal(|ui| {
                ui.set_min_height(ROW - 6.0);
                control(ui);
            });
            if !help.is_empty() {
                ui.label(
                    RichText::new(help)
                        .size(type_size::SMALL)
                        .color(t.text_weak),
                );
            }
        });
    });
    ui.add_space(10.0);
}

/// Buttons side by side, one of them chosen.
pub(super) fn choice<T: Copy + PartialEq>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) {
    for (option, label) in options {
        if ui.selectable_label(*value == *option, *label).clicked() {
            *value = *option;
        }
    }
}

/// Show `section` the next time the window draws (tests use it to reach a page).
#[cfg(test)]
pub fn select(ctx: &Context, section: Section) {
    ctx.data_mut(|d| d.insert_temp(section_id(), section));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_has_exactly_one_theme_command() {
        for key in [PaletteKey::Neutral, PaletteKey::Slate] {
            assert_eq!(THEMES.iter().filter(|(p, _)| *p == key).count(), 1);
        }
    }
}
