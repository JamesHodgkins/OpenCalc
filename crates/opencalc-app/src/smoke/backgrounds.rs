//! Page backgrounds: the pattern and paper colour, and that exports leave them
//! out unless asked.

#![cfg(test)]

use opencalc_core::document::{Background, PageSetup, Tint, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::eval::evaluate_with;

use crate::export::raster::Image;
use crate::export::{render_pages, Source};
use crate::theme::Theme;

fn page(setup: PageSetup) -> Image {
    let mut doc = Worksheet::new("t");
    doc.apply(Edit::SetPageSetup(setup)).unwrap();
    let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
    let theme = Theme::paper();
    let source = Source {
        doc: &doc,
        output: &output,
        theme: &theme,
    };
    let mut out = Vec::new();
    render_pages(&source, 1.0, |_, i, _| {
        out.push(i);
        Ok(())
    })
    .unwrap();
    out.remove(0)
}

fn not_white(p: &Image) -> usize {
    let mut n = 0;
    for y in 0..p.height {
        for x in 0..p.width {
            if p.pixel(x, y) != [255, 255, 255] {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn exports_are_plain_white_unless_the_background_is_included() {
    let styled = PageSetup {
        background: Background::Grid,
        tint: Tint::Cream,
        ..PageSetup::default()
    };
    assert_eq!(not_white(&page(styled)), 0, "left out by default");
    let printed = page(PageSetup {
        print_background: true,
        ..styled
    });
    // Between the lines the paper is cream; on a line it is the pattern colour.
    assert_eq!(printed.pixel(30, 30), [255, 250, 235]);
    assert!(not_white(&printed) > 100_000);
}

#[test]
fn each_pattern_draws_something_and_differently() {
    let with = |background| {
        page(PageSetup {
            background,
            print_background: true,
            ..PageSetup::default()
        })
    };
    let plain = not_white(&with(Background::Plain));
    let lines = not_white(&with(Background::Lines));
    let grid = not_white(&with(Background::Grid));
    let dots = not_white(&with(Background::Dots));
    assert_eq!(plain, 0);
    assert!(
        lines > 500 && grid > lines && dots > 100 && dots < lines,
        "{lines} {grid} {dots}"
    );
}

#[test]
fn a_tint_colours_the_whole_page() {
    let p = page(PageSetup {
        tint: Tint::Blue,
        print_background: true,
        ..PageSetup::default()
    });
    assert_eq!(p.pixel(400, 500), [236, 244, 255]);
}
