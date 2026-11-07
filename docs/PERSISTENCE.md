# Persistence design (Milestone 3A): save, open, check

Status: **accepted** (decisions in section 8 settled: ZIP, `.ocx`, native dialogs via `rfd`, revision-based dirty).

**Progress:** S1 done (`opencalc-core/src/persist*`, `tests/persist.rs`, fixtures in
`tests/fixtures/v1/`). Measured on the reference machine: save of 500 regions 0.9 ms,
load 2.1 ms (budget 20 ms each, in the benchmark gate).

## 1. Where we are

- `Worksheet` already derives serde. It saves `title`, `regions`, `next_id`,
  `unit_system`, `custom_units` and `page`; `revision` counters are skipped. A math
  region saves its **source text only** and re-parses on load (decision D5).
- `FORMAT_VERSION = 1` is declared, and nothing reads it. There is no serializer,
  no file dialog, no dirty state, no recent files. Nothing survives closing the app.
- `FileOpen` and `FileSave` exist as dimmed commands; the Home page has no recent list.
- SPEC 5.1 already fixes the shape: a ZIP with `manifest.json` and `worksheet.json`,
  a `format_version` from day one, a migration function per version, atomic writes,
  and a separate recovery file for auto-save.

## 2. Principles

1. **Never lose data.** A failed save leaves the old file untouched; a failed open
   leaves the app as it was; closing a modified worksheet asks first.
2. **Loading is untrusted input.** A damaged, truncated, hostile or newer-than-us file
   gives a clear error, never a panic, hang or giant allocation (rule R6).
3. **Core stays pure** (R1). Core turns a `Worksheet` into bytes and back; it does no
   file or window work. The UI and the CLI own paths and dialogs.
4. **The same code loads for the app and the CLI**, so `opencalc check` verifies
   exactly what the app would open.
5. **Old files keep opening.** Committed fixture files of every released version are
   loaded by the tests forever.

## 3. The file

Extension `.ocx` (see decision 2). A ZIP (deflate) holding:

```
manifest.json    {"format_version":1,"app":"opencalc 0.0.1"}
worksheet.json   the serde form of Worksheet
```

- **What is saved:** the document only. Zoom, scroll, selection, the caret and the
  undo history are UI state and are not saved; a file opens fitted to the page width
  with a clean history.
- **Unknown future parts:** entries other than the two above are ignored on load
  (assets for pictures and CSV snapshots will live under `assets/`). Unknown JSON
  fields are ignored by serde, so a file written by a newer *minor* version still
  opens in the data it understands.
- **Newer major version:** `format_version` above ours is refused with
  "made by a newer OpenCalc (format N)". It is never half-read.
- **Migration:** `migrate(version, serde_json::Value) -> Value` steps one version at
  a time before the typed load. Version 1 has none to run; the chain and its test
  harness exist from the start.
- **Deterministic:** the same document gives the same bytes (no timestamps in the
  file; the ZIP entries get a fixed modification time), so a round trip is
  byte-identical and files diff cleanly. JSON is pretty-printed: engineers will keep
  calculation notes in version control, and the size cost is small.

### What load checks (beyond "it parses")

| Check | Why |
|---|---|
| ZIP: at most 8 entries, each at most 64 MB inflated, no path with `..` or an absolute path | zip bombs, path tricks |
| `manifest.json` at most 64 KB, valid, `format_version` known | clear errors |
| Region IDs unique and below `next_id`; `next_id` repaired upward if too low | IDs are never reused |
| Region count at most 50,000; every math source within `MAX_SOURCE_CHARS` | bounded memory and time |
| Positions and sizes finite; absurd values clamped into a sane range | NaN or infinity never reaches layout |
| `PageSetup` passed through `sanitised()` | existing rule |
| `custom_units` built through `UnitRegistry::new`: a bad symbol, scale, duplicate or reserved name is a load error naming the unit | the registry's own rules, applied to untrusted data |
| Plot specs within `MAX_SERIES` and the existing length limits | bounded |

A check that can be repaired safely (`next_id`, clamping) is repaired and reported as
a **load warning** shown once after opening ("1 problem was repaired"). One that
cannot (bad unit, duplicate IDs) refuses the file with the reason.

## 4. Code layout

- `opencalc-core/src/persist.rs` (+ `persist/{container,migrate,validate}.rs`):
  `save_bytes(&Worksheet) -> Vec<u8>` and
  `load_bytes(&[u8]) -> Result<Loaded, LoadError>`, where `Loaded` holds the
  worksheet and the list of repairs. New core dependencies: `zip`, and `serde_json`
  moves from dev to normal.
- `opencalc-core/src/persist/file.rs`: `write_atomic(path, bytes)` and
  `read_limited(path)`, std only. Atomic write is: temporary file in the same folder,
  write, flush and sync, then rename over the target (on Windows `std::fs::rename`
  replaces). Any failure removes the temporary file and leaves the target alone.
- UI: `OpenSheet` gains `path: Option<PathBuf>` and `saved_revision`. **Dirty** is
  `doc.revision() != saved_revision`. (Undoing back to the saved state still shows as
  modified; that is the safe error, and tracking it exactly is not worth the code.)
- Rename on Save As goes through an `Edit::SetTitle`, so the rule "every document
  change is an Edit" holds.

## 5. App behaviour

| Action | Behaviour |
|---|---|
| Save (Ctrl+S) | Untitled: asks for a path (Save As). Otherwise writes to the same path. The dirty mark clears only after the write succeeds. |
| Save As (Ctrl+Shift+S) | Native dialog, `.ocx` filter; adds the extension; asks before replacing. |
| Open (Ctrl+O) | Native dialog; a file already open just switches to its tab; the sheet opens fitted, clean history. A failure shows a message with the path and the reason. |
| Dirty mark | A dot before the tab title and in the window title. |
| Close tab / Quit with unsaved changes | In-app dialog: Save, Don't save, Cancel (naming the worksheet). Quit asks once per modified sheet. Cancel keeps everything. |
| Recent files | The last 10 paths, kept in app settings (eframe storage, as the theme is). The Home page lists them; a missing file says so and offers to remove it. |
| Command line and drag and drop | `opencalc FILE` and dropping a file on the window both open it. |
| Errors | Always in-app messages. A save error says nothing was changed on disk. |

Native dialogs use the `rfd` crate (blocking, on the UI thread, which is acceptable
for a modal file dialog). Tests do not open dialogs: the commands take a path
argument internally, and only the thin dialog wrapper is untested by machine.

## 6. `opencalc check` (headless)

`opencalc-cli check FILE...` loads, evaluates and reports; it exits **0** when every
region evaluates and there are no warnings, **1** when there are errors or warnings,
**2** when a file cannot be loaded. Each finding prints as
`FILE: region N (x, y): message` followed by the offending source, with the failing
sub-expression marked using the same span code the app underlines. `--quiet` prints
findings only. A JSON report is a later flag, not part of this step. (The binary
keeps its name `opencalc-cli`; SPEC's `opencalc check` spelling is a packaging
decision for later, since `opencalc` is the GUI.)

## 7. Auto-save and recovery (step S5, after the above)

A timer (every 30 s while modified) writes `<file>.recovery` beside the file, or to
the app data folder for an untitled sheet, using the same atomic write. The main file
changes only on an explicit save. At startup, a recovery file newer than its
worksheet (or one with no worksheet) prompts Restore or Discard; a clean save and a
clean close delete it. `*.oc.recovery` is already ignored by git; the pattern will be
updated to `*.ocx.recovery`.

## 8. Decisions needed

1. **Container.** ZIP as SPEC says (room for pictures and CSV snapshots, one more
   dependency), or one plain JSON file for now (simplest, diffs best, move to ZIP when
   the first asset arrives, as a version bump). *Recommendation: ZIP now.* The
   migration chain exists either way, and pictures and CSV import are both in this
   milestone, so a later switch would cost a migration for little saving.
2. **Extension.** `.ocx` (chosen; matches `.odx` in OpenDraft). Rejected: `.ocalc`, `.opencalc`, `.oc`.
3. **File dialogs.** `rfd` for native dialogs (recommended), or an in-app file
   browser (more work, looks the same on every platform, no new dependency).
4. **Dirty tracking.** Revision-based (recommended, simple, errs towards "modified"),
   or exact (compare against a saved copy of the document).

## 9. Steps

Each ends with the gate green, tests added, and a commit.

| Step | Content | Main tests |
|---|---|---|
| S1 | `persist` in core: container, manifest, migrate chain, validation, `LoadError`, repairs | Round trip of every corpus sheet and 200 generated worksheets is exact and byte-identical on a second save; committed `v1` fixture files load; truncating a valid file at every length, and flipping bytes across it, never panics and either errors or loads a valid document; zip bomb, bad paths, too-new version, bad custom unit, duplicate IDs, NaN positions |
| S2 | `write_atomic`, `read_limited` | Target unchanged when the write fails (read-only folder, missing folder); temp file never left behind; oversize file refused |
| S3 | UI: Save, Save As, Open, dirty mark, close and quit prompts, recent files, command line and drop | Headless smoke: edit, save, reopen gives the same document; the dirty mark follows edits and clears on save; closing a modified sheet prompts and Cancel keeps it; failure paths keep the sheet |
| S4 | `check` in the CLI | Exit codes on good, erroring, warning and corrupt files; output names the right region and source range |
| S5 | Auto-save and recovery | Recovery file written only when modified; restore after a simulated crash; discard; clean save removes it |
| S6 | `docs/FILE_FORMAT.md`: the format as a standalone specification | Doc examples are extracted and loaded by a test |

Performance: save and load of a 500-region worksheet join the benchmark budgets
(target under 20 ms each on the reference machine, to be measured, not assumed).
