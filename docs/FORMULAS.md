# Formulas

A formula computes numbers from other numbers in the same table. You type it after an `=`,
either for a whole column (the formula bar above a column header) or for a single cell.

## Two places a formula can live

**A column formula** fills every row of a column. `= log10(A)` in column B means "for each
row, B is the base-10 log of A in that row". One formula, a million rows, no dragging.

**A cell formula** fills one cell, the way a spreadsheet does. Type `=A3 * 2` into cell B3.
Fill-down (Ctrl-D) copies it to the rows below and moves the row numbers along: B4 gets
`=A4 * 2`. A column holds either a column formula or cell formulas, not both.

Computed cells show a small corner mark. Change the data and every formula that depends on
it is recomputed the next time you look. A formula that would depend on itself, directly or
through other columns, is refused when you type it, with the loop spelled out:
`Circular reference: C → D → C`.

## Naming columns

| You write | It means |
|---|---|
| `A`, `B`, … `Z`, `AA` | data set A, B, … (its first subcolumn) |
| `A.2` | data set A, subcolumn 2 (replicate 2, or the SD in a mean/SD/N data set) |
| `X` | the X column of an XY or survival table |
| `` `Dose (nM)` `` | the data set (or X column) with that header, in backticks |
| `` `Dose (nM)`.3 `` | its third subcolumn |
| `A3` | one cell: data set A, row 3 (rows count from 1) |
| `A[3]` | the same cell |
| `A[2:5]` | rows 2 to 5 of A, both ends included |
| `A[:5]`, `A[2:]` | from the first row to row 5; from row 2 to the end |
| `[1, 2, 3]` | a list of numbers you type yourself |

The letter `X` always means the X column, so the twenty-fourth data set is reached by its
name in backticks.

## Values

A formula works with numbers, text (`"mg/dL"`), true/false, and *missing*. An empty cell
is missing. **An excluded cell is missing too**: it stays in the table, struck through, but
no formula or analysis uses it. `NA` (or `na()`) writes missing on purpose.

Missing spreads: `A + 1` is missing wherever A is missing. Summaries such as `mean` skip
missing values instead, because that is almost always what you want from "the mean of A".

A problem in one row does not stop the column. `sqrt(A)` on a row where A is −4 shows an
error in that cell (hover to read "sqrt: negative number") and computes every other row.
Text in a numeric column ("<LOD") is kept and shown; arithmetic on it gives an error in
that row, never a silent zero.

## Arithmetic and logic

From tightest to loosest binding:

| Operators | Meaning | Example |
|---|---|---|
| `A[…]` | pick rows | `A[1:3]` |
| `^` | power (right to left: `2^3^2` is `2^9`) | `B ^ 2` |
| `-x`, `+x` | sign (`-2^2` is −4) | `-A` |
| `*`, `/`, `%` | multiply, divide, remainder (floored, like MOD) | `A % 2` |
| `+`, `-` | add, subtract | `A - B` |
| `==` (or `=`), `!=` (or `<>`), `<`, `<=`, `>`, `>=` | compare; gives true/false | `A > 0` |
| `not x`, `!x` | not | `not isna(A)` |
| `&&`, `and` | both | `A > 0 && B > 0` |
| `\|\|`, `or` | either | `A < 0 or B < 0` |

True counts as 1 and false as 0 when stored in a column, which is how you make an event
code: `= if(A > 30, 1, 0)`. Logic with a missing value follows common sense:
`false && NA` is false, `true || NA` is true, `true && NA` is missing.

A single number combines with a whole column row by row: `A * 2`, `A - mean(A)`. Two
columns combine row by row. A list of a different length than the table cannot combine
with a column; that is an error, not a guess.

## Row-by-row functions

| Function | What it gives | Example |
|---|---|---|
| `abs(x)` | absolute value | `abs(A - B)` |
| `sqrt(x)` | square root (error below 0) | `sqrt(A)` |
| `exp(x)` | e to the power x | `exp(-0.693 * X)` |
| `ln(x)` | natural log (error at or below 0) | `ln(A)` |
| `log(x)` | natural log, same as `ln` (not base 10, unlike Excel) | `log(A)` |
| `log(x, b)` | log in base b | `log(A, 2)` |
| `log10(x)` | base-10 log | `log10(A)` |
| `log2(x)` | base-2 log | `log2(B / A)` |
| `pow(x, y)` | x to the power y | `pow(10, A)` |
| `sin`, `cos`, `tan` | trigonometry, radians | `sin(X * pi() / 180)` |
| `asin`, `acos`, `atan` | inverse trigonometry, radians | `atan(A)` |
| `sinh`, `cosh`, `tanh` | hyperbolic functions | `tanh(A)` |
| `floor(x)`, `ceil(x)` | round down, round up | `floor(A)` |
| `round(x)`, `round(x, n)` | round to n decimals, halves away from zero (2.675 → 2.68) | `round(A, 2)` |
| `trunc(x)` | drop the fraction | `trunc(A)` |
| `sign(x)` | −1, 0 or 1 | `sign(B - A)` |
| `if(c, a, b)` | a where c is true, b where false, missing where c is missing | `if(A > 0, A, NA)` |
| `isna(x)` | true where x is missing (or excluded) | `if(isna(A), 0, A)` |
| `na()` | a missing value | `if(A < 0, na(), A)` |
| `coalesce(a, b, …)` | the first argument that is not missing | `coalesce(A, B, 0)` |
| `clamp(x, lo, hi)` | x limited to the range lo to hi | `clamp(A, 0, 100)` |
| `pct(x, ref)` | 100 × x / ref | `pct(A, A1)` |
| `normalize(x, lo, hi)` | maps lo to 0 and hi to 1 | `normalize(A, A1, max(A)) * 100` |
| `convert(x, "from", "to")` | unit conversion (see below) | `convert(A, "nM", "µM")` |
| `convert(x, "from", "to", "substance")` | mass ↔ molar conversion | `convert(A, "mg/dL", "mmol/L", "glucose")` |
| `row()` | the row number, from 1 | `if(row() <= 3, NA, A)` |
| `pi()` | π | `2 * pi() * X` |

## Summaries (one number from many)

These skip missing, excluded and text cells. Give them a column, a slice, a list, or
several of these at once: `max(A, B)` is the largest value in A and B together. (For a
row-by-row limit use `clamp`, not `min`/`max`.)

| Function | What it gives | Example |
|---|---|---|
| `mean(x)` | arithmetic mean | `A - mean(A[1:3])` |
| `median(x)` | middle value | `A / median(A)` |
| `sd(x)` | sample standard deviation (n − 1) | `(A - mean(A)) / sd(A)` |
| `sem(x)` | standard error of the mean, sd/√n | `sem(A)` |
| `var(x)` | sample variance (n − 1) | `var(A)` |
| `sum(x)` | total | `A / sum(A)` |
| `count(x)` | how many numbers | `count(A)` |
| `min(x)`, `max(x)` | smallest, largest | `(A - min(A)) / (max(A) - min(A))` |
| `range(x)` | max − min | `range(A)` |
| `quantile(x, q)` | the q-quantile, q from 0 to 1, by linear interpolation (Excel's PERCENTILE.INC) | `quantile(A, 0.9)` |
| `geomean(x)` | geometric mean (all values must be positive) | `A / geomean(A)` |
| `cv(x)` | coefficient of variation, in percent: 100 × sd / mean | `cv(A)` |
| `first(x)`, `last(x)` | first and last non-missing value | `A - first(A)` |

The descriptive-statistics analysis computes quartiles by the (n + 1)p rule, as Prism
does; `quantile` uses the spreadsheet rule. For small samples the two can differ slightly.

## Series (whole-column operations that give a column back)

| Function | What it gives | Example |
|---|---|---|
| `lag(x)`, `lag(x, k)` | the value k rows above (missing for the first k rows) | `A - lag(A)` |
| `lead(x)`, `lead(x, k)` | the value k rows below | `lead(A) - A` |
| `diff(x)` | x minus the row above | `diff(X)` |
| `cumsum(x)` | running total; a missing row stays missing and is skipped in the total | `cumsum(A)` |
| `rank(x)` | rank from smallest (1), ties share the average rank | `rank(A)` |
| `zscore(x)` | (x − mean) / sd | `zscore(A)` |
| `normalize(x)` | (x − min) / (max − min) | `normalize(A) * 100` |
| `baseline(x, r1, r2, …)` | x minus the mean of x at the listed rows | `baseline(A, 1, 2, 3)` |
| `seq(a, b)`, `seq(a, b, step)` | the list a, a + step, …, b | `baseline(A, seq(1, 5))` |

## Units

`convert(x, "from", "to")` knows these units. Metric prefixes work on every base unit:
k (kilo), d (deci), c (centi), m (milli), µ or u or mc (micro), n (nano), p (pico),
f (femto).

| Quantity | Units |
|---|---|
| mass | g, kg, mg, µg, ng, pg |
| volume | L (or l), dL, mL, µL, nL, cc |
| amount | mol, mmol, µmol, nmol, pmol |
| concentration | M, mM, µM, nM, pM; or any mass or amount over a volume: mg/dL, g/L, mmol/L, µg/mL |
| time | s, ms, µs, min, h, d, wk |
| length | m, km, cm, mm, µm, nm |
| pressure | Pa, kPa, hPa, mmHg, atm, bar |
| temperature | C, F, K (or °C, °F) |

Mass and molar concentrations convert through a substance's molar mass, so you must name
the substance; Dioptra will not guess:

| Substance | Conversion |
|---|---|
| `"glucose"` | mg/dL ÷ 18.016 = mmol/L |
| `"cholesterol"` | mg/dL ÷ 38.67 = mmol/L |
| `"triglycerides"` | mg/dL ÷ 88.57 = mmol/L |
| `"creatinine"` | mg/dL × 88.42 = µmol/L |
| `"urea"` | molar mass 60.06 g/mol |

## Where formulas are also used

`Filter rows` takes a formula that is true for the rows to keep: `A > 0 && !isna(B)`.

## Differences from a spreadsheet

- `log(x)` is the natural log. Use `log10(x)` for base 10.
- Slices are inclusive and count from 1: `A[1:3]` is three rows.
- `%` is the remainder, not "per cent".
- There are no `$` absolute references; a column formula already means "this column, every
  row", and fill-down moves only single-cell references such as `A3`.
- Function names are case-insensitive (`MEAN(A)` works); column letters are capitals.
