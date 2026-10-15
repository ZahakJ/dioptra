# Entering and shaping data

Most errors in a published figure happen before any statistics: a column pasted one row
off, a gene name turned into a date, a value rounded by a spreadsheet's display format, a
baseline subtracted by hand in one column and forgotten in the next. This chapter is about
how Dioptra takes data in and changes it, and how it keeps a record of both.

## Typing

Select a cell and type. A number is stored as a number. Anything else (`<LOD`, `n.d.`,
`see notes`) is stored as **text**, shown in the cell, and never used in arithmetic. It is
not converted to zero and it is not thrown away. A text cell in a numeric column is
something you should see, so Dioptra shows it.

Start with `=` and you type a formula; see [Formulas](formulas.md).

The grid behaves like a spreadsheet: arrows, Tab and Enter move; Shift extends the
selection; F2 or Enter edits; Delete clears; Ctrl-D fills down; Ctrl-Z undoes. The full
list is in [Keyboard shortcuts](shortcuts.md).

## Pasting

Paste (Ctrl-V into a table, or **File → Paste as new table**, Ctrl-Shift-V) accepts any
delimited text: a block copied from Excel, LibreOffice or Google Sheets, a plate reader's
export, a CSV file opened in a text editor. Dioptra decides three things and tells you what
it decided.

**The separator.** A tab anywhere means tab-separated (this is what spreadsheets put on
the clipboard). Otherwise a semicolon or a comma that appears the same number of times on
every line wins, semicolon first; then columns of whitespace-separated numbers; then
commas.

**The decimal mark.** Much of the world writes one and a half as `1,5`. With tab or
semicolon separators, Dioptra counts the fields that look like `1,5` and those that look
like `1.5`; if comma decimals are the majority, commas are decimal marks, and the paste
report says so: "Commas were read as decimal marks (24 values looked like 1,5)." With
comma separators the decimal mark is always the point, because a comma cannot be both.

Some values are ambiguous: `1,234` is one thousand two hundred thirty-four in
Boston and one point two three four in Berlin. Dioptra does not guess. It keeps such values
as text, counts them in the report, and asks you to choose the decimal mark in the dialog.

**The header.** If the first row is text over columns of numbers, it is read as a header
and becomes the data set names.

Missing-value codes are recognised and stored as empty: an empty field, `-`, `.`, `NA`,
`N/A`, `NaN`, `#N/A`, `null`. Everything else that is not a number stays as text.

What Dioptra never does on paste: turn `SEPT2` into 2 September, read `(1.5)` as a
positive number, drop a minus sign, or take the rounded value a spreadsheet was displaying instead of the one it stored. When you paste from a
spreadsheet, check that the spreadsheet shows enough decimals: the clipboard carries what
is displayed. Importing the `.xlsx` file directly avoids this and reads the stored values.

## Importing files

**File → Import** (Ctrl-I) reads:

- CSV, TSV and plain text, with the same detection as paste;
- Excel workbooks (`.xlsx`, `.xls`) and OpenDocument spreadsheets (`.ods`), by a reader
  built into Dioptra, so it works the same on every system and needs no Office
  installation. Cell values are read as stored, at full precision, not as displayed. Date
  cells become ISO dates (`2026-10-11`) and the import notes say how many;
- Prism `.pzfx` files and Prism 10 `.prism` files: the data tables come across, with their
  titles, kinds (nested and multiple-variables included), column names, subcolumn titles,
  row titles and excluded values. Summary formats Dioptra lacks are converted and the
  import notes say how: mean, %CV, N becomes mean, SD, N; mean with +/− error or with upper
  and lower limits becomes a "(mean)" data set plus a lower, upper data set. Date X values
  keep Prism's elapsed time as X and the dates as row titles. An X error subcolumn is not
  imported (Dioptra tables have no X error). Prism's analyses and graphs are not converted;
  run the analyses again in Dioptra, which takes a minute and gives you assumption checks
  you did not have before.

Each imported column remembers the file, sheet and column it came from.

### Plate readers

Exports of four plate-reader programs are recognised by their content and read as plates:

| Program | File | Reads |
|---|---|---|
| Molecular Devices SoftMax Pro | `.txt` (UTF-16 or UTF-8), plate or time format | endpoint, kinetic, several wavelengths and plates |
| BMG Labtech MARS | `.csv`, `.txt` | endpoint plate grids |
| Agilent BioTek Gen5 | `.txt` | endpoint result grids (one read per label), kinetic time courses |
| Tecan Magellan | `.xlsx` with a *Well positions* column | endpoint labels, kinetic cycles |

Without a plate map, an endpoint file becomes a grouped table: rows A, B, … down, one data
set per read with the plate columns 1, 2, … as subcolumns. A kinetic file becomes an XY
table with time in seconds as X and one data set per well.

After choosing a plate file, *Import* asks for an optional **plate map**: paste a grid as
copied from a spreadsheet (row letters and column numbers optional), or one `well, label`
line per well. A label is a group (`Control`), a dose (`10`), or both (`Drug A @ 10`); in
the list form the dose may be a third column. Empty cells leave a well out.

```text
	1	2	3
A	Blank	Drug A @ 0.1	Drug A @ 1
B	Blank	Drug A @ 0.1	Drug A @ 1
```

With groups only, the plate becomes a column table, one data set per group. With doses, it
becomes an XY table: X is the dose, one data set per group, and wells with the same group
and dose are replicates. Kinetic reads keep time as X, with one data set per group.

Not read: spectrum and well-scan reads, SoftMax Pro group (calculation) blocks, Gen5
calculated results in kinetic files and MARS table-view exports. The import notes list what
was left out and any well values that were not numbers (`OVRFLW`, `>4.000`).

### Replacing a table's data

**File → Replace table data…** reads a file into the open table instead of a new one. The
table keeps its name, data set names and column formulas; its analyses, graphs and layouts
recompute. Data columns are filled left to right, skipping formula columns. A file with
more columns adds data sets; a file with fewer clears the columns it does not reach. The
file must give the same kind of table. One undo restores the old data.

This re-runs a project on new data by hand. [Batch analysis](batch.md) does it for many
files at once.

## Missing values

An empty cell is missing. It is not zero, and zero is not missing. Analyses say how they
handle missing values: most use every available value in each group; paired and
repeated-measures analyses drop a subject (row) with any missing value and say how many
were dropped. Formulas propagate missing (`A + 1` is missing where A is) while summaries
skip it (`mean(A)` is the mean of the values present).

## Excluded values

Sometimes a value is real but should not be analysed: a well that dried out, a failed
injection. Select it and press **Ctrl-E** (Edit → Exclude or include). The value stays in
the table, struck through, and every analysis, formula and graph treats it as missing.
Press Ctrl-E again to bring it back.

Why not delete it? Because the record matters. A deleted value is invisible to a reviewer,
to your colleague, and to you in six months. An excluded value is still there, and the
results sheet counts how many excluded values it left out. If you exclude, write down why;
outlier tests (Grubbs, ROUT) can tell you which values are unusual, but whether to exclude
one is a decision about the experiment, not about the numbers.

## Provenance: where every cell came from

Click a cell. The inspector's *Cell* panel says where its value came from:

- *Typed by hand*;
- *Pasted from the clipboard at* a time;
- *Imported from* a file, sheet and column;
- *Computed by* a formula, which it shows;
- *Derived from* another table by a transform.

If you edited a pasted or imported value by hand, the edit is recorded on top of the
source, with the old value and the time. This is the provenance chain, and it is saved in
the file. It answers the question every reviewer eventually asks: where did this number
come from?

## Transforms: new tables from old

Some operations change the shape of the data, not just its values. Dioptra does these as
**derived tables**: you choose a transform from the **Data** menu, and a new table appears
that remembers its source and the recipe. Change the source and the derived table follows.
The original is never modified.

| Transform | What it does | Typical use |
|---|---|---|
| Transpose | Rows become data sets and data sets become rows | A plate reader that exports samples as rows |
| Pivot long → wide | One row per id, one data set per key | Tidy data from R or a database into a Column or Grouped layout |
| Pivot wide → long | A Multiple variables table of (id, variable, value) | Going the other way |
| Stack | Columns into one (group, value) table | Preparing for a by-group analysis elsewhere |
| Filter rows | Keep rows where a formula is true, e.g. `A > 0 && !isna(B)` | Dropping blanks or a subset |
| Sort | Order rows by a column | Reading the extremes |
| Select rows, select columns | Keep a range or a subset | Analysing part of a big table |
| Remove baseline | Subtract, divide or take percent of a baseline: the first row, the mean of chosen rows, another column, or a titled row | Blank subtraction; percent of time zero |
| Fraction of total | Each value as a fraction of its row, column or table total | Proportions |
| Normalize | Map one value to 0 and another to 1 (or 100%): min and max, first and last row, or numbers you give | Percent of maximal response |
| Average replicates | Replicate subcolumns become mean, SD and N | Summarising before plotting |
| Prune | Keep one row in k | Thinning a long time series for display |

For per-row arithmetic (log transforms, unit conversions, ratios) use a column formula
instead: it is shorter, and it shows its recipe in the formula bar.

## Exporting tables

**File → Export table** (Ctrl-Shift-E) writes CSV, TSV or an Excel workbook, at full
precision. That is how you hand data to a colleague who uses something else; you never need
it to move data between steps in Dioptra.
