//! Painting a table region: its name, a heading row with units, and the cells.
//!
//! The size comes from the text, so the region's stored size follows the
//! content (as a math region's does). Everything is laid out in screen pixels at
//! the current zoom and the size is handed back in document units.

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Ui, Vec2};
use opencalc_core::document::{Region, TableSpec};

use super::paint::Place;
use super::Env;
use crate::theme::type_size;

/// Gap each side of a cell's text, in document units.
const CELL_PAD: f32 = 7.0;
/// Extra height of a row over its text, in document units.
const ROW_PAD: f32 = 6.0;

/// Paints `spec` with its top-left at `top_left`; returns the size in document
/// units. With `skip_content` it only measures.
pub(super) fn paint(
    ui: &Ui,
    painter: &Painter,
    env: &Env,
    r: &Region,
    spec: &TableSpec,
    place: Place,
) -> Vec2 {
    let Place {
        top_left,
        zoom,
        skip_content,
    } = place;
    let size = type_size::BODY * zoom;
    let font = FontId::proportional(size);
    let small = FontId::proportional(size * 0.82);
    let pad = CELL_PAD * zoom;
    let row_h = size + ROW_PAD * zoom;

    let width_of = |text: &str, font: &FontId| -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        ui.ctx().fonts(|f| {
            f.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
                .rect
                .width()
        })
    };

    let has_head = spec
        .columns
        .iter()
        .any(|c| !c.header.is_empty() || !c.unit.is_empty());
    let widths: Vec<f32> = (0..spec.cols())
        .map(|j| {
            let cells = spec
                .rows
                .iter()
                .map(|row| width_of(&row[j], &font))
                .fold(0.0_f32, f32::max);
            let h = if has_head {
                width_of(&header_text(spec, j), &small)
            } else {
                0.0
            };
            (cells.max(h) + 2.0 * pad).max(36.0 * zoom)
        })
        .collect();
    let grid_w: f32 = widths.iter().sum();

    let title = if spec.name.trim().is_empty() {
        "name this table".to_owned()
    } else {
        format!("{} :=", spec.name.trim())
    };
    let message = match env.output.values.get(&r.id) {
        Some(Err(e)) => Some(e.to_string()),
        _ => None,
    };
    let title_h = row_h;
    let head_h = if has_head { row_h * 0.9 } else { 0.0 };
    let message_h = if message.is_some() { row_h } else { 0.0 };
    let total_w = grid_w.max(width_of(&title, &font) + 2.0 * pad).max(
        message
            .as_deref()
            .map_or(0.0, |m| width_of(m, &small) + 2.0 * pad),
    );
    let total_h = title_h + head_h + row_h * spec.row_count() as f32 + message_h;

    if !skip_content {
        let measured = Measured {
            widths,
            grid_w,
            title,
            message,
            title_h,
            head_h,
            message_h,
            row_h,
            pad,
            zoom,
            has_head,
            font,
            small,
        };
        draw(painter, env, spec, top_left, measured);
    }
    Vec2::new(total_w, total_h) / zoom
}

/// The heading of column `j`: its name and unit as `name [unit]`.
fn header_text(spec: &TableSpec, j: usize) -> String {
    let c = &spec.columns[j];
    match (c.header.is_empty(), c.unit.is_empty()) {
        (_, true) => c.header.clone(),
        (true, false) => format!("[{}]", c.unit),
        (false, false) => format!("{} [{}]", c.header, c.unit),
    }
}

/// What measuring a table found, which drawing it needs.
struct Measured {
    widths: Vec<f32>,
    grid_w: f32,
    title: String,
    message: Option<String>,
    title_h: f32,
    head_h: f32,
    message_h: f32,
    row_h: f32,
    pad: f32,
    zoom: f32,
    has_head: bool,
    font: FontId,
    small: FontId,
}

/// Draw the name, headings, cells, brackets and any error message.
fn draw(painter: &Painter, env: &Env, spec: &TableSpec, top_left: Pos2, m: Measured) {
    let Measured {
        widths,
        grid_w,
        title,
        message,
        title_h,
        head_h,
        message_h,
        row_h,
        pad,
        zoom,
        has_head,
        font,
        small,
    } = m;
    let theme = env.paper;
    let blank = spec.name.trim().is_empty();
    painter.text(
        top_left + Vec2::new(pad, title_h * 0.5),
        Align2::LEFT_CENTER,
        &title,
        font.clone(),
        if blank {
            theme.text_disabled
        } else {
            theme.text
        },
    );
    let top = top_left.y + title_h;
    let rule = Stroke::new(1.0_f32, theme.border);
    let mut x = top_left.x;
    for (j, w) in widths.iter().enumerate() {
        if has_head {
            painter.text(
                Pos2::new(x + w - pad, top + head_h * 0.5),
                Align2::RIGHT_CENTER,
                header_text(spec, j),
                small.clone(),
                theme.text_weak,
            );
        }
        for (i, row) in spec.rows.iter().enumerate() {
            let y = top + head_h + row_h * (i as f32 + 0.5);
            painter.text(
                Pos2::new(x + w - pad, y),
                Align2::RIGHT_CENTER,
                &row[j],
                font.clone(),
                theme.text,
            );
        }
        x += w;
    }
    // Brackets, as a matrix is written, either side of the cells.
    let body = Rect::from_min_max(
        Pos2::new(top_left.x, top),
        Pos2::new(
            top_left.x + grid_w,
            top + head_h + row_h * spec.row_count() as f32,
        ),
    );
    if has_head {
        painter.hline(body.x_range(), top + head_h, rule);
    }
    let tick = 5.0 * zoom;
    for (x, dir) in [(body.left(), 1.0), (body.right(), -1.0)] {
        painter.vline(x, body.y_range(), rule);
        painter.hline(
            x.min(x + dir * tick)..=x.max(x + dir * tick),
            body.top(),
            rule,
        );
        painter.hline(
            x.min(x + dir * tick)..=x.max(x + dir * tick),
            body.bottom(),
            rule,
        );
    }
    if let Some(m) = &message {
        painter.text(
            Pos2::new(top_left.x + pad, body.bottom() + message_h * 0.5),
            Align2::LEFT_CENTER,
            format!("{} {m}", crate::icons::WARNING),
            small,
            theme.error,
        );
    }
}
