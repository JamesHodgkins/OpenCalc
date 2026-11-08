//! Rich text in the running window: styling marked characters, the whole text,
//! colours, and what the page shows.

#![cfg(test)]

use opencalc_core::document::{Point, RegionId, RegionKind, Run, RunStyle, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::eval::evaluate_with;

use super::Harness;
use crate::commands::CommandId;
use crate::export::raster::Image;
use crate::export::{render_pages, Source};
use crate::theme::Theme;

fn with_text(h: &mut Harness, text: &str) -> RegionId {
    let i = h.app.state.active.unwrap();
    let id = h.app.state.sheets[i].insert(RegionKind::Text { text: text.into() });
    h.app.state.sheets[i].canvas.selection = vec![id];
    // Not being typed in: a caret in the open editor would clear the marked range.
    h.app.state.sheets[i].canvas.editing = None;
    h.frame();
    id
}

fn runs(h: &Harness, id: RegionId) -> Vec<Run> {
    h.sheet().doc.region(id).unwrap().runs.clone()
}

fn style(bold: bool, underline: bool, colour: Option<u8>) -> RunStyle {
    RunStyle {
        bold,
        underline,
        colour,
        ..RunStyle::default()
    }
}

#[test]
fn bold_on_marked_characters_styles_only_those_and_toggles_back() {
    let mut h = Harness::new();
    h.settle();
    let id = with_text(&mut h, "hello world");
    h.app.state.sheets[0].canvas.text_sel = Some((id, 6, 11));
    h.command(CommandId::FormatBold);
    assert_eq!(
        runs(&h, id),
        [
            Run {
                len: 6,
                ..Run::default()
            },
            Run {
                len: 5,
                style: style(true, false, None)
            }
        ]
    );
    assert!(
        !h.sheet().doc.region(id).unwrap().style.bold,
        "the region itself is not bold"
    );
    assert!(h
        .app
        .state
        .active_toggles()
        .contains(&CommandId::FormatBold));
    h.command(CommandId::FormatBold);
    assert!(runs(&h, id).is_empty());
    h.command(CommandId::EditUndo);
    assert_eq!(runs(&h, id).len(), 2, "one undo step per click");
}

#[test]
fn with_nothing_marked_bold_still_styles_the_whole_region_as_before() {
    let mut h = Harness::new();
    h.settle();
    let id = with_text(&mut h, "hello");
    h.command(CommandId::FormatBold);
    assert!(h.sheet().doc.region(id).unwrap().style.bold);
    assert!(runs(&h, id).is_empty());
}

#[test]
fn underline_without_a_mark_underlines_all_the_text_and_a_stale_mark_is_ignored() {
    let mut h = Harness::new();
    h.settle();
    let id = with_text(&mut h, "abc");
    h.command(CommandId::FormatUnderline);
    assert_eq!(
        runs(&h, id),
        [Run {
            len: 3,
            style: style(false, true, None)
        }]
    );
    // a mark past the end of the text (the text got shorter) is not used
    h.app.state.sheets[0].canvas.text_sel = Some((id, 5, 9));
    h.command(CommandId::FormatUnderline);
    assert!(runs(&h, id).is_empty(), "toggled off over the whole text");
}

#[test]
fn locked_text_cannot_be_restyled() {
    let mut h = Harness::new();
    h.settle();
    let id = with_text(&mut h, "abc");
    h.app.state.sheets[0].edit(Edit::SetLocked { id, locked: true });
    h.command(CommandId::FormatUnderline);
    assert!(runs(&h, id).is_empty());
}

fn page_with(runs: Vec<Run>, text: &str) -> Image {
    let mut doc = Worksheet::new("t");
    let r = doc.new_region(
        RegionKind::Text { text: text.into() },
        Point::new(40.0, 40.0),
    );
    let id = r.id;
    doc.apply(Edit::InsertRegions {
        regions: vec![(0, r)],
    })
    .unwrap();
    doc.apply(Edit::SetRuns { id, runs }).unwrap();
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

fn count(page: &Image, f: impl Fn([u8; 3]) -> bool) -> usize {
    let mut n = 0;
    for y in 30..120 {
        for x in 30..400 {
            if f(page.pixel(x, y)) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn colour_underline_and_weight_show_on_the_page() {
    let text = "Colour and weight";
    let plain = page_with(vec![], text);
    let red = |p: [u8; 3]| p[0] > 150 && p[1] < 90 && p[2] < 90;
    assert_eq!(count(&plain, red), 0);
    let coloured = page_with(
        vec![Run {
            len: 6,
            style: style(false, false, Some(0)),
        }],
        text,
    );
    assert!(count(&coloured, red) > 20, "red letters");
    let dark = |p: [u8; 3]| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]) < 300;
    let underlined = page_with(
        vec![Run {
            len: 17,
            style: style(false, true, None),
        }],
        text,
    );
    let inked = |p: [u8; 3]| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]) < 700;
    assert!(
        count(&underlined, inked) > count(&plain, inked) + 40,
        "a line under the text"
    );
    let bold = page_with(
        vec![Run {
            len: 17,
            style: style(true, false, None),
        }],
        text,
    );
    assert!(
        count(&bold, dark) > count(&plain, dark) + 50,
        "heavier letters"
    );
}

#[test]
fn italic_is_a_real_face_and_differs_from_regular_and_slanted_regular() {
    let width = |bold: bool, italic: bool| {
        let ctx = eframe::egui::Context::default();
        ctx.set_fonts(crate::fonts::definitions());
        let _ = ctx.run(Default::default(), |_| {});
        ctx.fonts(|f| {
            f.layout_job(crate::canvas::text_style::job_runs(
                "Institutional wiggle",
                opencalc_core::document::FontStyle { bold, italic },
                &[],
                20.0,
                eframe::egui::Color32::BLACK,
                2000.0,
            ))
            .size()
            .x
        })
    };
    let (regular, italic, bold_italic, bold) = (
        width(false, false),
        width(false, true),
        width(true, true),
        width(true, false),
    );
    assert!(
        (regular - italic).abs() > 0.5,
        "italic has its own widths: {regular} {italic}"
    );
    assert!(bold_italic > italic && bold_italic != bold);
}

#[test]
fn a_styled_region_opens_in_the_editor_and_closes_without_losing_its_styling() {
    let mut h = Harness::new();
    h.settle();
    let id = with_text(&mut h, "hello world");
    h.command(CommandId::FormatUnderline);
    let before = runs(&h, id);
    assert!(!before.is_empty());
    h.app.state.sheets[0].canvas.editing = Some(id);
    h.app.state.sheets[0].canvas.focus_edit = true;
    for _ in 0..4 {
        h.frame();
    }
    // typing at the end keeps the underline on what was there
    h.app.state.sheets[0].edit(Edit::SetText {
        id,
        text: "hello world!".into(),
    });
    assert!(runs(&h, id)[0].style.underline);
    h.app.state.sheets[0].canvas.editing = None;
    h.frame();
}
