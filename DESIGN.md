# Dioptra — design

Dioptra is a desktop instrument for scientific data: you paste raw numbers in, shape them,
test them, and leave with a figure you can send to a journal. It replaces GraphPad Prism and
the spreadsheet you used before Prism. It runs on Linux, Windows and macOS, keeps your data
in an open file, and never hides an assumption from you.

The name is Hero of Alexandria's sighting instrument (research/00-name-and-stack.md).

## 1. Identity

Calm, precise, instrument-like. The reference objects are a surveyor's theodolite, a good
laboratory balance, a Leitz microscope: matte surfaces, engraved scales, one accent colour
used sparingly for the thing you are pointing at. Nothing glows. No glassmorphism, no neon,
no gradients for decoration, no rounded-everything. Dense where density helps (the grid,
results tables), generous where reading matters (results sentences, assumption notes).

- **Type.** Inter for the UI, with tabular figures everywhere numbers align. JetBrains Mono
  in the formula bar and for file-format views. Figures default to Inter too, bundled, so a
  graph rendered on Linux is byte-identical on a Mac.
- **Colour.** Warm neutral greys (paper, not slate) in light mode; deep graphite in dark
  mode. One accent: *verdigris*, the patina of old bronze instruments —
  light `#2E7D78`, dark `#5FB3AD`. Semantic colours only for meaning: amber for an
  assumption that is in doubt, red for a failed check or a flagged outlier, never for
  decoration. Figure palettes are a separate system (section 6) and are colour-blind safe.
- **Motion.** None that the user waits for. Selection and focus change instantly; graph
  redraws are synchronous when under 50 ms and otherwise show the previous frame until the
  new one is ready.
- **Voice.** Plain English, short sentences, no exclamation marks. Results sheets read like
  a careful colleague: "Welch's t test: the means differ by 3.2 units (95% CI 1.1 to 5.3),
  t(17.4) = 3.21, p = 0.005." Assumption notes say what was checked and what it means for
  this result, not "warning".

## 2. Principles

1. **Right by default and honest about assumptions.** Every analysis states the assumptions
   it rests on, checks what can be checked, shows the check, and offers the alternative when
   a check fails. Defaults follow the current statistical consensus (Welch rather than
   Student, two-sided, Tukey-Kramer for all-pairs, Holm rather than raw Bonferroni, exact
   tests when they are affordable) and say so. Nothing is silently one-sided, silently
   pooled, silently dropped.
2. **Data is sacred.** Raw data is never modified by an analysis or a transform. Transforms
   produce new columns or new tables that remember where they came from. Excluding a value
   marks it; it does not delete it. Every cell can answer "where did you come from?" (typed,
   pasted from the clipboard at a time, imported from a file and column, or computed by a
   formula).
3. **Everything reproducible.** The project file holds the data, every transform as a
   formula, every analysis as a specification, and the command history. Reopening the file
   and pressing "recompute" reproduces every number. There is no Excel step to lose, no
   dialog state to forget. The CLI (`dioptra`) runs the same engine on the same file.
4. **Publication-ready without Illustrator.** The graph you see is the SVG you export; PDF
   embeds the fonts; PNG is rendered from the same scene at any DPI. Sizes are in points and
   millimetres, not pixels. Line widths, tick lengths, font sizes and panel sizes are
   typed numbers, not sliders. Journals' common figure widths (single column 85 mm, double
   174 mm) are presets.
5. **No spreadsheet round trip.** Paste raw data straight from the plate reader, cytometer
   export or lab notebook. Reshape, filter, normalise, subtract baselines and convert units
   in Dioptra's own formula bar. Excel files can be *imported*; they are never required.
6. **Keyboard first, mouse welcome.** Everything in the command palette (Ctrl/⌘-K) with a
   shortcut shown. The grid behaves like a real spreadsheet: arrows, Tab, Enter, Shift-
   select, Ctrl-A, fill-down (Ctrl-D), multi-cell paste, Delete, undo/redo across every
   action including analyses and graph edits.
7. **Open file, open method.** The `.dioptra` file is a zip of pretty-printed JSON that
   `unzip` can inspect and `git` can diff after extraction. Every algorithm names its
   reference in `docs/METHODS.md` and is tested against published certified values.

## 3. The complaint map

`research/02-complaints.md` enumerates what practitioners dislike about Prism and the
Excel-then-Prism workflow as P1…Pn. The table below says how Dioptra answers each one; the
plan ties each answer to a phase. (Filled in from the research; see section 11 for the map.)

## 4. The document model

A **project** contains, in display order, *tables*, *analyses*, *graphs* and *layouts*. Each
object has a stable id, a user title and a `derived_from` link when it was produced by another
object.

### 4.1 Tables

One storage model, eight *kinds* that constrain shape and offer different analyses:

| Kind | Shape | Prism equivalent |
|---|---|---|
| `xy` | X column (optional X error or X replicates), Y datasets each with subcolumns | XY |
| `column` | datasets of values, no X, rows unaligned | Column |
| `grouped` | rows = levels of factor A (row titles), datasets = levels of factor B, subcolumns = replicates | Grouped |
| `contingency` | rows × datasets of counts | Contingency |
| `survival` | X = time, datasets with 1 = event, 0 = censored | Survival |
| `parts` | one dataset, rows = categories | Parts of whole |
| `variables` | flat data frame: each dataset is one variable with a declared type (number, category, text, date), rows = cases | Multiple variables |
| `nested` | datasets = groups, subcolumns = subgroups (e.g. animals), values = observations | Nested |

A **dataset** has a name and a *subcolumn format*: `values(k)` (k replicates),
`mean_sd_n`, `mean_sem_n`, `mean_ci_n`, `mean_sd`, `mean_sem`, `median_iqr`, `lower_upper`,
or `single`. A **column** stores `f64` with NaN for empty, a sparse set of excluded rows, a
sparse map of text entries (so pasted non-numeric values are kept and shown, not lost), and a
*source*: `entered`, `pasted{at}`, `imported{file, sheet, column}` or `formula{expr}`. A
sparse per-cell edit log records hand edits on top of any source. That is the provenance.

### 4.2 The formula language

Column-level by default (one formula fills a column, vectorised), with Excel-style cell
formulas supported for habit. Syntax is spreadsheet-familiar:

```
= log10(A)                          # column A, element-wise
= (A - mean(A[1:3])) / sd(A)        # baseline from the first three rows
= normalize(B, min(B), max(B)) * 100
= if(A > 0, A, NA)
= `Dose (nM)` * 1e-9                # header names in backticks
= B.2                               # dataset B, subcolumn 2
= lag(A, 1) - A                     # per-row difference with the previous row
= convert(A, "mg/dL", "mmol/L")     # unit conversions (glucose, cholesterol, mass, volume, time, temperature)
= A3 * 2                            # a cell formula, when typed in cell B3
```

Values: number, text, boolean, missing (NA), and vectors of these. Aggregates (`mean`,
`median`, `sd`, `sem`, `sum`, `count`, `min`, `max`, `quantile`, `geomean`, `cv`) reduce a
vector; element-wise functions broadcast. Whole-table operations (transpose, pivot wide↔long,
filter rows by predicate, sort, remove baseline by row or by column, fraction of total,
prune/thin rows, average replicates) are *derived tables* whose recipe is stored and replayed.

A dependency graph links columns, tables, analyses and graphs. An edit invalidates
everything downstream; recomputation is pull-based (when something is shown or exported) and
incremental (only dirty nodes). Cycles are rejected at entry with a message naming the loop.

### 4.3 Analyses

An **analysis** is a specification (which table, which kind of test, which options) plus a
cached **results sheet**. Results sheets are structured, not text: sections of typed tables
(numbers carry their display format and full precision; p values carry the style policy),
a list of *assumption checks* with status and plain-English meaning, APA-style summary
sentences, effect sizes with confidence intervals, and a *checklist* (what this test can and
cannot tell you). Every number is copyable; a whole sheet copies as TSV or Markdown.

P-value styles: `APA` (p = .005; p < .001), `NEJM`, `GP` (asterisks: * < .05, ** < .01,
*** < .001, **** < .0001), and `exact` (p = 0.0049). Chosen per project, changeable at any
time, applied everywhere including significance brackets on graphs.

### 4.4 Graphs and layouts

A **graph** is a specification: data source (table or analysis), graph family, aesthetic
mapping, a style (theme + overrides), annotations. The plot engine turns it into a *scene*
(device-independent, in points) and renders SVG, PDF or PNG. A **layout** arranges several
graphs and text blocks on a page with panel letters.

### 4.5 History

Every mutation is a command with an inverse. The command log is the undo stack and is saved
with the project as `history.jsonl` for audit; the UI shows it as a timeline.

## 5. The analysis flow

1. Select data (a table, or a block in a table) and press Analyze (Ctrl/⌘-Shift-A) or use the
   palette. The list shows only analyses that fit the table kind and selection; others are
   visible but say why they do not fit.
2. A single compact options panel: the few choices that matter (paired? which comparisons?
   which correction? one- or two-sided, with two-sided default and a note), with Prism-like
   defaults and one line of explanation each.
3. Run. The results sheet opens beside the table with the assumption checks first (collapsed
   when all pass, open when one does not), then the summary sentence, then the tables.
4. Make a graph from the results with one click; significance brackets and fit curves come
   from the analysis and stay linked.
5. Change the data; the sheet and graph show "stale" for the fraction of a second it takes to
   recompute, then update.

## 6. Figure design

Themes are designed, not default-looking. Three ship in v0.1: **Instrument** (light, thin
black axes, no frame, outside ticks, Inter), **Night** (dark, for slides), **Journal** (strict
black-and-white, Helvetica-metric widths, for print). Palettes: *Okabe-Ito* (default,
colour-blind safe, 8 colours), *Tol bright*, *Tol muted*, *Viridis* and *Cividis* for
sequential, *Tol sunset* for diverging, and a greyscale ramp. The user can override any
colour per dataset.

Every graph family supports: error bars (SD, SEM, 95% CI, range, IQR, custom), data points
over summaries, log axes with correct minor ticks, axis breaks, broken axes with a gap
glyph, axis titles with units and superscripts/subscripts (`^` and `_` in text, Greek via
`\alpha` or direct Unicode), legends inside or outside, significance brackets from an
analysis or placed by hand, fit curves with CI bands, inset panels, and sizes in mm.

Families in v0.1: scatter/XY with fits, line, column (bar) with points, box, violin (with
embedded box or strip), raincloud, before–after (paired lines), estimation (Gardner-Altman,
bootstrap CI of the difference), grouped column, stacked column, heatmap (optionally
clustered), histogram and density, Kaplan–Meier with at-risk table, ROC, Bland–Altman,
volcano, forest, correlation matrix, pie/donut (reluctantly), QQ plot, residual plots.

## 7. The UI contract

Layout: a left **project rail** (tables, analyses, graphs, layouts, as a tree), the **work
area** (grid, results sheet or graph editor, with split view), a right **inspector** (cell
provenance, column type and format, analysis options, graph properties), a **formula bar**
over the grid, a **status line** (selection summary: n, mean, SD, sum; recompute state).
The command palette overlays everything.

The grid: virtualised in both axes; renders 1,000,000 cells without lag because only the
viewport is materialised. Column headers show dataset and subcolumn structure for Prism
kinds. Cell states: empty, number (formatted with the column's decimals, full precision in
the formula bar), text, excluded (struck through), formula (small corner mark), stale.
Keyboard: full spreadsheet set. Mouse: click, drag-select, drag headers to resize, right-click
for context. Paste parses TSV/CSV with locale-aware decimal detection (commas as decimals
when every field is `\d+,\d+` and the separator is tab or semicolon) and reports what it
decided.

The graph editor: the engine's SVG is displayed inline; every scene element carries a target
id, so clicking an axis, a label, a series or a legend selects that object and the inspector
shows its properties; drag moves annotations and the legend; double-click edits text in
place. Changes rebuild the scene and swap the SVG. Export shows a dialog with size (mm,
inches, points), DPI for PNG, and the exact file that will be written.

Accessibility and themes: all colours are tokens; light and dark are complete; focus rings
are visible; nothing depends on hover alone; all commands reachable by keyboard.

## 8. Correctness

`dioptra-stats` is tested against (a) NIST StRD certified values for linear regression,
nonlinear regression, ANOVA and univariate statistics, reporting the log relative error
(LRE) digits achieved per dataset; (b) oracle fixtures generated from scipy, statsmodels,
lifelines and scikit-learn to stated tolerances (`research/05-nist-and-oracle-notes.md`);
(c) hand-checkable textbook examples. `docs/CORRECTNESS.md` is generated from the test run
and reproduced in the README. A method that is not covered by a fixture does not ship.

## 9. Platforms and distribution

Tauri 2 desktop on Linux (AppImage, deb, rpm), Windows (msi, exe) and macOS (arm64 and
x86_64 dmg), built by GitHub Actions on `v*` tags. The engine also compiles to WebAssembly;
the same UI runs in a browser from that build (used for end-to-end tests; a hosted web app is
deferred). A CLI binary runs analyses and renders graphs from a project file for scripting.

## 10. Out of scope for v0.1

See `docs/DEFERRED.md`: mixed-effects models, nested ANOVA beyond descriptive summaries,
multiple logistic regression, Bayesian anything, scripting language beyond the formula
language, cloud sync, collaboration, WebGL rendering for >10⁶ points, plugin system.

## 11. Complaint map

_Filled after research/02-complaints.md; each row names the complaint, the answer and the
plan phase that delivers it._
