//! What an expression reads and calls: the names its region depends on.
//!
//! This decides the evaluation order, so it must agree with the evaluator about
//! which names a call binds locally (`forms`).

use super::forms::{is_binding_form, is_origin_form, is_solver_form, solver_parts};
use super::{Expr, ORIGIN};

impl Expr {
    /// Names of the functions this expression calls, in first-seen order. The
    /// evaluator uses it to order definitions before their callers.
    pub fn called_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_called(&mut out);
        out
    }

    fn collect_called(&self, out: &mut Vec<String>) {
        if let Expr::Call { name, .. } = self {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        for c in self.children() {
            c.collect_called(out);
        }
    }

    /// Names this expression reads (free variables), in first-seen order.
    pub fn free_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_names(&mut out);
        out
    }

    fn collect_names(&self, out: &mut Vec<String>) {
        match self {
            Expr::Name(n) => {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
            Expr::Unary { arg, .. } => arg.collect_names(out),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.collect_names(out);
                rhs.collect_names(out);
            }
            Expr::Call { name, args } if name == "odesolve" => {
                // The time and state names (arguments 1 and 2) exist only inside f.
                let bound: Vec<&str> = args
                    .iter()
                    .skip(1)
                    .take(2)
                    .filter_map(|a| match a {
                        Expr::Name(n) => Some(n.as_str()),
                        _ => None,
                    })
                    .collect();
                let mut inner = Vec::new();
                if let Some(f) = args.first() {
                    f.collect_names(&mut inner);
                }
                for n in inner {
                    if !bound.contains(&n.as_str()) && !out.contains(&n) {
                        out.push(n);
                    }
                }
                for a in args.iter().skip(3) {
                    a.collect_names(out);
                }
            }
            Expr::Call { name, args } if is_origin_form(name) => {
                args.iter().for_each(|a| a.collect_names(out));
                if !out.iter().any(|n| n == ORIGIN) {
                    out.push(ORIGIN.to_owned());
                }
            }
            Expr::Call { name, args } if is_solver_form(name) => {
                // The unknowns are read from the worksheet as starting values; inside
                // the objective and constraints they are the call's own variables.
                let Ok(parts) = solver_parts(name, args) else {
                    return args.iter().for_each(|a| a.collect_names(out));
                };
                let own: Vec<&str> = parts.unknowns.iter().map(|(_, n)| *n).collect();
                for n in &own {
                    if !out.iter().any(|o| o == n) {
                        out.push((*n).to_owned());
                    }
                }
                let mut inner = Vec::new();
                if let Some(f) = parts.objective {
                    f.collect_names(&mut inner);
                }
                for (_, c) in &parts.constraints {
                    c.collect_names(&mut inner);
                }
                for n in inner {
                    if !own.contains(&n.as_str()) && !out.contains(&n) {
                        out.push(n);
                    }
                }
            }
            Expr::Call { name, args } if is_binding_form(name) => {
                // The bound variable is local: the body does not read it from the
                // worksheet, but the other arguments (the bounds) are outside it.
                let var = match args.get(1) {
                    Some(Expr::Name(v)) => Some(v),
                    _ => None,
                };
                let mut inner = Vec::new();
                if let Some(body) = args.first() {
                    body.collect_names(&mut inner);
                }
                for n in inner {
                    if Some(&n) != var && !out.contains(&n) {
                        out.push(n);
                    }
                }
                for a in args.iter().skip(2) {
                    a.collect_names(out);
                }
            }
            Expr::Call { args, .. } => args.iter().for_each(|a| a.collect_names(out)),
            Expr::Matrix { cells, .. } => cells.iter().for_each(|a| a.collect_names(out)),
            Expr::Range { start, step, end } => {
                start.collect_names(out);
                if let Some(step) = step {
                    step.collect_names(out);
                }
                end.collect_names(out);
            }
            Expr::Index { base, indices } => {
                base.collect_names(out);
                indices.iter().for_each(|a| a.collect_names(out));
                // Indexing counts from ORIGIN, so it depends on it.
                if !out.iter().any(|n| n == ORIGIN) {
                    out.push(ORIGIN.to_owned());
                }
            }
            Expr::Conditional {
                cond,
                then,
                otherwise,
            } => {
                cond.collect_names(out);
                then.collect_names(out);
                otherwise.collect_names(out);
            }
            Expr::Assign { value, .. } => value.collect_names(out),
            // The parameters are the function's own; everything else it reads is
            // a dependency of the definition.
            Expr::FunctionDef { params, body, .. } => {
                let mut inner = Vec::new();
                body.collect_names(&mut inner);
                for n in inner {
                    if !params.contains(&n) && !out.contains(&n) {
                        out.push(n);
                    }
                }
            }
            Expr::Fraction { num, den } => {
                num.collect_names(out);
                den.collect_names(out);
            }
            Expr::Root { radicand } => radicand.collect_names(out),
            Expr::Garbage { head, .. } => {
                if let Some(head) = head {
                    head.collect_names(out);
                }
            }
            Expr::Number(_) | Expr::Quantity { .. } | Expr::Error(_) | Expr::Hole => {}
        }
    }
}
