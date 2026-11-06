# Technical debt register

Known shortcuts and gaps, with what each costs and when it must be paid. A living
file: add an entry when a shortcut is taken, delete it when it is paid. Design
context is in `ARCHITECTURE.md`.

**Last reviewed:** 2026-11-06 (after phases A and B).  **Size:** S under a session, M a few, L a milestone's worth.

## Open

### Engine (pay with Milestone 2: phase C)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-06 | SPEC 3.3 says the AST is the truth, but a Math region is saved as source text and the expression is derived (decision D5, adopted). The two are private and change together, so they cannot disagree. | A format version bump will be needed when the structured editor lands. | M2 editor | M |
| TD-08 | **Partly done (C2).** `Value` is now `Scalar` (complex, dimensioned) or `Matrix` (flat, one dimension). Still missing: text, booleans, ranges, element-wise functions on matrices, matrix powers and inverses, indexing, and typeset matrix display (matrices print as one line `2x2 [..]`). Vectors and matrices can only be built with `vec(..)` and `mat(r, c, ..)` until the Milestone 2 editor has matrix syntax. | Needed before ranges and the matrix editor. | Milestone 2 | M |
| TD-28 | Matrix elements share one dimension, so a mixed-unit vector such as `(1 m, 2 s)` is an error. | Some engineering vectors (position and time, state vectors) mix units. | If users hit it; needs per-element dimensions | M |
| TD-09 | **Partly done (C3).** `UnitRegistry` (built-ins plus per-worksheet custom units, validated) and `UnitSystem` (SI/USCS/CGS, display only) are worksheet settings changed by undoable edits; the status-bar selector works. Still missing: a UI to define or manage custom units (`UnitsDefine`/`UnitsManage`), per-unit display choices (`UnitsDisplay*`), compound units such as `kN*m` or `m/s` as unit symbols, temperature offsets (degC, degF; arithmetic policy undecided, so there are no offset units yet), a unit parsed at parse time is still limited to the built-in table (custom units work as names, so a variable of the same name shadows them), and loading a file must call `Worksheet::units()` and report invalid definitions. | Core feature (SPEC 2.3) mostly there. | Milestone 2 (unit UI) and Milestone 3 (loading) | M |
| TD-10 | **Mostly done (C4).** Parse errors carry a character range into the source (`ParseError`); evaluation errors carry a path to the failing sub-expression (`Output::error_paths`, `Expr::at_path`); the properties panel quotes both. Not done: underlining the range in the editor and highlighting the sub-expression on the canvas (needs the Milestone 2 editor and a layout-box map), and a path through a dependency (an error that comes from another region points at the name, not at the cause). | Error tracing is readable but not drawn in place. | Milestone 2 | S |
| TD-07 | The evaluator recomputes the whole sheet on every edit; there is no dirty propagation. **Measured (docs/PERFORMANCE.md): 500 regions in 0.39 ms against a 200 ms budget, so this is not needed.** | None at current scale. | Revisit only if a measurement after phase C or Milestone 2 approaches a budget | M |
| TD-20 | No cancellation (`AtomicBool`) for long computations. | Needed once integrals or root-finding exist. | When those land | S |
| TD-19 | `MAX_SYMBOLS = 128` per region. | Rejects a legitimately very long equation. | Revisit if frames shrink or the structured editor changes input shape; keep the stack-margin tests | S |
| TD-23 | Region IDs restart at 1 on every worksheet. | Cross-sheet references and global definitions (`≡`) need a worksheet-qualified ID. | With global definitions | S |

### Persistence and platform (Milestone 3)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-12 | No file format, autosave or migrations; `FORMAT_VERSION` is unused. | Cannot save work. | M3, after TD-01 | L |
| TD-13 | CLI `check` is a stub that prints "not implemented". | Review/CI use case (SPEC 5.3) unavailable. | After TD-01, TD-12 | M |
| TD-11 | Display is fixed at 4 decimal places with an automatic unit; no precision, scientific or engineering formats (SPEC 2.4); no user-chosen display unit. | Core display feature missing. | M2/M3 | M |
| TD-21 | Only tested on Windows 11. No Linux or macOS build or packaging. | Unknown portability problems. | M3 | M |

### Hygiene

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-15 | Typesetting is redone every frame for visible regions; only the *sizes* are cached. **Measured: 0.76 ms for a frame with 200 visible regions against 16.7 ms, excluding tessellation and the GPU.** | None at current scale. Tessellation and GPU time are not yet measured. | Revisit if a real-window frame is slow | M |
| TD-27 | Measured region sizes (document units) vary slightly with zoom, because of font rounding at small pixel sizes. "Fit" computed from sizes measured at another zoom undershoots, so it needs two passes to be exact (found by the performance test). | A small, visible imprecision in Fit. | When a layout cache is built, key it by zoom bucket | S |
| TD-26 | No continuous integration and no `cargo deny` / `cargo audit`. `scripts/check.ps1` is run by hand. | The gate only works if someone remembers to run it. Only Windows is exercised. | With the first Linux/macOS work (M3) | S |
| TD-18 | Placeholder controls that look real: the "12 pt" font-size slot. | Misleading until Format is built. | When Format tab is implemented | S |
| TD-22 | Property tests for unit algebra and a corpus of hand-checked worksheets are done (R-19). Still missing: golden tests for the parser (SPEC Milestone 2) and a unit-algebra property over custom units (needs the phase C registry). Smoke tests cannot reproduce timing races. | Parser output is checked indirectly, through evaluation. | M2 | S |
| TD-24 | Only Inter Regular is bundled (no bold or italic), so the Format tab's Bold and Italic cannot be implemented yet. Lucide icons are private-use codepoints and rely on font order. | Blocks rich text. | When Format tab is built | S |

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
