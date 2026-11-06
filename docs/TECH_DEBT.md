# Technical debt register

Known shortcuts and gaps, with what each costs and when it must be paid. A living
file: add an entry when a shortcut is taken, delete it when it is paid. Design
context is in `ARCHITECTURE.md`.

**Last reviewed:** 2026-11-06.  **Size:** S under a session, M a few, L a milestone's worth.

## Open

### Structural (pay before Milestone 2: phases A and B)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-01 | Document model (`Sheet`, `Region`) lives in `opencalc-ui` and holds egui types and UI state. | CLI cannot open a worksheet; no file format; core is not testable alone. Violates R1, R2. | Before M3; do in phase A | L |
| TD-02 | No single mutation path; no undo. Regions are edited in three places. | Each new edit path is later rewritten for undo. Violates R3. | Phase A | M |
| TD-03 | Auto-measured region size is stored in the document and rewritten when zoom changes. | Zooming dirties the document; pollutes future undo and file diffs. | With TD-02 | S |
| TD-04 | Command IDs are strings defined in three lists; unknown IDs are silently ignored. 49 of 70 ribbon items (33 of 54 plain items plus the 16 Greek-letter flyout items) have no handler and nothing tracks it. | Typos are invisible; ribbon, shortcuts and menu can disagree. Violates R5. | Phase B | M |
| TD-05 | `App` is a god object (11 `pub` fields); `shell.rs` mutates it directly in 4 places; `app.rs` (488) and `shell.rs` (419) exceed the 400-line limit. | Violates R4 and R8. Ratchet in `scripts/line_limit_exceptions.txt`. | Phase B | M |
| TD-16 | Two `#[allow(clippy::too_many_arguments)]` (`canvas/mod.rs`, `canvas/paint.rs`). | Signals missing context structs. | Phase B | S |

### Engine (pay with Milestone 2: phase C)

| ID | Debt | Impact | Pay by | Size |
|---|---|---|---|---|
| TD-06 | A Math region holds both `source` and `expr`; the AST is not the single truth (SPEC 3.3). | The file format cannot be fixed until decided (D5). | Decide now; resolve with M2 editor | M |
| TD-08 | `Quantity` is a real `f64`. No `Value` enum: no vectors, matrices, complex, text, booleans. | Blocks ranges, vectors, `sqrt(-4) = 2i`. | Phase C, before M2 vectors | M |
| TD-09 | Units are a hard-coded `match` (`builtin_unit`). No `UnitRegistry`, custom units, per-worksheet unit system, or temperature offsets. The SI/USCS/CGS selector is now shown disabled in the status bar and unhandled in the ribbon (was clickable and did nothing). | Core feature (SPEC 2.3) missing; misleading control. | Phase C; disable the control now (owner decision 4) | M |
| TD-10 | Errors carry no source position (`Expr::Error(String)`, `EvalError`). | SPEC "error tracing" cannot highlight the failing sub-expression. | With spans in phase C | M |
| TD-07 | Evaluator recomputes the whole sheet on every edit; no dirty propagation; no benchmark for the 200-region / 500-region targets (SPEC 5.6). | Unknown until measured. | Add benchmark in phase C; build incremental recalculation if over budget | M |
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
| TD-14 | Not a git repository. No continuous integration, no `cargo deny` / `cargo audit`. | No history, no review points for the A and B refactors, no automated gate. | **Now**, needs owner approval | S |
| TD-15 | Typesetting is redone every frame for visible regions; no layout cache. | Fine at 10 regions; unknown at 200 (target 60 fps). | Measure in phase C; cache in phase A (see TD-03) | M |
| TD-17 | Layout numbers (bar heights 32/25/26/24, ribbon sizes) are inline in places rather than in `theme::metrics`. | Drift from the spec's metrics. | Phase B | S |
| TD-18 | Placeholder controls that look real: the "12 pt" font-size slot. | Misleading until Format is built. | When Format tab is implemented | S |
| TD-22 | No property tests for unit algebra or golden tests for the parser (SPEC Milestone 2 asks for both). Smoke tests are single-threaded and cannot reproduce timing races. | Algebra bugs are found by luck. | M2 | M |
| TD-24 | Only Inter Regular is bundled (no bold or italic), so the Format tab's Bold and Italic cannot be implemented yet. Lucide icons are private-use codepoints and rely on font order. | Blocks rich text. | When Format tab is built | S |
| TD-25 | No `rustfmt.toml` or `.gitattributes`; line endings are whatever the editor wrote. | Noisy diffs once in git, on mixed platforms. | With TD-14 | S |

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
