//! A colour button with a drop-down picker laid out like Microsoft Office's:
//! Automatic, "none", the theme palette (a row of colours over columns of their
//! lighter and darker shades), a row of standard colours, and "More colours…",
//! which opens a Colours dialog with a hue/saturation square, a brightness
//! slider, red, green, blue and alpha values, a hex field, and a new/current
//! preview.
//!
//! The picker only reports what was chosen; the caller turns that into an edit.

use eframe::egui::{
    self, popup::popup_below_widget, Color32, Id, PopupCloseBehavior, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};
use opencalc_core::document::{shade_mix, Colour, STANDARD_COLOURS, THEME_SHADES, THEME_SLOTS};

use crate::theme::Theme;

const SWATCH: f32 = 24.0;
const GAP: f32 = 4.0;

/// What the user chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    /// The new value: none, automatic, a palette colour or an exact one.
    pub colour: Option<Colour>,
    /// Whether this finishes a step. False while a value is still being dragged,
    /// so the whole drag undoes as one.
    pub done: bool,
}

/// `#RRGGBB`, or `#RRGGBBAA` when not opaque.
pub fn hex_of(c: [u8; 4]) -> String {
    if c[3] == 255 {
        format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3])
    }
}

/// Reads `RRGGBB` or `RRGGBBAA`, with or without a leading `#`.
pub fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let t = text.trim().trim_start_matches('#');
    if !t.is_ascii() || (t.len() != 6 && t.len() != 8) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    Some([
        byte(0)?,
        byte(2)?,
        byte(4)?,
        if t.len() == 8 { byte(6)? } else { 255 },
    ])
}

pub(super) fn to_color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// A swatch: the colour over a light and dark checker so transparency shows; a
/// red slash for "none".
fn swatch(ui: &mut Ui, rect: Rect, colour: Option<[u8; 4]>, theme: &Theme) {
    let p = ui.painter();
    p.rect_filled(rect, 2.0, Color32::WHITE);
    match colour {
        Some(c) => {
            if c[3] < 255 {
                let half = rect.size() / 2.0;
                for (dx, dy) in [(0.0, 0.0), (1.0, 1.0)] {
                    let min = rect.min + Vec2::new(dx * half.x, dy * half.y);
                    p.rect_filled(Rect::from_min_size(min, half), 0.0, Color32::from_gray(200));
                }
            }
            p.rect_filled(rect, 2.0, to_color(c));
        }
        None => {
            p.line_segment(
                [rect.left_bottom(), rect.right_top()],
                Stroke::new(1.5_f32, Color32::from_rgb(200, 40, 40)),
            );
        }
    }
    p.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0_f32, theme.border_strong),
        StrokeKind::Inside,
    );
}

fn swatch_button(
    ui: &mut Ui,
    colour: Colour,
    auto: [u8; 4],
    selected: bool,
    theme: &Theme,
    tip: &str,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(SWATCH), Sense::click());
    // Hovering grows the swatch a little, like the web picker's scale-110.
    let drawn = if resp.hovered() {
        rect.expand(1.2)
    } else {
        rect
    };
    swatch(ui, drawn, Some(colour.resolve(auto)), theme);
    if selected {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            3.0,
            Stroke::new(2.0_f32, theme.accent),
            StrokeKind::Outside,
        );
    }
    resp.on_hover_text(tip)
}

/// The palette's colour names, by column.
const SLOT_NAMES: [&str; THEME_SLOTS as usize] = [
    "White",
    "Black",
    "Light grey",
    "Slate",
    "Blue",
    "Orange",
    "Grey",
    "Amber",
    "Sky blue",
    "Green",
];

/// A palette place's name: its colour and how much lighter or darker it is.
fn place_name(slot: u8, shade: u8) -> String {
    let name = SLOT_NAMES[slot as usize];
    match shade_mix(slot, shade) {
        0 => name.to_string(),
        n if n > 0 => format!("{name}, lighter {n}%"),
        n => format!("{name}, darker {}%", -n),
    }
}

/// A small upper-case section heading.
fn heading(ui: &mut Ui, text: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(11.0)
            .strong()
            .color(theme.text_weak),
    );
}

/// A full-width action row ("Automatic", "No fill") with its swatch on the left.
fn bar(ui: &mut Ui, label: &str, face: Option<[u8; 4]>, selected: bool, theme: &Theme) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(SWATCH * 10.0), 28.0),
        Sense::click(),
    );
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, theme.accent_hover);
    }
    if selected {
        ui.painter().rect_stroke(
            rect,
            4.0,
            Stroke::new(1.5_f32, theme.accent),
            StrokeKind::Inside,
        );
    }
    let sw = Rect::from_center_size(
        Pos2::new(rect.left() + 16.0, rect.center().y),
        Vec2::splat(16.0),
    );
    swatch(ui, sw, face, theme);
    ui.painter().text(
        Pos2::new(rect.left() + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        theme.text,
    );
    resp.clicked()
}

/// The colour button. `current` is none, or a colour; `auto` is what Automatic
/// looks like here; `none_label` names "none" ("No fill", "No outline").
/// Returns a choice made this frame.
pub fn button(
    ui: &mut Ui,
    id: Id,
    current: Option<Colour>,
    auto: [u8; 4],
    none_label: &str,
    theme: &Theme,
) -> Option<Pick> {
    let face = current.map(|c| c.resolve(auto));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(46.0, 22.0), Sense::click());
    let inner = Rect::from_min_size(rect.min + Vec2::new(3.0, 3.0), Vec2::new(26.0, 16.0));
    ui.painter().rect_filled(
        rect,
        3.0,
        if resp.hovered() {
            theme.accent_hover
        } else {
            theme.base
        },
    );
    swatch(ui, inner, face, theme);
    crate::ribbon::widgets::paint_arrow(
        ui,
        Rect::from_min_max(Pos2::new(rect.right() - 16.0, rect.top()), rect.max),
        theme,
        crate::ribbon::widgets::Look {
            enabled: true,
            active: false,
        },
    );
    let popup = id.with("colour-popup");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    let mut pick = None;
    let dialog_key = id.with("colour-dialog");
    popup_below_widget(
        ui,
        popup,
        &resp,
        PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(SWATCH * 10.0 + GAP * 9.0 + 8.0);
            ui.spacing_mut().item_spacing = Vec2::new(GAP, GAP);
            let mut chosen = |c: Option<Colour>| {
                pick = Some(Pick {
                    colour: c,
                    done: true,
                });
            };
            heading(ui, "Theme colours", theme);
            for shade in 0..THEME_SHADES {
                ui.horizontal(|ui| {
                    for slot in 0..THEME_SLOTS {
                        let colour = Colour::theme(slot, shade);
                        let tip = place_name(slot, shade);
                        if swatch_button(ui, colour, auto, current == Some(colour), theme, &tip)
                            .clicked()
                        {
                            chosen(Some(colour));
                        }
                    }
                });
            }
            ui.separator();
            heading(ui, "Standard colours", theme);
            ui.horizontal(|ui| {
                for [r, g, b] in STANDARD_COLOURS.iter() {
                    let colour = Colour::rgb(*r, *g, *b);
                    let tip = hex_of([*r, *g, *b, 255]);
                    if swatch_button(ui, colour, auto, current == Some(colour), theme, &tip)
                        .clicked()
                    {
                        chosen(Some(colour));
                    }
                }
            });
            ui.separator();
            ui.spacing_mut().item_spacing = Vec2::new(GAP, 2.0);
            if bar(
                ui,
                "Automatic",
                Some(auto),
                current == Some(Colour::Auto),
                theme,
            ) {
                chosen(Some(Colour::Auto));
            }
            if bar(ui, none_label, None, current.is_none(), theme) {
                chosen(None);
            }
            let c = current.map_or(auto, |c| c.resolve(auto));
            if bar(ui, "More colours\u{2026}", Some(c), false, theme) {
                ui.memory_mut(|m| {
                    m.data.insert_temp(dialog_key, Dialog::new(c));
                    m.close_popup();
                });
            }
        },
    );
    if let Some(done) = dialog(ui.ctx(), dialog_key) {
        pick = Some(done);
    }
    pick
}
mod dialog;
use dialog::{dialog, Dialog};

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::THEME_COLOURS;

    #[test]
    fn hex_round_trips_and_shows_alpha_only_when_needed() {
        assert_eq!(hex_of([255, 0, 128, 255]), "#FF0080");
        assert_eq!(hex_of([1, 2, 3, 4]), "#01020304");
        assert_eq!(parse_hex("#FF0080"), Some([255, 0, 128, 255]));
        assert_eq!(parse_hex("ff008040"), Some([255, 0, 128, 64]));
        assert_eq!(parse_hex("  #abc  "), None);
        assert_eq!(parse_hex("#GG0000"), None);
        assert_eq!(parse_hex("#é12345"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn every_palette_place_has_a_name() {
        assert_eq!(SLOT_NAMES.len(), THEME_COLOURS.len());
        assert_eq!(place_name(0, 1), "White, darker 5%");
        assert_eq!(place_name(1, 1), "Black, lighter 50%");
        assert_eq!(place_name(4, 0), "Blue");
    }

    #[test]
    fn dialog_colour_survives_a_round_trip_and_keeps_hue_on_grey() {
        let mut d = Dialog::new([200, 40, 40, 128]);
        assert_eq!(d.rgba(), [200, 40, 40, 128]);
        let hue = d.hsva.h;
        d.set_rgba([90, 90, 90, 255]);
        assert_eq!(d.hsva.h, hue);
        assert_eq!(d.rgba(), [90, 90, 90, 255]);
    }
}
