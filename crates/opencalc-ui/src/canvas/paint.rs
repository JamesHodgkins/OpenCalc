//! Painting the grid and each region type. Everything here draws in screen
//! space from a region's top-left; the caller does the document-to-screen
//! transform and decides what is visible.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2,
};
use opencalc_core::expr::Expr;

use super::adapt::{kind_icon, vec};
use super::errors;
use super::result::{self, Beside};
use super::view::{CanvasView, GRID};
use super::Env;
use crate::format::format_value;
use crate::layout::{Layout, Op, Typesetter};
use crate::theme::{metrics, type_size, Theme};
use opencalc_core::document::{MathRegion, Region, RegionKind};

/// Typeset size of a math region at 100% zoom.
pub(super) const MATH_SIZE: f32 = 16.0;
const TEXT_SIZE: f32 = 13.0;
/// Padding around region content, in document units.
pub(super) const PAD: f32 = 6.0;
/// Gap between an expression and its result, in document units.
const RESULT_GAP: f32 = 6.0;

/// Minor lines at the snap grid (an eighth of an inch), a stronger line every
/// eighth (an inch). When zoomed out
/// far enough that lines would crowd, the spacing steps up by five.
pub fn grid(painter: &Painter, rect: Rect, view: &CanvasView, origin: Pos2, theme: &Theme) {
    let mut step = GRID * view.zoom;
    let mut major_every = 8;
    while step < 8.0 {
        step *= 5.0;
        major_every *= 5;
    }
    let base = origin + view.offset;
    let minor = Stroke::new(1.0_f32, theme.raised);
    // A little fainter than the border colour: halfway to the minor lines.
    let major = Stroke::new(1.0_f32, theme.raised.lerp_to_gamma(theme.border, 0.5));

    let first = ((rect.min.x - base.x) / step).floor() as i64;
    let last = ((rect.max.x - base.x) / step).ceil() as i64;
    for k in first..=last {
        let stroke = if k.rem_euclid(major_every) == 0 {
            major
        } else {
            minor
        };
        painter.vline(base.x + k as f32 * step, rect.y_range(), stroke);
    }
    let first = ((rect.min.y - base.y) / step).floor() as i64;
    let last = ((rect.max.y - base.y) / step).ceil() as i64;
    for k in first..=last {
        let stroke = if k.rem_euclid(major_every) == 0 {
            major
        } else {
            minor
        };
        painter.hline(rect.x_range(), base.y + k as f32 * step, stroke);
    }
}

/// Where and how to draw one region.
#[derive(Clone, Copy)]
struct Place {
    top_left: Pos2,
    zoom: f32,
    /// Measure only: used while the region is being edited in place.
    skip_content: bool,
}

fn plain(fonts: &eframe::egui::epaint::text::Fonts, s: &str, colour: Color32, size: f32) -> Layout {
    Typesetter::new(fonts, colour).layout(&Expr::Name(s.to_owned()), size)
}

/// Narrowest a wrapped message is made, in document units.
const MIN_MESSAGE: f32 = 160.0;

/// `s` on as many lines as it takes to fit `wrap` pixels. Long error messages
/// would otherwise run off the page.
fn wrapped(
    fonts: &eframe::egui::epaint::text::Fonts,
    s: &str,
    colour: Color32,
    size: f32,
    wrap: f32,
) -> Layout {
    let galley = fonts.layout(s.to_owned(), FontId::proportional(size), colour, wrap);
    let r = galley.rect;
    let first = galley.rows.first().map_or(r.height(), |row| row.height());
    Layout {
        w: r.width(),
        h: r.height(),
        baseline: first * 0.78,
        ops: vec![Op::Text {
            at: Vec2::ZERO,
            galley,
        }],
        marks: Vec::new(),
    }
}

/// Paint one region with its top-left at `top_left` (screen). Returns the
/// region's content size in document units, which the caller stores for
/// auto-sized kinds. With `skip_content`, nothing is drawn but the size is still
/// measured (used while the region is being edited in place).
pub fn region(
    ui: &Ui,
    painter: &Painter,
    env: &Env,
    r: &Region,
    top_left: Pos2,
    zoom: f32,
    skip_content: bool,
) -> Vec2 {
    let place = Place {
        top_left,
        zoom,
        skip_content,
    };
    match &r.kind {
        RegionKind::Math(m) => math(ui, painter, env, r, m, place),
        RegionKind::Text { text } => self::text(painter, env.paper, r, text, place),
        RegionKind::Area { title } => {
            if !skip_content {
                area(painter, env.paper, r, title, top_left, zoom);
            }
            vec(r.size)
        }
        RegionKind::Plot(spec) => {
            if !skip_content {
                super::plot::paint(painter, env, r, spec, top_left, zoom);
            }
            vec(r.size)
        }
        RegionKind::Table | RegionKind::Picture => {
            if !skip_content {
                placeholder(painter, env.paper, r, top_left, zoom);
            }
            vec(r.size)
        }
    }
}

fn math(ui: &Ui, painter: &Painter, env: &Env, r: &Region, m: &MathRegion, place: Place) -> Vec2 {
    let Place {
        top_left,
        zoom,
        skip_content,
    } = place;
    let (source, expr) = (m.source(), m.expr());
    let theme = env.paper;
    let size = MATH_SIZE * zoom;
    let blank = source.trim().is_empty();
    let result = env.output.values.get(&r.id);
    let path = env.output.error_paths.get(&r.id).map(Vec::as_slice);
    // How far a message beside the expression may reach: to the page's right margin.
    let right = (env.page.size().0 - env.page.margin() - r.pos.x) * zoom;
    let room =
        |used: f32| (right - 2.0 * PAD * zoom - RESULT_GAP * zoom - used).max(MIN_MESSAGE * zoom);
    let (lay, res, bands) = ui.ctx().fonts(|f| {
        let t = Typesetter::new(f, theme.text);
        let warn = |message: String, used: f32| {
            wrapped(
                f,
                &format!("{} {message}", crate::icons::WARNING),
                theme.error,
                size,
                room(used),
            )
        };
        if blank {
            return (
                plain(f, "math", theme.text_disabled, size),
                None,
                Vec::new(),
            );
        }
        // An error is underlined in the typeset form; the message goes beside it.
        let marked = errors::mark(f, theme.text, source, expr, path, size);
        let (typed, bands) = match marked {
            Some(m) if matches!(expr, Expr::Error(_)) || matches!(result, Some(Err(_))) => {
                (m.layout, m.bands)
            }
            _ => (t.layout(result::typeset_part(expr), size), Vec::new()),
        };
        if let Expr::Error(e) = expr {
            let typed = if bands.is_empty() {
                plain(f, source, theme.text, size)
            } else {
                typed
            };
            let used = typed.w;
            return (typed, Some(warn(e.to_string(), used)), bands);
        }
        // A definition or bare expression shows only what was typed; an
        // evaluation region (`x =`) also shows its answer. Errors always show.
        let after = match result::beside(expr, result) {
            Beside::Nothing => None,
            Beside::Pending => Some(plain(f, "= \u{2026}", theme.text_weak, size)),
            Beside::Value(v) => Some(plain(
                f,
                &format!("= {}", format_value(v, env.units)),
                theme.text,
                size,
            )),
            Beside::Failed(e) => Some(warn(e.to_string(), typed.w)),
        };
        // A name another region also defines: the last one wins, so say so here.
        let after = after.or_else(|| {
            let name = result::redefined(&env.output.warnings, r.id)?;
            let text = format!("{} '{name}' is defined again", crate::icons::WARNING);
            Some(plain(f, &text, theme.warning, size))
        });
        (typed, after, bands)
    });
    let pad = PAD * zoom;
    let gap = if res.is_some() {
        RESULT_GAP * zoom
    } else {
        0.0
    };
    let res_w = res.as_ref().map_or(0.0, |l| l.w);
    let res_h = res.as_ref().map_or(0.0, |l| l.h);
    let content_w = lay.w + gap + res_w;
    let content_h = lay.h.max(res_h);
    if !skip_content {
        let origin = top_left + Vec2::splat(pad);
        lay.paint(painter, origin, theme.text);
        errors::paint(painter, &bands, origin, theme.error, zoom);
        if let Some(res) = res {
            let at = origin + Vec2::new(lay.w + gap, lay.baseline - res.baseline);
            res.paint(painter, at, theme.text);
        }
    }
    Vec2::new(content_w + 2.0 * pad, content_h + 2.0 * pad) / zoom
}

fn text(painter: &Painter, theme: &Theme, r: &Region, text: &str, place: Place) -> Vec2 {
    let Place {
        top_left,
        zoom,
        skip_content,
    } = place;
    let pad = PAD * zoom;
    let (shown, colour) = if text.is_empty() {
        ("Text", theme.text_disabled)
    } else {
        (text, theme.text)
    };
    let galley = painter.layout(
        shown.to_owned(),
        FontId::proportional(TEXT_SIZE * zoom),
        colour,
        (r.size.w * zoom - 2.0 * pad).max(10.0),
    );
    if !skip_content {
        painter.galley(top_left + Vec2::splat(pad), galley.clone(), colour);
    }
    Vec2::new(r.size.w, galley.size().y / zoom + 2.0 * PAD)
}

fn area(painter: &Painter, theme: &Theme, r: &Region, title: &str, top_left: Pos2, zoom: f32) {
    let rect = Rect::from_min_size(top_left, vec(r.size) * zoom);
    let stroke = Stroke::new(1.0_f32, theme.border_strong);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    painter.extend(Shape::dashed_line(&corners, stroke, 6.0, 4.0));
    let label = if title.is_empty() { "Area" } else { title };
    painter.text(
        rect.left_top() + Vec2::new(8.0, 6.0) * zoom,
        Align2::LEFT_TOP,
        label,
        FontId::proportional(type_size::SMALL * zoom.max(0.6)),
        theme.text_weak,
    );
}

fn placeholder(painter: &Painter, theme: &Theme, r: &Region, top_left: Pos2, zoom: f32) {
    let rect = Rect::from_min_size(top_left, vec(r.size) * zoom);
    let radius = CornerRadius::same(metrics::RADIUS);
    painter.rect_filled(rect, radius, theme.surface);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, theme.border),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center() - Vec2::new(0.0, 8.0 * zoom),
        Align2::CENTER_CENTER,
        kind_icon(&r.kind),
        FontId::proportional(32.0 * zoom),
        theme.text_weak,
    );
    painter.text(
        rect.center() + Vec2::new(0.0, 28.0 * zoom),
        Align2::CENTER_CENTER,
        r.kind.label(),
        FontId::proportional(type_size::BODY * zoom.max(0.6)),
        theme.text_weak,
    );
}
