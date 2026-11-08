//! Expression AST (spec section 3.3). Spike subset: real numbers, names,
//! quantities, operators, calls and assignment. Layout nodes (fraction, root)
//! are included because the typesetting prototype needs them.

mod print;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    /// Comparisons give 1 (true) or 0 (false), dimensionless, as in Mathcad.
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignKind {
    /// `:=`
    Define,
    /// `=`
    Evaluate,
}

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
];

pub fn is_binding_form(name: &str) -> bool {
    BINDING_FORMS.contains(&name)
}

/// The name of the variable that sets where indexing starts, as in Mathcad.
pub const ORIGIN: &str = "ORIGIN";

/// A parse error with the part of the source it points at, as a half-open range
/// of characters (not bytes). An error at the end of the input has an empty range
/// at the end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

impl ParseError {
    pub fn new(message: String, start: usize, end: usize) -> ParseError {
        ParseError {
            message,
            start,
            end,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Number(f64),
    Name(String),
    /// A number with a unit symbol, such as `5 kN`.
    Quantity {
        value: f64,
        unit: String,
    },
    Unary {
        op: UnaryOp,
        arg: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Assign {
        kind: AssignKind,
        target: String,
        value: Box<Expr>,
    },
    /// A matrix or vector written out: `[1, 2; 3, 4]`. Cells are row-major.
    Matrix {
        rows: usize,
        cols: usize,
        cells: Vec<Expr>,
    },
    /// `base[i]` or `base[i, j]`. Counting starts at `ORIGIN` (default 0).
    Index {
        base: Box<Expr>,
        indices: Vec<Expr>,
    },
    /// `f(x, y) := body`. Only ever the top of a region. The body is not evaluated
    /// here; it runs, with the parameters bound, each time `f` is called.
    FunctionDef {
        name: String,
        params: Vec<String>,
        body: Box<Expr>,
    },
    /// `start..end`, or `start, next..end`, where `next - start` is the step.
    Range {
        start: Box<Expr>,
        step: Option<Box<Expr>>,
        end: Box<Expr>,
    },
    /// `if(cond, then, otherwise)`; only the chosen branch is evaluated.
    Conditional {
        cond: Box<Expr>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
    },
    // Layout-bearing nodes: typeset as written, evaluated as the plain operator.
    Fraction {
        num: Box<Expr>,
        den: Box<Expr>,
    },
    Root {
        radicand: Box<Expr>,
    },
    /// Parse errors are kept in the tree so the UI can show them.
    Error(ParseError),
    /// A missing operand. Only `parse_lenient` makes these (the editor's display
    /// tree); `parse` never does, so saved data and the evaluator never see one.
    Hole,
    /// Text the lenient parser could not use, drawn as plain text after `head`
    /// (the part before it that did parse, if any). Display tree only, like `Hole`.
    Garbage {
        head: Option<Box<Expr>>,
        text: String,
    },
}

impl Expr {
    pub fn num(v: f64) -> Expr {
        Expr::Number(v)
    }

    pub fn name(n: &str) -> Expr {
        Expr::Name(n.to_owned())
    }

    pub fn qty(value: f64, unit: &str) -> Expr {
        Expr::Quantity {
            value,
            unit: unit.to_owned(),
        }
    }

    pub fn bin(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn define(target: &str, value: Expr) -> Expr {
        Expr::Assign {
            kind: AssignKind::Define,
            target: target.to_owned(),
            value: Box::new(value),
        }
    }

    pub fn evaluate(target: &str, value: Expr) -> Expr {
        Expr::Assign {
            kind: AssignKind::Evaluate,
            target: target.to_owned(),
            value: Box::new(value),
        }
    }

    /// The direct sub-expressions, in the order evaluation numbers them: the
    /// operand of a negation or root, left then right of a binary operator or
    /// fraction, the arguments of a call, the value of an assignment.
    pub fn children(&self) -> Vec<&Expr> {
        match self {
            Expr::Unary { arg, .. } => vec![arg],
            Expr::Binary { lhs, rhs, .. } => vec![lhs, rhs],
            Expr::Fraction { num, den } => vec![num, den],
            Expr::Root { radicand } => vec![radicand],
            Expr::Call { args, .. } => args.iter().collect(),
            Expr::Matrix { cells, .. } => cells.iter().collect(),
            Expr::Range { start, step, end } => std::iter::once(&**start)
                .chain(step.as_deref())
                .chain(std::iter::once(&**end))
                .collect(),
            Expr::Index { base, indices } => std::iter::once(&**base).chain(indices).collect(),
            Expr::Conditional {
                cond,
                then,
                otherwise,
            } => vec![cond, then, otherwise],
            Expr::Assign { value, .. } => vec![value],
            Expr::FunctionDef { body, .. } => vec![body],
            Expr::Garbage { head, .. } => head.as_deref().into_iter().collect(),
            Expr::Number(_)
            | Expr::Name(_)
            | Expr::Quantity { .. }
            | Expr::Error(_)
            | Expr::Hole => vec![],
        }
    }

    /// The sub-expression at `path`, a list of child numbers from this node.
    pub fn at_path(&self, path: &[u16]) -> Option<&Expr> {
        match path.split_first() {
            None => Some(self),
            Some((first, rest)) => self.children().get(usize::from(*first))?.at_path(rest),
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn printed_expressions_parse_back_to_the_same_tree() {
        for src in [
            "1 + 2 * 3",
            "(1 + 2) * 3",
            "1 - (2 - 3)",
            "1 - 2 - 3",
            "2 ^ 3 ^ 2",
            "(2 ^ 3) ^ 2",
            "-(2 ^ 2)",
            "(-2) ^ 2",
            "2 * -3",
            "a / (b * c)",
            "a / b * c",
            "sqrt(x ^ 2 + y ^ 2)",
            "5 kN / (2 m) ^ 2",
            "F := 10 kN",
            "x =",
            "vec(1, 2 m, a + b)",
            "1e300 * 1e-300",
            "0.1 + 0.2",
            "[1, 2; 3, 4]",
            "[1; 2 m; 3]",
            "v[0]",
            "M[i + 1, j]",
            "M[0][1]",
            "(a + b)[0]",
            "-v[0] ^ 2",
            "if(x > 0, x, -x)",
            "if(a, if(b, 1, 2), 3)",
            "a == b",
            "a + 1 < b * 2",
            "(a < b) == (c > d)",
            "a - (b < c)",
            "2 * (a == b)",
            "[a < b, c]",
            "f(x) := x ^ 2 + 1",
            "area(w, h) := if(w > h, w, h)",
            "g(a) := 0..a",
        ] {
            let e = parse(src);
            assert!(!matches!(e, Expr::Error(_)), "{src}");
            assert_eq!(parse(&e.to_string()), e, "{src} printed as {e}");
        }
    }

    #[test]
    fn children_and_paths_agree() {
        let e = parse("y := f(a, b + 1) * 2");
        assert_eq!(e.at_path(&[]), Some(&e));
        // value, then the left factor, then the second argument of f.
        assert_eq!(
            e.at_path(&[0, 0, 1]).map(ToString::to_string).as_deref(),
            Some("b + 1")
        );
        assert_eq!(
            e.at_path(&[0, 1]).map(ToString::to_string).as_deref(),
            Some("2")
        );
        assert_eq!(e.at_path(&[0, 0, 5]), None);
        assert_eq!(e.at_path(&[1]), None);
    }
}
