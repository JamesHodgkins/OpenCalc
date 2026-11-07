# The OpenCalc file format (`.ocx`), version 1

This describes what is in a worksheet file, so other tools can read or write one.
The code that does it is `opencalc-core/src/persist*`. The examples below are
loaded by a test (`tests/file_format_doc.rs`), so they cannot drift from the code.

## 1. The container

A `.ocx` file is an ordinary ZIP archive (deflate) with these entries:

| Entry | Content |
|---|---|
| `manifest.json` | Format version and the program that wrote the file. At most 64 KB. |
| `worksheet.json` | The worksheet. UTF-8 JSON. At most 64 MB. |

Other entries are ignored, so later versions can add parts (pictures and data
snapshots will live under `assets/`). Entry names must not be absolute, contain
`..` or backslashes, or repeat. An archive may hold at most 8 entries. Unzip it with
any tool to read it; nothing is encrypted or compressed in a special way.

Files written by OpenCalc are deterministic: the same worksheet gives the same
bytes (fixed entry order and times, pretty-printed JSON), so they diff cleanly and
suit version control. A reader must not rely on that.

## 2. The manifest

```json manifest
{"format_version":1,"app":"opencalc 0.0.1"}
```

- `format_version` (whole number, at least 1) is the version of `worksheet.json`.
  A reader that only understands versions up to N must refuse a file with a larger
  number rather than guess at it.
- `app` names the program that wrote the file. Informational.

## 3. The worksheet

```json worksheet
{
  "title": "Axial stress",
  "regions": [
    {
      "id": 1,
      "pos": { "x": 40.0, "y": 20.0 },
      "size": { "w": 160.0, "h": 32.0 },
      "kind": { "Math": { "source": "F := 10 kN" } }
    },
    {
      "id": 2,
      "pos": { "x": 40.0, "y": 60.0 },
      "size": { "w": 240.0, "h": 40.0 },
      "kind": { "Text": { "text": "The load on the bar" } }
    }
  ],
  "next_id": 3
}
```

| Field | Meaning |
|---|---|
| `title` | The worksheet's name. |
| `regions` | The things on the page, **back to front**. This order is also creation order, and it decides which of several definitions of one name wins (the last). |
| `next_id` | The next region number to hand out. Numbers are never reused. Must be above every `id`; a reader repairs it if not. |
| `unit_system` | `"Si"` (default), `"Uscs"` or `"Cgs"`. Which units results are shown in. Display only: values are always computed in SI. |
| `custom_units` | Units the worksheet defines (default none). See section 5. |
| `page` | Paper, orientation and margin (default A4, portrait, 20 mm). See section 6. |

A region has an `id` (unique whole number), a `pos` (top-left corner) and a `size`,
both in **document units of 1/96 inch**, and a `kind`. A text region may also have
a `style`, `{ "bold": true, "italic": false }` (both default to false; omitted
when plain). Any region may be `"locked": true` (default false, omitted when
unlocked): it cannot be moved, edited or deleted until unlocked. Any region may
also have a `"label"`, a short name to find it by (the program sets at most 60
characters; omitted when empty):

| `kind` | JSON | Notes |
|---|---|---|
| Math | `{ "Math": { "source": "…", "format": { "notation": "scientific", "decimals": 4, "unit": "mm" } } }` | The expression, as text. `format` is how the result is shown and may be omitted: `notation` is `"general"` (default), `"scientific"` or `"engineering"`; `decimals` is 0 to 15, or omitted for up to three with trailing zeros dropped. `unit` is `"m"`, `"mm"`, `"in"`, `"N"` or `"kN"`: show the result in that unit when it is a length or a force of the same kind, otherwise ignored. Display only. A count above 15 is read as 15. |
| Text | `{ "Text": { "text": "…" } }` | |
| Area | `{ "Area": { "title": "…" } }` | A titled box. |
| Plot | `{ "Plot": { "title": "", "x_label": "", "y_label": "", "x": { "source": "…" }, "series": [ { "source": "…" } ] } }` | At most 8 series, at least 1. Titles and labels may be omitted. |
| Table | `"Table"` | Not built yet; reserved. |
| Picture | `"Picture"` | Not built yet; reserved. |

**Math is saved as the text typed, not as a parsed tree.** The program parses it
again on opening. This keeps the file independent of the program's internals: a
source that this version cannot read opens as an error in that region, and is
never lost.

Unknown fields are ignored on reading. A reader that writes a file back out should
keep what it does not understand where it can, but is not required to.

## 4. A fuller example

```json worksheet
{
  "title": "Everything",
  "regions": [
    {
      "id": 1,
      "pos": { "x": 40.0, "y": 20.0 },
      "size": { "w": 160.0, "h": 32.0 },
      "kind": { "Math": { "source": "a := 2 hand" } }
    },
    {
      "id": 2,
      "pos": { "x": 40.0, "y": 80.0 },
      "size": { "w": 320.0, "h": 200.0 },
      "kind": { "Area": { "title": "Loads" } }
    },
    {
      "id": 5,
      "pos": { "x": 400.0, "y": 80.0 },
      "size": { "w": 260.0, "h": 170.0 },
      "kind": {
        "Plot": {
          "title": "Square",
          "x_label": "x",
          "y_label": "y",
          "x": { "source": "w := 1, 2..5" },
          "series": [ { "source": "w^2" } ]
        }
      }
    },
    { "id": 6, "pos": { "x": 0.0, "y": 300.0 }, "size": { "w": 260.0, "h": 170.0 }, "kind": "Table" }
  ],
  "next_id": 7,
  "unit_system": "Uscs",
  "custom_units": [
    {
      "symbol": "hand",
      "dimension": { "length": 1, "mass": 0, "time": 0, "current": 0, "temperature": 0, "amount": 0, "luminosity": 0 },
      "to_si_scale": 0.1016,
      "definition": "4 in"
    }
  ],
  "page": { "paper": "Letter", "landscape": true, "margin_mm": 12.5 }
}
```

## 5. Custom units

Each entry has a `symbol`, a `dimension` (whole-number exponents of length, mass,
time, current, temperature, amount and luminosity) and a `to_si_scale`: a value in
the unit times the scale is the value in SI. An optional `definition` holds what
the user typed to define it (`4 in`), for showing and editing; the scale and
dimension are what count, and a file may leave `definition` out. The symbol must start with a letter or
underscore, then letters, digits or underscores (at most 32 characters); it must
not be a built-in unit, a function or a constant, and must not repeat. The scale
must be finite and greater than zero. A file that breaks these rules is refused,
and the message names the unit. At most 1,000 units.

## 6. The page

`paper` is `"A4"` or `"Letter"`; `landscape` is true or false; `margin_mm` is the
same margin on all four sides, from 0 to 60. Each may be omitted. A margin outside
the range is clamped on reading.

## 7. Limits and repairs

A reader treats a file as untrusted. OpenCalc enforces:

| Limit | Value |
|---|---|
| File size | 128 MB |
| Archive entries | 8 |
| `worksheet.json` / `manifest.json` inflated | 64 MB / 64 KB |
| Regions | 50,000 |
| Text in any one field | 65,536 characters |
| Plot series | 8 |
| Custom units | 1,000 |
| Position | within ±1,000,000 of the origin; non-numbers become 0 |
| Size | greater than 0 and at most 100,000; otherwise the region type's default |

What can be repaired safely is, and is reported once when the file is opened:
a `next_id` that is too low, an unusable position or size, a plot with no series
(one blank series is added), a margin out of range. What cannot is refused with a
reason: a damaged archive, a missing part, a format newer than the program
understands, duplicate region ids, a bad custom unit, or a limit exceeded.

## 8. Versions

| Version | Program | Change |
|---|---|---|
| 1 | OpenCalc 0.0.1 | First version. |

When the format changes in a way older programs cannot read, `format_version` goes
up and a migration is added that turns version N into N + 1. A new program opens
every older version; committed example files of each version (`tests/fixtures/`) are
loaded by the tests for as long as the program exists.

## 9. Recovery files

While a worksheet has unsaved changes, OpenCalc writes them to `<name>.ocx.recovery`
beside the file (or in its own folder for a worksheet that was never saved). A
recovery file has exactly the format above. It is deleted when the worksheet is
saved or closed; if one is found at the next start, the program offers to restore it.
