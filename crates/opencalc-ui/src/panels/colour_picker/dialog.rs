//! The Colours dialog: hue/saturation square, brightness slider, channel values, hex field.

use eframe::egui::{self, Color32, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::{hex_of, parse_hex, to_color, Pick};
use opencalc_core::document::Colour;

/// The open Colours dialog: the colour being worked on and the one it started as.
#[derive(Clone, Copy)]
pub(super) struct Dialog {
    original: [u8; 4],
    pub(super) hsva: egui::ecolor::Hsva,
}

impl Dialog {
    pub(super) fn new(c: [u8; 4]) -> Self {
        let mut hsva = egui::ecolor::Hsva::from_srgb([c[0], c[1], c[2]]);
        hsva.a = f32::from(c[3]) / 255.0;
        Self { original: c, hsva }
    }

    pub(super) fn rgba(&self) -> [u8; 4] {
        let [r, g, b] = self.hsva.to_srgb();
        [r, g, b, (self.hsva.a * 255.0).round() as u8]
    }

    pub(super) fn set_rgba(&mut self, c: [u8; 4]) {
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
pub(super) fn dialog(ctx: &egui::Context, key: Id) -> Option<Pick> {
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
