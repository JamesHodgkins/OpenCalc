//! Dependency-graph evaluation (spec section 3.6).
//!
//! Order comes from the graph, never from layout. Cycles are found with
//! Tarjan's strongly connected components. Tarjan emits components in reverse
//! topological order of the edges, so with edges drawn consumer -> producer the
//! output order is already producers first, which is the evaluation order.

use std::collections::HashMap;

use crate::expr::{AssignKind, BinaryOp, Expr, UnaryOp};
use crate::units::{Dimension, Quantity, UnitError};

pub use crate::document::RegionId;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EvalError {
    #[error("undefined name '{0}'")]
    Undefined(String),
    #[error(transparent)]
    Unit(#[from] UnitError),
    #[error("circular reference")]
    Cycle,
    #[error("unknown function '{0}'")]
    UnknownFunction(String),
    #[error("function '{0}' takes {1} argument(s)")]
    Arity(String, usize),
    #[error("cannot evaluate: {0}")]
    Invalid(String),
    /// Division by zero, overflow, or a domain error such as `sqrt(-4)` or `ln(0)`.
    /// Never returned as a number: a silent NaN or infinity is a wrong answer.
    /// Revisit when complex numbers arrive (`sqrt(-4)` becomes `2i`).
    #[error("result is not a finite number")]
    NotFinite,
    /// A bug in the evaluator was caught. The worker survives and reports this.
    #[error("internal error: {0}")]
    Internal(String),
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
    pub values: HashMap<RegionId, Result<Quantity, EvalError>>,
    pub warnings: Vec<Warning>,
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

/// Evaluate regions given in layout order.
pub fn evaluate(regions: &[(RegionId, Expr)]) -> Output {
    let mut out = Output::default();

    // name -> definer; last definition in layout order wins.
    let mut definers: HashMap<&str, Vec<RegionId>> = HashMap::new();
    for (id, expr) in regions {
        if let Expr::Assign {
            kind: AssignKind::Define,
            target,
            ..
        } = expr
        {
            definers.entry(target.as_str()).or_default().push(*id);
        }
    }
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
    let winner: HashMap<&str, RegionId> = definers
        .iter()
        .filter_map(|(n, ids)| ids.last().map(|id| (*n, *id)))
        .collect();

    // Edges: region index -> indices of the regions it depends on.
    let index_of: HashMap<RegionId, usize> = regions
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();
    let edges: Vec<Vec<usize>> = regions
        .iter()
        .map(|(_, e)| {
            e.free_names()
                .iter()
                .filter_map(|n| winner.get(n.as_str()))
                .map(|id| index_of[id])
                .collect()
        })
        .collect();

    let mut env: HashMap<String, Result<Quantity, EvalError>> = HashMap::new();
    for component in tarjan(&edges) {
        let cyclic = component.len() > 1 || edges[component[0]].contains(&component[0]);
        for &i in &component {
            let (id, expr) = &regions[i];
            let result = if cyclic {
                Err(EvalError::Cycle)
            } else {
                eval_region(expr, &env)
            };
            if let Expr::Assign {
                kind: AssignKind::Define,
                target,
                ..
            } = expr
            {
                if winner.get(target.as_str()) == Some(id) {
                    env.insert(target.clone(), result.clone());
                }
            }
            out.values.insert(*id, result);
        }
    }
    out
}

fn eval_region(
    expr: &Expr,
    env: &HashMap<String, Result<Quantity, EvalError>>,
) -> Result<Quantity, EvalError> {
    match expr {
        Expr::Assign { value, .. } => eval(value, env),
        other => eval(other, env),
    }
}

/// Evaluate one expression. The single choke point that rejects non-finite
/// results, so they cannot reach a displayed value.
fn eval(
    expr: &Expr,
    env: &HashMap<String, Result<Quantity, EvalError>>,
) -> Result<Quantity, EvalError> {
    let q = eval_node(expr, env)?;
    if q.si.is_finite() {
        Ok(q)
    } else {
        Err(EvalError::NotFinite)
    }
}

fn eval_node(
    expr: &Expr,
    env: &HashMap<String, Result<Quantity, EvalError>>,
) -> Result<Quantity, EvalError> {
    match expr {
        Expr::Number(v) => Ok(Quantity::dimensionless(*v)),
        Expr::Quantity { value, unit } => Ok(Quantity::from_unit(*value, unit)?),
        // A defined variable shadows a unit of the same name.
        Expr::Name(n) => match env.get(n) {
            Some(r) => r.clone(),
            None => Quantity::from_unit(1.0, n).map_err(|_| EvalError::Undefined(n.clone())),
        },
        Expr::Unary {
            op: UnaryOp::Neg,
            arg,
        } => Ok(eval(arg, env)?.negated()),
        Expr::Binary { op, lhs, rhs } => binary(*op, eval(lhs, env)?, eval(rhs, env)?),
        Expr::Fraction { num, den } => binary(BinaryOp::Div, eval(num, env)?, eval(den, env)?),
        Expr::Root { radicand } => sqrt(eval(radicand, env)?),
        Expr::Call { name, args } => call(name, args, env),
        Expr::Assign { value, .. } => eval(value, env),
        Expr::Error(msg) => Err(EvalError::Invalid(msg.clone())),
    }
}

fn binary(op: BinaryOp, a: Quantity, b: Quantity) -> Result<Quantity, EvalError> {
    Ok(match op {
        BinaryOp::Add => a.try_add(b)?,
        BinaryOp::Sub => a.try_sub(b)?,
        BinaryOp::Mul => a.times(b)?,
        BinaryOp::Div => a.over(b)?,
        BinaryOp::Pow => a.pow(b)?,
    })
}

fn sqrt(q: Quantity) -> Result<Quantity, EvalError> {
    match q.dim.halved() {
        Some(dim) => Ok(Quantity {
            si: q.si.sqrt(),
            dim,
        }),
        None => Err(UnitError::NotDimensionless("sqrt".into()).into()),
    }
}

fn call(
    name: &str,
    args: &[Expr],
    env: &HashMap<String, Result<Quantity, EvalError>>,
) -> Result<Quantity, EvalError> {
    let f: fn(f64) -> f64 = match name {
        "sqrt" | "abs" => {
            let [arg] = args else {
                return Err(EvalError::Arity(name.to_owned(), 1));
            };
            let q = eval(arg, env)?;
            return if name == "sqrt" {
                sqrt(q)
            } else {
                Ok(Quantity {
                    si: q.si.abs(),
                    dim: q.dim,
                })
            };
        }
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "ln" => f64::ln,
        "exp" => f64::exp,
        _ => return Err(EvalError::UnknownFunction(name.to_owned())),
    };
    let [arg] = args else {
        return Err(EvalError::Arity(name.to_owned(), 1));
    };
    let q = eval(arg, env)?;
    if q.dim != Dimension::NONE {
        return Err(UnitError::NotDimensionless(name.to_owned()).into());
    }
    Ok(Quantity::dimensionless(f(q.si)))
}

/// Tarjan's SCC over `edges[i]` = nodes that `i` points to. Components are
/// returned in reverse topological order (targets before sources).
fn tarjan(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct State<'a> {
        edges: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<Vec<usize>>,
    }

    fn visit(s: &mut State, v: usize) {
        s.index[v] = Some(s.next);
        s.low[v] = s.next;
        s.next += 1;
        s.stack.push(v);
        s.on_stack[v] = true;
        for k in 0..s.edges[v].len() {
            let w = s.edges[v][k];
            match s.index[w] {
                None => {
                    visit(s, w);
                    s.low[v] = s.low[v].min(s.low[w]);
                }
                Some(wi) if s.on_stack[w] => s.low[v] = s.low[v].min(wi),
                Some(_) => {}
            }
        }
        if Some(s.low[v]) == s.index[v] {
            let mut comp = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on_stack[w] = false;
                comp.push(w);
                if w == v {
                    break;
                }
            }
            comp.reverse();
            s.out.push(comp);
        }
    }

    let n = edges.len();
    let mut s = State {
        edges,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in 0..n {
        if s.index[v].is_none() {
            visit(&mut s, v);
        }
    }
    s.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::BinaryOp::*;

    fn id(n: u64) -> RegionId {
        RegionId(n)
    }

    fn si(out: &Output, n: u64) -> f64 {
        out.values[&id(n)].as_ref().unwrap().si
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
        let q = *evaluate(&regions).values[&id(3)].as_ref().unwrap();
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

    fn run_src(src: &str) -> Result<Quantity, EvalError> {
        let regions = vec![(id(1), crate::parser::parse(src))];
        evaluate(&regions).values.remove(&id(1)).unwrap()
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
            "sqrt(0 - 4)",
            "ln(0)",
            "10 ^ 400",
            "(0 - 8) ^ 0.5",
            "1e999",
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
        let q = *evaluate(&regions).values[&id(1)].as_ref().unwrap();
        assert_eq!(q.dim, Dimension::LENGTH);
        assert!((q.si - 3.0).abs() < 1e-12);
    }
}
