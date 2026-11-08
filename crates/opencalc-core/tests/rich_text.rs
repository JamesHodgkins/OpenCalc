//! Styled stretches in text regions: they follow edits, undo exactly, and save.

use opencalc_core::document::{Point, RegionId, RegionKind, Run, RunStyle, Worksheet};
use opencalc_core::edit::{Edit, History};
use opencalc_core::persist::{load_bytes, save_bytes};

fn bold(len: u32) -> Run {
    Run {
        len,
        style: RunStyle {
            bold: true,
            ..RunStyle::default()
        },
    }
}

fn sheet(text: &str) -> (Worksheet, RegionId) {
    let mut w = Worksheet::new("t");
    let r = w.new_region(RegionKind::Text { text: text.into() }, Point::default());
    let id = r.id;
    w.apply(Edit::InsertRegions {
        regions: vec![(0, r)],
    })
    .unwrap();
    (w, id)
}

fn runs(w: &Worksheet, id: RegionId) -> Vec<Run> {
    w.region(id).unwrap().runs.clone()
}

#[test]
fn runs_are_set_cut_to_the_text_and_undone() {
    let (mut w, id) = sheet("hello world");
    let mut h = History::default();
    h.apply(
        &mut w,
        Edit::SetRuns {
            id,
            runs: vec![bold(5), bold(500)],
        },
    )
    .unwrap();
    assert_eq!(runs(&w, id), [bold(11)]);
    assert!(h.undo(&mut w).unwrap());
    assert!(runs(&w, id).is_empty());
}

#[test]
fn editing_the_text_keeps_the_styling_with_its_words_and_undoes_exactly() {
    let (mut w, id) = sheet("hello world");
    w.apply(Edit::SetRuns {
        id,
        runs: vec![bold(5)],
    })
    .unwrap();
    let mut h = History::default();
    // type before "world": the bold "hello" is untouched
    h.apply(
        &mut w,
        Edit::SetText {
            id,
            text: "hello big world".into(),
        },
    )
    .unwrap();
    assert_eq!(runs(&w, id), [bold(5)]);
    // delete the first three letters: the bold part shrinks
    h.seal();
    h.apply(
        &mut w,
        Edit::SetText {
            id,
            text: "lo big world".into(),
        },
    )
    .unwrap();
    assert_eq!(runs(&w, id), [bold(2)]);
    assert!(h.undo(&mut w).unwrap());
    assert_eq!(
        runs(&w, id),
        [bold(5)],
        "undo brings the styling back exactly"
    );
    assert!(h.redo(&mut w).unwrap());
    assert_eq!(runs(&w, id), [bold(2)]);
}

#[test]
fn find_and_replace_style_edits_do_not_misplace_styling() {
    let (mut w, id) = sheet("aa bb cc");
    w.apply(Edit::SetRuns {
        id,
        runs: vec![
            Run::default(),
            Run {
                len: 3,
                ..Run::default()
            },
        ],
    })
    .unwrap();
    w.apply(Edit::SetRuns {
        id,
        runs: vec![
            Run {
                len: 3,
                ..Run::default()
            },
            bold(2),
        ],
    })
    .unwrap();
    // "bb" is bold; replace "cc" at the end and the bold part stays on "bb"
    w.apply(Edit::SetText {
        id,
        text: "aa bb dddd".into(),
    })
    .unwrap();
    assert_eq!(
        runs(&w, id),
        [
            Run {
                len: 3,
                ..Run::default()
            },
            bold(2)
        ]
    );
}

#[test]
fn styling_is_saved_and_a_plain_text_region_adds_nothing() {
    let (mut w, id) = sheet("hello world");
    let plain = serde_json::to_string(w.region(id).unwrap()).unwrap();
    assert!(!plain.contains("runs"), "{plain}");
    w.apply(Edit::SetRuns {
        id,
        runs: vec![bold(5)],
    })
    .unwrap();
    let back = load_bytes(&save_bytes(&w).unwrap()).unwrap().worksheet;
    assert_eq!(runs(&back, id), [bold(5)]);
}

#[test]
fn styling_on_a_region_that_is_not_text_is_refused() {
    let mut w = Worksheet::new("t");
    let r = w.new_region(RegionKind::math("a := 1"), Point::default());
    let id = r.id;
    w.apply(Edit::InsertRegions {
        regions: vec![(0, r)],
    })
    .unwrap();
    assert!(w
        .apply(Edit::SetRuns {
            id,
            runs: vec![bold(1)]
        })
        .is_err());
}
