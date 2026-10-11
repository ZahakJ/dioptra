# 01 — GraphPad Prism: the feature surface

What Prism does, as described by its own documentation. The current guides are for Prism 11 (pages carry the header "GraphPad Prism 11 User Guide") and live under three roots:

- User Guide: `https://www.graphpad.com/guides/prism/latest/user-guide/`
- Statistics Guide: `https://www.graphpad.com/guides/prism/latest/statistics/`
- Curve Fitting Guide: `https://www.graphpad.com/guides/prism/latest/curve-fitting/`

The guides are a Help+Manual web build; the full table of contents of each is served as a script at `<root>/js/hmcontent.js` (for example https://www.graphpad.com/guides/prism/latest/curve-fitting/js/hmcontent.js). The lists of analyses, checklists and equations below were taken from those tables of contents on 2026-10-11 and then checked page by page. Where this document says "Prism does X", the link next to it is the page that says so.

A note on tiers. Since Prism 11 some features are marked in the guide as "available with our new Pro and Enterprise plans" (for example eta squared, partial eta squared and Cohen's f after ANOVA, https://www.graphpad.com/guides/prism/latest/statistics/stat_anova_effect_sizes.htm; phi and Cramer's V for contingency tables, https://www.graphpad.com/guides/prism/latest/statistics/stat_chi-square_or_fishers_test.htm; in-table formulas, https://www.graphpad.com/guides/prism/latest/user-guide/data_inspector_overview.htm). Dioptra should treat all of them as baseline.

---

## 1. Project model

A Prism project has five sections: data tables, info sheets, results, graphs and layouts (https://www.graphpad.com/guides/prism/latest/user-guide/using_what_are_the_parts_of_a_prism_.htm). Everything is linked: editing a data value re-runs the analyses and redraws the graphs that depend on it, and every analysis keeps its dialog choices so it can be reopened and changed ("You'll never see a curve or a P value and wonder how it got there", https://www.graphpad.com/guides/prism/latest/user-guide/everything_is_hot_linked.htm). A data table, its analyses and its graphs form a "family" of sheets (https://www.graphpad.com/guides/prism/latest/user-guide/families_of_sheets.htm).

Supporting machinery that matters for a replacement:

- **Analysis chains**: an analysis can take another analysis's results table as input (https://www.graphpad.com/guides/prism/latest/user-guide/using_chaining_results.htm).
- **Frozen and orphaned results**: results can be frozen against further updates, and become orphaned if their source is deleted (https://www.graphpad.com/guides/prism/latest/user-guide/using_frozen_and_orphaned_results.htm).
- **Info sheets and constants**: info sheets record date, lot numbers, concentrations and similar; their values, and any value in a results table, can be "hooked" as a constant into a transform, a constraint or a text box on a graph (https://www.graphpad.com/guides/prism/latest/user-guide/using_analysis_constants.htm, https://www.graphpad.com/guides/prism/latest/user-guide/a_complete_record_of_your_work.htm).
- **Floating notes**: coloured sticky notes on any sheet, not printed or exported (https://www.graphpad.com/guides/prism/latest/user-guide/using_floating_notes.htm).
- **Repeat an experiment without scripting**: the Wand (analyze and graph just like another table), templates, methods, duplicating a family, cloning a graph (https://www.graphpad.com/guides/prism/latest/user-guide/use_the_wand_to_repeat_analyse.htm, https://www.graphpad.com/guides/prism/latest/user-guide/templates.htm).
- **Prism Magic**: copy the formatting of one graph onto others (https://www.graphpad.com/guides/prism/latest/user-guide/prism_magic_apply_style_from_an_example.htm).
- **Scripts**: a small Prism-specific command language (import, loop, export, write WBlock/WTBlock text), mostly used for batch import and Monte Carlo (https://www.graphpad.com/guides/prism/latest/user-guide/using_scripts.htm).
- **Limits**: up to 500 sheets of each type per project; any number of rows (limited by RAM; "We've tested hundreds of thousands of rows"); up to 2048 data-set columns (1024 in Prism 9, 256 in Prism 8, 104 in Prism 5); up to 512 subcolumns (https://www.graphpad.com/guides/prism/latest/user-guide/data_table_limits.htm).

---

## 2. The eight data tables

Prism's central idea is that the table format encodes the experimental design, and the format then decides which analyses and graphs are offered. The overview page lists the eight formats (https://www.graphpad.com/guides/prism/latest/user-guide/using_data_table_format.htm).

### 2.1 Common cell semantics

- **Blank = missing.** "Prism never ever treats an empty cell as if you had entered zero ... Similarly, it never treats a zero as a missing value." (https://www.graphpad.com/guides/prism/latest/user-guide/missing_values.htm)
- **Excluded values** are shown in blue italics followed by an asterisk; they stay in the table "to document its value" but are ignored by analyses and graphs exactly as if missing. Copy/export can render them as blank, as the plain number, or as number plus `*` (https://www.graphpad.com/guides/prism/latest/user-guide/excluding_values.htm).
- **Cell highlighting**: eight background colours for marking cells to check (same page).
- **Column titles, subcolumn titles, row titles**: row titles are an optional first column that labels subjects or samples; in Grouped tables row titles are labels, not X coordinates (https://www.graphpad.com/guides/prism/latest/user-guide/xy_table.htm, https://www.graphpad.com/guides/prism/latest/user-guide/use_of_row_titles_in_xy_graphs.htm).
- **Replicate layout differs by table**: side-by-side subcolumns in XY and Grouped tables; stacked down a column in Column tables; stacked within each subcolumn in Nested tables; not applicable to Contingency, Parts of whole, Survival and Multiple variables (https://www.graphpad.com/guides/prism/latest/user-guide/stacked_vs__side-by-side_repli.htm).

### 2.2 Y subcolumn formats (XY and Grouped tables)

When a table is created the user chooses either N side-by-side replicate subcolumns or a format for error values computed elsewhere (https://www.graphpad.com/guides/prism/latest/user-guide/graphing_errorbars_calclated_e.htm, https://www.graphpad.com/guides/prism/latest/user-guide/graphing-error-bars-computed-2.htm). The formats, with the attribute names the `.pzfx` XML uses for them (from the reader in the `pzfx` R package, section 7.4), are:

| Format | Subcolumns | `.pzfx` `YFormat` |
|---|---|---|
| Replicates (1..512) | values | `replicates` |
| Mean, SD, N | 3 | `SDN` |
| Mean, SEM, N | 3 | `SEN` |
| Mean, %CV, N | 3 | `CVN` |
| Mean and SD | 2 | `SD` |
| Mean and SEM | 2 | `SE` |
| Mean and %CV | 2 | `CV` |
| Mean (or median), +error, -error | 3 | `low-high` |
| Mean (or median), upper limit, lower limit | 3 | `upper-lower-limits` |

Rules that come with these formats:

- With N present, analyses can reconstruct the raw-data sums of squares; regression on mean/SD/N "will compute exactly the same results from least-squares regression as you would have gotten had you entered raw data" (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_method-tab.htm).
- Without N, error values are ignored by fitting; only means are fitted (https://graphpad.com/support/faq/nonlinear-regression-when-you-entered-error-values-directly).
- Upper/lower limits are bar end points; +/- values are distances from the mean (same KB article). The guide notes users may type median and quartiles into the mean/upper/lower format, with limited analyses (https://www.graphpad.com/guides/prism/latest/user-guide/graphing_errorbars_calclated_e.htm).
- Unbalanced two-way ANOVA from mean/SD/N uses the "unweighted means" approximation; from raw data it uses the Glantz & Slinker regression method with Type III sums of squares (https://www.graphpad.com/guides/prism/latest/user-guide/missing_values.htm).

### 2.3 Table by table

**XY** — every point has an X and a Y (https://www.graphpad.com/guides/prism/latest/user-guide/xy_table.htm). One X column (numbers, or dates/elapsed times chosen at creation, https://www.graphpad.com/guides/prism/latest/user-guide/xy_tables__x_represents_dates_.htm), optional row titles, and many Y data sets each with replicate or error subcolumns. **X error**: an X column can carry one error subcolumn; "You can only enter one value for X errors, so X error bars are always symmetrical"; Prism does not record whether the value is SD or SEM and never uses X errors in calculations. Analyses: linear and nonlinear regression, interpolation, Deming regression, splines/LOWESS, area under curve, smoothing, derivatives, integrals, correlation, row statistics.

**Column** — one grouping variable; each column is a group, replicates stacked down the column; rows have meaning only for paired/matched designs (one subject per row) (https://www.graphpad.com/guides/prism/latest/user-guide/column_tables.htm). Can also be formatted for entry of mean/SD/N. Analyses listed on that page: unpaired, paired and ratio-paired t tests, Mann-Whitney, Wilcoxon, Kolmogorov-Smirnov, ordinary one-way ANOVA, Brown-Forsythe and Welch ANOVA, repeated-measures one-way ANOVA, Kruskal-Wallis, Friedman, one-sample t and Wilcoxon, descriptive statistics, normality and lognormality tests, frequency distribution, ROC curve, Bland-Altman, outlier identification, analysis of a stack of P values.

**Grouped** — two grouping variables, one on rows and one on columns, replicates in side-by-side subcolumns or entered as mean/SD/N; up to 256 data-set columns and 256 subcolumns at the time the page was written (https://www.graphpad.com/guides/prism/latest/user-guide/two_grouping_variable_table.htm). Repeated-measures ANOVA cannot have missing values "unless an entire subcolumn is empty" for classical ANOVA; since Prism 8 a mixed model is fitted instead when values are missing (https://www.graphpad.com/guides/prism/latest/user-guide/missing_values.htm). Analyses: two-way and three-way ANOVA (and mixed models), row means with SD/SEM, multiple t tests (one per row).

**Contingency** — counts of subjects in each row-by-column cell (https://www.graphpad.com/guides/prism/latest/user-guide/contingency_table.htm). Values must be non-negative integers: "Prism won't let you enter a minus sign or decimal point or create subcolumns." Rows and columns are interpreted by design (prospective, retrospective case-control, experimental, cross-sectional). Analyses: chi-square and Fisher's exact test (with relative risk, odds ratio), fraction of total.

**Survival** — one row per subject; X is elapsed time (dates are not accepted); each Y column is a treatment group and holds a code, 1 for the event and 0 for censored (https://www.graphpad.com/guides/prism/latest/user-guide/survival_table.htm). Analyses: Kaplan-Meier curves, log-rank and Gehan-Breslow-Wilcoxon comparisons. Error bars (SE or 95% CI) are computed, never entered. Cox proportional hazards regression is run from a Multiple variables table instead (https://www.graphpad.com/guides/prism/latest/statistics/index.htm, "Survival analysis").

**Parts of whole** — "What fraction of the total is each value?"; any number of columns, but only column A is graphed automatically (https://www.graphpad.com/guides/prism/latest/user-guide/about_parts_of_whole.htm). Analyses: fraction of total, chi-square goodness of fit against expected proportions.

**Multiple variables** — the "tidy" table: rows are observations, columns are variables, no subcolumns (https://www.graphpad.com/guides/prism/latest/user-guide/multiple-variable-tables.htm). Since Prism 9 every variable has a type: categorical (text or numeric codes; case is ignored in analysis), continuous, or label (identifiers and notes, never analysed) (https://www.graphpad.com/guides/prism/latest/user-guide/mv_variable_types.htm). Prism 11 adds calculated variables defined by in-table formulas (unit conversions, ratios, logs, conditional categories), shown in italics, recalculated on change, with circular-reference detection; available only in this table type and only on Pro and Enterprise plans (https://www.graphpad.com/guides/prism/latest/user-guide/data_inspector_overview.htm). Analyses: correlation matrix, multiple linear (and Poisson) regression, multiple logistic regression, Cox regression, PCA, clustering, extract-and-rearrange, select-and-transform, outliers, descriptive statistics; t tests and Kaplan-Meier can also be run from this table.

**Nested** — two levels of hierarchy (for example rats within treatments, several measurements per rat); replicates stacked within each subcolumn, at least two subcolumns; "It is not possible to enter averaged data (mean, sd, n...)" (https://www.graphpad.com/guides/prism/latest/user-guide/nested-tables.htm). Analyses: nested t test, nested one-way ANOVA, and per-subcolumn descriptive statistics, normality tests, outlier tests, one-sample t test. The guide has a page on telling Nested and Grouped tables apart, because they look alike (https://www.graphpad.com/guides/prism/latest/user-guide/the-difference-between-nested-.htm).

---

## 3. The analyses

### 3.1 The Analyze menu (Prism 11)

The menu groups analyses by goal (https://www.graphpad.com/guides/prism/latest/user-guide/analyze_menu_groups.htm); the Analyze Data dialog instead groups them by the table type they usually run on, and many appear under several types (https://www.graphpad.com/guides/prism/latest/user-guide/using_choosing_an_analysis.htm).

| Menu group | Analyses |
|---|---|
| Data processing | Transform; Transform concentrations (X); Normalize; Prune rows; Remove baseline and column math; Transpose X and Y; Fraction of total; Identify outliers; Extract and rearrange; Select and transform |
| Data exploration and summary | Descriptive statistics; Row statistics; Correlation; Frequency distribution; Normality and lognormality tests; Compare observed distribution with expected; Smooth, differentiate or integrate curve; Area under curve; Principal component analysis; Analyze a stack of P values |
| Regression and curves | Nonlinear regression (curve fit); Interpolate a standard curve; Simple linear regression; Simple logistic regression; Fit spline/LOWESS; Deming (Model II) linear regression; Multiple linear regression; Multiple logistic regression; Plot a function |
| Group comparison | t tests (and nonparametric tests); One sample t and Wilcoxon test; One-way ANOVA (and nonparametric or mixed); Two-way ANOVA (or mixed model); Three-way ANOVA (or mixed model); Multiple t tests (and nonparametric tests); Nested t test; Nested one-way ANOVA; Chi-square (and Fisher's exact) test; Bland-Altman method comparison; ROC curve |
| Survival analysis | Simple survival analysis (Kaplan-Meier); Cox proportional hazards regression (from Multiple variables tables) |
| Simulation | Simulate XY data; Simulate column data; Simulate 2x2 contingency table; Monte Carlo |

The Statistics Guide adds, as analysis chapters: multifactor ANOVA, hierarchical and k-means clustering, the confidence interval of a proportion, and a QQ plot (https://www.graphpad.com/guides/prism/latest/statistics/js/hmcontent.js).

### 3.2 By table type

The same list regrouped the way the Analyze dialog presents it (drawn from the table pages in section 2 and the menu above):

- **XY analyses**: nonlinear regression; interpolate a standard curve; simple linear regression; simple logistic regression; Deming regression; spline/LOWESS; correlation; area under curve; smooth/differentiate/integrate; row statistics; plot a function; simulate XY data; plus data-processing transforms.
- **Column analyses**: t tests (unpaired, paired, ratio paired, Welch, lognormal); Mann-Whitney, Kolmogorov-Smirnov, Wilcoxon matched pairs; one-way ANOVA (ordinary, Brown-Forsythe/Welch, repeated measures/mixed model, lognormal), Kruskal-Wallis, Friedman; one-sample t and Wilcoxon signed rank; descriptive statistics; normality and lognormality; frequency distribution; identify outliers (ROUT, Grubbs); ROC; Bland-Altman; analyze a stack of P values; simulate column data.
- **Grouped analyses**: two-way ANOVA (ordinary, repeated measures, mixed model); three-way ANOVA; multiple t tests (with volcano plot, https://www.graphpad.com/guides/prism/latest/statistics/stat_volcano-plot-from-multiple-t-t.htm); row means and totals.
- **Contingency**: chi-square (with Yates option), Fisher's exact, chi-square for trend (Cochran-Armitage); fraction of total; simulate 2x2 table.
- **Survival**: Kaplan-Meier with log-rank (Mantel-Cox), log-rank for trend and Gehan-Breslow-Wilcoxon comparisons, median survival, hazard ratios (https://www.graphpad.com/guides/prism/latest/statistics/index.htm, "Survival analysis").
- **Parts of whole**: fraction of total; compare observed distribution with expected (chi-square or binomial goodness of fit).
- **Multiple variables**: multiple linear/Poisson regression; multiple logistic regression; correlation matrix; PCA; Cox regression; clustering; extract/select and transform; descriptive statistics.
- **Nested**: nested t test; nested one-way ANOVA (both are mixed models with the subcolumn as a random factor).

### 3.3 Data-processing "analyses"

All of them write a new results table and never change the data table (https://www.graphpad.com/guides/prism/latest/user-guide/using_transform.htm).

- **Transform**: built-in Y (and analogous X) functions — `Y*K`, `Y+K`, `Y-K`, `Y/K`, `Y^2`, `Y^K`, `log`, `-log`, `ln`, `10^Y`, `exp`, `1/Y`, `sqrt`, `logit`, `probit` (old definition, adds 5), `rank`, `zscore`, trig functions, `abs`, `Y+Random` (Gaussian noise, SD = K), `X/Y`, `Y/X`, `Y-X`, `Y+X`, `Y*X`, `X-Y`, `K-Y`, `K/Y`, `log2`, `2^Y`, rounding. K can differ per data set or be hooked to a constant. Options: interchange X and Y; transform each replicate or the mean; for asymmetric transforms of mean/SD data, either drop error bars or convert to a 95% CI and transform both ends. Pharmacology transforms for display only: Eadie-Hofstee, Hanes-Woolf, Hill, Lineweaver-Burk, log-log, Scatchard. User-defined transforms support `IF(cond, a, b)`, `AND/OR/NOT`, and per-data-set lines prefixed `<B>` or `<~A>`; dividing by zero yields a blank cell.
- **Transform concentrations (X)**: unit and log conversion of X (https://www.graphpad.com/guides/prism/latest/user-guide/transforming_concentrations.htm).
- **Remove baseline and column math**: subtract or divide by a baseline defined as first/last rows of the same column, or the same row of a baseline column, optionally via a regression line fitted to the baseline values; also fractional/percentage difference, add or multiply columns (https://www.graphpad.com/guides/prism/latest/user-guide/using_removebaseline.htm).
- **Normalize**: 0% = smallest value, first row, or entered value; 100% = largest value, last row, entered value, or column sum; results as fractions or percentages; SDs are scaled correctly; means of replicates or each subcolumn separately (https://www.graphpad.com/guides/prism/latest/user-guide/using_normalizing_data.htm).
- **Transpose**: rows become data sets; limited to 256 rows by the column limit at the time of writing (https://www.graphpad.com/guides/prism/latest/user-guide/using_transpose.htm).
- **Prune rows**: keep a range or average every K rows into one (decimation is also available at import) (https://www.graphpad.com/guides/prism/latest/user-guide/using_pruning_rows.htm).
- **Fraction of total**: divide by column total, row total or grand total; optional binomial CIs (Wilson/Brown recommended; Clopper-Pearson in older versions), only meaningful for raw counts (https://www.graphpad.com/guides/prism/latest/user-guide/fractions_of_total.htm).
- **Identify outliers**: ROUT or Grubbs on stacked column data (section 5.6).

### 3.4 Multiple comparisons after ANOVA

The Options tab offers three families (https://www.graphpad.com/guides/prism/latest/statistics/stat_summary_of_multiple_comparison.htm):

| Goal | With confidence intervals | Significance only |
|---|---|---|
| Every mean vs every other | Tukey (recommended), Bonferroni, Sidak | Holm-Sidak (preferred), Newman-Keuls, Dunn (nonparametric) |
| Every mean vs control | Dunnett, Sidak, Bonferroni | Holm-Sidak, Dunn |
| Selected pairs (up to 40) | Bonferroni-Dunn, Sidak-Bonferroni | Holm-Sidak, Dunn |
| Linear trend across columns | test for linear trend (one-way only) | |

Alternatively the user may control the false discovery rate (three methods; uncorrected P values first from Fisher's LSD, uncorrected Dunn, or Welch t), or not correct at all (Fisher's LSD), in which case the guide says the report must state that P values are unadjusted. Welch/Brown-Forsythe ANOVA has its own post-tests (Games-Howell, Dunnett T3, Tamhane T2) (same page and its neighbours in the TOC).

---

## 4. Graph types per table family

Graphs are created with each data table and are formatted through a type-specific Format Graph dialog, the Format Axes dialog and, for Multiple variables graphs, the Graph Inspector (https://www.graphpad.com/guides/prism/latest/user-guide/graphs2.htm). The guide is explicit that bar graphs follow the table, not the other way round: "Don't think 'make a bar graph'. Instead think about how the data are organized" (https://www.graphpad.com/guides/prism/latest/user-guide/using_change_graph_type.htm, https://www.graphpad.com/guides/prism/latest/user-guide/bar_graphs.htm).

| Family | Graph types documented |
|---|---|
| XY | points, points with connecting lines, lines only, spikes (XY bar graph), mean with error bars or each replicate, error envelopes, best-fit curves with confidence and prediction bands, residual plots, line of identity, spaghetti plots, box-and-whiskers on XY, right Y axis, date/elapsed-time axes, log axes, discontinuous axes (https://www.graphpad.com/guides/prism/latest/user-guide/using_xy_graphs.htm, https://www.graphpad.com/guides/prism/latest/user-guide/spaghetti-plot.htm, https://www.graphpad.com/guides/prism/latest/user-guide/graphing-error-envelopes.htm) |
| Column | scatter (dot) plots, bars with error bars, scatter with bar, box-and-whiskers, violin plots (new in Prism 8; light/medium/heavy smoothing; median and quartile lines; KDE truncated at data range, https://www.graphpad.com/guides/prism/latest/user-guide/graphing-box-and-whisker-and-v.htm), before-after plots, forest plots (built by a documented trick from a Column table, https://www.graphpad.com/guides/prism/latest/user-guide/forest_plot.htm), colour-coded column scatter |
| Grouped | interleaved, separated and stacked bars, grouped scatter, box/violin, heat maps (https://www.graphpad.com/guides/prism/latest/user-guide/heat_maps2.htm), grand mean/median lines, three-way graphs (https://www.graphpad.com/guides/prism/latest/user-guide/three_way_graphs.htm) |
| Contingency | interleaved and stacked bars of counts or fractions (https://www.graphpad.com/guides/prism/latest/user-guide/using_grouped_graphs_2.htm) |
| Survival | Kaplan-Meier staircase with censor ticks, SE or 95% CI error bars or bands, nudging, number-at-risk table (https://www.graphpad.com/guides/prism/latest/user-guide/using_survival_graphs.htm, https://www.graphpad.com/guides/prism/latest/user-guide/survival_number_at_risk_table.htm) |
| Parts of whole | pie and donut charts, horizontal and vertical slice bars (https://www.graphpad.com/guides/prism/latest/user-guide/parts_of_whole_graphs.htm) |
| Multiple variables | variable-mapped graphs: X/Y/colour/size assignments, symbols, connecting lines, confidence ellipses, convex hulls, heat maps, dendrograms, summary symbols and lines, bars, box-and-whiskers, violins, error bars, separate colour and size legends, level order (https://www.graphpad.com/guides/prism/latest/user-guide/using_multiple_variables_graphs.htm) |
| Nested | each subcolumn as a scatter or bar with SD, grouped by the top level (https://www.graphpad.com/guides/prism/latest/user-guide/nested-tables2.htm, https://www.graphpad.com/guides/prism/latest/user-guide/bar_graphs.htm) |
| From analyses | estimation plots (Prism 9+, unpaired and paired t tests only), residual plots, QQ plots, ROC curves, Bland-Altman plots, volcano plots, PCA scree/loadings/biplots, frequency histograms |

Annotations: pairwise comparison brackets with asterisks or P values (https://www.graphpad.com/guides/prism/latest/user-guide/simple_pairwise_comparisons.htm), compact letter display (https://www.graphpad.com/guides/prism/latest/user-guide/compact_letter_display.htm), embedded results tables on graphs, text, arrows, shapes, pictures. Layouts place several graphs on a page with a master legend (https://www.graphpad.com/guides/prism/latest/user-guide/page_layouts.htm).

**Estimation plots**, in Prism's implementation (https://www.graphpad.com/guides/prism/latest/statistics/stat_estimation_plots.htm): the left axis shows the data (scatter for n < 100, violin for larger n); the right axis shows the difference between means and its 95% CI, with its zero aligned to the mean of the first group; the right axis range must be left on automatic or the plot is wrong. A hidden results tab holds the plotted values. Only t tests produce them.

**Colour**: built-in colour schemes, including colour-blind-safe schemes added in 9.5, user-defined schemes, and semitransparent colours (https://www.graphpad.com/guides/prism/latest/user-guide/colors.htm, https://www.graphpad.com/support/faq/colorblind-safe-colors-schemes-and-transparency).

**Export formats** (https://www.graphpad.com/guides/prism/latest/user-guide/chosing_an_export_format.htm): PDF (vector, RGB or CMYK, multi-page), SVG, EPS (Windows: text as text or outlines; Mac: always outlines; no transparency), TIFF (up to 1200 dpi, RGB/CMYK/monochrome, LZW or PackBits), EMF/EMF+/WMF (Windows only), JPEG (least compression only), PNG (screen resolution; transparency on Windows only), BMP.

---

## 5. Results-sheet conventions

### 5.1 Layout

Each analysis produces a results sheet with tabs (for example "Table of results", "Descriptive statistics", "Multiple comparisons", "Normality tests", "Outliers", "Cleaned data", hidden estimation-plot data). Results tables with green grid lines can be graphed; red grid lines mean they cannot (https://www.graphpad.com/guides/prism/latest/user-guide/fractions_of_total.htm, https://www.graphpad.com/guides/prism/latest/user-guide/analysis-tabs2.htm). Key results can be colour-coded (https://www.graphpad.com/guides/prism/latest/user-guide/color_coding_key_results.htm). A multiple-comparison row reads, for example, "Predicted (LS) mean diff. | 95.00% CI of diff. | Below threshold? | Summary | Adjusted P Value" (https://www.graphpad.com/guides/prism/latest/user-guide/compact_letter_display.htm).

### 5.2 P value formats

Chosen per analysis (and as a preference) (https://www.graphpad.com/guides/prism/latest/statistics/stat_decimal_formatting_of_p_values.htm):

| Style | Example | Smallest shown | Asterisk scale |
|---|---|---|---|
| APA | `.123` (3 digits, no leading zero) | `< .001` | up to `***` |
| NEJM | `0.123` (3 digits, leading zero) | `< .001` | up to `***` |
| GraphPad (GP) | `0.1234` (4 digits, leading zero) | `< 0.0001` | up to `****` |
| Custom | 0 to 15 digits, decimal or scientific | per setting | up to `****` |

Asterisk thresholds (https://www.graphpad.com/guides/prism/latest/statistics/extremely_significant_results.htm):

| P | Wording | GP summary | APA/NEJM summary |
|---|---|---|---|
| < 0.0001 | Extremely significant | `****` | `***` |
| 0.0001 to 0.001 | Extremely significant | `***` | `***` |
| 0.001 to 0.01 | Very significant | `**` | `**` |
| 0.01 to 0.05 | Significant | `*` | `*` |
| >= 0.05 | Not significant | `ns` | `ns` |

The decision uses the stored double-precision P, not the displayed value: P = 0.05000001 displays as "0.0500" and is labelled `ns` (same page). Prism 8.0-8.2 showed the format choices with example values `.033`, `0.033`, `0.0332`, which "some people have misunderstood ... to mean that we define a single asterisk to mean P<0.0332"; GraphPad states the thresholds are the standard 0.05 ones (https://www.graphpad.com/support/faq/what-is-the-meaning-of--or--or--in-reports-of-statistical-significance-from-prism-or-instat/). The often-quoted "0.1234 / 0.0332 / 0.0021 / 0.0002" ladder is therefore a set of display examples, not thresholds. The guide also advises against "significant" and asterisks altogether (https://www.graphpad.com/guides/prism/latest/statistics/js/hmcontent.js, "Advice: Avoid the concept of 'statistical significance' when possible", "Advice: Avoid the word 'significant'").

### 5.3 Effect sizes and intervals

- **t tests** report the difference between means with its SE and 95% CI ("The most useful result is the confidence interval for the difference between the means"), t, df, the F test for unequal variances, and R squared (eta squared), which "Prism, unlike most statistics programs, reports" (https://www.graphpad.com/guides/prism/latest/statistics/how_the_unpaired_t_test_works2.htm). The sign of the difference is chosen on the Options tab (A-B or B-A).
- **ANOVA** reports R squared for the model; on Pro/Enterprise also eta squared, partial eta squared and Cohen's f, with the conventional small/medium/large table and a warning that those benchmarks vary by field (https://www.graphpad.com/guides/prism/latest/statistics/stat_anova_effect_sizes.htm). Multiple comparisons give differences with multiplicity-adjusted CIs where the test allows.
- **Contingency** tables report relative risk, attributable risk (difference of proportions) with NNT, odds ratio, sensitivity/specificity/predictive values, and on Pro/Enterprise phi and Cramer's V (https://www.graphpad.com/guides/prism/latest/statistics/stat_chi-square_or_fishers_test.htm).
- **Confidence level** is usually selectable (90/95/99%); results show the level in the heading ("95.00% CI of diff.").
- **Nonlinear regression** reports best-fit values, SEs, asymmetric (profile-likelihood) or symmetric (asymptotic) CIs, goodness of fit, and flags (section 6.4).

### 5.4 Analysis checklists

Every analysis has a checklist page that walks the user through its assumptions ("All statistical analysis is based on a set of assumptions. These checklists help you review the assumptions, and make sure you have picked a useful test") (https://www.graphpad.com/guides/prism/latest/statistics/analysis_checklists2.htm). Checklists exist for: unpaired, paired, lognormal and ratio t tests; Mann-Whitney; Kolmogorov-Smirnov; Wilcoxon matched pairs; ordinary, repeated-measures and lognormal one-way ANOVA; Kruskal-Wallis; Friedman; ordinary and repeated-measures two-way ANOVA; three-way ANOVA; Kaplan-Meier; Cox regression; descriptive statistics; outliers; correlation; Bland-Altman; ROC; contingency tables; PCA; nested t test; nested one-way ANOVA; simple linear regression; simple logistic regression; nonlinear regression (fitting, comparing, interpolating); interpolating from a standard curve; multiple linear and logistic regression. A typical checklist item for the unpaired t test asks whether values are Gaussian, whether the two populations have the same SD, whether the data are unpaired, whether errors are independent, whether a one-tailed P was chosen before collecting data. The checklist is documentation reached from the results sheet; it does not test anything itself.

### 5.5 Summary text

Results sheets list a "P value summary" (asterisks), "Significantly different (P < 0.05)?" Yes/No (wording governed by the alpha chosen in the dialog), and for comparisons "Below threshold?". Prism does not write a prose paragraph; the guide's "How to report statistical results" page covers what to write (https://www.graphpad.com/guides/prism/latest/statistics/js/hmcontent.js).

### 5.6 Outlier identification (ROUT and Grubbs)

ROUT for column data fits `Y = M` (a robust mean) by robust regression and flags points whose residuals are too large by a false-discovery-rate criterion; it can find any number of outliers up to 30% of n and works from n = 3 (https://www.graphpad.com/guides/prism/latest/statistics/stat_how_it_works_rout_method.htm). Q is the maximum desired FDR: with no outliers present Q behaves like alpha; with outliers present, at most Q of the flagged points are expected to be false. The recommended Q is 1%; "if all the scatter is Gaussian, Prism will falsely find one or more outliers in about 2-3% of experiments"; Q = 0 turns detection off (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_method-tab.htm). Grubbs is slightly better at a single outlier; iterative Grubbs suffers from masking with two. Reference: Motulsky HJ, Brown RE. Detecting outliers when fitting data with nonlinear regression — a new method based on robust nonlinear regression and the false discovery rate. BMC Bioinformatics 2006;7:123 (https://doi.org/10.1186/1471-2105-7-123).

---

## 6. Nonlinear regression

### 6.1 Built-in equation library

Equations as printed on each equation page of the Curve Fitting Guide (index: https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_models_built-in_to_prism.htm). Parameter names are Prism's own and should be kept, because users know them.

**Dose-response — stimulation** (X = log concentration unless the name says [Agonist]):

| Equation | Model | Parameters |
|---|---|---|
| log(agonist) vs. response (Hill slope 1) | `Y=Bottom + (Top-Bottom)/(1+10^(LogEC50-X))` | Bottom, Top, LogEC50 |
| log(agonist) vs. response — Variable slope (4PL) | `Y=Bottom + (Top-Bottom)/(1+10^((LogEC50-X)*HillSlope))` | + HillSlope |
| log(agonist) vs. normalized response | `Y=100/(1+10^(LogEC50-X))` | LogEC50 |
| ... normalized response — Variable slope | `Y=100/(1+10^((LogEC50-X)*HillSlope))` | LogEC50, HillSlope |
| [Agonist] vs. response | `Y=Bottom + X*(Top-Bottom)/(EC50 + X)` | Bottom, Top, EC50 |
| [Agonist] vs. response — Variable slope | `Y=Bottom + (X^HillSlope)*(Top-Bottom)/(X^HillSlope + EC50^HillSlope)` | + HillSlope |
| [Agonist] vs. normalized response (± variable slope) | `Y=100*X/(EC50+X)`; `Y=100*(X^HillSlope)/(EC50^HillSlope + X^HillSlope)` | EC50 (, HillSlope) |

Source pages: https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_dr_stim.htm, https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_dr_stim_variable.htm and siblings (`reg_dr_stim_normalized.htm`, `reg_dr_stim_2.htm`, `reg_dr_stim_variable_2.htm`, ...).

**Dose-response — inhibition**: the mirror set — log(inhibitor) vs. response `Y=Bottom + (Top-Bottom)/(1+10^(X-LogIC50))`; variable slope `Y=Bottom + (Top-Bottom)/(1+10^((LogIC50-X)*HillSlope))` (HillSlope negative for a falling curve); normalized versions with 100 and 0 fixed; [Inhibitor] versions `Y=Bottom + (Top-Bottom)/(1+(X/IC50))` and `Y=Bottom + (Top-Bottom)/(1+(IC50/X)^HillSlope)` (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_dr_inhibit.htm and siblings).

**Dose-response — special** (each in an X = concentration and an X = log concentration form; the concentration forms were added in Prism 10):

- Asymmetrical five-parameter (5PL): `LogXb = LogEC50 + (1/HillSlope)*Log((2^(1/S))-1)`; `Y = Bottom + (Top-Bottom)/(1+10^((LogXb-X)*HillSlope))^S`. Parameters Bottom, Top, LogEC50, HillSlope, S (S = 1 gives the 4PL). The guide warns it needs "lots of high quality data" and cites Gottschalk & Dunn 2005 and Liao & Liu 2009 on fitting EC50 rather than Xb (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_asymmetric_dose_response_ec.htm).
- Biphasic: `Span=Top-Bottom`; two logistic sections with LogEC50_1, LogEC50_2, nH1, nH2 and Frac (proportion due to the more potent phase) (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_biphasic_dose_response.htm).
- Bell-shaped: Plateau1, Plateau2, Dip, LogEC50_1, LogEC50_2, nH1, nH2 (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_bellshaped_dose_response.htm).
- Operational model, depletion and partial agonist: `operate=(((10^logKA)+(10^X))/(10^(logtau+X)))^n`; `Y=Basal + (Effectmax-Basal)/(1+operate)` (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_operational_model_depletion.htm).
- Gaddum/Schild EC50 shift: `Antag=1+(B/(10^(-1*pA2)))^SchildSlope`, curve shifted by Antag; pA2, SchildSlope, plus a variant with SchildSlope = 1 (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_gaddumschild.htm).
- EC50 shift; allosteric EC50 shift (`Antag=(1+B/KB)/(1+alpha*B/KB)`, LogKB, Logalpha); ECanything (fit EC80 or any ECF directly); absolute IC50 (`Fifty=(Top+Baseline)/2`, fits the concentration that gives a response of 50 in absolute terms) (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_ecanything.htm, https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_absolute_ic50.htm).

**Receptor binding — saturation** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_binding-saturation.htm): one site specific `Y = Bmax*X/(Kd + X)`; one site total `Y=Bmax*X/(Kd+X) + NS*X + Background`; one site fit total and nonspecific (global, shared Kd/Bmax/NS); one site total accounting for ligand depletion (quadratic in cpm, needs SpecAct and Vol); two sites specific (BmaxHi, KdHi, BmaxLo, KdLo); two sites fit total and nonspecific; one site with allosteric modulator; specific binding with Hill slope `Y=Bmax*X^h/(Kd^h + X^h)`; binding potential.

**Receptor binding — competition** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_binding_-_competition.htm): one site fit Ki (Cheng-Prusoff inside the model; needs HotNM and HotKDNM constants); one site fit logIC50; two sites fit Ki and fit logIC50 (FractionOne); one site with ligand depletion; homologous (one site, Bmax, logKD); allosteric modulator.

**Receptor binding — kinetics** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_kinetics_of_binding.htm): dissociation `Y=(Y0-NS)*exp(-K*X) + NS`; association at one or two ligand concentrations (Kon, Koff, Bmax; `Kob=Kon*L+Koff`); association then dissociation (Time0); kinetics of competitive binding (Motulsky-Mahan; K1..K4).

**Enzyme kinetics**: Michaelis-Menten `Y = Vmax*X/(Km + X)`; kcat `Y = Et*kcat*X/(Km + X)`; allosteric sigmoidal `Y=Vmax*X^h/(Khalf^h + X^h)`; inhibition models — competitive (`KmObs=Km*(1+I/Ki)`), noncompetitive (`Vmaxinh=Vmax/(1+I/Ki)`), uncompetitive (AlphaKi), mixed (Alpha, Ki), substrate inhibition `Y=Vmax*X/(Km + X*(1+X/Ki))`, tight-binding Morrison (Et, Ki, S, Km); inhibitor concentration I is typically a data-set constant from column titles (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_michaelis_menten_enzyme.htm, https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_enzyme_kinetics_--_inhibition.htm).

**Exponential** (https://www.graphpad.com/guides/prism/latest/curve-fitting/exponentialtoplevel.htm):

| Equation | Model | Parameters |
|---|---|---|
| One phase decay | `Y=(Y0 - Plateau)*exp(-K*X) + Plateau` | Y0, Plateau, K (derived: Span, half-life `ln(2)/K`, tau `1/K`) |
| Two phase decay | `SpanFast=(Y0-Plateau)*PercentFast*.01`; ... `Y=Plateau + SpanFast*exp(-KFast*X) + SpanSlow*exp(-KSlow*X)` | Y0, Plateau, PercentFast, KFast, KSlow |
| Plateau followed by one phase decay | `Y= IF(X<X0, Y0, Plateau+(Y0-Plateau)*exp(-K*(X-X0)))` | X0, Y0, Plateau, K |
| Three phase decay | YFast, YMedium, YSlow terms | Y0, Plateau, PercentFast, PercentSlow, KFast, KMedium, KSlow |
| One phase association | `Y=Y0 + (Plateau-Y0)*(1-exp(-K*X))` | Y0, Plateau, K |
| Plateau followed by one phase association | `IF(X<X0, Y0, ...)` | X0, Y0, Plateau, K |
| Two phase association | SpanFast/SpanSlow form | Y0, Plateau, PercentFast, KFast, KSlow |
| Exponential growth | `Y=Y0*exp(k*X)` | Y0, K |

**Lines** (https://www.graphpad.com/guides/prism/latest/curve-fitting/linestoplevel.htm): straight line `Y= YIntercept + Slope*X`; line through origin; segmental (piecewise) linear regression with intercept1, slope1, slope2, X0 joined at X0; hinge function `Y= Intercept + Slope1*(X – X0) + (Slope2 – Slope1)*Delta*ln(1+exp((X-X0)/Delta))`; straight lines on semilog and log-log axes; line on a probability axis (cumulative Gaussian via `zdist`); crossing point of two lines (Xcross, Ycross).

**Polynomial**: orders 1 to 6, `Y=B0 + B1*X + ... + B6*X^6`, and centered forms using `XC = X - mean X` (data-set constant "Mean X") (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_howtopolynomial.htm, https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_centered_polynomial_equations.htm).

**Gaussian family** (https://www.graphpad.com/guides/prism/latest/curve-fitting/gaussiantoplevel.htm): Gaussian `Y=Amplitude*exp(-0.5*((X-Mean)/SD)^2)` (and sum of two); log Gaussian `Y=(A/X)*exp(-0.5*(ln(X/GeoMean)/ln(GeoSD))^2)`; cumulative Gaussian `Y=Top*zdist((X-Mean)/SD)` (Top = 100, 1 or N); Lorentzian `Y=Amplitude/(1+((X-Center)/Width)^2)` (and sum of two).

**Sine waves**: standard (Amplitude, Wavelength, PhaseShift), damped (K), sinc, with nonzero baseline (Baseline) (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_sine_waves.htm).

**Growth**: exponential (Malthusian), log of exponential growth `Y=logY0 + k*X`, logistic `Y=YM*Y0/((YM-Y0)*exp(-k*x) +Y0)`, Gompertz `Y = YM*(Y0/YM)^(exp(-K*X))`, exponential plateau `Y=YM -(YM-Y0)*exp(-k*x)`, beta growth and decline (Ym, Te, Tm) (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_growth-equations.htm).

**Linear-quadratic cell survival**: `Y = exp(-1*(A*X + B*X^2))` with variants for fraction, percent and number surviving or dead (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_linear-quadratic-fs.htm).

**Classic equations from Prism 4** (kept for compatibility): one and two site binding (hyperbola), sigmoidal dose-response (fixed and variable slope), one and two site competition, Boltzmann sigmoid (BOTTOM, TOP, V50, SLOPE), one and two phase exponential decay and association, exponential growth, power series, sine wave, Gaussian (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_classicequations.htm). There is no separate built-in "Hill equation" family: the Hill form appears as the variable-slope [Agonist] model, specific binding with Hill slope, and the allosteric sigmoidal enzyme model.

**User-defined equations**: multiline models, IF, differential equations, implicit equations, per-data-set lines (`<A>Bottom=4.5`), column constants, two independent variables, rules for initial values, default constraints and reported parameter transforms (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_overview_user-defined_equation.htm and the "Entering a user-defined model" chapter of the TOC).

### 6.2 Fitting options (dialog tabs)

The nonlinear regression dialog has tabs Model, Method, Compare, Constrain, Initial values, Range, Output, Confidence, Diagnostics and Flags (https://www.graphpad.com/guides/prism/latest/curve-fitting/js/hmcontent.js).

**Method** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_method-tab.htm):

- Fitting method: least squares; robust regression (Lorentzian-type, "cannot generate confidence intervals", mainly the first step of outlier detection); Poisson regression (raw counts only); don't fit (plot the curve defined by initial values).
- Outliers: no detection, detect and count, or detect and eliminate, with ROUT Q (1% recommended); optional table of cleaned data (Prism 8+); an outlier table is always produced.
- Convergence: Quick (two iterations changing SS by < 0.01%), Medium (default; five iterations < 0.0001%), Strict (five iterations < 0.00000001%); maximum iterations 1000.
- Weighting: none; 1/Y^2 (constant relative scatter); 1/Y (Poisson-like); 1/Y^K ("general weighting"); 1/X; 1/X^2; 1/SD^2. Weighting is accounted for in residual plots and R squared; robust fitting disables some weights; with outlier removal, the first robust step is unweighted and outliers are judged on weighted residuals.
- Replicates: fit each replicate or only the means; fit means when replicates are not independent (for example triplicates from one animal per dose).

**Constrain** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_constrainttab.htm): per parameter — fit freely; constant value; range (greater than, less than, between); shared across data sets (global fit); data-set constant (column title or mean X). Constraints between parameters are allowed (Kfast > Kslow) but not when both are also individually constrained. A fit that ends at a bound is flagged "hit constraint".

**Initial values** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_initial_values_tab.htm, https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_rulesforinitialvalues.htm): every equation carries rules that compute starting values from the data: a multiplier times YMIN, YMID, YMAX, XMIN, XMID, XMAX, X at YMID, YatXmin/YatXmax, initial/middle/final slope (regression on the first/middle/last 10% of points when n >= 30, else two points), `SIGN(YatXmax - YatXmin)` (used for HillSlope), mean of column titles or its log, or a fixed value. Users can override per data set. Example: Michaelis-Menten Vmax = 1.0 x YMAX, Km = 1.0 x (X at YMID).

**Compare** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_comparing_models_tab.htm): questions — which of two models fits better; does a parameter differ between data sets; does one curve fit all data sets; does a parameter differ from a hypothetical value. Methods — extra sum-of-squares F test (nested models only, different df; Prism does not enforce nesting) or AICc (nested or not; reports probabilities and the ratio of likelihoods); option to first check that both fits are unambiguous. Poisson fits use the likelihood ratio test (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_howtheftestworks.htm, https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_how_the_aicc_computations_work.htm).

**Confidence**: asymmetric profile-likelihood CIs or symmetric asymptotic CIs; confidence or prediction bands; the "unstable parameter" detection that replaced "ambiguous" as the default in Prism 9 (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_confidence_tab.htm, https://www.graphpad.com/guides/prism/latest/user-guide/prism-lab-feature.htm).

**Diagnostics** (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_diagnostics_tab.htm): R squared, adjusted R squared, sum of squares, Sy.x and RMSE, AICc (per fit); for robust fits the RSDR (68.27th percentile of |residuals| x n/(n-K)); for Poisson fits pseudo-R squared, dispersion index and deviance; normality tests of residuals (D'Agostino-Pearson recommended among four); **replicates test** (are points further from the curve than the scatter among replicates predicts?) when replicates are fitted; **runs test** (too few runs of same-sign residuals) when single values or means are fitted, reported per curve and not for the global fit; test for appropriate weighting / homoscedasticity; five kinds of residual plot; dependency of each parameter; covariance matrix; Hougaard's measure of skewness.

**Output, Range, Flags**: table of the curve (number of points), interpolation of unknowns, the X range over which the curve is drawn, and thresholds for flagging fits.

### 6.3 Interpolation

Unknowns placed below the standards in the same table are interpolated from the fitted curve, with optional CIs; Prism will not search for a "linear range", supports interpolation from replicates and log X, and explains every reason for a blank result (https://www.graphpad.com/guides/prism/latest/curve-fitting/js/hmcontent.js, "Interpolating from a standard curve").

### 6.4 Result flags

Parameter-level: "Unstable", "Very wide", "???", "~" (an approximate CI), incomplete CI, "Not used". Fit-level: "Bad initial values", "Interrupted", "Not converged", "Ambiguous", "Hit constraint", "Don't fit", "Too few points", "Perfect fit", "Impossible weights", "Equation not defined", "Can't calculate" (Curve Fitting Guide TOC, "Error messages from nonlinear regression").

---

## 7. Getting data in, and the file formats

### 7.1 Paste from Excel

Prism pastes the HTML rendition of the Excel clipboard by default (Greek letters, super/subscripts, locale decimal separators survive). Prism for Windows refuses to paste a cell that mixes digits and text into a numeric cell; Prism for Mac extracts the digits. With old Excel versions there was a choice between the displayed (rounded) value and full precision; with Excel 2007 and later the full-precision copy is not on the clipboard, so what is pasted is what is displayed (https://www.graphpad.com/guides/prism/latest/user-guide/options_when_pasting_data_from.htm). Excel's red-without-minus negatives and parenthesised negatives paste as positive numbers (https://www.graphpad.com/guides/prism/latest/user-guide/why_do_negative_numbers_become.htm).

### 7.2 Import, link and embed

The "Import and Paste Special" dialog has five tabs (https://www.graphpad.com/guides/prism/latest/user-guide/using_paste_special_and_import_options.htm):

- **Source**: insert data only; insert and maintain a live link to the source file (re- analyses when the file changes; Windows links to Excel or text, Mac to text only); embed the whole source file as an OLE object (Windows only); auto-update option; how to read spaces and commas (https://www.graphpad.com/guides/prism/latest/user-guide/using_paste_special_source.htm).
- **View**: preview.
- **Filter**: missing-value code (for example 99); treat `value*` as excluded; row range; skip rows until/after/where a column meets a criterion (`<=, <, =, >, >=, <>` against a constant, never column against column); decimate (keep one row in K); column range and stride; **unstack** an indexed (long) file using a group column, which must contain integers, not text (https://www.graphpad.com/guides/prism/latest/user-guide/using_paste_special_filter.htm).
- **Placement**: rename table/columns from the file; top-left cell; transpose; fill "by rows" or "by columns" with N values per row (Prism ignores the subcolumn/data-set boundary when doing so) (https://www.graphpad.com/guides/prism/latest/user-guide/using_paste_special_placement.htm).
- **Info & Notes**: structured `<title>`, `<notes>`, `<info>` blocks at the top of text files.

Direct `.xls` import is Windows only, reads only the worksheet that was active when the workbook was saved, and drives a hidden Excel through OLE, so it needs Excel installed; the guide recommends copy and paste instead (https://www.graphpad.com/guides/prism/latest/user-guide/using_importing_native_excel_files_w.htm). Text formats accepted: TXT, DAT, CSV, PRN.

### 7.3 Project file formats

- **`.prism`** (Prism 10 and later, default): "operates much like a zip file"; "data are stored in CSV format and all analysis parameters and results are stored in standard JSON schemas"; GraphPad presents it as FAIR and non-proprietary. No public schema is published; the page does not say how graphs are encoded. Prism 9 and earlier cannot open it; Prism 10+ can save down to `.pzfx`/`.pzf` when the Compatibility Checker finds no critical features (https://www.graphpad.com/guides/prism/latest/user-guide/prism_file_format.htm, https://graphpad.com/support/faq/opening-new-file-format-in-old-prism-versions).
- **`.pzfx`** (Prism 5+): "The first part holds the data tables and info sheets as plain-text XML ... The rest of the file is unreadable outside Prism"; never stores analysis results (recomputed on open); about 10% of the size of `.pzf` but slower to open and save large files (https://www.graphpad.com/guides/prism/latest/user-guide/pzf_vs__pzfx_file_format.htm).
- **`.pzf`**: binary, Prism 4+, can store results.

### 7.4 What is publicly known of the `.pzfx` XML

From a sample file shipped with the R package `pzfx` (https://github.com/Yue-Jiang/pzfx/blob/master/inst/extdata/exponential_decay.pzfx) and its reader and writer source (https://github.com/Yue-Jiang/pzfx/tree/master/R):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<GraphPadPrismFile PrismXMLVersion="5.00">
  <Created>
    <OriginalVersion CreatedByProgram="GraphPad Prism" CreatedByVersion="6.0f.254"
                     Login="..." DateTime="2018-09-02T16:57:49-07:00"/>
  </Created>
  <InfoSequence><Ref ID="Info0" Selected="1"/></InfoSequence>
  <Info ID="Info0">
    <Title>Project info 1</Title>
    <Notes>...</Notes>
    <Constant><Name>Experiment Date</Name><Value>2018-09-02</Value></Constant>
  </Info>
  <TableSequence Selected="1"><Ref ID="Table0" Selected="1"/></TableSequence>
  <Table ID="Table0" XFormat="numbers" YFormat="replicates" Replicates="3"
         TableType="XY" EVFormat="AsteriskAfterNumber">
    <Title>Exponential decay</Title>
    <FloatingNote ID="Sticky0" Color="Yellow" Left="399" Top="127" Width="546" Height="321">
      <WebLink URL="@help:REG_Exponential_decay_1phase">...</WebLink>
      <B><Font Size="9" Face="Verdana">...</Font></B><BR/>
    </FloatingNote>
    <XColumn Width="103" Decimals="1" Subcolumns="1">
      <Title>Minutes</Title>
      <Subcolumn><d>1.0</d><d>2.0</d>...</Subcolumn>
    </XColumn>
    <YColumn Width="269" Decimals="0" Subcolumns="3">
      <Title>Control</Title>
      <Subcolumn><d>8887.</d><d>8329.</d>...</Subcolumn>
      <Subcolumn><d>7366.</d><d></d>...</Subcolumn>   <!-- empty d = missing -->
      <Subcolumn>...</Subcolumn>
    </YColumn>
    ...
  </Table>
  <Template dt:dt="bin.base64" xmlns:dt="urn:schemas-microsoft-com:datatypes">eNrs...</Template>
</GraphPadPrismFile>
```

Observed and documented details:

- Tables are `Table` or `HugeTable` elements; the reader stops with "Can't work with this pzfx file, is it later than v6.0?" when a table lacks `Title`.
- `TableType` values seen: `XY`, `OneWay` (Column) in the writer; others (Grouped, Contingency, Survival, Parts of whole, Multiple variables, Nested) exist in real files but are not documented by either package.
- `XFormat`: `none`, `numbers`, `error` (X plus one X-error subcolumn), `date` (with an extra `XAdvancedColumn` holding the date strings such as "8-Sep-2008").
- `YFormat`: `replicates`, `SDN`, `SEN`, `CVN`, `SD`, `SE`, `CV`, `low-high`, `upper-lower-limits` (section 2.2).
- `RowTitlesColumn` holds row titles as a single `Subcolumn`.
- Each value is a `<d>` element; an empty `<d>` is missing; `Excluded="1"` on a `<d>` marks an excluded value; `EVFormat="AsteriskAfterNumber"` describes how excluded values render.
- Notes and titles carry inline formatting markup (`<B>`, `<Font>`, `<BR>`), as the floating note above shows; numbers are written in the author's locale, which is why the R reader converts a decimal comma to a point before parsing.
- `Template` is a base64 blob (the bytes start with a zlib header, `eNr...`) and is, by elimination, where analyses, graphs and layouts live; no public parser decodes it. This is the part the guide calls unreadable outside Prism.

Public parsers:

- **`pzfx`** (R, CRAN, MIT; Yue Jiang): `pzfx_tables(path)`, `read_pzfx(path, table, strike_action = c("exclude","keep","star"), date_x = c("numeric","character","both"))`, `write_pzfx(x, path, ...)` which writes XY and Column tables with replicates or SD/N (https://cran.r-project.org/package=pzfx, vignette https://cran.r-project.org/web/packages/pzfx/vignettes/pzfx.html, source https://github.com/Yue-Jiang/pzfx).
- **`pzfx`** (Python, PyPI): a port of the R package, `read_pzfx` and `pzfx_tables`, `strike_action` (https://pypi.org/project/pzfx/).
- **`prism2R`** (R, r-universe; Miha Kosmac): reads `.pzfx` and `.prism` into data frames; `get_table_names`, `read_prism`, `get_analysis_tables` (https://r-multiverse.r-universe.dev/prism2R). The pzfx README points to it for the new format.

### 7.5 Exporting data

Data and results tables export to text/CSV with a choice of how excluded values appear (https://www.graphpad.com/guides/prism/latest/user-guide/using_exporting_from_data_tables.htm).

---

## 8. Platform, licensing and collaboration (facts only)

- Windows and macOS only; "No plans to create a Linux version of Prism or InStat"; GraphPad does not support Wine (https://www.graphpad.com/support/faq/will-you-port-prism-or-instat-to-linux-does-prism-work-with-the-windows-emulator-wine/).
- Feature differences between Windows and Mac are documented (OLE linking/embedding, EMF export, EPS text handling, PNG transparency, Excel import) (https://www.graphpad.com/guides/prism/latest/user-guide/features-in-windows-but-not-ma.htm).
- Prices on 2026-10-11 (https://www.graphpad.com/how-to-buy/): personal annual Student $142, Academic $260, Corporate $520, monthly $50; group academic $520/2 seats to $2,600/10 seats, corporate $1,120 to $5,600; Enterprise by quote (SSO, SCIM, "Advanced Analysis & Graphing"); perpetual Prism 11 licences $1,600 academic, $3,200 corporate, which GraphPad says it does "not recommend" (no upgrades, no guaranteed OS support).
- An expired trial becomes a viewer, so colleagues without a licence can read files (https://www.graphpad.com/guides/prism/latest/user-guide/a_complete_record_of_your_work.htm).
- Prism Cloud (beta, eligible subscriptions, some regions): publish a project, share with Commenter or Editor roles, discussions; "co-editing" is download, edit locally, publish a new version; full version history on Enterprise only; Prism 10.5+ (https://www.graphpad.com/guides/prism/latest/user-guide/prism_cloud_co_editing.htm).

---

## 9. What this means for Dioptra (short)

1. The eight table formats are the product. Keep them as *views* over one long, typed table, so that the Multiple variables table is the storage form and the other seven are layouts with design metadata (paired rows, replicate subcolumns, nesting).
2. Reproduce every subcolumn format and keep the mean/SD/N arithmetic exact, including the unweighted-means fallback, so results match Prism to the printed digit.
3. Keep Prism's parameter names, equation forms and initial-value rules; a user moving a dose-response fit should see `LogEC50`, `HillSlope`, `Top`, `Bottom`.
4. Ship P value styles APA, NEJM, GP and custom with the documented thresholds and the stored-precision rule, but default to effect size plus CI first.
5. Read `.pzfx` (data and info sheets) and, later, `.prism` (CSV + JSON); treat `Template` as opaque and say so to the user.

## Sources

- GraphPad Prism 11 User Guide, https://www.graphpad.com/guides/prism/latest/user-guide/ (pages cited inline; TOC at https://www.graphpad.com/guides/prism/latest/user-guide/js/hmcontent.js)
- GraphPad Prism 11 Statistics Guide, https://www.graphpad.com/guides/prism/latest/statistics/ (TOC at https://www.graphpad.com/guides/prism/latest/statistics/js/hmcontent.js)
- GraphPad Prism 11 Curve Fitting Guide, https://www.graphpad.com/guides/prism/latest/curve-fitting/ (TOC at https://www.graphpad.com/guides/prism/latest/curve-fitting/js/hmcontent.js)
- GraphPad KB: asterisks and the 8.0-8.2 display examples, https://www.graphpad.com/support/faq/what-is-the-meaning-of--or--or--in-reports-of-statistical-significance-from-prism-or-instat/
- GraphPad KB #2038, fitting entered error values, https://graphpad.com/support/faq/nonlinear-regression-when-you-entered-error-values-directly
- GraphPad KB, opening `.prism` in older versions, https://graphpad.com/support/faq/opening-new-file-format-in-old-prism-versions
- GraphPad KB #326, Linux and Wine, https://www.graphpad.com/support/faq/will-you-port-prism-or-instat-to-linux-does-prism-work-with-the-windows-emulator-wine/
- GraphPad KB, colour-blind-safe schemes, https://www.graphpad.com/support/faq/colorblind-safe-colors-schemes-and-transparency
- GraphPad, How to buy, https://www.graphpad.com/how-to-buy/
- Motulsky HJ, Brown RE. BMC Bioinformatics 2006;7:123, https://doi.org/10.1186/1471-2105-7-123
- Jiang Y. pzfx: Read and Write GraphPad Prism Files (R), https://cran.r-project.org/package=pzfx ; vignette https://cran.r-project.org/web/packages/pzfx/vignettes/pzfx.html ; source https://github.com/Yue-Jiang/pzfx
- pzfx (Python port), https://pypi.org/project/pzfx/
- Kosmac M. prism2R, https://r-multiverse.r-universe.dev/prism2R
