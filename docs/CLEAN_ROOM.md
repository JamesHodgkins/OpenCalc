# Clean-room log

This log records where each feature's behaviour came from. It supports the
clean-room rules in `SPEC.md` section 9.1.

## Rules (summary)
- Licence: OpenCalc is GPL-3.0-or-later.
- Reference only: commercial or freeware products may be studied for behaviour and UX.
- No copying of code, text, screenshots, icons, sample worksheets or file formats.
- No decompiling, disassembly or inspection of proprietary file internals.
- Black-box comparison of results is allowed, using original or textbook inputs.
- Code from GPL-3.0-or-later or MIT sources may be used only with its licence notice and attribution kept, and recorded here.

## Sources consulted

| Date | Source | Used for | Notes |
|---|---|---|---|
| 2026-11-05 | PTC Mathcad Prime public help (units, worksheets, regions) | Feature inventory and expected behaviour | Public documentation only |
| 2026-11-05 | PTC Mathcad Prime keyboard shortcut references (v3, v4, v11, v12) | Shortcut conventions | Public references only |
| 2026-11-05 | Public alternatives list (AlternativeTo) | Competitor survey | Summary only |
| 2026-11-06 | SMath Studio vs Mathcad comparison (vendor page) | Comparison of feature claims | Vendor-authored; reference only |
| 2026-11-06 | TriStar comparison article | Comparison of feature claims | Reference only |
| 2026-11-06 | PTC Mathcad Prime help: About the Workspace, About Worksheets and Regions, To Use KeyTips; PTC Mathcad Prime guide tables of contents (tab names) | Window parts and ribbon tab names, for `docs/UI_ALIGNMENT.md` | Public documentation only; behaviours noted, nothing copied. Panel contents of each tab were not confirmed and are marked inferred |
| 2026-11-06 | OpenDraft desktop app (the owner's own project, GPL-3.0-or-later): `crates/od-ui` theme, ribbon, menus, document tabs; `ui_plan.md`; `CLAUDE.md` | Visual style tokens, ribbon structure and sizes, window layout, engineering conventions | Read for design only. No code copied (decided: SPEC section 4.11, option 1). |
| 2026-11-08 | SMath Studio forum and plugin pages on data import (`importData`, Xlsx Import/Export plugin, Data Exchange plugin); search-result summaries, pages not opened | That CSV import is a function with decimal and separator arguments, and that Excel is a separate plugin | Behaviour only; nothing copied. Used to confirm `DATA_IO.md` choices |
| 2026-11-08 | SMath Studio forum threads and bug tracker on temperature units (search-result summaries; pages not opened) | The idea of separate delta units for temperature differences, and the trap that `-5 degC` reads as `-(5 degC)` | Behaviour only, from public forum posts of 2012 to 2019; nothing copied. The owner states Mathcad also stores temperature in kelvin (not verified from PTC documentation) |
| 2026-11-08 | PTC Community forum threads on angle units in Mathcad Prime (search-result summaries; pages not opened) | That a bare number is radians, that `deg` is an alias for pi/180, and that inverse functions give radians | User-reported behaviour from public forum posts, not PTC documentation; nothing copied |
| 2026-11-10 | PTC Mathcad Prime help: Mathcad Options and License Settings (r9 to r12), https://support.ptc.com/help/mathcad/r12.0/en/PTC_Mathcad_Help/mathcad_options_and_license_settings.html; Setting Calculation Options and Formatting the Result Display (search-result summaries); PTC Community posts on Mathcad 15 Tools > Preferences auto-save (search-result summaries) | What an engineering worksheet app puts in its application options, for the Options window | Public documentation and user posts; nothing copied. Prime's application options are few (templates, help location, integrations); calculation and result format are worksheet settings there. Mathcad 15 kept auto-save in its preferences |
| 2026-11-10 | SMath Studio forum posts on Tools > Options (decimal places, exponential threshold, fractions) and its version history (search-result summaries; one forum page opened) | Same purpose | User posts; nothing copied. SMath keeps result precision in application options, applying to every worksheet; OpenCalc did not follow that, so worksheets look the same on every computer |
| 2026-11-08 | PTC Mathcad help: system of equations functions (find, minerr), https://support.ptc.com/help/mathcad/r11.0/en/PTC_Mathcad_Help/system_of_equations_functions.html; PTC Community posts on solve-block tolerances (search-result summaries) | Solve-block rules: a guess for each unknown, scalar or vector result, Levenberg-Marquardt, minerr returning the best point reached | Public documentation and user-reported behaviour; nothing copied. Which inequality operators are allowed and what find returns on failure are not stated there, so those are repository decisions (`SOLVE_BLOCKS.md`) |

## Code provenance
No source code has been taken from any other project yet. Two third-party font
assets are bundled, both under permissive licences, with their licence files kept
beside them (`crates/opencalc-ui/assets/fonts/`):

| Asset | Source | Licence |
|---|---|---|
| `Inter-Regular.ttf` | Inter 4 (rsms/inter), via `@expo-google-fonts/inter` 0.2.3 | SIL OFL 1.1 |
| `lucide.ttf` | Lucide, via `lucide-static` 0.468.0 | ISC |

`src/icons.rs` was generated from Lucide's own `info.json` codepoint map.

2026-11-06: SPEC 4.11 decided on option 1. `opencalc-ui` is written from SPEC
section 4 and does not copy `od-ui`. OpenDraft is consulted for design only.
Algorithms (Tarjan SCC, Pratt parsing) are standard textbook techniques written
from scratch. The same holds for the Phase 2.1 numerical methods, written from
their published descriptions: adaptive Gauss-Kronrod quadrature (the G7/K15
nodes and weights are the standard published constants), Ridders' extrapolated
central differences, Richardson extrapolation, Brent's root-finding method,
Aberth-Ehrlich simultaneous root iteration, Levenberg-Marquardt least squares,
BFGS quasi-Newton minimisation with a backtracking line search, the
augmented-Lagrangian method for constraints, and the Dormand-Prince 5(4)
embedded Runge-Kutta pair (its coefficients are the standard published ones,
checked by tests that they sum as the method requires). No code from QUADPACK,
Numerical Recipes or any other library was copied.

2026-11-11: Shape regions and draw order are standard drawing features. The colour
picker's layout (a row of theme colours over lighter and darker shades, a row of
standard colours, and exact values) is a common convention in office programs; the
palette colours and shade percentages are our own choices and the code is written
from scratch. Lucide icons added for shapes and draw order come from the existing
Lucide font (see the icon note above).

2026-11-11: The ribbon was redone from scratch, organised by task (Home, Insert, Draw, Format,
Units, View, Help, and contextual tabs for a selected shape). Tab and group names are our own and
no longer follow PTC's tab names; the earlier note that they did is superseded. Contextual tabs are a
common convention in office programs; the code is written from scratch.
