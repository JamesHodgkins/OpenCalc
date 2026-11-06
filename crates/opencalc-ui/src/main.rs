//! OpenCalc desktop application.

mod app;
mod canvas;
mod commands;
mod fonts;
mod format;
mod icons;
mod layout;
mod ribbon;
mod sheet;
mod shell;
#[cfg(test)]
mod smoke;
mod theme;

use eframe::egui;

/// Initial and minimum window width in logical pixels.
pub const INITIAL_WIDTH: f32 = 1280.0;
pub const MIN_WIDTH: f32 = 900.0;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenCalc")
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
            Ok(Box::new(app::App::new(cc.egui_ctx.clone(), palette)))
        }),
    )
}
