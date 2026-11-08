# Technical debt register

Known shortcuts and gaps, with what each costs and when it must be paid. A living
file: add an entry when a shortcut is taken, delete it when it is paid. Design
context is in `ARCHITECTURE.md`.

**Last reviewed:** 2026-11-08 (stale persistence, CLI, CI and font entries corrected; engine entries not re-audited).  **Size:** S under a session, M a few, L a milestone's worth.

## Open

### Engine (pay with Milestone 2: phase C)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-06 | SPEC 3.3 says the AST is the truth, but a Math region is saved as source text and the expression is derived (decision D5, adopted). The two are private and change together, so they cannot disagree. | A format version bump will be needed when the structured editor lands. | M2 editor | M |
| TD-08 | **Partly done (C2).** `Value` is now `Scalar` (complex, dimensioned) or `Matrix` (flat, one dimension). Done since: ranges, indexing, element-wise functions on matrices, matrix integer powers and inverses (`M ^ n`). Still missing: text, booleans, and typeset matrix display (matrices print as one line `2x2 [..]`). Vectors and matrices can only be built with `vec(..)` and `mat(r, c, ..)` until the Milestone 2 editor has matrix syntax. | Needed before ranges and the matrix editor. | Milestone 2 | M |
| TD-28 | Matrix elements share one dimension, so a mixed-unit vector such as `(1 m, 2 s)` is an error. | Some engineering vectors (position and time, state vectors) mix units. | If users hit it; needs per-element dimensions | M |
| TD-09 | **Mostly done.** `UnitRegistry`, `UnitSystem` (display only), a Define/Manage units window, per-dimension display units, and load-time validation of custom units (`Worksheet::units()` in `persist/validate.rs`) all exist. Compound units work as expressions and display by name or as numerator over denominator (ROADMAP 1.1). Still missing: temperature display in degC or degF (input works), and a unit parsed at parse time is limited to the built-in table (custom units work as names, so a variable of the same name shadows them). | Unit coverage (roadmap Phase 1). | Roadmap Phase 1 | M |
| TD-10 | **Mostly done (C4).** Parse errors carry a character range into the source (`ParseError`); evaluation errors carry a path to the failing sub-expression (`Output::error_paths`, `Expr::at_path`); the properties panel quotes both. Not done: underlining the range in the editor and highlighting the sub-expression on the canvas (needs the Milestone 2 editor and a layout-box map), and a path through a dependency (an error that comes from another region points at the name, not at the cause). | Error tracing is readable but not drawn in place. | Milestone 2 | S |
| TD-07 | The evaluator recomputes the whole sheet on every edit; there is no dirty propagation. **Measured (docs/PERFORMANCE.md): 500 regions in 0.39 ms against a 200 ms budget, so this is not needed.** | None at current scale. | Revisit only if a measurement after phase C or Milestone 2 approaches a budget | M |
| TD-20 | **Resolved (Phase 2.1).** Repeated evaluations are metered (`eval/meter.rs`, 1,000,000 steps per region) and cancellable (`EvalError::Cancelled`, raised by the worker when a newer request arrives). | | | |
| TD-19 | `MAX_SYMBOLS = 128` per region. | Rejects a legitimately very long equation. | Revisit if frames shrink or the structured editor changes input shape; keep the stack-margin tests | S |
| TD-23 | Region IDs restart at 1 on every worksheet. | Cross-sheet references and global definitions (`≡`) need a worksheet-qualified ID. | With global definitions | S |

### Persistence and platform (Milestone 3)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-12 | **Resolved.** `.ocx` file format with `FORMAT_VERSION`, migration chain, atomic writes and crash recovery (see `PERSISTENCE.md`). | | | |
| TD-13 | **Resolved.** `opencalc check` loads, evaluates and reports worksheets (`crates/opencalc-cli`). | | | |
| TD-11 | **Mostly done.** Result format (decimals, scientific, engineering) and display units exist (`document/result_format.rs`, Math Formatting tab). Still missing: a user-chosen display unit beyond the fixed set (m, mm, in, N, kN). | Minor. | When asked | S |
| TD-21 | **Partly done.** CI builds and tests on Linux, and deb/AppImage packaging is configured. Not yet run by hand on a Linux desktop; macOS is not supported. | Possible Linux-only UI problems (fonts, file association, single-instance hand-off). | Before a Linux release | S |

### Hygiene

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-15 | Typesetting is redone every frame for visible regions; only the *sizes* are cached. **Measured: 0.76 ms for a frame with 200 visible regions against 16.7 ms, excluding tessellation and the GPU.** | None at current scale. Tessellation and GPU time are not yet measured. | Revisit if a real-window frame is slow | M |
| TD-27 | Measured region sizes (document units) vary slightly with zoom, because of font rounding at small pixel sizes. "Fit" computed from sizes measured at another zoom undershoots, so it needs two passes to be exact (found by the performance test). | A small, visible imprecision in Fit. | When a layout cache is built, key it by zoom bucket | S |
| TD-26 | **Partly done.** CI runs the gate on Windows and Linux (`.github/workflows/ci.yml`) and `cargo deny check licenses`. Still missing: `cargo audit` / the advisories check, and macOS (not supported). | Known vulnerable dependencies are not flagged. | Before a public release | S |
| TD-18 | **Resolved (U2).** The fake "12 pt" font-size slot and the ribbon's slot mechanism were removed rather than shipped as a control that does nothing. | | | |
| TD-22 | **Done (M2.1).** `tests/parser_golden.rs` records the tree for 75 inputs in `tests/golden/parser.golden`; accept a deliberate change with `UPDATE_GOLDEN=1` and read the diff. | | | |
| TD-29 | M2.1 language decisions to revisit: comparisons give 1 or 0 (no boolean type); equality is exact (no `TOL`); all comparison operators share one precedence level so `1 < 2 == 3` chains left to right; `ORIGIN` is an ordinary variable that indexing reads (default 0, as in Mathcad); `x_i` subscript entry is not parsed (indexing is `x[i]`, typeset as a subscript); `if` evaluates only the chosen branch but counts both for dependencies, so a cycle through an unused branch is still a cycle. ; ranges are lists (`Value::Series`) combined element by element while matrices keep linear-algebra rules, so `k ^ 2` squares each element and `M ^ 2` is the matrix product (whole-number powers only, `M ^ -1` is the inverse); `a, b..c` takes a step from the first two values and is only allowed where a comma cannot separate arguments, so `f(0, 2..10)` is two arguments; `stdev` is the population form (Mathcad) and `stdev_sample` divides by n - 1; a list or matrix cannot be a matrix cell or a range end. | Behaviour that is easy to change now and costly once worksheets exist. | Before Milestone 3 saves real files | S |
| TD-30 | M2.3 function decisions to revisit: no recursion (a function that reaches itself, directly or through others, is a circular reference, and so is every caller of it); calls may nest 12 deep (`MAX_CALL_DEPTH`, sized from a stack test: about 3 MB in debug at the limit against the worker's 8 MB); a function body reads variables from the worksheet, not from where it is called, and only its parameters are local; functions and variables are separate namespaces, and a function cannot take a built-in's name; an error inside a body is reported at the call as `in function 'f': ...` without a path into the body; `5 m` is always a quantity in metres even when `m` is a parameter or variable; no default arguments, no functions that return several values. | Recursion and local variables are the usual next requests from engineers. | When a worksheet needs them | M |
| TD-31 | M2.4 plot decisions to revisit: plots are painted with the canvas painter (like every region, so they scale with zoom and a drag moves the region), not with `egui_plot`, which needs its own `Ui` and would not follow the canvas zoom; revisit `egui_plot` for an interactive plot view (hover readout, pan and zoom, log axes) later; a plot has one y axis, so every series must share one dimension; axis bounds are automatic; series are expressions that evaluate to a list or vector (no `f(x)` function-plotting syntax, no parametric or bar plots); plot data is evaluated after all math regions, so a plot can read anything and nothing can read a plot; the plot editor lives in the properties panel, not on the canvas; line colours are a fixed cycle of eight. | An interactive plot is a likely request once real data is plotted. | Milestone 3 or when asked | M |
| TD-24 | Inter Bold is bundled (`Inter-Bold.ttf`); no italic face is, so italic cannot be a true italic. Lucide icons are private-use codepoints and rely on font order. | Limits rich text (roadmap Phase 5). | With rich text | S |
| TD-32 | Phase 2.1 calculus decisions to revisit: the forms are function-call syntax (`integrate(f, x, a, b)`), not typeset notation; integrals, derivatives and limits are numerical with stated tolerances and refuse when they cannot reach them (no symbolic result, no improper integrals, first derivative only, two-sided limits only); `sum_over`/`product_over` refuse an empty range; the two bounds of an integral must have the same dimension, so `0` and `3 m` do not mix; an unevaluated bound variable cannot be a unit name written after a number (`2 m` is parsed as a quantity before the variable is known), so use names like `x`, `t`, `k`; the budget counts evaluations, not time; `root` finds real roots of real functions only and, from a guess, the nearest sign change rather than the nearest root; `polyroots` reads coefficients lowest power first and reports repeated roots only to about 1e-8. | Behaviour to settle before the structured editor gives these forms notation. | With the structured editor | S |

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
