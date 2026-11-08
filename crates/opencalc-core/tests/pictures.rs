//! Picture regions: editing, undo, saving with the image inside, and the limits.

use opencalc_core::document::{PictureSpec, Point, RegionId, RegionKind, Worksheet, MAX_PICTURES};
use opencalc_core::edit::{Edit, History};
use opencalc_core::persist::{load_bytes, save_bytes};

/// The start of a PNG file with the given size (core never decodes the rest).
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    v.extend(std::iter::repeat_n(7u8, 5000));
    v
}

fn add(w: &mut Worksheet) -> RegionId {
    let r = w.new_region(RegionKind::picture(), Point::default());
    let id = r.id;
    let at = w.regions().len();
    w.apply(Edit::InsertRegions {
        regions: vec![(at, r)],
    })
    .unwrap();
    id
}

#[test]
fn setting_a_picture_is_one_undoable_edit_that_never_recalculates() {
    let mut w = Worksheet::new("t");
    let id = add(&mut w);
    let mut h = History::default();
    let e = w.eval_revision();
    let spec = PictureSpec::from_png("beam.png", png(40, 20)).unwrap();
    h.apply(
        &mut w,
        Edit::SetPicture {
            id,
            spec: Box::new(spec.clone()),
        },
    )
    .unwrap();
    assert_eq!(w.eval_revision(), e);
    let RegionKind::Picture(p) = &w.region(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(p.dimensions(), Some((40, 20)));
    assert!(h.undo(&mut w).unwrap());
    let RegionKind::Picture(p) = &w.region(id).unwrap().kind else {
        panic!()
    };
    assert!(p.is_empty());
}

#[test]
fn the_image_is_saved_inside_the_file_byte_for_byte() {
    let mut w = Worksheet::new("t");
    let id = add(&mut w);
    let spec = PictureSpec::from_png("beam.png", png(40, 20))
        .unwrap()
        .with_alt("a beam");
    w.apply(Edit::SetPicture {
        id,
        spec: Box::new(spec.clone()),
    })
    .unwrap();
    let back = load_bytes(&save_bytes(&w).unwrap()).unwrap().worksheet;
    assert_eq!(back, w);
    let RegionKind::Picture(p) = &back.region(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(p.png(), spec.png());
    assert_eq!(p.alt, "a beam");
}

#[test]
fn too_many_pictures_is_refused_when_loading() {
    let mut w = Worksheet::new("t");
    for _ in 0..=MAX_PICTURES {
        let id = add(&mut w);
        w.apply(Edit::SetPicture {
            id,
            spec: Box::new(PictureSpec::from_png("a", png(2, 2)).unwrap()),
        })
        .unwrap();
    }
    // Saving refuses what loading would refuse.
    assert!(save_bytes(&w).is_err());
}

#[test]
fn a_locked_picture_refuses_a_new_image() {
    let mut w = Worksheet::new("t");
    let id = add(&mut w);
    w.apply(Edit::SetLocked { id, locked: true }).unwrap();
    assert!(w
        .apply(Edit::SetPicture {
            id,
            spec: Box::default()
        })
        .is_err());
}
