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
