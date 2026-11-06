//! The About window.

use eframe::egui::{self, Context};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.about_open {
        return;
    }
    let mut open = true;
    egui::Window::new("About OpenCalc")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!("OpenCalc {}", opencalc_core::version()));
            ui.label("An original engineering worksheet written in Rust.");
            ui.label("GPL-3.0-or-later. Milestone 1 (skeleton).");
        });
    if !open {
        out.push(CommandId::HelpAboutClose);
    }
}
