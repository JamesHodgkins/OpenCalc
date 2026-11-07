//! The OpenCalc badge icon (`assets/logos/badge_icon_dark.svg`, rasterised to
//! 256 px). Used as the window icon and as the logo on the Home and About pages.
//! The dark variant is used because every window palette is dark.

use eframe::egui::{self, Context, TextureHandle, TextureOptions};

const PNG: &[u8] = include_bytes!("../../../assets/logos/badge_icon_dark_256.png");

/// The icon as window-icon data.
pub fn icon_data() -> egui::IconData {
    eframe::icon_data::from_png_bytes(PNG).expect("the embedded logo is a valid PNG")
}

/// The logo as a texture, loaded once per context.
pub fn texture(ctx: &Context) -> TextureHandle {
    let id = egui::Id::new("opencalc-logo");
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return t;
    }
    let icon = icon_data();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    let t = ctx.load_texture("opencalc-logo", image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

/// The logo as an image `size` points square.
pub fn image(ctx: &Context, size: f32) -> egui::Image<'static> {
    egui::Image::new(&texture(ctx)).fit_to_exact_size(egui::vec2(size, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_logo_decodes() {
        let icon = icon_data();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}
