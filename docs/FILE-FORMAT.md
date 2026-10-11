# The `.dioptra` project file

A `.dioptra` file is a zip archive (deflate) of pretty-printed JSON. `unzip -l` lists it;
`unzip` extracts it; `git diff` on the extracted tree shows exactly what changed. Keys are
written in a stable order; numbers use shortest round-trip representation; empty cells are
`null`.

```
manifest.json            format version, Dioptra version, created/modified timestamps
project.json             title, p-value style, display order of objects, layouts
tables/<id>.json         one table: kind, datasets, subcolumns, columns (columnar arrays),
                         row titles, column sources and formulas, excluded cells, text cells,
                         edit log
analyses/<id>.json       analysis spec (table id, kind, options); no results
graphs/<id>.json         graph spec (source id, family, mapping, style, annotations)
results/<id>.json        cached results sheets (optional; recomputed when absent or stale)
history.jsonl            command log: one JSON object per line, newest last
```

## manifest.json

```json
{ "format": "dioptra/1", "app": "0.1.0", "created": "2026-10-11T09:00:00Z", "modified": "..." }
```

Readers must accept any `dioptra/1.x`; unknown keys are preserved on round trip where
possible and ignored otherwise.

## tables/<id>.json

```json
{
  "id": "…", "title": "Dose response", "kind": "xy",
  "x": { "title": "Dose (nM)", "values": [1, 10, 100, null], "source": {"kind": "pasted", "at": "…"} },
  "row_titles": ["", "", "", ""],
  "datasets": [
    { "name": "Control", "format": {"kind": "values", "n": 3},
      "subcolumns": [
        { "values": [0.1, 0.5, 0.9, null], "excluded": [3], "text": {"2": "n/a"},
          "source": {"kind": "formula", "expr": "log10(A)"}, "decimals": 2,
          "edits": [{"row": 1, "at": "…", "from": 0.4, "to": 0.5}] }
      ] }
  ],
  "variables": null
}
```

- `values` is a dense array the length of the table's row count; `null` means empty.
- `excluded` is a sorted list of row indices.
- `text` holds non-numeric entries by row index (kept, shown, never used in numerics).
- `source` is one of `entered`, `pasted {at}`, `imported {file, sheet, column}`,
  `formula {expr}`, `derived {table, op}`.
- `edits` is the sparse per-cell edit log that, with `source`, gives each cell's provenance.
- For `variables` tables, each dataset has a single subcolumn and a `type`.
- Derived tables carry `derived_from: {table, op}` at the top level.

## history.jsonl

Each line is `{ "at": "…", "command": { … }, "label": "Paste 3×120 into Table 1" }`. The
engine replays nothing from it on open; it is an audit trail and the source of the undo
timeline in the UI.

## Compatibility promise

The format version bumps the minor number for additive changes and the major number for
anything a 1.x reader could misread. Readers of version N open any file ≤ N.
