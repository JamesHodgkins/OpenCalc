//! The examples in `docs/FILE_FORMAT.md` are real: each fenced block marked
//! `json worksheet` is loaded as a worksheet, and `json manifest` as a manifest.

mod common;

use std::io::Write;

use opencalc_core::persist::{load_bytes, save_bytes};

/// The contents of every fenced block whose info string is exactly `tag`.
fn blocks(doc: &str, tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in doc.lines() {
        match (&mut current, line.trim_end()) {
            (None, l) if l == format!("```{tag}") => current = Some(String::new()),
            (Some(_), "```") => out.push(current.take().unwrap()),
            (Some(text), l) => {
                text.push_str(l);
                text.push('\n');
            }
            _ => {}
        }
    }
    out
}

fn zip(manifest: &str, worksheet: &str) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    z.start_file("manifest.json", o).unwrap();
    z.write_all(manifest.as_bytes()).unwrap();
    z.start_file("worksheet.json", o).unwrap();
    z.write_all(worksheet.as_bytes()).unwrap();
    z.finish().unwrap().into_inner()
}

fn doc() -> String {
    std::fs::read_to_string(common::crate_path("../../docs/FILE_FORMAT.md")).unwrap()
}

#[test]
fn every_worksheet_example_loads_cleanly_and_saves_again() {
    let doc = doc();
    let manifest = blocks(&doc, "json manifest");
    assert_eq!(manifest.len(), 1);
    let examples = blocks(&doc, "json worksheet");
    assert!(examples.len() >= 2, "the document has its examples");
    for example in examples {
        let loaded = load_bytes(&zip(&manifest[0], &example)).expect(&example);
        assert!(
            loaded.repairs.is_empty(),
            "an example needed repairs: {:?}\n{example}",
            loaded.repairs
        );
        // What the document shows is what the program would write.
        let again = load_bytes(&save_bytes(&loaded.worksheet).unwrap()).unwrap();
        assert_eq!(again.worksheet, loaded.worksheet);
    }
}

#[test]
fn the_manifest_example_names_the_current_version() {
    let doc = doc();
    let manifest = &blocks(&doc, "json manifest")[0];
    let value: serde_json::Value = serde_json::from_str(manifest).unwrap();
    assert_eq!(value["format_version"], opencalc_core::FORMAT_VERSION);
}

#[test]
fn the_limits_table_matches_the_code() {
    use opencalc_core::persist::*;
    let doc = doc();
    for (text, value) in [
        ("| Regions | 50,000 |", MAX_REGIONS as u64),
        (
            "| Text in any one field | 65,536 characters |",
            MAX_TEXT_CHARS as u64,
        ),
        ("| Custom units | 1,000 |", MAX_CUSTOM_UNITS as u64),
        ("| Archive entries | 8 |", MAX_ENTRIES as u64),
        (
            "| File size | 128 MB |",
            MAX_FILE_BYTES as u64 / (1024 * 1024),
        ),
    ] {
        assert!(doc.contains(text), "{text}");
        let digits: String = text
            .trim_end_matches(" characters |")
            .trim_end_matches(" MB |")
            .trim_end_matches(" |")
            .rsplit(' ')
            .next()
            .unwrap()
            .replace(',', "");
        assert_eq!(digits.parse::<u64>().unwrap(), value, "{text}");
    }
}
