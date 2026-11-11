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
- [ ] `powershell -File scripts/check.ps1` passes end to end (fmt, clippy, tests and perf pass; the file-size step fails, see 1b)
- [ ] Commit or stash the 54 files of uncommitted work (1,926 insertions, 456 deletions)

### 1b. File-size gate fails (found while running item 1)
`scripts/line_limit_exceptions.txt` is empty, but 12 files exceed 400 production lines. Split each below the limit, or list it in the exceptions file as a ratchet.

- [x] `opencalc-core/src/csv.rs` (410)
- [ ] `opencalc-core/src/parser.rs` (424)
- [ ] `opencalc-core/src/edit/mod.rs` (507)
- [ ] `opencalc-core/src/eval/node.rs` (500)
- [ ] `opencalc-core/src/parser/program.rs` (470)
- [ ] `opencalc-ui/src/layout.rs` (521)
- [x] `opencalc-ui/src/app/dispatch.rs` (404)
- [ ] `opencalc-ui/src/canvas/mod.rs` (608)
- [x] `opencalc-ui/src/commands/id.rs` (405)
- [x] `opencalc-ui/src/layout/source.rs` (410)
- [ ] `opencalc-ui/src/panels/colour_picker.rs` (469)
- [ ] `opencalc-ui/src/ribbon/show.rs` (518)

### 2. Overlong functions
No function-length limit exists. Split the first four; the last four are large matches and may be fine.

- [ ] `opencalc-core/src/parser/lexer.rs:46` `tokenize` (~229 lines)
- [ ] `opencalc-ui/src/canvas/plot.rs:126` `chart` (~219 lines)
- [ ] `opencalc-ui/src/canvas/plot/polar.rs:88` `draw` (~206 lines)
- [ ] `opencalc-ui/src/canvas/editor.rs:39` `math` (~234 lines)
- [ ] Review only: `opencalc-core/src/edit/mod.rs:198` `apply_inner` (~215 lines)
- [ ] Review only: `opencalc-core/src/expr/names.rs:42` `collect_names` (~203 lines)
- [ ] Review only: `opencalc-core/src/eval.rs:181` `evaluate_cancellable` (~183 lines)
- [ ] Review only: `opencalc-ui/src/app/dispatch.rs:70` `dispatch` (~176 lines)
- [ ] Decide whether to add a function-length check to `scripts/check.ps1`

### 3. `docs/TECH_DEBT.md` out of date
- [ ] Re-audit the engine entries (last reviewed 2026-11-08)
- [ ] Add entries for solve blocks, programs and typeset program editing
- [ ] Split TD-29 into one row per decision
- [ ] Split TD-30 into one row per decision
- [ ] Update the "Last reviewed" date

## Minor

- [ ] Remove the production `unwrap`/`expect` in `opencalc-ui/src/logo.rs` and `opencalc-core/src/value/mod.rs` (the five in `benches/recalc.rs` are acceptable)
- [ ] Remove or justify the `unreachable!` calls:
  - [ ] `opencalc-core/src/functions.rs:282`
  - [ ] `opencalc-core/src/parser/forms.rs:22`
  - [ ] `opencalc-core/src/parser/program.rs:436`
- [ ] Share one constructor for the `RefCell` locals context, now repeated in:
  - [ ] `eval/node.rs`
  - [ ] `eval/calculus.rs`
  - [ ] `eval/calculus/ode.rs`
  - [ ] `eval/calculus/solve.rs`
  - [ ] `eval.rs`
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
