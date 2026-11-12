//! Exporting to PDF and PNG: the pictures, the files and the commands.

#![cfg(test)]

use std::path::PathBuf;

use opencalc_core::document::{PageSetup, PaperSize, Point, RegionKind};
use opencalc_core::edit::Edit;
use opencalc_core::eval::{evaluate_all, evaluate_with};

use super::files::{setup, Rig};
use crate::app::filing::Dialog;
use crate::commands::CommandId;
use crate::export::raster::Image;
use crate::export::{pdf_bytes, png_bytes, render_pages, Source};
use crate::sheet::OpenSheet;
use crate::theme::{PaletteKey, Theme};

fn pages(doc: &opencalc_core::document::Worksheet, scale: f32) -> Vec<Image> {
    let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
    let theme = Theme::palette(PaletteKey::Neutral);
    let source = Source {
        doc,
        output: &output,
        theme: &theme,
    };
    let mut out = Vec::new();
    render_pages(&source, scale, |_, image, _| {
        out.push(image);
        Ok(())
    })
    .unwrap();
    out
}

fn dark_in(image: &Image, x0: usize, y0: usize, x1: usize, y1: usize) -> usize {
    let mut n = 0;
    for y in y0..y1.min(image.height) {
        for x in x0..x1.min(image.width) {
            let [r, g, b] = image.pixel(x, y);
            if u32::from(r) + u32::from(g) + u32::from(b) < 300 {
                n += 1;
            }
        }
    }
    n
}

fn sheet_with(items: &[(&str, f32, f32)]) -> opencalc_core::document::Worksheet {
    let mut s = OpenSheet::new("t");
    for (src, x, y) in items {
        s.add(RegionKind::math(src), Point::new(*x, *y));
    }
    s.doc
}

#[test]
fn a_page_is_the_paper_size_at_the_resolution_and_white_where_nothing_is() {
    let doc = sheet_with(&[("F := 10 kN", 100.0, 100.0)]);
    let one = pages(&doc, 1.0);
    assert_eq!(one.len(), 1);
    // A4 is 793.7 x 1122.5 document units.
    assert_eq!((one[0].width, one[0].height), (794, 1123));
    let two = pages(&doc, 2.0);
    assert_eq!((two[0].width, two[0].height), (1587, 2245));
    let img = &one[0];
    assert_eq!(img.pixel(0, 0), [255, 255, 255]);
    assert_eq!(img.pixel(793, 1122), [255, 255, 255]);
    // Text is drawn where the region is, and nowhere near the far corner.
    assert!(dark_in(img, 100, 100, 300, 140) > 30, "the text is drawn");
    assert_eq!(dark_in(img, 500, 600, 794, 1123), 0);
    // No editing marks: no grid, no margin box, no page shadow.
    assert_eq!(dark_in(img, 0, 0, 90, 90), 0);
}

#[test]
fn the_page_setup_decides_the_size() {
    let mut doc = sheet_with(&[("a := 1", 40.0, 40.0)]);
    doc.apply(Edit::SetPageSetup(PageSetup {
        paper: PaperSize::Letter,
        landscape: true,
        margin_mm: 20.0,
        ..PageSetup::default()
    }))
    .unwrap();
    let p = pages(&doc, 1.0);
    assert_eq!((p[0].width, p[0].height), (1056, 816));
}

#[test]
fn content_below_the_first_page_goes_on_the_next_one_at_the_same_place() {
    // Page 2 starts at 1122.5 + 24 = 1146.5; this is 100 below its top.
    let doc = sheet_with(&[("a := 1", 100.0, 100.0), ("b := 2", 100.0, 1246.0)]);
    let p = pages(&doc, 1.0);
    assert_eq!(p.len(), 2);
    assert!(dark_in(&p[0], 100, 100, 300, 140) > 10);
    assert_eq!(
        dark_in(&p[0], 0, 400, 794, 1123),
        0,
        "nothing else on page 1"
    );
    assert!(
        dark_in(&p[1], 100, 95, 300, 140) > 10,
        "b at the same place on page 2"
    );
    assert_eq!(dark_in(&p[1], 0, 400, 794, 1123), 0);
}

#[test]
fn results_and_errors_are_printed_as_on_screen() {
    let doc = sheet_with(&[("x := 2 m", 100.0, 100.0), ("x =", 100.0, 200.0)]);
    let p = pages(&doc, 1.0);
    // The evaluation region draws its answer, so it is wider than "x =" alone.
    let answer = dark_in(&p[0], 140, 195, 300, 240);
    assert!(
        answer > 20,
        "the answer is drawn after the equals sign: {answer}"
    );
    // An error is drawn in the error colour.
    let bad = sheet_with(&[("y := 1 m + 2 s", 100.0, 100.0)]);
    let p = pages(&bad, 1.0);
    let red = (0..1123)
        .flat_map(|y| (0..794).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let [r, g, b] = p[0].pixel(x, y);
            r > 150 && g < 100 && b < 100
        })
        .count();
    assert!(red > 10, "the error underline and message are red: {red}");
}

#[test]
fn a_long_error_message_wraps_inside_the_margin_instead_of_running_off_the_page() {
    let doc = sheet_with(&[("q := 1 kN + 1 m / (1 s^2)", 420.0, 100.0)]);
    let p = pages(&doc, 1.0);
    let img = &p[0];
    // The right margin is at 794 - 76 = 718; a little slack for the last glyph.
    assert_eq!(
        dark_in(img, 730, 0, 794, 1123),
        0,
        "nothing past the margin"
    );
    // And it took more than one line to say it.
    let rows: Vec<usize> = (90..400)
        .filter(|y| dark_in(img, 420, *y, 720, *y + 1) > 0)
        .collect();
    let (first, last) = (rows[0], *rows.last().unwrap());
    assert!(
        last - first > 28,
        "the message spans several lines: {first}..{last}"
    );
}

#[test]
fn png_files_decode_to_the_same_pixels() {
    let doc = sheet_with(&[("F := 10 kN", 100.0, 100.0)]);
    let img = &pages(&doc, 1.0)[0];
    let bytes = png_bytes(img).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes))
        .read_info()
        .unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.width as usize, info.height as usize),
        (img.width, img.height)
    );
    assert_eq!(&buf[..info.buffer_size()], img.rgb().as_slice());
}

#[test]
fn a_pdf_has_a_page_for_each_page_at_the_paper_size_in_points() {
    let doc = sheet_with(&[("a := 1", 100.0, 100.0), ("b := 2", 100.0, 1246.0)]);
    let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
    let theme = Theme::palette(PaletteKey::Neutral);
    let source = Source {
        doc: &doc,
        output: &output,
        theme: &theme,
    };
    let pdf = pdf_bytes(&source, 0.5).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(text.contains("/Count 2"));
    assert!(
        text.contains("/MediaBox [0 0 595.28 841.89]"),
        "A4 in points"
    );
    assert!(text.contains("/Width 397 /Height 561"), "half resolution");
}

#[test]
fn a_worksheet_too_long_to_export_says_so() {
    let doc = sheet_with(&[("a := 1", 100.0, 300_000.0)]);
    let output = Default::default();
    let theme = Theme::palette(PaletteKey::Neutral);
    let source = Source {
        doc: &doc,
        output: &output,
        theme: &theme,
    };
    let err = render_pages(&source, 0.1, |_, _, _| Ok(())).unwrap_err();
    assert!(err.contains("at most 200"), "{err}");
}

// ---- through the app ----------------------------------------------------------

fn rig(name: &str) -> Rig {
    let mut t = setup(name, crate::app::AppState::new(PaletteKey::Neutral));
    t.h.app.export_shrink = 0.25;
    t.type_into_active("F := 10 kN");
    t
}

fn message(t: &Rig) -> String {
    match &t.state().dialog {
        Some(Dialog::Message { title, text }) => format!("{title}: {text}"),
        other => panic!("expected a message, found {other:?}"),
    }
}

#[test]
fn exporting_writes_the_file_names_it_and_leaves_the_worksheet_alone() {
    let mut t = rig("export_pdf");
    let revision = t.state().sheets[0].doc.revision();
    let target = t.dir.join("out.pdf");
    t.answer_export(Some(target.clone()));
    t.run(CommandId::FileExportPdf);
    let bytes = std::fs::read(&target).unwrap();
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert!(message(&t).contains("Exported"));
    assert_eq!(
        t.state().sheets[0].doc.revision(),
        revision,
        "no edit was made"
    );
    assert!(t.state().sheets[0].dirty(), "exporting does not save");

    // PNG, and a name without an extension gets one.
    t.run(CommandId::DialogOk);
    t.answer_export(Some(t.dir.join("pic")));
    t.run(CommandId::FileExportPng);
    let png = std::fs::read(t.dir.join("pic.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
}

#[test]
fn a_long_worksheet_exports_numbered_png_files() {
    let mut t = rig("export_pages");
    t.type_into_active("b := 2");
    // Move the second region onto the second page.
    let id = t.state().sheets[0].doc.regions()[1].id;
    t.h.app.state.sheets[0].edit(Edit::MoveRegions {
        moves: vec![(id, Point::new(100.0, 1300.0))],
    });
    t.answer_export(Some(t.dir.join("sheet.png")));
    t.run(CommandId::FileExportPng);
    assert!(t.dir.join("sheet-1.png").exists() && t.dir.join("sheet-2.png").exists());
    assert!(!t.dir.join("sheet.png").exists());
    assert!(message(&t).contains("2 page(s)"));
}

#[test]
fn a_cancelled_or_failed_export_changes_nothing_and_reports() {
    let mut t = rig("export_fail");
    t.answer_export(None);
    t.run(CommandId::FileExportPdf);
    assert!(t.state().dialog.is_none());

    t.answer_export(Some(t.dir.join("no_such_folder").join("x.pdf")));
    t.run(CommandId::FileExportPdf);
    let m = message(&t);
    assert!(m.contains("Could not export") && m.contains("x.pdf"), "{m}");
    assert_eq!(std::fs::read_dir(&t.dir).unwrap().count(), 0);
}

#[test]
fn the_export_commands_need_a_worksheet() {
    let mut t = rig("export_home");
    t.run(CommandId::TabHome);
    let r = t.h.app.registry();
    assert!(!r.allows(CommandId::FileExportPdf) && !r.allows(CommandId::FileExportPng));
    let _: Option<PathBuf> = None;
}

/// Writes the example worksheet as a PNG and a PDF to the folder in
/// `OPENCALC_EXPORT_DIR`, to look at by eye.
#[test]
#[ignore = "writes files for a human to look at"]
fn write_the_example_export() {
    let dir = std::env::var("OPENCALC_EXPORT_DIR").expect("OPENCALC_EXPORT_DIR");
    let sheet = OpenSheet::sample();
    let output = evaluate_all(
        &sheet.doc.math_regions(),
        &sheet.doc.plot_inputs(),
        &sheet.doc.units().unwrap(),
    );
    let theme = Theme::palette(PaletteKey::Neutral);
    let source = Source {
        doc: &sheet.doc,
        output: &output,
        theme: &theme,
    };
    render_pages(&source, 2.0, |k, image, _| {
        std::fs::write(
            format!("{dir}/example-{}.png", k + 1),
            png_bytes(&image).unwrap(),
        )
        .map_err(|e| e.to_string())
    })
    .unwrap();
    std::fs::write(
        format!("{dir}/example.pdf"),
        pdf_bytes(&source, 2.0).unwrap(),
    )
    .unwrap();
}

#[test]
fn the_text_of_the_worksheet_is_in_the_pdf_for_selecting_and_searching() {
    let doc = sheet_with(&[("F := 10 kN", 100.0, 100.0), ("x := 2 m", 100.0, 200.0)]);
    let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
    let theme = Theme::palette(PaletteKey::Neutral);
    let source = Source {
        doc: &doc,
        output: &output,
        theme: &theme,
    };
    let mut seen = Vec::new();
    render_pages(&source, 0.5, |_, _, text| {
        seen = text;
        Ok(())
    })
    .unwrap();
    let words: Vec<&str> = seen.iter().map(|r| r.text.as_str()).collect();
    let all = words.join(" ");
    for part in ["F", "10", "kN", "x", "2", "m"] {
        assert!(words.iter().any(|w| w.trim() == part), "{part} in {all}");
    }
    // Reading order: the first region before the second.
    let at = |needle: &str| words.iter().position(|w| w.trim() == needle).unwrap();
    assert!(at("kN") < at("m"), "{all}");
    for r in &seen {
        assert!(r.x >= 0.0 && r.baseline > 0.0 && r.baseline < 1123.0 && r.width > 0.0);
        assert!(r.text.chars().all(|c| !c.is_control()));
    }
    // And the file carries it.
    let pdf = pdf_bytes(&source, 0.5).unwrap();
    assert!(String::from_utf8_lossy(&pdf).contains(" Tj ET"));
}

#[test]
fn exporting_results_that_are_out_of_date_asks_first() {
    let mut t = rig("export_stale");
    t.h.app.state.live_recalc = false;
    t.type_into_active("b := 2");
    assert!(t.state().results_stale() && t.state().stale_notice());
    let target = t.dir.join("late.pdf");
    t.answer_export(Some(target.clone()));
    t.run(CommandId::FileExportPdf);
    assert!(
        matches!(t.state().dialog, Some(Dialog::StaleExport(_))),
        "asked, and wrote nothing"
    );
    assert!(!target.exists());

    t.run(CommandId::DialogCancel);
    assert!(t.state().dialog.is_none() && !target.exists());

    // Recalculating instead brings the numbers up to date; nothing is exported.
    t.run(CommandId::FileExportPdf);
    t.run(CommandId::DialogRecalculate);
    for _ in 0..40 {
        t.h.frame();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!t.state().results_stale() && !target.exists());

    // Exporting what is shown goes ahead.
    t.type_into_active("c := 3");
    t.answer_export(Some(target.clone()));
    t.run(CommandId::FileExportPdf);
    t.run(CommandId::DialogExportAnyway);
    assert!(std::fs::read(&target).unwrap().starts_with(b"%PDF-1.4"));
    assert!(message(&t).contains("Exported"));
}

#[test]
fn up_to_date_results_export_without_asking() {
    let mut t = rig("export_fresh");
    assert!(!t.state().stale_notice());
    t.answer_export(Some(t.dir.join("fresh.pdf")));
    t.run(CommandId::FileExportPdf);
    assert!(message(&t).contains("Exported"));
}
