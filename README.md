# OpenCalc

An original engineering worksheet written in Rust, using egui for the interface.
Free-form calculation notes with units, live recalculation and document export.

**Status:** Milestone 1 (skeleton): window shell and free-form canvas working. See `docs/SPEC.md` for the full specification and roadmap.

## Layout

```
crates/
  opencalc-core/   document model, parser, evaluator, units (no UI code)
  opencalc-ui/     desktop application (eframe / egui)  -> binary: opencalc
  opencalc-cli/    headless tool (check, later calc and export)
docs/
  SPEC.md          technical specification
  ARCHITECTURE.md  code structure, rules, refactor plan
  TECH_DEBT.md     register of known shortcuts
  CLEAN_ROOM.md    provenance log for every feature's source
tests/corpus/     worksheets with known answers (see its README)
```

## Build and run

Requires the Rust stable toolchain (`rust-toolchain.toml` sets it up).

```
cargo run -p opencalc-ui          # desktop app
cargo run -p opencalc-cli -- check some.oc
cargo test --workspace            # all tests
cargo clippy --workspace          # lints
powershell -File scripts/check.ps1   # all commit checks (fmt, clippy, tests, file sizes)
```

## Clean-room policy

OpenCalc is written from public behaviour and original design. Commercial and freeware products are used for reference only. See `docs/SPEC.md` section 9.1 and `docs/CLEAN_ROOM.md`.

## Licence

GNU General Public License v3.0 or later (GPL-3.0-or-later). See `LICENSE`. This matches OpenDraft, so code can be shared between the two projects.
