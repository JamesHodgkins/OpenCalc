# Ribbon map

Status: **built (redesigned 2026-11-11)**. This replaces the first ribbon map (Mathcad-style
tabs: Math, Functions, Matrices/Tables, Plots, Math Formatting, Calculation, Document). That layout
left tabs with one button, a Math tab with nine unrelated groups, and a Document tab that mixed page,
view and drawing commands. The owner asked for it to be redone from scratch.

The tab names and groups below are **our own, organised by task**; they are not a copy of any
other product's ribbon. The ribbon is data in `ribbon/defs.rs`.

## Rules

1. **One task per tab.** A tab answers "what am I doing": editing, inserting, drawing,
   formatting, working with units, or changing the view.
2. **Every command sits in exactly one tab.** (A test checks it.) The Properties panel and the
   right-click menu may repeat commands that the selection needs close at hand.
3. **No near-empty tab.** Every ordinary tab but Help holds at least four commands in at least two
   groups (a test checks it). A lone button belongs in a bigger tab.
4. **Large buttons for the main action of a group**, small stacked rows for the rest.
5. **Everything fits the narrowest window** (900 px). A test draws each tab and measures it.
6. File commands, Undo and Redo, Options and the recent list are not ribbon items: they live in the
   File menu and the quick access toolbar.

## Ordinary tabs

| Tab | Group | Contents |
|---|---|---|
| **Home** | Clipboard | Paste (large), Cut, Copy, Duplicate |
| | Regions | Math region (large), Text region (large) |
| | Edit | Find, Replace, Select all |
| | Region | Delete, Lock, Tag |
| | Calculate | Calculate worksheet (large), Automatic calculation (toggle) |
| | Check | Run checks (large), Next problem, Previous problem |
| **Insert** | Data | Matrix, Range, Table (large) |
| | Objects | Plot, Picture, Area (large) |
| | Programming | Solve block, Program (large) |
| | Symbols | Greek letters (large, opens a flyout of letters) |
| | Functions | Insert function (large, opens the Functions window) |
| **Draw** | Shapes | Rectangle, Ellipse, Triangle, Diamond, Polygon, Line (large; pick one, then drag on the page; Shift turns a line in 15 degree steps, Ctrl draws from the middle) |
| | Order | Bring to front, Bring forward, Send backward, Send to back |
| | Align | Align horizontally, Align vertically, Separate regions |
| **Format** | Text | Bold, Italic, Underline |
| | Numbers | More decimals, Fewer decimals, Scientific, Engineering |
| | Region | Border (toggle) |
| **Units** | Unit system | SI, USCS, CGS (split button) |
| | Result unit | A drop-down of the units of the selected result's kind, Automatic first |
| | Custom units | Define unit, Manage units |
| **View** | Zoom | In, Out, Fit, 100% |
| | Page | Page setup (large), Page View, Draft View |
| | Show | Grid, Snap, Labels |
| | Window | Reset layout (large) |
| **Help** | Help | Shortcuts, User guide *(dimmed)*, About |

## The Properties panel

There are no contextual tabs. The settings of the selected thing are in the **Properties** side
panel at the right of the page. It shares the window with the page (nothing is covered), can be
resized, and appears while one shape, plot, table, picture or text region is selected.

| Selected | Settings |
|---|---|
| Shape | Shape (kind), Corners (a polygon's sides), Fill, Outline, Width, Line (solid, dashed, dotted), Start and End arrowheads (lines only), then Arrange: draw order, Lock, Delete. A rectangle also has a corner grip on the page. |
| Plot | Title, x values, a section per series, labels, axes and grid, legend, log axes |
| Table | Name, column headings and units, cells, add and remove rows and columns, CSV import and export |
| Picture | Choose or remove the image, description |
| Text | Bold, italic, underline, text colours |

Fill and outline use a colour picker: Automatic, none, the palette with its shades, standard
colours, and exact RGBA with hex.

## Where the old commands went

| Old place | New place |
|---|---|
| Math > Regions, Clipboard, Edit, Region | Home |
| Math > Programming, Symbols | Insert |
| Math > Units, Result Unit, Custom Units | Units |
| Functions > Insert | Insert > Functions |
| Matrices/Tables | Insert > Data |
| Plots > Insert | Insert > Objects |
| Math Formatting | Format |
| Calculation | Home > Calculate and Check |
| Document > Zoom, Page, Show, Window | View |
| Document > Arrange, Shapes | Draw |
| Document > Insert (Area, Picture) | Insert > Objects |

## Still to do

- A right-click menu with the draw-order
  commands.
