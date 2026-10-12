# Changelog

All notable changes to Dioptra. Versions follow [semantic versioning](https://semver.org/);
the file format has its own version (see the manual's File format chapter).

## 0.3.0 — 2026-10-12

Connecting lines: Prism's before–after graphs, made an option of every bar graph.

### Graphs

- **Connecting lines** on column, strip, box and violin graphs (`options.connecting`): a
  line per subject joining the values of each row across the bars, the means or medians
  joined across the bars, or both. Lines are drawn above bars, boxes and violins and beneath
  the points; the summary line on top. Style (solid, dashed, dotted), width, opacity and
  colour: one colour, by data set, per subject from a palette, or by a grouping column with a
  legend. A missing value leaves a gap (or the line joins across it). On grouped tables the
  lines run within each group across its data sets (matched subcolumns) or along each data
  set across the groups. With jitter on, each line ends at its own jittered point. Lines
  are cut where they cross an axis break. Every subject line is a target in the graph
  editor.
- **Paired t, ratio paired t, Wilcoxon, repeated-measures ANOVA and Friedman** now graph as
  bars with connecting lines; the results sheet's button says **Graph with connecting
  lines**. The New graph dialog offers **Bars with connecting lines** and **Before–after
  (points)** as presets for column and grouped tables.
- **Horizontal** column, strip, box and violin graphs (`options.orientation`); `axes.y`
  stays the value axis, as in Prism.
- Bar options from a comparison with Prism's Format Graph: error-bar **direction** (both,
  above, below) and **cap width**; **bar width** and **slot width** (the gaps between bars
  and between groups); **bar fill** (tint, solid, outline); **fill patterns** per data set
  (diagonals, crosshatch, horizontal, vertical, dots), drawn as geometry so SVG, PDF and PNG
  agree; strip graphs without points (mean ± error only) or without the mean line.
- Open markers (`circle_open` and friends) now draw open on column graphs.
- Any line now breaks where it crosses an axis break, and horizontal bars are cut at breaks
  of the value axis.
- New inspector sections: **Connecting lines** and **Bars and points** (orientation,
  summary, jitter, fill, widths, error-bar direction and caps); a pattern control per data
  set.

### File format

- Format **1.1**. Graph specs gain the fields above; defaults are not written, so graphs
  without them are saved as before. The `before_after` family is retired: opening a 1.0
  file turns such graphs into the strip spec that draws the same picture (unjittered points,
  subject lines, the mean line when it was on). `docs/FILE-FORMAT.md` documents the
  migration.

### Research and docs

- `research/06-connecting-lines.md`: what Prism's column, grouped and categorical graphs
  offer for connecting lines, with sources, and how Dioptra maps each option.
- Manual: **Connecting lines and paired graphs**, with a worked example.

## 0.2.1 — 2026-10-12

### Fixed

- macOS: the app opened as "Dioptra is damaged and can't be opened". The bundle is now
  ad-hoc signed as a whole (`bundle.macOS.signingIdentity: "-"`), and the release job fails
  unless `codesign --verify --deep --strict` passes on the `.app` inside the
  `.dmg`. The app is still not notarized: the first launch needs right-click → Open (macOS 15
  and later: Privacy & Security → Open Anyway) or `xattr -cr /Applications/Dioptra.app`.
  See `docs/macos-gatekeeper.md`.

## 0.2.0 — 2026-10-12

The design release: the same engine, a redesigned instrument.

### Interface

- A design system (`DESIGN.md` §12): warm paper neutrals in light, deep graphite in dark,
  one verdigris accent, Inter for the interface and JetBrains Mono with tabular figures for
  every number, an 8-pt spacing scale, Lucide icons, designed focus rings and thin themed
  scrollbars. Every text/background pair meets WCAG AA in both themes
  (`docs/DESIGN-AUDIT.md`, `scripts/contrast.mjs`).
- No native form control anywhere: own Select, Segmented, Toggle, Checkbox, NumberField
  with unit suffixes, ColorSwatch, Menu, Dialog and Tooltip.
- The in-window menu bar is gone. A slim top bar carries the mark, the project title
  (click to rename), the command field and the theme control; the menu tree lives in a
  compact Menu popover and the command palette. macOS gets a native system menu.
- Dark mode follows the operating system (and the Tauri window theme) by default, with a
  light/dark/system control in the top bar, View and the palette; every surface is designed
  for dark rather than inverted.
- The grid is a bounded sheet: rows end where the data ends plus a margin, with "+ rows"
  and "+ Data set" affordances; data-set headers carry a kind glyph and the colour swatch
  the graph will use; a designed selection with corner handles; comfortable and compact
  density; a first-class formula bar; human-readable provenance times.
- Results sheets in a reading column with the type system, hairline tables, mono numbers,
  assumption cards with designed marks and the alternative on a second line.
- The graph editor is a desk with millimetre rulers, a paper choice (white, theme,
  transparent), a designed zoom control, hover halos and a selection frame on targets, and
  collapsible inspector sections that remember their state.
- A designed first launch: paste data, open the bundled sample project, or read the manual.
- A new icon set from the dioptra mark; window size, position and panel widths are
  remembered.
- The paste dialog's segmented choices sit at their own width instead of stretching across
  the column.
- A designed README banner (also the landing page's Open Graph image), and a landing page
  whose captures follow its light or dark theme.

### Engine

- `SetProjectTitle` command (undoable); the risk-difference Wald interval is clamped to
  [-1, 1].

## 0.1.0 — 2026-10-11

The first public release.

### Data tables

- Eight table kinds: XY, Column, Grouped, Contingency, Survival, Parts of whole, Multiple
  variables and Nested, each explained with a sketch when a table is created.
- Subcolumn formats: replicate values, mean/SD/N, mean/SEM/N, mean/CI/N, mean/SD,
  mean/SEM, median/IQR, lower/upper and single values.
- A virtualised grid that holds a million cells, with the full spreadsheet keyboard set,
  fill down, undo and redo across every action, and a status line with n, mean, SD and sum.
- Text cells kept as text, never coerced; excluded values struck through and kept.
- Cell provenance: typed, pasted (with time), imported (file, sheet, column), formula or
  derived, plus a per-cell edit log.

### Getting data in and out

- Paste with automatic separator, decimal-comma and header detection, and a report of what
  was decided; ambiguous values such as `1,234` are kept as text and reported.
- Paste as a new table with a suggested table kind.
- Import of CSV, TSV, text, XLSX, XLS, ODS (built-in reader, stored values at full
  precision) and Prism `.pzfx` data tables.
- Export of tables to CSV, TSV and XLSX.

### Formulas and transforms

- Column formulas and cell formulas with fill-down, a dependency graph, pull-based
  recomputation and circular-reference detection.
- Arithmetic, logic, row functions, summaries, series functions (`lag`, `lead`, `diff`,
  `cumsum`, `rank`, `zscore`, `baseline`), slices, and unit conversion including
  mass-to-molar conversion for glucose, cholesterol, triglycerides, creatinine and urea.
- Derived tables with stored recipes: transpose, pivot long to wide and wide to long, stack,
  filter, sort, select rows and columns, remove baseline, fraction of total, normalise,
  average replicates and prune.

### Command line

- `dioptra info`, `run`, `import`, `analyze`, `render`, `export`, `bench` and `equations` drive
  the same engine from scripts and continuous integration.

### Analyses

- Twenty analyses: descriptive statistics; t tests (one-sample, Welch, Student, paired,
  ratio paired); Mann–Whitney; Wilcoxon signed-rank; one-way ANOVA with Welch and
  Brown–Forsythe; repeated-measures one-way ANOVA with Greenhouse–Geisser and Huynh–Feldt;
  two-way ANOVA (types I, II, III; one repeated factor); Kruskal–Wallis; Friedman;
  chi-square; Fisher's exact test (2 × 2 and r × c); correlation (Pearson, Spearman,
  Kendall); simple linear regression with bands and diagnostics; nonlinear regression;
  normality tests; outlier identification (Grubbs, ROUT); survival (Kaplan–Meier,
  log-rank, Gehan–Breslow–Wilcoxon, hazard ratios); ROC curves; Bland–Altman; principal
  component analysis.
- Multiple comparisons: Tukey–Kramer, Dunnett, Šidák, Holm, Bonferroni, Dunn and
  Benjamini–Hochberg.
- Nonlinear regression with the equation library (4PL, 5PL, exponential, Michaelis–Menten,
  Hill, binding, Gaussian, polynomials and more), custom equations, weighting, robust
  fitting, constraints, shared parameters, profile-likelihood intervals, model comparison
  by extra sum of squares and AICc, and ROUT.
- Results sheets with assumption checks shown first, APA-style summary sentences, a
  methods sentence, effect sizes with confidence intervals, and a reporting checklist.
- P-value styles APA (default), NEJM, GP and exact, applied project-wide.
- Analyses listed by fit to the table, with the reason for each that does not fit.

### Graphs

- Twenty-three families: scatter, line, column, box, violin, strip, raincloud,
  before–after, estimation, SuperPlot, histogram, density, heatmap, correlation matrix,
  Kaplan–Meier with at-risk table, ROC, Bland–Altman, volcano, forest, pie, donut, QQ and
  residual plots; column graphs can be grouped, stacked or percent-stacked.
- An interactive editor: click any element to edit it in the inspector, drag the legend
  and annotations, edit text in place; significance brackets, reference lines and labels.
- Themes Instrument, Night and Journal; palettes Okabe–Ito (default), Tol bright, muted
  and light, viridis, cividis, Tol sunset and grey.
- Sizes in millimetres, type and lines in points, journal presets; multi-panel layouts
  with panel letters.
- Export to SVG (real text), PDF (embedded subset fonts) and PNG at any resolution;
  identical bytes on every platform.

### Files and reproducibility

- The `.dioptra` project file: a zip of documented, pretty-printed JSON with the data,
  formulas, analysis and graph specifications, and the command history.

### Correctness and performance

- All 58 NIST StRD data sets used (univariate, linear regression, ANOVA, nonlinear
  regression) pass; 8,327 comparisons against SciPy, statsmodels, lifelines and
  scikit-learn agree within tolerance.
- A million-cell paste in about 0.13 s; a column formula over a million rows in under
  0.11 s.

### Platforms

- Linux (AppImage, deb, rpm), Windows (msi, setup exe) and macOS (Apple silicon and Intel
  dmg), built from one codebase.
