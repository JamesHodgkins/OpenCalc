//! Property tests: statements that must hold for *any* input, checked on many
//! generated ones. They cover what hand-written examples cannot: unit algebra
//! over the whole exponent range, and the engine's promise that no input can
//! panic it or make it return a non-finite number.
//!
//! A failing case is shrunk to a minimal one and saved under
//! `proptest-regressions/`; commit that file so the case stays a test.

mod common;

use opencalc_core::eval::{evaluate, EvalError, Output, RegionId};
use opencalc_core::expr::Expr;
use opencalc_core::parser::{parse, MAX_SYMBOLS};
use opencalc_core::units::{builtin_symbols, builtin_unit, Dimension, Quantity, UnitError};
use proptest::prelude::*;

// -------------------------------------------------------------- strategies

fn from_exps(e: [i8; 7]) -> Dimension {
    Dimension {
        length: e[0],
        mass: e[1],
        time: e[2],
        current: e[3],
        temperature: e[4],
        amount: e[5],
        luminosity: e[6],
    }
}

fn exps(d: Dimension) -> [i8; 7] {
    [
        d.length,
        d.mass,
        d.time,
        d.current,
        d.temperature,
        d.amount,
        d.luminosity,
    ]
}

/// Exponents small enough that sums of a few cannot overflow.
fn small_dim() -> impl Strategy<Value = Dimension> {
    prop::array::uniform7(-6i8..=6).prop_map(from_exps)
}

/// Every possible exponent, including the extremes.
fn any_dim() -> impl Strategy<Value = Dimension> {
    prop::array::uniform7(any::<i8>()).prop_map(from_exps)
}

fn symbol() -> impl Strategy<Value = &'static str> {
    prop::sample::select(builtin_symbols().collect::<Vec<_>>())
}

fn quantity() -> impl Strategy<Value = Quantity> {
    (symbol(), -1e6f64..1e6).prop_map(|(s, v)| Quantity::from_unit(v, s).expect("built-in unit"))
}

/// Pairs of distinct built-in units that measure the same thing.
fn same_dimension_pairs() -> Vec<(&'static str, &'static str)> {
    let all: Vec<&str> = builtin_symbols().collect();
    let mut pairs = Vec::new();
    for a in &all {
        for b in &all {
            let (ua, ub) = (
                builtin_unit(a).expect("unit"),
                builtin_unit(b).expect("unit"),
            );
            if a != b && ua.dimension == ub.dimension {
                pairs.push((*a, *b));
            }
        }
    }
    pairs
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    a == b || (a - b).abs() <= tol * a.abs().max(b.abs())
}

fn eval_lines(lines: &[String]) -> Output {
    evaluate(&common::regions(lines, &common::identity(lines.len())))
}

// ---------------------------------------------------------- dimension algebra

proptest! {
    /// The checked operations agree with arithmetic done in a wider integer:
    /// they give a result exactly when every exponent fits, and the same result.
    #[test]
    fn checked_dimension_ops_match_wide_arithmetic(a in any_dim(), b in any_dim(), n in any::<i8>()) {
        let (ea, eb) = (exps(a), exps(b));
        for (name, got, wide) in [
            ("times", a.checked_times(b), std::array::from_fn::<i16, 7, _>(|i| i16::from(ea[i]) + i16::from(eb[i]))),
            ("over", a.checked_over(b), std::array::from_fn(|i| i16::from(ea[i]) - i16::from(eb[i]))),
            ("powi", a.checked_powi(n), std::array::from_fn(|i| i16::from(ea[i]) * i16::from(n))),
        ] {
            let fits = wide.iter().all(|w| i8::try_from(*w).is_ok());
            match got {
                Some(d) => {
                    prop_assert!(fits, "{name} returned a result although an exponent overflows");
                    prop_assert_eq!(exps(d), std::array::from_fn::<i8, 7, _>(|i| wide[i] as i8));
                }
                None => prop_assert!(!fits, "{name} reported overflow but everything fits"),
            }
        }
    }

    #[test]
    fn dimension_multiplication_is_a_commutative_associative_group(a in small_dim(), b in small_dim(), c in small_dim()) {
        prop_assert_eq!(a.checked_times(b), b.checked_times(a));
        let left = a.checked_times(b).and_then(|ab| ab.checked_times(c));
        let right = b.checked_times(c).and_then(|bc| a.checked_times(bc));
        prop_assert_eq!(left, right);
        prop_assert_eq!(a.checked_times(Dimension::NONE), Some(a));
        prop_assert_eq!(a.checked_times(b).and_then(|ab| ab.checked_over(b)), Some(a));
        prop_assert_eq!(a.checked_over(a), Some(Dimension::NONE));
    }

    #[test]
    fn powers_are_repeated_multiplication(a in small_dim(), n in 0i8..6) {
        let mut product = Dimension::NONE;
        for _ in 0..n {
            product = product.checked_times(a).expect("small exponents cannot overflow");
        }
        prop_assert_eq!(a.checked_powi(n), Some(product));
    }

    #[test]
    fn halving_inverts_squaring(a in small_dim()) {
        let squared = a.checked_powi(2).expect("small");
        prop_assert_eq!(squared.halved(), Some(a));
    }
}

// --------------------------------------------------------------- quantities

proptest! {
    #[test]
    fn converting_to_a_unit_and_back_round_trips(sym in symbol(), v in -1e9f64..1e9) {
        let q = Quantity::from_unit(v, sym).expect("built-in unit");
        let back = q.in_unit(sym).expect("same unit");
        prop_assert!(close(back, v, 1e-12), "{v} {sym} came back as {back}");
    }

    #[test]
    fn converting_between_units_goes_through_their_scale_ratio(
        pair in prop::sample::select(same_dimension_pairs()),
        v in -1e6f64..1e6,
    ) {
        let (from, to) = pair;
        let q = Quantity::from_unit(v, from).expect("unit");
        let got = q.in_unit(to).expect("same dimension");
        let (sf, st) = (builtin_unit(from).expect("unit").to_si_scale, builtin_unit(to).expect("unit").to_si_scale);
        prop_assert!(close(got, v * sf / st, 1e-12), "{v} {from} in {to}: {got}");
        // Converting to a unit of a different dimension is refused.
        let other = if q.dim == Dimension::TIME { "kg" } else { "s" };
        prop_assert!(matches!(q.in_unit(other), Err(UnitError::Mismatch(..))));
    }

    #[test]
    fn adding_same_dimension_quantities_commutes_and_keeps_the_dimension(
        sym in symbol(), x in -1e6f64..1e6, y in -1e6f64..1e6,
    ) {
        let a = Quantity::from_unit(x, sym).expect("unit");
        let b = Quantity::from_unit(y, sym).expect("unit");
        let (ab, ba) = (a.try_add(b), b.try_add(a));
        prop_assert_eq!(ab.clone(), ba);
        prop_assert_eq!(ab.expect("same dimension").dim, a.dim);
        prop_assert_eq!(a.try_sub(a).expect("same dimension").si, 0.0);
    }

    #[test]
    fn adding_different_dimensions_is_always_an_error(a in quantity(), b in quantity()) {
        prop_assume!(a.dim != b.dim);
        prop_assert!(matches!(a.try_add(b), Err(UnitError::Mismatch(..))));
        prop_assert!(matches!(a.try_sub(b), Err(UnitError::Mismatch(..))));
    }

    #[test]
    fn multiplication_and_division_combine_dimensions(a in quantity(), b in quantity()) {
        if let Ok(p) = a.times(b) {
            prop_assert_eq!(Some(p.dim), a.dim.checked_times(b.dim));
            prop_assert!(close(p.si, a.si * b.si, 1e-15));
        }
        if let Ok(q) = a.over(b) {
            prop_assert_eq!(Some(q.dim), a.dim.checked_over(b.dim));
        }
    }

    #[test]
    fn negation_is_an_involution(a in quantity()) {
        prop_assert_eq!(a.negated().negated(), a);
    }

    #[test]
    fn integer_powers_scale_the_dimension(a in quantity(), n in -3i8..=3) {
        let p = a.pow(Quantity::dimensionless(f64::from(n)));
        if a.si != 0.0 || n >= 0 {
            let p = p.expect("a small integer power is defined");
            prop_assert_eq!(Some(p.dim), a.dim.checked_powi(n));
        }
    }
}

// ------------------------------------------------------------- the engine

fn token() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "0",
        "1",
        "2.5",
        "1e3",
        "1e308",
        "100",
        "a",
        "b",
        "x1",
        "m",
        "kg",
        "kN",
        "s",
        "mm",
        "+",
        "-",
        "*",
        "/",
        "^",
        "(",
        ")",
        ",",
        ":=",
        "=",
        "sin(",
        "sqrt(",
        "ln(",
        "abs(",
        "foo(",
        "e",
        "1e",
        ".",
        "\u{3c3}",
        "$",
        "i",
        "vec(",
        "mat(",
        "det(",
        "norm(",
        "transpose(",
        "dot(",
        "conj(",
        "re(",
    ])
}

fn line() -> impl Strategy<Value = String> {
    prop::collection::vec(token(), 1..30).prop_map(|t| t.join(" "))
}

fn sheet() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(line(), 1..6)
}

proptest! {
    /// The promise from the audit: whatever is typed, parsing and evaluating it
    /// never panics, every region gets an answer, and no answer is NaN or infinite.
    #[test]
    fn no_input_can_panic_the_engine_or_produce_a_non_finite_number(lines in sheet()) {
        let regions = common::regions(&lines, &common::identity(lines.len()));
        let out = evaluate(&regions);
        prop_assert_eq!(out.values.len(), lines.len());
        for (id, result) in &out.values {
            if let Ok(q) = result {
                prop_assert!(q.is_finite(), "{:?} gave a non-finite value from {:?}", id, lines);
            }
        }
        // Every error location resolves to a real part of that region's expression.
        for (id, path) in &out.error_paths {
            let expr = &regions.iter().find(|(rid, _)| rid == id).expect("region").1;
            prop_assert!(expr.at_path(path).is_some(), "{:?} {:?} in {:?}", id, path, lines);
            prop_assert!(out.values[id].is_err());
        }
        // Deterministic: evaluating again gives exactly the same answers.
        prop_assert_eq!(evaluate(&regions).values, out.values);
    }

    #[test]
    fn arbitrary_text_never_panics_the_parser(s in "\\PC{0,300}") {
        let _ = parse(&s);
    }

    #[test]
    fn arbitrary_bytes_as_text_never_panic_the_parser(bytes in prop::collection::vec(any::<u8>(), 0..400)) {
        let _ = parse(&String::from_utf8_lossy(&bytes));
    }

    #[test]
    fn text_over_the_limit_is_rejected_with_an_error_value(n in (MAX_SYMBOLS + 1)..2000usize) {
        let src = "1+".repeat(n);
        prop_assert!(matches!(parse(&src), Expr::Error(_)));
    }

    #[test]
    fn a_decimal_number_parses_back_to_exactly_itself(x in 0f64..1e12) {
        let regions = vec![(RegionId(1), parse(&format!("{x}")))];
        let got = evaluate(&regions).values[&RegionId(1)].clone();
        prop_assert_eq!(got, Ok(Quantity::dimensionless(x).into()));
    }

    #[test]
    fn a_number_followed_by_a_unit_is_that_quantity(sym in symbol(), v in 0f64..1e6) {
        let regions = vec![(RegionId(1), parse(&format!("{v} {sym}")))];
        let got = evaluate(&regions).values[&RegionId(1)].clone();
        prop_assert_eq!(got, Ok(Quantity::from_unit(v, sym).expect("unit").into()));
    }

    /// Typed exponents reach the dimension arithmetic: results are exact or an
    /// overflow error, never a silently wrapped dimension.
    #[test]
    fn typed_unit_powers_are_exact_or_report_overflow(a in -127i32..=127, b in -127i32..=127) {
        for (op, want) in [("*", a + b), ("/", a - b)] {
            let src = format!("(1 m)^({a}) {op} (1 m)^({b})");
            let regions = vec![(RegionId(1), parse(&src))];
            match &evaluate(&regions).values[&RegionId(1)] {
                Ok(q) => {
                    prop_assert_eq!(i32::from(q.dim().length), want, "{}", src);
                }
                Err(EvalError::Unit(UnitError::DimensionOverflow)) => {
                    prop_assert!(want.abs() > 127, "{} reported overflow for a representable exponent {}", src, want);
                }
                Err(other) => prop_assert!(false, "{}: unexpected error {:?}", src, other),
            }
        }
    }
}

proptest! {
    /// The headline guarantee: a worksheet means the same wherever its regions are
    /// placed. Checked on generated, dimensionally valid worksheets.
    #[test]
    fn layout_order_never_changes_a_result(n in 5usize..80, seed in any::<u64>(), shuffle in any::<u64>()) {
        let lines = common::worksheet_lines(n, seed);
        let base = evaluate(&common::regions(&lines, &common::identity(n)));
        prop_assert!(base.values.values().all(Result::is_ok), "the generator must make valid worksheets");
        let moved = evaluate(&common::regions(&lines, &common::shuffled(n, shuffle)));
        prop_assert_eq!(moved.values, base.values);
        prop_assert!(moved.warnings.is_empty());
    }

    /// A ring of definitions is a cycle: every region in it fails, and an
    /// unrelated region still evaluates.
    #[test]
    fn a_ring_of_definitions_is_a_cycle_and_does_not_poison_others(k in 2usize..10, offset in 0usize..10) {
        let mut lines: Vec<String> = (0..k).map(|i| format!("c{i} := c{} + 1", (i + 1) % k)).collect();
        lines.push("ok := 1 m + 2 m".to_owned());
        let out = eval_lines(&lines);
        for i in 0..k {
            prop_assert_eq!(&out.values[&RegionId(i as u64 + 1)], &Err(EvalError::Cycle));
        }
        prop_assert!(out.values[&RegionId(k as u64 + 1)].is_ok());
        // A dependent of the ring fails too, rather than reading a half-computed value.
        lines.push(format!("dep := c{} * 2", offset % k));
        let out = eval_lines(&lines);
        prop_assert!(out.values[&RegionId(k as u64 + 2)].is_err());
    }

    /// A region that depends on a failed region fails with the same error.
    #[test]
    fn errors_propagate_unchanged_to_dependents(depth in 1usize..12) {
        let mut lines = vec!["e0 := 1 m + 1 kg".to_owned()];
        for i in 1..=depth {
            lines.push(format!("e{i} := e{} * 2", i - 1));
        }
        let out = eval_lines(&lines);
        let root = out.values[&RegionId(1)].clone();
        prop_assert!(matches!(root, Err(EvalError::Unit(UnitError::Mismatch(..)))));
        for i in 0..=depth {
            prop_assert_eq!(&out.values[&RegionId(i as u64 + 1)], &root);
        }
    }
}

// ------------------------------------------- differential test against a reference

/// A dimensionless arithmetic expression, kept as a tree so a tiny independent
/// interpreter can compute what the engine should say.
#[derive(Debug, Clone)]
enum Tree {
    Num(&'static str, f64),
    Neg(Box<Tree>),
    Bin(Box<Tree>, &'static str, Box<Tree>),
    Call(&'static str, Box<Tree>),
}

fn tree() -> impl Strategy<Value = Tree> {
    // Extreme values on purpose: overflow, underflow, zero and exact fractions.
    let leaf = prop::sample::select(vec![
        ("0", 0.0),
        ("1", 1.0),
        ("2", 2.0),
        ("3", 3.0),
        ("0.5", 0.5),
        ("100", 100.0),
        ("1e-5", 1e-5),
        ("1e155", 1e155),
        ("1e308", 1e308),
        ("1e-308", 1e-308),
    ])
    .prop_map(|(s, v)| Tree::Num(s, v));
    leaf.prop_recursive(4, 24, 2, |inner| {
        prop_oneof![
            3 => (inner.clone(), prop::sample::select(vec!["+", "-", "*", "/", "^"]), inner.clone())
                .prop_map(|(a, op, b)| Tree::Bin(Box::new(a), op, Box::new(b))),
            1 => inner.clone().prop_map(|a| Tree::Neg(Box::new(a))),
            1 => (prop::sample::select(vec!["sqrt", "ln", "exp", "sin", "abs"]), inner)
                .prop_map(|(f, a)| Tree::Call(f, Box::new(a))),
        ]
    })
}

fn render(t: &Tree) -> String {
    match t {
        Tree::Num(s, _) => (*s).to_owned(),
        // Parenthesised as a whole: `^` binds tighter than unary minus, so a bare
        // `-(1) ^ 2` means -(1^2), not (-1)^2.
        Tree::Neg(a) => format!("(-({}))", render(a)),
        Tree::Bin(a, op, b) => format!("({} {op} {})", render(a), render(b)),
        Tree::Call(f, a) => format!("{f}({})", render(a)),
    }
}

/// What the reference says about an expression.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ref {
    /// A finite real number, exactly.
    Real(f64),
    /// Some step is not a finite number (division by zero, overflow, `ln(0)`).
    Bad,
    /// Some step leaves the real numbers (`sqrt(-1)`); the reference is real-only,
    /// so it makes no claim about the value.
    Complex,
}

/// What the engine must answer. `Bad` dominates `Complex`: both branches of an
/// operation are evaluated, and either failing makes the whole region an error.
fn reference(t: &Tree) -> Ref {
    let finite = |x: f64| {
        if x.is_finite() {
            Ref::Real(x)
        } else {
            Ref::Bad
        }
    };
    let combine = |a: Ref, b: Ref, f: &dyn Fn(f64, f64) -> Ref| match (a, b) {
        (Ref::Bad, _) | (_, Ref::Bad) => Ref::Bad,
        (Ref::Complex, _) | (_, Ref::Complex) => Ref::Complex,
        (Ref::Real(x), Ref::Real(y)) => f(x, y),
    };
    match t {
        Tree::Num(_, v) => Ref::Real(*v),
        Tree::Neg(a) => match reference(a) {
            Ref::Real(x) => Ref::Real(-x),
            other => other,
        },
        Tree::Bin(a, op, b) => combine(reference(a), reference(b), &|x, y| match *op {
            "+" => finite(x + y),
            "-" => finite(x - y),
            "*" => finite(x * y),
            "/" => finite(x / y),
            _ if x < 0.0 && y.fract() != 0.0 => Ref::Complex,
            _ => finite(x.powf(y)),
        }),
        Tree::Call(f, a) => match reference(a) {
            Ref::Real(x) => match *f {
                "sqrt" if x < 0.0 => Ref::Complex,
                "ln" if x < 0.0 => Ref::Complex,
                "sqrt" => finite(x.sqrt()),
                "ln" => finite(x.ln()),
                "exp" => finite(x.exp()),
                "sin" => finite(x.sin()),
                _ => finite(x.abs()),
            },
            other => other,
        },
    }
}

proptest! {
    /// Printing an expression and parsing it again gives the same tree, for any
    /// shape of arithmetic (the error display depends on this).
    #[test]
    fn printed_expressions_parse_back_identically(t in tree()) {
        let parsed = parse(&render(&t));
        prop_assume!(!matches!(parsed, Expr::Error(_)));
        prop_assert_eq!(parse(&parsed.to_string()), parsed);
    }

    /// The engine agrees exactly with the reference on every generated
    /// expression, and answers `NotFinite` precisely when a step is not finite.
    /// A step that overflows and then comes back (`1 / (1e308 * 10)`) must still
    /// be an error, not a quietly wrong 0.
    #[test]
    fn the_engine_matches_an_independent_reference(t in tree()) {
        let src = render(&t);
        let parsed = parse(&src);
        prop_assume!(!matches!(parsed, Expr::Error(_)));
        let got = evaluate(&[(RegionId(1), parsed)]).values[&RegionId(1)].clone();
        match reference(&t) {
            Ref::Real(v) => prop_assert_eq!(got, Ok(Quantity::dimensionless(v).into()), "{}", src),
            Ref::Bad => prop_assert_eq!(got, Err(EvalError::NotFinite), "{}", src),
            // A complex intermediate may overflow or come back to the reals; the
            // value is covered by the corpus. It must still be a clean answer.
            Ref::Complex => prop_assert!(
                matches!(&got, Ok(v) if v.is_finite()) || got == Err(EvalError::NotFinite),
                "{}: {:?}", src, got
            ),
        }
    }
}

// ------------------------------------------------- complex numbers, matrices

/// Evaluate one expression to a value (a failure is a test failure).
fn value_of(src: &str) -> opencalc_core::value::Value {
    let regions = vec![(RegionId(1), parse(src))];
    match evaluate(&regions).values.remove(&RegionId(1)) {
        Some(Ok(v)) => v,
        other => panic!("{src} did not evaluate: {other:?}"),
    }
}

/// The real and imaginary parts of a scalar result.
fn parts(src: &str) -> (f64, f64) {
    match value_of(src) {
        opencalc_core::value::Value::Scalar(s) => (s.z.re, s.z.im),
        other => panic!("{src} is not a scalar: {other:?}"),
    }
}

/// Equal within rounding, for the small exact inputs of the complex tests: a
/// relative part, and a tiny absolute part for results that are zero in theory.
/// (Determinants have their own derived bound, `rounding_bound`.)
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 + 1e-9 * a.abs().max(b.abs())
}

fn small() -> impl Strategy<Value = i32> {
    -20i32..=20
}

fn to_i128(items: &[i32]) -> Vec<i128> {
    items.iter().map(|&x| i128::from(x)).collect()
}

/// Product of two n x n integer matrices, row-major.
fn int_product(a: &[i128], b: &[i128], n: usize) -> Vec<i128> {
    (0..n * n)
        .map(|k| (0..n).map(|j| a[(k / n) * n + j] * b[j * n + k % n]).sum())
        .collect()
}

/// Exact determinant of an integer matrix by fraction-free (Bareiss) elimination:
/// every division is exact, so no rounding is involved.
fn det_exact(entries: &[i128], n: usize) -> i128 {
    let mut m = entries.to_vec();
    let (mut sign, mut previous) = (1, 1);
    for k in 0..n - 1 {
        if m[k * n + k] == 0 {
            let Some(row) = (k + 1..n).find(|&r| m[r * n + k] != 0) else {
                return 0;
            };
            for c in 0..n {
                m.swap(k * n + c, row * n + c);
            }
            sign = -sign;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                m[i * n + j] =
                    (m[i * n + j] * m[k * n + k] - m[i * n + k] * m[k * n + j]) / previous;
            }
        }
        previous = m[k * n + k];
    }
    sign * m[(n - 1) * n + n - 1]
}

/// How far a floating-point elimination may legitimately stray from the exact
/// determinant: n steps, each with a few roundings of relative size epsilon, on
/// values no larger than Hadamard's bound (the product of the row lengths), which
/// bounds the determinant and every intermediate minor.
fn rounding_bound(entries: &[i128], n: usize) -> f64 {
    let hadamard: f64 = (0..n)
        .map(|r| {
            let squares: f64 = (0..n).map(|c| (entries[r * n + c] as f64).powi(2)).sum();
            squares.sqrt()
        })
        .product();
    8.0 * n as f64 * f64::EPSILON * hadamard
}

fn matrix_source(rows: usize, cols: usize, items: &[i32]) -> String {
    let list: Vec<String> = items.iter().map(|x| format!("({x})")).collect();
    format!("mat({rows}, {cols}, {})", list.join(", "))
}

proptest! {
    /// Complex arithmetic obeys the field identities (within rounding).
    #[test]
    fn complex_arithmetic_obeys_the_field_identities(
        (a, b, c, d) in (small(), small(), small(), small()),
    ) {
        prop_assume!(c != 0 || d != 0);
        let z = format!("(({a}) + ({b}) * i)");
        let w = format!("(({c}) + ({d}) * i)");
        // (z * w) / w == z
        let (re, im) = parts(&format!("({z} * {w}) / {w}"));
        prop_assert!(near(re, f64::from(a)) && near(im, f64::from(b)), "{re} {im}");
        // |z|^2 == re^2 + im^2
        let (m2, _) = parts(&format!("abs({z}) ^ 2"));
        prop_assert!(near(m2, f64::from(a * a + b * b)));
        // conj(z * w) == conj(z) * conj(w)
        let lhs = parts(&format!("conj({z} * {w})"));
        let rhs = parts(&format!("conj({z}) * conj({w})"));
        prop_assert!(near(lhs.0, rhs.0) && near(lhs.1, rhs.1));
        // sqrt(z)^2 == z
        let (re, im) = parts(&format!("sqrt({z}) ^ 2"));
        prop_assert!(near(re, f64::from(a)) && near(im, f64::from(b)), "{re} {im}");
    }

    /// The engine's determinant agrees with an exact integer one, to within the
    /// rounding error elimination can legitimately make, and the identities
    /// det(A B) = det A det B, det(A^T) = det A and (A^T)^T = A hold.
    #[test]
    fn matrix_identities_hold(a in prop::collection::vec(small(), 9), b in prop::collection::vec(small(), 9)) {
        let (ma, mb) = (matrix_source(3, 3, &a), matrix_source(3, 3, &b));
        let (ia, ib) = (to_i128(&a), to_i128(&b));
        let iab = int_product(&ia, &ib, 3);

        let engine = |src: &str| parts(src).0;
        let check = |got: f64, exact: i128, entries: &[i128], what: &str| {
            let allowed = rounding_bound(entries, 3);
            let error = (got - exact as f64).abs();
            prop_assert!(error <= allowed, "{what}: engine {got}, exact {exact}, error {error} > {allowed}");
            Ok(())
        };
        check(engine(&format!("det({ma})")), det_exact(&ia, 3), &ia, "det A")?;
        check(engine(&format!("det({mb})")), det_exact(&ib, 3), &ib, "det B")?;
        check(
            engine(&format!("det({ma} * {mb})")),
            det_exact(&ia, 3) * det_exact(&ib, 3),
            &iab,
            "det (A B)",
        )?;
        check(engine(&format!("det(transpose({ma}))")), det_exact(&ia, 3), &ia, "det A^T")?;
        prop_assert_eq!(value_of(&format!("transpose(transpose({ma}))")), value_of(&ma));
    }

    /// Shapes that do not fit are reported, never panic or produce garbage.
    #[test]
    fn mismatched_shapes_are_errors(r1 in 1usize..4, c1 in 1usize..4, r2 in 1usize..4, c2 in 1usize..4) {
        let a = matrix_source(r1, c1, &vec![1; r1 * c1]);
        let b = matrix_source(r2, c2, &vec![2; r2 * c2]);
        let regions = vec![(RegionId(1), parse(&format!("{a} * {b}"))), (RegionId(2), parse(&format!("{a} + {b}")))];
        let out = evaluate(&regions);
        prop_assert_eq!(out.values[&RegionId(1)].is_ok(), c1 == r2);
        prop_assert_eq!(out.values[&RegionId(2)].is_ok(), (r1, c1) == (r2, c2));
    }

    /// A matrix keeps one dimension: scaling by a quantity multiplies it in, and
    /// elements of different dimensions are rejected.
    #[test]
    fn matrix_dimensions_follow_the_elements(n in 1usize..5, x in small()) {
        let items = vec![format!("{x} m"); n].join(", ");
        let v = value_of(&format!("vec({items}) * 2 kg"));
        prop_assert_eq!(v.dim().length, 1);
        prop_assert_eq!(v.dim().mass, 1);
        let mixed = vec![(RegionId(1), parse("vec(1 m, 2 s)"))];
        prop_assert!(evaluate(&mixed).values[&RegionId(1)].is_err());
    }
}
