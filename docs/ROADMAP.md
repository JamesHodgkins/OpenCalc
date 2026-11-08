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
- [ ] Per-element dimensions in vectors and matrices, so `(1 m, 2 s)` works (TD-28). Larger (M), may need a format bump.

**1.2 Functions**
- [x] Inverse and hyperbolic trig: `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh`. Real inside the real domain, complex outside. Open points: `atan2(x, y)` argument order follows Mathcad as I recall it and should be checked against PTC's help; the sign of the imaginary part on a branch cut (`asin(2)`) is whatever `num-complex` gives and is not asserted by any test.
- [x] Logs and rounding: `log(x)` (base 10), `log(x, b)`, `floor`, `ceil`, `round(x[, n])`, `trunc`, `sign`, `mod` (sign of the divisor), `gcd`, `lcm`. No separate `log10`/`log2`. Rounding needs a dimensionless argument (rounding a length would depend on the display unit). `ln` was already there.
- [x] Element-wise functions on matrices (TD-08): every one-number function except `abs` (which is not element by element for a matrix). Done 2026-11-08.
- [x] Matrix: inverse and integer powers as `M ^ n` (including `M ^ -1`), `identity`, `tr`, `rank`, `cross`, `augment`, `stack` (`rows`/`cols` existed). Singular and rank tolerance: largest element x size x machine epsilon. Done 2026-11-08.
- [ ] `submatrix`: needs the evaluator's `ORIGIN` so it does not silently differ when a worksheet sets `ORIGIN := 1`. Do with the Phase 2 evaluator context work.
- [x] Stats: `median`, `mode` (smallest on a tie), `var` and `var_sample`, `sort`, `reverse`, `cumsum`, `cvar` (population covariance), `corr`. Done 2026-11-08. Not done: `range`, row vectors in `sort`/`reverse`/`cumsum` (column vectors and lists only).
- [x] Number theory and combinatorics: `factorial` (to 170), `combin`, `permut`, `isprime` (exact for every whole number to 2^53). Done 2026-11-08. The `n!` postfix operator is a parser change and is not done.
- [ ] Text and boolean values (TD-08), needed by later string functions and programming.

**Exit:** each function has a unit test with a hand-checked value and a dimension-handling test; corpus gains a worksheet per group.

## Phase 2: Calculus and solving (L)

Depends on Phase 1 (inverse, boolean values). Builds the cancellation token that TD-20 is waiting for.

**2.1 Numeric calculus**
- [ ] Cancellation (`AtomicBool`) and an iteration/time budget in the evaluator (TD-20). **Do first.**
- [ ] Definite integral, derivative, limit, summation and product operators (parser, typesetting, evaluator).
- [ ] Units through integrals and derivatives (dimension of the result follows the operator).

**2.2 Solvers**
- [ ] `root(f, x, a, b)` and `polyroots`.
- [ ] Solve blocks: `Given`, constraints, `Find`, `Minerr` (new region kind or grouped Math regions; this is the wide exhaustive-match change, so audit first).
- [ ] `minimize` / `maximize`, with a choice of algorithm.
- [ ] `odesolve` (fixed then adaptive step).
- [ ] Non-convergence diagnostics and error badges; MultiStart.
- [ ] Linear-algebra extras: eigenvalues/eigenvectors, LU/QR/Cholesky/SVD.

**Exit:** solver corpus worksheets with known answers; non-convergence is an error, never a wrong number; the cancel path is tested.

**Deferred beyond this phase:** `pdesolve`, KNITRO-class constrained optimisation, symbolic solutions.

## Phase 3: Plots (L)

Extends `PlotSpec` (format bump with migration). TD-31 records the current plot decisions.

- [ ] Plot options: axis bounds, grid, legend, markers, line styles/colours, log axes.
- [ ] `f(x)`-style function plotting and parametric plots.
- [ ] Mixed units on an axis (per-series dimensions).
- [ ] Plot types: bar/column, stem, scatter, polar, error bars, box.
- [ ] Contour plot, then 3D (surface, curve, scatter) with rotate/pan/zoom.
- [ ] Interactive plot view with hover readout, pan and zoom (revisit `egui_plot`, TD-31).
- [ ] Export of plots in PDF/PNG covers every new type.

## Phase 4: Data I/O (M to L)

Fills the Table and Picture placeholder regions.

- [ ] Table region (editable grid, evaluates to a matrix).
- [ ] CSV/text read and write functions (`READPRN`, `WRITEPRN`-style, names to be chosen clean-room).
- [ ] Picture region (PNG/JPG import, embedded in the `.ocx`).
- [ ] CSV import and export in the Input/Output tab.
- [ ] Excel (`.xlsx`) read and write; evaluate a crate against the dependency policy first.
- [ ] Image data to matrix and back.

## Phase 5: Document features (M)

- [ ] Headers and footers (first page and following pages), with page-number fields; in PDF export.
- [ ] Page background styles.
- [ ] Rich text in text regions (needs bold/italic font files, TD-24).
- [ ] Region borders.
- [ ] Content protection: password-protected regions and hidden content (needs a security design before code).
- [ ] Templates (`.ocxt`, mirroring the worksheet format).
- [ ] Global definitions `≡` (needs worksheet-qualified region IDs, TD-23).

## Phase 6: Programming (L)

Needs Phase 1 (booleans, text) and Phase 2 (cancellation).

- [ ] Multi-line programs with local variables.
- [ ] `for`, `while`, `break`, `continue`, `return`.
- [ ] `try` / `on error`.
- [ ] Recursion, behind a depth and time guard (reverses a TD-30 decision; record it).
- [ ] Default arguments; functions that return several values.
- [ ] Manual calculation mode (distinct from the current auto-calculate toggle).
- [ ] Binary, octal and hexadecimal numbers.

## Phase 7: Controls and scripting (L, optional)

- [ ] Scripted controls: combo box, list box, slider, check box, radio button, button, text box.
- [ ] Scripting bridge (Python first); needs a sandbox and a security review before code.

## Out of scope for now

Symbolic engine (`->`), `pdesolve`, DOE, signal/image-processing library, Fourier and
digital filtering, OLE, Creo integration, whole custom unit systems, function-analysis
functions. Revisit after Phase 4; each needs its own design note first. Fourier and
statistics-library items can move up if the Phase 1 function work goes quickly.

## Progress log

| Date | Change |
|---|---|
| 2026-11-08 | Roadmap created from the audit of `private/mathcad_features.md`. |
| 2026-11-08 | Phase 0 done: `TECH_DEBT.md` refreshed, `HelpGuide` left paused on purpose. |
| 2026-11-08 | Phase 1.1: compound units shown by name or as numerator over denominator in the chosen unit system; corpus `compound_units.calc`. |
| 2026-11-08 | Phase 1.1: angle units `rad` and `deg`, corpus `angles.calc`. |
| 2026-11-08 | Phase 1.1: temperature units (`degC`, `degF`, `deltaC`, `deltaF`, `degR`), corpus `temperatures.calc`. |
| 2026-11-08 | Phase 1.2 part 2: matrix inverse/powers/`identity`/`tr`/`rank`/`cross`/`augment`/`stack`, element-wise functions on matrices, `median`/`mode`/`var`/`sort`/`cumsum`/`cvar`/`corr`, `factorial`/`combin`/`permut`/`isprime`; corpus `matrices_and_lists.calc`. |
| 2026-11-08 | Phase 1.2 part: inverse and hyperbolic trig, `log`, rounding, `mod`, `sign`, `gcd`, `lcm` (new `functions/elementary.rs`, `functions::all()`), corpus worksheet `elementary_functions.calc`. |
| 2026-11-08 | Phase 1.1 part: about 45 built-in units added with display tables for energy, power, voltage, resistance, frequency and volume. |
