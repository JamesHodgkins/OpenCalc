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
        "0", "1", "2.5", "1e3", "1e308", "100", "a", "b", "x1", "m", "kg", "kN", "s", "mm", "+",
        "-", "*", "/", "^", "(", ")", ",", ":=", "=", "sin(", "sqrt(", "ln(", "abs(", "foo(", "e",
        "1e", ".", "\u{3c3}", "$",
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
                prop_assert!(q.si.is_finite(), "{:?} gave a non-finite value from {:?}", id, lines);
            }
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
        prop_assert_eq!(got, Ok(Quantity::dimensionless(x)));
    }

    #[test]
    fn a_number_followed_by_a_unit_is_that_quantity(sym in symbol(), v in 0f64..1e6) {
        let regions = vec![(RegionId(1), parse(&format!("{v} {sym}")))];
        let got = evaluate(&regions).values[&RegionId(1)].clone();
        prop_assert_eq!(got, Ok(Quantity::from_unit(v, sym).expect("unit")));
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
                    prop_assert_eq!(i32::from(q.dim.length), want, "{}", src);
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

/// What the engine must answer: the value, or `None` if any step along the way is
/// not a finite number (division by zero, overflow, `sqrt(-1)`, `ln(0)`).
fn reference(t: &Tree) -> Option<f64> {
    let finite = |x: f64| x.is_finite().then_some(x);
    match t {
        Tree::Num(_, v) => Some(*v),
        Tree::Neg(a) => reference(a).map(|x| -x),
        Tree::Bin(a, op, b) => {
            let (x, y) = (reference(a)?, reference(b)?);
            finite(match *op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => x / y,
                _ => x.powf(y),
            })
        }
        Tree::Call(f, a) => {
            let x = reference(a)?;
            finite(match *f {
                "sqrt" => x.sqrt(),
                "ln" => x.ln(),
                "exp" => x.exp(),
                "sin" => x.sin(),
                _ => x.abs(),
            })
        }
    }
}

proptest! {
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
            Some(v) => prop_assert_eq!(got, Ok(Quantity::dimensionless(v)), "{}", src),
            None => prop_assert_eq!(got, Err(EvalError::NotFinite), "{}", src),
        }
    }
}
