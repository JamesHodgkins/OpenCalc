//! The Properties panel: a side panel at the right of the page that holds the
//! settings of the selected plot, table, picture or text. It shares the window
//! with the page instead of floating over it, so nothing it shows hides the
//! worksheet, and it can be made wider or narrower.

use eframe::egui::{self, Context, Frame, RichText};

use super::{picture_window, plot_window, table_window, text_window, Commands};
use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::theme::type_size;

/// What the panel is showing settings for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Plot,
    Table,
    Picture,
    Text,
}

impl Kind {
    pub fn title(self) -> &'static str {
        match self {
            Kind::Plot => "Plot",
            Kind::Table => "Table",
            Kind::Picture => "Picture",
            Kind::Text => "Text",
        }
    }
}

/// The settings the current selection calls for, if any.
pub fn wanted(state: &AppState) -> Option<Kind> {
    if plot_window::selected_plot(state).is_some() {
        Some(Kind::Plot)
    } else if table_window::selected_table(state).is_some() {
        Some(Kind::Table)
    } else if picture_window::selected_picture(state).is_some() {
        Some(Kind::Picture)
    } else if text_window::wanted(state) {
        Some(Kind::Text)
    } else {
        None
    }
}

pub fn show(ctx: &Context, state: &AppState, cmds: &mut Commands, out: &mut Vec<CanvasCmd>) {
    let Some(kind) = wanted(state) else {
        return;
    };
    let theme = state.theme;
    let frame = Frame::new().fill(theme.surface).inner_margin(8);
    egui::SidePanel::right("properties")
        .default_width(280.0)
        .width_range(240.0..=560.0)
        .resizable(true)
        .frame(frame)
        .show(ctx, |ui| {
            // A panel is saved at its frame's width, and a frame shrinks to its
            // content, so claim the full width or the panel collapses.
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(kind.title().to_uppercase())
                    .size(type_size::SMALL)
                    .color(theme.text_weak),
            );
            ui.add_space(4.0);
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| match kind {
                    Kind::Plot => plot_window::body(ui, state, out),
                    Kind::Table => table_window::body(ui, state, cmds, out),
                    Kind::Picture => picture_window::body(ui, state, cmds, out),
                    Kind::Text => text_window::body(ui, state, cmds, out),
                });
        });
}
