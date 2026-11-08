//! Headers and footers: what is printed in the margins of the exported pages.

#![cfg(test)]

use opencalc_core::document::{HeaderFooter, PageText, Point, RegionKind, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::eval::evaluate_with;

use crate::export::raster::Image;
use crate::export::{render_pages, Source};
use crate::theme::Theme;

fn pages(doc: &Worksheet) -> Vec<Image> {
    let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
    let theme = Theme::paper();
    let source = Source {
        doc,
        output: &output,
        theme: &theme,
    };
    let mut out = Vec::new();
    render_pages(&source, 1.0, |_, i, _| {
        out.push(i);
        Ok(())
    })
    .unwrap();
    out
}

/// A worksheet that runs to two pages.
fn two_pages() -> Worksheet {
    let mut doc = Worksheet::new("Beam check");
    for (n, y) in [(0, 60.0), (1, 1300.0)] {
        let r = doc.new_region(RegionKind::math("a := 1"), Point::new(100.0, y));
        doc.apply(Edit::InsertRegions {
            regions: vec![(n, r)],
        })
        .unwrap();
    }
    doc
}

fn text(t: &str) -> PageText {
    PageText {
        centre: t.into(),
        ..PageText::default()
    }
}

/// Dark pixels in the top margin band of a page (the margin is 20 mm, 75 units).
fn header_ink(page: &Image) -> usize {
    let mut n = 0;
    for y in 10..60 {
        for x in 0..page.width {
            let [r, g, b] = page.pixel(x, y);
            if u32::from(r) + u32::from(g) + u32::from(b) < 500 {
                n += 1;
            }
        }
    }
    n
}

fn footer_ink(page: &Image) -> usize {
    let mut n = 0;
    for y in page.height - 60..page.height - 10 {
        for x in 0..page.width {
            let [r, g, b] = page.pixel(x, y);
            if u32::from(r) + u32::from(g) + u32::from(b) < 500 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn text_in_the_header_and_footer_is_printed_on_every_page() {
    let mut doc = two_pages();
    let blank = pages(&doc);
    assert_eq!(blank.len(), 2);
    assert_eq!((header_ink(&blank[0]), footer_ink(&blank[0])), (0, 0));
    doc.apply(Edit::SetHeaders(Box::new(HeaderFooter {
        header: text("{title}"),
        footer: PageText {
            right: "Page {page} of {pages}".into(),
            ..PageText::default()
        },
        ..HeaderFooter::default()
    })))
    .unwrap();
    let with = pages(&doc);
    for p in &with {
        assert!(header_ink(p) > 30, "header printed");
        assert!(footer_ink(p) > 30, "footer printed");
    }
}

#[test]
fn the_first_page_can_differ() {
    let mut doc = two_pages();
    doc.apply(Edit::SetHeaders(Box::new(HeaderFooter {
        header: text("later pages"),
        different_first: true,
        ..HeaderFooter::default()
    })))
    .unwrap();
    let p = pages(&doc);
    assert_eq!(
        header_ink(&p[0]),
        0,
        "the first page has no header of its own"
    );
    assert!(header_ink(&p[1]) > 30);
}

#[test]
fn fields_change_what_is_printed_page_to_page() {
    let mut doc = two_pages();
    doc.apply(Edit::SetHeaders(Box::new(HeaderFooter {
        footer: text("{page}"),
        ..HeaderFooter::default()
    })))
    .unwrap();
    let p = pages(&doc);
    let differs = (0..p[0].width).any(|x| {
        (p[0].height - 60..p[0].height - 10).any(|y| p[0].pixel(x, y) != p[1].pixel(x, y))
    });
    assert!(differs, "page 1 and page 2 print different numbers");
}

#[test]
fn a_page_with_a_tiny_margin_prints_no_header() {
    let mut doc = two_pages();
    doc.apply(Edit::SetPageSetup(opencalc_core::document::PageSetup {
        margin_mm: 4.0,
        ..Default::default()
    }))
    .unwrap();
    doc.apply(Edit::SetHeaders(Box::new(HeaderFooter {
        header: text("hidden"),
        ..HeaderFooter::default()
    })))
    .unwrap();
    assert_eq!(header_ink(&pages(&doc)[0]), 0);
}
