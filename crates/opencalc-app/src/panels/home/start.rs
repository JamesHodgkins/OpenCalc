//! The Start section of the Home page: big tiles for the ways to begin.

use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::layout::columns;
use crate::commands::CommandId;
use crate::panels::Commands;
use crate::theme::{metrics, type_size, Theme};

const TILE_MIN_W: f32 = 170.0;
const TILE_GAP: f32 = 12.0;
const TILE_H: f32 = 62.0;

/// The tiles, left to right: the command, and a line saying what it does. The first
/// is the main one.
const TILES: [(CommandId, &str); 4] = [
    (CommandId::FileNew, "A blank worksheet"),
    (CommandId::FileOpen, "An OCALC worksheet"),
    (CommandId::FileNewFromTemplate, "Start from a template"),
    (CommandId::FileExample, "A worked example"),
];

pub fn section(ui: &mut Ui, theme: &Theme, width: f32, out: &mut Commands) {
    super::section_label(ui, theme, "Start");
    ui.add_space(6.0);
    let per_row = columns(width, TILE_MIN_W, TILE_GAP, TILES.len());
    let tile_w = (width - TILE_GAP * (per_row as f32 - 1.0)) / per_row as f32;
    for row in TILES.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TILE_GAP;
            for (command, subtitle) in row {
                let primary = *command == CommandId::FileNew;
                if tile(ui, theme, tile_w, *command, subtitle, primary) {
                    out.push(*command);
                }
            }
        });
        ui.add_space(TILE_GAP - ui.spacing().item_spacing.y);
    }
}

/// One tile; returns whether it was clicked. The main one is filled with the brand
/// colour; the others are a raised grey.
fn tile(
    ui: &mut Ui,
    theme: &Theme,
    width: f32,
    command: CommandId,
    subtitle: &str,
    primary: bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, TILE_H), Sense::click());
    let painter = ui.painter_at(rect);
    let radius = CornerRadius::same(metrics::WINDOW_RADIUS);
    let fill = match (primary, resp.hovered()) {
        (true, false) => theme.accent,
        (true, true) => theme.accent_hover,
        (false, false) => theme.raised,
        (false, true) => theme.raised_high,
    };
    painter.rect_filled(rect, radius, fill);
    if !primary {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, theme.border),
            StrokeKind::Inside,
        );
    }
    let (strong, weak) = if primary {
        (
            theme.text_on_accent,
            theme.text_on_accent.gamma_multiply(0.85),
        )
    } else {
        (theme.text, theme.text_weak)
    };
    let icon_box = Rect::from_min_size(rect.min + Vec2::new(14.0, 0.0), Vec2::new(28.0, TILE_H));
    painter.text(
        icon_box.center(),
        Align2::CENTER_CENTER,
        command.icon(),
        FontId::proportional(20.0),
        strong,
    );
    let text_x = rect.min.x + 56.0;
    let label = command.label().trim_end_matches("...");
    painter.text(
        egui::pos2(text_x, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(type_size::TAB),
        strong,
    );
    painter.text(
        egui::pos2(text_x, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        subtitle,
        FontId::proportional(type_size::SMALL),
        weak,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::commands::Registry;
    use crate::theme::PaletteKey;

    /// With no worksheet open every tile must work: a tile that does nothing on the
    /// page it is on would be the worst kind of button.
    #[test]
    fn every_tile_works_with_no_worksheet_open() {
        let mut state = AppState::new(PaletteKey::Neutral);
        state.sheets.clear();
        state.active = None;
        let registry = Registry::new(&state.command_context(), &state.active_toggles());
        for (command, _) in TILES {
            assert!(registry.allows(command), "{command:?}");
            assert!(command.is_implemented(), "{command:?}");
        }
        assert_eq!(
            TILES[0].0,
            CommandId::FileNew,
            "the main tile starts a worksheet"
        );
    }
}
