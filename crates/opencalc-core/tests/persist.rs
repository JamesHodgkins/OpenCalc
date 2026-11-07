//! Saving and loading: round trips, committed fixtures of every released format
//! version, and hostile or damaged input. Loading must never panic, hang or
//! allocate without bound, and must say why when it refuses.

mod common;

use std::io::Write;

use opencalc_core::document::{PageSetup, PaperSize, PlotSpec, Point, RegionKind, Size, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::persist::{
    load_bytes, save_bytes, LoadError, Repair, MAX_REGIONS, MAX_WORKSHEET_BYTES,
};
use opencalc_core::registry::UnitSystem;
use opencalc_core::units::{Dimension, UnitDef};
use zip::write::SimpleFileOptions;

fn add(ws: &mut Worksheet, kind: RegionKind, x: f32, y: f32) {
    let r = ws.new_region(kind, Point::new(x, y));
    let at = ws.regions().len();
    ws.apply(Edit::InsertRegions {
        regions: vec![(at, r)],
    })
    .unwrap();
}

/// A worksheet with one of everything, in non-default settings.
fn sample() -> Worksheet {
    let mut w = Worksheet::new("Beam stress \u{3c3} \"quoted\" \\ \n tab\t");
    add(&mut w, RegionKind::math("F := 10 kN"), 40.0, 20.5);
    add(
        &mut w,
        RegionKind::math("\u{3c3}_max := F / (0.04 m^2)"),
        40.0,
        60.0,
    );
    add(
        &mut w,
        RegionKind::Text {
            text: "Notes\nline two \u{1F600}".into(),
        },
        80.0,
        20.0,
    );
    add(
        &mut w,
        RegionKind::Area {
            title: "Loads".into(),
        },
        120.0,
        20.0,
    );
    let plot = PlotSpec::default()
        .with_title("t")
        .with_x("x")
        .with_series(0, "x^2")
        .with_series(1, "x");
    add(&mut w, RegionKind::Plot(plot), 160.0, 20.0);
    add(&mut w, RegionKind::Table, 200.0, 20.0);
    add(&mut w, RegionKind::Picture, 240.0, 20.0);
    w.apply(Edit::SetUnitSystem(UnitSystem::Uscs)).unwrap();
    w.apply(Edit::SetPageSetup(PageSetup {
        paper: PaperSize::Letter,
        landscape: true,
        margin_mm: 12.5,
    }))
    .unwrap();
    w.apply(Edit::SetCustomUnits(vec![UnitDef {
        symbol: "hand".into(),
        dimension: Dimension::LENGTH,
        to_si_scale: 0.1016,
        definition: String::new(),
    }]))
    .unwrap();
    w
}

fn round_trip(w: &Worksheet) -> Worksheet {
    let bytes = save_bytes(w).unwrap();
    let loaded = load_bytes(&bytes).unwrap();
    assert!(loaded.repairs.is_empty(), "{:?}", loaded.repairs);
    assert_eq!(&loaded.worksheet, w);
    assert_eq!(
        save_bytes(&loaded.worksheet).unwrap(),
        bytes,
        "a second save must be byte-identical"
    );
    loaded.worksheet
}

#[test]
fn every_region_type_and_setting_round_trips_exactly() {
    let w = sample();
    let mut back = round_trip(&w);
    // Region numbers keep counting, so none is ever reused.
    assert_eq!(back.new_region(RegionKind::Table, Point::default()).id.0, 8);
    assert!(round_trip(&Worksheet::new("")).regions().is_empty());
}

#[test]
fn a_units_definition_text_is_saved_and_a_file_without_it_still_loads() {
    let hand = opencalc_core::unit_define::define_unit("hand", "4 in", &[], None).unwrap();
    let mut w = Worksheet::new("u");
    w.apply(Edit::SetCustomUnits(vec![hand])).unwrap();
    let back = round_trip(&w);
    assert_eq!(back.custom_units()[0].definition, "4 in");
    // The committed format-1 fixture has a unit with no definition.
    assert!(sample().custom_units()[0].definition.is_empty());
}

#[test]
fn the_file_says_who_wrote_it_and_holds_no_ui_state() {
    let bytes = save_bytes(&sample()).unwrap();
    let loaded = load_bytes(&bytes).unwrap();
    assert!(
        loaded.saved_by.starts_with("opencalc "),
        "{}",
        loaded.saved_by
    );
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("manifest.json") && text.contains("worksheet.json"));
}

#[test]
fn corpus_sources_and_generated_worksheets_round_trip() {
    for (n, src) in common::known_sources().iter().enumerate() {
        let mut w = Worksheet::new("corpus");
        add(&mut w, RegionKind::math(src), n as f32, 0.0);
        round_trip(&w);
    }
    for seed in 0..200u64 {
        round_trip(&generated(seed));
    }
}

/// A worksheet of random kinds, positions and awkward text.
fn generated(seed: u64) -> Worksheet {
    let mut rng = common::Rng::new(seed);
    let words = [
        "a",
        "\u{3b1}",
        "\"",
        "\\",
        "\n",
        "{}",
        "\u{1F600}",
        " ",
        "x_1",
        "0.1",
        "\u{0}",
    ];
    let text = |rng: &mut common::Rng| -> String {
        (0..rng.below(8)).map(|_| *rng.pick(&words)).collect()
    };
    let mut w = Worksheet::new(&text(&mut rng));
    for i in 0..rng.below(30) {
        let kind = match rng.below(6) {
            0 => RegionKind::math(&text(&mut rng)),
            1 => RegionKind::Text {
                text: text(&mut rng),
            },
            2 => RegionKind::Area {
                title: text(&mut rng),
            },
            3 => RegionKind::Plot(PlotSpec::default().with_title(&text(&mut rng)).with_x("t")),
            4 => RegionKind::Table,
            _ => RegionKind::Picture,
        };
        let (x, y) = (rng.below(2000) as f32 / 8.0 - 50.0, i as f32 * 3.7);
        add(&mut w, kind, x, y);
    }
    w
}

// ---- committed fixtures -------------------------------------------------------

fn fixture(name: &str) -> std::path::PathBuf {
    common::crate_path(&format!("tests/fixtures/{name}"))
}

/// Run once with `--ignored` to (re)write the fixtures of the current format. Old
/// versions' files are never regenerated: they are the proof old files still open.
#[test]
#[ignore = "writes files; run by hand when the format version changes"]
fn write_fixtures() {
    let dir = fixture("v1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("empty.ocx"),
        save_bytes(&Worksheet::new("Empty")).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.join("sample.ocx"), save_bytes(&sample()).unwrap()).unwrap();
}

#[test]
fn files_written_by_format_1_still_open() {
    let empty = load_bytes(&std::fs::read(fixture("v1/empty.ocx")).unwrap()).unwrap();
    assert_eq!(empty.worksheet, Worksheet::new("Empty"));
    let sheet = load_bytes(&std::fs::read(fixture("v1/sample.ocx")).unwrap()).unwrap();
    assert!(sheet.repairs.is_empty());
    assert_eq!(sheet.worksheet, sample());
}

// ---- hostile and damaged input ------------------------------------------------

fn zip_of(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, data) in parts {
        z.start_file(*name, SimpleFileOptions::default()).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap().into_inner()
}

const MANIFEST: &[u8] = br#"{"format_version":1,"app":"test"}"#;

fn load_json(json: &str) -> Result<opencalc_core::persist::Loaded, LoadError> {
    load_bytes(&zip_of(&[
        ("manifest.json", MANIFEST),
        ("worksheet.json", json.as_bytes()),
    ]))
}

fn region(id: u64, kind: &str) -> String {
    format!(r#"{{"id":{id},"pos":{{"x":1,"y":2}},"size":{{"w":3,"h":4}},"kind":{kind}}}"#)
}

fn sheet(regions: &[String], next_id: u64) -> String {
    format!(
        r#"{{"title":"t","regions":[{}],"next_id":{next_id}}}"#,
        regions.join(",")
    )
}

#[test]
fn a_minimal_hand_written_worksheet_loads_with_defaults() {
    let json = sheet(&[region(1, r#"{"Math":{"source":"1+1"}}"#)], 2);
    let l = load_json(&json).unwrap();
    assert!(l.repairs.is_empty());
    assert_eq!(l.worksheet.regions().len(), 1);
    assert_eq!(l.worksheet.unit_system(), UnitSystem::Si);
    assert_eq!(l.worksheet.page(), PageSetup::default());
}

#[test]
fn unknown_entries_and_fields_are_ignored() {
    let json = r#"{"title":"t","regions":[],"next_id":1,"from_the_future":[1,2]}"#;
    let bytes = zip_of(&[
        ("manifest.json", MANIFEST),
        ("assets/pic.png", b"\x89PNG"),
        ("worksheet.json", json.as_bytes()),
    ]);
    assert!(load_bytes(&bytes).is_ok());
}

#[test]
fn a_newer_format_is_refused_by_name() {
    let bytes = zip_of(&[
        ("manifest.json", br#"{"format_version":2}"#),
        ("worksheet.json", b"{}"),
    ]);
    let err = load_bytes(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            LoadError::TooNew {
                found: 2,
                supported: 1
            }
        ),
        "{err}"
    );
    assert!(err.to_string().contains("newer OpenCalc"));
    let zero = zip_of(&[
        ("manifest.json", br#"{"format_version":0}"#),
        ("worksheet.json", b"{}"),
    ]);
    assert!(matches!(load_bytes(&zero), Err(LoadError::BadManifest(_))));
}

#[test]
fn things_that_are_not_worksheets_say_so() {
    for bytes in [&b""[..], b"hello", b"PK\x03\x04", &[0u8; 100][..]] {
        assert!(matches!(load_bytes(bytes), Err(LoadError::NotAnArchive(_))));
    }
    let only_manifest = zip_of(&[("manifest.json", MANIFEST)]);
    assert!(matches!(
        load_bytes(&only_manifest),
        Err(LoadError::Missing("worksheet.json"))
    ));
    let no_manifest = zip_of(&[("worksheet.json", b"{}")]);
    assert!(matches!(
        load_bytes(&no_manifest),
        Err(LoadError::Missing("manifest.json"))
    ));
    let bad = zip_of(&[("manifest.json", b"not json"), ("worksheet.json", b"{}")]);
    assert!(matches!(load_bytes(&bad), Err(LoadError::BadManifest(_))));
    assert!(matches!(load_json("[1,2"), Err(LoadError::BadWorksheet(_))));
    assert!(matches!(
        load_json(r#"{"title":"t"}"#),
        Err(LoadError::BadWorksheet(_))
    ));
}

#[test]
fn unsafe_entry_names_and_too_many_entries_are_refused() {
    for name in ["../evil", "/abs", "a/../../b", "C:/x"] {
        let bytes = zip_of(&[
            ("manifest.json", MANIFEST),
            ("worksheet.json", b"{}"),
            (name, b"x"),
        ]);
        assert!(
            matches!(load_bytes(&bytes), Err(LoadError::UnsafePath(_))),
            "{name}"
        );
    }
    let mut parts: Vec<(String, &[u8])> = vec![
        ("manifest.json".into(), MANIFEST),
        ("worksheet.json".into(), b"{}"),
    ];
    for i in 0..8 {
        parts.push((format!("assets/{i}"), b"x"));
    }
    let refs: Vec<(&str, &[u8])> = parts.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    assert!(matches!(
        load_bytes(&zip_of(&refs)),
        Err(LoadError::TooMany {
            what: "archive entries",
            ..
        })
    ));
}

#[test]
fn a_zip_bomb_is_refused_without_inflating_it() {
    // 70 MB of spaces deflates to about 70 KB.
    let bomb = vec![b' '; MAX_WORKSHEET_BYTES as usize + 1024 * 1024];
    let bytes = zip_of(&[("manifest.json", MANIFEST), ("worksheet.json", &bomb)]);
    assert!(
        bytes.len() < 1024 * 1024,
        "the bomb is small on disk: {}",
        bytes.len()
    );
    assert!(matches!(
        load_bytes(&bytes),
        Err(LoadError::TooLarge {
            what: "worksheet.json",
            ..
        })
    ));
    let big_manifest = vec![b' '; 70_000];
    let bytes = zip_of(&[("manifest.json", &big_manifest), ("worksheet.json", b"{}")]);
    assert!(matches!(
        load_bytes(&bytes),
        Err(LoadError::TooLarge {
            what: "manifest.json",
            ..
        })
    ));
}

#[test]
fn deeply_nested_json_is_an_error_not_a_stack_overflow() {
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(matches!(load_json(&deep), Err(LoadError::BadWorksheet(_))));
}

#[test]
fn bad_custom_units_name_the_unit() {
    let unit = |symbol: &str, scale: &str| {
        format!(
            r#"{{"title":"t","regions":[],"next_id":1,"custom_units":[{{"symbol":"{symbol}","dimension":{},"to_si_scale":{scale}}}]}}"#,
            serde_json::to_string(&Dimension::LENGTH).unwrap()
        )
    };
    assert!(load_json(&unit("hand", "0.1")).is_ok());
    for (symbol, scale) in [
        ("m", "1"),
        ("sin", "1"),
        ("2x", "1"),
        ("hand", "0"),
        ("hand", "-1"),
    ] {
        let err = load_json(&unit(symbol, scale)).unwrap_err();
        assert!(
            matches!(err, LoadError::Units(_)),
            "{symbol} {scale}: {err}"
        );
        assert!(err.to_string().contains(symbol), "{err}");
    }
}

#[test]
fn duplicate_region_numbers_are_refused() {
    let m = r#"{"Math":{"source":"1"}}"#;
    let err = load_json(&sheet(&[region(3, m), region(3, m)], 4)).unwrap_err();
    assert!(
        matches!(err, LoadError::DuplicateId(id) if id.0 == 3),
        "{err}"
    );
    let err = load_json(&sheet(&[region(u64::MAX, m)], 1)).unwrap_err();
    assert!(matches!(err, LoadError::IdOverflow), "{err}");
}

#[test]
fn a_next_number_that_is_too_low_is_repaired_and_reported() {
    let m = r#"{"Math":{"source":"1"}}"#;
    let l = load_json(&sheet(&[region(5, m), region(9, m)], 2)).unwrap();
    assert_eq!(l.repairs, [Repair::NextIdRaised { from: 2, to: 10 }]);
    let mut ws = l.worksheet;
    assert_eq!(ws.new_region(RegionKind::Table, Point::default()).id.0, 10);
    assert!(l.repairs[0].to_string().contains("raised to 10"));
}

#[test]
fn unusable_numbers_never_reach_layout() {
    // 1e39 is finite as a double and infinite as a single.
    let json = r#"{"title":"t","next_id":3,"regions":[
        {"id":1,"pos":{"x":1e39,"y":-1e39},"size":{"w":0,"h":-5},"kind":"Table"},
        {"id":2,"pos":{"x":9e9,"y":1},"size":{"w":1e30,"h":4},"kind":"Table"}]}"#;
    let l = load_json(json).unwrap();
    for r in l.worksheet.regions() {
        assert!(r.pos.x.is_finite() && r.pos.y.is_finite());
        assert!(r.pos.x.abs() <= opencalc_core::persist::MAX_COORD);
        assert!(r.size.w > 0.0 && r.size.w <= opencalc_core::persist::MAX_EXTENT);
        assert!(r.size.h > 0.0 && r.size.h <= opencalc_core::persist::MAX_EXTENT);
    }
    assert_eq!(l.worksheet.regions()[0].pos, Point::new(0.0, 0.0));
    assert_eq!(l.worksheet.regions()[0].size, Size::new(260.0, 170.0));
    assert_eq!(l.repairs.len(), 4, "{:?}", l.repairs);
}

#[test]
fn the_page_margin_is_clamped_and_reported() {
    let json = r#"{"title":"t","regions":[],"next_id":1,"page":{"margin_mm":9999}}"#;
    let l = load_json(json).unwrap();
    assert_eq!(l.worksheet.page().margin_mm, 60.0);
    assert_eq!(l.repairs, [Repair::MarginFixed]);
}

#[test]
fn limits_on_counts_and_lengths_refuse_the_file() {
    let m = r#"{"Math":{"source":"1"}}"#;
    let many: Vec<String> = (1..=MAX_REGIONS as u64 + 1).map(|i| region(i, m)).collect();
    let err = load_json(&sheet(&many, MAX_REGIONS as u64 + 2)).unwrap_err();
    assert!(
        matches!(
            err,
            LoadError::TooMany {
                what: "regions",
                ..
            }
        ),
        "{err}"
    );
    let ok: Vec<String> = (1..=MAX_REGIONS as u64).map(|i| region(i, m)).collect();
    assert!(load_json(&sheet(&ok, MAX_REGIONS as u64 + 1)).is_ok());

    let long = "x".repeat(opencalc_core::persist::MAX_TEXT_CHARS + 1);
    let text = format!(r#"{{"Text":{{"text":"{long}"}}}}"#);
    let err = load_json(&sheet(&[region(1, &text)], 2)).unwrap_err();
    assert!(matches!(err, LoadError::TooLong { .. }), "{err}");
    let math = format!(r#"{{"Math":{{"source":"{long}"}}}}"#);
    assert!(matches!(
        load_json(&sheet(&[region(1, &math)], 2)),
        Err(LoadError::TooLong { .. })
    ));
}

#[test]
fn plots_are_bounded_and_always_have_a_series_to_type_in() {
    let series = |n: usize| {
        let one = r#"{"source":"x"}"#;
        let list = vec![one; n].join(",");
        format!(r#"{{"Plot":{{"x":{one},"series":[{list}]}}}}"#)
    };
    let err = load_json(&sheet(&[region(1, &series(9))], 2)).unwrap_err();
    assert!(
        matches!(
            err,
            LoadError::TooMany {
                what: "series in a plot",
                ..
            }
        ),
        "{err}"
    );
    assert!(load_json(&sheet(&[region(1, &series(8))], 2)).is_ok());
    let l = load_json(&sheet(&[region(1, &series(0))], 2)).unwrap();
    assert_eq!(l.repairs.len(), 1);
    match &l.worksheet.regions()[0].kind {
        RegionKind::Plot(p) => assert_eq!(p.series.len(), 1),
        other => panic!("{other:?}"),
    }
}

#[test]
fn hostile_math_source_is_an_error_expression_not_a_hang() {
    let src = "(".repeat(opencalc_core::persist::MAX_TEXT_CHARS);
    let math = format!(r#"{{"Math":{{"source":"{src}"}}}}"#);
    let l = load_json(&sheet(&[region(1, &math)], 2)).unwrap();
    assert_eq!(l.worksheet.regions().len(), 1);
}

#[test]
fn a_worksheet_that_would_not_load_cannot_be_saved() {
    let mut w = sample();
    w.apply(Edit::MoveRegions {
        moves: vec![(w.regions()[0].id, Point::new(f32::NAN, 0.0))],
    })
    .unwrap();
    assert!(
        save_bytes(&w).is_err(),
        "NaN position must not reach a file"
    );
}

// ---- damage -------------------------------------------------------------------

/// What a damaged file may do: refuse, or load something that is itself valid.
fn assert_sane(bytes: &[u8]) {
    if let Ok(loaded) = load_bytes(bytes) {
        let again = save_bytes(&loaded.worksheet).expect("a loaded worksheet can be saved");
        assert_eq!(load_bytes(&again).unwrap().worksheet, loaded.worksheet);
    }
}

#[test]
fn a_file_cut_short_at_any_length_never_panics_and_never_loads() {
    let bytes = save_bytes(&sample()).unwrap();
    for len in 0..bytes.len() {
        assert!(
            load_bytes(&bytes[..len]).is_err(),
            "cut at {len} of {}",
            bytes.len()
        );
    }
}

#[test]
fn flipping_any_byte_never_panics_and_never_gives_a_broken_worksheet() {
    let bytes = save_bytes(&sample()).unwrap();
    for i in 0..bytes.len() {
        for flip in [0xFFu8, 0x01] {
            let mut damaged = bytes.clone();
            damaged[i] ^= flip;
            assert_sane(&damaged);
        }
    }
}

#[test]
fn random_garbage_never_panics() {
    let mut rng = common::Rng::new(7);
    for _ in 0..300 {
        let len = rng.below(400);
        let junk: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
        assert_sane(&junk);
        // Garbage that starts like a zip.
        let mut like_zip = b"PK\x03\x04".to_vec();
        like_zip.extend(junk);
        assert_sane(&like_zip);
    }
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn any_generated_worksheet_round_trips_byte_identically(seed in any::<u64>()) {
            round_trip(&generated(seed));
        }

        #[test]
        fn any_bytes_load_or_refuse_without_panicking(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
            assert_sane(&bytes);
        }
    }
}
