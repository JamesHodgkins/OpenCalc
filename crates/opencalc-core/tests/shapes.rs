//! Shape regions: editing, undo, draw order, saving, and repair of a bad file.

use opencalc_core::document::{
    Arrange, Colour, Dash, Point, RegionId, RegionKind, ShapeKind, ShapeSpec, Worksheet, MAX_STROKE,
};
use opencalc_core::edit::{Edit, History};
use opencalc_core::persist::{load_bytes, save_bytes};

fn add(w: &mut Worksheet, kind: RegionKind) -> RegionId {
    let r = w.new_region(kind, Point::default());
    let id = r.id;
    let at = w.regions().len();
    w.apply(Edit::InsertRegions {
        regions: vec![(at, r)],
    })
    .unwrap();
    id
}

#[test]
fn restyling_a_shape_is_one_undoable_edit_that_never_recalculates() {
    let mut w = Worksheet::new("t");
    let id = add(&mut w, RegionKind::shape(ShapeKind::Ellipse));
    let mut h = History::default();
    let eval = w.eval_revision();
    let spec = ShapeSpec {
        fill: None,
        stroke: Some(Colour::Rgba([10, 20, 30, 128])),
        dash: Dash::Dashed,
        ..ShapeSpec::of(ShapeKind::Ellipse)
    };
    h.apply(
        &mut w,
        Edit::SetShape {
            id,
            spec: Box::new(spec.clone()),
        },
    )
    .unwrap();
    assert_eq!(w.eval_revision(), eval);
    assert!(matches!(&w.region(id).unwrap().kind, RegionKind::Shape(s) if **s == spec));
    h.undo(&mut w).unwrap();
    assert!(
        matches!(&w.region(id).unwrap().kind, RegionKind::Shape(s) if **s == ShapeSpec::of(ShapeKind::Ellipse))
    );
}

#[test]
fn a_shape_edit_on_another_kind_of_region_is_refused() {
    let mut w = Worksheet::new("t");
    let id = add(
        &mut w,
        RegionKind::Text {
            text: String::new(),
        },
    );
    let spec = Box::new(ShapeSpec::default());
    assert!(w.apply(Edit::SetShape { id, spec }).is_err());
}

#[test]
fn a_bad_setting_is_repaired_on_the_edit_not_stored() {
    let mut w = Worksheet::new("t");
    let id = add(&mut w, RegionKind::shape(ShapeKind::Rectangle));
    let spec = ShapeSpec {
        stroke_width: 1.0e9,
        ..ShapeSpec::default()
    };
    w.apply(Edit::SetShape {
        id,
        spec: Box::new(spec),
    })
    .unwrap();
    let RegionKind::Shape(s) = &w.region(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(s.stroke_width, MAX_STROKE);
}

#[test]
fn shapes_and_draw_order_survive_saving() {
    let mut w = Worksheet::new("t");
    let a = add(&mut w, RegionKind::shape(ShapeKind::RoundedRectangle));
    let b = add(&mut w, RegionKind::math("x := 1"));
    let c = add(&mut w, RegionKind::shape(ShapeKind::Ellipse));
    let moves = w.arrange(&[a], Arrange::ToFront, &|_, _| true);
    w.apply(Edit::SetZ { moves }).unwrap();
    assert_eq!(w.paint_order(), [b, c, a]);
    let back = load_bytes(&save_bytes(&w).unwrap()).unwrap().worksheet;
    assert_eq!(back, w);
    assert_eq!(back.paint_order(), [b, c, a]);
}

#[test]
fn a_file_without_z_or_with_bad_shape_numbers_still_loads() {
    let mut w = Worksheet::new("t");
    add(&mut w, RegionKind::shape(ShapeKind::Rectangle));
    let json = serde_json::to_string(&w).unwrap();
    assert!(!json.contains("\"z\""), "z is left out when 0: {json}");
    let broken = json.replace("\"stroke_width\":1.5", "\"stroke_width\":-3.0");
    assert_ne!(json, broken);
    let read: Worksheet = serde_json::from_str(&broken).unwrap();
    assert_eq!(read.regions().len(), 1);
}

#[test]
fn dragging_a_setting_is_one_undo_step_until_it_is_sealed() {
    let mut w = Worksheet::new("t");
    let id = add(&mut w, RegionKind::shape(ShapeKind::Rectangle));
    let mut h = History::default();
    h.seal();
    for width in [2.0, 3.0, 4.0, 5.0] {
        let spec = ShapeSpec {
            stroke_width: width,
            ..ShapeSpec::default()
        };
        h.apply(
            &mut w,
            Edit::SetShape {
                id,
                spec: Box::new(spec),
            },
        )
        .unwrap();
    }
    h.undo(&mut w).unwrap();
    assert!(
        matches!(&w.region(id).unwrap().kind, RegionKind::Shape(s) if **s == ShapeSpec::default())
    );
}

#[test]
fn a_box_edit_moves_and_resizes_together_merges_while_dragging_and_undoes_once() {
    use opencalc_core::document::Size;
    let mut w = Worksheet::new("t");
    let id = add(&mut w, RegionKind::shape(ShapeKind::Rectangle));
    let start = (w.region(id).unwrap().pos, w.region(id).unwrap().size);
    let mut h = History::default();
    h.seal();
    for k in 1..=5 {
        let k = k as f32;
        h.apply(
            &mut w,
            Edit::SetRegionBox {
                id,
                pos: Point::new(-k * 3.0, 2.0),
                size: Size::new(100.0 + k * 10.0, 80.0),
            },
        )
        .unwrap();
    }
    assert_eq!(w.region(id).unwrap().size, Size::new(150.0, 80.0));
    h.undo(&mut w).unwrap();
    let r = w.region(id).unwrap();
    assert_eq!((r.pos, r.size), start, "the whole drag is one step");
}

#[test]
fn a_box_edit_keeps_the_size_usable_and_respects_the_lock() {
    use opencalc_core::document::Size;
    let mut w = Worksheet::new("t");
    let id = add(&mut w, RegionKind::shape(ShapeKind::Rectangle));
    w.apply(Edit::SetRegionBox {
        id,
        pos: Point::new(f32::NAN, 5.0),
        size: Size::new(f32::NAN, -4.0),
    })
    .unwrap();
    let r = w.region(id).unwrap();
    assert_eq!((r.pos.x, r.size.w, r.size.h), (0.0, 1.0, 1.0));
    w.apply(Edit::SetLocked { id, locked: true }).unwrap();
    assert!(w
        .apply(Edit::SetRegionBox {
            id,
            pos: Point::default(),
            size: Size::new(9.0, 9.0),
        })
        .is_err());
}
