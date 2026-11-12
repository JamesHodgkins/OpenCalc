//! A small software renderer for egui's output, so a page can be drawn to an
//! image without a window or a graphics card.
//!
//! egui tessellates everything it paints into triangles with a colour per corner
//! and, for text, coordinates into the font texture. This draws those triangles
//! the way egui's own back ends do: the corner colours and the texture are
//! premultiplied, multiplied together, and blended over what is there.

use std::collections::HashMap;

use eframe::egui::epaint::textures::TexturesDelta;
use eframe::egui::epaint::{ImageData, Primitive};
use eframe::egui::{ClippedPrimitive, Color32, TextureId};

/// A texture egui asked for, kept as premultiplied colours.
struct Texture {
    size: [usize; 2],
    pixels: Vec<Color32>,
}

/// The textures egui has set so far (the font atlas, mostly).
#[derive(Default)]
pub struct Textures {
    map: HashMap<TextureId, Texture>,
}

impl Textures {
    /// Apply the texture changes of one frame.
    pub fn apply(&mut self, delta: &TexturesDelta) {
        for (id, change) in &delta.set {
            let (size, pixels): ([usize; 2], Vec<Color32>) = match &change.image {
                ImageData::Color(image) => (image.size, image.pixels.clone()),
                ImageData::Font(image) => (image.size, image.srgba_pixels(None).collect()),
            };
            match change.pos {
                None => {
                    self.map.insert(*id, Texture { size, pixels });
                }
                Some([x0, y0]) => {
                    if let Some(tex) = self.map.get_mut(id) {
                        // A patch into an existing texture; ignore one that does not fit.
                        if x0 + size[0] <= tex.size[0] && y0 + size[1] <= tex.size[1] {
                            for row in 0..size[1] {
                                let from = row * size[0];
                                let to = (y0 + row) * tex.size[0] + x0;
                                tex.pixels[to..to + size[0]]
                                    .copy_from_slice(&pixels[from..from + size[0]]);
                            }
                        }
                    }
                }
            }
        }
    }

    /// The colour at texture coordinates `(u, v)`, bilinearly filtered, as
    /// premultiplied floats in `0..=1`. A texture we do not have is plain white.
    fn sample(&self, id: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some(tex) = self.map.get(&id) else {
            return [1.0; 4];
        };
        let [w, h] = tex.size;
        if w == 0 || h == 0 {
            return [1.0; 4];
        }
        let x = (u * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
        let y = (v * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |px: usize, py: usize| {
            let c = tex.pixels[py * w + px];
            [c.r(), c.g(), c.b(), c.a()].map(|b| f32::from(b) / 255.0)
        };
        let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
        let mut out = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bottom = c[k] + (d[k] - c[k]) * fx;
            out[k] = top + (bottom - top) * fy;
        }
        out
    }
}

/// A picture being drawn: opaque premultiplied pixels, starting white.
pub struct Image {
    pub width: usize,
    pub height: usize,
    px: Vec<[u8; 4]>,
}

impl Image {
    pub fn white(width: usize, height: usize) -> Image {
        Image {
            width,
            height,
            px: vec![[255; 4]; width * height],
        }
    }

    /// The pixels as RGB bytes, row by row from the top.
    pub fn rgb(&self) -> Vec<u8> {
        self.px.iter().flat_map(|p| [p[0], p[1], p[2]]).collect()
    }

    #[cfg(test)]
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let p = self.px[y * self.width + x];
        [p[0], p[1], p[2]]
    }

    /// Draw what egui painted, at `ppp` pixels per point.
    pub fn draw(&mut self, primitives: &[ClippedPrimitive], textures: &Textures, ppp: f32) {
        for clipped in primitives {
            let Primitive::Mesh(mesh) = &clipped.primitive else {
                continue;
            };
            let clip = clipped.clip_rect;
            let clip = [
                (clip.min.x * ppp).max(0.0),
                (clip.min.y * ppp).max(0.0),
                (clip.max.x * ppp).min(self.width as f32),
                (clip.max.y * ppp).min(self.height as f32),
            ];
            let (triangles, _) = mesh.indices.as_chunks::<3>();
            for tri in triangles {
                let corner = |i: u32| mesh.vertices.get(i as usize);
                let (Some(a), Some(b), Some(c)) = (corner(tri[0]), corner(tri[1]), corner(tri[2]))
                else {
                    continue;
                };
                self.triangle([a, b, c], mesh.texture_id, textures, ppp, clip);
            }
        }
    }

    fn triangle(
        &mut self,
        v: [&eframe::egui::epaint::Vertex; 3],
        texture: TextureId,
        textures: &Textures,
        ppp: f32,
        clip: [f32; 4],
    ) {
        let p = v.map(|v| (v.pos.x * ppp, v.pos.y * ppp));
        let area = edge(p[0], p[1], p[2]);
        if area == 0.0 || !area.is_finite() {
            return;
        }
        let inv = 1.0 / area;
        // Pixels exactly on an edge shared by two triangles belong to one of them,
        // chosen by the edge's direction, so they are not drawn twice.
        let flip = area < 0.0;
        let owns = |a: (f32, f32), b: (f32, f32)| {
            let (dx, dy) = if flip {
                (a.0 - b.0, a.1 - b.1)
            } else {
                (b.0 - a.0, b.1 - a.1)
            };
            dy > 0.0 || (dy == 0.0 && dx > 0.0)
        };
        let owned = [owns(p[1], p[2]), owns(p[2], p[0]), owns(p[0], p[1])];
        let min_x = p
            .iter()
            .map(|q| q.0)
            .fold(f32::INFINITY, f32::min)
            .max(clip[0]);
        let max_x = p
            .iter()
            .map(|q| q.0)
            .fold(f32::NEG_INFINITY, f32::max)
            .min(clip[2]);
        let min_y = p
            .iter()
            .map(|q| q.1)
            .fold(f32::INFINITY, f32::min)
            .max(clip[1]);
        let max_y = p
            .iter()
            .map(|q| q.1)
            .fold(f32::NEG_INFINITY, f32::max)
            .min(clip[3]);
        if !(min_x < max_x && min_y < max_y) {
            return;
        }
        let colours = v.map(|v| {
            let c = v.color;
            [c.r(), c.g(), c.b(), c.a()].map(|b| f32::from(b) / 255.0)
        });
        // Solid triangles (no texture lookup) are the common case: a flat colour is
        // quick to blend.
        let (x0, x1) = (
            min_x.floor() as usize,
            (max_x.ceil() as usize).min(self.width),
        );
        let (y0, y1) = (
            min_y.floor() as usize,
            (max_y.ceil() as usize).min(self.height),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                let q = (x as f32 + 0.5, y as f32 + 0.5);
                if q.0 < clip[0] || q.0 >= clip[2] || q.1 < clip[1] || q.1 >= clip[3] {
                    continue;
                }
                let w0 = edge(p[1], p[2], q) * inv;
                let w1 = edge(p[2], p[0], q) * inv;
                let w2 = edge(p[0], p[1], q) * inv;
                let inside = |w: f32, own: bool| w > 0.0 || (w == 0.0 && own);
                if !(inside(w0, owned[0]) && inside(w1, owned[1]) && inside(w2, owned[2])) {
                    continue;
                }
                let mix = |f: &dyn Fn(usize) -> f32| w0 * f(0) + w1 * f(1) + w2 * f(2);
                let u = mix(&|k| v[k].uv.x);
                let t = mix(&|k| v[k].uv.y);
                let tex = textures.sample(texture, u, t);
                let mut src = [0.0; 4];
                for ch in 0..4 {
                    src[ch] = mix(&|k| colours[k][ch]) * tex[ch];
                }
                self.blend(x, y, src);
            }
        }
    }

    /// `src` (premultiplied, `0..=1`) over the pixel: `src + dst * (1 - src.a)`.
    fn blend(&mut self, x: usize, y: usize, src: [f32; 4]) {
        let dst = &mut self.px[y * self.width + x];
        let keep = 1.0 - src[3].clamp(0.0, 1.0);
        for ch in 0..4 {
            let value = src[ch] * 255.0 + f32::from(dst[ch]) * keep;
            dst[ch] = value.round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Twice the signed area of the triangle `a, b, c`; positive for one winding.
fn edge(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::epaint::{Mesh, Vertex};
    use eframe::egui::{pos2, Rect};

    fn quad(rect: Rect, colour: Color32, uv: Rect, id: TextureId) -> ClippedPrimitive {
        let mut mesh = Mesh::with_texture(id);
        let v = |p, uv| Vertex {
            pos: p,
            uv,
            color: colour,
        };
        mesh.vertices = vec![
            v(rect.left_top(), uv.left_top()),
            v(rect.right_top(), uv.right_top()),
            v(rect.right_bottom(), uv.right_bottom()),
            v(rect.left_bottom(), uv.left_bottom()),
        ];
        mesh.indices = vec![0, 1, 2, 0, 2, 3];
        ClippedPrimitive {
            clip_rect: Rect::EVERYTHING,
            primitive: Primitive::Mesh(mesh),
        }
    }

    fn solid(rect: Rect, colour: Color32) -> ClippedPrimitive {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(0.0, 0.0));
        quad(rect, colour, uv, TextureId::Managed(7))
    }

    #[test]
    fn a_rectangle_fills_exactly_its_pixels() {
        let mut img = Image::white(10, 10);
        let red = Color32::from_rgb(200, 0, 0);
        let r = Rect::from_min_max(pos2(2.0, 3.0), pos2(6.0, 8.0));
        img.draw(&[solid(r, red)], &Textures::default(), 1.0);
        for y in 0..10 {
            for x in 0..10 {
                let inside = (2..6).contains(&x) && (3..8).contains(&y);
                let want = if inside { [200, 0, 0] } else { [255, 255, 255] };
                assert_eq!(img.pixel(x, y), want, "({x}, {y})");
            }
        }
    }

    #[test]
    fn the_scale_turns_points_into_pixels() {
        let mut img = Image::white(8, 8);
        let r = Rect::from_min_max(pos2(1.0, 1.0), pos2(3.0, 2.0));
        img.draw(&[solid(r, Color32::BLACK)], &Textures::default(), 2.0);
        assert_eq!(img.pixel(2, 2), [0, 0, 0]);
        assert_eq!(img.pixel(5, 3), [0, 0, 0]);
        assert_eq!(img.pixel(6, 2), [255, 255, 255]);
        assert_eq!(img.pixel(2, 4), [255, 255, 255]);
    }

    #[test]
    fn a_translucent_colour_blends_over_what_is_there() {
        let mut img = Image::white(4, 4);
        // Half-transparent black, premultiplied: (0, 0, 0, 128).
        let half = Color32::from_black_alpha(128);
        img.draw(
            &[solid(
                Rect::from_min_max(pos2(0.0, 0.0), pos2(4.0, 4.0)),
                half,
            )],
            &Textures::default(),
            1.0,
        );
        let [r, g, b] = img.pixel(1, 1);
        assert!((126..=129).contains(&r) && r == g && g == b, "{r} {g} {b}");
    }

    #[test]
    fn the_clip_rectangle_limits_drawing_and_either_winding_works() {
        let mut img = Image::white(10, 10);
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let mut p = solid(r, Color32::BLACK);
        p.clip_rect = Rect::from_min_max(pos2(2.0, 2.0), pos2(4.0, 4.0));
        if let Primitive::Mesh(m) = &mut p.primitive {
            m.indices = vec![0, 2, 1, 0, 3, 2]; // the other way round
        }
        img.draw(&[p], &Textures::default(), 1.0);
        assert_eq!(img.pixel(3, 3), [0, 0, 0]);
        assert_eq!(img.pixel(1, 3), [255, 255, 255]);
        assert_eq!(img.pixel(5, 5), [255, 255, 255]);
    }

    #[test]
    fn a_texture_is_sampled_and_multiplied_by_the_corner_colour() {
        use eframe::egui::epaint::ImageDelta;
        use eframe::egui::ColorImage;
        use eframe::egui::TextureOptions;
        let id = TextureId::Managed(1);
        // A 2x1 texture: left transparent, right opaque white.
        let image = ColorImage {
            size: [2, 1],
            pixels: vec![Color32::TRANSPARENT, Color32::WHITE],
        };
        let mut tex = Textures::default();
        tex.apply(&TexturesDelta {
            set: vec![(id, ImageDelta::full(image, TextureOptions::NEAREST))],
            free: vec![],
        });
        let mut img = Image::white(4, 1);
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(4.0, 1.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        img.draw(&[quad(r, Color32::BLACK, uv, id)], &tex, 1.0);
        assert_eq!(
            img.pixel(0, 0),
            [255, 255, 255],
            "transparent texel: untouched"
        );
        assert_eq!(
            img.pixel(3, 0),
            [0, 0, 0],
            "opaque texel: the corner colour"
        );
    }

    #[test]
    fn degenerate_and_out_of_range_input_does_nothing_and_does_not_panic() {
        let mut img = Image::white(4, 4);
        let flat = Rect::from_min_max(pos2(1.0, 1.0), pos2(1.0, 3.0));
        let off = Rect::from_min_max(pos2(-50.0, -50.0), pos2(-10.0, -10.0));
        let huge = Rect::from_min_max(pos2(-1e9, -1e9), pos2(1e9, 1e9));
        let nan = Rect::from_min_max(pos2(f32::NAN, 0.0), pos2(3.0, 3.0));
        img.draw(
            &[
                solid(flat, Color32::BLACK),
                solid(off, Color32::BLACK),
                solid(nan, Color32::BLACK),
                solid(huge, Color32::from_rgb(1, 2, 3)),
            ],
            &Textures::default(),
            1.0,
        );
        assert_eq!(img.pixel(2, 2), [1, 2, 3]);
        let mut bad = solid(flat, Color32::BLACK);
        if let Primitive::Mesh(m) = &mut bad.primitive {
            m.indices = vec![0, 1, 99];
        }
        img.draw(&[bad], &Textures::default(), 1.0);
    }
}
