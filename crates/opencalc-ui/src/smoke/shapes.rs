//! Shapes: inserting, draw order through the commands, and the export.

#![cfg(test)]

use opencalc_core::document::{
    Colour, Point, RegionId, RegionKind, ShapeKind, ShapeSpec, Size, Worksheet,
};
use opencalc_core::edit::Edit;
use opencalc_core::eval::evaluate_with;

use super::Harness;
use crate::commands::CommandId;
use crate::export::{render_pages, Source};
use crate::theme::Theme;

fn three_shapes(h: &mut Harness) -> [RegionId; 3] {
    let s = &mut h.app.state.sheets[0];
    // Each overlaps the next, far below the example worksheet.
    let a = s.add(
        RegionKind::shape(ShapeKind::Rectangle),
        Point::new(60.0, 900.0),
    );
    let b = s.add(
        RegionKind::shape(ShapeKind::Ellipse),
        Point::new(120.0, 930.0),
    );
    let c = s.add(
        RegionKind::shape(ShapeKind::RoundedRectangle),
        Point::new(180.0, 960.0),
    );
    [a, b, c]
}

fn select(h: &mut Harness, ids: &[RegionId]) {
    h.app.state.sheets[0].canvas.selection = ids.to_vec();
    h.frame();
}

fn order(h: &Harness) -> Vec<RegionId> {
    h.sheet().doc.paint_order()
}

/// Press at `from`, drag to `to` in steps, release; the canvas origin is the viewport's.
fn drag_between(h: &mut Harness, from: eframe::egui::Pos2, to: eframe::egui::Pos2) {
    use eframe::egui::Event;
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![super::press(from)]);
    for k in 1..=8 {
        let at = from + (to - from) * (k as f32 / 8.0);
        h.frame_with(vec![Event::PointerMoved(at)]);
    }
    h.frame_with(vec![super::release(to)]);
    h.frame();
}

#[test]
fn a_shape_command_picks_up_the_tool_and_adds_nothing_until_you_draw() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    for (command, kind) in [
        (CommandId::InsertRectangle, ShapeKind::Rectangle),
        (
            CommandId::InsertRoundedRectangle,
            ShapeKind::RoundedRectangle,
        ),
        (CommandId::InsertEllipse, ShapeKind::Ellipse),
    ] {
        h.command(command);
        assert_eq!(h.sheet().canvas.pen, Some(kind));
        assert_eq!(h.sheet().doc.regions().len(), before);
    }
}

#[test]
fn dragging_with_the_tool_draws_a_shape_of_that_size_as_one_undo_step() {
    let mut h = Harness::new();
    h.settle();
    h.app.state.snap = false;
    let before = h.sheet().doc.regions().len();
    h.command(CommandId::InsertEllipse);
    let zoom = h.sheet().view.zoom;
    let from = h.app.state.viewport.center();
    let to = from + eframe::egui::vec2(160.0 * zoom, 100.0 * zoom);
    let doc = h.sheet().view.screen_to_doc(from, h.app.state.viewport.min);
    drag_between(&mut h, from, to);
    let sheet = h.sheet();
    assert_eq!(sheet.doc.regions().len(), before + 1);
    let r = sheet.doc.regions().last().unwrap();
    assert_eq!(r.kind.label(), "Ellipse");
    assert!(
        (r.pos.x - doc.x).abs() < 2.0 && (r.pos.y - doc.y).abs() < 2.0,
        "{:?}",
        r.pos
    );
    assert!(
        (r.size.w - 160.0).abs() < 2.0 && (r.size.h - 100.0).abs() < 2.0,
        "{:?}",
        r.size
    );
    assert_eq!(sheet.canvas.selection, vec![r.id]);
    assert_eq!(sheet.canvas.pen, None, "the tool is put away");
    h.command(CommandId::EditUndo);
    assert_eq!(h.sheet().doc.regions().len(), before, "one undo removes it");
}

#[test]
fn a_click_with_the_tool_makes_the_usual_size_and_escape_puts_the_tool_away() {
    use eframe::egui::{Event, Key, Modifiers};
    let mut h = Harness::new();
    h.settle();
    h.app.state.snap = false;
    let before = h.sheet().doc.regions().len();
    h.command(CommandId::InsertRectangle);
    let p = h.app.state.viewport.center();
    h.frame_with(vec![Event::PointerMoved(p)]);
    h.frame_with(vec![super::press(p)]);
    h.frame_with(vec![super::release(p)]);
    h.frame();
    let sheet = h.sheet();
    assert_eq!(sheet.doc.regions().len(), before + 1);
    let r = sheet.doc.regions().last().unwrap();
    assert_eq!(
        r.size,
        RegionKind::shape(ShapeKind::Rectangle).default_size()
    );

    h.command(CommandId::InsertEllipse);
    h.key(Key::Escape, Modifiers::NONE);
    assert_eq!(h.sheet().canvas.pen, None);
    let q = h.app.state.viewport.center() + eframe::egui::vec2(100.0, 60.0);
    drag_between(&mut h, q, q + eframe::egui::vec2(80.0, 50.0));
    assert_eq!(
        h.sheet().doc.regions().len(),
        before + 1,
        "nothing drawn after Escape"
    );
}

#[test]
fn bring_to_front_and_send_to_back_reorder_the_drawing_not_the_list() {
    let mut h = Harness::new();
    h.settle();
    let [a, b, c] = three_shapes(&mut h);
    let list: Vec<_> = h.sheet().doc.regions().iter().map(|r| r.id).collect();
    select(&mut h, &[a]);
    h.command(CommandId::ArrangeToFront);
    let o = order(&h);
    assert_eq!(*o.last().unwrap(), a);
    h.command(CommandId::ArrangeToBack);
    assert_eq!(order(&h)[0], a);
    let after: Vec<_> = h.sheet().doc.regions().iter().map(|r| r.id).collect();
    assert_eq!(after, list, "calculation order is untouched");
    h.command(CommandId::EditUndo);
    assert_eq!(*order(&h).last().unwrap(), a, "undo is one step");
    let _ = (b, c);
}

#[test]
fn forward_and_backward_step_past_one_overlapping_region() {
    let mut h = Harness::new();
    h.settle();
    let [a, b, c] = three_shapes(&mut h);
    select(&mut h, &[a]);
    h.command(CommandId::ArrangeForward);
    let o = order(&h);
    let at = |id| o.iter().position(|x| *x == id).unwrap();
    assert!(at(a) > at(b) && at(a) < at(c), "a stepped over b only");
    h.command(CommandId::ArrangeBackward);
    let o = order(&h);
    let at = |id| o.iter().position(|x| *x == id).unwrap();
    assert!(at(a) < at(b));
}

#[test]
fn the_draw_order_commands_need_a_selection_and_leave_locked_regions_alone() {
    let mut h = Harness::new();
    h.settle();
    let [a, b, _] = three_shapes(&mut h);
    select(&mut h, &[]);
    assert!(!h.app.registry().allows(CommandId::ArrangeToFront));
    h.app.state.sheets[0].edit(Edit::SetLocked {
        id: b,
        locked: true,
    });
    select(&mut h, &[b]);
    let before = order(&h);
    h.command(CommandId::ArrangeToFront);
    assert_eq!(
        order(&h),
        before,
        "a locked region does not move in the order"
    );
    select(&mut h, &[a]);
    h.command(CommandId::ArrangeToFront);
    assert_eq!(
        *order(&h).last().unwrap(),
        a,
        "others may pass a locked one"
    );
}

#[test]
fn a_key_press_brings_the_selection_to_the_front() {
    use eframe::egui::{Key, Modifiers};
    let mut h = Harness::new();
    h.settle();
    let [a, ..] = three_shapes(&mut h);
    select(&mut h, &[a]);
    h.key(
        Key::CloseBracket,
        Modifiers {
            shift: true,
            ..Modifiers::COMMAND
        },
    );
    assert_eq!(*order(&h).last().unwrap(), a);
}

#[test]
fn a_shape_is_drawn_in_the_export_and_the_one_in_front_wins() {
    let mut doc = Worksheet::new("t");
    let add = |doc: &mut Worksheet, colour: Colour, x: f32| {
        let mut r = doc.new_region(
            RegionKind::shape(ShapeKind::Rectangle),
            Point::new(x, 100.0),
        );
        r.size = Size::new(100.0, 100.0);
        let id = r.id;
        let at = doc.regions().len();
        doc.apply(Edit::InsertRegions {
            regions: vec![(at, r)],
        })
        .unwrap();
        let spec = ShapeSpec {
            fill: Some(colour),
            stroke: None,
            ..ShapeSpec::default()
        };
        doc.apply(Edit::SetShape {
            id,
            spec: Box::new(spec),
        })
        .unwrap();
        id
    };
    let red = Colour::rgb(255, 0, 0);
    let blue = Colour::rgb(0, 0, 255);
    let first = add(&mut doc, red, 100.0);
    let _second = add(&mut doc, blue, 150.0);
    let pixel_at_overlap = |doc: &Worksheet| {
        let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
        let theme = Theme::paper();
        let source = Source {
            doc,
            output: &output,
            theme: &theme,
        };
        let mut pages = Vec::new();
        render_pages(&source, 1.0, |_, i, _| {
            pages.push(i);
            Ok(())
        })
        .unwrap();
        let page = pages.remove(0);
        (
            page.pixel(125, 150),
            page.pixel(175, 150),
            page.pixel(300, 150),
        )
    };
    let (only_red, overlap, empty) = pixel_at_overlap(&doc);
    assert_eq!(only_red, [255, 0, 0], "the first rectangle is painted");
    assert_eq!(overlap, [0, 0, 255], "the later one lies on top");
    assert_ne!(empty, [255, 0, 0]);
    let moves = doc.arrange(
        &[first],
        opencalc_core::document::Arrange::ToFront,
        &|_, _| true,
    );
    doc.apply(Edit::SetZ { moves }).unwrap();
    let (_, overlap, _) = pixel_at_overlap(&doc);
    assert_eq!(overlap, [255, 0, 0], "brought to the front, red wins");
}

#[test]
fn selecting_a_shape_opens_its_window_and_other_regions_do_not() {
    use crate::panels::shape_controls::selected_shape;
    let mut h = Harness::new();
    h.settle();
    let [a, ..] = three_shapes(&mut h);
    select(&mut h, &[a]);
    assert!(selected_shape(&h.app.state).is_some());
    let math = h.sheet().doc.regions()[0].id;
    select(&mut h, &[math]);
    assert!(selected_shape(&h.app.state).is_none());
    select(&mut h, &[a, math]);
    assert!(selected_shape(&h.app.state).is_none(), "one at a time");
}

#[test]
fn restyling_through_the_edit_the_window_sends_is_undoable() {
    let mut h = Harness::new();
    h.settle();
    let [a, ..] = three_shapes(&mut h);
    let spec = ShapeSpec {
        fill: Some(Colour::Rgba([1, 2, 3, 128])),
        stroke: None,
        ..ShapeSpec::default()
    };
    h.app.state.sheets[0].seal();
    h.app.state.sheets[0].edit(Edit::SetShape {
        id: a,
        spec: Box::new(spec.clone()),
    });
    h.app.state.sheets[0].seal();
    let now = |h: &Harness| match &h.sheet().doc.region(a).unwrap().kind {
        RegionKind::Shape(s) => (**s).clone(),
        _ => panic!(),
    };
    assert_eq!(now(&h), spec);
    h.command(CommandId::EditUndo);
    assert_eq!(now(&h), ShapeSpec::of(ShapeKind::Rectangle));
}

/// The screen position of one of the selected region's handles.
fn handle_screen(
    h: &Harness,
    id: RegionId,
    handle: crate::canvas::resize::Handle,
) -> eframe::egui::Pos2 {
    let sheet = h.sheet();
    let r = sheet.doc.region(id).unwrap();
    let doc = handle.point(crate::canvas::adapt::rect_of(r, &sheet.canvas.sizes));
    sheet.view.doc_to_screen(doc, h.app.state.viewport.min)
}

/// Draw a rectangle in the middle of the view and leave it selected.
fn drawn_rectangle(h: &mut Harness) -> RegionId {
    h.app.state.snap = false;
    h.command(CommandId::InsertRectangle);
    let zoom = h.sheet().view.zoom;
    let from = h.app.state.viewport.center();
    drag_between(
        h,
        from,
        from + eframe::egui::vec2(200.0 * zoom, 100.0 * zoom),
    );
    h.sheet().doc.regions().last().unwrap().id
}

#[test]
fn dragging_a_handle_resizes_the_shape_as_one_undo_step() {
    use crate::canvas::resize::Handle;
    let mut h = Harness::new();
    h.settle();
    let id = drawn_rectangle(&mut h);
    let before = h.sheet().doc.region(id).unwrap().clone();
    let zoom = h.sheet().view.zoom;
    let from = handle_screen(&h, id, Handle::SE);
    drag_between(
        &mut h,
        from,
        from + eframe::egui::vec2(50.0 * zoom, 30.0 * zoom),
    );
    let r = h.sheet().doc.region(id).unwrap();
    assert_eq!(r.pos, before.pos, "the fixed corner stays");
    assert!(
        (r.size.w - (before.size.w + 50.0)).abs() < 2.0,
        "{:?}",
        r.size
    );
    assert!(
        (r.size.h - (before.size.h + 30.0)).abs() < 2.0,
        "{:?}",
        r.size
    );
    h.command(CommandId::EditUndo);
    let r = h.sheet().doc.region(id).unwrap();
    assert_eq!((r.pos, r.size), (before.pos, before.size));
}

#[test]
fn the_top_left_handle_moves_the_corner_and_a_locked_shape_has_no_handles() {
    use crate::canvas::resize::Handle;
    let mut h = Harness::new();
    h.settle();
    let id = drawn_rectangle(&mut h);
    let before = h.sheet().doc.region(id).unwrap().clone();
    let zoom = h.sheet().view.zoom;
    let from = handle_screen(&h, id, Handle::NW);
    drag_between(
        &mut h,
        from,
        from - eframe::egui::vec2(20.0 * zoom, 10.0 * zoom),
    );
    let r = h.sheet().doc.region(id).unwrap().clone();
    assert!((r.pos.x - (before.pos.x - 20.0)).abs() < 2.0);
    assert!((r.pos.y - (before.pos.y - 10.0)).abs() < 2.0);
    assert!((r.size.w - (before.size.w + 20.0)).abs() < 2.0);

    h.app.state.sheets[0].edit(Edit::SetLocked { id, locked: true });
    h.frame();
    let locked = h.sheet().doc.region(id).unwrap().clone();
    let from = handle_screen(&h, id, Handle::SE);
    drag_between(&mut h, from, from + eframe::egui::vec2(40.0, 40.0));
    let after = h.sheet().doc.region(id).unwrap();
    assert_eq!((after.pos, after.size), (locked.pos, locked.size));
}

#[test]
fn typeset_regions_have_no_handles() {
    let mut h = Harness::new();
    h.settle();
    let math = h.sheet().doc.regions()[0].id;
    select(&mut h, &[math]);
    let before = h.sheet().doc.region(math).unwrap().size;
    let sheet = h.sheet();
    let r = sheet.doc.region(math).unwrap();
    let corner = sheet.view.doc_to_screen(
        crate::canvas::adapt::rect_of(r, &sheet.canvas.sizes).max,
        h.app.state.viewport.min,
    );
    drag_between(&mut h, corner, corner + eframe::egui::vec2(60.0, 60.0));
    assert_eq!(h.sheet().doc.region(math).unwrap().size, before);
}

/// Draw from the middle of the view dragging 120 by 60 (document units) with the
/// modifiers held throughout, and return the new region.
fn draw_holding(h: &mut Harness, mods: eframe::egui::Modifiers) -> opencalc_core::document::Region {
    use eframe::egui::{vec2, Event, PointerButton};
    let zoom = h.sheet().view.zoom;
    let from = h.app.state.viewport.center();
    let to = from + vec2(120.0 * zoom, 60.0 * zoom);
    h.command(CommandId::InsertRectangle);
    h.held = mods;
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: mods,
    }]);
    for k in 1..=8 {
        let at = from + (to - from) * (k as f32 / 8.0);
        h.frame_with(vec![Event::PointerMoved(at)]);
    }
    h.frame_with(vec![Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: mods,
    }]);
    h.held = eframe::egui::Modifiers::NONE;
    h.frame();
    h.sheet().doc.regions().last().unwrap().clone()
}

#[test]
fn shift_draws_equal_sides() {
    let mut h = Harness::new();
    h.settle();
    h.app.state.snap = false;
    let r = draw_holding(&mut h, eframe::egui::Modifiers::SHIFT);
    assert!((r.size.w - r.size.h).abs() < 1.0, "{:?}", r.size);
    assert!(r.size.w > 100.0, "the longer side wins: {:?}", r.size);
}

#[test]
fn ctrl_draws_from_the_centre() {
    let mut h = Harness::new();
    h.settle();
    h.app.state.snap = false;
    let doc = h
        .sheet()
        .view
        .screen_to_doc(h.app.state.viewport.center(), h.app.state.viewport.min);
    let r = draw_holding(&mut h, eframe::egui::Modifiers::COMMAND);
    let c = (r.pos.x + r.size.w / 2.0, r.pos.y + r.size.h / 2.0);
    assert!(
        (c.0 - doc.x).abs() < 2.0 && (c.1 - doc.y).abs() < 2.0,
        "{c:?} vs {doc:?}"
    );
    assert!((r.size.w - 240.0).abs() < 3.0, "{:?}", r.size);
}

#[test]
fn the_status_bar_says_how_to_draw_only_while_the_tool_is_out() {
    let mut h = Harness::new();
    h.settle();
    assert!(h.sheet().canvas.pen.is_none());
    h.command(CommandId::InsertEllipse);
    assert!(h.sheet().canvas.pen.is_some());
}

fn shape_tab(h: &Harness) -> usize {
    h.app
        .ribbon
        .tabs
        .iter()
        .position(|t| t.context == Some(crate::ribbon::Context::Shape))
        .unwrap()
}

#[test]
fn drawing_a_shape_brings_up_its_tab_and_deselecting_takes_it_away() {
    let mut h = Harness::new();
    h.settle();
    let first = h.app.ribbon_state.active_tab;
    assert_eq!(first, 0);
    let _ = drawn_rectangle(&mut h);
    h.frame();
    h.frame();
    assert_eq!(
        h.app.ribbon_state.active_tab,
        shape_tab(&h),
        "the tab comes forward"
    );
    // Deselect: the tab is gone and the first tab is back.
    h.app.state.sheets[0].canvas.selection.clear();
    h.frame();
    h.frame();
    assert_eq!(h.app.ribbon_state.active_tab, 0);
    // Selecting the shape again shows the tab but does not jump to it.
    let id = h.sheet().doc.regions().last().unwrap().id;
    select(&mut h, &[id]);
    h.frame();
    assert_eq!(h.app.ribbon_state.active_tab, 0);
    h.app.ribbon_state.active_tab = shape_tab(&h);
    h.frame();
    assert_eq!(
        h.app.ribbon_state.active_tab,
        shape_tab(&h),
        "it can be picked"
    );
}

#[test]
fn the_shape_tab_is_the_same_height_as_every_other() {
    let mut h = Harness::new();
    h.settle();
    let id = drawn_rectangle(&mut h);
    let height = |h: &Harness| {
        eframe::egui::containers::panel::PanelState::load(&h.ctx, eframe::egui::Id::new("ribbon"))
            .expect("the ribbon panel")
            .rect
            .height()
    };
    h.app.ribbon_state.active_tab = 0;
    select(&mut h, &[id]);
    h.frame();
    let plain = height(&h);
    h.app.ribbon_state.active_tab = shape_tab(&h);
    h.frame();
    h.frame();
    assert!(
        (height(&h) - plain).abs() < 0.5,
        "{} vs {plain}",
        height(&h)
    );
}
