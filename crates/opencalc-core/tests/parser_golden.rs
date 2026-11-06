//! Golden tests for the parser: each input is paired with the shape of the tree it
//! must produce, written as a compact S-expression. They pin down precedence,
//! associativity, implicit multiplication and every syntax form, so a change to
//! the parser that alters a tree shows up as a readable diff.
//!
//! The expectations live in `tests/golden/parser.golden`:
//!
//! ```text
//! 1 + 2 * 3
//!   (+ 1 (* 2 3))
//! ```
//!
//! An input line is followed by its tree, indented. To accept a deliberate change,
//! run with `UPDATE_GOLDEN=1`, then read the diff of the golden file before you
//! commit it.

use std::fmt::Write as _;
use std::path::PathBuf;

use opencalc_core::expr::{AssignKind, BinaryOp, Expr, UnaryOp};
use opencalc_core::parser::parse;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/parser.golden")
}

fn sexp(e: &Expr) -> String {
    match e {
        Expr::Number(v) => format!("{v}"),
        Expr::Name(n) => n.clone(),
        Expr::Quantity { value, unit } => format!("(qty {value} {unit})"),
        Expr::Unary {
            op: UnaryOp::Neg,
            arg,
        } => format!("(neg {})", sexp(arg)),
        Expr::Binary { op, lhs, rhs } => {
            let symbol = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Pow => "^",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Le => "<=",
                BinaryOp::Ge => ">=",
            };
            format!("({symbol} {} {})", sexp(lhs), sexp(rhs))
        }
        Expr::Call { name, args } => {
            let args: Vec<String> = args.iter().map(sexp).collect();
            format!("(call {name} {})", args.join(" "))
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
            format!("({op} {target} {})", sexp(value))
        }
        Expr::Matrix { rows, cols, cells } => {
            let cells: Vec<String> = cells.iter().map(sexp).collect();
            format!("(matrix {rows}x{cols} {})", cells.join(" "))
        }
        Expr::Index { base, indices } => {
            let idx: Vec<String> = indices.iter().map(sexp).collect();
            format!("(index {} {})", sexp(base), idx.join(" "))
        }
        Expr::FunctionDef { name, params, body } => {
            format!("(defn {name} ({}) {})", params.join(" "), sexp(body))
        }
        Expr::Range { start, step, end } => match step {
            Some(step) => format!("(range {} {} {})", sexp(start), sexp(step), sexp(end)),
            None => format!("(range {} {})", sexp(start), sexp(end)),
        },
        Expr::Conditional {
            cond,
            then,
            otherwise,
        } => format!("(if {} {} {})", sexp(cond), sexp(then), sexp(otherwise)),
        Expr::Fraction { num, den } => format!("(frac {} {})", sexp(num), sexp(den)),
        Expr::Root { radicand } => format!("(root {})", sexp(radicand)),
        Expr::Error(e) => format!("(error {}..{} {})", e.start, e.end, e.message),
    }
}

/// Inputs, in the order they appear in the golden file.
const INPUTS: &[&str] = &[
    // Numbers, names, units.
    "42",
    "1.5e3",
    "x",
    "\u{3c3}_y",
    "5 kN",
    "5 kN / (2 m) ^ 2",
    // Precedence and associativity.
    "1 + 2 * 3",
    "(1 + 2) * 3",
    "1 - 2 - 3",
    "1 - (2 - 3)",
    "8 / 4 / 2",
    "2 ^ 3 ^ 2",
    "-2 ^ 2",
    "(-2) ^ 2",
    "2 * -3",
    "-x + y",
    // Implicit multiplication.
    "2 x",
    "2 (x + 1)",
    "2 x ^ 2",
    "3 m ^ 2",
    // Calls and roots.
    "sin(x)",
    "max(1, 2, 3)",
    "f()",
    "sqrt(x ^ 2 + y ^ 2)",
    // Definitions and evaluation.
    "x := 3",
    "x = ",
    "x = 5",
    "area := w * h",
    // Comparisons.
    "a == b",
    "a != b",
    "a < b",
    "a >= b",
    "a + 1 < b * 2",
    "1 < 2 == 3 > 2",
    "a \u{2264} b",
    "a \u{2260} b",
    // Matrices, indexing, conditionals.
    "[1, 2, 3]",
    "[1; 2; 3]",
    "[1, 2; 3, 4]",
    "[1 m, 2 m]",
    "[a + 1, b * 2]",
    "v[0]",
    "M[i, j]",
    "M[0][1]",
    "f(x)[2]",
    "(a + b)[0]",
    "[1, 2][0]",
    "-v[0]",
    "v[0] ^ 2",
    "2 v[0]",
    "v[i + 1]",
    "if(x > 0, x, -x)",
    "if(a, if(b, 1, 2), 3)",
    "if(1, 2)",
    // Ranges.
    "0..5",
    "0..n",
    "0, 2..10",
    "i := 0..n",
    "i := 0, 0.5..10",
    "a + 1 .. b * 2",
    "x := (0..5) ^ 2",
    "sum(0..5)",
    "f(1, 2..3)",
    "1.5..2.5",
    "0 .. 5",
    "1..",
    "..5",
    "0, 2",
    "0, 2 ..",
    "f(0, 2..10)",
    // Function definitions and calls.
    "f(x) := x ^ 2",
    "area(w, h) := w * h",
    "g(a, b, c) := if(a > b, c, 0)",
    "h(x) := 0..x",
    "f(x) := g(x) + g(x + 1)",
    "f(2) =",
    "f(x) = x",
    "f(x)[0]",
    "2 f(3)",
    // Errors.
    "",
    "f() := 1",
    "f(x, x) := 1",
    "f(1) := 2",
    "f(x + 1) := 2",
    "f(x) := ",
    "1 +",
    "(1 + 2",
    "1 + * 2",
    "[1, 2; 3]",
    "[1, 2",
    "[]",
    "v[0",
    "v[]",
    "2 [1]",
    "1 = = 2",
    "2 + 3 := 4",
    "1 ! 2",
];

fn render() -> String {
    let mut out = String::new();
    for src in INPUTS {
        writeln!(out, "{src}").unwrap();
        writeln!(out, "  {}", sexp(&parse(src))).unwrap();
    }
    out
}

#[test]
fn the_parser_produces_the_recorded_trees() {
    let got = render();
    let path = golden_path();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} is missing; run with UPDATE_GOLDEN=1", path.display()))
        .replace("\r\n", "\n");
    if got != want {
        // Show the first differing input rather than two walls of text.
        let (g, w): (Vec<&str>, Vec<&str>) = (got.lines().collect(), want.lines().collect());
        for (i, (a, b)) in g.iter().zip(&w).enumerate() {
            if a != b {
                let input = g[..=i]
                    .iter()
                    .rev()
                    .find(|l| !l.starts_with("  "))
                    .unwrap_or(&"?");
                panic!("tree changed for input {input:?}\n  now:      {a}\n  recorded: {b}\n(run with UPDATE_GOLDEN=1 to accept)");
            }
        }
        panic!(
            "golden file and inputs differ in length ({} vs {} lines)",
            g.len(),
            w.len()
        );
    }
}

#[test]
fn the_golden_inputs_have_no_duplicates() {
    for (i, a) in INPUTS.iter().enumerate() {
        assert!(!INPUTS[..i].contains(a), "{a:?} is listed twice");
    }
}
