//! Expression AST (spec section 3.3). Spike subset: real numbers, names,
//! quantities, operators, calls and assignment. Layout nodes (fraction, root)
//! are included because the typesetting prototype needs them.

mod forms;
mod names;
mod print;

pub use forms::{
    is_binding_form, is_evaluator_form, is_origin_form, is_solver_form, solver_parts, SolverParts,
    BINDING_FORMS, SOLVER_FORMS,
};
pub use names::assigned_names;

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

/// The parts of a `given` block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GivenBlock {
    /// `find`, `minerr`, `minimize` or `maximize`.
    pub solver: String,
    /// What the closing line names: the unknowns (and the objective, for the
    /// optimising forms).
    pub args: Vec<Expr>,
    pub constraints: Vec<Expr>,
}

impl GivenBlock {
    /// A line of the form `x := 1` is a guess for the unknown `x`, local to the
    /// block; every other line is a constraint.
    fn guess(line: &Expr) -> Option<(&str, &Expr)> {
        match line {
            Expr::Assign {
                kind: AssignKind::Define,
                target,
                value,
            } => Some((target, value)),
            _ => None,
        }
    }

    /// The guesses written in the block: the line each is on, the unknown, its value.
    pub fn guesses(&self) -> impl Iterator<Item = (usize, &str, &Expr)> {
        self.constraints
            .iter()
            .enumerate()
            .filter_map(|(i, line)| GivenBlock::guess(line).map(|(n, v)| (i, n, v)))
    }

    /// The lines that are constraints, by position among the lines.
    pub fn constraint_lines(&self) -> Vec<usize> {
        (0..self.constraints.len())
            .filter(|i| GivenBlock::guess(&self.constraints[*i]).is_none())
            .collect()
    }

    /// The call this block means, without its guesses.
    pub fn lowered(&self) -> Expr {
        let lines = self
            .constraints
            .iter()
            .filter(|line| GivenBlock::guess(line).is_none());
        Expr::Call {
            name: self.solver.clone(),
            args: self.args.iter().chain(lines).cloned().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Number(f64),
    Name(String),
    /// Text in quotes: `"like this"`. No escapes; at most 256 characters.
    Text(String),
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
        name: Box<str>,
        params: Box<[String]>,
        /// Default values of the last `defaults.len()` parameters (`f(x, n := 2)`).
        /// Not children: they run at the call, in the worksheet's scope. The
        /// fields are boxed to keep `Expr` at 56 bytes (it is on the stack of every
        /// recursion, and inside `Edit`, whose size clippy watches).
        #[allow(clippy::box_collection)]
        defaults: Box<Vec<Expr>>,
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
    /// A `given` block (docs/SOLVE_BLOCKS.md): constraints on indented lines, then a
    /// `find`, `minerr`, `minimize` or `maximize` line. It means the same as the
    /// call `solver(args..., constraints...)`. Its children are in the order of the
    /// text: the constraints, then the closing line's arguments.
    Given(Box<GivenBlock>),
    /// `[x; y] := value`: define several names at once from the elements of a vector,
    /// each element keeping its own unit.
    MultiDefine {
        targets: Box<[String]>,
        value: Box<Expr>,
    },
    // Programs (docs/PROGRAMMING.md). A `Program` is a list of statements, and is
    // both `program` itself and every indented block inside one. The rest are the
    // statements. Names assigned in a program are local to it, all through it.
    /// `program` and the indented lines under it, or one of those blocks.
    Program(Vec<Expr>),
    /// `name ← value`: a local variable of the program.
    LocalAssign {
        target: String,
        value: Box<Expr>,
    },
    /// `name[i] ← value` or `name[i, j] ← value`: set one element of a local
    /// vector or matrix, making it or growing it (with zeros) as needed.
    IndexAssign {
        target: String,
        indices: Box<[Expr]>,
        value: Box<Expr>,
    },
    /// `if cond` with an indented block, and optionally `else`. Both are `Program`s.
    IfBlock {
        cond: Box<Expr>,
        then: Box<Expr>,
        otherwise: Option<Box<Expr>>,
    },
    /// `for var in over`: once for each value of a range, vector or matrix.
    For {
        var: String,
        over: Box<Expr>,
        body: Box<Expr>,
    },
    While {
        cond: Box<Expr>,
        body: Box<Expr>,
    },
    Break,
    Continue,
    /// `return value`: the value of the whole program.
    Return(Box<Expr>),
    /// `try` with a block, and the `on error` block that runs if it fails. With
    /// `on error e`, the message of the error is the text `e` in that block.
    Try {
        body: Box<Expr>,
        handler: Box<Expr>,
        message: Option<String>,
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
            Expr::Given(g) => g.constraints.iter().chain(&g.args).collect(),
            Expr::MultiDefine { value, .. } => vec![value],
            Expr::Program(statements) => statements.iter().collect(),
            Expr::LocalAssign { value, .. } => vec![value],
            Expr::IndexAssign { indices, value, .. } => {
                indices.iter().chain(std::iter::once(&**value)).collect()
            }
            Expr::IfBlock {
                cond,
                then,
                otherwise,
            } => [&**cond, &**then]
                .into_iter()
                .chain(otherwise.as_deref())
                .collect(),
            Expr::For { over, body, .. } => vec![over, body],
            Expr::While { cond, body } => vec![cond, body],
            Expr::Return(value) => vec![value],
            Expr::Try { body, handler, .. } => vec![body, handler],
            Expr::Break | Expr::Continue => vec![],
            Expr::Number(_)
            | Expr::Name(_)
            | Expr::Text(_)
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
