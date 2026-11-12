//! What is painted over the regions: the selection's frame and grips, and the
//! rubber band of the drawing tool.

use eframe::egui::{Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use opencalc_core::document::{Region, ShapeKind};

use super::adapt::rect_of;
use super::{grip, pointer, resize, shape, CanvasUi, CanvasView, Drag, Env};
use crate::theme::Theme;

/// The frame round each selected region, and the handles and grips of a lone
/// resizable one.
pub(super) fn selection(
    painter: &Painter,
    theme: &Theme,
    regions: &[Region],
    view: &CanvasView,
    origin: Pos2,
    st: &CanvasUi,
) {
    let outline = Stroke::new(1.5_f32, theme.selection);
    // A lone resizable object gets a thin grey frame on its edge with round
    // grips, as in Office; other selections keep the selection outline.
    let grey = Stroke::new(1.0_f32, theme.text_weak);
    for id in &st.selection {
        if let Some(r) = regions.iter().find(|r| r.id == *id) {
            let inner = view.rect_to_screen(rect_of(r, &st.sizes), origin);
            if st.selection.len() == 1 && pointer::resizable(r) {
                // A line has no box to pull: its two ends are its grips.
                if pointer::has_handles(r) {
                    painter.rect_stroke(inner, 0.0, grey, StrokeKind::Middle);
                    for h in resize::Handle::ALL {
                        let at = h.point(inner);
                        let radius = pointer::HANDLE_SIZE / 2.0;
                        painter.circle_filled(at, radius, eframe::egui::Color32::WHITE);
                        painter.circle_stroke(
                            at,
                            radius,
                            Stroke::new(1.0_f32, eframe::egui::Color32::BLACK),
                        );
                    }
                }
                for (g, at) in grip::grips(r, inner, view.zoom) {
                    let radius = pointer::HANDLE_SIZE / 2.0;
                    if g == grip::Grip::Corner {
                        painter.circle_filled(at, radius, theme.selection);
                        painter.circle_stroke(
                            at,
                            radius,
                            Stroke::new(1.0_f32, eframe::egui::Color32::WHITE),
                        );
                    } else {
                        painter.circle_filled(at, radius, eframe::egui::Color32::WHITE);
                        painter.circle_stroke(
                            at,
                            radius,
                            Stroke::new(1.0_f32, eframe::egui::Color32::BLACK),
                        );
                    }
                }
            } else {
                painter.rect_stroke(inner.expand(2.0), 2.0, outline, StrokeKind::Outside);
            }
        }
    }
}

/// The band (and, for a line, the line itself) shown while the drawing tool is
/// being dragged out.
pub(super) fn pen_preview(
    ui: &Ui,
    painter: &Painter,
    env: &Env,
    theme: &Theme,
    view: &CanvasView,
    origin: Pos2,
    st: &CanvasUi,
) {
    if let (Some(kind), Some(Drag::Draw { start, current })) = (st.pen, &st.drag) {
        let mods = ui.input(|i| i.modifiers);
        let grid = (env.snap && !mods.alt).then_some(st.grid.step());
        if kind == ShapeKind::Line {
            let (from, to) = shape::drawn_line(
                *start,
                *current,
                grid,
                mods.shift,
                mods.ctrl || mods.command,
            );
            let a = view.doc_to_screen(Pos2::new(from.x, from.y), origin);
            let b = view.doc_to_screen(Pos2::new(to.x, to.y), origin);
            painter.line_segment([a, b], Stroke::new(1.5_f32, theme.selection));
        }
        let (pos, size) = shape::drawn_box(
            kind,
            *start,
            *current,
            grid,
            mods.shift,
            mods.ctrl || mods.command,
        );
        let band = view.rect_to_screen(
            Rect::from_min_size(Pos2::new(pos.x, pos.y), Vec2::new(size.w, size.h)),
            origin,
        );
        shape::preview(painter, kind, band, theme.selection);
    }
}
