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

const SWATCH: f32 = 20.0;
const GAP: f32 = 2.0;

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

fn to_color(c: [u8; 4]) -> Color32 {
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
        Stroke::new(1.0_f32, theme.text_weak),
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
    swatch(ui, rect, Some(colour.resolve(auto)), theme);
    if selected || resp.hovered() {
        ui.painter().rect_stroke(
            rect.expand(1.5),
            2.0,
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

/// A full-width choice bar ("Automatic", "No fill") with its swatch.
fn bar(ui: &mut Ui, label: &str, face: Option<[u8; 4]>, selected: bool, theme: &Theme) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(SWATCH * 10.0), 22.0),
        Sense::click(),
    );
    if resp.hovered() || selected {
        ui.painter().rect_stroke(
            rect,
            2.0,
            Stroke::new(1.5_f32, theme.accent),
            StrokeKind::Inside,
        );
    }
    let sw = Rect::from_min_size(rect.min + Vec2::new(4.0, 3.0), Vec2::new(24.0, 16.0));
    swatch(ui, sw, face, theme);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
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
            ui.spacing_mut().item_spacing = Vec2::new(GAP, 4.0);
            let mut chosen = |c: Option<Colour>| {
                pick = Some(Pick {
                    colour: c,
                    done: true,
                });
            };
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
            ui.add_space(2.0);
            ui.label(egui::RichText::new("Theme colours").strong());
            ui.spacing_mut().item_spacing = Vec2::new(GAP, 0.0);
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
                if shade == 0 {
                    ui.add_space(5.0);
                }
            }
            ui.spacing_mut().item_spacing = Vec2::new(GAP, 4.0);
            ui.add_space(2.0);
            ui.label(egui::RichText::new("Standard colours").strong());
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
            if ui.button("More colours\u{2026}").clicked() {
                let c = current.map_or(auto, |c| c.resolve(auto));
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

/// The open Colours dialog: the colour being worked on and the one it started as.
#[derive(Clone, Copy)]
struct Dialog {
    original: [u8; 4],
    hsva: egui::ecolor::Hsva,
}

impl Dialog {
    fn new(c: [u8; 4]) -> Self {
        let mut hsva = egui::ecolor::Hsva::from_srgb([c[0], c[1], c[2]]);
        hsva.a = f32::from(c[3]) / 255.0;
        Self { original: c, hsva }
    }

    fn rgba(&self) -> [u8; 4] {
        let [r, g, b] = self.hsva.to_srgb();
        [r, g, b, (self.hsva.a * 255.0).round() as u8]
    }

    fn set_rgba(&mut self, c: [u8; 4]) {
        let old = self.hsva;
        let mut n = egui::ecolor::Hsva::from_srgb([c[0], c[1], c[2]]);
        // Greys and black have no hue or saturation of their own: keep the ones
        // the user was on so the picker does not jump.
        if n.v == 0.0 {
            n.s = old.s;
            n.h = old.h;
        } else if n.s == 0.0 {
            n.h = old.h;
        }
        n.a = f32::from(c[3]) / 255.0;
        self.hsva = n;
    }
}

fn hsv_colour(h: f32, s: f32, v: f32) -> Color32 {
    let [r, g, b] = egui::ecolor::Hsva::new(h, s, v, 1.0).to_srgb();
    Color32::from_rgb(r, g, b)
}

fn quad(painter: &egui::Painter, rect: Rect, tl: Color32, tr: Color32, br: Color32, bl: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), tl);
    mesh.colored_vertex(rect.right_top(), tr);
    mesh.colored_vertex(rect.right_bottom(), br);
    mesh.colored_vertex(rect.left_bottom(), bl);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// Shows the dialog if open. Returns the pick when OK is pressed.
fn dialog(ctx: &egui::Context, key: Id) -> Option<Pick> {
    let mut d = ctx.memory(|m| m.data.get_temp::<Dialog>(key))?;
    let mut result = None;
    let mut close = false;
    egui::Window::new("Colours")
        .id(key.with("window"))
        .collapsible(false)
        .resizable(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let size = Vec2::new(240.0, 170.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                let v = d.hsva.v;
                let cols = 6;
                for i in 0..cols {
                    let (h0, h1) = (i as f32 / cols as f32, (i + 1) as f32 / cols as f32);
                    let x0 = rect.left() + rect.width() * h0;
                    let x1 = rect.left() + rect.width() * h1;
                    quad(
                        &painter,
                        Rect::from_x_y_ranges(x0..=x1, rect.y_range()),
                        hsv_colour(h0, 1.0, v),
                        hsv_colour(h1.min(0.999), 1.0, v),
                        hsv_colour(h1.min(0.999), 0.0, v),
                        hsv_colour(h0, 0.0, v),
                    );
                }
                if let (true, Some(p)) = (
                    resp.dragged() || resp.clicked(),
                    resp.interact_pointer_pos(),
                ) {
                    d.hsva.h = ((p.x - rect.left()) / rect.width()).clamp(0.0, 0.999);
                    d.hsva.s = (1.0 - (p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                }
                let at = Pos2::new(
                    rect.left() + d.hsva.h * rect.width(),
                    rect.top() + (1.0 - d.hsva.s) * rect.height(),
                );
                painter.circle_stroke(at, 5.0, Stroke::new(2.0_f32, Color32::WHITE));
                painter.circle_stroke(at, 6.5, Stroke::new(1.0_f32, Color32::BLACK));

                let (srect, sresp) =
                    ui.allocate_exact_size(Vec2::new(16.0, size.y), Sense::click_and_drag());
                let sp = ui.painter_at(srect.expand(3.0));
                quad(
                    &sp,
                    srect,
                    hsv_colour(d.hsva.h, d.hsva.s, 1.0),
                    hsv_colour(d.hsva.h, d.hsva.s, 1.0),
                    Color32::BLACK,
                    Color32::BLACK,
                );
                if let (true, Some(p)) = (
                    sresp.dragged() || sresp.clicked(),
                    sresp.interact_pointer_pos(),
                ) {
                    d.hsva.v = (1.0 - (p.y - srect.top()) / srect.height()).clamp(0.0, 1.0);
                }
                let y = srect.top() + (1.0 - d.hsva.v) * srect.height();
                sp.rect_stroke(
                    Rect::from_x_y_ranges(srect.x_range(), (y - 2.0)..=(y + 2.0)),
                    1.0,
                    Stroke::new(1.5_f32, Color32::WHITE),
                    StrokeKind::Outside,
                );
            });
            ui.add_space(6.0);
            let mut c = d.rgba();
            let mut changed = false;
            ui.horizontal(|ui| {
                for (k, name) in ["Red", "Green", "Blue", "Alpha"].iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.label(*name);
                        changed |= ui
                            .add(egui::DragValue::new(&mut c[k]).range(0..=255).speed(1.0))
                            .changed();
                    });
                }
            });
            let hex_key = key.with("hex");
            ui.horizontal(|ui| {
                ui.label("Hex");
                let mut text = ui
                    .memory(|m| m.data.get_temp::<(String, [u8; 4])>(hex_key))
                    .filter(|(_, was)| *was == c)
                    .map_or_else(|| hex_of(c), |(t, _)| t);
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(90.0));
                if r.changed() {
                    ui.memory_mut(|m| m.data.insert_temp(hex_key, (text.clone(), c)));
                }
                if r.lost_focus() {
                    if let Some(parsed) = parse_hex(&text) {
                        c = parsed;
                        changed = true;
                    }
                    ui.memory_mut(|m| m.data.remove::<(String, [u8; 4])>(hex_key));
                }
            });
            if changed {
                d.set_rgba(c);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(60.0, 40.0), Sense::hover());
                let top = Rect::from_min_size(r.min, Vec2::new(r.width(), r.height() / 2.0));
                let bottom = Rect::from_min_max(top.left_bottom(), r.max);
                let p = ui.painter();
                p.rect_filled(r, 0.0, Color32::WHITE);
                p.rect_filled(top, 0.0, to_color(d.rgba()));
                p.rect_filled(bottom, 0.0, to_color(d.original));
                ui.vertical(|ui| {
                    ui.label("New (top), Current (bottom)");
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            result = Some(Pick {
                                colour: Some(Colour::Rgba(d.rgba())),
                                done: true,
                            });
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    ctx.memory_mut(|m| {
        if close {
            m.data.remove::<Dialog>(key);
        } else {
            m.data.insert_temp(key, d);
        }
    });
    result
}

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
