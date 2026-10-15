# Batch analysis

Part of Dioptra Lab. Importing a single file, plate-reader exports included, and
[replacing a table's data](entering-data.md#replacing-a-tables-data) are free.

A **template** is any project plus the table that receives the data. Batch analysis puts
each data file into a copy of the template, recomputes every formula, analysis, graph and
layout, and writes:

- `<file>.dioptra`, the recomputed project;
- `<file> - <graph>.pdf` (or `.png`, `.svg`) for every graph and layout, at its set size;
- `summary.csv`, one row per file.

Files are read as by [Replace table data](entering-data.md#replacing-a-tables-data):
names and column formulas of the receiving table stay, the data change. A plate map, if
given, applies to every plate-reader file.

## The summary

The summary has a *File* column, one column per headline number and an *Error* column.
Headline numbers come from every section of every results sheet: the columns headed
*Value*, *Best-fit* or *Estimate*, and every column of p values. Each is named after the
analysis and the row, such as `Fit · Data set A · LogEC50`. A nonlinear fit gives its
parameters (Bottom, Top, LogEC50, HillSlope) and derived values (EC50); a t test gives t,
df, the P value, the difference and the effect sizes. Rename an analysis in the template to
name its columns.

A file that cannot be analysed (no data, a table of the wrong kind) gets its error in the
summary; the others go on.

## In the app

**File → Batch analysis…** asks for:

- the template: the open project, or a project file;
- the table that receives the data;
- the data files (on the desktop, files or a whole folder);
- the figure format and an optional plate map;
- the output folder (desktop). In the browser, everything downloads as one zip.

The dialog shows progress and, at the end, the summary and any errors.

**File → Watch a folder…** (desktop) applies a template to each new file that appears in a
folder, once its size has stopped changing for two seconds. Results go to the folder's
`analysed` subfolder, with a `summary.csv` that grows by one row per file. Files already in
the folder are left alone. **File → Stop watching the folder** ends it; so does quitting.

## From the command line

```sh
dioptra batch assay.dioptra --table "Plate" exports/ --out results --format png
```

Inputs are files or folders (their data files, not subfolders). `--plate-map map.txt`
applies a plate map; `--dpi` sets the PNG resolution (300). The exit status is non-zero
when any file fails.

The key is read from `$DIOPTRA_LICENSE`, or from the file the desktop app keeps:

| System | File |
|---|---|
| Linux | `~/.config/io.github.zahakj.dioptra/license.key` |
| macOS | `~/Library/Application Support/io.github.zahakj.dioptra/license.key` |
| Windows | `%APPDATA%\io.github.zahakj.dioptra\license.key` |

`dioptra license` shows it, `dioptra license --set <key>` stores it and
`dioptra license --remove` deletes it.
