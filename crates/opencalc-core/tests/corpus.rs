//! Runs the hand-checked worksheets in `tests/corpus` (see its README for the
//! file format) and checks three things:
//!
//! 1. every expectation in every case holds;
//! 2. results do not depend on the order regions are laid out in;
//! 3. the harness itself can fail: its self-tests feed it wrong expectations and
//!    check that each one is reported, so a green run means something.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use opencalc_core::eval::{evaluate, EvalError, Output, RegionId, Warning};
use opencalc_core::expr::{AssignKind, Expr};
use opencalc_core::parser::parse;
use opencalc_core::units::UnitError;
use opencalc_core::value::Value;

#[derive(Debug, Clone, PartialEq)]
enum Target {
    Name(String),
    /// The nth source region, counting from 1.
    Index(usize),
}

#[derive(Debug, Clone)]
enum Expect {
    Value {
        target: Target,
        tol: f64,
        expr: String,
        line: usize,
    },
    Error {
        target: Target,
        kind: String,
        line: usize,
    },
    Warn {
        name: String,
        line: usize,
    },
}

struct Case {
    file: String,
    has_source: bool,
    ordered: bool,
    regions: Vec<String>,
    expects: Vec<Expect>,
}

const DEFAULT_TOLERANCE: f64 = 1e-9;

fn parse_target(s: &str, line: usize) -> Result<Target, String> {
    if let Some(n) = s.strip_prefix('#') {
        let index: usize = n
            .parse()
            .map_err(|_| format!("line {line}: bad region number '{s}'"))?;
        return Ok(Target::Index(index));
    }
    if s.is_empty() || s.contains(char::is_whitespace) {
        return Err(format!("line {line}: bad target '{s}'"));
    }
    Ok(Target::Name(s.to_owned()))
}

fn parse_case(file: &str, text: &str) -> Result<Case, String> {
    let mut case = Case {
        file: file.to_owned(),
        has_source: false,
        ordered: false,
        regions: Vec::new(),
        expects: Vec::new(),
    };
    for (n, raw) in text.lines().enumerate() {
        let (line, no) = (raw.trim(), n + 1);
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            if comment.trim_start().starts_with("source:") {
                case.has_source = true;
            }
        } else if line == "@ordered" {
            case.ordered = true;
        } else if line.starts_with('@') {
            return Err(format!("line {no}: unknown directive '{line}'"));
        } else if let Some(rest) = line.strip_prefix("=>") {
            let (lhs, expr) = rest
                .split_once('=')
                .ok_or_else(|| format!("line {no}: '=>' needs 'name = expression'"))?;
            let mut parts = lhs.split_whitespace();
            let target = parse_target(parts.next().unwrap_or(""), no)?;
            let tol = match parts.next() {
                None => DEFAULT_TOLERANCE,
                Some(t) => t
                    .strip_prefix('~')
                    .and_then(|v| v.parse::<f64>().ok())
                    .ok_or_else(|| format!("line {no}: bad tolerance '{t}'"))?,
            };
            if parts.next().is_some() || expr.trim().is_empty() {
                return Err(format!("line {no}: malformed '=>' line"));
            }
            case.expects.push(Expect::Value {
                target,
                tol,
                expr: expr.trim().to_owned(),
                line: no,
            });
        } else if let Some(rest) = line.strip_prefix("!>") {
            let (lhs, rhs) = rest
                .split_once(':')
                .ok_or_else(|| format!("line {no}: '!>' needs 'name : kind'"))?;
            let (lhs, rhs) = (lhs.trim(), rhs.trim());
            if lhs == "warn" {
                let name = rhs
                    .strip_prefix("redefinition")
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .ok_or_else(|| format!("line {no}: expected 'warn : redefinition name'"))?;
                case.expects.push(Expect::Warn {
                    name: name.to_owned(),
                    line: no,
                });
            } else {
                case.expects.push(Expect::Error {
                    target: parse_target(lhs, no)?,
                    kind: rhs.to_owned(),
                    line: no,
                });
            }
        } else {
            case.regions.push(line.to_owned());
        }
    }
    if !case.has_source {
        return Err(format!(
            "{file}: missing a '# source:' line (README rule 3)"
        ));
    }
    Ok(case)
}

fn kind_of(e: &EvalError) -> &'static str {
    match e {
        EvalError::Unit(UnitError::Mismatch(..)) => "mismatch",
        EvalError::Unit(UnitError::NotDimensionless(_)) => "not-dimensionless",
        EvalError::Unit(UnitError::DimensionOverflow) => "overflow",
        EvalError::Unit(UnitError::UnknownUnit(_)) => "unknown-unit",
        EvalError::Undefined(_) => "undefined",
        EvalError::Cycle => "cycle",
        EvalError::NotFinite => "notfinite",
        EvalError::UnknownFunction(_) => "unknown-function",
        EvalError::Arity(..) => "arity",
        EvalError::Invalid(_) => "invalid",
        EvalError::Cancelled => "cancelled",
        EvalError::OutOfBudget(_) => "budget",
        EvalError::Internal(_) => "internal",
        // An error inside a function is judged by what went wrong inside it.
        EvalError::InFunction { error, .. } | EvalError::InPlot { error, .. } => kind_of(error),
        EvalError::Value(_) => "value",
    }
}

/// Regions with ids 1..=n in the given layout order (`order[k]` is the index of
/// the source line placed k-th). Ids follow the source line, not the position.
fn build(sources: &[String], order: &[usize]) -> Vec<(RegionId, Expr)> {
    order
        .iter()
        .map(|&i| (RegionId(i as u64 + 1), parse(&sources[i])))
        .collect()
}

fn region_for(sources: &[String], regions: &[(RegionId, Expr)], t: &Target) -> Option<RegionId> {
    match t {
        Target::Index(n) => (1..=sources.len())
            .contains(n)
            .then_some(RegionId(*n as u64)),
        // The last definition in layout order wins, as in the evaluator.
        Target::Name(name) => regions.iter().rev().find_map(|(id, e)| match e {
            Expr::Assign {
                kind: AssignKind::Define,
                target,
                ..
            } if target == name => Some(*id),
            // A function is named by its name alone: `!> area : ...`.
            Expr::FunctionDef { name: f, .. } if f == name => Some(*id),
            _ => None,
        }),
    }
}

fn describe(t: &Target) -> String {
    match t {
        Target::Name(n) => n.clone(),
        Target::Index(i) => format!("#{i}"),
    }
}

fn close(got: f64, want: f64, tol: f64) -> bool {
    let diff = (got - want).abs();
    if want == 0.0 {
        diff <= 1e-12
    } else {
        diff <= tol * want.abs()
    }
}

/// Why `got` is not `want` within the relative tolerance, if it is not. Compares
/// dimensions and, element by element, the real and imaginary parts in SI.
fn differs(got: &Value, want: &Value, tol: f64) -> Option<String> {
    if got.dim() != want.dim() {
        return Some(format!(
            "has dimension {:?}, expected {:?}",
            got.dim(),
            want.dim()
        ));
    }
    let parts = |v: &Value| -> Vec<f64> {
        match v {
            Value::Scalar(s) => vec![s.z.re, s.z.im],
            Value::Matrix(m) => {
                let mut out = vec![m.rows() as f64, m.cols() as f64];
                out.extend(m.elements().iter().flat_map(|z| [z.re, z.im]));
                out
            }
            // A different kind of value must not compare equal to a matrix of the
            // same numbers, so the length is tagged with a marker no matrix has.
            Value::Series(r) => {
                let mut out = vec![-1.0, r.len() as f64];
                out.extend(r.elements().iter().flat_map(|z| [z.re, z.im]));
                out
            }
            Value::Function { params } => vec![-2.0, params.len() as f64],
        }
    };
    let (g, w) = (parts(got), parts(want));
    if g.len() != w.len() {
        return Some(format!("is {got:?}, expected {want:?} (different shape)"));
    }
    if g.iter().zip(&w).any(|(a, b)| !close(*a, *b, tol)) {
        return Some(format!("is {got:?} (SI), expected {want:?}"));
    }
    None
}

/// Check every expectation of a case against one evaluation. Returns failures.
fn check(case: &Case, regions: &[(RegionId, Expr)], out: &Output) -> Vec<String> {
    let mut failures = Vec::new();
    let mut fail = |line: usize, msg: String| failures.push(format!("{}:{line}: {msg}", case.file));
    for expect in &case.expects {
        match expect {
            Expect::Value {
                target,
                tol,
                expr,
                line,
            } => {
                let Some(id) = region_for(&case.regions, regions, target) else {
                    fail(*line, format!("no region defines '{}'", describe(target)));
                    continue;
                };
                let want_regions = [(RegionId(0), parse(expr))];
                let want = match evaluate(&want_regions).values.remove(&RegionId(0)) {
                    Some(Ok(q)) => q,
                    other => {
                        fail(
                            *line,
                            format!("expected value '{expr}' does not evaluate: {other:?}"),
                        );
                        continue;
                    }
                };
                match &out.values[&id] {
                    Err(e) => fail(
                        *line,
                        format!("{} should equal {expr} but failed: {e}", describe(target)),
                    ),
                    Ok(got) => {
                        if let Some(problem) = differs(got, &want, *tol) {
                            fail(
                                *line,
                                format!("{}: {problem} (expected '{expr}')", describe(target)),
                            );
                        }
                    }
                }
            }
            Expect::Error { target, kind, line } => {
                let Some(id) = region_for(&case.regions, regions, target) else {
                    fail(*line, format!("no region defines '{}'", describe(target)));
                    continue;
                };
                match &out.values[&id] {
                    Ok(q) => fail(
                        *line,
                        format!(
                            "{} should fail with '{kind}' but gave {q:?}",
                            describe(target)
                        ),
                    ),
                    Err(e) if kind_of(e) != kind => fail(
                        *line,
                        format!(
                            "{} failed with '{}' ({e}), expected '{kind}'",
                            describe(target),
                            kind_of(e)
                        ),
                    ),
                    Err(_) => {}
                }
            }
            Expect::Warn { name, line } => {
                let warned = out.warnings.iter().any(|w| match w {
                    Warning::Redefinition { name: n, .. } => n == name,
                    Warning::UndefinedEvaluation { .. } => false,
                });
                if !warned {
                    fail(
                        *line,
                        format!("expected a redefinition warning for '{name}'"),
                    );
                }
            }
        }
    }
    failures
}

fn run(case: &Case) -> Vec<String> {
    let order: Vec<usize> = (0..case.regions.len()).collect();
    let regions = build(&case.regions, &order);
    let out = evaluate(&regions);
    check(case, &regions, &out)
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

fn load_corpus() -> Vec<Case> {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("tests/corpus exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "calc"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let text = fs::read_to_string(path).expect("readable");
            parse_case(&name, &text).unwrap_or_else(|e| panic!("{e}"))
        })
        .collect()
}

// ---------------------------------------------------------------- the corpus

#[test]
fn every_corpus_case_matches_its_expectations() {
    let cases = load_corpus();
    assert!(
        cases.len() >= 12,
        "the corpus has shrunk to {} cases",
        cases.len()
    );
    let mut failures = Vec::new();
    for case in &cases {
        assert!(!case.expects.is_empty(), "{} checks nothing", case.file);
        assert!(!case.regions.is_empty(), "{} has no regions", case.file);
        failures.extend(run(case));
    }
    assert!(
        failures.is_empty(),
        "corpus failures:\n{}",
        failures.join("\n")
    );
}

/// Deterministic Fisher-Yates shuffle.
fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    for i in (1..n).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        order.swap(i, (state >> 33) as usize % (i + 1));
    }
    order
}

#[test]
fn results_do_not_depend_on_the_order_regions_are_laid_out_in() {
    let mut checked = 0;
    for case in load_corpus().iter().filter(|c| !c.ordered) {
        let n = case.regions.len();
        let identity: Vec<usize> = (0..n).collect();
        let baseline = evaluate(&build(&case.regions, &identity)).values;
        let mut orders = vec![identity.iter().rev().copied().collect::<Vec<_>>()];
        orders.extend((1..=12).map(|seed| shuffled(n, seed)));
        for order in orders {
            let got = evaluate(&build(&case.regions, &order)).values;
            assert_eq!(
                got, baseline,
                "{}: layout order {order:?} changed a result",
                case.file
            );
        }
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} cases were order-checked");
}

#[test]
fn ordered_cases_really_are_order_dependent() {
    // The @ordered flag must be earned: reversing the redefinition case changes
    // which definition wins, so skipping the shuffle check for it is justified.
    let case = load_corpus()
        .into_iter()
        .find(|c| c.ordered)
        .expect("an @ordered case exists");
    let n = case.regions.len();
    let fwd: Vec<usize> = (0..n).collect();
    let rev: Vec<usize> = fwd.iter().rev().copied().collect();
    let a = evaluate(&build(&case.regions, &fwd)).values;
    let b = evaluate(&build(&case.regions, &rev)).values;
    assert_ne!(a, b);
}

// ----------------------------------------------------------- harness self-tests

fn run_text(body: &str) -> Vec<String> {
    let text = format!("# source: self-test\n{body}");
    run(&parse_case("self-test", &text).expect("a well-formed case"))
}

#[test]
fn a_correct_case_passes() {
    assert!(run_text("a := 5 m\nb := a * 2\n=> b = 10 m\n=> b = 10000 mm\n").is_empty());
}

#[test]
fn a_wrong_value_is_reported_with_its_line() {
    let f = run_text("a := 5\n=> a = 6\n");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].contains("self-test:3"), "{f:?}");
}

#[test]
fn a_wrong_dimension_is_reported_even_if_the_number_matches() {
    let f = run_text("a := 5 m\n=> a = 5 kg\n");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].contains("dimension"), "{f:?}");
}

#[test]
fn tolerance_is_enforced_and_can_be_widened() {
    assert_eq!(run_text("a := 1.0000001\n=> a = 1\n").len(), 1);
    assert!(run_text("a := 1.0000001\n=> a ~1e-6 = 1\n").is_empty());
}

#[test]
fn zero_is_compared_with_an_absolute_tolerance() {
    assert!(run_text("a := 1 - 1\n=> a = 0\n").is_empty());
    assert_eq!(run_text("a := 0.001\n=> a = 0\n").len(), 1);
}

#[test]
fn a_value_expected_from_a_region_that_fails_is_reported() {
    let f = run_text("a := 1 m + 1 kg\n=> a = 1 m\n");
    assert!(f[0].contains("failed"), "{f:?}");
}

#[test]
fn an_error_expected_from_a_region_that_succeeds_is_reported() {
    let f = run_text("a := 1 m\n!> a : mismatch\n");
    assert!(f[0].contains("should fail"), "{f:?}");
}

#[test]
fn the_wrong_kind_of_error_is_reported() {
    let f = run_text("a := 1 m + 1 kg\n!> a : undefined\n");
    assert!(f[0].contains("expected 'undefined'"), "{f:?}");
    assert!(run_text("a := 1 m + 1 kg\n!> a : mismatch\n").is_empty());
}

#[test]
fn an_expectation_about_a_name_nobody_defines_is_reported() {
    let f = run_text("a := 1\n=> nobody = 1\n");
    assert!(f[0].contains("no region defines"), "{f:?}");
    let g = run_text("a := 1\n!> #9 : cycle\n");
    assert!(g[0].contains("no region defines"), "{g:?}");
}

#[test]
fn a_missing_warning_is_reported_and_a_real_one_is_not() {
    assert_eq!(run_text("x := 1\n!> warn : redefinition x\n").len(), 1);
    assert!(run_text("x := 1\nx := 2\n!> warn : redefinition x\n").is_empty());
}

#[test]
fn an_expected_expression_that_does_not_evaluate_is_reported() {
    let f = run_text("a := 1\n=> a = 1 +\n");
    assert!(f[0].contains("does not evaluate"), "{f:?}");
}

#[test]
fn index_targets_address_regions_that_define_no_name() {
    assert!(run_text("1 m + 2 m\n=> #1 = 3 m\n").is_empty());
    assert_eq!(run_text("1 m + 2 m\n=> #1 = 4 m\n").len(), 1);
}

#[test]
fn malformed_case_files_are_rejected() {
    let bad = |body: &str| parse_case("t", &format!("# source: x\n{body}")).is_err();
    assert!(parse_case("t", "a := 1\n").is_err(), "no source line");
    assert!(bad("@frobnicate\n"));
    assert!(bad("=> a 5\n"));
    assert!(bad("=> a ~nonsense = 5\n"));
    assert!(bad("=> = 5\n"));
    assert!(bad("!> a mismatch\n"));
    assert!(bad("!> warn : something else\n"));
    assert!(bad("=> #x = 5\n"));
}

#[test]
fn every_corpus_file_names_its_source() {
    // load_corpus() already fails on a missing source; this states the rule
    // directly and checks that the source text is more than a placeholder.
    for entry in fs::read_dir(corpus_dir()).expect("corpus dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|x| x == "calc") {
            let text = fs::read_to_string(&path).expect("readable");
            let source = text
                .lines()
                .find_map(|l| l.trim().strip_prefix("# source:"))
                .unwrap_or_else(|| panic!("{} has no source", path.display()));
            assert!(
                source.trim().len() > 15,
                "{}: source is too thin",
                path.display()
            );
        }
    }
}

#[test]
fn kinds_documented_in_the_readme_are_all_produced_by_some_error() {
    // Keeps README, harness and engine in step: each documented kind maps from a
    // real error, so a typo in a corpus file cannot silently match nothing.
    let documented = [
        "mismatch",
        "not-dimensionless",
        "overflow",
        "unknown-unit",
        "undefined",
        "cycle",
        "notfinite",
        "unknown-function",
        "arity",
        "invalid",
        "internal",
    ];
    let produced: HashMap<&str, EvalError> = HashMap::from([
        (
            "mismatch",
            EvalError::Unit(UnitError::Mismatch(Default::default(), Default::default())),
        ),
        (
            "not-dimensionless",
            EvalError::Unit(UnitError::NotDimensionless("f".into())),
        ),
        ("overflow", EvalError::Unit(UnitError::DimensionOverflow)),
        (
            "unknown-unit",
            EvalError::Unit(UnitError::UnknownUnit("u".into())),
        ),
        ("undefined", EvalError::Undefined("x".into())),
        ("cycle", EvalError::Cycle),
        ("notfinite", EvalError::NotFinite),
        ("unknown-function", EvalError::UnknownFunction("f".into())),
        ("arity", EvalError::Arity("f".into(), 1)),
        ("invalid", EvalError::Invalid("x".into())),
        ("cancelled", EvalError::Cancelled),
        ("budget", EvalError::OutOfBudget(1)),
        ("internal", EvalError::Internal("x".into())),
    ]);
    for kind in documented {
        assert_eq!(kind_of(&produced[kind]), kind);
    }
    let readme = fs::read_to_string(corpus_dir().join("README.md")).expect("README");
    for kind in documented {
        assert!(readme.contains(kind), "README does not document '{kind}'");
    }
}
