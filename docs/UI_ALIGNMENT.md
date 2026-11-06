# Making the window and behaviour closer to Mathcad

Status: **proposed**. For review before any change.

## 1. Why this note exists

The spec (section 4) copied the *look and window layout of OpenDraft*, a CAD
application, and gave it a red accent. That gave OpenCalc docks and ribbon tabs that
a CAD user expects and a Mathcad user does not. The owner's goal is a Mathcad-style
worksheet, so the window and the everyday behaviour should follow Mathcad. This note
lists what Mathcad has, what OpenCalc has, and what to change.

**Sources and confidence.** Public PTC help only (see `CLEAN_ROOM.md`). Items marked
**[verified]** are stated in PTC's published help or guides. Items marked
**[inferred]** are from general knowledge of how Mathcad behaves and need a check
by someone who has the product in front of them. Nothing here is copied from the
product; only the behaviours are being matched.

## 2. What Mathcad Prime's window contains

**[verified]** from PTC's "About the PTC Mathcad Prime Workspace" page:

| Part | Notes |
|---|---|
| Title bar with an application button | The button holds file commands, Print, Options, About, Exit, recent worksheets |
| Ribbon | Tabs grouping related commands, with tooltips and KeyTips (press Alt or F10) |
| Quick Access Toolbar | Above the ribbon by default; the user can move it below |
| Open Worksheets Bar | One tab per open worksheet; a worksheet named *Untitled* is open at startup. **No Home page** |
| Worksheet area | Regions placed anywhere on the page: Math, Text box, Text block, Table, Plot, Solve block, Image |
| Status bar | Page numbers, a calculation-state indicator, Find and Replace, a zoom slider, Page View and Draft View buttons |
| Functions window | **Optional**, dockable, searchable list of built-in functions |
| Shortcut menus | Right-click on the worksheet or on a worksheet tab |

**[verified]** ribbon tabs named in PTC guides: Math, Input/Output, Matrices/Tables,
Plots, Math Formatting, Calculation, Document, and a Functions tab that opens the
Functions window. (Which commands sit on each tab, and whether View and Help are
tabs, was not confirmed.)

**There is no Names browser and no Region Properties pane.** PTC's region help
lists no properties pane either. Details of a region are changed in place or with
the ribbon.

## 3. What OpenCalc has now

| Part | Now |
|---|---|
| Startup | Opens a sample worksheet *and* a Home page tab |
| Ribbon tabs | Home, Insert, Format, Units, Calculate, View, Help (from OpenDraft) |
| Docks | Names and Units on the left, Region Properties on the right, both on by default |
| Status bar | Grid, Snap, Live, unit system, warnings count, zoom percentage |
| Canvas | Infinite, with a grid on by default |
| Results | A definition shows its value beside it (`F := 10 kN  = 10.0000 kN`), four decimals |

## 4. Differences, and what to do about each

Decision key: **Align** = change to match Mathcad. **Keep** = a deliberate difference.
**Later** = depends on other work.

| # | Difference | Decision | Notes |
|---|---|---|---|
| 1 | Docks open by default | **Align**: closed, and no permanent Names or Properties panes | Plot editing moves into the plot itself (see 3 below); until then Properties opens only while a plot is selected |
| 2 | Names browser | **Align**: remove. Replace with a **Functions window**: optional, searchable, from the Functions tab, built from the `FUNCTIONS` table (signature and summary), click to insert | |
| 3 | Plot editing in a side pane | **Align** (with the editor): a plot has editable placeholders for its x and y expressions on the plot, and a title; no side pane | Needs editor step E4 |
| 4 | Home page tab; sample opens at startup | **Align**: open one blank *Untitled* worksheet; no Home tab. The example moves to File > Example | |
| 5 | Ribbon tabs from OpenDraft | **Align**: Math, Functions, Matrices/Tables, Plots, Math Formatting, Calculation, Document (Input/Output when import and export exist). Content is mapped in section 5 | Panel contents are **[inferred]**; review before building |
| 6 | Definitions show their value | **Align**: `x := 5` shows only the definition; `x =` shows `x = 5` | **[inferred]** from how Mathcad worksheets read. Large visible change; see decision B |
| 7 | Four decimals, trailing zeros | **Align**: three decimals, trailing zeros not shown (`10 kN`, `0.333`) | **[inferred]**; the Math Formatting tab changes it |
| 8 | Click blank space and just start typing to make a math region; a quote mark starts a text region | **Align** | **[inferred]**; belongs with the editor (E4) |
| 9 | Typing `:` gives `:=` | **Align** (input rule, with E5) | **[inferred]** |
| 10 | Grid on and Snap in the status bar | **Align**: grid off by default; the options move to the Document tab. Status bar becomes: calculation state, Find, zoom slider, Page and Draft view | |
| 11 | Warnings count in the status bar | **Align**: errors show on the region itself (red, with the message) as in Mathcad; the Calculation tab can step through them | |
| 12 | Infinite canvas | **Later**: Mathcad worksheets are paged (Page View) with a Draft View. Paging arrives with print and PDF export (Milestone 3) | |
| 13 | Evaluation order | **Decision A** below | |
| 14 | Quick Access Toolbar and app button | **Keep** (already match) | |

## 5. Ribbon content (proposed, to be reviewed)

| Tab | Panels |
|---|---|
| Math | Regions (Math, Text), Greek and symbols, Units (unit system, insert unit, define and manage units) |
| Functions | Insert function (opens the Functions window), the built-in function list grouped by kind |
| Matrices/Tables | Vector and matrix literals, range, matrix functions; Table (later) |
| Plots | X-Y plot; further plot kinds later |
| Math Formatting | Result format (decimals, scientific, engineering), bold and italic |
| Calculation | Calculate worksheet, automatic calculation on or off, next and previous error, check |
| Document | Zoom, grid and snap, Page and Draft view (later), align and separate regions, area, picture |
| Input/Output | Hidden until CSV import and export exist (Milestone 3) |

## 6. Decisions needed

**A. Evaluation order.** Mathcad evaluates **top to bottom, left to right**: a name
must be defined above or to the left of where it is used, otherwise the region shows
an error. OpenCalc's spec instead chose **order-independent** evaluation by a
dependency graph (a region can use a name defined below it), and it is built and
tested that way. The practical differences: in Mathcad, redefining `x` partway down
gives later regions the new value and earlier ones the old; in OpenCalc the last
definition applies everywhere and a warning is shown. Options:
1. **Keep the dependency graph** (recommended: more forgiving and already proven), and
   record it as a deliberate difference, or
2. **Switch to Mathcad's order**, which means worksheets from Mathcad behave the same
   but a name used before it is defined becomes an error.

**B. Definitions show no value** (item 6). It is closest to Mathcad but removes the
at-a-glance value beside each definition. A middle path is a Document option "show
values next to definitions", off by default.

**C. Start with a blank Untitled worksheet and no Home tab** (item 4).

## 7. Phases

| Phase | Content |
|---|---|
| U1 | Remove default docks and the Names pane; blank Untitled at startup; no Home tab; grid off by default; add the Functions window. Properties opens only for a selected plot |
| U2 | Ribbon rebuilt to the tabs above (commands unchanged, rearranged and renamed) |
| U3 | Results only on `=` regions (if B is agreed); three-decimal default; errors on the region |
| U4 | Status bar to Mathcad's items, with a zoom slider |
| later | Click-and-type region creation, `:` to `:=`, in-plot placeholders (with the editor); paging (with export) |

The editor work (E2 to E6) continues after U1 to U3, because U1 to U3 change what the
user sees first and the editor steps are internal until E4.
