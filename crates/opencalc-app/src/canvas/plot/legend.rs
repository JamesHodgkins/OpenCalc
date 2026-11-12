//! The legend of a plot: which series is which.

use eframe::egui::{Color32, FontId, Painter, Pos2, Rect, Vec2};
use opencalc_core::document::{LegendMode, PlotSpec, SeriesKind, SeriesStyle};
use opencalc_core::eval::PlotData;

use super::context::series_name;
use super::marks;
use super::PlotCtx;

/// One legend row: a series' name, how it is drawn and in what colour.
pub struct Entry<'a> {
    pub name: String,
    pub style: &'a SeriesStyle,
    pub colour: Color32,
}

/// The legend's rows, or `None` when the plot is not to show one: when asked for, or
/// by default when there is more than one series. `used[k]` is the position in the
/// spec of the k-th series drawn; series on the right axis are marked as such when
/// `mark_right`.
pub fn entries<'a>(
    ctx: &PlotCtx,
    spec: &PlotSpec,
    data: &PlotData,
    styles: &'a [SeriesStyle],
    used: &[usize],
    mark_right: bool,
) -> Option<Vec<Entry<'a>>> {
    let wanted = match spec.options.legend {
        LegendMode::Hide => false,
        LegendMode::Show => !data.series.is_empty(),
        LegendMode::Auto => data.series.len() >= 2,
    };
    wanted.then(|| {
        styles
            .iter()
            .enumerate()
            .map(|(k, style)| {
                let base = series_name(spec, used.get(k).copied().unwrap_or(k), style);
                Entry {
                    name: if mark_right && data.series[k].right {
                        format!("{base} (right)")
                    } else {
                        base
                    },
                    style,
                    colour: ctx.paper.series[style.colour.map_or(k, usize::from) % 8],
                }
            })
            .collect()
    })
}

/// Draw the legend in the top right of `area`, on a faint backing panel because it
/// usually lands on top of the data.
pub fn draw(
    painter: &Painter,
    ctx: &PlotCtx,
    entries: &[Entry],
    area: Rect,
    font: &FontId,
    zoom: f32,
) {
    let row = font.size * 1.4;
    let x_right = area.max.x - 6.0 * zoom;
    // Lay the names out first so the panel can be sized to the widest.
    let texts: Vec<_> = entries
        .iter()
        .map(|e| painter.layout_no_wrap(e.name.clone(), font.clone(), ctx.paper.text))
        .collect();
    let widest = texts.iter().map(|g| g.size().x).fold(0.0_f32, f32::max);
    let top = area.min.y + 4.0 * zoom;
    let panel = Rect::from_min_max(
        Pos2::new(x_right - widest - 24.0 * zoom, top - 2.0 * zoom),
        Pos2::new(x_right + 3.0 * zoom, top + row * entries.len() as f32),
    );
    painter.rect_filled(panel, 3.0 * zoom, ctx.paper.surface.gamma_multiply(0.85));
    let mut y = top;
    for (entry, text) in entries.iter().zip(texts) {
        let x_text = x_right - text.size().x;
        painter.galley(Pos2::new(x_text, y), text, ctx.paper.text);
        let swatch = Rect::from_min_size(
            Pos2::new(x_text - 20.0 * zoom, y + row * 0.35),
            Vec2::new(14.0 * zoom, (2.0 * zoom).max(1.5)),
        );
        let radius = (3.0 * zoom).max(2.0);
        let (style, colour) = (entry.style, entry.colour);
        match style.kind {
            SeriesKind::Bar => {
                painter.rect_filled(swatch.expand2(Vec2::new(0.0, 3.0 * zoom)), 0.0, colour);
            }
            SeriesKind::Markers => {
                marks::marker(painter, style.marker, swatch.center(), radius, colour);
            }
            kind => {
                painter.rect_filled(swatch, 0.0, colour);
                if matches!(kind, SeriesKind::LineMarkers | SeriesKind::Stem) {
                    marks::marker(painter, style.marker, swatch.center(), radius, colour);
                }
            }
        }
        y += row;
    }
}
