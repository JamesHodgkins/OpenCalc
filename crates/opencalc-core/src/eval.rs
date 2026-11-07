//! Dependency-graph evaluation (spec section 3.6).
//!
//! Order comes from the graph, never from layout. Cycles are found with
//! Tarjan's strongly connected components. Tarjan emits components in reverse
//! topological order of the edges, so with edges drawn consumer -> producer the
//! output order is already producers first, which is the evaluation order.

use std::collections::HashMap;

mod graph;
mod node;
mod plot;

pub use plot::{PlotData, PlotSeries};

use graph::tarjan;
use node::{eval_region, Located};

use crate::document::PlotInput;
use crate::expr::{AssignKind, Expr};
use crate::registry::UnitRegistry;
use crate::units::UnitError;
use crate::value::{Value, ValueError};

pub use crate::document::RegionId;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EvalError {
    #[error("undefined name '{0}'")]
    Undefined(String),
    #[error(transparent)]
    Unit(#[from] UnitError),
    #[error(transparent)]
    Value(ValueError),
    #[error("circular reference")]
    Cycle,
    #[error("unknown function '{0}'")]
    UnknownFunction(String),
    #[error("function '{0}' takes {1} argument(s)")]
    Arity(String, usize),
    #[error("cannot evaluate: {0}")]
    Invalid(String),
    /// An error in one part of a plot (its x values or a series).
    #[error("in {part}: {error}")]
    InPlot { part: String, error: Box<EvalError> },
    /// An error inside a user-defined function's body, reported at the call.
    #[error("in function '{name}': {error}")]
    InFunction { name: String, error: Box<EvalError> },
    /// Division by zero, overflow, or a domain error such as `ln(0)`. Never
    /// returned as a number: a silent NaN or infinity is a wrong answer.
    #[error("result is not a finite number")]
    NotFinite,
    /// A bug in the evaluator was caught. The worker survives and reports this.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<ValueError> for EvalError {
    fn from(e: ValueError) -> EvalError {
        match e {
            ValueError::Unit(u) => EvalError::Unit(u),
            other => EvalError::Value(other),
        }
    }
}

type Env = HashMap<String, Result<Value, EvalError>>;

/// A user-defined function: its parameter names and its body.
struct UserFn<'a> {
    params: &'a [String],
    body: &'a Expr,
    /// The definition is part of a circular reference, so calling it is too.
    circular: bool,
}

type Fns<'a> = HashMap<&'a str, UserFn<'a>>;

/// Functions may call other functions this many levels deep. Recursion is not
/// supported (a function that reaches itself is a circular reference), so this
/// only bounds chains of different functions; it keeps the stack use of nested
/// calls inside the worker thread's stack (see the test in `eval::tests`).
const MAX_CALL_DEPTH: usize = 12;

/// What an expression is evaluated against: the defined names, the user's
/// functions, the units, and, inside a call, the function's parameters.
struct Cx<'a> {
    env: &'a Env,
    fns: &'a Fns<'a>,
    units: &'a UnitRegistry,
    /// Parameters of the function being evaluated, innermost scope only. They
    /// shadow everything else.
    locals: Vec<(String, Value)>,
    depth: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Warning {
    /// A name defined by more than one region. The last in layout order wins.
    Redefinition {
        name: String,
        regions: Vec<RegionId>,
    },
}

#[derive(Debug, Default)]
pub struct Output {
    pub values: HashMap<RegionId, Result<Value, EvalError>>,
    pub warnings: Vec<Warning>,
    /// For a region that failed inside its expression, the path to the failing
    /// sub-expression (see [`Expr::at_path`]). Absent when the whole expression
    /// failed at its root, or for a cycle.
    pub error_paths: HashMap<RegionId, Vec<u16>>,
    /// What each plot region draws, or why it cannot.
    pub plots: HashMap<RegionId, Result<PlotData, EvalError>>,
}

/// An output where every region failed with an internal error. Used by the
/// worker when the evaluator itself panics, so a bug shows as an error on each
/// region instead of silently killing evaluation.
pub fn all_failed(regions: &[(RegionId, Expr)], message: &str) -> Output {
    let mut out = Output::default();
    for (id, _) in regions {
        out.values
            .insert(*id, Err(EvalError::Internal(message.to_owned())));
    }
    out
}

/// Evaluate regions given in layout order, with only the built-in units.
pub fn evaluate(regions: &[(RegionId, Expr)]) -> Output {
    evaluate_with(regions, &UnitRegistry::builtin())
}

/// Evaluate regions given in layout order, with the worksheet's units.
pub fn evaluate_with(regions: &[(RegionId, Expr)], units: &UnitRegistry) -> Output {
    evaluate_all(regions, &[], units)
}

/// Evaluate math regions and then the plots that read their results.
pub fn evaluate_all(
    regions: &[(RegionId, Expr)],
    plots: &[(RegionId, PlotInput)],
    units: &UnitRegistry,
) -> Output {
    let mut out = Output::default();

    // Variables and functions are separate namespaces. For each name the last
    // definition in layout order wins.
    let mut var_definers: HashMap<&str, Vec<RegionId>> = HashMap::new();
    let mut fn_definers: HashMap<&str, Vec<RegionId>> = HashMap::new();
    for (id, expr) in regions {
        match expr {
            Expr::Assign {
                kind: AssignKind::Define,
                target,
                ..
            } => var_definers.entry(target.as_str()).or_default().push(*id),
            Expr::FunctionDef { name, .. } => {
                fn_definers.entry(name.as_str()).or_default().push(*id)
            }
            _ => {}
        }
    }
    for definers in [&var_definers, &fn_definers] {
        let mut names: Vec<&&str> = definers.keys().collect();
        names.sort();
        for name in names {
            let ids = &definers[*name];
            if ids.len() > 1 {
                out.warnings.push(Warning::Redefinition {
                    name: (**name).to_owned(),
                    regions: ids.clone(),
                });
            }
        }
    }
    let last = |definers: &HashMap<&str, Vec<RegionId>>| -> HashMap<String, RegionId> {
        definers
            .iter()
            .filter_map(|(n, ids)| ids.last().map(|id| ((*n).to_owned(), *id)))
            .collect()
    };
    let (var_winner, fn_winner) = (last(&var_definers), last(&fn_definers));

    // Edges: region index -> indices of the regions it depends on: the definers
    // of the variables it reads and of the functions it calls.
    let index_of: HashMap<RegionId, usize> = regions
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();
    let edges: Vec<Vec<usize>> = regions
        .iter()
        .map(|(_, e)| {
            let variables = e.free_names();
            let functions = e.called_names();
            let reads = variables.iter().filter_map(|n| var_winner.get(n));
            let calls = functions.iter().filter_map(|n| fn_winner.get(n));
            reads.chain(calls).map(|id| index_of[id]).collect()
        })
        .collect();

    let components = tarjan(&edges);
    let circular = |i: usize, component: &[usize]| component.len() > 1 || edges[i].contains(&i);
    let in_cycle: Vec<bool> = {
        let mut flags = vec![false; regions.len()];
        for component in &components {
            for &i in component {
                flags[i] = circular(i, component);
            }
        }
        flags
    };

    // Winning definitions of functions, ready to be called.
    let fns: Fns = regions
        .iter()
        .enumerate()
        .filter_map(|(i, (id, e))| match e {
            Expr::FunctionDef { name, params, body } if fn_winner.get(name) == Some(id) => {
                let circular = in_cycle[i];
                Some((
                    name.as_str(),
                    UserFn {
                        params,
                        body,
                        circular,
                    },
                ))
            }
            _ => None,
        })
        .collect();

    let mut env: Env = HashMap::new();
    for component in &components {
        let cyclic = in_cycle[component[0]];
        for &i in component {
            let (id, expr) = &regions[i];
            let result = if cyclic {
                Err(Located::here(EvalError::Cycle))
            } else {
                let cx = Cx {
                    env: &env,
                    fns: &fns,
                    units,
                    locals: Vec::new(),
                    depth: 0,
                };
                eval_region(expr, &cx)
            };
            let result = result.map_err(|l| {
                if !l.rev_path.is_empty() {
                    out.error_paths
                        .insert(*id, l.rev_path.iter().rev().copied().collect());
                }
                l.error
            });
            if let Expr::Assign {
                kind: AssignKind::Define,
                target,
                ..
            } = expr
            {
                if var_winner.get(target.as_str()) == Some(id) {
                    env.insert(target.clone(), result.clone());
                }
            }
            out.values.insert(*id, result);
        }
    }
    // Plots define nothing, so they are evaluated last, against the final names.
    let cx = Cx {
        env: &env,
        fns: &fns,
        units,
        locals: Vec::new(),
        depth: 0,
    };
    for (id, input) in plots {
        out.plots.insert(*id, plot::evaluate_plot(input, &cx));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::BinaryOp::*;
    use crate::units::{Dimension, Quantity};

    fn id(n: u64) -> RegionId {
        RegionId(n)
    }

    fn si(out: &Output, n: u64) -> f64 {
        out.values[&id(n)].as_ref().unwrap().as_real().unwrap().si
    }

    #[test]
    fn region_can_use_a_name_defined_below_it() {
        let regions = vec![
            (
                id(1),
                Expr::evaluate("y", Expr::bin(Mul, Expr::name("x"), Expr::num(2.0))),
            ),
            (id(2), Expr::define("x", Expr::num(21.0))),
        ];
        assert_eq!(si(&evaluate(&regions), 1), 42.0);
    }

    #[test]
    fn layout_order_does_not_change_results() {
        let a = (id(1), Expr::define("a", Expr::num(3.0)));
        let b = (
            id(2),
            Expr::define("b", Expr::bin(Add, Expr::name("a"), Expr::num(1.0))),
        );
        let c = (
            id(3),
            Expr::define("c", Expr::bin(Mul, Expr::name("b"), Expr::name("a"))),
        );
        let fwd = evaluate(&[a.clone(), b.clone(), c.clone()]);
        let rev = evaluate(&[c, b, a]);
        assert_eq!(si(&fwd, 3), 12.0);
        assert_eq!(si(&rev, 3), 12.0);
    }

    #[test]
    fn unit_mismatch_is_an_error_and_propagates() {
        let regions = vec![
            (
                id(1),
                Expr::define(
                    "bad",
                    Expr::bin(Add, Expr::qty(1.0, "m"), Expr::qty(1.0, "kg")),
                ),
            ),
            (
                id(2),
                Expr::evaluate("r", Expr::bin(Mul, Expr::name("bad"), Expr::num(2.0))),
            ),
        ];
        let out = evaluate(&regions);
        assert!(matches!(
            out.values[&id(1)],
            Err(EvalError::Unit(UnitError::Mismatch(..)))
        ));
        assert!(out.values[&id(2)].is_err());
    }

    #[test]
    fn cycle_marks_every_region_in_it() {
        let regions = vec![
            (id(1), Expr::define("a", Expr::name("b"))),
            (id(2), Expr::define("b", Expr::name("a"))),
            (id(3), Expr::define("ok", Expr::num(1.0))),
        ];
        let out = evaluate(&regions);
        assert_eq!(out.values[&id(1)], Err(EvalError::Cycle));
        assert_eq!(out.values[&id(2)], Err(EvalError::Cycle));
        assert!(out.values[&id(3)].is_ok());
    }

    #[test]
    fn self_reference_is_a_cycle() {
        let regions = vec![(
            id(1),
            Expr::define("a", Expr::bin(Add, Expr::name("a"), Expr::num(1.0))),
        )];
        assert_eq!(evaluate(&regions).values[&id(1)], Err(EvalError::Cycle));
    }

    #[test]
    fn redefinition_warns_and_last_wins() {
        let regions = vec![
            (id(1), Expr::define("x", Expr::num(1.0))),
            (id(2), Expr::define("x", Expr::num(2.0))),
            (id(3), Expr::evaluate("r", Expr::name("x"))),
        ];
        let out = evaluate(&regions);
        assert_eq!(si(&out, 3), 2.0);
        assert_eq!(
            out.warnings,
            vec![Warning::Redefinition {
                name: "x".into(),
                regions: vec![id(1), id(2)]
            }]
        );
    }

    #[test]
    fn undefined_name_is_reported() {
        let regions = vec![(id(1), Expr::evaluate("r", Expr::name("nope")))];
        assert_eq!(
            evaluate(&regions).values[&id(1)],
            Err(EvalError::Undefined("nope".into()))
        );
    }

    #[test]
    fn stress_over_units_gives_pressure_in_mpa() {
        let regions = vec![
            (id(1), Expr::define("F", Expr::qty(10.0, "kN"))),
            (
                id(2),
                Expr::define("A", Expr::bin(Pow, Expr::qty(0.01, "m"), Expr::num(2.0))),
            ),
            (
                id(3),
                Expr::evaluate("s", Expr::bin(Div, Expr::name("F"), Expr::name("A"))),
            ),
        ];
        let q = evaluate(&regions).values[&id(3)]
            .as_ref()
            .unwrap()
            .as_real()
            .unwrap();
        assert!((q.in_unit("MPa").unwrap() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn transcendental_function_rejects_dimensioned_argument() {
        let regions = vec![(
            id(1),
            Expr::evaluate(
                "r",
                Expr::Call {
                    name: "sin".into(),
                    args: vec![Expr::qty(1.0, "m")],
                },
            ),
        )];
        assert!(evaluate(&regions).values[&id(1)].is_err());
    }

    fn run_value(src: &str) -> Result<Value, EvalError> {
        let regions = vec![(id(1), crate::parser::parse(src))];
        evaluate(&regions).values.remove(&id(1)).unwrap()
    }

    /// A real result; a complex or matrix result is an error here.
    fn run_src(src: &str) -> Result<Quantity, EvalError> {
        run_value(src).and_then(|v| {
            v.as_real()
                .ok_or_else(|| EvalError::Invalid("not a real number".into()))
        })
    }

    #[test]
    fn exponent_overflow_from_typed_input_is_an_error() {
        for src in [
            "(1 m^2)^100",
            "(1 m)^100 * (1 m)^100",
            "(1 m)^100 / (1 m)^(0 - 100)",
        ] {
            assert_eq!(
                run_src(src),
                Err(EvalError::Unit(UnitError::DimensionOverflow)),
                "{src}"
            );
        }
    }

    #[test]
    fn non_finite_results_are_errors_never_numbers() {
        for src in [
            "1 / 0",
            "0 / 0",
            "ln(0)",
            "10 ^ 400",
            "1e999",
            "sqrt(1 / 0)",
            "(1 / 0) * i",
        ] {
            assert_eq!(run_src(src), Err(EvalError::NotFinite), "{src}");
        }
    }

    #[test]
    fn non_finite_intermediate_results_are_caught_too() {
        assert_eq!(run_src("10 ^ 400 - 10 ^ 400"), Err(EvalError::NotFinite));
        // And it propagates to a dependent region instead of being displayed.
        let regions = vec![
            (id(1), crate::parser::parse("a := 1 / 0")),
            (id(2), crate::parser::parse("b := a + 1")),
        ];
        let out = evaluate(&regions);
        assert_eq!(out.values[&id(2)], Err(EvalError::NotFinite));
    }

    #[test]
    fn ordinary_arithmetic_is_unaffected_by_the_finite_check() {
        assert_eq!(run_src("2 ^ 10").map(|q| q.si), Ok(1024.0));
        assert_eq!(run_src("0 * 5").map(|q| q.si), Ok(0.0));
        assert!(run_src("1e300 * 1e7").is_ok_and(|q| q.si > 9.9e306 && q.si < 1.1e307));
    }

    #[test]
    fn a_worksheets_own_units_work_after_a_number_and_as_a_name() {
        use crate::units::UnitDef;
        let units = UnitRegistry::new(vec![UnitDef {
            symbol: "furlong".into(),
            dimension: Dimension::LENGTH,
            to_si_scale: 201.168,
            definition: String::new(),
        }])
        .unwrap();
        let regions = vec![
            (id(1), crate::parser::parse("a := 2 furlong")),
            (id(2), crate::parser::parse("b := a + 1 furlong")),
            (id(3), crate::parser::parse("c := b / 1 m")),
        ];
        let out = evaluate_with(&regions, &units);
        let c = out.values[&id(3)].as_ref().unwrap().as_real().unwrap();
        assert!((c.si - 3.0 * 201.168).abs() < 1e-9 && c.dim == Dimension::NONE);
        // Without the definition the same worksheet reports the unit as undefined.
        assert!(matches!(
            evaluate(&regions).values[&id(1)],
            Err(EvalError::Undefined(_))
        ));
    }

    #[test]
    fn a_variable_shadows_a_custom_unit_of_the_same_name() {
        use crate::units::UnitDef;
        let units = UnitRegistry::new(vec![UnitDef {
            symbol: "w".into(),
            dimension: Dimension::LENGTH,
            to_si_scale: 10.0,
            definition: String::new(),
        }])
        .unwrap();
        let regions = vec![
            (id(1), crate::parser::parse("w := 3")),
            (id(2), crate::parser::parse("r := 2 w")),
        ];
        let out = evaluate_with(&regions, &units);
        assert_eq!(si(&out, 2), 6.0);
    }

    /// Source text of the sub-expression an error points at, via its path.
    fn failing_part(lines: &[&str], which: u64) -> Option<String> {
        let regions: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, s)| (id(i as u64 + 1), crate::parser::parse(s)))
            .collect();
        let out = evaluate(&regions);
        assert!(out.values[&id(which)].is_err(), "{lines:?} should fail");
        let path = out.error_paths.get(&id(which))?;
        let expr = &regions[which as usize - 1].1;
        let part = expr
            .at_path(path)
            .expect("a path from the evaluator resolves");
        Some(format!("{part:?}"))
    }

    #[test]
    fn an_error_points_at_the_part_of_the_expression_that_failed() {
        let part = |lines: &[&str], which| failing_part(lines, which).unwrap();
        // The whole sum fails (mismatch); it is the value of the definition.
        assert!(part(&["x := 1 m + 2 kg"], 1).starts_with("Binary { op: Add"));
        // The division inside the root, not the root.
        assert!(part(&["x := sqrt(1 / 0)"], 1).starts_with("Binary { op: Div"));
        // The right operand of the product.
        assert!(part(&["y := 2 * (3 m + 1 s)"], 1).starts_with("Binary { op: Add"));
        // A name that is undefined is itself the failing part.
        assert_eq!(part(&["z := 1 + nope"], 1), "Name(\"nope\")");
        // A region using a failed region points at the name that carries the error.
        assert_eq!(part(&["a := 1 / 0", "b := a + 1"], 2), "Name(\"a\")");
    }

    #[test]
    fn a_failure_at_the_root_or_in_a_cycle_has_no_path() {
        let regions = vec![
            (id(1), crate::parser::parse("a := b")),
            (id(2), crate::parser::parse("b := a")),
            (id(3), crate::parser::parse("1 +")),
        ];
        let out = evaluate(&regions);
        assert!(out.error_paths.is_empty(), "{:?}", out.error_paths);
    }

    #[test]
    fn a_successful_region_has_no_error_path() {
        let out = evaluate(&[(id(1), crate::parser::parse("x := 1 m"))]);
        assert!(out.error_paths.is_empty());
    }

    #[test]
    fn all_failed_marks_every_region_with_the_message() {
        let regions = vec![(id(1), Expr::num(1.0)), (id(2), Expr::num(2.0))];
        let out = all_failed(&regions, "boom");
        assert_eq!(out.values.len(), 2);
        for id in [id(1), id(2)] {
            assert_eq!(out.values[&id], Err(EvalError::Internal("boom".into())));
        }
    }

    #[test]
    fn sqrt_of_area_is_length() {
        let regions = vec![(
            id(1),
            Expr::evaluate(
                "r",
                Expr::Root {
                    radicand: Box::new(Expr::bin(Pow, Expr::qty(3.0, "m"), Expr::num(2.0))),
                },
            ),
        )];
        let q = evaluate(&regions).values[&id(1)]
            .as_ref()
            .unwrap()
            .as_real()
            .unwrap();
        assert_eq!(q.dim, Dimension::LENGTH);
        assert!((q.si - 3.0).abs() < 1e-12);
    }

    /// A chain of `levels` functions, each body the deepest expression allowed
    /// (a run of negations) around a call of the previous one.
    fn deep_chain(levels: usize) -> Vec<(RegionId, Expr)> {
        let minuses = "-".repeat(crate::parser::MAX_SYMBOLS - 12);
        let mut lines = vec!["f0(x) := x".to_owned()];
        for k in 1..levels {
            lines.push(format!("f{k}(x) := {minuses}f{}(x)", k - 1));
        }
        lines.push(format!("answer := f{}(1)", levels - 1));
        lines
            .iter()
            .enumerate()
            .map(|(i, l)| (id(i as u64 + 1), crate::parser::parse(l)))
            .collect()
    }

    /// Evaluate on a thread with a stack of `kb` kilobytes.
    fn evaluate_on_stack(regions: Vec<(RegionId, Expr)>, kb: usize) -> Output {
        std::thread::Builder::new()
            .stack_size(kb * 1024)
            .spawn(move || evaluate(&regions))
            .unwrap()
            .join()
            .unwrap()
    }

    #[test]
    fn nested_calls_at_the_depth_limit_fit_in_half_the_worker_stack() {
        // The worker thread has 8 MB (see `worker::WORKER_STACK`); this must fit in
        // half of it, in a debug build, which uses the most stack.
        let regions = deep_chain(MAX_CALL_DEPTH);
        let last = id(regions.len() as u64);
        let out = evaluate_on_stack(regions, 4 * 1024);
        // Even number of negations per level, so the value is unchanged.
        assert_eq!(si(&out, last.0), 1.0);
    }

    #[test]
    fn calls_nested_past_the_limit_are_an_error_not_a_crash() {
        let regions = deep_chain(MAX_CALL_DEPTH + 2);
        let last = id(regions.len() as u64);
        let out = evaluate_on_stack(regions, 4 * 1024);
        assert!(matches!(
            &out.values[&last],
            Err(EvalError::InFunction { .. })
        ));
    }
}
