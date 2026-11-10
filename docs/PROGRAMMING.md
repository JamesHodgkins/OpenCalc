# Programming: design (Phase 6)

Status: built are sections 1, 3.3, 3.4, 3.5 and the language and editor of section 7.
Decisions the owner must make are in section 6.

## 1. Number literals in other bases (built)

`0x1F`, `0b1011`, `0o17` (prefix letter in either case). Chosen over Mathcad's suffix
forms (`1Fh`, `101b`, `17o`) because `h` and `b` are unit symbols (hour, bel) and a
suffix would be ambiguous with `1 h`.

- At least one digit of the radix must follow the prefix, and the digits must not run on
  into another letter or digit. Otherwise it is not a literal (`0b12`, `0x`, `0xG` are an
  undefined name, as before), so nothing is silently read as `0b1` times `2`.
- The value must be at most 2^53, the largest range of integers an `f64` holds exactly;
  larger is an error that says so. No rounding.
- A literal is a plain number: `0x1F m` is 31 m. Not done: showing a result in another
  base (a result format option), and bitwise functions (`and`, `or`, `xor`, shifts).

## 2. What exists today

- A region holds one expression. `f(x) := body` is `Expr::FunctionDef` with a single
  expression body; a body may use `if(c, a, b)` (`Expr::Conditional`, lazy).
- Functions read worksheet variables; only parameters are local (TD-30). No recursion
  (a function reaching itself is a circular reference). Calls nest 12 deep
  (`MAX_CALL_DEPTH`), sized from a stack test.
- Evaluation is metered and cancellable (`eval/meter.rs`), which a loop needs from day one.
- The dependency graph (`eval/graph.rs`) orders regions by what names they use. A program
  body must report its free names to it, excluding its own locals.

## 3. Proposed shape

### 3.1 Authoring: one region, several lines

Mathcad draws a program as a vertical bar of lines inside one region. Proposal: a region
whose text has several lines, with the existing math editor allowed to hold a line break
(Shift+Enter) only inside a `program` form, which is written

    f(x) := program
        s ← 0
        for i in 0..x
            s ← s + i
        return s

Open: `←` (local assignment, Mathcad's) is the same token as `:=` but local, or a new
keyword. Recommendation: `←` for local assignment (also accepted as typed `<-`), so
`:=` keeps its one meaning (a worksheet definition) and a program cannot define
globals by accident.

Recommendation on storage: **no new region kind.** A program is the body of a
`FunctionDef` or the value of a definition, so it stays inside the existing math
region, and the file format does not change (the source text just contains line breaks).
This avoids the 24-file cost that the Solve block audit found for a new `RegionKind`.
The cost is editor work: multi-line layout and caret movement in the math editor, which
is the larger part of this phase. The alternative (a program region kind) is easier to
edit but repeats the Solve-block migration.

### 3.2 Language

Statements: local assignment, `if`/`else`, `for i in range`, `for x in vector`, `while`,
`break`, `continue`, `return`, `try ... on error`, and an expression as the last line
(the value of the program if no `return` ran). Blocks are closed by indentation
or by explicit `end`: **needs decision** (6.2).

Values and scoping: locals live for one call, shadow worksheet names, are typed like
any value (units kept). A local is an ordinary name for dependency purposes, so the
graph must treat `s` in a body as bound, not a reference to a region.

`try` catches evaluation errors (not cancellation, not the step budget) and binds
`error` to its message as text. `return` with several values gives a vector; "functions
that return several values" is the same thing unless a tuple type is wanted (6.4).

### 3.3 Guards

Everything runs on the existing meter: each statement and each loop iteration costs a
step, cancel is checked at each, and the step budget ends a runaway `while` with an
error that names the program. Recursion (reverses TD-30) is allowed with a depth limit
well under the stack test's safe figure; the 12 limit is for nested *calls* and stays
the budget for non-recursive nesting. Recursive depth limit proposal: 64, after
re-running the stack test with the larger frame a program needs (the test must move
with the evaluator, not be guessed). A function that reaches itself stops being a
circular-reference error, so the cycle check in `graph.rs` has to separate "function
refers to itself" (allowed, guarded at run time) from "variable refers to itself"
(still an error).

### 3.4 Default arguments

`f(x, n = 2) := ...`. Defaults are evaluated at call time in the worksheet scope,
may not refer to earlier parameters (keeps the dependency list simple), and a call
may omit only trailing arguments. `FunctionDef.params` becomes a list of name and optional
default expression; no format change if stored as source text.

### 3.5 Manual calculation

Today there is an auto-calculate toggle. Manual mode adds: edits mark regions dirty but
do not re-run the worker; a Calculate command (F9) runs it; results shown are marked
stale (dimmed, with a status-bar note) until then. This is independent of the language
work and could be done first. Stored per document or per session: **needs decision**
(6.5).

### 3.6 Built so far (2026-11-10)

Base literals, manual calculation mode and default arguments are built; see the roadmap
entries for what each does. Notes for what follows: `Expr` is kept at 56 bytes (the
`FunctionDef` fields are boxed), because it sits on every recursion's stack and inside
`Edit`; the stack-depth tests fail first if it grows. New fields on `Expr` need the same
care. A new check that walks the tree must not recurse (see `has_default_argument`).

## 7. Programs as built (2026-11-10)

Syntax and evaluator are as section 3 proposed, with these particulars.

- **Tree.** `Expr::Program(Vec<Expr>)` is both `program` and each indented block;
  `LocalAssign`, `IfBlock`, `For`, `While`, `Break`, `Continue`, `Return`, `Try` are
  the statements. All are children in the tree, so error paths, spans and
  find-and-replace work as for any expression. `Expr` is still 56 bytes.
- **Source.** The lexer gives a `Line` token for a line break; indentation is read from
  the source text (a tab counts four). A block is the lines indented further than the
  line that opens it, all at one indent. `program` is a keyword only when a line break
  follows it. The statement words (`if else for while break continue return try on`)
  are keywords only at the start of a statement.
- **Limits.** Each line is held to `MAX_SYMBOLS` (128) as before, which is what bounds
  the depth of an expression; a whole program to `MAX_PROGRAM_SYMBOLS` (2048), 8192
  characters, and blocks nest 12 deep.
- **Locals.** Everything a program assigns (and every `for` variable) is local to the
  whole program, wherever the assignment is. The dependency analysis relies on that:
  what a program reads and does not assign is a dependency of the region. At run time
  a not-yet-assigned local is recorded under a marker name so reading it is an error
  rather than a fall-back to a worksheet variable. `Cx.locals` is a `RefCell` for this.
- **Value.** `return`, else the last line if it is an expression. A program that ends on
  a statement is an error.
- **Editing.** While a region holds a program it is edited as monospace text (the
  carets come from the text itself, not from node positions) and typeset when
  editing ends. In a program, spaces and line breaks are text. Enter is a new line even
  with the completion list open (Tab accepts a completion); Enter on a blank last line
  ends editing.
- **Element assignment.** `v[i] ← x` and `m[i, j] ← x` (`Expr::IndexAssign`; its
  children are the indices, then the value). The target is a local; it need not exist.
  An index past the end grows it, filling with zeros of the new element's unit; a vector
  grows along its length, a single index on a real matrix is an error. Copying costs
  budget in proportion to the size.
- **Recursion.** Allowed. In `eval.rs`, a strongly connected component made only of
  function definitions is not circular; one that includes any other region still is.
  `MAX_CALL_DEPTH` is 64 and every call charges the meter. Worst case per level is a
  128-symbol line inside blocks nested 10 deep; the test
  `the_deepest_recursion_of_the_deepest_functions_fits_in_the_evaluation_stack` runs it
  64 levels deep in half of `EVAL_STACK` (64 MB). Anything that evaluates a worksheet
  must do it on a thread with that stack (`worker` does; `check` now does).
- **Still to do.** The typeset form while editing, and an error message for `on error`
  (needs a text value). The `Given`/`Find` solve block is a separate roadmap item.

## 4. Order of work

1. Base literals (done).
2. Manual calculation mode (independent, small).
3. Default arguments (parser + call binding, small).
4. Programs, in this order: parser for the `program` form; evaluator with locals,
   `if`, `for` over a range, `return`; then `while`, `break`, `continue`; then
   `try`/`on error`; then recursion with its guard; then the editor's multi-line
   support (can start earlier in parallel, since it is the long pole).
5. Multiple return values, if still wanted after `return` of a vector.

Each step ships with tests: corpus worksheets with known answers, a runaway loop that
must stop on the budget, a cancelled loop, a recursive function at and beyond the depth
limit, and the printed-form round trip for every new form (the parser golden file).

## 5. Not planned

Closures and first-class functions, modules or imports, `goto`, string-building
statements, and any way for a program to write outside its own return value (no file or
worksheet side effects). Bitwise operators are separate and unscheduled.

## 6. Decisions needed

1. **Storage (decided 2026-11-10):** programs live inside the existing math region, chosen
   for the best end result, not for effort. A program is an expression, so it can be a
   function body, a definition's value, or an argument to `odesolve`, `find` or `minimize`;
   a separate region kind could not be used inline and would need the graph, units, error
   badges, find and replace and undo wired up a second time. Shown Mathcad-style, with a
   vertical bar per block and indentation for nesting. The app has no files to keep
   compatible, so the format changes in place: no migration code, no old shape kept.
2. **Block syntax (decided):** indentation, shown with a vertical bar per block.
3. **Local assignment sign (decided):** `←` (typed `<-`); `:=` keeps its one meaning.
4. **Several return values (decided):** a vector; no tuple type.
5. **Manual mode scope (decided):** per session, never saved in the file.
6. **Recursion depth limit:** 64, to be confirmed by the stack test when recursion is built.
