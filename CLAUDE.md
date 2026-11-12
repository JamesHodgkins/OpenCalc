# CLAUDE.md

OpenCalc: a Rust engineering worksheet on egui/eframe: free-form calculations with units, live recalculation, plots, and PDF/PNG export. Windows and Linux are the targets. Licence GPL-3.0-or-later. One of three sibling apps with OpenDraft and OpenPath (`../OpenDraft`, `../OpenPath`); keep the three consistent.

## Read first
- `docs/SPEC.md`: the specification and roadmap. `docs/ARCHITECTURE.md`: code structure and rules. `docs/TECH_DEBT.md`: the register of known shortcuts.
- `docs/HOUSE_STYLE.md`: the shared look and interaction language of the sibling apps.
- `docs/FILE_FORMAT.md`: the `.ocalc` format (a ZIP); `docs/PERSISTENCE.md`: saving, recovery and the decision log.
- `private/` is git-ignored scratch (comparison notes, drafts). Nothing in code or tests may depend on it.

## Crates (dependencies point down)
`opencalc-core` (document, parser, evaluator, units; no UI) <- `opencalc-app` (binary `opencalc`: the egui program) and `opencalc-cli` (binary `opencalc-cli`: `check`).

## Naming (shared by all three apps)
- Crates are `<product>-<role>` with the full lowercase product name, never a short prefix (a later app would collide). Roles: `-core`, `-ui` (widgets only), `-app` (the program), `-cli`, `-files`, and a format crate named after its format.
- Files: "o" plus the product word: `.ocalc` here, templates `.ocalct` (`.odraft`, `.opath` in the siblings). Environment variables are `OPENCALC_*`.

## Conventions
- Source files use LF line endings; set `git config core.autocrlf false` in your clone or `cargo fmt --check` fails on CRLF files (see `.editorconfig`).
- Files stay under about 400 production lines and functions under 150 lines; `scripts/check.ps1` enforces this with a ratchet file (`scripts/line_limit_exceptions.txt`, which may shrink but never grow).
- No `todo!` or `dbg!`; `unsafe` is forbidden (workspace lints). Clippy does not yet enforce `unwrap` here, but new production code should not add them.
- Keep the family consistent: when you change lints, CI, licence lists or policy files, make the same change in the siblings and in `../OpenAppTemplate` (the scaffold new apps start from).

## Checks (CI runs these on Windows and Linux)
```
powershell -File scripts/check.ps1     # fmt, clippy, tests, performance budgets, file sizes
```
Or the three commands: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Gate commits on the real exit codes.

## Working with the user (James)
- Commit as James <133605871+JamesHodgkins@users.noreply.github.com> with NO Co-Authored-By trailer. Never use any other email address; check `git config user.email` first.
- Work on a feature branch and push only when asked. Stage with `git add -u` plus named new files; check `git status` first.
- Finish with a short report: what changed, what is not done, and what to take next. Do not start new feature work without confirmation.
