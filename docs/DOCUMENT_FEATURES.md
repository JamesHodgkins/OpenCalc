# Document features: the rest of Phase 5 (design)

Status: **proposal; nothing here is built.** Written 2026-11-08. Covers the five
Phase 5 items left after region borders and headers and footers: page backgrounds,
templates, rich text, content protection and global definitions. Roadmap context:
`ROADMAP.md` Phase 5. Feature source: `private/mathcad_features.md` (lines 77, 84, 86, 88).

## 1. Summary and order

| # | Item | Size | Needs a decision | Recommended |
|---|---|---|---|---|
| 1 | Page background styles | S | which styles; print or not | Build now |
| 2 | Templates (`.ocalct`) | M | built-in templates or none | Build now |
| 3 | Global definitions `≡` | none | whether to re-scope | **Close as satisfied by design**; accept `≡` as another spelling of `:=` |
| 4 | Rich text | L | bundle an italic font; colours | Build after 1 and 2 |
| 5 | Content protection | M to L | what threat it covers | Build only the honest, no-crypto part first |

## 2. Page background styles

**What Mathcad has.** A page can have a background colour or pattern (the feature list
says "multiple page sizes and background styles").

**What OpenCalc has.** `PageSetup` (paper, orientation, margin) and the page painter in
`canvas/page.rs`, which fills each page with the paper colour; exports draw the same
way through `render_pages`. Headers and footers already show how a page-wide
setting is edited (Page setup window) and drawn in both places.

**Proposal.**
- `PageSetup.background: Background` with `White` (default), `Grid`, `Lines`, `Dots`.
  A pattern is drawn in a pale grey under the regions, spaced at the snap grid (an
  eighth of an inch) so regions line up with it.
- Colour: a short list of tints (white, cream, light grey, light blue) as
  `PageSetup.tint`. Dark pages are out: ink colours are fixed for paper.
- Saved as two optional fields of `page`; omitted when default. Version 2 is still
  unreleased, so no migration step.
- **Printing.** Patterns are for working on screen. Exports leave them out unless
  "Print the background" is ticked (default off), because a pale grid on every
  printed page is rarely wanted. A tint follows the same switch.
- Draft view shows the background too (it is one long sheet).

**Decision A:** is that set of styles right, and is "off when exporting" the right default?

## 3. Templates

**What Mathcad has.** Worksheet templates that standardise a process.

**What OpenCalc has.** `FileNew` makes an empty "Untitled n"; `FileExample` opens a built-in
sample. The container, migrations and validation already handle any worksheet, and
recovery files exist for untitled sheets.

**Proposal.**
- A template is **the same file as a worksheet** with the extension `.ocalct` and
  `"kind": "template"` in `manifest.json` (an unknown manifest field is ignored by older
  readers, per `FILE_FORMAT.md`). No second format to maintain.
- **File > New from template...** opens a picker (or the user's template folder) and
  creates a new untitled worksheet from a copy of the template's contents. It never
  opens the template itself for editing, so it cannot be overwritten by Save.
- **File > Save as template...** writes the active worksheet as `.ocalct`.
- Templates live wherever the user keeps them; the app also remembers a template
  folder (app data directory) shown first in the picker.
- Opening a `.ocalct` by double-click asks "New worksheet from this template, or edit
  the template?" (two buttons), so editing a template stays possible.
- File association for `.ocalct` follows the `.ocalc` one (Options > file type).

**Decision B:** ship a few built-in templates (blank, beam check skeleton, units
conversion sheet), or none? Built-ins are about a day each to write well and test; none
means templates are only what users make.

## 4. Global definitions (`≡`)

**What Mathcad means.** In Mathcad `:=` is evaluated top to bottom and left to right, so
a name is only visible *below and to the right* of its definition. `≡` makes a definition
visible *everywhere on the worksheet*, above and below.

**What OpenCalc does.** The evaluator orders work by the dependency graph and never by
layout (architecture decision; tested by shuffling region order in the corpus harness).
So **every definition is already available everywhere on the worksheet**, above or
below. A second definition of the same name gets a warning, and the last in layout order
wins. The roadmap item (and TD-23) assumed `≡` needed worksheet-qualified IDs, but those are
needed only for something different: using definitions from *another file*
(Mathcad's Reference component).

**Proposal.**
- Close "global definitions" as satisfied by design, and say so in `ROADMAP.md` and in the
  user guide ("every definition is global; order on the page does not matter").
- Accept typing `≡` as another spelling of `:=` (it parses to the same definition and is
  shown as `:=`), so worksheets written with it open sensibly. Small parser change.
- Cross-worksheet references are a **separate, larger feature** (needs TD-23, a way to name
  another file, and a decision on live versus snapshot, as in `DATA_IO.md` section 2). Not
  started; recorded under "Not in this design".

**Decision C:** agree to close it this way?

## 5. Rich text

**What Mathcad has.** Fonts, sizes, colours, bold, italic and underline on parts of a
text region, and on equations.

**What OpenCalc has.** A text region is one string with one `FontStyle` (bold, italic
for the whole region). Only `Inter-Regular` and `Inter-Bold` are bundled, so "italic" is a
synthetic slant (TD-24). Editing is in place with egui's `TextEdit`; the page painter
uses a `LayoutJob`.

**Proposal (in stages).**
1. Bundle `Inter-Italic` and `Inter-BoldItalic` (OFL licence; about 0.6 MB together) so italic
   is real.
2. Model: keep `text: String` and add sparse `runs: Vec<Run>` (a run is a length in
   characters plus bold, italic, underline and an optional colour from a fixed palette).
   Text without runs is exactly today's region, so old files and the whole-region
   `FontStyle` keep working. Saved fields are optional; no migration step.
3. Editing: a custom `TextEdit` layouter builds the `LayoutJob` from the runs; Bold and
   Italic buttons apply to the selection (or the whole region when nothing is selected, as
   now). Runs are adjusted as text is typed, deleted or pasted.
4. Out of this stage: font families and sizes, bullet lists, links, rich text inside math.

The risky part is step 3 (keeping runs aligned with edits); it gets property tests that
apply random edits and check the runs still cover the text exactly.

**Decision D:** bundle the two italic fonts; and is a fixed palette of about 8 text colours
enough?

## 6. Content protection

Mathcad offers password protection of regions and locking content to hide proprietary
information. These are different goals with different honest answers, so they are split.

**What OpenCalc has.** `locked` on a region: no move, edit, restyle or deletion until
unlocked (enforced in core). No password, no hiding.

**The hard fact.** A worksheet is a ZIP of JSON. Anything saved in it can be read by anyone
who unzips it. A password that only gates the *interface* stops accidents, not people.
Real secrecy needs encryption.

**Proposal.**
- **Stage A, edit protection with a password (no crypto):** a locked region may carry a
  password (kept as a salted hash). Unlocking asks for it. It is labelled in the interface
  and in `FILE_FORMAT.md` as *protection against accidental change, not security*.
  Needs a vetted password hash (a small, permissive crate such as `argon2`/`scrypt`) or,
  to avoid a dependency, a salted iterated SHA-256 written with the care that implies. Either
  is a new dependency decision.
- **Stage B, hidden content:** a region can be set to show only its result (the typed
  expression is collapsed), still evaluated, editable by unlocking. This is display only
  and the same caveat applies.
- **Stage C, real encryption (not proposed now):** encrypt the worksheet file or selected
  regions with an authenticated cipher and a password-derived key. Evaluating hidden
  regions means the key is needed to open the file at all, or results are stored in clear
  beside the encrypted source. It adds a cryptography dependency, key handling, and
  recovery-file and autosave questions (the recovery file would be plain text unless it is
  encrypted too). Worth its own design if anyone needs real secrecy.

**Decision E:** which do you want: A and B with the plain label (my recommendation), A and
B and later C, or only the existing lock? And is a new hashing dependency acceptable?

## 7. Audit: where each change lands

| Item | Core | UI | File format |
|---|---|---|---|
| Background | `PageSetup` fields, sanitising | page painter, Page setup window, export | `page.background`, `page.tint`, `page.print_background` |
| Templates | `manifest.kind` | New from template, Save as template, open prompt | `.ocalct`, `manifest.kind` |
| `≡` alias | parser | none | none |
| Rich text | `Run`, `Edit::SetText` extended, run maintenance | layouter, editor, fonts | optional `runs` on `Text` |
| Protection | password hash, lock check | lock dialog, collapsed painting | optional `lock` data on a region |

## 8. Not in this design

Cross-worksheet references; rich text inside math regions; font families and sizes;
background images; encrypting whole files; date fields in headers.
