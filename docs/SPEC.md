# OpenCalc: Technical Specification (v1 draft)

A Rust + egui rebuild of a Mathcad-style engineering worksheet. This is an original product. It is not a clone of PTC Mathcad: it uses its own name, its own file format, and its own interface, built from publicly documented behaviour only.

**Status:** Draft 1. Milestone 0 done; Milestone 1 shell and canvas working. Code structure, rules and the refactor plan are in `ARCHITECTURE.md`; known shortcuts are in `TECH_DEBT.md`.

**Sources:** PTC Mathcad Prime public help and keyboard-shortcut references (versions 3, 4, 11 and 12), PTC's units help, and public alternatives (Calcpad, SMath Studio, Maxima). Items marked **[verify]** need checking against current PTC documentation before they are relied on.

---

## 1. Executive Summary and Scope

### 1.1 Overview
A worksheet calculator. Engineers write equations in natural notation on a free-form 2D page. The software computes them, tracks units, and keeps the calculation readable as a document. The result is a calculation note a reviewer can follow from inputs to results.

**Target users:** engineers (mechanical, civil, pipeline and process) who write verifiable calculation notes; students and technical writers.

**Core value proposition:**
1. Natural notation that a reader can check.
2. Dimensional analysis: values carry units and unit errors are caught.
3. Results recalculate automatically when any input changes, in any layout order.
4. The document doubles as the calculation record.

### 1.2 In scope for v1
- Free-form worksheet canvas with **math**, **text** and **area** (collapsible group) regions.
- Assignment operators: definition `:=`, evaluation `=`, boolean equality `==`.
- Dependency-based recalculation, independent of visual order.
- Real and complex numbers; scalars, vectors and matrices; indexing and ranged variables.
- Built-in maths functions and user-defined functions.
- Units engine: SI, USCS and CGS systems, unit conversion on display, custom units.
- Display precision and number-format control.
- X-Y plots (line and scatter).
- Native save and load in the project's own format, with undo and redo.
- Export to PDF and PNG; copy regions to the clipboard as images.
- Multiple worksheets open as document tabs, inside the ribbon interface (section 4.8).

### 1.3 Out of scope for v1
- Symbolic algebra (simplify, expand, symbolic solve).
- Solve blocks (`Given`/`Find`, `Minerr`) and optimisation. Candidates for v2.
- Import of PTC `.mcd` or `.mcdx` files. The format is proprietary.
- 3D surface and contour plots.
- CAD integration.
- Real-time collaboration and cloud storage.
- Programming constructs (`for`, `while`, local variables). Added after the core evaluator works.
- Mobile and web targets.

---

## 2. Feature Matrix

Categories: **Core** (required for v1), **Secondary** (nice to have), **Advanced** (power-user, post-v1).

### 2.1 Document and layout
| Feature | Category | Notes |
|---|---|---|
| Free-form region placement on a grid | Core | Regions sit at any X/Y position, not in a linear cell list. |
| Snap grid; toggle grid display | Core | Grid is for alignment only. |
| Math region | Core | Holds equations. |
| Text region (plain and formatted) | Core | Bold, italic, font, size, colour. |
| Area region (collapsible group) | Core | Collapses and expands its contents. |
| Region selection (single, multiple, contiguous and non-contiguous) | Core | Needed for copy, move and delete. |
| Separate overlapping regions (vertically or horizontally) | Secondary | Mathcad Prime exposes this from its Document tab. |
| Align regions horizontally and vertically | Secondary | |
| Plot region | Core | X-Y line and scatter. |
| Table region | Secondary | Data input and output. |
| Picture region (image embed) | Secondary | |
| Region tags and internal hyperlinks | Secondary | Lets equations reference each other by name. |
| Locked areas and worksheet protection | Secondary | Stops accidental edits to a finished calculation. |
| Ruler and guidelines | Secondary | |
| Page view and page breaks | Secondary | Affects PDF export. |
| Auto-save | Secondary | |
| Templates / standard calculation worksheets | Secondary | Reusable templates for repeat checks. |

### 2.2 Math entry and evaluation
| Feature | Category | Notes |
|---|---|---|
| Natural-notation entry (fractions, superscripts, subscripts, roots, sums, integrals) | Core | Typed input becomes structured layout. |
| Definition `:=` | Core | Order-independent. |
| Evaluation `=` | Core | Shows a result. |
| Boolean equality `==` | Core | |
| Global definition `≡` | Advanced | Global scope across worksheets. **[verify]** |
| Dependency-based recalculation | Core | Order comes from the graph. |
| Redefinition warnings | Core | Flags a name defined more than once. |
| Error tracing | Core | Highlights the source of an error. |
| Real, complex and imaginary-unit handling | Core | |
| Vector and matrix operators | Core | Element-wise operations and matrix multiply. |
| Indexing and subscripts | Core | `x_i`, `M_{i,j}`. |
| Ranged variables (`i := 0..n`) | Core | Drives vectorised evaluation. |
| Built-in functions (trig, log, exp, sqrt, abs, mean, stdev) | Core | |
| User-defined functions (`f(x) := ...`) | Core | |
| Inline conditional | Core | Inline, not a programming block. |
| Piecewise (`if` block) | Secondary | |
| Root finding (`root`) | Secondary | Numeric. |
| Polynomial roots, interpolation, regression | Secondary | |
| Solve blocks (`Given`/`Find`) | Advanced | v2. |
| Symbolic simplify, expand, solve | Advanced | Out of scope for v1. |
| Custom and prefix/postfix operators | Advanced | |
| Numerical integration and differentiation | Secondary | |

### 2.3 Units
| Feature | Category | Notes |
|---|---|---|
| Built-in SI, USCS and CGS unit systems | Core | |
| Unit-carrying values with dimensional checking | Core | Mismatches are errors. |
| Display in a chosen unit | Core | Conversion on display only. |
| Unit-aware built-in functions | Core | Sine of a mass is an error; mean of a unit-bearing data set works. |
| Custom user-defined units | Core | |
| Default unit system per worksheet | Core | |
| Dimensionless units and scaling functions | Secondary | |
| Units inside user-defined functions | Core | |
| Units in empirical formulas | Secondary | |
| Unit-label vs variable disambiguation | Core | For example `m` as a variable versus metres. |
| Force vs mass units (kgf, lbf) | Secondary | Needs an explicit policy. |

### 2.4 Display and formatting
| Feature | Category | Notes |
|---|---|---|
| Decimal places and significant figures | Core | |
| Decimal, scientific and engineering formats | Core | |
| Greek letters and a Greek toolbar | Core | |
| Typeset output (fractions, roots, sub/superscripts) | Core | |
| Rendering regions as images for export | Secondary | |
| Light theme | Secondary | Dark only in v1 (section 4.6). |

### 2.5 Editing, navigation and workflow
| Feature | Category | Notes |
|---|---|---|
| Keyboard-first math entry | Core | |
| Insert math, text, area and plot regions by keyboard | Core | Ctrl+Shift+A inserts an area in Mathcad Prime. |
| Enter moves the caret out of a region | Core | |
| Spacebar expands a group by one term | Core | |
| Undo and redo (multi-step) | Core | |
| Copy, cut and paste regions | Core | |
| Find and replace (text and names) | Core | |
| Multiple worksheets as tabs | Core | Ctrl+W closes the active worksheet. |
| Copy regions to clipboard as images | Secondary | |
| Print and PDF export | Core | |
| Spell check with a technical dictionary | Secondary | |

### 2.6 I/O and data
| Feature | Category | Notes |
|---|---|---|
| Native save and open | Core | Project's own format. |
| CSV import into table or vector | Core | |
| CSV export of results | Secondary | |
| XLSX import (read-only) | Secondary | |
| Embed an image | Secondary | |
| Recent files | Secondary | |

### 2.7 Advanced and power-user
| Feature | Category | Notes |
|---|---|---|
| Symbolic engine | Advanced | v3 or separate tool. |
| Scripting and programming blocks | Advanced | `for`, `while`, local variables. |
| Plugin API | Advanced | |
| Data regression and design of experiments | Advanced | |
| Polar and 3D plots | Advanced | |

**Gap:** the search results covered Mathcad Prime versions 3, 4, 11 and 12, not the full current help. Before freezing v1 scope, walk the current Mathcad Prime help table of contents and add anything missing.

---

## 3. Data Model and State Architecture

### 3.1 Principle
Keep the **document model** (pure, serialisable data) separate from **UI state** (selection, caret, scroll, open dialogs). Edits go into the document through a command step, never directly from inside the UI pass. This is the main defence against borrow-checker friction in egui's immediate-mode redraw.

### 3.2 Core domain model

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RegionId(pub u64);

/// Document-space coordinates (not screen pixels).
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Rect { pub min: Point, pub max: Point }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Worksheet {
    pub id: u64,
    pub title: String,
    pub settings: WorksheetSettings,
    /// Regions in z-order (back to front). Layout order is NOT calculation order.
    pub regions: Vec<Region>,
    pub next_id: u64,
    pub protected: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WorksheetSettings {
    pub unit_system: UnitSystemId,
    pub display: DisplayPrefs,
    pub grid_size: f32,
    pub custom_units: Vec<UnitDef>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisplayPrefs {
    pub decimal_places: u8,
    pub number_format: NumberFormat,        // Decimal | Scientific | Engineering
    pub significant_figures: Option<u8>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub id: RegionId,
    pub rect: Rect,
    pub tag: Option<String>,                // for internal links
    pub locked: bool,
    pub kind: RegionKind,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum RegionKind {
    Math(MathRegion),
    Text(TextRegion),
    Area(AreaRegion),
    Plot(PlotRegion),
    Table(TableRegion),
    Picture(PictureRegion),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MathRegion {
    pub root: Expr,                         // structured AST, not a string
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TextRegion {
    pub runs: Vec<TextRun>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TextRun {
    pub text: String,
    pub style: TextStyle,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub size: f32,
    pub colour: [u8; 4],
    pub font_family: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AreaRegion {
    pub collapsed: bool,
    pub title: String,
    pub children: Vec<RegionId>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlotRegion {
    pub kind: PlotKind,                     // XY line | scatter
    pub series: Vec<SeriesRef>,             // references to worksheet names
    pub x_range: Option<(f64, f64)>,
    pub y_range: Option<(f64, f64)>,
}
```

### 3.3 Expression AST
The AST is the source of truth. Layout and evaluation are both derived from it. Never store maths as a string as the primary form; editing and layout become very hard.

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    Number(Number),
    Name(String),
    Quantity { value: Box<Expr>, unit: UnitExpr },
    Unary { op: UnaryOp, arg: Box<Expr> },
    Binary { op: BinaryOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Call { name: String, args: Vec<Expr> },
    Index { base: Box<Expr>, indices: Vec<Expr> },
    Vector(Vec<Expr>),
    Matrix { rows: usize, cols: usize, cells: Vec<Expr> },
    Range { start: Box<Expr>, end: Box<Expr>, step: Option<Box<Expr>> },
    Conditional { cond: Box<Expr>, then: Box<Expr>, otherwise: Box<Expr> },
    Assign { kind: AssignKind, target: Box<Expr>, value: Box<Expr> },
    // Layout-bearing nodes: they affect typesetting and editing, not the value.
    Fraction { num: Box<Expr>, den: Box<Expr> },
    Root { radicand: Box<Expr>, index: Option<Box<Expr>> },
    Sum { var: String, range: Box<Expr>, body: Box<Expr> },
    Integral { var: String, lower: Box<Expr>, upper: Box<Expr>, body: Box<Expr> },
    Error(String),                          // parse errors kept so the UI can show them
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssignKind { Define, Evaluate, Equal, Global }   // :=  =  ==  ≡

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Number { Real(f64), Complex(f64, f64) }
```

### 3.4 Units and quantities
Represent dimensions as integer exponents over the seven SI base dimensions, not as strings. Store values in SI internally and keep a display unit for output.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Dimension {
    pub length: i8,      // L
    pub mass: i8,        // M
    pub time: i8,        // T
    pub current: i8,     // I
    pub temperature: i8, // Θ
    pub amount: i8,      // N
    pub luminosity: i8,  // J
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UnitDef {
    pub symbol: String,          // "psi", "kN"
    pub dimension: Dimension,
    pub to_si_scale: f64,        // multiply by this to get SI
    pub to_si_offset: f64,       // for temperature scales (degC, degF)
}
```

**Design rule:** all arithmetic goes through the quantity type. Multiplication adds dimension exponents. Addition requires matching dimensions. Transcendental functions require a dimensionless argument. Keep this in one module with exhaustive tests. It is the part of the app most likely to produce silently wrong answers.

**Complex values also need units.** Use a value enum (`Scalar(f64)` or `Complex(Complex64)`) plus a `Dimension`, not a bare `f64`.

### 3.5 Evaluation values

```rust
#[derive(Clone, Debug)]
pub enum Value {
    Scalar(Quantity),
    Complex(num_complex::Complex64, Dimension),
    Vector(Vec<Value>),
    Matrix { rows: usize, cols: usize, data: Vec<Value> },
    Text(String),
    Boolean(bool),
    Function(FunctionId),
    Error(EvalError),
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum EvalError {
    #[error("undefined name '{0}'")]
    Undefined(String),
    #[error("unit mismatch: {0}")]
    UnitMismatch(String),
    #[error("circular reference")]
    Cycle,
    #[error("dimension error in function {0}")]
    DimensionError(String),
    #[error("index out of range")]
    Index,
}
```

Vectors and matrices should use a flat buffer with a shape, not nested `Vec`. Consider `ndarray` if broadcasting is needed. Keep `Value` small: it is cloned during evaluation.

### 3.6 Dependency graph and evaluation
Evaluation order comes from the dependency graph, not from layout. A region can use a name defined below it.

```rust
pub struct DependencyGraph {
    pub definitions: HashMap<String, RegionId>,      // name -> definer
    pub deps: HashMap<RegionId, Vec<RegionId>>,      // consumer -> producers
    pub dependents: HashMap<RegionId, Vec<RegionId>>,// producer -> consumers
}

pub struct Evaluator {
    pub graph: DependencyGraph,
    pub cache: HashMap<RegionId, Value>,
    pub dirty: HashSet<RegionId>,
}
```

**Algorithm:**
1. Parse every math region to an AST. A parse failure becomes `Expr::Error`, never a panic.
2. Build the graph. Each free name is an edge to its definer. If a name has several definers, emit a redefinition warning and use the last definition in layout order.
3. Detect cycles with Tarjan's strongly connected components. Every region in a cycle gets `EvalError::Cycle`.
4. On edit, mark the edited region and its transitive dependents dirty. Do not recompute the whole worksheet.
5. Evaluate dirty regions in topological order.

---

## 4. UI and egui Component Architecture

### 4.0 Visual reference and approach

**Target look:** the OpenDraft desktop app (`C:\Users\james\Documents\Creative\Rust\OpenDraft`, shared UI crate `crates/od-ui`). OpenCalc should match its ribbon, general style and theming.

**Source of truth:** the code, not `ui_plan.md`. Where they differ, the code wins. OpenDraft's accent is teal (`#00866B`). `ui_plan.md` still describes an old blue accent (`#1473E6`), which is out of date.

**OpenCalc's accent is red**, so the two apps are easy to tell apart. Everything else in the look is shared.

**Reuse approach:** pending a code-sharing decision (section 4.11). Until then, OpenCalc re-implements the same design tokens and component behaviour in its own `opencalc-ui` crate. No code is copied.

### 4.1 Window structure (top to bottom)

1. **Title row** (`base` tone): an app button at the far left (a square logo button that opens the application menu), then the Quick Access Toolbar (New, Open, Save, Save As, Undo, Redo, with dropdowns where relevant, and a customise chevron), then the worksheet title centred.
2. **Tab strip** (`base` tone): plain text tabs (Home, Insert, Format, Units, Calculate, View, Help). The active tab takes the ribbon body's tone and merges into it, with a 4 px top corner radius. Tab height 25 px.
3. **Ribbon body** (`raised` tone, 6 × 4 px inner margin): panels laid out left to right, each separated by a 1 px `divider` line. Every panel is as tall as a large button (66 px, three small rows of 22 px), whatever it holds.
4. **Document tab row** (`base` tone): a Home tab with a house icon (never closed), then one tab per open worksheet with a close button, then a `+` for a new worksheet. Tab height 26 px. The active tab takes the canvas tone.
5. **Canvas**: the worksheet. A viewport overlay sits at its top-left (zoom percentage and view mode), drawn as clickable controls.
6. **Dock panels** (`surface` tone): the Names and Units browser on the left, Region Properties on the right. Layout persists between sessions. Default split: left panel 13% of width; right column 30%. A "Reset layout" command restores it.
7. **Status bar** (`base` tone, 1 px `divider` above): icon toggles in fixed order: grid display, grid snap, live recalculation, unit system, warnings count, zoom.

### 4.1a Floating and overlay rules
- Popups and menus use the `panel_raised` tone, a 2 px margin and 22 px rows. Lists over 12 rows scroll.
- Popups close on a click outside them or on Escape.
- The ribbon's expanded panel flyout is a foreground area under its panel. It closes on a click outside, on Escape, or after an item runs.

### 4.2 The worksheet canvas
The canvas is the hardest part. Use one custom widget, not a per-region layout of egui widgets. Regions overlap, can sit at any coordinate, and must stay in sync with zoom and pan. Ordinary egui layout cannot express that cleanly.

- Allocate one rect with `ui.allocate_painter(size, Sense::click_and_drag())`.
- Keep a view transform: `screen = origin + doc * zoom + offset`. Everything stays in document space until painting.
- Paint regions back to front in z-order.
- Hit-test in reverse z-order against each region's rect.
- Drive selection, dragging and resizing from one pointer state.

```rust
pub struct CanvasView {
    pub zoom: f32,              // 0.25 .. 4.0
    pub offset: egui::Vec2,
    pub grid: f32,
}

impl CanvasView {
    pub fn doc_to_screen(&self, p: Point, origin: egui::Pos2) -> egui::Pos2 {
        origin + egui::vec2(p.x, p.y) * self.zoom + self.offset
    }
    pub fn screen_to_doc(&self, p: egui::Pos2, origin: egui::Pos2) -> Point {
        let v = (p - origin - self.offset) / self.zoom;
        Point { x: v.x, y: v.y }
    }
}
```

### 4.3 Maths typesetting and editing
Maths is not text, so a `TextEdit` cannot edit a fraction. The editor has three parts.

**(a) Layout engine.** Convert the `Expr` tree into a box tree. Each box has a size, a baseline, and glyphs and rules. Fractions stack; superscripts shift up; roots draw a radical; integrals scale their glyph. Measure with egui's font layout. Cache the box tree per region and rebuild only when that region changes.

**(b) Caret model.** The caret is a path into the AST, for example `[child 1 of Binary, child 0 of Fraction, numerator]`. Typing inserts a node at the path. Arrow keys move between siblings and into or out of structure.

**(c) Input mapping.** A small state machine maps key sequences to AST edits. `/` wraps the previous term in a fraction. `^` opens a superscript slot. Spacebar expands a group by one term.

```rust
pub struct MathEditor {
    pub region: RegionId,
    pub caret: CaretPath,
    pub selection: Option<(CaretPath, CaretPath)>,
    pub pending_key: Option<KeySequence>,
}

pub enum EditAction {
    Insert(Expr),
    Replace(CaretPath, Expr),
    Delete,
    Wrap(fn(Expr) -> Expr),
    MoveCaret(CaretMove),
}
```

### 4.4 Immediate-mode rules
1. **Do not mutate the document while iterating over it.** The UI pass produces `Vec<Command>`; a separate step applies them after the pass.
2. **Keep ephemeral UI state out of the document.** Selection, hover, drag offsets and open dialogs live in `AppState` or egui memory. Only edits go into `Worksheet`.
3. **Do not allocate per frame.** Cache layout boxes and parsed ASTs; recompute only on a revision change.
4. **Use stable IDs.** Pass explicit `egui::Id::new(("region", id.0))` so widget state survives layout changes.

```rust
pub enum Command {
    SelectRegion(RegionId, SelectMode),
    MoveRegion(RegionId, egui::Vec2),
    InsertRegion(RegionKind, Point),
    DeleteSelection,
    Edit(RegionId, EditAction),
    Undo,
    Redo,
}

pub struct AppState {
    pub worksheets: Vec<Worksheet>,
    pub active: usize,
    pub selection: Selection,
    pub history: History,
    pub view: CanvasView,
    pub editor: Option<MathEditor>,
}
```

### 4.5 Plotting
Use `egui_plot` for X-Y plots instead of building axes. Feed it series from the evaluator's cache. Do not compute plot data in the render pass: store sampled points on the `PlotRegion` and recompute in the background when referenced values change.

### 4.6 Styling, theming and typography

Dark only for v1. The app always uses the dark theme and does not follow the OS setting.

**Four tones of elevation.** Every background is one of these four. Never add a new background colour; pick a tone.

| Tone | Used for | Neutral (default) | Blue slate |
|---|---|---|---|
| `base` | canvas, title and status bars, tab rows, input wells (sunken) | `#252525` | `#222933` |
| `surface` | side panels, inactive tabs, popups | `#323232` | `#313946` |
| `raised` | ribbon body, active tabs, cards, buttons | `#3E3E3E` | `#3B4453` |
| `raised_high` | hover state of raised items | `#4C4C4C` | `#4F5A6E` |

Outline colours: `border` `#474747` (neutral) or `#4A5466` (slate); `border_strong` `#6A6A6A` or `#6B778C`; `pressed` `#585858` or `#5D6980`; ribbon `divider` uses `base`.

**Accent and text (shared by both palettes):**

| Token | Value | Use |
|---|---|---|
| `accent` | `#C62828` | primary actions, selection, focus rings |
| `accent_hover` | `#D32F2F` | hover on accent |
| `accent_pressed` | `#B71C1C` | pressed accent |
| `accent_text` | `#FF8A80` | accent-coloured small text, focus ring |
| `warning` | `#F5B942` | warnings (amber, never used for buttons) |
| `error` | `#FF7B7B` | error text and error icons only, never fills or buttons |
| `text` | `#DCDCDC` | primary text |
| `text_weak` | `#B4B4B4` | labels, inactive tabs, panel titles |
| `text_disabled` | `#646464` | disabled items |
| `text_on_accent` | `#FFFFFF` | text on accent fills |

**Red is used for both the accent and errors.** The rule that keeps them apart: `accent` is only for fills and selection; `error` is only for error text and icons. Errors are also marked with an icon, so they never rely on colour alone. The contrast test in section 4.9 checks `accent_text` and `error` against every tone.

Palettes are persisted by key (`neutral`, `slate`). Preferences offers both.

**Metrics.**
- Spacing unit 8 px. Everything is a multiple of it.
- Corner radius 4 px for inputs, buttons and popups. Pill radius 13 px for outlined pill buttons.
- Control height 26 px.
- Window corner radius is the control radius plus 2.
- Shadow only on windows and popups: offset (0, 6), blur 18, black at alpha 110. Everything else is flat.
- Scrollbars: always visible, 6 px wide, no track, handle opacity 0.45 dormant, 0.7 active, 0.9 hovered.

**Typography.**
- Inter Regular, bundled with `include_bytes!`, so rendering is identical on Windows and Linux. egui's defaults remain as fallback for missing glyphs.
- Body 12.5 px. Small (field labels, captions) 11 px. Tab 13 px. Heading 15 px. Monospace is used for the math input line.
- Small ALL-CAPS section headers in `text_weak`.
- Ribbon labels are 11.5 px; ribbon panel titles are 10.5 px in `text_weak`.

**Icons.** The Lucide icon font, bundled, referenced by codepoint from one `icons` module. Monochrome outline icons tinted by the current text colour. Disabled icons use `text_disabled`. Lucide is ISC-licensed; keep its licence file with the font.

**Application.** One function applies the theme at start-up (fonts, style and visuals together). No widget hard-codes a colour; it reads from the theme. A contrast test checks the accent text and focus ring against each tone.

### 4.7 Commands and keyboard
- A central **command registry** maps stable IDs to a label, default shortcut and handler. This drives menus, shortcut display and remapping.
- Check for text focus before handling shortcuts so typing in a text region does not trigger commands.
- Default bindings follow Mathcad conventions where they make sense (Ctrl+Shift+A area, Ctrl+W close worksheet, Enter exits region). Document every difference.

### 4.12 Drag and drop
- Regions: drag the body to move; drag edges and corners to resize. Snap to grid while dragging, with a modifier to disable.
- Files: `ctx.input(|i| i.raw.dropped_files)` accepts CSV and images dropped on the canvas, creating a table or picture region at the drop point.

---

### 4.8 Ribbon

The ribbon is a custom widget, because egui has no ribbon. It follows the OpenDraft design: tab strip, panels, and item kinds laid out in columns.

**Data-driven.** The ribbon is declared as data. Each item names a command ID. Label, icon, enabled state and shortcut text come from the command registry, so nothing is defined twice.

```rust
pub struct Item { pub command: &'static str, pub label: &'static str, pub icon: char }

pub struct Split { pub label: Option<&'static str>, pub items: Vec<Item> }

pub enum Cell {
    Button(Item),              // small icon + label
    Icon(Item),                // small icon; label is the tooltip
    Split(Split),              // small icon + label + arrow
    IconSplit(Split),          // small icon + arrow
    Slot { id: &'static str, width: f32 }, // room for a control the app draws
}

pub enum Column {
    Large(Item),               // big icon over a two-line label
    LargeSplit(Split),         // icon runs the last pick; label opens the list
    Stack(Vec<Item>),          // up to three small icon + label rows
    Rows(Vec<Vec<Cell>>),      // up to three rows of small cells
}

pub struct Panel { pub title: &'static str, pub columns: Vec<Column>, pub extra: Vec<Item> }
pub struct Tab { pub title: &'static str, pub panels: Vec<Panel> }
pub struct Ribbon { pub tabs: Vec<Tab> }

/// Per-frame UI state kept beside the ribbon.
pub struct RibbonState {
    pub active_tab: usize,
    pub open_panel: Option<(usize, usize)>,
    pub split_picks: HashMap<&'static str, &'static str>,
}

/// What the ribbon needs from the command registry. The ribbon never calls commands itself.
pub trait CommandSource {
    fn is_enabled(&self, command: &str) -> bool;   // unknown commands: false
    fn shortcut_text(&self, command: &str) -> Option<&str>;
    fn is_active(&self, command: &str) -> bool { false }
}

pub struct RibbonOutput { pub clicked: Option<&'static str>, pub slots: Vec<(&'static str, Rect)> }
```

`Ribbon::show(ui, theme, state, source) -> RibbonOutput` draws the active tab and returns the clicked command ID and the rectangle of each slot. The app draws its own controls in the slots (for example, the Units dropdown).

**Layout rules (from OpenDraft).**
- Small row height 22 px. Large button: 66 px tall (three small rows), at least 48 px wide, wider only for a longer label.
- Large icon 32 px, small icon 14 px, large gap 3 px, two-line label pitch 13.5 px.
- Small button padding: 4 px before the icon, 5 px between icon and label, 6 px after the label.
- In a row of cells, the last labelled cell takes any spare width, so stacked buttons line up.
- Icon-only cells are 22 px wide.
- Disabled items draw dimmed and ignore clicks.
- Tooltips read "Label (Shortcut)".

**Expandable panels.** A panel with `extra` items shows an arrow beside its title. The arrow opens a flyout under the panel: an icon-only grid, 26 px buttons, with the title along its bottom edge. The flyout closes on a click outside, on Escape, or after an item runs.

**Split buttons.** The face runs the item last picked from the list, which starts as the first item. The arrow opens the list. The pick is remembered per split for the session. `Split::labelled` gives a fixed label instead, as AutoCAD's Paste does.

**Not in v1** (OpenDraft's own known limits, carried over): responsive collapse, galleries, pinning an expanded panel open.

**OpenCalc ribbon (confirmed):**

| Tab | Panels |
|---|---|
| Home | Clipboard (Paste split, Cut, Copy), Edit (Undo, Redo, Find, Replace), Region (Delete, Lock, Tag) |
| Insert | Math (Math region), Text (Text region), Layout (Area, Plot, Table, Picture), Symbols (Greek toolbar in a flyout) |
| Format | Font (Bold, Italic, size split), Number (decimal places, scientific, engineering), Arrange (align, separate regions) |
| Units | Unit System (SI, USCS, CGS split), Convert (display unit), Custom (define unit, manage units) |
| Calculate | Recalculate (all, changed), Check (run warnings), Warnings (list, next, previous) |
| View | Zoom (in, out, fit, 100%), Show (grid, snap, guides), Panels (Names, Properties, Reset layout) |
| Help | Shortcuts, User guide, About |

### 4.9 Controls and interaction

- **Buttons:** flat `raised` fill, 4 px radius, no border at rest. Hover: `raised_high` fill and a 1 px `border` outline. Pressed: `pressed` fill and `border_strong` outline. Active (toggled on): `pressed` fill with no outline.
- **Inputs:** `base` fill (sunken), 1 px border, 4 px radius. Dropdowns show a chevron on the right.
- **Focus:** a 2 px `accent_text` ring on every custom-painted control (through one `focus_ring` helper). Egui's own focus treatment is kept for native widgets.
- **Keyboard access:** Tab moves focus; Enter or Space activates. A roving focus controller (arrow keys, Home and End) for ribbon and document tabs comes after v1, as in OpenDraft's UX audit.
- **Text and icon tabs:** plain text with `text_weak` when inactive and `text` when active or hovered.

### 4.10 Engineering conventions (adopted from OpenDraft's CLAUDE.md)

- Crates depend downward: `opencalc-core` (no UI) <- `opencalc-ui` and `opencalc-cli`. Geometry, maths and document logic must be testable with no window.
- No `unwrap`, `todo!`, `dbg!` or prints in production code (the CLI binary's output layer is exempt: printing is its job). `unsafe` is forbidden. Tests may unwrap.
- Keep files under about 400 production lines. Split by responsibility (for example, `ribbon/buttons.rs`, `ribbon/rows.rs`).
- Entities and worksheet regions are referred to by stable ID, never by list position.
- Commit checks, run before every merge: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. A failing check blocks the commit. `scripts/check.ps1` runs all three plus the file-size rule.
- Every user action is a named command. Menus, ribbon items, shortcuts and tools all call the registry.
- Update `docs/SPEC.md` and `docs/CLEAN_ROOM.md` in the same change as a feature.

### 4.11 Licence and code sharing with OpenDraft

**Licence (decided):** OpenCalc is GPL-3.0-or-later, the same as OpenDraft. The `LICENSE` file is the GPL v3 text.

**Code sharing (decided 2026-11-06): option 1, re-implement.** `opencalc-ui` is written from this section; OpenDraft is read for reference only and no `od-ui` code is copied. The options considered were:

1. **Re-implement.** Write `opencalc-ui` from this section, reading OpenDraft for reference only. Slower, but the result is OpenCalc's own code, and the design rules above are specific enough to build from.
2. **Copy the `od-ui` modules.** Faster. Each copied file keeps its header and a note of where it came from, and the change is recorded in `docs/CLEAN_ROOM.md`.
3. **Extract a shared crate.** Move `od-ui` into a separate crate both apps depend on. Cleanest long term, but it changes OpenDraft's repository layout, so it is a bigger decision.

Revisit option 3 (a shared crate) only if the two UI crates start to drift in ways that cost time.

---

## 5. External Integrations, File I/O and Concurrency

### 5.1 File format
The project designs its own format. It does not reverse-engineer any other product's file format.

A ZIP container:
```
opencalc-document.oc/
  manifest.json      // format_version, app version, timestamps
  worksheet.json     // serde-serialised Worksheet
  assets/            // embedded images, CSV snapshots
```

- **Versioning:** a `format_version` from day one, with a migration function per version.
- **Atomic writes:** write to a temporary file, then rename. A crash mid-save must never corrupt a document.
- **Auto-save:** a separate recovery file on a timer; the main file changes only on explicit save.
- **Compression:** the `zip` crate (deflate). Pretty-print JSON only in debug builds so diffs stay readable.

### 5.2 Import and export

| Direction | Format | Approach | Priority |
|---|---|---|---|
| Import | CSV | `csv` | Core |
| Export | CSV | `csv` | Secondary |
| Export | PDF | `printpdf` or layout-to-PDF | Core |
| Export | PNG | Render canvas offscreen | Core |
| Export | SVG | Render regions to SVG | Secondary |
| Import | XLSX (read-only) | `calamine` | Secondary |
| Clipboard | Image per region | `arboard` | Secondary |

PDF export reproduces the page layout through the same layout engine, targeting a page size, breaking regions across pages where needed. It is not a screenshot.

### 5.3 CLI

```
opencalc [FILE]                     # open GUI
opencalc calc FILE [--format pdf]   # headless: evaluate and export
opencalc check FILE                 # headless: report errors/warnings; exit code
```

`check` allows calculation notes to be verified in review or CI, which suits design sign-off.

### 5.4 Concurrency
Evaluation must never block the render loop. egui redraws at display rate, so a slow calculation would freeze the UI.

**Architecture:** one background worker thread owns the evaluator. The UI thread sends snapshots of changed work and receives results over channels.

```rust
pub enum EvalRequest {
    Recalculate { worksheet_id: u64, revision: u64, changed: Vec<RegionId>, doc: Worksheet },
    Shutdown,
}

pub struct EvalResult {
    pub worksheet_id: u64,
    pub revision: u64,
    pub values: HashMap<RegionId, Value>,
    pub warnings: Vec<Diagnostic>,
}
```

Key points:
- **Revisions:** every edit increments a counter. A result is applied only if its revision is current, which discards stale results from fast typing.
- **Repaint:** the worker holds a clone of `egui::Context` and calls `request_repaint()` when a result is ready. No polling.
- **Why `std::sync::mpsc`, not tokio:** the evaluator is CPU-bound with no async I/O. A dedicated thread is simpler and keeps an async runtime out of a desktop app. Add tokio only for network features (for example cloud sync), on a separate thread.
- **Cancellation:** long computations (integrals, root finding) check a shared `AtomicBool` periodically.
- **No shared mutable state:** the worker owns its evaluator and receives a cloned worksheet snapshot.

### 5.5 Undo and redo
A command history, not full document snapshots.
- Each mutating command records its inverse; undo replays inverses.
- Coalesce consecutive keystrokes in one region into one undo step.
- Cap history size; store deltas, not copies.
- Any new edit clears the redo stack.

### 5.6 Performance targets
- Edit to updated result: under 16 ms for worksheets under 200 regions.
- Parse one math region: under 1 ms.
- Full recalculation of 500 regions: under 200 ms on the worker; UI never blocked.
- Startup to first frame: under 500 ms with an empty document.
- Pan and zoom at 60 fps with 200 visible regions: cache layouts and cull off-screen regions before painting.

---

## 6. Phased Roadmap

Each milestone ends with something runnable and testable.

### Milestone 0: Spike (1–2 weeks)
Prototype the two riskiest parts in isolation before committing to M1:
- A layout engine that renders a fraction, superscript and root from an `Expr`.
- A worker thread that evaluates a small dependency graph and returns results to a repainting eframe window.

If either takes much longer than expected, revise the design before building on it.

**Exit criterion:** both prototypes run in one window, and an edit to an input updates a dependent result via the worker.

### Milestone 1: Skeleton and UI Layout
- [x] Cargo workspace: `opencalc-core` (model, parser, evaluator, units), `opencalc-ui` (egui app), `opencalc-cli` (headless).
- [x] eframe app shell matching section 4.1: title row with app button and Quick Access Toolbar, tab strip, ribbon body, document tabs, canvas, dock panels, status bar.
- [x] `opencalc-ui` theme module (section 4.6): four tones, Neutral and Blue slate palettes, Inter and Lucide bundled.
- [x] Ribbon widget (section 4.8) drawn from one data definition; all seven tabs populated.
- [x] `CanvasView` with pan, zoom, grid, and coordinate transforms.
- [x] Region model and z-ordered painting; placeholder for each region type.
- [x] Hit-testing, single and multiple selection (click, shift/ctrl-click, marquee), drag to move with grid snap.
- [x] Command registry with stable IDs and a keybinding table (shortcut text and tooltips; remapping is Milestone 3).
- [x] Text region: basic editing; rich formatting later.
- [x] Math region placeholder showing raw text (plus typeset form and live result).
- [~] Contrast test done and Greek flyout present; maths fallback font and the Greek-insert action are not.
- [x] Unit tests for coordinate transforms and hit-testing.

**Exit criterion:** create, move and select each region type on a canvas that pans and zooms smoothly. *(Met 2026-11-06 for all six region types; resize handles and the maths-fallback font remain open items.)*

### Milestone 2: Core Logic
- [ ] Pratt parser producing the `Expr` AST; errors become `Expr::Error`.
- [ ] Math editor with the caret model and structured entry.
- [ ] `:=`, `=`, `==` with name resolution.
- [ ] Dependency graph; Tarjan SCC cycle detection; topological evaluation; dirty propagation.
- [ ] Redefinition warnings; error tracing.
- [ ] Units engine: `Dimension`, quantities, SI/USCS/CGS, custom units, display conversion, dimension checks on every operator and function.
- [ ] Real and complex values; scalar and vector arithmetic; ranged variables; indexing.
- [ ] Built-in and user-defined functions.
- [ ] Worker thread with revisioned results and repaint requests.
- [ ] X-Y plot region via `egui_plot`.
- [ ] Property-based tests for unit algebra and conversion round-trips; golden tests for the parser.

**Exit criterion:** a 50-equation worksheet with units recalculates correctly in any layout order, and a unit mismatch is reported as an error.

### Milestone 3: Persistence and Polish
- [ ] Native save and open with `format_version`, atomic writes and migration scaffolding.
- [ ] Auto-save with a recovery prompt on startup.
- [ ] Undo and redo with coalescing.
- [ ] Find and replace across regions and names.
- [ ] CSV import; drag and drop of files.
- [ ] PDF and PNG export.
- [ ] Copy regions to clipboard as images.
- [ ] Separate overlapping regions; align; area region.
- [ ] Preferences: precision, number format, default unit system, shortcut remapping.
- [ ] Headless `opencalc check`.
- [ ] Packaging for Windows, macOS and Linux.
- [ ] User guide and file-format spec.

**Exit criterion:** an engineer can write, save, reopen, export and check a calculation note with no data loss on all three platforms.

### Beyond v1
Solve blocks and optimisation; custom operators and programming blocks; plugin API; SVG export; a symbolic engine if justified.

---

## 7. Suggested Crates

| Need | Crate |
|---|---|
| GUI | `eframe`, `egui`, `egui_plot` |
| Serialisation | `serde`, `serde_json` |
| Archive | `zip` |
| Errors | `thiserror`, `anyhow` |
| Complex numbers | `num-complex` |
| Matrices | `nalgebra` or `ndarray` |
| Parsing | Hand-written Pratt parser |
| CSV / XLSX | `csv`, `calamine` |
| PDF | `printpdf` |
| Clipboard | `arboard` |
| CLI | `clap` |
| Testing | `proptest`, `insta` |

---

## 8. Prior Art and Reference Points
- **Calcpad** (MIT licence): a free, worksheet-style engineering calculator. Read its parser and unit handling for design ideas.
- **SMath Studio:** proprietary freeware (free for personal use). Use it only as a UX and behaviour reference, and as a black-box test oracle by comparing its calculated results with OpenCalc's. Its code, binaries, and file format must not be copied, decompiled, or reverse-engineered. It is not a base to fork. The project was started because of SMath's licence terms.
- **Maxima, SymPy:** symbolic engines, relevant if symbolic support is ever added.
- **Mathcad documentation:** used for behaviour only. Do not copy its text, UI assets, file format or trademarks.

---

## 9. Legal and Naming
- The name "Mathcad" is a PTC trademark. The product is named **OpenCalc**.
- Design the file format, UI and documentation from scratch. Use Mathcad documentation only to understand behaviour, and never copy text or screenshots.
- Keep a short clean-room note in the repository recording which sources informed each feature.
- **Licence:** GPL-3.0-or-later (see `LICENSE`). Every crate declares it through the workspace.

### 9.1 Clean-room rules (binding on all contributors)
1. **Reference only.** Commercial and freeware products (Mathcad, SMath Studio) may be used to learn expected behaviour, workflows and UX conventions, and to check results.
2. **No copying.** Do not copy code, documentation text, screenshots, icons, sample worksheets, file formats or internal data structures from any product.
3. **No reverse engineering.** Do not decompile, disassemble, inspect binaries, or analyse proprietary file internals.
4. **Test oracle is black-box only.** Results may be compared by entering the same inputs and recording outputs. Tests use original or textbook examples, never proprietary or company calculations.
5. **Compatible code only.** Code may be taken from GPL-3.0-or-later sources (such as OpenDraft) or MIT-licensed sources (such as Calcpad), provided it keeps its licence notice and attribution and the change is recorded in `CLEAN_ROOM.md`. Code from any product with an incompatible or proprietary licence is never copied.
6. **Record provenance.** Each feature's design note lists the sources consulted, so the origin of every behaviour can be shown.
