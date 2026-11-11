# OpenCalc

An original engineering worksheet written in Rust, using egui for the interface.
Free-form calculation notes with units, live recalculation and document export.

**Status:** early but usable. Worksheets with units, live recalculation, plots, custom units, find and replace, clipboard, undo, `.ocalc` files with crash recovery, and PDF and PNG export all work. See `docs/SPEC.md` for the full specification and roadmap, and `docs/FILE_FORMAT.md` for the file format.

## Layout

```
crates/
  opencalc-core/   document model, parser, evaluator, units (no UI code)
  opencalc-app/     desktop application (eframe / egui)  -> binary: opencalc
  opencalc-cli/    headless tool (check, later calc and export)
docs/
  SPEC.md          technical specification
  ARCHITECTURE.md  code structure, rules, refactor plan
  TECH_DEBT.md     register of known shortcuts
tests/corpus/     worksheets with known answers (see its README)
```

## Build and run

Requires the Rust stable toolchain (`rust-toolchain.toml` sets it up).

```
cargo run -p opencalc-app          # desktop app
cargo run -p opencalc-cli -- check some.ocalc --quiet
cargo test --workspace            # all tests
cargo clippy --workspace          # lints
powershell -File scripts/check.ps1   # all commit checks (fmt, clippy, tests, file sizes)
bash scripts/package.sh              # release build and archive in dist/ (Git Bash or Linux)
cargo packager --release -f wix      # the Windows installer (.msi), after package.sh
```

Windows and Linux are supported; macOS is not yet. On Windows the program carries its own icon and the worksheet icon; the installer (`installer/windows/main.wxs`, built with cargo-packager like OpenDraft's) installs for the current user without an administrator prompt, adds a Start menu shortcut and makes double-clicking an `.ocalc` file open it. Without the installer, Options has a button that does the file part. OpenCalc is a single window: starting it again, or double-clicking another worksheet, opens that worksheet as a tab in the running window. On Linux, `cargo packager --release -f deb,appimage` builds the packages (`installer/linux/`). `scripts/make_icons.py` and `scripts/installer_images.py` rebuild the icons and installer pictures from the logo. Third-party licence notices are made by `scripts/third_party.sh` (cargo-about) and shipped with every package.

GitHub Actions (`.github/workflows/ci.yml`) runs the checks on Windows and Linux; `release.yml` builds and publishes the packages when a version tag is pushed.

## Licence

GNU General Public License v3.0 or later (GPL-3.0-or-later). See `LICENSE`. This matches OpenDraft, so code can be shared between the two projects.
