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
- [ ] Angle units `deg` and `rad`: built-in units may not be dimensionless today (a unit-table test enforces it), so this needs a design decision first.
- [x] More US customary and length/mass/time units (`yd`, `mi`, `oz`, `gal`, `psf`, `um`, `nm`, `mg`, `tonne`, `ms`). `ton` (short or metric?) left out on purpose; `tonne` is the metric one. Done 2026-11-08.
- [ ] Note: new unit symbols (`A`, `C`, `K`, `L`, `V`, `W`, `J`) take over `5 A`-style implicit multiplication from variables of those names, and a saved custom unit with one of these names would now be rejected. No released files exist yet; revisit if that changes.
- [ ] Compound unit symbols in the parser (`kN*m`, `m/s`) (TD-09).
- [ ] Temperature units `degC`, `degF` with a decided offset policy (TD-09). Needs a written decision first.
- [ ] Per-element dimensions in vectors and matrices, so `(1 m, 2 s)` works (TD-28). Larger (M), may need a format bump.

**1.2 Functions**
- [ ] Inverse and hyperbolic trig: `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh` and inverses.
- [ ] Logs and rounding: `log`, `log10`, `log2`, `floor`, `ceil`, `round`, `trunc`, `sign`, `mod`, `gcd`, `lcm`.
- [ ] Element-wise functions on matrices (TD-08).
- [ ] Matrix: inverse, integer powers, `identity`, `trace`, `rank`, `cross`, `augment`/`stack`, `rows`/`cols`, `submatrix`.
- [ ] Stats: `median`, `var`, `mode`, `range`, `sort`, `reverse`, `cumsum`, `corr`, `cov`.
- [ ] Number theory and combinatorics: `factorial`, `combin`, `permut`, `isprime`.
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
| 2026-11-08 | Phase 1.1 part: about 45 built-in units added with display tables for energy, power, voltage, resistance, frequency and volume. |
