//! Painting a picture region: the image, scaled to fit its box without
//! stretching, or a note when there is none or it cannot be read.

use std::collections::HashMap;

use eframe::egui::{
    Align2, Color32, ColorImage, FontId, Painter, Pos2, Rect, TextureHandle, TextureOptions, Ui,
    Vec2,
};
use opencalc_core::document::{PictureSpec, Region};

use super::adapt::vec;
use super::paint::Place;
use super::Env;
use crate::theme::type_size;

/// Most decoded pictures kept as textures at once.
const CACHE_LIMIT: usize = 24;

/// Decoded textures by image fingerprint (`None`: this image could not be read).
type Cache = HashMap<u64, Option<TextureHandle>>;

/// Decodes a PNG file into pixels. Errors say what is wrong in words.
pub fn decode(bytes: &[u8]) -> Result<ColorImage, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    decoder.set_limits(png::Limits {
        bytes: 128 * 1024 * 1024,
    });
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("the image is too large")?
    ];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<Color32> = match info.color_type {
        png::ColorType::Rgba => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect(),
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| Color32::from_rgb(p[0], p[1], p[2]))
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[0], p[0], p[1]))
            .collect(),
        png::ColorType::Grayscale => data.iter().map(|g| Color32::from_rgb(*g, *g, *g)).collect(),
        png::ColorType::Indexed => return Err("an indexed image was not expanded".into()),
    };
    if rgba.len() != w * h {
        return Err("the image data is the wrong length".into());
    }
    Ok(ColorImage {
        size: [w, h],
        pixels: rgba,
    })
}

/// The texture for `spec`, decoding it the first time it is seen.
fn texture(ui: &Ui, spec: &PictureSpec) -> Option<TextureHandle> {
    let id = eframe::egui::Id::new("picture-textures");
    let key = spec.content_id();
    if let Some(found) = ui
        .ctx()
        .data(|d| d.get_temp::<Cache>(id).and_then(|c| c.get(&key).cloned()))
    {
        return found;
    }
    let made = decode(spec.png()).ok().map(|image| {
        ui.ctx()
            .load_texture(format!("picture-{key}"), image, TextureOptions::LINEAR)
    });
    ui.ctx().data_mut(|d| {
        let cache = d.get_temp_mut_or_default::<Cache>(id);
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, made.clone());
    });
    made
}

/// Paints the picture in the region's box; returns the box size (a picture's
/// size is the one the user chose).
pub(super) fn paint(
    ui: &Ui,
    painter: &Painter,
    env: &Env,
    r: &Region,
    spec: &PictureSpec,
    place: Place,
) -> Vec2 {
    let Place {
        top_left,
        zoom,
        skip_content,
    } = place;
    if skip_content {
        return vec(r.size);
    }
    let theme = env.paper;
    let rect = Rect::from_min_size(top_left, vec(r.size) * zoom);
    let shown = if spec.is_empty() {
        None
    } else {
        texture(ui, spec)
    };
    match shown {
        Some(tex) => {
            let [w, h] = tex.size();
            let fit = (rect.width() / w as f32).min(rect.height() / h as f32);
            let size = Vec2::new(w as f32, h as f32) * fit;
            let at = Rect::from_center_size(rect.center(), size);
            painter.image(
                tex.id(),
                at,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => {
            super::paint::placeholder(painter, theme, r, top_left, zoom);
            let note = if spec.is_empty() {
                "no image: choose one in the Picture window"
            } else {
                "this image could not be read"
            };
            painter.text(
                rect.center() + Vec2::new(0.0, 48.0 * zoom),
                Align2::CENTER_CENTER,
                note,
                FontId::proportional(type_size::SMALL * zoom.max(0.6)),
                theme.text_weak,
            );
        }
    }
    vec(r.size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, width, height);
        e.set_color(color);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(data).unwrap();
        out
    }

    #[test]
    fn colour_grey_and_alpha_images_decode_to_the_right_pixels() {
        let rgb = encode(2, 1, png::ColorType::Rgb, &[255, 0, 0, 0, 0, 255]);
        let im = decode(&rgb).unwrap();
        assert_eq!(im.size, [2, 1]);
        assert_eq!(im.pixels[0], Color32::from_rgb(255, 0, 0));
        assert_eq!(im.pixels[1], Color32::from_rgb(0, 0, 255));
        let grey = encode(2, 1, png::ColorType::Grayscale, &[10, 200]);
        assert_eq!(
            decode(&grey).unwrap().pixels[1],
            Color32::from_rgb(200, 200, 200)
        );
        let rgba = encode(1, 1, png::ColorType::Rgba, &[255, 255, 255, 0]);
        assert_eq!(decode(&rgba).unwrap().pixels[0].a(), 0);
    }

    #[test]
    fn rubbish_is_an_error_not_a_panic() {
        assert!(decode(b"not a png").is_err());
        let mut cut = encode(4, 4, png::ColorType::Rgb, &[9; 48]);
        cut.truncate(cut.len() - 12);
        assert!(decode(&cut).is_err());
        assert!(decode(&[]).is_err());
    }
}
