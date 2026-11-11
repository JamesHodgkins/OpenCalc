# Technical debt register

Known shortcuts and gaps, with what each costs and when it must be paid. A living
file: add an entry when a shortcut is taken, delete it when it is paid. Design
context is in `ARCHITECTURE.md`.

**Last reviewed:** 2026-11-11 (engine entries re-audited against the code; TD-29 to TD-32 split into one row per decision; solve block, program and editing entries added).  **Size:** S under a session, M a few, L a milestone's worth.

## Open

### Engine (pay with Milestone 2: phase C)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-06 | **Accepted (2026-11-11).** SPEC 3.3 says the AST is the truth, but a Math region is saved as source text and the expression is derived (decision D5). The editor edits that text (`EDITOR.md`, option A: hybrid), so there is no structured editor to force a change, and the two representations are private and change together. | `SPEC.md` 3.3 describes a design the code does not follow. | Amend SPEC 3.3 to say the source text is the saved form. | S |
| TD-08 | **Mostly done (C2).** `Value` is `Scalar` (complex, dimensioned), `Matrix` (flat, one dimension per element), `Series` (ranges and lists), `Function` and `Text` (joined with `+`, compared with `==` and `!=`, nothing else). Matrices are typeset with brackets and can be written as literals. Still missing: a boolean type (comparisons give 1 or 0: TD-29.1), and arithmetic on text beyond `+`. | Truth values and text handling are thin. | When asked | S |
| TD-28 | **Resolved (2026-11-08).** Every matrix element has its own dimension (stored once when they agree). Operations that need one unit throughout refuse a mixed matrix with a unit mismatch; see ROADMAP 1.1 for which. | | | |
| TD-09 | **Mostly done.** `UnitRegistry`, `UnitSystem` (display only), a Define/Manage units window, per-dimension display units, and load-time validation of custom units (`Worksheet::units()` in `persist/validate.rs`) all exist. Compound units work as expressions and display by name or as numerator over denominator (ROADMAP 1.1). Still missing: temperature display in degC or degF (input works), and a unit parsed at parse time is limited to the built-in table (custom units work as names, so a variable of the same name shadows them). | Unit coverage (roadmap Phase 1). | Roadmap Phase 1 | M |
| TD-10 | **Mostly done (C4, E6).** Parse errors carry a character range (`ParseError`); evaluation errors carry a path to the failing sub-expression (`Output::error_paths`); both are underlined in the typeset form, in the editor and on the canvas (`canvas/errors.rs`). Not done: a path through a dependency (an error that comes from another region points at the name, not at the cause). | Error tracing stops at the region boundary. | When asked | S |
| TD-07 | The evaluator recomputes the whole sheet on every edit; there is no dirty propagation. **Measured (docs/PERFORMANCE.md): 500 regions in 0.39 ms against a 200 ms budget, so this is not needed.** | None at current scale. | Revisit only if a measurement after phase C or Milestone 2 approaches a budget | M |
| TD-20 | **Resolved (Phase 2.1).** Repeated evaluations are metered (`eval/meter.rs`, 1,000,000 steps per region) and cancellable (`EvalError::Cancelled`, raised by the worker when a newer request arrives). | | | |
| TD-19 | `MAX_SYMBOLS = 128` per line of source, `MAX_PROGRAM_SYMBOLS = 2048` and 8,192 characters for a whole program, blocks nested 12 deep (`MAX_BLOCK_DEPTH`). The math editor refuses to edit a source over 8,192 characters (`text::MAX_CHARS`) and shows a message instead. | Rejects a legitimately very long equation or program. | Revisit if frames shrink or the editor changes input shape; keep the stack-margin tests | S |
| TD-23 | Region IDs restart at 1 on every worksheet. | Cross-sheet references and global definitions (`≡`) need a worksheet-qualified ID. | With global definitions | S |

### Persistence and platform (Milestone 3)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-12 | **Resolved.** `.ocx` file format with `FORMAT_VERSION`, migration chain, atomic writes and crash recovery (see `PERSISTENCE.md`). | | | |
| TD-13 | **Resolved.** `opencalc check` loads, evaluates and reports worksheets (`crates/opencalc-cli`). | | | |
| TD-11 | **Mostly done.** Result format (decimals, scientific, engineering) and display units exist (`document/result_format.rs`, Math Formatting tab). A result's unit is also a slot: click it and type any unit (`N/mm^2`). Still missing: custom (worksheet-defined) units in the slot, and a unit typed in the expression itself. | Minor. | When asked | S |
| TD-21 | **Partly done.** CI builds and tests on Linux, and deb/AppImage packaging is configured. Not yet run by hand on a Linux desktop; macOS is not supported. | Possible Linux-only UI problems (fonts, file association, single-instance hand-off). | Before a Linux release | S |

### Hygiene

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-15 | Typesetting is redone every frame for visible regions; only the *sizes* are cached. **Measured: 0.76 ms for a frame with 200 visible regions against 16.7 ms, excluding tessellation and the GPU.** | None at current scale. Tessellation and GPU time are not yet measured. | Revisit if a real-window frame is slow | M |
| TD-27 | Measured region sizes (document units) vary slightly with zoom, because of font rounding at small pixel sizes. "Fit" computed from sizes measured at another zoom undershoots, so it needs two passes to be exact (found by the performance test). | A small, visible imprecision in Fit. | When a layout cache is built, key it by zoom bucket | S |
| TD-26 | **Partly done.** CI builds, lints, tests and runs the performance budgets on Windows and Linux (`.github/workflows/ci.yml`), plus `cargo deny check licenses` on Linux. The file-size and function-length rules live in `scripts/check.ps1`, which CI runs on Windows only. Still missing: an advisories check in CI, and macOS (not supported). **Run by hand on 2026-11-11 (`cargo deny check advisories`): it fails** with 2 vulnerabilities (RUSTSEC-2026-0194 and 0195, `quick-xml` 0.30, Linux build only) and 5 unmaintained crates (`paste`, `ttf-parser`, `zbus_names`, `zvariant`, `zvariant_derive`). All come in through `eframe` 0.31 (`egui-winit`/`accesskit`, `epaint`), and none is code OpenCalc calls itself. The fix is to move to a newer `eframe` (0.37 exists), which is a migration of its own. | Known vulnerable dependencies are not flagged in CI, and the size rules are not checked on Linux. | Before a public release: upgrade `eframe`, then add the check to CI with no ignore list | M |
| TD-18 | **Resolved (U2).** The fake "12 pt" font-size slot and the ribbon's slot mechanism were removed rather than shipped as a control that does nothing. | | | |
| TD-22 | **Done (M2.1).** `tests/parser_golden.rs` records the tree for 75 inputs in `tests/golden/parser.golden`; accept a deliberate change with `UPDATE_GOLDEN=1` and read the diff. | | | |
| TD-24 | **Resolved** (rich text). Inter Italic and Bold Italic are bundled, so italic is a real face. Note they come from `@expo-google-fonts/inter` 0.4.0 while the upright faces are from 0.2.3; slightly different Inter releases. Lucide icons are private-use codepoints and rely on font order. | | | |

### Language and engine decisions to revisit

Choices made quickly that are easy to change now and costly once worksheets exist.
Worksheets are saved today (`.ocx`), so the milestone this register once named for
them ("before Milestone 3 saves real files") has passed: changing any of these changes
saved results. Each row is one decision, split out of the old TD-29 to TD-32 so that
one can be paid without the others.

#### TD-29: operators, indexing and lists (M2.1)

Impact: behaviour people will rely on. Pay by: before a public release. Size: S each.

| ID | Decision |
|---|---|
| TD-29.1 | Comparisons give 1 or 0; there is no boolean type. |
| TD-29.2 | Equality is exact; there is no `TOL`. |
| TD-29.3 | All comparison operators share one precedence level, so `1 < 2 == 3` chains left to right. |
| TD-29.4 | `ORIGIN` is an ordinary variable that indexing reads (default 0, as in Mathcad). |
| TD-29.5 | `x_i` is one name drawn with a lowered `i` (`F_x`, `layout/names.rs`), not an index. Indexing is `x[i]`, typeset as a subscript. (The old entry said subscript entry was not parsed; underscore names were added with the editor, E5.) |
| TD-29.6 | `if` evaluates only the chosen branch but counts both for dependencies, so a cycle through an unused branch is still a cycle. |
| TD-29.7 | Ranges are lists (`Value::Series`) combined element by element, while matrices keep linear-algebra rules: `k ^ 2` squares each element and `M ^ 2` is the matrix product (whole-number powers only; `M ^ -1` is the inverse). |
| TD-29.8 | `a, b..c` takes its step from the first two values and is only allowed where a comma cannot separate arguments, so `f(0, 2..10)` is two arguments. |
| TD-29.9 | `stdev` is the population form (Mathcad); `stdev_sample` divides by n - 1. |
| TD-29.10 | A list or matrix cannot be a matrix cell or a range end. |

#### TD-30: user functions (M2.3, Phase 6)

Impact: recursion and local variables are the usual next requests from engineers. Pay by: when a worksheet needs them. Size: S each.

| ID | Decision |
|---|---|
| TD-30.1 | Recursion is allowed (Phase 6 reversed the first decision): a cycle made only of function definitions is fine; a cycle through a variable is still circular. |
| TD-30.2 | Calls nest 64 deep (`MAX_CALL_DEPTH`) and each call costs a step of the budget. The evaluation stack is 64 MB (`worker::EVAL_STACK`); a test checks the deepest recursion of the deepest function bodies fits in half of it. |
| TD-30.3 | A body reads variables from the worksheet, not from where it is called; only its parameters are local. |
| TD-30.4 | Functions and variables are separate namespaces; a function cannot take a built-in's name. |
| TD-30.5 | An error inside a body is reported at the call as `in function 'f': ...`, without a path into the body. |
| TD-30.6 | `5 m` is always a quantity in metres, even when `m` is a parameter or variable. |
| TD-30.7 | Default arguments exist. Several return values are a vector (no tuple type; PROGRAMMING 6.4). |

#### TD-31: plots (M2.4)

Impact: an interactive plot is a likely request once real data is plotted. Pay by: Milestone 3 or when asked. Size: S each.

| ID | Decision |
|---|---|
| TD-31.1 | Plots are painted with the canvas painter, like every region (so they scale with zoom and a drag moves the region), not with `egui_plot`, which needs its own `Ui`. The interactive view (hover, pan, zoom, log axes) is a separate window using the same painter. |
| TD-31.2 | A plot has up to two y axes (left and right); the series on one axis must share a dimension. |
| TD-31.3 | Axis bounds are automatic unless a limit is given. |
| TD-31.4 | Series are expressions that evaluate to a list or vector: no `f(x)` function-plotting syntax. Line, marker, step, stem and bar series and a polar plot kind exist (the old entry said no bar plots). |
| TD-31.5 | Plot data is evaluated after all math regions, so a plot can read anything and nothing can read a plot. |
| TD-31.6 | The plot editor lives in the Plot window, not on the canvas. |
| TD-31.7 | Line colours are a cycle of eight, or one chosen per series; the colour is saved as a palette index, so it follows the theme's palette. |

#### TD-32: calculus and solvers (Phase 2.1)

Impact: behaviour to settle before the forms get typeset notation. Pay by: with any typeset notation for them. Size: S each.

| ID | Decision |
|---|---|
| TD-32.1 | The forms are function-call syntax (`integrate(f, x, a, b)`), not typeset notation. |
| TD-32.2 | Integrals, derivatives and limits are numerical with stated tolerances and refuse when they cannot reach them: no symbolic result, no improper integrals, first derivative only, two-sided limits only. |
| TD-32.3 | `sum_over` and `product_over` refuse an empty range. |
| TD-32.4 | The two bounds of an integral must have the same dimension, so `0` and `3 m` do not mix. |
| TD-32.5 | An unevaluated bound variable cannot be a unit name written after a number (`2 m` is parsed as a quantity before the variable is known); use names like `x`, `t`, `k`. |
| TD-32.6 | The budget counts evaluations, not time. |
| TD-32.7 | `root` finds real roots of real functions only and, from a guess, the nearest sign change rather than the nearest root. `polyroots` reads coefficients lowest power first and reports repeated roots only to about 1e-8. |
| TD-32.8 | `odesolve` is an explicit non-stiff method (a stiff equation is an error, not a slow answer), returns the state at the end or a table (not a function of time), and has no events. `col` and `row` are evaluator forms because they count from `ORIGIN`. |

### Solve blocks, programs and editing (Phases 2.2 and 6)

Built 2026-11-08 to 2026-11-10 (`SOLVE_BLOCKS.md`, `PROGRAMMING.md`, `EDITOR.md`).

#### TD-33: solve blocks

Impact: answers depend on the starting guess. Pay by: when users report a wrong or failed solve. Size: S to M each.

| ID | Debt |
|---|---|
| TD-33.1 | The solvers are local methods, so the answer depends on the guess. A guess on the wrong side of a constraint can stall (`find(a, a^2 == 4, a < 0)` from `a := 1`); `find` then reports an error and never a wrong point, `minerr` returns the best point. There is no MultiStart. A test records this. |
| TD-33.2 | Tolerances are fixed: `find` and `minerr` hold constraints to 1e-9 relative to their sides; `minimize` and `maximize` reach constraints through a penalty method and are held to 1e-7. Strictness of an inequality is ignored (a solution on the boundary is accepted) and `!=` is refused. |
| TD-33.3 | `minimize` and `maximize` return the unknowns only, not the objective's value; a function unbounded below is the error "did not converge". |
| TD-33.4 | No complex or matrix unknowns. At most 16 unknowns and 64 constraints (the 128-symbol limit means a call that long does not parse anyway). |
| TD-33.5 | A `given` block is text inside an ordinary math region, not a region kind of its own, so it has no dedicated UI (no collapse, no per-block settings). A guess line (`name := value`) inside a block is local to it. `[a; b] := value` defines each name from its element of a vector and is a general feature, not tied to blocks. |

#### TD-34: programs

Impact: limits and scoping rules a long program can hit. Pay by: when a program needs more. Size: S each.

| ID | Debt |
|---|---|
| TD-34.1 | Blocks are closed by indentation only (a tab counts four); there is no explicit `end`. `program` and `given` are keywords only when a line break follows; the statement words (`if else for while break continue return try on`) only at the start of a statement. |
| TD-34.2 | Everything a program assigns, and every `for` variable, is local to the whole program, wherever the assignment is. A not-yet-assigned local is recorded under a marker name so reading it is an error and never a fall-back to a worksheet variable. |
| TD-34.3 | `Expr` is kept at 56 bytes (`FunctionDef` fields are boxed) because it sits on every recursion's stack and inside `Edit`; the stack-depth tests fail first if it grows. A new check that walks the tree must not recurse (see `has_default_argument`). |
| TD-34.4 | Evaluator locals are a `RefCell` on `Cx` (`Cx.locals`), repeated in five places that build a context. A double borrow would panic at run time; one shared constructor would remove the repetition. |

#### TD-35: typeset editing of source

Impact: editing feels structural but is not. Pay by: if structural operations are wanted. Size: M.

| ID | Debt |
|---|---|
| TD-35.1 | Editing is of the text (`EDITOR.md` option A), drawn typeset. Structural operations (wrap in a fraction, unwrap) are not built; they could be added on top of the node spans. |
| TD-35.2 | The lenient parse (`parse_lenient`) keeps what is typed so far: a line that does not parse is kept as text (`Garbage`) and a new empty line is a `Hole` (a box). Strict parsing is unchanged. |
| TD-35.3 | A caret between two known positions on one line is placed by interpolation; across lines it takes the nearer position (`layout/source/stops.rs`, `caret_at`). In a program, carets among a line's leading spaces collapse to where its text starts (`indent_carets`). |
| TD-35.4 | Program indentation is spaces the user types, shown as a bar per block. |

### Process

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-36 | The function-length limit is 150 lines (`scripts/check.ps1`, no exceptions). About 18 production functions are over 100 lines, many of them flat tables or matches (`dispatch`, `is_implemented`, `Expr` printing, `layout_node`). | A tighter limit would need those split; at 150 a function can still grow large before the gate objects. | Tighten to 100 once the flat tables are resolved | M |
| TD-37 | Copying ignores a failed write to the system clipboard (`NativeClipboard::set`, `app/clipboard.rs`). Paste recognises this app's own copy by comparing the system clipboard's text, so after a failed write it sees whatever was there before and pastes that as a text region instead of the copied regions. | A copy can silently turn into the wrong paste when the clipboard is busy or unavailable. | When it is reported | S |

## Paid in the 2026-11-06 review

| ID | Was | Fix |
|---|---|---|
| R-01 | Unit exponent overflow panicked the evaluator (debug) or wrapped silently (release). | Checked arithmetic, `DimensionOverflow`. |
| R-02 | `1/0`, `sqrt(-4)`, `ln(0)` returned numbers. | `EvalError::NotFinite` at one choke point. |
| R-03 | About 2,000 nested parentheses overflowed the stack and aborted the app. | `MAX_SYMBOLS`, with stack-margin tests at 512 KB (core) and 1 MB (whole frame). |
| R-04 | A stale worker result could be applied to the wrong sheet. | `Worker::poll_for(revision)`. |
| R-05 | Wrong-looking display units (`5 N` as `0.0050 kN`). | Per-dimension display table. |
| R-06 | An evaluator panic killed the worker for good. | `catch_unwind` plus `EvalError::Internal`; explicit 8 MB stack. |
| R-07 | Unused direct `egui` dependency (version-skew risk). | Removed; everything goes through `eframe::egui`. |
| R-08 | No integration tests of the UI. | `smoke.rs`: 7 headless end-to-end tests. |
| R-09 | No automated quality gate; the 400-line rule was unchecked. | `scripts/check.ps1` with a ratchet. |
| R-10 | TD-01: document model in the UI crate with egui types and UI state. | `core::document` with its own `Point`/`Size`; UI state in `OpenSheet`/`CanvasUi`. Core's dependency tree contains no UI crate. |
| R-11 | TD-02: regions edited in three places; no undo. | One path, `Worksheet::apply(Edit) -> inverse`, plus `History` with coalescing. Undo/redo in the ribbon, Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z. |
| R-12 | TD-03: measured size stored in the document and rewritten on zoom. | Measured sizes are UI state (`CanvasUi::sizes`); zooming no longer touches the document. |
| R-13 | TD-14: no version control. | Git repository; each phase is a commit. (CI and `cargo deny` are still open: TD-26.) |
| R-14 | TD-16: two `#[allow(clippy::too_many_arguments)]`. | `Scene` and `Place` context structs; no `#[allow]` left in the workspace. |
| R-15 | TD-25: no line-ending policy. | `.gitattributes` (`eol=lf`) and `rustfmt.toml` (`newline_style = "Unix"`). |
| R-16 | TD-04: command IDs were strings in three lists; unknown IDs ignored; 49 of 70 ribbon items had no handler and nothing tracked it. | One `CommandId` table (a macro generates the enum, name, label and icon together). Key bindings are one table that gives both the behaviour and the tooltip text. `is_implemented` is an exhaustive match, and a test checks `dispatch` agrees with it. |
| R-17 | TD-05: `App` was a god object; `shell.rs` mutated it in 4 places; two files over the size limit. | `AppState` (data) + `dispatch` (the only mutator) + `panels/` (read-only, push commands). The size exceptions list is empty. |
| R-18 | TD-17: bar heights and similar were inline numbers. | `theme::metrics` holds the bar heights; the frame code uses them. |
| R-19 | TD-22 (part): no property tests and no corpus. | 22 property tests (unit algebra over the whole exponent range, no input can panic or yield a non-finite number, layout order never changes a result, plus a differential test against an independent reference interpreter) and 13 hand-checked textbook worksheets with a self-testing harness. Each audit bug was reintroduced to confirm the tests catch it. |
| R-20 | No performance baseline; the SPEC 5.6 targets were unmeasured. | `benches/recalc.rs`, a UI frame-time test, and `docs/PERFORMANCE.md`. A coarse tripwire in the gate. |

## Paid in the 2026-11-11 review

| ID | Was | Fix |
|---|---|---|
| R-21 | `cargo fmt --check` failed on CRLF files: a Windows clone with `core.autocrlf=true` fought `.gitattributes` and `rustfmt.toml`. | Working copy normalised to LF; `.editorconfig` added with a note to set `core.autocrlf false`. (That setting is per clone; git cannot enforce it from a tracked file.) |
| R-22 | The 400-line file rule was red: 12 files over the limit and an empty exceptions list (8 already so at the previous commit). | All 12 split into child modules with no behaviour change; the exceptions list is still empty. |
| R-23 | Functions up to about 230 lines (`tokenize`, `chart`, `polar::draw`, `editor::math`, `apply_inner`, `collect_names`, `evaluate_cancellable`, `dispatch`, `paint::math`, `table::paint`) and no limit on function length. | Split into named steps; 150-line limit added to the gate with no exceptions (see TD-36 for tightening it). |
| R-24 | `app/dispatch.rs` and `app/actions.rs` imported each other. | The two helpers that used `apply_canvas` and `with_eval_check` moved into `dispatch.rs`; imports run one way. |
