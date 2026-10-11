# Changelog

All notable changes to Dioptra. Versions follow [semantic versioning](https://semver.org/);
the file format has its own version (see the manual's File format chapter).

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
