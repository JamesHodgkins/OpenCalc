//! OpenCalc desktop application.

// A release build on Windows is a window, not a console program.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;
mod canvas;
mod commands;
mod export;
mod fonts;
mod format;
mod icons;
mod instance;
mod layout;
mod logo;
mod number_format;
mod panels;
mod ribbon;
mod sheet;
#[cfg(test)]
mod smoke;
mod theme;

use eframe::egui;

/// Initial and minimum window width in logical pixels.
pub const INITIAL_WIDTH: f32 = 1280.0;
pub const MIN_WIDTH: f32 = 900.0;

fn main() -> eframe::Result {
    // `opencalc FILE...` opens those files. If OpenCalc is already running they go
    // to its window, as tabs, and this copy has nothing more to do.
    let files: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
    let window: std::sync::Arc<std::sync::OnceLock<egui::Context>> = Default::default();
    let wake = window.clone();
    let later_files = match instance::start(instance::default_dir().as_deref(), &files, move || {
        if let Some(ctx) = wake.get() {
            ctx.request_repaint();
        }
    }) {
        instance::Start::Forwarded => return Ok(()),
        instance::Start::First(receiver) => receiver,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenCalc")
            .with_icon(logo::icon_data())
            .with_inner_size([INITIAL_WIDTH, 780.0])
            .with_min_inner_size([MIN_WIDTH, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "OpenCalc",
        options,
        Box::new(|cc| {
            let palette = cc
                .storage
                .and_then(|s| s.get_string("palette"))
                .map_or(theme::PaletteKey::Neutral, |k| {
                    theme::PaletteKey::from_key(&k)
                });
            let _ = window.set(cc.egui_ctx.clone());
            let mut app = app::App::new(cc.egui_ctx.clone(), palette);
            app.listen_for_files(later_files);
            app.state.recent = cc
                .storage
                .and_then(|s| s.get_string("recent"))
                .map(|list| list.lines().map(std::path::PathBuf::from).collect())
                .unwrap_or_default();
            app.enable_autosave(app::recovery_dir());
            app.scan_recovery();
            for file in &files {
                app.open_path(file);
            }
            Ok(Box::new(app))
        }),
    )
}
