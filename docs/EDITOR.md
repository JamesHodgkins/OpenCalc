# Math editor design (Milestone 2.5)

Status: **approved** (hybrid editor, `lpha` Greek entry, underscore subscripts). Progress: **E1 done** (spans); E2 to E6 not started.

## 1. What exists today

- A Math region saves its **source text** (decision D5). The parsed `Expr` is
  derived from it, and every change is `Edit::SetMathSource` (merged while typing,
  one undo step per run).
- While you edit, the canvas shows a plain **monospace text box** (`edit_overlay`
  in `canvas/mod.rs`). When you stop, the region is typeset from the parsed `Expr`.
  So you type `a/b` as text and only see a fraction afterwards.
- Typesetting (`layout.rs`) turns an `Expr` into positioned draw operations. It keeps
  no link from a drawn piece back to the source text, so a click cannot be mapped
  to a character and a character cannot be mapped to a position on screen.
- Already built and relevant: parse errors carry a character range
  (`ParseError`), evaluation errors carry a path into the tree (`error_paths`,
  `Expr::at_path`), the printer round-trips (`Expr` to source and back), and
  `FUNCTIONS` lists every built-in with its signature.

## 2. Goals

1. Type maths and **see it typeset while typing**: fractions stack, exponents rise,
   roots get a radical, matrices form a grid.
2. A caret you can place with the mouse and move with the arrow keys through the
   typeset form, plus selection, copy, paste and delete.
3. Mistakes drawn **in place**: the offending characters underlined, the failing
   sub-expression highlighted (closes TD-10).
4. Entry aids: Greek letters, subscripts, autocomplete from the function table and
   the names defined in the worksheet.
5. Nothing is lost: the file format stays source text, every change stays an `Edit`
   (rules R1 to R9 unchanged), no input can panic or hang.

## 3. The central choice

**Option A, the hybrid (recommended): the source string stays the editing buffer;
the editor draws it typeset and keeps a caret as a character offset.**

- Every drawn piece remembers which source characters it came from (a *span*). A
  caret is just a character offset; the editor finds where to draw it from the spans,
  and finds the offset for a click the same way.
- Half-typed input (`x^`, `(1 + 2`) is drawn using a *lenient* parse that puts a small
  placeholder box where an operand is missing. The source text is never changed to
  make it parse.
- Typing `/`, `^`, `[`, `(` just inserts that character. Fractions and exponents
  appear because the parser and typesetter already turn those characters into
  structure. Precedence also leaves an exponent or denominator automatically: in
  `x^2+1` the `+` ends the exponent, as it does when you read it.

**Option B: a structural editor, where the caret is a path into the `Expr` tree
and typing edits the tree.** This is how some products work. Not recommended now:

| | A: hybrid | B: structural |
|---|---|---|
| Saved form | source text (unchanged) | source text, by printing after each edit |
| Half-typed input | placeholders in a display-only parse | needs an `Expr::Hole` that the evaluator must also handle |
| New language feature | only needs parse + typeset + spans | also needs insert, delete, wrap, unwrap and caret rules for the new node |
| Copy and paste | source text, trivially | needs tree fragments |
| Size and risk | moderate | large; touches every node type |
| Structural selection ("select this fraction") | yes, from spans | yes |
| Upgrade path | B can be built on top of the spans later | n/a |

The hybrid gets the visible result of a structural editor (2D entry, in-place
errors, structural selection) for far less machinery, and it cannot disagree with the
file format because the buffer *is* the file format.

## 4. How it works

### 4.1 Spans (core) - built

`parse_with_spans(src) -> (Expr, SpanNode)`. A `SpanNode` tree has the same shape as
the `Expr` (its `kids` follow `Expr::children()`), so a node is found with the same
path that `Expr::at_path` and the error paths use. Each node has an `outer` span
(including any parentheses written around it) and an `inner` span (without them), in
characters. The parser logs one entry per finished node, and the tree is rebuilt from
that log only when spans are asked for, so `parse` costs and behaves as before (parse
of 200 regions went from about 105 to 150 microseconds, from the extra log pushes). The
operator between two children, the brackets of a call and so on are not separate
nodes: they are the gaps between a node's `inner` and its kids' `outer` spans.

This also turns an evaluation error path into a source range, which is what
underlining needs.

### 4.2 Lenient parse (core)

`parse_lenient(src) -> (Expr, Spans)` never returns `Expr::Error` for the whole
region. A missing operand becomes `Expr::Hole`; an unmatched `(` or `[` is treated as
closed at the end; stray tokens are kept as `Expr::Garbage(text)` drawn as plain text.
`Hole` and `Garbage` exist **only in this display tree**. `parse` (used for
evaluation and saved data) is untouched, so the evaluator never sees them.

### 4.3 Typesetter anchors (UI)

Each laid-out piece records an *anchor*: its source span and its rectangle, and for
text, its galley so the position of a character inside it can be found.
Brackets, operators and the fraction bar are anchors too, so every character has a
home. Characters that draw nothing (the parentheses around a numerator, spaces)
collapse onto the nearest visible edge.

Two pure functions come out of this, and both are tested without a window:

- `caret_rect(layout, offset) -> Rect`
- `offset_at(layout, point) -> usize`

### 4.4 The editor widget (UI)

One widget replaces the text box. It owns the caret and selection (UI state, not
document state) and turns input into `Edit::SetMathSource`:

- Typing and paste insert at the caret; Backspace and Delete remove; Enter or leaving
  the region ends editing; Esc ends editing too, as now.
- Arrow keys move by character, skipping characters that draw nothing; Ctrl+arrows
  move by token; Home and End go to the ends; Shift extends the selection.
- Click places the caret; drag selects; double click selects the enclosing node (from
  the spans); Ctrl+A selects all.
- Copy and cut put the **source text** of the selection on the clipboard.
- Everything works in characters, not bytes, and respects the existing limits
  (`MAX_SOURCE_CHARS`, `MAX_SYMBOLS`): going over shows the error, it never crashes.

### 4.5 Entry aids

- **Subscripts:** a name with an underscore (`F_x`, `sigma_y`) is drawn with a
  subscript. It is still one name; indexing stays `x[i]`.
- **Greek:** type `\alpha` and then space or Tab to get the letter; the same table
  backs the ribbon's Greek button. Plain ASCII names stay available.
- **Autocomplete:** a small list under the caret, from `FUNCTIONS` (with its
  signature and summary), defined names, and units. Tab or Enter accepts, Esc closes.
- **Operators:** `*` is drawn as a centred dot, `<=` as ≤, `!=` as ≠. The source keeps
  the ASCII form (the lexer already accepts both).

### 4.6 Errors in place

- A parse error underlines its character range in the typeset form.
- An evaluation error highlights the sub-expression at its path, using the spans.
- The properties panel keeps its text description; the underline is the new part.

## 5. Steps

Each step ends with the gate green, tests added, and a commit, as before.

| Step | Content | Main tests |
|---|---|---|
| E1 | `parse_with_spans`, spans for every node | For every golden and corpus source: spans nest correctly, children lie inside parents, the root covers the source |
| E2 | `parse_lenient` with `Hole` and `Garbage` | Every prefix of every corpus source parses to *something*; a full source gives the same tree as `parse`; random text never panics |
| E3 | Anchors in the typesetter; `caret_rect` and `offset_at` | For every source and every offset: the caret rectangle is inside the layout, and `offset_at(caret_rect(o))` returns `o` or an offset that draws at the same place |
| E4 | Editor widget: input, selection, clipboard, in-place caret; replaces the text box | Frame tests with synthetic key and mouse events; undo steps unchanged |
| E5 | Subscripts, Greek, operator glyphs, autocomplete | Table tests; autocomplete never offers a name that would not parse |
| E6 | Errors drawn in place; double-click structural selection | Underline ranges equal the error ranges; paths map to spans |

E1 to E3 are pure and fully testable before any widget exists, so the riskiest part
(mapping between characters and screen positions) is proven first.

## 6. Risks and how they are handled

| Risk | Handling |
|---|---|
| Caret and click mapping is wrong through nested structure (fractions, roots, matrices) | E3 tests every offset of every corpus source, not a few examples |
| The lenient parse invents structure the user did not mean, so the display jumps | It only fills in what is missing and never reorders; E2 checks that prefixes of valid sources keep the structure of the prefix |
| Characters that draw nothing (parentheses around a numerator) make the caret seem stuck | They collapse onto the nearest edge and arrow keys skip them; a test walks the caret across every corpus source and checks it advances |
| Performance on each key press | One region is parsed and laid out per key press. Measured today: parse 0.5 µs per region, frame with 200 regions 0.8 ms. Re-measured at E4 |
| Input methods, key repeat, non-ASCII | Characters not bytes throughout; Greek and `²` style input tested; IME checked on the real window at E4 |
| It looks like a structural editor but is not | Stated here: editing is of the text. Structural operations (wrap in fraction, unwrap) can be added later on top of the spans |

## 7. Decisions needed

1. **Option A (hybrid), as recommended, or Option B (structural)?**
2. **Greek entry:** backslash names (`\alpha`) plus the ribbon button, or another
   scheme (for example a key chord)?
3. **Subscript syntax:** underscore in names (`F_x`), as proposed?

Everything else in this note follows from those, or can be changed at review time.
