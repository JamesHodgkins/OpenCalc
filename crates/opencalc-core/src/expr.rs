//! Expression AST (spec section 3.3). Spike subset: real numbers, names,
//! quantities, operators, calls and assignment. Layout nodes (fraction, root)
//! are included because the typesetting prototype needs them.

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignKind {
    /// `:=`
    Define,
    /// `=`
    Evaluate,
}

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
            Expr::Assign { value, .. } => vec![value],
            Expr::Number(_) | Expr::Name(_) | Expr::Quantity { .. } | Expr::Error(_) => vec![],
        }
    }

    /// The sub-expression at `path`, a list of child numbers from this node.
    pub fn at_path(&self, path: &[u16]) -> Option<&Expr> {
        match path.split_first() {
            None => Some(self),
            Some((first, rest)) => self.children().get(usize::from(*first))?.at_path(rest),
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
            Expr::Call { args, .. } => args.iter().for_each(|a| a.collect_names(out)),
            Expr::Assign { value, .. } => value.collect_names(out),
            Expr::Fraction { num, den } => {
                num.collect_names(out);
                den.collect_names(out);
            }
            Expr::Root { radicand } => radicand.collect_names(out),
            Expr::Number(_) | Expr::Quantity { .. } | Expr::Error(_) => {}
        }
    }
}

/// Binding strength of an expression when printed, higher binds tighter.
fn strength(e: &Expr) -> u8 {
    match e {
        Expr::Binary {
            op: BinaryOp::Add | BinaryOp::Sub,
            ..
        } => 1,
        Expr::Binary {
            op: BinaryOp::Mul | BinaryOp::Div,
            ..
        }
        | Expr::Fraction { .. } => 2,
        Expr::Unary { .. } => 3,
        Expr::Binary {
            op: BinaryOp::Pow, ..
        } => 4,
        Expr::Assign { .. } => 0,
        // `5 kN` reads as one unit of meaning but must be bracketed under `^`.
        Expr::Quantity { .. } => 4,
        _ => 5,
    }
}

fn operand(f: &mut std::fmt::Formatter<'_>, e: &Expr, at_least: u8) -> std::fmt::Result {
    if strength(e) < at_least {
        write!(f, "({e})")
    } else {
        write!(f, "{e}")
    }
}

/// The expression as source text that parses back to the same tree (apart from
/// layout-only nodes, which print as the operator they stand for). Used to show
/// the part of an expression an error points at.
impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Number(v) => write!(f, "{v}"),
            Expr::Name(n) => f.write_str(n),
            Expr::Quantity { value, unit } => write!(f, "{value} {unit}"),
            Expr::Unary { arg, .. } => {
                f.write_str("-")?;
                operand(f, arg, 4)
            }
            Expr::Binary { op, lhs, rhs } => {
                let (symbol, left, right) = match op {
                    BinaryOp::Add => ("+", 1, 2),
                    BinaryOp::Sub => ("-", 1, 2),
                    BinaryOp::Mul => ("*", 2, 3),
                    BinaryOp::Div => ("/", 2, 3),
                    // Power is right associative; its operands are always bracketed
                    // unless they are atoms, which is never ambiguous.
                    BinaryOp::Pow => ("^", 5, 5),
                };
                operand(f, lhs, left)?;
                write!(f, " {symbol} ")?;
                operand(f, rhs, right)
            }
            Expr::Fraction { num, den } => {
                operand(f, num, 2)?;
                f.write_str(" / ")?;
                operand(f, den, 3)
            }
            Expr::Root { radicand } => write!(f, "sqrt({radicand})"),
            Expr::Call { name, args } => {
                write!(f, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{a}")?;
                }
                f.write_str(")")
            }
            Expr::Assign {
                kind,
                target,
                value,
            } => {
                let op = match kind {
                    AssignKind::Define => ":=",
                    AssignKind::Evaluate => "=",
                };
                write!(f, "{target} {op} {value}")
            }
            Expr::Error(e) => write!(f, "? {e}"),
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
