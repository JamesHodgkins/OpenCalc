//! Header and footer text: edits, undo, saving, and that no result depends on it.

use opencalc_core::document::{HeaderFooter, PageText, Worksheet};
use opencalc_core::edit::{Edit, History};
use opencalc_core::persist::{load_bytes, save_bytes};

fn setup() -> HeaderFooter {
    HeaderFooter {
        header: PageText {
            left: "{title}".into(),
            ..PageText::default()
        },
        footer: PageText {
            right: "{page} / {pages}".into(),
            ..PageText::default()
        },
        different_first: true,
        first_header: PageText {
            centre: "Cover".into(),
            ..PageText::default()
        },
        ..HeaderFooter::default()
    }
}

#[test]
fn typing_merges_into_one_undo_step_and_changes_no_result() {
    let mut w = Worksheet::new("t");
    let mut h = History::default();
    let e = w.eval_revision();
    for text in ["A", "AB", "ABC"] {
        h.apply(
            &mut w,
            Edit::SetHeaders(Box::new(HeaderFooter {
                header: PageText {
                    centre: text.into(),
                    ..PageText::default()
                },
                ..HeaderFooter::default()
            })),
        )
        .unwrap();
    }
    assert_eq!(w.headers().header.centre, "ABC");
    assert_eq!(w.eval_revision(), e);
    assert!(h.undo(&mut w).unwrap());
    assert!(
        w.headers().is_default(),
        "all three keystrokes undo together"
    );
}

#[test]
fn headers_are_saved_and_a_blank_setup_adds_nothing_to_the_file() {
    let mut w = Worksheet::new("t");
    let plain = save_bytes(&w).unwrap();
    w.apply(Edit::SetHeaders(Box::new(setup()))).unwrap();
    let back = load_bytes(&save_bytes(&w).unwrap()).unwrap().worksheet;
    assert_eq!(back.headers(), &setup());
    w.apply(Edit::SetHeaders(Box::default())).unwrap();
    assert_eq!(save_bytes(&w).unwrap(), plain);
}

#[test]
fn long_or_odd_text_is_trimmed_when_set() {
    let mut w = Worksheet::new("t");
    w.apply(Edit::SetHeaders(Box::new(HeaderFooter {
        header: PageText {
            left: format!("a\nb{}", "x".repeat(1000)),
            ..PageText::default()
        },
        ..HeaderFooter::default()
    })))
    .unwrap();
    assert!(w.headers().header.left.chars().count() <= 120);
    assert!(!w.headers().header.left.contains('\n'));
}
