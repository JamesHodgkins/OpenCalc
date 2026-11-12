# Feature gap roadmap

Closes the gaps between OpenCalc and `private/mathcad_features.md`, in the order agreed
on 2026-11-08. Tick a box when the work is merged and tested. Design context is in
`ARCHITECTURE.md`; known shortcuts are in `TECH_DEBT.md` (TD-nn references below).

**Rules for every item:** no silent approximation of maths or units (an unsupported
operation gives a clear error); one logical edit is one undo step; a change to the saved
form needs a `FORMAT_VERSION` bump (currently 1) and a migration; files stay under the
size limit; `scripts/check.ps1` passes.

**Sizes:** S under a session, M a few sessions, L a milestone's worth.

---

## Phase 0: Housekeeping (S)

- [x] Refresh `TECH_DEBT.md`: TD-12 and TD-13 resolved; TD-09, TD-11, TD-21, TD-24, TD-26 corrected. Engine entries (TD-06, 08, 10, 20, 23, 28) were not re-audited.
- [x] `HelpGuide`: left dimmed on purpose. The user guide is paused until the shared GitHub Pages approach is settled; when it returns the button should open the hosted URL.
- [x] Progress is recorded in the log below as items are ticked.

## Phase 1: Cheap gaps in functions and units (M)

Extends `functions.rs`, `functions/stats.rs`, `units.rs`. No format change unless noted.

**1.1 Units**
- [x] Remaining SI base units (`A`, `K`, `mol`, `cd`) and derived units (`J`, `kJ`, `MJ`, `kWh`, `cal`, `BTU`, `W`, `kW`, `MW`, `hp`, `C`, `V`, `ohm`, `Hz`, `bar`, `atm`, `L`, `mL`, `min`, `hr`, `day`, ...). Ohm is spelt `ohm`. Done 2026-11-08.
- [x] Angle units (decided 2026-11-08): `rad` is 1 and `deg` is pi/180, both plain numbers, the only dimensionless built-in units. `sin(30 deg)` works; an inverse function gives radians, and `asin(0.5) / deg` reads in degrees. Values display without a unit (`30 deg` shows as 0.524), so `30 deg + 3` is allowed. A variable named `rad` or `deg` shadows the unit; a number written directly before the name (`2 rad`) is still the unit. Not done: showing a result in degrees (needs a display-unit choice) and `grad`, arc minutes and arc seconds.
- [x] More US customary and length/mass/time units (`yd`, `mi`, `oz`, `gal`, `psf`, `um`, `nm`, `mg`, `tonne`, `ms`). `ton` (short or metric?) left out on purpose; `tonne` is the metric one. Done 2026-11-08.
- [ ] Note: new unit symbols (`A`, `C`, `K`, `L`, `V`, `W`, `J`) take over `5 A`-style implicit multiplication from variables of those names, and a saved custom unit with one of these names would now be rejected. No released files exist yet; revisit if that changes.
- [x] Compound units (2026-11-08, TD-09). Input already worked as ordinary expressions (`5 m/s`, `9.8 m/s^2`, `5 kN*m`, and custom units defined from them, such as `1000 lbf / ft^2`): a unit binds to the number before it, so `250 m / (9 s)` is not `250 / 9 m / s`. What was missing was display, now done: named display units for speed, acceleration, density and stiffness in each unit system, and any other dimension written as numerator over denominator in the chosen system's own mass, length and time units (`kg*m/s`, `lb*ft/s`, `kg/(m*s)`). Not done: a compound unit as a single typed symbol, a compound result-unit choice, and display of `m/s` as `km/hr`.
- [x] Temperature units (decided 2026-11-08, TD-09). Everything is calculated in kelvin. `degC` and `degF` are offset units accepted only directly after a number (`20 degC`); `-5 degC` means minus five degrees Celsius (268.15 K); a bare, powered or compound use (`degC`, `degC ^ 2`, `W / degC`) is refused with a message pointing to `deltaC` / `deltaF`. `deltaC`, `deltaF` and `degR` are ordinary scale-only units for differences and rates. Results show in K; `20 degC + 10 degC` is not refused (plain kelvin arithmetic). Not done: showing results in degC or degF (needs new display-unit choices, a file-format and ribbon change).
- [x] Per-element dimensions in vectors and matrices (TD-28), done 2026-11-08, no format change (values are not saved): `vec(1 m, 2 s)`, `[2 m, 3 m/s]`, `mat(...)` with mixed units. Stored once when uniform and per element only when they differ, so uniform matrices behave and compare exactly as before. Works with: indexing (each element keeps its unit), `+` and `-` (element by element, units must match), scaling, matrix products (every term of each sum must agree), `transpose`, `dot`, `cross`, `augment`, `stack`, element-wise functions, display (`2x1 [2 m; 3 s]`), the solver (unknowns of different kinds). Refused with a unit mismatch, because they need one unit throughout: `det`, `^` (inverse and powers), `tr`, `rank`, `norm`, `sum`/`mean`/`min`/`max`/statistics, `sort`/`reverse`/`cumsum`, `polyroots`, plotting a vector. `Value::dim()` became `uniform_dim()` and `dims()`.

**1.2 Functions**
- [x] Inverse and hyperbolic trig: `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh`. Real inside the real domain, complex outside. Open points: `atan2(x, y)` argument order follows Mathcad as I recall it and should be checked against PTC's help; the sign of the imaginary part on a branch cut (`asin(2)`) is whatever `num-complex` gives and is not asserted by any test.
- [x] Logs and rounding: `log(x)` (base 10), `log(x, b)`, `floor`, `ceil`, `round(x[, n])`, `trunc`, `sign`, `mod` (sign of the divisor), `gcd`, `lcm`. No separate `log10`/`log2`. Rounding needs a dimensionless argument (rounding a length would depend on the display unit). `ln` was already there.
- [x] Element-wise functions on matrices (TD-08): every one-number function except `abs` (which is not element by element for a matrix). Done 2026-11-08.
- [x] Matrix: inverse and integer powers as `M ^ n` (including `M ^ -1`), `identity`, `tr`, `rank`, `cross`, `augment`, `stack` (`rows`/`cols` existed). Singular and rank tolerance: largest element x size x machine epsilon. Done 2026-11-08.
- [ ] `submatrix`: needs the evaluator's `ORIGIN` so it does not silently differ when a worksheet sets `ORIGIN := 1`. Do with the Phase 2 evaluator context work.
- [x] Stats: `median`, `mode` (smallest on a tie), `var` and `var_sample`, `sort`, `reverse`, `cumsum`, `cvar` (population covariance), `corr`. Done 2026-11-08. Not done: `range`, row vectors in `sort`/`reverse`/`cumsum` (column vectors and lists only).
- [x] Number theory and combinatorics: `factorial` (to 170), `combin`, `permut`, `isprime` (exact for every whole number to 2^53). Done 2026-11-08. The `n!` postfix operator is a parser change and is not done.
- [ ] Text and boolean values (TD-08), needed by later string functions and programming. **Text, minimal part done (2026-11-10, for `on error e`)**: `"like this"` (no escapes, 256 characters at most), `Value::Text`; `+` joins, `==` and `!=` compare two texts; everything else on text is an error; shown with its quotes. Not done: string functions, number-to-text, text in matrices, booleans (comparisons still give 1 and 0).

**Exit:** each function has a unit test with a hand-checked value and a dimension-handling test; corpus gains a worksheet per group.

## Phase 2: Calculus and solving (L)

Depends on Phase 1 (inverse, boolean values). Builds the cancellation token that TD-20 is waiting for.

**2.1 Numeric calculus**
- [x] Cancellation and a step budget in the evaluator (TD-20), done 2026-11-08: every repeated evaluation is charged to a per-region `Meter` (1,000,000 steps, deterministic, shared by nested forms); the worker raises a cancel flag when a newer edit arrives, giving the new `EvalError::Cancelled`.
- [x] Evaluator forms (2026-11-08): `integrate(f, x, a, b)` (adaptive Gauss-Kronrod 7/15, relative accuracy 1e-10, at most 400 pieces), `derivative(f, x, at)` (Ridders extrapolation, with a corner check), `limit(f, x, to)` (two-sided Richardson extrapolation, never evaluating f at the point), `sum_over(f, k, a, b)` and `product_over(f, k, a, b)` (whole-number bounds; an empty range is an error). The second argument is a variable local to the form, shadowing outer names, units and variables, and it is not a dependency of the worksheet. A form that cannot reach its tolerance (pole, jump, corner, oscillation) is an error, never a number.
- [ ] Typeset notation for them (integral sign, d/dx, big sigma and pi, lim) and the parser/editor syntax for it: waits for the structured editor; today they are written as function calls.
- [ ] Higher-order derivatives, limits at infinity, one-sided limits, improper integrals (infinite bounds).
- [x] Units through integrals and derivatives: the result's dimension is the integrand's times the variable's (integral) or divided by it (derivative); the two bounds must share a dimension (write `0 m`, not `0`).

**2.2 Solvers**
- [x] `root` and `polyroots` (2026-11-08). `root(f, x, a, b)` runs Brent's method inside a bracket where f changes sign (guaranteed to converge; accurate to about 2 eps |x| plus 1e-14 of the bracket). `root(f, x, guess)` steps outward from the guess, growing by 1.6 and dropping any side where f cannot be evaluated, until f changes sign, then runs the same method; it finds the nearest sign change, not necessarily the nearest root, and fails clearly when there is none. A sign change at a pole or jump (`1/x`, a step) is refused as a discontinuity, not returned as a root. Real roots of real functions only; the bounds share a dimension. `polyroots(v)` takes coefficients lowest power first (`vec(2, -3, 1)` is 2 - 3x + x^2) and returns all roots as a sorted column vector (Aberth-Ehrlich iteration; degree up to 64; roots at zero taken out exactly; complex coefficients allowed; a root of a real polynomial with a tiny imaginary part, 1e-12 relative or less, is shown as real; repeated roots are located only to about 1e-8). Not done: complex roots of `root`, multiple roots at once, `root` over a matrix of guesses.
- [x] Solve blocks, Stage 1 (2026-11-08, `SOLVE_BLOCKS.md`): `find`, `minerr`, `minimize`, `maximize` as evaluator forms, no format change. Unknowns are named in the call and their guesses are the worksheet values of the same names; constraints use `==`, `<`, `<=`, `>`, `>=`; one unknown gives a number, several a column vector. Levenberg-Marquardt for `find`/`minerr` (`find` is an error rather than an unsatisfied point; `minerr` returns the best point), BFGS with an augmented Lagrangian for `minimize`/`maximize`. Constraints held to 1e-9 (1e-7 when optimising). Each unknown keeps its own guess's dimension. Local methods: the guess matters. Not done: complex unknowns, MultiStart, matrix unknowns.
- [x] Solve blocks, Stage 2 (2026-11-10), **without a new region kind**: the `given` block is a multi-line form inside an ordinary math region (like a program), so no format change and none of the 24-file audit. Written `r := given`, the constraints on indented lines, then a `find(...)`, `minerr(...)`, `minimize(...)` or `maximize(...)` line at the indent of the `given` line. It lowers to the Stage 1 call (`Expr::Given`; children in text order), so the numerics, tolerances and errors are Stage 1's; typeset as Given / the constraints beside a bar / Find(...); edited like a program; guesses are worksheet variables as before. **Naming the results** (same day): a line `x := 1` in the block is a guess for `x`, local to it, and `[x; y] := given ...` defines each name from its element of the result with its own unit (`Expr::MultiDefine`, which works for any vector: `[a; b] := [1 m; 2 s]`). Math > Programming has Solve block and Program, each inserting a working template (it finds 2; it doubles a number) with the name selected, into the region being edited or a new one.
- [x] `minimize` / `maximize`: done in Stage 1 above. A choice of algorithm is not planned.

- [x] `odesolve` (2026-11-08): `odesolve(f, t, y, t0, y0, t1)` solves dy/dt = f(t, y) and gives the state at `t1`; `odesolve(..., n)` gives a table of n + 1 rows `[t, y1, y2, ...]` with a unit per column. `t` and `y` are the names f uses; they are local to the call. The state is a number or a column vector whose elements may have different units (`vec(100 m, 0 m/s)`); f must return the same shape with the units of y per unit of t (checked at every evaluation). Adaptive Dormand-Prince 5(4) Runge-Kutta, relative tolerance 1e-9 with an absolute part scaled to the problem, reaching `t1` exactly, forward or backward in time. At most 100,000 steps, every evaluation metered and cancellable; a stiff or exploding equation is an error that says so. A stage where f cannot be evaluated only shortens the step. Not done: stiff solvers (Radau/BDF), events, the solution as a function of time, `pdesolve`, complex states.
- [x] `col(M, j)` and `row(M, i)` (2026-11-08): a column or row of a matrix, counting from `ORIGIN` like indexing, keeping each element's unit. They are evaluator forms because plain functions cannot see `ORIGIN`; the same mechanism will serve `submatrix`.
- [ ] Non-convergence diagnostics and error badges; MultiStart.
- [ ] Linear-algebra extras: eigenvalues/eigenvectors, LU/QR/Cholesky/SVD.

**Exit:** solver corpus worksheets with known answers; non-convergence is an error, never a wrong number; the cancel path is tested.

**Deferred beyond this phase:** `pdesolve`, KNITRO-class constrained optimisation, symbolic solutions.

## Phase 3: Plots (L)

Extends `PlotSpec` (format bump with migration). TD-31 records the current plot decisions.

- [x] Plot options and series styles (2026-11-08), **file format 2** (a real migration step; format 1 files open unchanged, fixtures of both are tested). Per plot: axis limits for x and y (math sources, so they carry units or read a variable; blank is automatic; a limit must have its axis's dimension and the minimum must be below the maximum), grid on or off, legend automatic, shown or hidden, and a logarithmic x or y axis (every value and limit on it must be positive, else an error naming the part). Per series: type (line, markers, line and markers, step, stem, bar), marker shape (circle, square, triangle, cross), a colour from the palette, and a legend name. Columns grow from zero, group side by side when there are several bar series, and leave room for half a column at each end; markers and stems get a little room too. Edited in the Plot window (a section per series, an Axes and grid section), searched and replaced by Find and Replace, painted identically by the PDF and PNG export. Not done: error bars, secondary axes, line width and dash styles, user-chosen tick marks.
- [x] Parametric plots (2026-11-08): a series may have its own x values (a math source in its settings; blank uses the plot's x), so `sin(t)` against `cos(t)` draws a circle with nothing in the shared x field. All x values of one plot must share a dimension; a series lacking its own x still needs the shared ones. `f(x)`-style plots need no new syntax: a range variable already gives them. Same file format 2 (not yet released, so extended in place).
- [x] Mixed units (2026-11-08): a second, right-hand y axis. A series is set to the left or right axis; the series on one axis share a dimension, the two axes may differ (a displacement and a force on one plot). The right axis has its own unit, limits, log setting, ticks, caption and baseline for bars and stems; the legend marks `(right)` series; the evaluator checks units per axis and reports the part (`series 3`, `right axis max`). A series asking for the right axis with none on the left is drawn on the left; right-axis limits are ignored while nothing uses it. Not done: more than two axes, and x axes of different kinds.
- [x] Plot types so far (2026-11-08): bar/column, stem and scatter (markers) are series types (Phase 3.1); **polar** is a plot type (`kind` in the plot options, file format 2). A polar plot's x values are angles in radians (plain numbers; `30 deg` works, a unit like seconds is an error) and each series is a radius, shown in a readable unit with its caption. Drawn with rings (radius, labelled in the unit), spokes every 30 degrees with degree labels set outside the ring, and an outer ring at a round number above the data (or the plot's y maximum); a negative radius lies on the opposite side; line, markers and line-and-markers series work, steps draw as lines, stems and bars draw as spokes from the centre. Log axes, x limits and the right axis do not apply and are hidden in the window and ignored. In the Plot view window hover names the angle and radius, the status line gives the pointer's radius and angle, scroll zooms the outer ring, pan does nothing. Not done: error bars, box plots, an inner radius, a rotated zero angle or clockwise direction.
- [ ] Contour plot, then 3D (surface, curve, scatter) with rotate/pan/zoom.
- [x] Interactive plot view (2026-11-08), without `egui_plot`: the Plot window's "Explore this plot" button opens a resizable Plot view window with the same painter as the canvas. Hover shows a crosshair and, near a data point (within 14 px), a ring round it and a readout of its series and values in the units of its axis (`load (right): x = 3 s, y = 12.5 N`); the pointer's own values are shown in a status line, also for the right axis. Scroll or pinch zooms about the pointer on every axis (the data under the pointer stays put), drag pans, double-click or Reset view goes back to the plot's own view. The view (a range per axis in SI) lives in the window's own memory: it is never saved, has no undo, and does not mark the worksheet changed. Pure, tested pieces: axis zoom/pan/inverse mapping (linear and log, with limits on how far it goes), nearest-point search, readout text. Not done: box zoom, zoom of one axis only, a trace that follows a series, copying the readout.
- [x] Export of plots in PDF/PNG covers every type built so far: the export paints with the same code as the canvas, and rendering tests check the pixels of each series type and option.

## Phase 4: Data I/O (M to L)

Fills the Table and Picture placeholder regions. Design and open decisions:
`DATA_IO.md`. Started: the CSV module (`opencalc-core::csv`) is built; the rest waits for
the decisions in `DATA_IO.md` section 7.

- [x] CSV reader and writer module (own parser, bounded, tested; no UI yet).
- [x] Table region (editable grid, evaluates to a matrix): `TableSpec`, `Edit::SetTable`, painting, a table window for editing; CSV import and export buttons in the table window (commands `table.import_csv`, `table.export_csv`).
- [ ] ~~CSV/text read and write functions~~ proposed to be replaced by import and export commands (`DATA_IO.md` section 2); awaiting decision.
- [x] Picture region: PNG only, embedded in the `.ocalc` (base64 in the worksheet JSON), Picture window to choose, remove and describe. JPEG is not supported (no decoder dependency yet).
- [x] CSV import and export (in the table window; the Input/Output ribbon tab entry is still to add).
- [ ] Excel (`.xlsx`) read and write; evaluate a crate against the dependency policy first.
- [ ] Image data to matrix and back.

## Phase 5: Document features (M)

Design for the items still open: `DOCUMENT_FEATURES.md` (decisions A to E).

- [x] Headers and footers (first page and following pages): left, centre and right text with `{page}`, `{pages}`, `{title}` fields, edited in Page setup; drawn on the page and in PNG and PDF exports. No date field, on purpose.
- [x] Page background styles: plain, grid, lines, dots; paper tints; left out of exports unless ticked.
- [x] Global definitions `≡`: satisfied by design (every definition is already global); `≡` is accepted as `:=`. Cross-worksheet references stay out of scope.
- [x] Rich text in text regions: bold, italic, underline and 8 colours on marked characters or the whole text (`runs` on the region, `Edit::SetRuns`); real Inter Italic and Bold Italic bundled (TD-24 resolved). Out: font families and sizes, lists, links.
- [x] Region borders: `Region.border`, `Edit::SetBorder`, Math Formatting > Region > Border toggle; drawn on the page and in exports.
- [ ] Content protection: password-protected regions and hidden content (needs a security design before code).
- [x] Templates (`.ocalct`, the worksheet format with `kind: template`): New from template, Save as template. No built-in templates yet; no file association for `.ocalct` yet.

## Phase 6: Programming (L)

Needs Phase 1 (booleans, text) and Phase 2 (cancellation). Design and open decisions:
`PROGRAMMING.md`.

- [x] Multi-line programs with local variables (2026-11-10, `PROGRAMMING.md` section 7). `f(x) := program` and indented lines under it, or any definition's value; no file format change (the source text holds the line breaks). Locals are assigned with `←` (typed `<-`), belong to the program from its first line to its last, and never leak; a name read before it is given a value is an error, not the worksheet's. The program's value is `return`'s, else its last line. Typeset with a bar per block; edited in its typeset form, with the bars (Enter = new line with the indent carried, Enter on a blank last line = done, Tab / Shift+Tab indent, Escape or a click away also end it). A line that does not parse yet is kept as typed, and the new line being typed shows as a box where it will go. Element assignment `v[i] ← x` / `m[i, j] ← x` creates the vector or grows it with zeros of the new element's unit.
- [x] `for`, `while`, `break`, `continue`, `return` (2026-11-10). `for x in range-or-vector-or-matrix` (a matrix goes through its elements row by row, each keeping its unit); every statement and every pass of a loop costs a step of the region's budget and checks for cancellation, so a runaway `while` ends with the budget error. `if cond` / `else if` / `else` blocks; `if(a, b, c)` on a line of its own is still the expression.
- [x] `try` / `on error` (2026-11-10): the `on error` block runs if the `try` block fails. It never catches cancellation or the step budget. `on error e` binds the error's message to the text `e` in the handler (a local of the program, like any other).
- [x] Recursion (2026-11-10), reversing the TD-30 decision (recorded there): a function may call itself or others in a cycle. The graph treats a cycle of only function definitions as fine (a body runs at the call, not at the definition); a cycle that includes a variable stays circular. Guards: calls nest 64 deep (an error that says so), each call is a step of the region's budget (so `fib(32)` stops on the budget). The evaluation stack grew from 8 MB to 64 MB (virtual; committed as used) and the headless `check` now evaluates on such a thread too.
- [x] Default arguments (2026-11-10): `f(x, n := 2) := x^n`. Defaults are the trailing parameters (one without a default cannot follow one with), are worked out at the call in the worksheet's scope and may not use the other parameters; a call may leave out trailing arguments (`f(3)`), else the arity error says "takes 1 to 2 arguments". `:=` rather than `=` because `=` already means evaluate. A default written outside a definition is a parse error. Not children of the definition in the tree (no error paths into them).
- [x] Functions that return several values (2026-11-10): `return [a; b]` gives a vector; no tuple type (decided).
- [x] Manual calculation mode (2026-11-10): the Automatic calculation toggle now means it. Off holds back every recalculation, not only typing (insert, delete, paste, undo too); Calculate (Ctrl+F9) runs it; results left on screen are dimmed and the status bar says "Manual: results out of date" until then; turning automatic back on catches up at once. Per session, never saved in the file. Switching worksheet always calculates the one you switch to (the results on screen belong to one sheet). Export in manual mode does not mark stale numbers.
- [x] Binary, octal and hexadecimal numbers (2026-11-10): `0x1F`, `0b1011`, `0o17` (prefix form, since the suffix forms clash with units `h` and `b`); exact up to 2^53, larger is an error. Not done: showing a result in another base, bitwise functions.

## Phase 7: Controls and scripting (L, optional)

- [ ] Scripted controls: combo box, list box, slider, check box, radio button, button, text box.
- [ ] Scripting bridge (Python first); needs a sandbox and a security review before code.

## Drawing shapes (M)

Design decisions: draw order is a `z` number on each region, kept apart from list order,
because list order is creation order and decides which of several definitions of one name
wins; reordering the drawing must never change a calculation. Colours are a palette colour
with a shade, an exact RGBA value, or Automatic, as in common office programs.

- [x] Core model (2026-11-11): `RegionKind::Shape` (rectangle with optional rounded corners, ellipse; fill, outline, width, corner radius, dash), `Colour`, `Region::z`, `Edit::SetShape` and `Edit::SetZ`, file format 3 (`FILE_FORMAT.md` 3e).
- [x] Painting and hit-testing (2026-11-11): back to front by `z`; an empty frame is hit only on its outline, so what lies inside stays reachable; shapes appear in PDF and PNG export.
- [x] Insert > Shapes and draw order commands (Bring to front, Bring forward, Send backward, Send to back; Ctrl+] and Ctrl+[, with Shift for front and back).
- [x] Shape settings with a palette colour picker (theme colours and shades, standard colours, RGBA and hex); dragging a value is one undo step. They live in the **Properties** side panel (2026-11-12), which appears while one shape is selected; they were first a floating window and then a contextual Shape Format ribbon tab, both gone.
- [x] Drag to draw a new shape (the shape insert commands arm a crosshair; drag sets the box, Escape or a click without a drag gives the default size).
- [x] Resize handles (eight, on one selected shape, picture, plot or area; typeset regions size themselves) and drawing modifiers: Shift keeps equal sides or the original proportions, Ctrl works from the centre; one undo step per drag (`Edit::SetRegionBox`); a status-bar hint while the tool is out.
- [x] Alt+click to pick what lies under a filled shape (2026-11-11): the first goes to the region under the topmost, each further one a step down, wrapping to the top.
- [ ] A right-click menu with the arrange commands (waits for a right-click menu).
- [x] Lines and arrows (2026-11-11): a `Line` shape kind in the region's box with flip flags, a grip at each end (one undo step per drag, `Edit::SetLineEnds`), Shift turns it in 15 degree steps, hit-testing along the stroke, arrowheads (triangle, open, circle, diamond) at either end.
- [x] Triangle, diamond and polygon (a side count) shapes (2026-11-11).
- [ ] Rotation, grouping, gradients and shadows, text inside a shape.
- [x] Shapes and lines align by their box like any region; Separate regions leaves shapes alone (a frame round a result overlaps it on purpose) (2026-11-11).
- [x] Snap to a shape's edges (2026-11-11): while moving (any region), pulling a resize edge, or dragging a line end, an edge or middle within 6 pixels of a shape's edge or middle is pulled onto it, beating the grid on that axis, with a guide line; Alt turns it off with the grid.
- [ ] Distribute (even spacing) for any regions.

## Ribbon redevelopment (M)

- [x] Contextual tabs and hosted value controls (2026-11-11); the Shape Format tab. Removed 2026-11-12: its settings moved into the Properties side panel, and the ribbon has no contextual tabs.
- [x] Ordinary tabs redone by task: Home (with Calculate and Check), Insert, Draw, Format, Units, View, Help; no tab with one or two buttons; each tab measured to fit the narrowest window (`RIBBON_MAP.md`).
- [x] Home page redesign (2026-11-12), laid out like OpenDraft's: header (logo, version, drop hint); a Start row of four tiles (New in the brand colour, Open, From template, Example); Recent worksheets as cards (kind badge, name, folder, age and size, greyed and marked Missing when the file is gone, Clear list); a Getting started card in a side column from 900 px wide. The layout is a pure, tested function. Thumbnails and pinning were added the same day (below). Not done: recovery and update cards.
- [x] Home page thumbnails and pinning (2026-11-12): each recent file gets a picture of the top of its first page, drawn on a background thread by the export code (read, calculated, drawn small; kept for the session, not on disk), and a pin button; pinned files are listed first and stay however many others follow, are saved with the settings, and survive Clear unpinned. A card menu (button or right-click): Open, Pin or Unpin, Copy path, Remove from this list. The program now opens on the Home page instead of a blank worksheet.
- [x] Properties side panel (2026-11-12): the Plot, Table, Picture and Text settings moved out of four floating windows (fixed in place, covering the page) into one resizable panel at the right that shares the window with the page.
- [x] Alt+click to pick under a filled shape.
- [ ] A right-click menu with the draw-order commands.
- [x] Lines and arrows in Draw > Shapes.

## Out of scope for now

Symbolic engine (`->`), `pdesolve`, DOE, signal/image-processing library, Fourier and
digital filtering, OLE, Creo integration, whole custom unit systems, function-analysis
functions. Revisit after Phase 4; each needs its own design note first. Fourier and
statistics-library items can move up if the Phase 1 function work goes quickly.

## Progress log

| Date | Change |
|---|---|
| 2026-11-11 | Drawing shapes: rectangle, rounded rectangle and ellipse regions with fill and outline (palette or RGBA colours), draw order commands, Shape window, file format 3. |
| 2026-11-10 | Options window: theme, interface size, grid spacing, grid and snap, defaults for new worksheets (unit system and page), recovery interval, clear recent list; settings kept between sessions. |
| 2026-11-10 | Phase 2: Given / Find solve block (Stage 2), as a form inside a math region. |
| 2026-11-10 | Phase 6 complete: typeset editing of programs; text values and `on error e`. |
| 2026-11-10 | Phase 6: recursion; element assignment. |
| 2026-11-10 | Phase 6: programs (parser, evaluator, typesetting, editor). |
| 2026-11-10 | Phase 6: manual calculation mode; default arguments. |
| 2026-11-10 | Phase 6 started: design note `PROGRAMMING.md`; binary, octal and hexadecimal literals. |
| 2026-11-08 | Phase 5: rich text. |
| 2026-11-08 | Phase 5: templates. |
| 2026-11-08 | Phase 5: page backgrounds and tints; `≡` accepted as `:=`. |
| 2026-11-08 | Phase 5: design note for backgrounds, templates, rich text, content protection and global definitions (`DOCUMENT_FEATURES.md`); no code. |
| 2026-11-08 | Phase 5: headers and footers. |
| 2026-11-08 | Phase 5: region borders. |
| 2026-11-08 | Phase 4: Picture region (PNG import with limits, embedded, drawn on the page and in exports, undoable). |
| 2026-11-08 | Phase 4: CSV import and export for tables (file dialogs behind the `Files` trait; bad files change nothing). |
| 2026-11-08 | Phase 4: Table region (format 2, placeholder migration, evaluation as a matrix definition, painting, table window); CSV decimal-mark option. |
| 2026-11-08 | Phase 4 started: design note `DATA_IO.md`; `opencalc-core::csv` (reader, writer, delimiter detection, number reading, header split, formula-safe output). |
| 2026-11-08 | Roadmap created from the audit of `private/mathcad_features.md`. |
| 2026-11-08 | Phase 0 done: `TECH_DEBT.md` refreshed, `HelpGuide` left paused on purpose. |
| 2026-11-08 | Phase 3.4: polar plots; the plot window and the legend split into smaller modules. |
| 2026-11-08 | Phase 3.3: interactive plot view window (hover readout, zoom, pan, reset); the painter takes a `PlotCtx` and a `View` and reports its `Geometry`. |
| 2026-11-08 | Phase 3.2: parametric plots (a series' own x values) and a second y axis; scales, legend and frame split into `canvas/plot/` modules. |
| 2026-11-08 | Phase 3.1: plot options and series styles (file format 2): axis limits, grid, legend, log axes, six series types, markers, colours, legend names; rendering tests on real pixels. |
| 2026-11-08 | Phase 2.2: `odesolve` (adaptive RK45, mixed-unit states, table output), `col` / `row`; corpus `odes.calc`, `tests/ode.rs`. |
| 2026-11-08 | Phase 1.1: per-element dimensions in matrices (TD-28 resolved); solver unknowns may now differ in dimension; corpus `mixed_units.calc`. |
| 2026-11-08 | Phase 2.2: solve blocks Stage 1: `find`, `minerr`, `minimize`, `maximize`; corpus `solve.calc`, `tests/solve.rs`; `EvalError::OutOfBudget`. |
| 2026-11-08 | Phase 2.2: solve-block audit and design (`SOLVE_BLOCKS.md`), no code. |
| 2026-11-08 | Phase 2.2 part: `root` (bracketed and from a guess) and `polyroots`; corpus `roots.calc`. |
| 2026-11-08 | Phase 2.1: step budget and cancellation; `integrate`, `derivative`, `limit`, `sum_over`, `product_over`; corpus `calculus.calc`, integration tests `tests/calculus.rs`. |
| 2026-11-08 | Phase 1.1: compound units shown by name or as numerator over denominator in the chosen unit system; corpus `compound_units.calc`. |
| 2026-11-08 | Phase 1.1: angle units `rad` and `deg`, corpus `angles.calc`. |
| 2026-11-08 | Phase 1.1: temperature units (`degC`, `degF`, `deltaC`, `deltaF`, `degR`), corpus `temperatures.calc`. |
| 2026-11-08 | Phase 1.2 part 2: matrix inverse/powers/`identity`/`tr`/`rank`/`cross`/`augment`/`stack`, element-wise functions on matrices, `median`/`mode`/`var`/`sort`/`cumsum`/`cvar`/`corr`, `factorial`/`combin`/`permut`/`isprime`; corpus `matrices_and_lists.calc`. |
| 2026-11-08 | Phase 1.2 part: inverse and hyperbolic trig, `log`, rounding, `mod`, `sign`, `gcd`, `lcm` (new `functions/elementary.rs`, `functions::all()`), corpus worksheet `elementary_functions.calc`. |
| 2026-11-08 | Phase 1.1 part: about 45 built-in units added with display tables for energy, power, voltage, resistance, frequency and volume. |
