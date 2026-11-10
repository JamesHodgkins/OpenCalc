# Ribbon map: today's commands to Mathcad-style tabs (U2)

Status: **built (U2)**. Owner decisions: the start-page Home tab stays (recent files, release news later; there is no ribbon Home tab); Units sit on the Math tab; the theme moved to an Options window opened from the application menu. Also removed with the old tabs: the placeholder font-size slot.

Tab names come from PTC's public guides (Math, Input/Output, Matrices/Tables, Plots,
Math Formatting, Calculation, Document, and a Functions tab). Which commands sit
on each tab was only partly confirmed (Math covers operators, symbols and units;
Matrices/Tables covers vectors and matrices; Plots covers 2D and 3D graphs;
Input/Output covers reading data). Everything else below is **inferred**; it is a
reasonable grouping, not a copy. See `UI_ALIGNMENT.md` and `CLEAN_ROOM.md`.

## 1. Proposed tabs

| Tab | Panels | Contents |
|---|---|---|
| **Math** | Regions | Math region (large), Text region (large) |
| | Programming | Solve block (large), Program (large) |
| | Clipboard | Paste (split), Cut, Copy |
| | Edit | Find, Replace |
| | Region | Delete, Lock, Tag |
| | Symbols | Greek letters (flyout) |
| | Units | Unit system (SI, USCS, CGS split), Display in (metres, millimetres, inches, newtons, kilonewtons, reset), Define unit, Manage units |
| **Functions** | Insert | Insert function (opens the Functions window) |
| **Matrices/Tables** | Matrix | Matrix and vector, Range *(new, need the editor, dimmed until E4)* |
| | Table | Table region *(dimmed, as today)* |
| **Plots** | Insert | X-Y plot (large) |
| **Math Formatting** | Result | More decimals, Fewer decimals, Scientific, Engineering *(dimmed, as today)* |
| | Font | Bold, Italic *(dimmed, as today)* |
| | Region | Border (toggle: a line round each selected region) |
| **Calculation** | Calculate | Calculate worksheet (large), Automatic calculation (toggle, moved from the status bar and View) |
| | Check | Run checks *(dimmed until `opencalc check`)* |
| | Errors | Next, Previous *(dimmed until errors are navigable)* |
| **Document** | Zoom | In, Out, Fit, 100% |
| | Page | Page setup: size, orientation, margins *(new; P2)* |
| | Show | Grid, Snap |
| | Arrange | Align horizontally, Align vertically, Separate regions *(dimmed, as today)* |
| | Insert | Area, Picture *(dimmed, as today)* |
| | Window | Properties pane (optional), Reset layout, Switch palette |
| **Help** | Help | Shortcuts, User guide, About |
| *Input/Output* | (hidden) | CSV import and export arrive in Milestone 3 |

## 2. Where every existing command goes

| Command | Now | Proposed |
|---|---|---|
| New, Open, Save, Close, Quit | app button, Quick Access Toolbar | unchanged |
| Undo, Redo | Home tab and Quick Access Toolbar | Quick Access Toolbar only (already there) |
| Paste, Cut, Copy | Home | Math > Clipboard |
| Find, Replace | Home | Math > Edit |
| Delete, Lock, Tag | Home | Math > Region |
| Math region, Text region | Insert | Math > Regions |
| Solve block, Program | Insert | Math > Programming |
| Greek letters | Insert | Math > Symbols |
| Area, Picture | Insert | Document > Insert |
| Plot | Insert | Plots > Insert |
| Table | Insert | Matrices/Tables > Table |
| Bold, Italic | Format | Math Formatting > Font |
| Decimals more, fewer, Scientific, Engineering | Format | Math Formatting > Result |
| Align, Separate | Format | Document > Arrange |
| SI, USCS, CGS, display units, Define, Manage | Units | Math > Units |
| Calculate all | Calculate | Calculation > Calculate |
| Live recalculation | status bar and View | Calculation > Calculate (toggle) |
| Changed only | Calculate | **removed**: automatic calculation makes it meaningless |
| Run checks | Calculate | Calculation > Check |
| Warnings list, next, previous | Calculate | Calculation > Errors (next, previous); the list is **removed** because errors show on the regions |
| Zoom in, out, fit, 100% | View | Document > Zoom |
| Grid, Snap | View | Document > Show |
| Guides | View | **removed** (never built; the margin box covers it) |
| Functions window | View | Functions > Insert (and Document > Window) |
| Properties pane, Reset layout, Switch palette | View | Document > Window |
| Shortcuts, User guide, About | Help | unchanged |

## 3. Tabs that go away

Home (ribbon), Insert, Format, Units, Calculate, View. Their contents are spread as
above. The document **Home tab** (the start page beside the worksheet tabs) is not a
ribbon tab and stays.

## 4. What this costs

- It is a rearrangement and renaming of data in `ribbon/defs.rs`; command behaviour
  does not change, so existing tests of commands still hold.
- New commands (Page setup, Matrix, Range) are added dimmed and implemented later
  (P2 and the editor), so the ribbon never shows a control that looks live and is not.
- Tests: every command still appears exactly once; every ribbon item names a real
  command; the tab list and panel titles are as above; nothing implemented is
  unreachable from the ribbon.
- Status bar (U4): built; page, calculation indicator and a zoom slider.

## 5. Decisions needed

1. **Which "Home tab" did you mean to keep?** The start page beside the worksheet
   tabs (this note assumes so), or the ribbon's Home tab (Clipboard, Edit, Region)?
   Mathcad has no ribbon Home tab, so the default here distributes its contents to
   Math. If you want a Home ribbon tab, it would hold Clipboard, Edit and Region and
   the Math tab would keep Regions, Symbols and Units.
2. **Units on the Math tab** (as PTC's guide lists "operators, symbols and units"
   there), or a tab of its own as now?
3. **Switch palette** under Document > Window, or somewhere else (for example the
   application button)?
