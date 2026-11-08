# Solve blocks: design (Phase 2.2)

Status: **Stage 1 built (2026-11-08); Stage 2 not started.** The decisions in section 7 were
settled as recommended (staged approach, the syntax below, strict tolerance, `find` is an
error on failure, mixed-dimension unknowns refused for now). Section 9 records where the
build differs from, or adds to, the proposal.
Roadmap context: `ROADMAP.md` Phase 2.2. Written 2026-11-08.

Labels: **[PTC]** documented by PTC, **[inferred]** inferred from PTC or community
material, **[decision]** a choice for this repository.

## 1. What Mathcad does

- **[PTC]** A solve block is the word `Given`, constraints (equations and inequalities),
  and a solver function: `Find`, `Minerr`, `Minimize` or `Maximize`. `find` and
  `minerr` only work inside solve blocks.
  ([PTC help: system of equations functions](https://support.ptc.com/help/mathcad/r11.0/en/PTC_Mathcad_Help/system_of_equations_functions.html))
- **[PTC]** Each unknown needs a **guess value**, defined above the block or inside it.
  Complex solutions need complex guesses. To solve for n unknowns the block needs n
  equations.
- **[PTC]** One unknown returns a scalar; several return a vector in the order listed.
- **[PTC]** `find` and `minerr` use the Levenberg-Marquardt algorithm (`find` may pick
  another method for linear problems). `minerr` returns the best answer of the last
  allowed iteration even when it did not converge, so its result must be validated.
  The page does not say what `find` returns on failure; it suggests `minerr`,
  different guesses, another CTOL or an added inequality.
- **[PTC, older versions / community]** CTOL (default 0.001) set how closely constraints
  had to be met. PTC's Prime 8.0 help says the tolerances of these functions are now
  set internally. Not found in PTC's pages: which inequality operators are allowed.
- **[inferred]** The block is read in worksheet order: `Given` above its constraints
  above the `Find`. That position-dependence is the one thing that cannot be copied
  (section 4).

## 2. What OpenCalc has today

- Constraints are expressible now: `==` is an ordinary comparison inside a call, and
  `<`, `<=`, `>`, `>=` exist (`=` is the statement-level "evaluate" and is not used).
- Evaluator forms with a **local bound variable**, a **step budget** and **cancellation**
  exist (`integrate`, `root`, ... in `eval/calculus.rs`, `eval/meter.rs`). Bound names
  are excluded from a region's dependencies (`expr.rs`, `BINDING_FORMS`).
- Matrices, complex numbers, units and a deterministic dependency-graph evaluator exist.
- Missing: any solver for several unknowns; per-element dimensions in vectors (TD-28);
  a region kind that holds several lines.

## 3. Recommendation: two stages

**Stage 1 (no format change): solver forms.** Add `find`, `minerr`, `minimize` and
`maximize` as evaluator forms, written as calls. The solver, its tolerances, error
taxonomy and tests are all here. Nothing on disk changes, no region type is added,
no exhaustive match is touched.

**Stage 2: the `Given` / `Find` block.** A new region kind that shows and edits the
Mathcad-style block, and **lowers to the Stage 1 call**. All the numerics are already
shipped and tested by then; Stage 2 is presentation, editing, persistence and the
exhaustive-match work in section 6.

Why this order: the numerics are the risk and are independent of how the block looks;
the region kind has a wide blast radius (section 6) and a format version bump; and
the project rule is to ship a useful, testable baseline first rather than half of a
block with no solver behind it.

## 4. Why not copy Mathcad's layout semantics

In Mathcad the block is separate regions read top to bottom. Here, evaluation order
comes from the dependency graph and never from layout (`eval.rs`; tested by
`results_do_not_depend_on_the_order_regions_are_laid_out_in`). Giving `Given` regions a
position-dependent meaning would break that guarantee for the whole worksheet. So
**one block is one region** (Stage 2), or one call (Stage 1). **[decision]**

## 5. Stage 1 design

### 5.1 Syntax

```
x := 1                      # guess values: ordinary definitions
y := 1
s := find(x, y, x ^ 2 + y ^ 2 == 25, x - y == 1)     # 4, 3  (a column vector)
t := minimize(x ^ 2 + y ^ 2, x, y, x + y >= 2)
```

- `find(u1, ..., un, c1, ..., ck)` and `minerr(...)`: leading bare names are the
  unknowns (at least one); the rest are constraints (at least one).
- `minimize(f, u1, ..., un, c1, ...)` and `maximize(...)`: the first argument is the
  objective, then the unknowns, then optional constraints.
- A constraint is a comparison: `==` (equation), `<`, `<=`, `>`, `>=`. `!=` is refused.
- **Guesses** are the worksheet values of the unknowns' names, read **outside** the
  call. Inside the constraints and objective the names are bound to the iterate and
  shadow everything, as in `integrate`. An undefined guess is the error "give `x` a
  starting value, such as `x := 1`".
- **Result:** one unknown gives a scalar, several give a column vector in the order
  listed **[PTC]**. Assign it to a new name; assigning to an unknown's own name is
  allowed but cyclic (`x := find(x, ...)` reads `x` as its own guess: a circular
  reference, reported as one).
- Dependencies: the unknowns' names count as read (the guesses); bound uses inside the
  constraints do not. Same mechanism as `BINDING_FORMS` with a rule for the leading
  names.

### 5.2 Meaning and algorithms

All arithmetic is on SI values; dimensions are checked, not scaled.

| Piece | Rule |
|---|---|
| Equation `a == b` | residual `a - b`; `a` and `b` must share a dimension (different constraints may have different dimensions). |
| Inequality `a < b`, `a <= b` | violation `max(0, a - b)`; strictness is ignored (a solution on the boundary is accepted) **[decision]**. `>`, `>=` mirror. |
| `find` | Levenberg-Marquardt on the equation residuals plus inequality violations, finite-difference Jacobian. After it stops, **every constraint is checked**; if any is violated beyond tolerance the result is the error "no solution found, best attempt leaves residual R" and never the failed point. |
| `minerr` | The same iteration; returns the best point reached even if constraints are not met **[PTC: documented behaviour of minerr]**. Also never returns a non-finite value. |
| `minimize`, `maximize` | BFGS with a finite-difference gradient and backtracking line search from the guess; constraints by an exterior quadratic penalty with increasing weight, then the same final check as `find`. Local optimum from the guess **[decision]**; `maximize` minimises the negated objective. |
| Tolerances | Constraints met to `1e-9` relative to their terms' size (plus a tiny absolute floor) **[decision]**; stricter than Mathcad's historic 0.001, and fixed (no user setting) like Prime 8.0 **[PTC, version 8.0]**. |
| Limits | At most 200 iterations; every residual evaluation is charged to the region's step budget (shared with nested forms) and observes cancellation. Up to 16 unknowns and 64 constraints. |
| Complex unknowns | Not in the baseline. A complex guess is an error. **[decision]** |
| Units | Each unknown keeps its guess's dimension. **All unknowns of one call must share a dimension** until per-element dimensions exist (TD-28), else a clear error. |

### 5.3 Errors

New kinds are not needed beyond `EvalError::Invalid` messages, as for the calculus
forms: no unknowns / no constraints; unknown has no guess; guess not a real number;
constraint is not a comparison; dimension mismatch inside a constraint; unknowns of
different dimensions; too many unknowns or constraints; "no solution found (residual
R)"; budget exhausted; cancelled.

### 5.4 Tests

Unit tests of the solver core on textbook systems (circle and line, a 3x3 nonlinear
system, Rosenbrock for `minimize`, an over-determined fit for `minerr`, an infeasible
`find`); property checks (a returned `find` result satisfies every constraint; the
result does not depend on region order); corpus worksheets with hand-worked answers;
bounded-work tests with pathological constraints; cancellation. Files:
`eval/solve.rs` (forms, argument handling), `eval/solve/lm.rs`, `eval/solve/bfgs.rs`,
the function-table entries, `BINDING_FORMS` additions.

## 6. Stage 2: audit of what a new region kind touches

`RegionKind` appears in 24 production files (40 including tests and benches); the ones that need an intentional
decision are below. Everything marked "text-like" treats the block as multi-line source
text, as a Math region does. **[decision]** on all of the behaviours.

| Area | Files | Behaviour for a Solve region |
|---|---|---|
| Model | `document/region.rs` (`RegionKind`, `label`, `default_size`, `is_auto_sized`, `is_editable`), `document/mod.rs` | New variant `Solve(SolveBlock)`; auto-sized and editable like Math. |
| File format | `persist/validate.rs`, `persist.rs` (version), `persist/migrate.rs`, `docs/FILE_FORMAT.md`, `tests/file_format_doc.rs` | **Format version 2.** A new variant is unreadable by version 1 readers, so a bump is required; a v1 file loads unchanged (no migration step beyond the version). Validate: source length and line limits as for Math. |
| Edits and undo | `edit/mod.rs`, `edit/history.rs` | Edit the source with the existing text edit (one undo step per commit, coalescing as for Math); the evaluation revision moves. |
| Evaluation | `eval.rs`, `document/mod.rs` (inputs), `check.rs`, `eval/plot.rs` (no) | Lowered to the Stage 1 call; defines no names; its value is the solver result; included in dependency edges through the unknowns' guesses and the constraints. |
| Search | `find.rs` | Searched and replaced like Math source. |
| Canvas | `canvas/paint.rs`, `adapt.rs`, `hit.rs`, `overlay.rs`, `editor.rs`, `abandon.rs`, `align.rs`, `separate.rs`, `layout/*` | Typeset as a block (`Given`, constraints, `Find(...)`, then the result); hit-test and selection as a Math region; error badge from the solver error. |
| Commands and panels | `app/dispatch.rs`, `app/inserting.rs`, `app/clipboard.rs`, `app/labels.rs`, `app/formatting.rs`, `sheet.rs`, `panels/*` | Insert command (Math > Regions), copy/cut/paste, labels, result format; Functions window lists the forms. |
| Export | `export/*` | Painted by the canvas painter, so PDF and PNG follow. |
| Compatibility matrix | | Select, move, delete, lock, label: supported. Collapse/group: n/a (no such command yet). Unit conversion and decimals: apply to the result. |

Entry (how the block is typed) belongs to the structured-editor milestone; until then
the block's source is the multi-line text above, edited like a Math region.

## 7. Decisions needed

1. **Staging.** Approve Stage 1 (forms) first, Stage 2 (block region, format version 2)
   afterwards?
2. **Syntax.** Approve `find(x, y, c1, ...)` with guesses taken from same-named
   worksheet variables, constraints written with `==`, `<`, `<=`, `>`, `>=`, and
   `minimize(f, x, ..., c1, ...)`?
3. **Mixed-dimension unknowns.** Accept "all unknowns share a dimension" for now, or do
   per-element matrix dimensions (TD-28, a value-model change) before the solver?
4. **Tolerance.** Fixed strict `1e-9` (this design), or Mathcad's historic `1e-3` default?
5. **`find` failure.** Error (this design) rather than returning the best point, so a
   wrong answer is never silent; `minerr` is the way to ask for the best point.

## 8. Not in this design

Complex unknowns; symbolic solving; `odesolve`/`pdesolve`; MultiStart; user-chosen
algorithms; the nonlinear "KNITRO" class of constrained solvers; matrix unknowns;
parameterised solutions (`f(a) := find(x, ...)` works as an ordinary user function
only if a function body may use forms, which it may).

## 9. Stage 1 as built

Code: `eval/calculus/solve.rs` (arguments, constraints, the four forms), `solve/lm.rs`
(Levenberg-Marquardt and a small dense solver), `solve/bfgs.rs` (BFGS and the
augmented-Lagrangian penalty), `expr/forms.rs` (the form tables and how a solver call's
arguments divide into unknowns, objective and constraints; shared with dependency
analysis). Tests: unit tests in each numerical file, `tests/solve.rs`, corpus
`tests/corpus/solve.calc`.

Differences from, and additions to, the proposal:

- **Signed residuals.** Levenberg-Marquardt drives the *signed* difference of an equation
  to zero (clamped at zero only for an inequality). The first version used the absolute
  value, whose kink at the solution made the finite-difference Jacobian useless once a
  step was smaller than the distance to it; it stalled about 1e-7 short. A regression test
  (`ln(a) == 1`, `sqrt(a - 1) == 2`, `a^2 == c`) guards it.
- **Tolerance.** `find` and `minerr` hold constraints to `1e-9` relative to their sides,
  with a floor of `1e-14` of their starting size. `minimize` and `maximize` reach
  constraints through a penalty method and are held to `1e-7`. Both are fixed.
- **Out of budget** is its own error kind (`EvalError::OutOfBudget`), so a solver can tell a
  rejected trial point (any ordinary error, even a domain error) from an order to stop
  (out of budget, cancelled: `EvalError::is_stop`).
- **The guess matters.** A local method from the wrong side of a constraint can stall
  between pulls (`find(a, a^2 == 4, a < 0)` from `a := 1`). `find` then reports an error
  and never a wrong point; `minerr` returns the best point; a better guess fixes it. A
  test records this. MultiStart is not built.
- **The constraint limit of 64 is a backstop:** a region holds at most 128 symbols, so a call
  that long does not parse.
- **Optimising forms** return the unknowns only (not the objective's value); a function
  unbounded below is the error "did not converge".
