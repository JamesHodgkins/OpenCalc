# Data in and out: design (Phase 4)

Status: **The CSV module, the Table region with CSV import and export, and the Picture region (PNG only) are built; Excel, JPEG and image to matrix remain proposals.** Needs the
decisions in section 7 before the Table and Picture regions. Written 2026-11-08.
Roadmap context: `ROADMAP.md` Phase 4.

Labels: **[PTC]** documented by PTC, **[decision]** a choice for this repository.

## 1. What exists today

- `RegionKind::Table` and `RegionKind::Picture` are insertable **placeholders**: a
  box with no content. They are saved as the bare strings `"Table"` and `"Picture"`,
  which `FILE_FORMAT.md` reserves. Version 1 files may already contain them.
- 22 production files mention these two variants (paint, hit-testing, find, edit,
  validation, clipboard, labels, export). Each already has an intentional arm for
  "no content"; they all need revisiting when a variant gains data.
- The `.ocx` container is a ZIP with `manifest.json` and `worksheet.json`; extra
  entries are ignored, at most 8 entries are accepted (`FILE_FORMAT.md` section 1), and
  the project's notes already expect `assets/` for pictures and data snapshots.
- Matrices can now have a unit per element (TD-28 resolved), so a table whose
  columns have different units evaluates to one matrix.
- Dependencies in the build: `png` (a direct dependency of `opencalc-ui`, used by
  PNG export) and `zip`. **Not** in the build: a CSV parser, a JPEG decoder, an Excel
  reader or writer.

## 2. Principle: no file access during evaluation

**[decision]** Evaluating a worksheet never reads or writes a file. Data comes in by a
**command** that copies it into the document (import) and goes out by a command that
copies results out (export). PTC's own worksheets read files with functions
(`READPRN`, `WRITEPRN`); OpenCalc deliberately does not, for these reasons:

- A worksheet someone else sends you must not read, or overwrite, files on your
  machine when it opens. A function that takes a path makes every worksheet a program
  with that power.
- Evaluation stays a pure function of the document. The same worksheet gives the same
  answer on any machine and in the command-line checker, with no "file not found".
- The dependency graph then sees every input: an imported table is a region, so
  changing it recalculates, undoes, and is saved with the worksheet.
- A snapshot is what an engineering record needs: the numbers the calculation used,
  stored beside it.

The cost is that data does not refresh when the source file changes; the user imports
again. A live link can be added later as an explicit, visible, permissioned feature.

## 3. Table region

A grid of numbers that **defines a name**, like a Math region `T := matrix`, so
other regions use it by name. Its value is a matrix. **[PTC]** Mathcad's input table
region is the model; its component-based Excel link is out of scope.

- **Data** (saved as text, so a number round-trips exactly): a name; per column a
  unit (text, blank for none) and an optional header; rows of cell text.
- **Value:** a matrix of `rows x cols`, column `j` with the dimension of its unit, so
  `(1 m, 2 s)` columns work. Real numbers only in the baseline.
- **Cells must be numbers.** A blank or unreadable cell is an error naming it (row,
  column); the region shows the error and defines nothing. A matrix cannot have holes
  and NaN is never a value (`EvalError::NotFinite`).
- **Limit:** a matrix holds 4096 elements, so a table does. A larger CSV is refused
  with that message, not truncated.
- **Edits:** the whole table is replaced by one edit (as `SetPlot` does), merging while
  typing and sealing on focus loss, so a cell edit is one undo step. Import is one undo
  step. Boxed in `RegionKind`, to keep every region small (the plot lesson).
- **File format:** version 2 is unreleased, so the table is added to it. The reserved
  `"Table"` string of version 1 files is turned into an empty table by the **version
  1 to 2 migration step** (the first real content for that step).
- **Placement and editing:** typeset as a grid with a header row of names and units,
  edited in place cell by cell with Tab and Enter, plus a properties section in the
  Matrices/Tables tab for the name, column units and import/export. Detailed layout is
  the next design note.

## 4. Picture region

- **Stored** as the original PNG bytes, in base64, inside the region in
  `worksheet.json` (built this way instead of separate `assets/` entries: simpler, and
  undo, copy and paste carry the image with no extra bookkeeping; the cost is about a
  third more size, hence the 2 MB per image and 16 picture limits). The original file
  name is a label only.
- **Import:** a command (and drag and drop of an image file) copies the file in. PNG
  uses the `png` crate already in the build. **JPEG needs a new dependency** (section 7).
- **Limits** (hostile files): the file at most 10 MB, at most 16 million pixels
  decoded, at most 32 pictures per worksheet, container entries raised from 8 to 64;
  a picture that fails to decode is shown as a placeholder with the reason and kept
  in the file, not dropped.
- **Display and export:** drawn as a texture on the canvas, scaled to the region;
  painted into PNG and PDF export (the PDF writer needs an image path, a small
  addition).
- **Image to matrix:** a command on a selected picture that makes a Table of its
  grey levels (0 to 1), limited to 4096 pixels, so only small images; and the
  reverse (matrix to picture) later. Mathcad's image functions are not copied.

## 5. CSV and Excel

- **CSV import:** a command on a Table region (and File > Import) reads a `.csv` or
  `.txt`: comma, semicolon or tab separated (detected), quotes and line endings as
  RFC 4180 describes plus old Mac `\r` endings, an optional byte order mark. A first
  row that is not numbers becomes the headers (a unit may be written in brackets,
  `force [kN]`). The numbers' decimal mark is the point.
- **CSV export:** a command on a Table (or a Math region whose value is a matrix or
  list) writes the numbers in the units shown, with a header row of names and units.
  Only numbers and headers are written, so a worksheet cannot make a file that runs
  a formula when opened in a spreadsheet.
- **Excel:** reading `.xlsx` with `calamine` (MIT, read-only, no macros executed) into
  a Table, as a new dependency. Writing `.xlsx` needs a second crate
  (`rust_xlsxwriter`); recommended only after reading proves useful. Excel as a live
  component (PTC's OLE link) is not built.
- **No read and write functions** (section 2). Mathcad-style `READPRN`/`WRITEPRN` are
  refused by design and listed as a difference.

## 6. Stages

1. **CSV module** (built): `opencalc-core::csv`, a bounded, tested parser and writer
   with delimiter and header detection and number reading. No UI, no format change.
2. **Table region:** data model, evaluation, edits, migration step, painting, in-place
   editing, CSV import and export commands, tests (corpus, round trips, hostile CSV).
3. **Picture region:** container assets, PNG decode, painting, export, limits.
4. **JPEG and Excel import**, once the dependencies are decided.
5. **Image to matrix** and back.

## 7. Decisions needed

1. **No file access during evaluation** (section 2): import and export are commands,
   not functions. Agree?
2. **CSV parser:** write our own (about 300 lines, bounded and fuzz-tested, no new
   dependency), as stage 1 does, or use the `csv` crate as `SPEC` section 5.2 listed?
3. **Cells must be numbers** (no blanks or text in a table)? Or allow empty cells as
   errors only where used, which needs holes in matrices?
4. **JPEG:** accept a new decoder dependency (a pure-Rust one such as `zune-jpeg` or
   `jpeg-decoder`, both permissive), or support PNG only for now?
5. **Excel import** with `calamine`: now, later, or not at all?
6. **Table units:** one unit per **column** (this design), or per cell?

## 8. Not in this design

Live links to files or spreadsheets; text, date or complex cells; formulas inside
cells; pictures as plot backgrounds; SVG and other vector import; image editing.
