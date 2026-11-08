//! Picture regions in the running window: choosing an image, refusing bad files,
//! drawing it on the page and keeping it through a save.

#![cfg(test)]

use opencalc_core::document::{Point, RegionKind, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::eval::evaluate_with;

use super::files::{setup, Rig};
use crate::app::filing::Dialog;
use crate::commands::CommandId;
use crate::export::{render_pages, Source};
use crate::panels::picture_window::selected_picture;
use crate::theme::{PaletteKey, Theme};

/// A solid-colour PNG file of the given size.
fn png_file(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, w, h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    let data: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
    e.write_header().unwrap().write_image_data(&data).unwrap();
    out
}

fn rig(name: &str) -> Rig {
    let mut t = setup(name, crate::app::AppState::new(PaletteKey::Neutral));
    let i = t.state().active.unwrap();
    let id = t.h.app.state.sheets[i].insert(RegionKind::picture());
    t.h.app.state.sheets[i].canvas.selection = vec![id];
    t.h.frame();
    t
}

fn message(t: &Rig) -> String {
    match &t.state().dialog {
        Some(Dialog::Message { title, text }) => format!("{title}: {text}"),
        other => panic!("expected a message, found {other:?}"),
    }
}

fn spec(t: &Rig) -> opencalc_core::document::PictureSpec {
    t.state().sheets[0]
        .doc
        .regions()
        .iter()
        .find_map(|r| match &r.kind {
            RegionKind::Picture(p) => Some((**p).clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn choosing_an_image_fills_the_picture_sizes_it_and_undoes_as_one_step() {
    let mut t = rig("pic_choose");
    assert!(selected_picture(t.state()).is_some());
    let file = t.dir.join("red.png");
    std::fs::write(&file, png_file(200, 100, [255, 0, 0])).unwrap();
    t.answer_image_open(Some(file));
    t.run(CommandId::PictureChoose);
    let p = spec(&t);
    assert_eq!(p.dimensions(), Some((200, 100)));
    assert_eq!(p.name, "red.png");
    let size = t.state().sheets[0].doc.regions()[0].size;
    assert_eq!((size.w, size.h), (200.0, 100.0));
    t.run(CommandId::EditUndo);
    assert!(spec(&t).is_empty(), "one undo takes the image out again");
}

#[test]
fn a_big_image_is_scaled_to_fit_the_page() {
    let mut t = rig("pic_big");
    let file = t.dir.join("big.png");
    std::fs::write(&file, png_file(2000, 1000, [0, 0, 255])).unwrap();
    t.answer_image_open(Some(file));
    t.run(CommandId::PictureChoose);
    let size = t.state().sheets[0].doc.regions()[0].size;
    assert_eq!((size.w, size.h), (480.0, 240.0));
}

#[test]
fn files_that_are_not_usable_images_change_nothing_and_say_why() {
    let mut t = rig("pic_bad");
    let before = spec(&t);
    let mut broken = png_file(8, 8, [1, 2, 3]);
    broken.truncate(broken.len() - 20);
    for (name, bytes, word) in [
        ("a.png", b"GIF89a not a png".to_vec(), "not a PNG"),
        ("b.png", broken, ""),
        (
            "c.png",
            {
                let mut v = png_file(2, 2, [0; 3]);
                v.resize(opencalc_core::document::MAX_PICTURE_BYTES + 1, 0);
                v
            },
            "MB",
        ),
    ] {
        let file = t.dir.join(name);
        std::fs::write(&file, bytes).unwrap();
        t.answer_image_open(Some(file));
        t.run(CommandId::PictureChoose);
        let m = message(&t);
        assert!(
            m.contains("Could not use this image") && m.contains(word),
            "{name}: {m}"
        );
        t.run(CommandId::DialogOk);
        assert_eq!(spec(&t), before, "{name}");
    }
    t.answer_image_open(None);
    t.run(CommandId::PictureChoose);
    assert!(t.state().dialog.is_none());
}

#[test]
fn the_image_is_drawn_on_the_page_and_in_the_export() {
    let mut doc = Worksheet::new("t");
    let r = doc.new_region(RegionKind::picture(), Point::new(40.0, 40.0));
    let id = r.id;
    doc.apply(Edit::InsertRegions {
        regions: vec![(0, r)],
    })
    .unwrap();
    let image =
        opencalc_core::document::PictureSpec::from_png("r.png", png_file(60, 40, [255, 0, 0]))
            .unwrap();
    doc.apply(Edit::Batch(vec![
        Edit::SetPicture {
            id,
            spec: Box::new(image),
        },
        Edit::SetRegionSize {
            id,
            size: opencalc_core::document::Size::new(120.0, 80.0),
        },
    ]))
    .unwrap();
    let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
    let theme = Theme::paper();
    let source = Source {
        doc: &doc,
        output: &output,
        theme: &theme,
    };
    let mut pages = Vec::new();
    render_pages(&source, 1.0, |_, image, _| {
        pages.push(image);
        Ok(())
    })
    .unwrap();
    let page = &pages[0];
    // The middle of the box is red; just outside it is paper.
    assert_eq!(page.pixel(100, 80), [255, 0, 0]);
    assert_eq!(page.pixel(20, 20), [255, 255, 255]);
    assert_eq!(page.pixel(100, 200), [255, 255, 255]);
}

#[test]
fn the_image_survives_saving_and_opening() {
    let mut t = rig("pic_save");
    let file = t.dir.join("g.png");
    let bytes = png_file(30, 30, [0, 200, 0]);
    std::fs::write(&file, &bytes).unwrap();
    t.answer_image_open(Some(file));
    t.run(CommandId::PictureChoose);
    let doc = t.state().sheets[0].doc.clone();
    let loaded =
        opencalc_core::persist::load_bytes(&opencalc_core::persist::save_bytes(&doc).unwrap())
            .unwrap();
    let RegionKind::Picture(p) = &loaded.worksheet.regions()[0].kind else {
        panic!("not a picture")
    };
    assert_eq!(p.png(), bytes.as_slice());
}
