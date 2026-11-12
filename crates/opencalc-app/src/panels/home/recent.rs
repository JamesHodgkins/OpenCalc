//! The Recent section of the Home page: the worksheets opened or saved lately, as
//! cards with their name, folder, age and size.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{
    self, text::LayoutJob, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke,
    StrokeKind, TextFormat, Ui, Vec2,
};

use super::layout::columns;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::panels::Commands;
use crate::theme::{metrics, type_size, Theme};

const CARD_MIN_W: f32 = 190.0;
const CARD_GAP: f32 = 12.0;
const PREVIEW_H: f32 = 72.0;
const INFO_H: f32 = 64.0;

/// What is known about a file on the recent list.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub name: String,
    pub folder: String,
    /// `None` when the file is no longer there.
    pub found: Option<(SystemTime, u64)>,
    pub template: bool,
}

pub fn info(path: &Path) -> Info {
    let name = path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |f| f.to_string_lossy().into_owned(),
    );
    let folder = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let found = std::fs::metadata(path)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())));
    let template = path
        .extension()
        .is_some_and(opencalc_core::persist::is_template_extension);
    Info {
        name,
        folder,
        found,
        template,
    }
}

/// "just now", "5 minutes ago", "3 days ago".
pub fn age_text(elapsed_secs: u64) -> String {
    let (n, unit) = match elapsed_secs {
        0..=59 => return "just now".to_owned(),
        60..=3599 => (elapsed_secs / 60, "minute"),
        3600..=86_399 => (elapsed_secs / 3600, "hour"),
        86_400..=2_591_999 => (elapsed_secs / 86_400, "day"),
        2_592_000..=31_535_999 => (elapsed_secs / 2_592_000, "month"),
        _ => (elapsed_secs / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// "812 B", "12 KB", "1.4 MB".
pub fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", (bytes as f64 / 1024.0).round() as u64)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn meta_text(info: &Info, now: SystemTime) -> String {
    match info.found {
        Some((modified, bytes)) => {
            let secs = now.duration_since(modified).map_or(0, |d| d.as_secs());
            format!("{} \u{b7} {}", age_text(secs), size_text(bytes))
        }
        None => "Missing".to_owned(),
    }
}

/// One line of text, cut with an ellipsis where it would run past `width`.
fn one_line(
    ui: &Ui,
    text: &str,
    size: f32,
    colour: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat::simple(FontId::proportional(size), colour),
    );
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    ui.fonts(|f| f.layout_job(job))
}

pub fn section(ui: &mut Ui, state: &AppState, width: f32, out: &mut Commands) {
    let theme = state.theme;
    ui.horizontal(|ui| {
        ui.set_width(width);
        super::section_label(ui, &theme, "Recent worksheets");
        if !state.recent.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new("Clear list").small())
                    .on_hover_text("Forget the recent worksheets (the files are not touched)")
                    .clicked()
                {
                    out.push(CommandId::FileClearRecent);
                }
            });
        }
    });
    ui.add_space(6.0);
    if state.recent.is_empty() {
        ui.label(
            egui::RichText::new("Worksheets you open or save appear here.").color(theme.text_weak),
        );
        return;
    }
    let now = SystemTime::now();
    let per_row = columns(width, CARD_MIN_W, CARD_GAP, 4);
    let card_w = (width - CARD_GAP * (per_row as f32 - 1.0)) / per_row as f32;
    let infos: Vec<(usize, PathBuf, Info)> = state
        .recent
        .iter()
        .enumerate()
        .map(|(n, p)| (n, p.clone(), info(p)))
        .collect();
    for row in infos.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            for (n, path, info) in row {
                if card(ui, &theme, card_w, path, info, now) {
                    out.push(CommandId::OpenRecent(*n as u8));
                }
            }
        });
        ui.add_space(CARD_GAP - ui.spacing().item_spacing.y);
    }
}

/// One card; returns whether it was clicked.
fn card(ui: &mut Ui, theme: &Theme, width: f32, path: &Path, info: &Info, now: SystemTime) -> bool {
    let missing = info.found.is_none();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, PREVIEW_H + INFO_H), Sense::click());
    let painter = ui.painter_at(rect);
    let radius = CornerRadius::same(metrics::WINDOW_RADIUS);
    let hovered = resp.hovered() && !missing;
    painter.rect_filled(
        rect,
        radius,
        if hovered { theme.raised } else { theme.surface },
    );
    // The preview: no thumbnail yet, so a file glyph on a sunken field.
    let preview = Rect::from_min_size(rect.min, Vec2::new(width, PREVIEW_H));
    let top_radius = CornerRadius {
        nw: metrics::WINDOW_RADIUS,
        ne: metrics::WINDOW_RADIUS,
        sw: 0,
        se: 0,
    };
    painter.rect_filled(preview, top_radius, theme.base);
    let glyph = if info.template {
        crate::icons::TEMPLATE
    } else {
        crate::icons::FILE_TEXT
    };
    painter.text(
        preview.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(30.0),
        if missing {
            theme.text_disabled
        } else {
            theme.text_weak
        },
    );
    // A badge for the kind of file.
    let badge = if info.template { "OCALCT" } else { "OCALC" };
    let badge_galley = one_line(ui, badge, type_size::SMALL, theme.text_weak, width);
    let badge_rect = Rect::from_min_size(
        preview.min + Vec2::new(8.0, 8.0),
        badge_galley.size() + Vec2::new(10.0, 4.0),
    );
    painter.rect_filled(badge_rect, 3.0, theme.raised);
    painter.galley(
        badge_rect.min + Vec2::new(5.0, 2.0),
        badge_galley,
        theme.text_weak,
    );
    if missing {
        painter.text(
            preview.center() + Vec2::new(0.0, 26.0),
            Align2::CENTER_CENTER,
            "Missing",
            FontId::proportional(type_size::SMALL),
            theme.text_weak,
        );
    }
    // Name, folder, age and size.
    let (strong, weak) = if missing {
        (theme.text_disabled, theme.text_disabled)
    } else {
        (theme.text, theme.text_weak)
    };
    let inner = width - 20.0;
    let mut y = rect.min.y + PREVIEW_H + 8.0;
    for (text, size, colour) in [
        (info.name.as_str(), type_size::BODY, strong),
        (info.folder.as_str(), type_size::SMALL, weak),
        (meta_text(info, now).as_str(), type_size::SMALL, weak),
    ] {
        let galley = one_line(ui, text, size, colour, inner);
        let h = galley.size().y;
        painter.galley(Pos2::new(rect.min.x + 10.0, y), galley, colour);
        y += h + 3.0;
    }
    let border = if hovered {
        theme.border_strong
    } else {
        theme.border
    };
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let tip = if missing {
        format!("{}\nThis file is no longer there", path.display())
    } else {
        path.display().to_string()
    };
    resp.on_hover_text(tip).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_read_naturally() {
        assert_eq!(age_text(5), "just now");
        assert_eq!(age_text(60), "1 minute ago");
        assert_eq!(age_text(150), "2 minutes ago");
        assert_eq!(age_text(3600), "1 hour ago");
        assert_eq!(age_text(86_400 * 3), "3 days ago");
        assert_eq!(age_text(86_400 * 60), "2 months ago");
        assert_eq!(age_text(86_400 * 800), "2 years ago");
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(size_text(812), "812 B");
        assert_eq!(size_text(12 * 1024), "12 KB");
        assert_eq!(size_text(1_500_000), "1.4 MB");
    }

    #[test]
    fn a_missing_file_says_so_and_a_template_is_marked() {
        let gone = info(Path::new("C:/nowhere/at/all/beam.ocalc"));
        assert_eq!(gone.name, "beam.ocalc");
        assert!(gone.found.is_none() && !gone.template);
        assert_eq!(meta_text(&gone, SystemTime::now()), "Missing");
        assert!(info(Path::new("x/starter.ocalct")).template);
    }
}
