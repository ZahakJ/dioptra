# Performance

Measured with `cargo test -p dioptra-core --release -- --ignored --nocapture bench_`
(release profile: thin LTO, opt-level 3) on a 16-core desktop. Times are wall clock for
one `Session::dispatch_json` call, so they include JSON parsing of the command, the work,
and the undo snapshot.

| Operation | Size | Time | Target |
|---|---|---|---|
| Paste (parse, detect, store) | 1,000 columns × 1,000 rows (10.8 MB of text) | 0.135 s | < 1.5 s |
| Paste (parse, detect, store) | 10 columns × 100,000 rows (10.8 MB of text) | 0.130 s | < 1.5 s |
| Read a 50-row viewport right after the paste | 30 columns × 50 rows | 0.5–1.3 ms | — |
| Column formula `= A * 2 + 1` (recompute on first read) | 1,000,000 rows | 44 ms | < 150 ms |
| Column formula `= log10(A) * 100 / (A + 1)` | 1,000,000 rows | 105 ms | — |
| Column formula `= (A - mean(A)) / sd(A)` | 1,000,000 rows | 51 ms | — |
| Column formula `= if(A > 500, A, NA)` | 1,000,000 rows | 79 ms | — |
| Column formula `= lag(A, 1) - A` | 1,000,000 rows | 44 ms | — |

How it stays fast:

- Paste parses straight into column vectors of `f64`; only text cells allocate.
- A formula is compiled once against the table: summaries (`mean(A)`) and series
  (`lag(A, 1)`) are computed whole, then a small expression tree runs once per row.
- Recompute is pull-based and per column: an edit marks downstream formula columns dirty,
  and they are recomputed when something reads them.
- `GetCells` materialises only the requested viewport.
- Undo stores the cells a command touches, or the whole previous table when a paste
  reshapes it (for a paste into an empty table that copy is nearly free).

## Graphs (dioptra-plot)

Measured with `cargo test -p dioptra-plot --release --test bench -- --ignored --nocapture`
on the same desktop; medians of 9–30 runs after a warm-up. "SVG" is the whole path from
spec and data to the SVG string (family, layout with text measurement, scene, writer).

| Operation | Size | Time | Target |
|---|---|---|---|
| Scatter → SVG | 10,000 points | 1.2–2.3 ms (381 kB) | < 30 ms |
| Scatter → SVG | 100,000 points (decimated to the 0.75 pt grid) | 6.3–6.9 ms (744 kB) | < 300 ms |
| Column graph with points and brackets → SVG | 3 groups × 12 | 0.04 ms | — |
| Volcano → SVG | 1,500 genes, labels placed | 0.24 ms | — |
| Column graph, 85 × 70 mm → PNG | 300 dpi (1004 × 827 px) | 3.8 ms | < 400 ms |
| Column graph, 85 × 70 mm → PDF | fonts embedded as subsets | 1.4 ms (16 kB) | < 300 ms |
| Scatter, 10,000 points → PNG | 300 dpi | 61 ms | — |
| Scatter, 10,000 points → PDF | | 126 ms (608 kB) | — |

How it stays fast:

- Text is measured from advance-width tables built once from the bundled fonts; no
  shaping happens during layout.
- Markers of one series share one `<g>` with the paint, so each point is one short
  `<circle>` or `<path>`; numbers are written with two decimals by integer arithmetic.
- Above 20,000 points a series keeps one point per 0.75 pt cell (a pixel at 96 dpi) of
  the plot area, which is invisible at any print size and bounds the SVG; the SVG
  carries a comment saying how many points were drawn.
- PNG and PDF parse the engine's own SVG once with usvg against a font database holding
  only the four bundled Inter files.

## Measured by `dioptra bench`

`dioptra bench` measures the whole engine through the same JSON commands the app sends,
so these are the numbers a user sees, not those of an isolated function. Run it yourself
with a release build:

```sh
cargo build -p dioptra-cli --release
./target/release/dioptra bench            # or --markdown for this table
```

Measured on 2026-10-11 on an AMD Ryzen 7 7800X3D (8 cores, 16 threads), Linux, release
build; each time is the median of 3 to 9 runs and includes parsing the command and, for
mutations, the undo snapshot.

| Operation | Size | Time (median) |
|---|---|---|
| Paste (parse, detect, store) | 1,000,000 cells, 10 × 100,000 (10.8 MB of text) | 76.2 ms |
| Export table to CSV | 1,000,000 cells | 91.7 ms |
| Export table to XLSX | 1,000,000 cells | 611 ms |
| Save the project (.dioptra) | 1,000,000 cells | 235 ms |
| Column formula = A * 2 + 1 (set, recompute on read) | 1,000,000 rows | 19.6 ms |
| Edit one cell, formula column recomputes | 1,000,000 rows | 17.4 ms |
| Scatter → SVG for the editor (with hit-test targets) | 10,000 points (381 kB) | 2.02 ms |
| Scatter → SVG file | 10,000 points (381 kB) | 1.81 ms |
| Scatter → PDF (fonts embedded) | 10,000 points (549 kB) | 146 ms |
| Scatter → PNG at 300 dpi | 10,000 points (73 kB) | 57.5 ms |

What the rows do:

- **Paste**: a fresh column table receives 10.8 MB of tab-separated text (10 columns by
  100,000 rows); the engine detects separator and decimal mark, parses, and stores every
  cell.
- **Export / save**: the same 1,000,000 cells written as CSV, as an Excel workbook, and as
  a `.dioptra` project (zip with the data, the analyses and the history).
- **Column formula**: setting `= A * 2 + 1` on a second column of 1,000,000 rows and
  reading a viewport, which recomputes the column; then editing one cell of `A`, which
  marks the formula column dirty, and reading again.
- **Scatter**: 10,000 points rendered for the graph editor (SVG plus the hit-test boxes of
  every clickable element), then exported as SVG, as PDF with the fonts embedded, and as
  PNG at 300 dpi.
