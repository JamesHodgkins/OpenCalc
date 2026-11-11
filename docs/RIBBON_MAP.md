# Ribbon map

Status: **built (redesigned 2026-11-11)**. This replaces the first ribbon map (Mathcad-style
tabs: Math, Functions, Matrices/Tables, Plots, Math Formatting, Calculation, Document). That layout
left tabs with one button, a Math tab with nine unrelated groups, and a Document tab that mixed page,
view and drawing commands. The owner asked for it to be redone from scratch.

The tab names and groups below are **our own, organised by task**; they are not a copy of any
other product's ribbon (see `CLEAN_ROOM.md`). The ribbon is data in `ribbon/defs.rs`.

## Rules

1. **One task per tab.** A tab answers "what am I doing": editing, inserting, drawing,
   formatting, working with units, or changing the view.
2. **Every command sits in exactly one ordinary tab.** (A test checks it.) A *contextual* tab may repeat
   commands that its selection needs close at hand.
3. **No near-empty tab.** Every ordinary tab but Help holds at least four commands in at least two
   groups (a test checks it). A lone button belongs in a bigger tab.
4. **Large buttons for the main action of a group**, small stacked rows for the rest.
5. **Everything fits the narrowest window** (900 px). A test draws each tab and measures it.
6. File commands, Undo and Redo, Options and the recent list are not ribbon items: they live in the
   File menu and the quick access toolbar.

## Ordinary tabs

| Tab | Group | Contents |
|---|---|---|
| **Home** | Clipboard | Paste (large), Cut, Copy |
| | Regions | Math region (large), Text region (large) |
| | Edit | Find, Replace |
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

## Contextual tabs

A contextual tab belongs to the selected thing. It is shown, tinted along its top edge, only while
that thing is selected, and comes forward once when it is drawn. Selecting it again does not jump to it.

| Tab | Shown while | Groups |
|---|---|---|
| **Shape Format** | one shape is selected | Shape (kind, corner radius or a polygon's sides; a rectangle also has a corner grip on the page), Style (fill, outline, width, line style), Arrowheads (start and end of a line), Arrange (draw order, Lock, Delete) |

Value controls (a colour, a number, a choice) are drawn by the app through `ControlHost`, because a
ribbon item otherwise only runs a command. Fill and outline use a colour picker: Automatic, none, the
palette with its shades, standard colours, and exact RGBA with hex.

Planned, each replacing a floating window the same way: **Picture**, **Table** and **Plot**.

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
- Picture, Table and Plot contextual tabs (their floating windows go away as they arrive).
