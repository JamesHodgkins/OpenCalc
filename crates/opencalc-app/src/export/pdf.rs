//! A small PDF writer: one page per picture, each page an image the size of the
//! page, with the page's text laid over it invisibly. The picture is what prints
//! and zooms; the text on top is what can be selected, copied and searched. (The
//! letters are not real glyphs in the file, so zooming in shows the picture's
//! pixels, as it always did.)

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use super::runs::TextRun;

/// One page: its size in points (1/72 inch), its pixels (already compressed, so a
/// long worksheet does not hold every page uncompressed at once) and its text.
pub struct PdfPage {
    width_pt: f32,
    height_pt: f32,
    px_width: usize,
    px_height: usize,
    deflated: Vec<u8>,
    text: Vec<TextRun>,
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
            text: Vec::new(),
        }
    }

    /// The text on the page, in page units (1/96 inch) from its top left.
    pub fn with_text(mut self, runs: Vec<TextRun>) -> PdfPage {
        self.text = runs;
        self
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

/// Document units (1/96 inch) to points.
const PT: f32 = 0.75;
/// Every code is drawn 0.5 em wide by the viewer, and the run is stretched to the
/// width it really had.
const NOMINAL_WIDTH: f32 = 0.5;

/// Which one-byte code stands for each character of the text. Printable ASCII is
/// its own code, so even a viewer that ignores the font's mapping reads it right;
/// any other character gets the next free code from 128 (a `ToUnicode` map says
/// what it is). Past 255 characters in all, the rest become `?`.
struct Codes {
    others: Vec<char>,
}

impl Codes {
    fn new(pages: &[PdfPage]) -> Codes {
        let mut others: Vec<char> = Vec::new();
        for c in pages
            .iter()
            .flat_map(|p| &p.text)
            .flat_map(|r| r.text.chars())
        {
            if !(' '..='~').contains(&c) && !others.contains(&c) && others.len() < 128 {
                others.push(c);
            }
        }
        Codes { others }
    }

    fn code(&self, c: char) -> u8 {
        if (' '..='~').contains(&c) {
            c as u8
        } else {
            self.others
                .iter()
                .position(|o| *o == c)
                .map_or(b'?', |i| 128 + i as u8)
        }
    }

    /// The `ToUnicode` CMap: what each code means.
    fn to_unicode(&self) -> String {
        let mut pairs: Vec<(u8, char)> = (b' '..=b'~').map(|b| (b, b as char)).collect();
        pairs.extend(
            self.others
                .iter()
                .enumerate()
                .map(|(i, c)| (128 + i as u8, *c)),
        );
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
             /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
             /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
             1 begincodespacerange\n<00> <FF>\nendcodespacerange\n",
        );
        for block in pairs.chunks(100) {
            cmap.push_str(&format!("{} beginbfchar\n", block.len()));
            for (code, c) in block {
                let mut units = [0u16; 2];
                let hex: String = c
                    .encode_utf16(&mut units)
                    .iter()
                    .map(|u| format!("{u:04X}"))
                    .collect();
                cmap.push_str(&format!("<{code:02X}> <{hex}>\n"));
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend");
        cmap
    }
}

/// The page's drawing: the picture, then each run as invisible text (render mode
/// 3) placed where it was painted and stretched to its painted width.
fn content(page: &PdfPage, codes: &Codes) -> String {
    let mut out = format!(
        "q {:.2} 0 0 {:.2} 0 0 cm /Im0 Do Q\n",
        page.width_pt, page.height_pt
    );
    for run in &page.text {
        let (size, chars) = (run.size * PT, run.text.chars().count());
        let placed = [run.x, run.baseline, run.width]
            .iter()
            .all(|v| v.is_finite());
        if chars == 0 || size <= 0.0 || !placed || run.width <= 0.0 {
            continue;
        }
        let stretch =
            (run.width * PT / (chars as f32 * NOMINAL_WIDTH * size) * 100.0).clamp(1.0, 1000.0);
        let hex: String = run
            .text
            .chars()
            .map(|c| format!("{:02X}", codes.code(c)))
            .collect();
        out.push_str(&format!(
            "BT 3 Tr /F1 {size:.2} Tf {stretch:.1} Tz {:.2} {:.2} Td <{hex}> Tj ET\n",
            run.x * PT,
            page.height_pt - run.baseline * PT,
        ));
    }
    out
}

/// The bytes of a PDF with `pages`, titled `title`.
pub fn write_pdf(title: &str, pages: &[PdfPage]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new(); // of objects 1, 2, 3, ...
    out.extend(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // Object numbers: 1 catalog, 2 page tree, 3 info, 4 the text font, 5 its
    // character map; then three per page.
    let first_page = |i: usize| 6 + 3 * i;
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
    // The font is never drawn (the text is invisible); it is here so a viewer has
    // the character widths and the map from codes to characters.
    let widths = vec!["500"; 224].join(" ");
    object(
        &mut out,
        format!(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding \
             /FirstChar 32 /LastChar 255 /Widths [{widths}] /ToUnicode 5 0 R >>"
        )
        .as_bytes(),
    );
    let codes = Codes::new(pages);
    let map = codes.to_unicode();
    object(
        &mut out,
        format!("<< /Length {} >>\nstream\n{map}\nendstream", map.len()).as_bytes(),
    );
    for (i, page) in pages.iter().enumerate() {
        let n = first_page(i);
        object(
            &mut out,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                 /Resources << /XObject << /Im0 {} 0 R >> /Font << /F1 4 0 R >> >> \
                 /Contents {} 0 R >>",
                page.width_pt,
                page.height_pt,
                n + 2,
                n + 1
            )
            .as_bytes(),
        );
        let content = content(page, &codes);
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

    fn run(x: f32, baseline: f32, text: &str) -> TextRun {
        TextRun {
            x,
            baseline,
            size: 16.0,
            width: 8.0 * text.chars().count() as f32,
            text: text.to_owned(),
        }
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
            pdf[xref..].starts_with(b"xref\n0 12\n"),
            "5 + 2 * 3 objects, and the free one"
        );
        let table = &pdf[xref + "xref\n0 12\n".len()..];
        for n in 1..12 {
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

    #[test]
    fn text_is_placed_invisibly_where_it_was_painted_and_stretched_to_its_width() {
        let p = page(2, 2, [255, 255, 255]).with_text(vec![run(96.0, 192.0, "ab")]);
        let codes = Codes::new(std::slice::from_ref(&p));
        let c = content(&p, &codes);
        // 96 units are 72 pt across; the baseline 192 units (144 pt) from the top.
        assert!(c.contains("BT 3 Tr /F1 12.00 Tf"), "{c}");
        assert!(c.contains("72.00 697.89 Td <6162> Tj ET"), "{c}");
        // Two letters 0.5 em each are 12 pt; they were 16 units (12 pt) wide: 100%.
        assert!(c.contains(" 100.0 Tz "), "{c}");
    }

    #[test]
    fn letters_outside_ascii_get_codes_and_the_map_says_what_they_are() {
        let p = page(2, 2, [0, 0, 0]).with_text(vec![run(0.0, 20.0, "\u{3c3} = 1\u{b2}")]);
        let codes = Codes::new(std::slice::from_ref(&p));
        assert_eq!(codes.code('A'), b'A');
        assert_eq!(codes.code('\u{3c3}'), 128);
        assert_eq!(codes.code('\u{b2}'), 129);
        assert_eq!(
            codes.code('\u{4e16}'),
            b'?',
            "not in the document: a question mark"
        );
        let map = codes.to_unicode();
        assert!(map.contains("<80> <03C3>") && map.contains("<81> <00B2>"));
        assert!(map.contains("<41> <0041>"));
        // The document carries it.
        let pdf = write_pdf("t", &[p]);
        assert!(find(&pdf, b"<80> <03C3>").is_some());
    }

    #[test]
    fn nothing_odd_in_a_run_can_break_the_file() {
        let mut bad = run(f32::NAN, 5.0, "x");
        bad.width = f32::INFINITY;
        let p = page(1, 1, [0, 0, 0]).with_text(vec![bad, run(1.0, 2.0, ""), run(1.0, 2.0, "ok")]);
        let pdf = write_pdf("t", &[p]);
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert!(find(&pdf, b"<6F6B> Tj").is_some(), "the good run is kept");
    }
}
