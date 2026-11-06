# Test corpus

Worksheets with known, hand-checked answers. OpenCalc's results must match
them. Each case is a small file with its inputs and the expected outputs.

Rules:

1. Use textbook or original examples only. Do not add company or client calculations.
2. Expected results are worked by hand or from the textbook, not copied from another product's output.
3. Record the source of each case in the case file (`# source:` line, required; the test fails without it).

## File format (`*.calc`)

One region per line, in layout order. The engine must not care about that order,
and the test checks it by evaluating every case in reversed and shuffled orders.

| Line | Meaning |
|---|---|
| `# text` | Comment. `# source: ...` is required. |
| `@ordered` | Layout order matters (a redefinition case): skip the shuffle check. |
| `name := expr`, `expr`, ... | A region's source text. |
| `=> name = expr` | `name` must evaluate to the value of `expr` (same dimension, relative tolerance 1e-9). |
| `=> name ~1e-12 = expr` | The same with an explicit relative tolerance. |
| `!> name : kind` | `name` must fail with that kind of error. |
| `!> #3 : kind` | The same for the third region, for regions that define no name. |
| `!> warn : redefinition name` | The worksheet must warn that `name` is defined more than once. |

Error kinds: `mismatch`, `not-dimensionless`, `overflow`, `unknown-unit`,
`undefined`, `cycle`, `notfinite`, `unknown-function`, `arity`, `invalid`, `internal`.

The right-hand side of `=>` is itself evaluated by the engine, so state expected
values in coherent SI units (`Pa`, `N`, `m`, `kg`, `s`, or products of them),
where the conversion factor is 1 and the check does not depend on the unit table
under test. Cases that test the unit table itself state exact SI numbers.

Do not name a variable after a unit (`m`, `g`, `s`, `in`, ...): a defined variable
shadows the unit, which is the intended rule but makes a worksheet confusing.
