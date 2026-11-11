# Audit fixes (temporary)

Tracks the findings from the 2026-11-11 code audit. Delete this file when every box is ticked, and move anything left over into `docs/TECH_DEBT.md`.

Audit basis: clippy clean, no TODO/FIXME, one `#[allow]`, about 60k lines in 229 files. Function lengths were counted roughly (one `fn` to the next).

## Worth fixing

### 1. Formatting gate fails (line endings)
`core.autocrlf=true` fights `.gitattributes` (`eol=lf`) and `rustfmt.toml` (`newline_style = "Unix"`), so `cargo fmt --check` fails in about 11 files.

- [x] `git config --local core.autocrlf false` for this repo
- [x] Re-normalise the working copy to LF (no CRLF files remain)
- [x] `cargo fmt --all`
- [x] `cargo fmt --all --check` passes
- [x] `powershell -File scripts/check.ps1` passes end to end
- [x] Commit the uncommitted work (checkpoint commit on branch `audit-fixes`)

### 1b. File-size gate fails (found while running item 1)
`scripts/line_limit_exceptions.txt` is empty, but 12 files exceed 400 production lines. Split each below the limit, or list it in the exceptions file as a ratchet.

- [x] `opencalc-core/src/csv.rs` (410)
- [x] `opencalc-core/src/parser.rs` (424)
- [x] `opencalc-core/src/edit/mod.rs` (507)
- [x] `opencalc-core/src/eval/node.rs` (500)
- [x] `opencalc-core/src/parser/program.rs` (470)
- [x] `opencalc-ui/src/layout.rs` (521)
- [x] `opencalc-ui/src/app/dispatch.rs` (404)
- [x] `opencalc-ui/src/canvas/mod.rs` (608)
- [x] `opencalc-ui/src/commands/id.rs` (405)
- [x] `opencalc-ui/src/layout/source.rs` (410)
- [x] `opencalc-ui/src/panels/colour_picker.rs` (469)
- [x] `opencalc-ui/src/ribbon/show.rs` (518)

### 2. Overlong functions
No function-length limit exists. Split the first four; the last four are large matches and may be fine.

- [x] `opencalc-core/src/parser/lexer.rs` `tokenize` (now a loop over `symbol`, `text`, `number`, `unknown`)
- [x] `opencalc-ui/src/canvas/plot.rs` `chart` (now `plot/cartesian.rs`: plotting_area, ticks, series, captions)
- [x] `opencalc-ui/src/canvas/plot/polar.rs` `draw` (now `Disc` with rings_and_spokes, series; stand_in_scales)
- [x] `opencalc-ui/src/canvas/editor.rs` `math` (now refuse_long, prepare_field, pointer_input, key_events, paint, autocomplete)
- [x] `opencalc-core/src/edit/mod.rs` `apply_inner` now one-line arms calling `edit/content.rs`
- [x] `opencalc-core/src/expr/names.rs` `collect_names` (call forms moved to `call_names`)
- [x] `opencalc-core/src/eval.rs` `evaluate_cancellable` (planning steps moved to `eval/plan.rs`; stack and perf tests pass)
- [x] `opencalc-ui/src/app/dispatch.rs` `dispatch` (view and window commands moved to `app/view.rs`; the rest is a flat command table)
- [x] Function-length check added to `scripts/check.ps1` (150 lines, no exceptions list; `paint::math` and `table::paint` were fixed first)

### 3. `docs/TECH_DEBT.md` out of date
- [x] Re-audit the engine entries (TD-06, 08, 10, 19 corrected against the code; others checked only as noted in the commit message)
- [x] Add entries for solve blocks, programs and typeset program editing (TD-33, TD-34, TD-35)
- [x] Split TD-29 into one row per decision (TD-29.1 to 29.10; TD-30, 31 and 32 split too)
- [x] Split TD-30 into one row per decision (TD-30.1 to 30.7)
- [x] Update the "Last reviewed" date

## Minor

- [x] Production `unwrap`/`expect`: `value/mod.rs` one removed (`Series::negated`); the logo one stays (the PNG is embedded at build time); the bench ones are acceptable
- [x] Remove or justify the `unreachable!` calls (all three removed):
  - [x] `opencalc-core/src/functions.rs:282` (now an error)
  - [x] `opencalc-core/src/parser/forms.rs:22` (`try_from` into an array)
  - [x] `opencalc-core/src/parser/program.rs:436` (now `parser/statement.rs`, matched directly)
- [x] Share one constructor for the `RefCell` locals context (`Cx::top` and `Cx::scoped`), replacing:
  - [x] `eval/node.rs` (now `eval/node/call.rs`)
  - [x] `eval/calculus.rs`
  - [x] `eval/calculus/ode.rs`
  - [x] `eval/calculus/solve.rs`
  - [x] `eval.rs`
- [ ] Review the 40 `let _ =` in production code; confirm none hide a save, load or recovery error
- [ ] Consider splitting `opencalc-ui/src/smoke.rs` (1,743 lines) into topic files like the existing `smoke/` ones
- [ ] Spot-check per-frame paint code for hot-path `.clone()` calls (373 in the workspace overall)

## Already in the register (not new)

- [ ] TD-26: add `cargo audit` / advisories check to CI
- [ ] TD-21: run the Linux build by hand on a desktop

## Done when

- [ ] Every box above is ticked or deliberately dropped with a note
- [ ] `scripts/check.ps1` passes
- [ ] This file is deleted
