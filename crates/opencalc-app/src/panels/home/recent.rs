//! The Recent section of the Home page: pinned worksheets, then the ones opened or
//! saved lately, as cards with a picture of the first page, the name, folder, age
//! and size. A card can be pinned and has a menu.

use std::time::SystemTime;

use eframe::egui::{
    self, popup::popup_below_widget, text::LayoutJob, Align2, Color32, CornerRadius, FontId, Id,
    PopupCloseBehavior, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2,
};

use super::info::{info, meta_text, visible_fraction, Info};
use super::layout::columns;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::panels::Commands;
use crate::theme::{metrics, type_size, Theme};

const CARD_MIN_W: f32 = 190.0;
const CARD_GAP: f32 = 12.0;
const PREVIEW_H: f32 = 112.0;
const INFO_H: f32 = 64.0;
const BUTTON: f32 = 24.0;

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

/// A file on the list: its place in `state.recent`, and what is known of it.
struct Entry {
    n: usize,
    path: std::path::PathBuf,
    info: Info,
    pinned: bool,
}

pub fn section(ui: &mut Ui, state: &AppState, width: f32, out: &mut Commands) {
    let theme = state.theme;
    let entries: Vec<Entry> = state
        .recent
        .iter()
        .enumerate()
        .map(|(n, path)| Entry {
            n,
            path: path.clone(),
            info: info(path),
            pinned: state.pinned.contains(path),
        })
        .collect();
    let (pinned, recent): (Vec<&Entry>, Vec<&Entry>) = entries.iter().partition(|e| e.pinned);
    if !pinned.is_empty() {
        super::section_label(ui, &theme, "Pinned");
        ui.add_space(6.0);
        grid(ui, state, width, &pinned, out);
        ui.add_space(12.0);
    }
    ui.horizontal(|ui| {
        ui.set_width(width);
        super::section_label(ui, &theme, "Recent worksheets");
        if !recent.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new("Clear unpinned").small())
                    .on_hover_text("Forget the recent worksheets that are not pinned (the files are not touched)")
                    .clicked()
                {
                    out.push(CommandId::FileClearRecent);
                }
            });
        }
    });
    ui.add_space(6.0);
    if recent.is_empty() {
        let text = if pinned.is_empty() {
            "Worksheets you open or save appear here."
        } else {
            "Nothing else lately."
        };
        ui.label(egui::RichText::new(text).color(theme.text_weak));
        return;
    }
    grid(ui, state, width, &recent, out);
}

fn grid(ui: &mut Ui, state: &AppState, width: f32, entries: &[&Entry], out: &mut Commands) {
    let theme = state.theme;
    let now = SystemTime::now();
    let per_row = columns(width, CARD_MIN_W, CARD_GAP, 4);
    let card_w = (width - CARD_GAP * (per_row as f32 - 1.0)) / per_row as f32;
    for row in entries.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            for entry in row {
                card(ui, state, &theme, card_w, entry, now, out);
            }
        });
        ui.add_space(CARD_GAP - ui.spacing().item_spacing.y);
    }
}

/// The picture area of a card: the first page if it has been drawn, else a glyph.
fn preview(ui: &Ui, state: &AppState, theme: &Theme, rect: Rect, entry: &Entry) {
    let painter = ui.painter_at(rect);
    let top_radius = CornerRadius {
        nw: metrics::WINDOW_RADIUS,
        ne: metrics::WINDOW_RADIUS,
        sw: 0,
        se: 0,
    };
    painter.rect_filled(rect, top_radius, theme.base);
    let texture = entry
        .info
        .found
        .and_then(|stamp| state.thumbs.texture(ui.ctx(), &entry.path, stamp));
    if let Some(texture) = texture {
        let f = visible_fraction(rect.width(), rect.height(), texture.size());
        let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, f));
        painter.image(texture.id(), rect.shrink(1.0), uv, Color32::WHITE);
        return;
    }
    let missing = entry.info.found.is_none();
    let glyph = if entry.info.template {
        crate::icons::TEMPLATE
    } else {
        crate::icons::FILE_TEXT
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(30.0),
        if missing {
            theme.text_disabled
        } else {
            theme.text_weak
        },
    );
    if missing {
        painter.text(
            rect.center() + Vec2::new(0.0, 26.0),
            Align2::CENTER_CENTER,
            "Missing",
            FontId::proportional(type_size::SMALL),
            theme.text_weak,
        );
    }
}

/// One card. A click opens the file; the pin button and the menu button act on it.
fn card(
    ui: &mut Ui,
    state: &AppState,
    theme: &Theme,
    width: f32,
    entry: &Entry,
    now: SystemTime,
    out: &mut Commands,
) {
    let info = &entry.info;
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
    let area = Rect::from_min_size(rect.min, Vec2::new(width, PREVIEW_H));
    preview(ui, state, theme, area, entry);
    // A badge for the kind of file, and a pin mark on a pinned one.
    let badge = if info.template { "OCALCT" } else { "OCALC" };
    let badge_galley = one_line(ui, badge, type_size::SMALL, theme.text, width);
    let badge_rect = Rect::from_min_size(
        area.min + Vec2::new(8.0, 8.0),
        badge_galley.size() + Vec2::new(10.0, 4.0),
    );
    painter.rect_filled(badge_rect, 3.0, theme.raised.gamma_multiply(0.92));
    painter.galley(
        badge_rect.min + Vec2::new(5.0, 2.0),
        badge_galley,
        theme.text,
    );
    if entry.pinned {
        let at = Pos2::new(area.right() - 16.0, area.top() + 16.0);
        painter.circle_filled(at, 11.0, theme.raised.gamma_multiply(0.92));
        painter.text(
            at,
            Align2::CENTER_CENTER,
            crate::icons::PIN,
            FontId::proportional(13.0),
            theme.text,
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

    // The two small buttons at the bottom right: the menu, and the pin.
    let id = Id::new(("recent-card", entry.n));
    let at = |k: f32| {
        Rect::from_min_size(
            Pos2::new(
                rect.right() - 8.0 - BUTTON * k - 2.0 * (k - 1.0),
                rect.bottom() - 8.0 - BUTTON,
            ),
            Vec2::splat(BUTTON),
        )
    };
    let more = button(
        ui,
        theme,
        at(1.0),
        crate::icons::MORE,
        id.with("more"),
        false,
    );
    let pin_glyph = if entry.pinned {
        crate::icons::PIN_OFF
    } else {
        crate::icons::PIN
    };
    let pin = button(ui, theme, at(2.0), pin_glyph, id.with("pin"), entry.pinned);
    let pin_tip = if entry.pinned {
        "Unpin"
    } else {
        "Pin to the top"
    };
    if pin.on_hover_text(pin_tip).clicked() {
        out.push(CommandId::PinRecent(entry.n as u8));
    }
    let menu_id = id.with("menu");
    if more.on_hover_text("More").clicked() {
        ui.memory_mut(|m| m.toggle_popup(menu_id));
    }
    popup_below_widget(
        ui,
        menu_id,
        &more_anchor(ui, at(1.0), id),
        PopupCloseBehavior::CloseOnClick,
        |ui| menu(ui, entry, out),
    );
    resp.context_menu(|ui| menu(ui, entry, out));
    // A click on the card itself (a button over it takes its own) opens the file.
    if resp.clicked() && !missing {
        out.push(CommandId::OpenRecent(entry.n as u8));
    }
}

/// A response standing for the menu button, for a popup to hang below.
fn more_anchor(ui: &Ui, rect: Rect, id: Id) -> egui::Response {
    ui.interact(rect, id.with("more-anchor"), Sense::hover())
}

/// A small square icon button; `lit` draws it as switched on.
fn button(
    ui: &mut Ui,
    theme: &Theme,
    rect: Rect,
    glyph: char,
    id: Id,
    lit: bool,
) -> egui::Response {
    let resp = ui.interact(rect, id, Sense::click());
    let painter = ui.painter();
    if resp.hovered() {
        painter.rect_filled(rect, metrics::RADIUS, theme.raised_high);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    } else if lit {
        painter.rect_filled(rect, metrics::RADIUS, theme.raised);
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(14.0),
        if resp.hovered() || lit {
            theme.text
        } else {
            theme.text_weak
        },
    );
    resp
}

/// The card's menu: what can be done with the file.
fn menu(ui: &mut Ui, entry: &Entry, out: &mut Commands) {
    ui.set_min_width(170.0);
    let missing = entry.info.found.is_none();
    if ui
        .add_enabled(!missing, egui::Button::new("Open"))
        .clicked()
    {
        out.push(CommandId::OpenRecent(entry.n as u8));
        ui.close_menu();
    }
    let pin = if entry.pinned {
        "Unpin"
    } else {
        "Pin to the top"
    };
    if ui.button(pin).clicked() {
        out.push(CommandId::PinRecent(entry.n as u8));
        ui.close_menu();
    }
    if ui.button("Copy path").clicked() {
        ui.ctx().copy_text(entry.path.display().to_string());
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Remove from this list").clicked() {
        out.push(CommandId::ForgetRecent(entry.n as u8));
        ui.close_menu();
    }
}
