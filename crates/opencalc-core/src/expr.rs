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
    Error(String),
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
