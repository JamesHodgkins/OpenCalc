//! Painting each region type. Everything here draws in screen
//! space from a region's top-left; the caller does the document-to-screen
//! transform and decides what is visible.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2,
};
use opencalc_core::expr::Expr;

use super::adapt::{kind_icon, vec};
use super::errors;
use super::result::{self, Beside};
use super::Env;
use crate::format::format_value_with;
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

/// Where and how to draw one region.
#[derive(Clone, Copy)]
pub(super) struct Place {
    pub(super) top_left: Pos2,
    pub(super) zoom: f32,
    /// Measure only: used while the region is being edited in place.
    pub(super) skip_content: bool,
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
        first_line: 0.0,
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
    let size = content(ui, painter, env, r, top_left, zoom, skip_content);
    if r.border && !skip_content {
        let rect = Rect::from_min_size(top_left, size * zoom).expand(BORDER_GAP * zoom);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new((1.0 * zoom).max(0.75), env.paper.text),
            StrokeKind::Middle,
        );
    }
    size
}

/// Space between a region's content and its border, in document units.
const BORDER_GAP: f32 = 3.0;

fn content(
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
        RegionKind::Table(spec) => super::table::paint(ui, painter, env, r, spec, place),
        RegionKind::Picture(spec) => super::picture::paint(ui, painter, env, r, spec, place),
        RegionKind::Shape(spec) => super::shape::paint(painter, env, r, spec, place),
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
    // A unit being typed into the result's slot (read outside `fonts`, which
    // holds the context).
    let typing = super::unit_slot::typing(ui.ctx(), r.id);
    let (lay, res, bands, slot) = ui.ctx().fonts(|f| {
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
                None,
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
            return (typed, Some(warn(e.to_string(), used)), bands, None);
        }
        // A definition or bare expression shows only what was typed; an
        // evaluation region (`x =`) also shows its answer. Errors always show.
        let mut slot = None;
        let after = match result::beside(expr, result) {
            Beside::Nothing => None,
            Beside::Pending => Some(plain(f, "= \u{2026}", theme.text_weak, size)),
            // `b = 5 m` with no `b` defined only echoes the right side: say so.
            Beside::Value(_)
                if result::undefined_evaluation(&env.output.warnings, r.id).is_some() =>
            {
                let name = result::undefined_evaluation(&env.output.warnings, r.id);
                let text = format!(
                    "{} '{}' is not defined here: use := to define it",
                    crate::icons::WARNING,
                    name.unwrap_or_default()
                );
                Some(plain(f, &text, theme.warning, size))
            }
            Beside::Value(v) => {
                let (answer, unit_slot) = answer_beside(f, env, m, v, typing.as_deref(), size);
                slot = unit_slot;
                Some(answer)
            }
            Beside::Failed(e) => Some(warn(e.to_string(), typed.w)),
        };
        // A name another region also defines: the last one wins, so say so here.
        let after = after.or_else(|| {
            let name = result::redefined(&env.output.warnings, r.id)?;
            let text = format!("{} '{name}' is defined again", crate::icons::WARNING);
            Some(plain(f, &text, theme.warning, size))
        });
        (typed, after, bands, slot)
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
    if skip_content {
        super::unit_slot::forget(ui.ctx(), r.id);
    } else {
        let origin = top_left + Vec2::splat(pad);
        lay.paint(painter, origin, theme.text);
        errors::paint(painter, &bands, origin, theme.error, zoom);
        match res {
            Some(res) => {
                let at = origin + Vec2::new(lay.w + gap, lay.baseline - res.baseline);
                res.paint(painter, at, theme.text);
                match slot {
                    Some((x, w, label)) => {
                        let rect = Rect::from_min_size(at + Vec2::new(x, 0.0), Vec2::new(w, res.h));
                        super::unit_slot::paint(ui, painter, env, r, rect, label, size);
                    }
                    None => super::unit_slot::forget(ui.ctx(), r.id),
                }
            }
            None => super::unit_slot::forget(ui.ctx(), r.id),
        }
    }
    Vec2::new(content_w + 2.0 * pad, content_h + 2.0 * pad) / zoom
}

/// Where the unit of a result sits beside it: the offset of the unit in the answer,
/// its width, and its text.
type UnitSlot = (f32, f32, String);

/// The answer shown beside an evaluation region. A quantity is its number and then
/// its unit typeset (a raised 2, not a caret), which is also where the unit is
/// changed by clicking it; `typing` is the unit being typed there, if any.
fn answer_beside(
    f: &eframe::egui::epaint::text::Fonts,
    env: &Env,
    m: &MathRegion,
    v: &opencalc_core::value::Value,
    typing: Option<&str>,
    size: f32,
) -> (Layout, Option<UnitSlot>) {
    let colour = if env.stale {
        env.paper.text_weak
    } else {
        env.paper.text
    };
    // While a unit is typed, the number already reads in it once it is a unit of
    // the right kind (unit_in ignores anything else).
    let format = match typing {
        Some(t) => m.format().with_custom_unit(Some(t.to_owned())),
        None => m.format(),
    };
    match crate::format::scalar_parts(v, env.units, &format) {
        Some((number, unit)) if !unit.is_empty() => {
            let ts = Typesetter::new(f, colour);
            let head = plain(f, &format!("= {number}"), colour, size);
            let gap = ts.gap(size * 0.25);
            let tail = match typing {
                // Blank room the size of the text, where the box types.
                Some(t) => {
                    let mut room = ts.text(t, size);
                    room.ops.clear();
                    room.w = room.w.max(size) + size * 0.4;
                    if t.is_empty() {
                        room.h = size;
                        room.baseline = size * 0.78;
                    }
                    room
                }
                None => ts.unit(&unit, size),
            };
            let slot = (head.w + gap.w, tail.w, unit);
            (ts.row(vec![head, gap, tail]), Some(slot))
        }
        _ => (
            plain(
                f,
                &format!("= {}", format_value_with(v, env.units, &m.format())),
                colour,
                size,
            ),
            None,
        ),
    }
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
    let wrap = (r.size.w * zoom - 2.0 * pad).max(10.0);
    let job = super::text_style::job_runs(shown, r.style, &r.runs, TEXT_SIZE * zoom, colour, wrap);
    let galley = painter.layout_job(job);
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

pub(super) fn placeholder(painter: &Painter, theme: &Theme, r: &Region, top_left: Pos2, zoom: f32) {
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
