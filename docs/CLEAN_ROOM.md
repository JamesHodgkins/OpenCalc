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
| 2026-11-06 | OpenDraft desktop app (the owner's own project, GPL-3.0-or-later): `crates/od-ui` theme, ribbon, menus, document tabs; `ui_plan.md`; `CLAUDE.md` | Visual style tokens, ribbon structure and sizes, window layout, engineering conventions | Read for design only. No code copied (decided: SPEC section 4.11, option 1). |

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
from scratch.
