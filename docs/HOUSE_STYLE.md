# OpenCalc House Style

Reusable visual and interaction specification for companion desktop apps. This document captures the familiar OpenCalc feel without prescribing another app's subject matter, data model, or exact screen layout.

## 1. Design intent

OpenCalc should feel like a focused, capable desktop tool: compact, calm, keyboard-friendly, and built for sustained work. The interface uses a consistent dark shell, restrained colour, clear elevation, and predictable command placement. Content remains the visual focus. A new app may have different workflows and content, but should retain these shared signals.

### Keep consistent across apps

- Dark, neutral surfaces with four ordered elevation tones.
- Inter typography, Lucide-style monochrome outline icons, and the same spacing/radius/control metrics.
- A compact desktop shell with an application menu, quick actions, task tabs/tool groups, a central work surface, optional contextual panels, and a quiet status bar.
- Flat controls at rest; elevation and outline appear on hover, press, selection, and focus.
- A single restrained brand accent, separate from warning and error colours.
- Keyboard access, visible focus, consistent menus, and command-driven actions.

### Adapt to the app

Choose task-specific navigation labels, work surface, panels, status information, and command groups. The second app's content should shape its layout; do not force a worksheet, ribbon, dock, or document tab if the workflow does not benefit from it. Keep the hierarchy and interaction language familiar even when the components differ.

## 2. Shell and hierarchy

Use the following as a reference hierarchy, not a mandatory wireframe:

1. **Application row:** app/menu button at left, compact quick-access actions beside it, current workspace or document title centered or clearly anchored.
2. **Task navigation:** a slim row of plain-text tabs or equivalent primary task navigation. Active navigation visually joins the tool area beneath it.
3. **Tools:** grouped actions on a raised surface, ordered by task and separated by subtle one-pixel dividers. Keep the same control rhythm across groups.
4. **Workspace tabs (when multiple items can be open):** a separate, low-profile row immediately above the work surface, with a clear active state and close/new actions where appropriate.
5. **Main work surface:** largest area, visually quiet, and specific to the app's job.
6. **Context panels (when useful):** optional panels sit beside the workspace on a surface tone. They should support, not compete with, the main task.
7. **Status row:** low-emphasis operational state and view controls, separated by a thin divider.

Keep application-level actions easy to find and workspace-level actions close to the work. Prefer a small number of stable top-level groups over deep, changing navigation. Tool groups should be data-driven from a central command list so labels, enabled state, shortcuts, and menus agree.

## 3. Colour system

Use semantic theme tokens instead of literal colours in individual controls. All shell backgrounds use one of four tones; do not introduce extra surface colours for one-off panels.

### Dark neutral palette

| Token | Value | Purpose |
|---|---|---|
| `base` | `#252525` | Window shell, title/status rows, tab rows, sunken wells |
| `surface` | `#323232` | Panels, popups, secondary surfaces |
| `raised` | `#3E3E3E` | Tool areas, cards, resting buttons, active navigation |
| `raised_high` | `#4C4C4C` | Hover on raised items |
| `border` | `#474747` | Quiet outlines and separators |
| `border_strong` | `#6A6A6A` | Pressed/strong outlines |
| `pressed` | `#585858` | Pressed neutral control fill |

A blue-slate alternate palette is available for OpenCalc (`base #222933`, `surface #313946`, `raised #3B4453`, `raised_high #4F5A6E`, `border #4A5466`, `border_strong #6B778C`, `pressed #5D6980`). If a companion app offers palette choice, retain the same token roles and luminance order.

### Shared accent and semantic colours

| Token | Value | Purpose |
|---|---|---|
| `accent` | `#C62828` | Primary action fills and selected state |
| `accent_hover` | `#D32F2F` | Hover over accent actions |
| `accent_pressed` | `#B71C1C` | Pressed accent actions |
| `accent_text` | `#FF8A80` | Accent text and visible focus ring |
| `warning` | `#F5B942` | Warning text/icons |
| `error` | `#FF7B7B` | Error text/icons only; never a fill |
| `text` | `#DCDCDC` | Main text and icons |
| `text_weak` | `#B4B4B4` | Captions, inactive navigation, secondary labels |
| `text_disabled` | `#646464` | Disabled content |
| `text_on_accent` | `#FFFFFF` | Text on accent fills |

The brand accent is OpenCalc red and is intended to distinguish OpenCalc from its companion OpenDraft. For another app in the family, retain the accent's restrained role and interaction states; a distinct brand hue may be substituted if product identity requires it. Keep warning amber and error red semantically separate from action fills. Never communicate errors through colour alone.

Maintain readable contrast: target at least 4.5:1 for normal text, 3:1 for meaningful icons and focus indicators. Verify the chosen theme against every surface tone.

## 4. Interaction states and components

- **Resting controls:** flat `raised` fill, no outline, main text/icon colour.
- **Hover:** `raised_high` fill with a subtle `border` outline.
- **Pressed/active:** `pressed` fill and stronger outline. Accent actions use the accent pressed token.
- **Selected:** use the accent for selection/focus emphasis, with text/icons remaining legible. Do not fill large areas with the accent.
- **Keyboard focus:** 2 px accent-text ring outside the control bounds. Every custom-painted control must expose the same focus treatment.
- **Disabled:** use the disabled text/icon token and an unmistakable non-interactive state.
- **Menus/popups:** surface or raised tone, compact rows, rounded corners, subtle shadow, close on outside click or Escape.
- **Application menu:** opened from the app button (logo and "File") and by Alt+F. Commands with icons and right-aligned shortcuts on the left, application commands (Options, About, Quit) at the foot of that column, and a content pane on the right: recent documents by default, or the choices under the selected command with a one-line description each. The selected row takes the pane's tone and joins it, marked with a short accent bar.
- **Panels/windows:** shadow only for floating windows and popups; persistent shell surfaces remain flat.
- **Icons:** monochrome outline glyphs, tinted like adjacent text. Pair unfamiliar icons with labels or tooltips.
- **Feedback:** show errors/warnings with both an icon or label and semantic colour. Keep transient feedback brief and anchored near its cause.

## 5. Typography and iconography

- Bundle **Inter Regular** and **Inter Bold** for stable cross-platform appearance; retain a system/engine fallback for missing glyphs.
- Use **Lucide** outline icons from one centralized icon map. Preserve the font's license if bundled.
- Reference sizes: body 12.5 px; small/caption 11 px; tab 13 px; heading 15 px; tool label 11.5 px; tool group title 10.5 px.
- Use regular weight for routine labels and bold sparingly for hierarchy. Small section headings may use uppercase with weak text colour.
- Reserve monospace for code, identifiers, or technical input where character alignment helps; use proportional text for the rest.
- Keep icon sizing and alignment consistent within each control size; avoid mixing filled and outline icon styles.

## 6. Layout and sizing tokens

| Token | Reference |
|---|---:|
| Base spacing step | 8 px |
| Typical item gap | 4 px (half step) |
| Control height | 26 px |
| Standard corner radius | 4 px |
| Window corner radius | 6 px |
| Compact tab row | 25–26 px |
| Status row | 30 px |
| Scrollbar width | 6 px |
| Tool group height | 90 px reference (66 px large action + 16 px label row + margins) |

Use multiples of the 8 px spacing step for major layout gaps and padding; half steps are acceptable for dense control interiors. Keep controls compact but not cramped. Align labels, icons, and group edges to a consistent grid. Use rounded corners sparingly; avoid pill-shaped controls except for intentionally outlined compact toggles.

The shell is intentionally dense compared with a consumer web app. Preserve that efficient desktop feel while allowing the content area to breathe. Scale the shell gracefully for smaller window sizes by collapsing secondary controls or panels before reducing readable text sizes.

## 7. Navigation, commands, and keyboard behavior

- Every action has a stable command identity, label, enabled state, and optional shortcut. Menus, tool groups, tooltips, and shortcuts use the same command definition.
- Common, reversible actions belong in quick access. Keep task-specific actions near the relevant workspace or tool group.
- Shortcuts must not fire while a text editor or other text-input control owns focus, unless explicitly intended for that editor.
- Tab navigation reaches all interactive controls; Enter/Space activates focused controls. Arrow-key navigation may be added for tab and tool groups where it feels native.
- Escape closes the topmost popup/dialog first and cancels transient interaction states.
- Tooltips explain icon-only actions and may include the keyboard shortcut.
- Preserve predictable placement and interaction across sibling apps, even when command names differ.

## 8. Content and workspace surfaces

The work surface is product-specific. Keep it visually quieter than the shell so users can concentrate on their material. Use a clear selected/active treatment and avoid decoration that competes with the user's content. If content is document-like, give it a distinct paper or page treatment; if it is a canvas, table, timeline, or editor, use the app's natural model while retaining the same selection, focus, and status conventions.

Panels should be optional or collapsible when they are not essential to the primary task. Avoid opening many side panels by default. Persist user-selected panel and layout state only when doing so improves return-to-work continuity.

## 9. Motion and polish

Use restrained, short transitions for hover, press, menu opening, and panel changes. Avoid decorative animation. Motion should clarify state changes and remain unobtrusive during repetitive work. The application should remain responsive while content recalculates or loads; show a compact status indicator when an operation takes noticeable time.

## 10. Implementation and review checklist

- Define semantic theme tokens centrally; controls do not hard-code colours.
- Apply fonts, visuals, spacing, and control metrics from one theme setup path.
- Use stable IDs for stateful custom controls and central command IDs for actions.
- Give custom controls the same hover, press, disabled, selected, and focus behavior as native controls.
- Check keyboard navigation and shortcut behavior with text fields focused and unfocused.
- Verify contrast for text, icons, focus rings, warnings, errors, and accent-filled controls.
- Review at the default desktop size and a compact window size; ensure the workspace retains priority.
- Keep app-specific navigation and content structure appropriate to the new product while preserving the shared palette, typography, density, and interaction language.

## 11. OpenCalc implementation references

This reusable spec is derived from the current OpenCalc UI implementation and its project specification. For concrete token and behavior references, see:

- `crates/opencalc-ui/src/theme.rs` — palettes, metrics, type sizes, widget states, focus ring.
- `crates/opencalc-ui/src/fonts.rs` — bundled Inter/Lucide font setup.
- `crates/opencalc-ui/src/panels/title.rs` — application row and quick access actions.
- `crates/opencalc-ui/src/panels/status.rs` — status row behavior.
- `docs/SPEC.md`, sections 4.1, 4.6, 4.8, and 4.9 — shell, theme, tool groups, and accessibility details.
