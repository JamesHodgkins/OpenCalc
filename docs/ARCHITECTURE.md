# OpenCalc architecture

How the code is organised, the rules that keep it that way, and the plan to get
from where it is to where it should be. `SPEC.md` says *what* to build; this says
*how the code is shaped*. `TECH_DEBT.md` is the register of known shortcuts.

**Status:** written after the Milestone 1 review, 2026-11-06. **Phases A and B
landed the same day** (section 7). Section 3 shows the code as it was before
them, with the rows they fixed marked *done*; section 2 shows the code as built.

---

## 1. Layers and dependency rules

```
   opencalc-cli ──┐
                  ├──▶  opencalc-core      std, serde, thiserror. No egui, no windowing.
   opencalc-app  ──┘
```

| # | Rule | Why |
|---|---|---|
| R1 | `opencalc-core` never names an egui, eframe or winit type. | The CLI, file loader and tests must run without a window (SPEC 3.1, 4.10). |
| R2 | The **document** (what is saved) and **UI state** (selection, caret, view, drag) are different types in different modules. | A save must never depend on where the user scrolled; undo must not record a selection. |
| R3 | **Every change to a document is an `Edit`** applied through one function that returns its inverse. | Undo/redo, autosave and the file format all hang off one choke point. Mutation sprinkled through the UI makes each of them a rewrite. |
| R4 | UI panels **read** state and **return commands**. They never mutate application state. | Keeps the egui pass free of borrow fights (SPEC 4.4) and makes "every action is a named command" true. |
| R5 | Every user action is a **command with a stable ID**, defined once. | Menus, ribbon, shortcuts, tooltips and the keymap share one table (SPEC 4.7). |
| R6 | **No user input can panic, hang or overflow the stack.** Limits are explicit, tested, and return an error value. | A calculation note is a professional record; the app must never lose work to a typo. |
| R7 | **No silently wrong answers.** Non-finite numbers, dimension mismatches and overflow are errors, never values. | The core promise is trustworthy results (SPEC 1.1). |
| R8 | About 400 production lines per file; no `unwrap`/`todo!`/`dbg!`/prints in production code. The CLI binary's output layer is exempt from the print rule. | Readability and reviewability (SPEC 4.10). Enforced by `scripts/check.ps1`. |
| R9 | A bug fix lands with a test that fails without it. | The review in section 6 found five bugs that unit tests had missed; each now has a regression test. |

---

## 2. Module map

As built after phases A and B. Items marked *planned* do not exist yet.

```
crates/opencalc-core/src/
  expr.rs            AST (Expr)
  parser.rs          Pratt parser; MAX_SYMBOLS limit                   (spans: planned, C)
  units.rs           Dimension, Quantity, built-in units               (-> units/ + registry: planned, C)
  eval.rs            dependency graph, Tarjan, evaluation              (-> eval/ + Value: planned, C)
  worker.rs          background evaluation, revision protocol, panic safety
  document/          Worksheet, Region, RegionKind, MathRegion; own Point and Size
  edit/              Edit enum, Worksheet::apply -> inverse; History (undo/redo, coalescing)
  file/              container, manifest, migrations                   (planned, M3)

crates/opencalc-app/src/
  main.rs            entry point and window size
  app/               mod.rs (App: worker, ribbon, effects), state.rs (AppState: data only),
                     dispatch.rs (command -> state change + Effects), frame.rs (panel layout,
                     key bindings)
  commands/          id.rs (CommandId table, one macro), keys.rs (key bindings),
                     registry.rs (what is enabled and active right now)
  sheet.rs           OpenSheet: a core Worksheet + History + view + canvas state
  panels/            title, doc_tabs, names, properties, status, home, about, slots:
                     read-only views that push commands
  ribbon/            mod.rs, defs.rs (data: command IDs only), show.rs, widgets.rs
  canvas/            mod.rs (widget), view.rs (transform), hit.rs, paint.rs, adapt.rs
                     (core <-> egui types, measured sizes)             (layout cache: planned, TD-15)
  theme.rs  fonts.rs  icons.rs  format.rs  layout.rs (typesetter)
  smoke.rs           headless end-to-end tests

crates/opencalc-cli/src/
  main.rs            check / calc, using core only                     (needs core::file, M3)
```

---

## 3. Where the code stands

Snapshot, 2026-11-06: about 6,600 lines of Rust (including tests), 105 tests (45 core, 60 UI), clippy clean.

| Concern | Today | Problem | Target | Phase |
|---|---|---|---|---|
| Document model | `ui/sheet.rs`, holds `egui::Pos2/Rect/Vec2`, and also `CanvasView` and selection state inside `Sheet` | Violates R1 and R2. The CLI cannot open a worksheet; nothing can be saved without dragging egui in. | `core::document` with own geometry types; UI keeps view and selection in a separate `SheetView` keyed by worksheet ID. | A: **Done (phase A).** |
| Mutation | Three places edit regions directly (`app.rs` insert, delete, `apply_doc_cmds`; `sheet.rs` add/remove) | Violates R3. No undo possible. Move is applied every frame; typing re-parses per keystroke. | `Edit` enum + `Worksheet::apply` returning the inverse; `History` with coalescing (a drag is one step, a burst of typing is one step). | A: **Done (phase A).** |
| Auto-measured size | Stored in `Region.size`; rewritten when the zoom changes (font rounding) | Zooming dirties the document. | `LayoutCache` in the UI keyed by `(RegionId, content revision, zoom bucket)`; the document stores only user-set size. | A: **Done (phase A).** |
| Commands | Strings in three lists (`HANDLED`, `run()` match arms, ribbon defs). Unknown IDs fall through `_ => {}`. | Typos are silent. 49 of 70 ribbon items (33 of 54 plain items plus the 16 Greek-letter flyout items) have no handler and nothing tracks that. | `CommandId` enum, one table with label, icon and shortcut; `match` is exhaustive. | B: **Done (phase B).** |
| App structure | `App` has 11 `pub` fields; `shell.rs` mutates it in 4 places, bypassing the command queue. `app.rs` is 488 lines, `shell.rs` 419 (limit 400). | Violates R4 and R8. | `AppState` + `dispatch(state, cmd) -> Effects`; panels take `&AppView`, return `Vec<CommandId>`. | B: **Done (phase B).** |
| Math source | A region holds both `source` (text) and `expr` (parsed) | SPEC 3.3 says the AST is the truth. Unresolved, so the file format cannot be fixed. | **Decision D5 below.** | with M2 |
| Evaluator | Whole sheet recomputed per edit; `Quantity` is a real `f64`; functions and units are hard-coded matches | No vectors, matrices, complex, custom units or incremental recalculation. | `Value` enum; `UnitRegistry` passed to the evaluator; dirty propagation when measured necessary. | C |
| Errors | `Expr::Error(String)`, `EvalError` with no position | SPEC "error tracing" cannot highlight a sub-expression. | Spans on tokens and errors. | C |
| Persistence | None. `FORMAT_VERSION` is defined and unused. | Blocked by the document model living in the UI. | `core::file` per SPEC 5.1. | M3 (after A) |

---

## 4. Decisions

**D1. The document model lives in `core`, with its own geometry types.**
Plain `Point`, `Size`, `Rect` (f32, `Serialize`). Alternative considered: keep egui
types and feature-gate them; rejected because it still puts a UI dependency in the
crate that the CLI and loader link against. Conversion to egui types is a handful of
one-line `From` impls in the UI.

**D2. One mutation path, with inverses.**
```rust
pub enum Edit {
    InsertRegion { region: Region, index: usize },
    DeleteRegions { regions: Vec<(usize, Region)> },        // keeps positions for undo
    MoveRegions { moves: Vec<(RegionId, Point, Point)> },   // id, from, to
    SetMathSource { id: RegionId, from: String, to: String },
    SetText { id: RegionId, from: String, to: String },
    SetRegionSize { id: RegionId, from: Size, to: Size },   // user resize only
}
impl Worksheet { pub fn apply(&mut self, e: Edit) -> Result<Edit, EditError>; }  // returns inverse
```
`History` stores applied edits, coalesces consecutive `MoveRegions` of the same
region set and consecutive `SetMathSource` on one region, caps its depth, and clears
redo on any new edit (SPEC 5.5). The canvas keeps emitting intent (a drag in progress
is UI state); an `Edit` is created when the drag ends.

**D3. `CommandId` is an enum, not a string.**
Exhaustive `match` turns "handler missing" into a compile error or an explicit
`Unimplemented` arm, instead of a silent no-op. `as_str()` gives the stable ID used by
a future keymap file. Alternative: keep `&'static str` and add a startup self-check;
rejected as weaker for the same cost.

**D4. Panels are pure functions of a read-only view.**
`fn names_panel(ui, &AppView) -> Vec<CommandId>`. Where a panel needs to change
something (the status bar's Live toggle, closing a tab, the About window), that is a
command too.

**D5. What is canonical for a Math region: the source text or the AST? (open)**
- *Recommend:* persist the **source text** as canonical until the structured math
  editor (Milestone 2) exists; `expr` is derived and `#[serde(skip)]`. The file format
  then commits to nothing about the AST, which is still changing (vectors, complex,
  units-as-expressions). When the editor lands, a format version bump migrates text to
  AST, using the migration scaffolding that SPEC 5.1 already requires.
- *Alternative:* persist the AST now. Honours SPEC 3.3 literally, but every AST change
  until M2 ends is a file-format migration.

**D6. Evaluation boundaries.**
`UnitRegistry` (built-ins plus the worksheet's custom units) is passed into
`evaluate`; `builtin_unit` stops being a global function. `Value` replaces
`Quantity` as the evaluator's result type (scalar, complex, vector, matrix, text,
bool) before ranges or vectors are written. Both are core-internal changes; the UI
sees them only through `format.rs`.

**D7. Robustness policy (implemented; see section 6).**
Parse limit of 128 symbols per region; checked unit arithmetic; non-finite results are
`EvalError::NotFinite`; the worker survives an evaluator panic; results are accepted
only for the current revision. When complex numbers arrive, `sqrt(-4)` stops being an
error and becomes `2i`; the non-finite check stays for genuine infinities.

**D8. One worker thread, revision protocol.**
Unchanged from SPEC 5.4 except for the strict rule: the UI applies a result only if
`result.revision == current revision`. Safe because the worker always evaluates the
newest queued request, so the current revision's result is always coming.

---

## 5. Conventions

- **Naming:** `Edit` for document changes, `Command`/`CommandId` for user actions,
  `Effect` for what dispatch asks the shell to do (open a dialog, close the window).
- **Errors:** library crates return `Result` with `thiserror` enums; user input never
  reaches `unwrap`/`expect`; `anyhow` only in binaries.
- **State ownership:** the document is owned by core types; the UI owns `SheetView`
  (zoom, offset, selection, editing, drag). IDs, not indices, cross every boundary.
- **Constants:** layout numbers live in `theme::metrics`, not inline.
- **Tests:** pure logic gets unit tests next to it; anything that crosses the UI gets a
  `smoke.rs` test driving real frames headlessly; every bug fix gets a regression test.

---

## 6. Robustness review, 2026-11-06

Five defects found by probing, all fixed, each with a regression test. Kept here
because they show what kind of problem to look for next.

| # | Defect (found by) | Effect | Fix |
|---|---|---|---|
| 1 | `(1 m^2)^100` overflowed an `i8` dimension exponent (probe) | Debug: panic on the worker thread, which ended evaluation permanently ("worker stopped"). Release: wrapped silently to a wrong dimension. | Checked arithmetic; `UnitError::DimensionOverflow`. |
| 2 | `1/0`, `sqrt(-4)`, `ln(0)`, `10^400` returned `Ok(NaN/inf)` (probe) | Displayed as numbers. | One choke point in `eval()` rejects non-finite values: `EvalError::NotFinite`. |
| 3 | About 2,000 nested parentheses overflowed the stack (probe) | Aborted the whole process, from the UI thread, on a pasted line. | `MAX_SYMBOLS = 128`, chosen by measurement (below). |
| 4 | Results were applied if `revision > applied` (code reading) | A late result from the previous sheet could be shown on the new one; region IDs restart at 1 per sheet. | `Worker::poll_for(revision)` accepts only the current revision. |
| 5 | `5 N` displayed as `0.0050 kN`, `100 Pa` as `0.0001 MPa` (probe) | Misleading display. | Per-dimension unit table choosing the largest unit giving a value of at least 1. |

Also: the worker now catches an evaluator panic and reports it on every region
(`EvalError::Internal`), runs on an explicit 8 MB stack, and a thread that cannot be
created makes `request` return `false` rather than panic.

**How the stack limit was chosen.** Debug builds use about 3 KB of stack per
nesting level in the worst shape (a chain of unary minus signs). Measured at 512
symbols: overflow even at 1 MB. At 128 symbols: parse, clone, evaluate and drop fit in
384 KB, and a complete frame (typeset and paint) fits in 512 KB, about half of the
1 MB Windows main thread. Both margins are tested (`parser.rs` at 512 KB,
`smoke.rs` at 1 MB). 128 symbols is several times the length of any real equation, but
it is a real limit: raise it only together with those tests.

---

## 7. Plan

Do the structural work **before** Milestone 2. Every new feature written first (math
editor, vectors, undo-able edits) would be written twice.

| Phase | Work | Exit check | Unblocks |
|---|---|---|---|
| **A** Document in core (**done 2026-11-06**) | `core::document` with own geometry; `Edit` + `apply` + `History`; measured sizes moved out of the document into `CanvasUi`; UI state in `OpenSheet`. | Met: `cargo tree -p opencalc-core` has no UI crate and the source has no `egui` mention. Undo/redo works for insert, delete, move and typing, through the ribbon and the keyboard (smoke tests). | Undo (M3), file format, CLI `check`, headless document tests. |
| **B** UI structure (**done 2026-11-06**) | `CommandId` enum and one command table. `AppState` + `dispatch`. `shell.rs` split into `panels/`, `app.rs` into `app/`. Context structs replaced both `too_many_arguments` allows. | Met: the size-limit exceptions list is empty (largest file 394 lines); no panel takes `&mut` state; every command is either handled by `dispatch` or marked unimplemented, and a test keeps the two in agreement. | New features land in small files; consistent behaviour across ribbon, shortcuts, menu. |
| **C** Evaluator foundations | **C1 (done):** corpus, property tests, benchmark. Then `numeric` module and `Value` with complex numbers (C2), `UnitRegistry` and unit systems (C3), function table and error paths (C4), re-measure (C5). | Custom units resolve; a vector literal evaluates; an error highlights its sub-expression; benchmark numbers recorded and within budget. | Milestone 2 proper. |
| **M2** Core logic | Math editor, ranges, vectors, functions, plots (SPEC section 6). | SPEC exit criterion. | |
| **M3** Persistence | `core::file`, autosave, export, CLI. | SPEC exit criterion. | |

Phases A and B each leave the app working and all checks green; neither changes
behaviour the user can see, apart from gaining undo in A. Phase C can interleave with
M2 once `Value` exists.

---

## 8. Quality gates

`powershell -File scripts/check.ps1` runs: `cargo fmt --all --check`, clippy with
`-D warnings`, `cargo test --workspace`, then the file-size rule. Files already over the
limit are in `scripts/line_limit_exceptions.txt` with their current size; the ratchet
lets them shrink but never grow, and fails if one is back under the limit but still
listed. Not yet in place: version control, continuous integration, dependency audit
(`cargo deny`), benchmarks. See `TECH_DEBT.md`.

---

## 9. Decisions

Resolved 2026-11-06: phases A then B before Milestone 2 (A is done); D5 adopted
(source text is canonical); `git init`; the unit-system selector disabled until
units are per-worksheet. As originally put:

1. **Approve phases A then B before Milestone 2.** (Recommended.)
2. **D5:** persist Math regions as source text until the structured editor exists?
   (Recommended.)
3. **Version control:** `git init` now, so the refactors in A and B are reviewable
   steps. Nothing is committed today.
4. **Unit system selector:** it is clickable but does nothing. Disable it until units
   are per-worksheet (phase C), or keep it as a visible stub?
