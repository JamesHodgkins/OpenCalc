//! Performance baseline for the SPEC 5.6 targets that the core can measure.
//!
//! Run in release:  cargo bench -p opencalc-core --bench recalc
//! Fail if a budget is exceeded:  ... --bench recalc -- --check
//! Fewer samples, for a quick look:  ... --bench recalc -- --quick
//!
//! It is a plain program rather than a benchmarking framework, to keep the core's
//! dependencies small. Each workload is run several times after a warm-up and the
//! median is reported with the best and worst. Numbers are recorded in
//! docs/PERFORMANCE.md.

#[path = "../tests/common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use opencalc_core::document::{Point, RegionKind, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::eval::{evaluate, RegionId};
use opencalc_core::expr::Expr;
use opencalc_core::parser::parse;
use opencalc_core::worker::Worker;

struct Stats {
    min: Duration,
    median: Duration,
    max: Duration,
}

/// Time `f` over `runs` runs after a short warm-up.
fn sample(runs: usize, mut f: impl FnMut()) -> Stats {
    for _ in 0..3 {
        f();
    }
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    times.sort();
    Stats {
        min: times[0],
        median: times[times.len() / 2],
        max: times[times.len() - 1],
    }
}

fn show(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1000.0 {
        format!("{us:8.1} us")
    } else {
        format!("{:8.2} ms", us / 1000.0)
    }
}

struct Row {
    name: String,
    stats: Stats,
    budget: Option<Duration>,
}

fn sheet_regions(n: usize) -> Vec<(RegionId, Expr)> {
    let lines = common::worksheet_lines(n, 7);
    // Shuffled, so layout order is not dependency order (as in a real worksheet).
    common::regions(&lines, &common::shuffled(n, 11))
}

/// A worksheet of `n` independent definitions: no region depends on another.
fn wide(n: usize) -> Vec<(RegionId, Expr)> {
    (0..n)
        .map(|i| {
            (
                RegionId(i as u64 + 1),
                parse(&format!("w{i} := {} m * 2", i + 1)),
            )
        })
        .collect()
}

/// A single dependency chain `c0, c1 := c0 + 1, ...`, in reverse layout order.
fn chain(n: usize) -> Vec<(RegionId, Expr)> {
    let mut v: Vec<(RegionId, Expr)> = (0..n)
        .map(|i| {
            let src = if i == 0 {
                "c0 := 1".to_owned()
            } else {
                format!("c{i} := c{} + 1", i - 1)
            };
            (RegionId(i as u64 + 1), parse(&src))
        })
        .collect();
    v.reverse();
    v
}

fn worksheet_document(n: usize) -> Worksheet {
    let mut doc = Worksheet::new("bench");
    for (i, line) in common::worksheet_lines(n, 7).iter().enumerate() {
        let r = doc.new_region(RegionKind::math(line), Point::new(0.0, i as f32 * 40.0));
        let at = doc.regions().len();
        doc.apply(Edit::InsertRegions {
            regions: vec![(at, r)],
        })
        .expect("insert");
    }
    doc
}

/// Wait for the worker's answer to `revision`, spinning (a sleep would add the
/// operating system's timer granularity to the measurement).
fn round_trip(worker: &Worker, revision: u64, regions: Vec<(RegionId, Expr)>) {
    assert!(worker.request(revision, regions));
    let deadline = Instant::now() + Duration::from_secs(10);
    while worker.poll_for(revision).is_none() {
        assert!(Instant::now() < deadline, "the worker did not answer");
        std::thread::yield_now();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let check = args.iter().any(|a| a == "--check");
    let runs = if args.iter().any(|a| a == "--quick") {
        5
    } else {
        40
    };
    let ms = Duration::from_millis;
    let mut rows: Vec<Row> = Vec::new();
    let mut add = |name: &str, stats: Stats, budget: Option<Duration>| {
        rows.push(Row {
            name: name.to_owned(),
            stats,
            budget,
        });
    };

    // Parse one region (SPEC: under 1 ms). Timed per call over every line.
    let lines = common::worksheet_lines(200, 7);
    let mut per_call: Vec<Duration> = Vec::new();
    for _ in 0..(runs.max(5)) {
        for line in &lines {
            let start = Instant::now();
            std::hint::black_box(parse(line));
            per_call.push(start.elapsed());
        }
    }
    per_call.sort();
    add(
        "parse one region",
        Stats {
            min: per_call[0],
            median: per_call[per_call.len() / 2],
            max: per_call[per_call.len() - 1],
        },
        Some(ms(1)),
    );
    add(
        "parse 200 regions",
        sample(runs, || {
            for line in &lines {
                std::hint::black_box(parse(line));
            }
        }),
        None,
    );

    // Full recalculation on the evaluator alone.
    for n in [200, 500, 2000] {
        let regions = sheet_regions(n);
        let errors = evaluate(&regions)
            .values
            .values()
            .filter(|r| r.is_err())
            .count();
        assert_eq!(errors, 0, "the generated worksheet must evaluate cleanly");
        let budget = (n == 500).then(|| ms(200));
        add(
            &format!("evaluate {n} regions (mixed)"),
            sample(runs, || {
                std::hint::black_box(evaluate(&regions));
            }),
            budget,
        );
    }
    let wide500 = wide(500);
    add(
        "evaluate 500 regions (all independent)",
        sample(runs, || {
            std::hint::black_box(evaluate(&wide500));
        }),
        None,
    );
    let chain500 = chain(500);
    add(
        "evaluate 500 regions (one long chain)",
        sample(runs, || {
            std::hint::black_box(evaluate(&chain500));
        }),
        None,
    );

    // Edit to updated result, in pieces and end to end (SPEC: under 16 ms for 200).
    let doc = worksheet_document(200);
    add(
        "snapshot 200 regions (clone for the worker)",
        sample(runs, || {
            std::hint::black_box(doc.math_regions());
        }),
        None,
    );

    let regions200 = sheet_regions(200);
    let edited = &lines[100];
    add(
        "edit, evaluator only: parse + evaluate 200",
        sample(runs, || {
            std::hint::black_box(parse(edited));
            std::hint::black_box(evaluate(&regions200));
        }),
        None,
    );

    let worker = Worker::spawn(|| {});
    let mut revision = 0u64;
    add(
        "worker round trip, 200 regions",
        sample(runs, || {
            revision += 1;
            round_trip(&worker, revision, regions200.clone());
        }),
        None,
    );

    let mut doc = worksheet_document(200);
    let target = doc.regions()[100].id;
    let mut n = 0u64;
    add(
        "EDIT TO RESULT: apply edit + snapshot + worker, 200",
        sample(runs, || {
            n += 1;
            doc.apply(Edit::SetMathSource {
                id: target,
                source: format!("L100 := {n} m"),
            })
            .expect("edit");
            revision += 1;
            round_trip(&worker, revision, doc.math_regions());
        }),
        Some(ms(16)),
    );

    println!("\nOpenCalc core performance (release build; {runs} runs each)\n");
    println!(
        "{:<58} {:>11} {:>11} {:>11}   budget",
        "workload", "median", "best", "worst"
    );
    let mut over = 0;
    for row in &rows {
        let verdict = match row.budget {
            None => String::new(),
            Some(b) if row.stats.median <= b => format!("{} ok", show(b).trim()),
            Some(b) => {
                over += 1;
                format!("{} OVER", show(b).trim())
            }
        };
        println!(
            "{:<58} {:>11} {:>11} {:>11}   {verdict}",
            row.name,
            show(row.stats.median),
            show(row.stats.min),
            show(row.stats.max)
        );
    }
    println!();
    if over > 0 {
        println!("{over} budget(s) exceeded.");
        if check {
            std::process::exit(1);
        }
    } else {
        println!("All budgets met.");
    }
}
