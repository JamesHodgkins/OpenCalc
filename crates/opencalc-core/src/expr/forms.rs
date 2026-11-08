//! Names of the calls the evaluator handles itself, and how the solver forms'
//! arguments divide up. They matter to more than the evaluator: finding what a
//! region reads (its dependencies) must know which names a call binds locally.

use super::Expr;

/// Functions whose second argument names a variable that exists only inside the
/// call (`integrate(x ^ 2, x, 0, 1)`). The evaluator treats them specially, and
/// that variable is not a name the worksheet has to define.
pub const BINDING_FORMS: &[&str] = &[
    "sum_over",
    "product_over",
    "integrate",
    "derivative",
    "limit",
    "root",
    "find",
    "minerr",
    "minimize",
    "maximize",
];

/// Every call the evaluator handles itself rather than through the function table:
/// the binding forms, the solvers, `odesolve` (two bound names) and `col` / `row`
/// (which count from `ORIGIN`).
pub fn is_evaluator_form(name: &str) -> bool {
    is_binding_form(name) || matches!(name, "odesolve" | "col" | "row")
}

/// Forms whose result depends on `ORIGIN`, so a region using one reads it.
pub fn is_origin_form(name: &str) -> bool {
    matches!(name, "col" | "row")
}

/// The solver forms: unknowns named in the call, constraints as comparisons.
pub const SOLVER_FORMS: &[&str] = &["find", "minerr", "minimize", "maximize"];

pub fn is_solver_form(name: &str) -> bool {
    SOLVER_FORMS.contains(&name)
}

/// How a solver call's arguments divide up.
pub struct SolverParts<'a> {
    /// `minimize` and `maximize` only: the expression to optimise (argument 0).
    pub objective: Option<&'a Expr>,
    /// The unknowns' names, with their argument positions.
    pub unknowns: Vec<(usize, &'a str)>,
    /// The constraints, with their argument positions.
    pub constraints: Vec<(usize, &'a Expr)>,
}

/// Split `find(x, y, c1, c2)` or `minimize(f, x, y, c1)`: the objective (for
/// the optimising forms), then the leading bare names, then the constraints.
pub fn solver_parts<'a>(name: &str, args: &'a [Expr]) -> Result<SolverParts<'a>, String> {
    let optimising = matches!(name, "minimize" | "maximize");
    let start = usize::from(optimising);
    let mut at = start;
    let mut unknowns = Vec::new();
    while let Some(Expr::Name(n)) = args.get(at) {
        unknowns.push((at, n.as_str()));
        at += 1;
    }
    if optimising && args.is_empty() {
        return Err(format!(
            "{name} needs an expression to optimise, then the unknowns"
        ));
    }
    if unknowns.is_empty() {
        return Err(format!("{name} needs the names of the unknowns first"));
    }
    let constraints: Vec<(usize, &Expr)> = args.iter().enumerate().skip(at).collect();
    if !optimising && constraints.is_empty() {
        return Err(format!(
            "{name} needs at least one constraint after the unknowns"
        ));
    }
    Ok(SolverParts {
        objective: optimising.then(|| &args[0]),
        unknowns,
        constraints,
    })
}

pub fn is_binding_form(name: &str) -> bool {
    BINDING_FORMS.contains(&name)
}
