//! Exporting a worksheet as PDF or PNG.
//!
//! The pages are painted by the same code that paints the canvas, into an egui
//! context of their own at a high resolution, and drawn to an image by the
//! software renderer in `raster`. What is exported is therefore what is on screen
//! (without the grid, selection and other editing marks). A PDF page is that
//! picture with the page's text laid over it invisibly (`runs`, `pdf`), so the text
//! can be selected, copied and searched. The picture is what is seen and printed.

pub mod pdf;
pub mod raster;
pub mod runs;

use eframe::egui::{self, pos2, vec2, Context, Rect};
use opencalc_core::document::Worksheet;
use opencalc_core::eval::Output;

use crate::app::finding::Found;
use crate::canvas::adapt::{rect_of, Sizes};
use crate::canvas::{page, paint, Env};
use crate::theme::Theme;
use raster::{Image, Textures};
use runs::TextRun;

/// Most pages exported at once.
pub const MAX_PAGES: usize = 200;
/// Pixels per document unit (1/96 inch): 2 is 192 dpi, 3 is 288 dpi.
pub const PNG_SCALE: f32 = 2.0;
pub const PDF_SCALE: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pdf,
    Png,
}

impl Kind {
    pub fn extension(self) -> &'static str {
        match self {
            Kind::Pdf => "pdf",
            Kind::Png => "png",
        }
    }
}

/// What to draw: the worksheet, and its evaluated results as the screen shows them.
pub struct Source<'a> {
    pub doc: &'a Worksheet,
    pub output: &'a Output,
    pub theme: &'a Theme,
}

/// A rendering session: one egui context and the textures it has produced.
struct Session {
    ctx: Context,
    textures: Textures,
    scale: f32,
    time: f64,
}

impl Session {
    fn new(scale: f32) -> Session {
        let ctx = Context::default();
        crate::fonts::install(&ctx);
        Session {
            ctx,
            textures: Textures::default(),
            scale,
            time: 0.0,
        }
    }

    /// Run one frame over a screen of `size` points; `paint` draws into it.
    fn frame(
        &mut self,
        size: egui::Vec2,
        paint: impl FnOnce(&egui::Ui),
    ) -> (Vec<egui::ClippedPrimitive>, f32, Vec<TextRun>) {
        self.time += 1.0;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
            time: Some(self.time),
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale);
        let mut paint = Some(paint);
        let full = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    if let Some(paint) = paint.take() {
                        paint(ui);
                    }
                });
        });
        self.textures.apply(&full.textures_delta);
        let ppp = full.pixels_per_point;
        // The text is read off the shapes before they become triangles.
        let text = runs::collect(&full.shapes);
        (self.ctx.tessellate(full.shapes, ppp), ppp, text)
    }
}

/// Render every page of the worksheet, handing each to `each` as soon as it is
/// drawn. Returns the number of pages, or why it could not be done.
pub fn render_pages(
    source: &Source,
    scale: f32,
    mut each: impl FnMut(usize, Image, Vec<TextRun>) -> Result<(), String>,
) -> Result<usize, String> {
    let doc = source.doc;
    let setup = doc.page();
    let paper = Theme::paper();
    let found = Found::default();
    let env = Env {
        theme: source.theme,
        paper: &paper,
        page: setup,
        output: source.output,
        units: doc.unit_system(),
        show_grid: false,
        snap: false,
        text_box_focus: false,
        draft: false,
        found: &found,
    };
    let mut session = Session::new(scale);

    // First measure what each region needs, so we know where the content ends.
    let mut sizes = Sizes::new();
    session.frame(vec2(4000.0, 4000.0), |ui| {
        let nothing = ui.painter().with_clip_rect(Rect::NOTHING);
        for r in doc.regions() {
            let top_left = pos2(r.pos.x, r.pos.y);
            let size = paint::region(ui, &nothing, &env, r, top_left, 1.0, false);
            if r.kind.is_auto_sized() {
                sizes.insert(r.id, size);
            }
        }
    });
    let count = page::page_count(&setup, page::content_bottom(doc.regions(), &sizes));
    if count > MAX_PAGES {
        return Err(format!(
            "the worksheet runs to {count} pages; at most {MAX_PAGES} can be exported at once"
        ));
    }

    let (w, h) = setup.size();
    for k in 0..count {
        let page_rect = page::page_rect(&setup, k);
        let (primitives, ppp, text) = session.frame(vec2(w, h), |ui| {
            let painter = ui.painter().clone();
            let whole = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
            painter.rect_filled(whole, 0.0, paper.base);
            for r in doc.regions() {
                if !rect_of(r, &sizes).intersects(page_rect) {
                    continue;
                }
                let top_left = pos2(r.pos.x - page_rect.min.x, r.pos.y - page_rect.min.y);
                paint::region(ui, &painter, &env, r, top_left, 1.0, false);
            }
        });
        let mut image = Image::white((w * ppp).round() as usize, (h * ppp).round() as usize);
        image.draw(&primitives, &session.textures, ppp);
        each(k, image, text)?;
    }
    Ok(count)
}

/// An image as the bytes of a PNG file.
pub fn png_bytes(image: &Image) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width as u32, image.height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&image.rgb())
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// The whole worksheet as the bytes of a PDF file.
pub fn pdf_bytes(source: &Source, scale: f32) -> Result<Vec<u8>, String> {
    let (w, h) = source.doc.page().size();
    let mut pages = Vec::new();
    render_pages(source, scale, |_, image, text| {
        // The page is w x h document units of 1/96 inch; a point is 1/72 inch.
        pages.push(
            pdf::PdfPage::new(w * 0.75, h * 0.75, image.width, image.height, &image.rgb())
                .with_text(text),
        );
        Ok(())
    })?;
    Ok(pdf::write_pdf(source.doc.title(), &pages))
}
