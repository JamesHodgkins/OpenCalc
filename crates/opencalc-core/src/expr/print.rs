//! Printing an expression back as source text.

use super::{AssignKind, BinaryOp, Expr};

/// Binding strength of an expression when printed, higher binds tighter.
fn strength(e: &Expr) -> u8 {
    match e {
        Expr::Assign { .. } | Expr::FunctionDef { .. } | Expr::Range { .. } => 0,
        Expr::Binary {
            op:
                BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge,
            ..
        } => 1,
        Expr::Binary {
            op: BinaryOp::Add | BinaryOp::Sub,
            ..
        } => 2,
        Expr::Binary {
            op: BinaryOp::Mul | BinaryOp::Div,
            ..
        }
        | Expr::Fraction { .. } => 3,
        Expr::Unary { .. } => 4,
        Expr::Binary {
            op: BinaryOp::Pow, ..
        } => 5,
        // `5 kN` reads as one unit of meaning but must be bracketed under `^`.
        Expr::Quantity { .. } => 5,
        _ => 6,
    }
}

fn operand(f: &mut std::fmt::Formatter<'_>, e: &Expr, at_least: u8) -> std::fmt::Result {
    if strength(e) < at_least {
        write!(f, "({e})")
    } else {
        write!(f, "{e}")
    }
}

fn list(f: &mut std::fmt::Formatter<'_>, items: &[Expr]) -> std::fmt::Result {
    for (i, a) in items.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{a}")?;
    }
    Ok(())
}

impl BinaryOp {
    /// The operator as written, and the least strength each side may have
    /// without brackets.
    fn written(self) -> (&'static str, u8, u8) {
        match self {
            BinaryOp::Eq => ("==", 1, 2),
            BinaryOp::Ne => ("!=", 1, 2),
            BinaryOp::Lt => ("<", 1, 2),
            BinaryOp::Gt => (">", 1, 2),
            BinaryOp::Le => ("<=", 1, 2),
            BinaryOp::Ge => (">=", 1, 2),
            BinaryOp::Add => ("+", 2, 3),
            BinaryOp::Sub => ("-", 2, 3),
            BinaryOp::Mul => ("*", 3, 4),
            BinaryOp::Div => ("/", 3, 4),
            // Power is right associative; its operands are always bracketed
            // unless they are atoms, which is never ambiguous.
            BinaryOp::Pow => ("^", 6, 6),
        }
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
                operand(f, arg, 5)
            }
            Expr::Binary { op, lhs, rhs } => {
                let (symbol, left, right) = op.written();
                operand(f, lhs, left)?;
                write!(f, " {symbol} ")?;
                operand(f, rhs, right)
            }
            Expr::Fraction { num, den } => {
                operand(f, num, 3)?;
                f.write_str(" / ")?;
                operand(f, den, 4)
            }
            Expr::Root { radicand } => write!(f, "sqrt({radicand})"),
            Expr::Call { name, args } => {
                write!(f, "{name}(")?;
                list(f, args)?;
                f.write_str(")")
            }
            Expr::Matrix { cols, cells, .. } => {
                f.write_str("[")?;
                for (i, row) in cells.chunks(*cols).enumerate() {
                    if i > 0 {
                        f.write_str("; ")?;
                    }
                    list(f, row)?;
                }
                f.write_str("]")
            }
            Expr::Index { base, indices } => {
                operand(f, base, 6)?;
                f.write_str("[")?;
                list(f, indices)?;
                f.write_str("]")
            }
            Expr::Range { start, step, end } => {
                // Written without brackets at the top; bracketed as an operand.
                operand(f, start, 1)?;
                if let Some(step) = step {
                    f.write_str(", ")?;
                    operand(f, step, 1)?;
                }
                f.write_str("..")?;
                operand(f, end, 1)
            }
            Expr::Conditional {
                cond,
                then,
                otherwise,
            } => write!(f, "if({cond}, {then}, {otherwise})"),
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
            Expr::FunctionDef { name, params, body } => {
                write!(f, "{name}({}) := {body}", params.join(", "))
            }
            Expr::Error(e) => write!(f, "? {e}"),
        }
    }
}
