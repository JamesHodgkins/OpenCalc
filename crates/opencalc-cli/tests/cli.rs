//! `opencalc-cli check`, run as a real process on real files.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use opencalc_core::document::{Point, RegionKind, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::persist::save_bytes;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let p = std::env::temp_dir().join(format!("opencalc-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn sheet(&self, name: &str, sources: &[&str]) -> PathBuf {
        let mut w = Worksheet::new("t");
        for (i, s) in sources.iter().enumerate() {
            let r = w.new_region(
                RegionKind::math(s),
                Point::new(10.0, 20.0 + i as f32 * 40.0),
            );
            let at = w.regions().len();
            w.apply(Edit::InsertRegions {
                regions: vec![(at, r)],
            })
            .unwrap();
        }
        let p = self.path(name);
        std::fs::write(&p, save_bytes(&w).unwrap()).unwrap();
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&Path], flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_opencalc-cli"))
        .arg("check")
        .args(flags)
        .args(args)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_clean_worksheet_exits_0_and_says_how_much_it_checked() {
    let d = Dir::new("clean");
    let f = d.sheet("ok.ocalc", &["a := 1 m", "b := a * 2"]);
    let out = run(&[&f], &[]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("2 regions, 0 error(s), 0 warning(s)"));
    assert!(out.stderr.is_empty());
    // Quiet prints nothing when there is nothing to say.
    let out = run(&[&f], &["--quiet"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

#[test]
fn an_error_exits_1_and_points_at_the_region_and_the_failing_part() {
    let d = Dir::new("error");
    let f = d.sheet("bad.ocalc", &["ok := 1", "q := 1 m + 2 s"]);
    let out = run(&[&f], &["--quiet"]);
    assert_eq!(out.status.code(), Some(1));
    let s = text(&out.stdout);
    let lines: Vec<&str> = s.lines().collect();
    assert!(
        lines[0].contains("bad.ocalc: region 2 (10, 60): error:"),
        "{s}"
    );
    assert_eq!(lines[1], "    q := 1 m + 2 s");
    // The sum is characters 5 to 14: nine carets, under it.
    assert_eq!(lines[2], format!("    {}{}", " ".repeat(5), "^".repeat(9)));
}

#[test]
fn a_warning_alone_still_exits_1() {
    let d = Dir::new("warn");
    let f = d.sheet("w.ocalc", &["a := 1", "a := 2"]);
    let out = run(&[&f], &[]);
    assert_eq!(out.status.code(), Some(1));
    let s = text(&out.stdout);
    assert!(s.contains("warning: 'a' is defined more than once"), "{s}");
    assert!(s.contains("0 error(s), 2 warning(s)"), "{s}");
}

#[test]
fn files_that_cannot_be_opened_exit_2_and_say_why_on_stderr() {
    let d = Dir::new("corrupt");
    std::fs::write(d.path("junk.ocalc"), b"not a worksheet").unwrap();
    let out = run(&[&d.path("junk.ocalc")], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("junk.ocalc: error: cannot open"),
        "{}",
        text(&out.stderr)
    );
    let out = run(&[&d.path("missing.ocalc")], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("not found"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn with_several_files_each_is_reported_and_the_worst_result_wins() {
    let d = Dir::new("many");
    let good = d.sheet("good.ocalc", &["a := 1"]);
    let bad = d.sheet("bad.ocalc", &["a := b"]);
    let out = run(&[&good, &bad], &[]);
    assert_eq!(out.status.code(), Some(1));
    let s = text(&out.stdout);
    assert!(
        s.contains("good.ocalc: 1 regions, 0 error(s)") && s.contains("bad.ocalc: region 1"),
        "{s}"
    );
    let out = run(&[&good, &bad, &d.path("missing.ocalc")], &[]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn repairs_made_while_loading_are_reported_as_warnings() {
    let d = Dir::new("repair");
    let json = r#"{"title":"t","next_id":1,"regions":[{"id":5,"pos":{"x":0,"y":0},"size":{"w":1,"h":1},"kind":{"Math":{"source":"1"}}}]}"#;
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    z.start_file("manifest.json", o).unwrap();
    z.write_all(br#"{"format_version":1}"#).unwrap();
    z.start_file("worksheet.json", o).unwrap();
    z.write_all(json.as_bytes()).unwrap();
    std::fs::write(d.path("r.ocalc"), z.finish().unwrap().into_inner()).unwrap();
    let out = run(&[&d.path("r.ocalc")], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stdout).contains("repaired on load"),
        "{}",
        text(&out.stdout)
    );
}

#[test]
fn no_files_is_a_usage_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_opencalc-cli"))
        .arg("check")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "clap's usage error is exit 2");
}
