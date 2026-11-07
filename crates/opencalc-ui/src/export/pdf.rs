//! A small PDF writer: one page per picture, each page a single image the size
//! of the page. The text is therefore not selectable, but it prints and zooms
//! like any other page at the resolution it was drawn.

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

/// One page: its size in points (1/72 inch) and its pixels, already compressed
/// so that a long worksheet does not hold every page uncompressed at once.
pub struct PdfPage {
    width_pt: f32,
    height_pt: f32,
    px_width: usize,
    px_height: usize,
    deflated: Vec<u8>,
}

impl PdfPage {
    /// `rgb` is 8 bits a channel, row by row from the top.
    pub fn new(
        width_pt: f32,
        height_pt: f32,
        px_width: usize,
        px_height: usize,
        rgb: &[u8],
    ) -> PdfPage {
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        // Writing to memory cannot fail.
        let _ = z.write_all(rgb);
        PdfPage {
            width_pt,
            height_pt,
            px_width,
            px_height,
            deflated: z.finish().unwrap_or_default(),
        }
    }
}

/// A text string for the PDF info dictionary: UTF-16 with a byte order mark.
fn hex_string(text: &str) -> String {
    let mut out = String::from("<FEFF");
    for unit in text.encode_utf16() {
        out.push_str(&format!("{unit:04X}"));
    }
    out.push('>');
    out
}

/// The bytes of a PDF with `pages`, titled `title`.
pub fn write_pdf(title: &str, pages: &[PdfPage]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new(); // of objects 1, 2, 3, ...
    out.extend(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // Object numbers: 1 catalog, 2 page tree, 3 info; then three per page.
    let first_page = |i: usize| 4 + 3 * i;
    let mut object = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        let n = offsets.len();
        out.extend(format!("{n} 0 obj\n").as_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    };

    object(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", first_page(i)))
        .collect();
    object(
        &mut out,
        format!(
            "<< /Type /Pages /Count {} /Kids [{}] >>",
            pages.len(),
            kids.join(" ")
        )
        .as_bytes(),
    );
    object(
        &mut out,
        format!(
            "<< /Title {} /Producer {} >>",
            hex_string(title),
            hex_string(&format!("OpenCalc {}", opencalc_core::version()))
        )
        .as_bytes(),
    );
    for (i, page) in pages.iter().enumerate() {
        let n = first_page(i);
        object(
            &mut out,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                 /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>",
                page.width_pt,
                page.height_pt,
                n + 2,
                n + 1
            )
            .as_bytes(),
        );
        let content = format!(
            "q {:.2} 0 0 {:.2} 0 0 cm /Im0 Do Q",
            page.width_pt, page.height_pt
        );
        object(
            &mut out,
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            )
            .as_bytes(),
        );
        let data = &page.deflated;
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} \
             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
            page.px_width,
            page.px_height,
            data.len()
        )
        .into_bytes();
        body.extend(data);
        body.extend(b"\nendstream");
        object(&mut out, &body);
    }

    let xref = out.len();
    out.extend(format!("xref\n0 {}\n", offsets.len() + 1).as_bytes());
    out.extend(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(w: usize, h: usize, rgb: [u8; 3]) -> PdfPage {
        let pixels: Vec<u8> = rgb.iter().copied().cycle().take(w * h * 3).collect();
        PdfPage::new(595.28, 841.89, w, h, &pixels)
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn the_file_has_the_right_shape_and_every_xref_entry_points_at_its_object() {
        let pdf = write_pdf(
            "Beam \u{3c3}",
            &[page(4, 3, [255, 0, 0]), page(2, 2, [0, 0, 0])],
        );
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert!(find(&pdf, b"/Count 2").is_some());
        // The cross-reference table: each offset is where "N 0 obj" begins.
        let start = find(&pdf, b"startxref\n").unwrap() + "startxref\n".len();
        let end = pdf[start..].iter().position(|b| *b == b'\n').unwrap();
        let xref: usize = std::str::from_utf8(&pdf[start..start + end])
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            pdf[xref..].starts_with(b"xref\n0 10\n"),
            "3 + 2 * 3 objects, and the free one"
        );
        let table = &pdf[xref + "xref\n0 10\n".len()..];
        for n in 1..10 {
            let line = &table[n * 20..n * 20 + 20];
            let offset: usize = std::str::from_utf8(&line[..10]).unwrap().parse().unwrap();
            let want = format!("{n} 0 obj\n");
            assert!(pdf[offset..].starts_with(want.as_bytes()), "object {n}");
        }
    }

    #[test]
    fn the_image_inflates_back_to_the_pixels() {
        use std::io::Read;
        let pdf = write_pdf("t", &[page(5, 4, [10, 20, 30])]);
        let at = find(&pdf, b"/FlateDecode").unwrap();
        let stream = at + find(&pdf[at..], b"stream\n").unwrap() + "stream\n".len();
        let len_at = find(&pdf[at..], b"/Length ").unwrap() + at + "/Length ".len();
        let len: usize = std::str::from_utf8(&pdf[len_at..len_at + 12])
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(&pdf[stream..stream + len])
            .read_to_end(&mut raw)
            .unwrap();
        assert_eq!(raw.len(), 5 * 4 * 3);
        assert_eq!(&raw[..3], &[10, 20, 30]);
    }

    #[test]
    fn the_title_is_utf16_and_no_pages_is_still_a_valid_file() {
        assert_eq!(hex_string("A\u{3c3}"), "<FEFF004103C3>");
        let pdf = write_pdf("t", &[]);
        assert!(find(&pdf, b"/Count 0").is_some());
        assert!(pdf.ends_with(b"%%EOF\n"));
    }
}
