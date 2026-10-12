# Analyses

One entry per analysis kind: when to use it, the tables it accepts, its options (JSON keys
of the spec, with defaults; every key may be left out), what it checks, what the results
sheet contains, the method it rests on, and a worked example. The general rules (how
missing, excluded and summary values are handled, the assumption-check policy, the
sentence format) are in [METHODS.md, Results sheets](METHODS.md#results-sheets).

Every sheet has: assumption checks (shown first), summary sentences, a methods line, typed
sections, and a checklist of what the analysis assumes and cannot tell you. `columns` are
flat column indices (the X column, when there is one, is 0); data sets are named by any of
their subcolumns. An empty `columns` means "the first data sets" (or all, as stated).
Multiple-comparison requests have the shape
`{"method": {"name": "tukey" | "dunnett" (+ "control") | "sidak" | "holm" | "bonferroni" | "dunn" | "benjamini_hochberg"}, "comparisons": {"kind": "all_pairs" | "vs_control" (+ "control") | "selected" (+ "pairs")}}`.

The default graph family is what `CreateGraph` picks for an analysis when `family` is empty
or `"auto"`.

---

## descriptive — Descriptive statistics

- **When:** describe each data set (n, mean, SD, SEM, CI of the mean, quartiles, range, CV,
  geometric mean).
- **Tables:** column, XY, grouped, variables, nested.
- **Options:** `columns` [] (all), `ci_level` 0.95, `percentiles` [] (extra, 0–100).
- **Checks:** values used (text and excluded cells counted); independence and normality
  stated, not tested.
- **Outputs:** Descriptive statistics table. Graph: `column`.
- **Method:** [descriptive](METHODS.md#descriptive); quartiles by the (n + 1)p rule.
- **Example:**
  - Data: PlantGrowth `ctrl` (n = 10).
  - Result: M = 5.032, SD = 0.5831, 95% CI [4.615, 5.449].
  - Median 5.155, quartiles by Prism's rule.
  - Checks: every value used.

## t_test — t tests

- **When:** compare two means (unpaired or paired), a ratio of paired values, or one mean
  with a hypothetical value.
- **Tables:** column, variables, nested.
- **Options:** `columns` (first two data sets), `variant` `"welch"` (also `"unpaired"`
  = Student, `"paired"`, `"ratio_paired"`, `"one_sample"`), `mu` 0, `alternative`
  `"two_sided"` (`"greater"`: the first data set is larger), `ci_level` 0.95.
- **Checks:** values used (pairs dropped for a missing side), sample size, normality of
  each group / the paired differences / the log ratios, equal variances (informational for
  Welch), outliers (Grubbs note), effective pairing (correlation of the pairs), independence.
  Unpaired and one-sample tests also run from mean/SD/N.
- **Outputs:** Descriptive, Test (t, df, two-sided P, one-sided P with its direction),
  Difference (estimate, SE, CI), Effect size (Cohen's d, Hedges' g with approximate CIs,
  R²), Normality, Equal variances, Ratio (ratio paired), Pairing. Brackets: the two groups.
  Graph: `column` (unpaired, one-sample), `paired` preset (paired): bars with a line per subject.
- **Method:** [ttest](METHODS.md#ttest).
- **Example:**
  - Data: the `sleep` data, drug1 and drug2 as columns, paired by row.
  - Paired: the mean of the differences was 1.6 (95% CI 0.7 to 2.5), t(9) = 4.06, p = .003, Hedges' g = 1.17.
  - Welch (as unpaired groups): 1.6 (95% CI -0.2 to 3.4), t(17.8) = 1.86, p = .079.
  - The pairing removes the between-subject scatter: same difference, much narrower CI.

## mann_whitney — Mann–Whitney test

- **When:** two unpaired groups, no assumption of Gaussian distributions.
- **Tables:** column, variables.
- **Options:** `columns` (first two), `alternative` `"two_sided"`, `exact` null (automatic:
  exact without ties up to n₁n₂ = 10,000; `true` forces exact, `false` asymptotic),
  `ci_level` 0.95.
- **Checks:** values used, sample size, similar shapes (spread, by Brown–Forsythe), exact or
  approximate P, independence. Not possible from summary data.
- **Outputs:** Descriptive (medians, quartiles), Test (U, U₁, rank sums, z, P), Difference
  (Hodges–Lehmann shift with Moses CI and achieved level), Effect size (rank-biserial r),
  Equal variances. Graph: `box` with a bracket.
- **Method:** [mannwhitney](METHODS.md#rank-mannwhitney-wilcoxon).
- **Example:**
  - Data: `sleep` drug1 against drug2 as independent groups.
  - The Hodges–Lehmann shift was 1.4 (95% CI -0.1 to 3.6), U = 25.5, p = .069.
  - Medians 1.75 and 0.35; ties, so P is the normal approximation.
  - Check "Exact or approximate P" is a caution.

## wilcoxon — Wilcoxon signed-rank test

- **When:** paired values (two columns) or one column against a hypothetical median, no
  Gaussian assumption.
- **Tables:** column, variables, nested.
- **Options:** `columns` (one: against `mu`; two: paired), `mu` 0, `alternative`
  `"two_sided"`, `ci_level` 0.95.
- **Checks:** values used, sample size (6 pairs), symmetric differences (skewness), exact or
  approximate P, independence.
- **Outputs:** Descriptive (medians of each column and of the differences), Test (W+, W−,
  W, zeros dropped, z, P), Pseudo-median (Hodges–Lehmann with CI). Graph: the `paired` preset (bars with connecting lines)
  (paired) or `box`.
- **Method:** [wilcoxon](METHODS.md#rank-mannwhitney-wilcoxon).
- **Example:**
  - Data: `sleep`, paired by row.
  - The Hodges–Lehmann median of the differences was 1.3 (95% CI 0.8 to 2.8).
  - W+ = 45, W− = 0, p = .008 (normal approximation: the differences have ties).
  - Symmetry check passes.

## one_way_anova — One-way ANOVA

- **When:** three or more independent groups, compare means.
- **Tables:** column, nested.
- **Options:** `columns` [] (all), `post_hoc` Tukey all pairs (`null` for none), `welch`
  false, `brown_forsythe` false.
- **Checks:** values used, sample size, normality of the residuals, equal variances
  (Brown–Forsythe; informational with Welch), outliers, independence. Runs from mean/SD/N
  (exact), with Bartlett for the variances.
- **Outputs:** Descriptive, ANOVA table (SS, df, MS, F, P), tests not assuming equal
  variances (Welch, Brown–Forsythe), Effect size (η², ω², Cohen's f), multiple comparisons
  (Comparison, mean difference, CI, q or t, unadjusted and adjusted P, significant,
  asterisks), Normality, Equal variances. Tukey for all pairs, Dunnett for a control,
  Holm–Šidák for selected pairs, Šidák/Bonferroni/Benjamini–Hochberg on request. Brackets:
  every comparison (up to six; else the significant ones). Graph: `column`.
- **Method:** [anova](METHODS.md#anova), [posthoc](METHODS.md#posthoc).
- **Example:**
  - Data: PlantGrowth (ctrl, trt1, trt2; 10 each).
  - F(2, 27) = 4.85, p = .016, η² = .26 (ω² = .20).
  - Tukey: trt1 vs. trt2 differs (mean difference -0.87, 95% CI -1.56 to -0.17, p = .012).
  - The graph carries three brackets: p = .391, p = .198, p = .012.

## repeated_measures_anova — Repeated-measures one-way ANOVA

- **When:** each row is one subject measured under every condition (columns).
- **Tables:** column.
- **Options:** `columns` [] (all), `gg_correction` true, `post_hoc` all pairs.
- **Checks:** values used (incomplete rows dropped, counted), sample size, sphericity
  (Mauchly; pass when corrected), normality of the residuals, independence of subjects.
- **Outputs:** Descriptive, ANOVA table (treatment, subjects, residual, total), corrected
  and uncorrected tests (none, Greenhouse–Geisser, Huynh–Feldt, lower bound, with ε),
  Sphericity (Mauchly W, χ², df, P), Effect size (partial η²), paired multiple comparisons
  (paired t tests with Holm–Šidák, or the adjustment asked; CIs for Bonferroni and Šidák),
  Normality. Graph: the `paired` preset (bars with connecting lines) with brackets.
- **Method:** [anova `rm_anova`](METHODS.md#anova), [posthoc `p_adjust`](METHODS.md#posthoc).
- **Example:**
  - Data: 6 subjects × Pre, Mid, Post.
  - Greenhouse–Geisser: F(1.53, 7.65) = 61.37, p < .001, partial η² = .92, ε = .77.
  - Every pair differs after Holm–Šidák (largest adjusted p = .007).
  - Sphericity passes (corrected P in use).

## two_way_anova — Two-way ANOVA

- **When:** two factors: rows of a grouped table (factor A) and its data sets (factor B),
  replicates in subcolumns.
- **Tables:** grouped.
- **Options:** `ss_type` `"iii"` (also `"i"`, `"ii"`), `post_hoc` null (any request compares
  the cell means within each row on the pooled residual variance: Tukey for all pairs,
  else the adjustment named), `repeated` null (`"rows"`: each row's subcolumn is one
  subject measured in every data set; `"columns"`: each subcolumn of a data set is one
  subject measured in every row).
- **Checks:** values used, replicates in every cell, normality of the residuals, equal
  variances across cells, independence; with a repeated factor: sphericity stated (not
  tested), balance.
- **Outputs:** ANOVA table (Source, SS, df, MS, F, P, partial η²) of the chosen type, Cell
  means, within-row comparisons, Normality, Equal variances. The interaction sentence comes
  first. With a repeated factor: the split-plot table (interaction, repeated factor,
  between factor, subjects, residual). Graph: `column` (grouped).
- **Method:** [anova `two_way_anova`](METHODS.md#anova); the split-plot partition is the
  standard one for one between and one within factor (sequential with unequal groups).
- **Example:**
  - Data: 2 rows × 2 data sets × 3 replicates.
  - Interaction F(1, 8) = 8.02, p = .022: the column effect depends on the row.
  - Row factor F(1, 8) = 18.69, p = .003; column factor F(1, 8) = 72.20, p < .001.
  - Tukey within rows: B exceeds A in both rows (p = .004 and p < .001).

## kruskal_wallis — Kruskal–Wallis test

- **When:** three or more independent groups, no Gaussian assumption.
- **Tables:** column, nested.
- **Options:** `columns` [] (all), `dunn` true.
- **Checks:** values used, chi-square approximation (groups of 5 or more), similar shapes
  stated, independence.
- **Outputs:** Descriptive (medians, quartiles, mean ranks), Test (H, df, P, ε², tie
  correction), Dunn's comparisons with Holm adjustment. Graph: `box` with brackets.
- **Method:** [anova `kruskal_wallis`](METHODS.md#anova), [posthoc `dunn_kruskal`](METHODS.md#posthoc).
- **Example:**
  - Data: PlantGrowth.
  - H(2) = 7.99, p = .018, ε² = .28.
  - Dunn (Holm): trt1 vs. trt2 differs (p = .015).
  - The other two comparisons do not reach adjusted p < .05.

## friedman — Friedman test

- **When:** matched rows (subjects) under three or more conditions, no Gaussian assumption.
- **Tables:** column.
- **Options:** `columns` [] (all), `dunn` true.
- **Checks:** values used (incomplete rows dropped), chi-square approximation (10 subjects),
  independence.
- **Outputs:** Descriptive (medians, mean ranks), Test (χ², df, P, Kendall's W, subjects
  used and dropped), Dunn's comparisons (Holm). Graph: the `paired` preset (bars with connecting lines).
- **Method:** [anova `friedman`](METHODS.md#anova), [posthoc `dunn_friedman`](METHODS.md#posthoc).
- **Example:**
  - Data: 6 subjects × Pre, Mid, Post.
  - χ²(2, N = 6) = 12.00, p = .002, Kendall's W = 1.00 (every subject ranks the conditions alike).
  - Dunn (Holm): Pre vs. Post differs (p = .002).
  - Chi-square approximation flagged (fewer than 10 subjects).

## chi_square — Chi-square test

- **When:** association between the rows and columns of a table of counts; goodness of fit
  to equal proportions for a parts-of-whole table (or a one-column table).
- **Tables:** contingency, parts.
- **Options:** `yates` false (2 × 2 only).
- **Checks:** counts are whole numbers (the bad cell is named), expected counts (Cochran's
  rule, Fisher offered), independence of subjects.
- **Outputs:** Observed counts with totals, Expected counts, Test (Pearson χ², Yates, G, and
  Fisher's exact P for 2 × 2), Association (Cramér's V, φ), Adjusted residuals, and for 2 × 2
  the relative risk (Katz CI), odds ratio (Woolf CI) and risk difference (Wald CI).
  Goodness of fit: observed and expected, χ², df, P. Graph: `column`.
- **Method:** [contingency](METHODS.md#contingency).
- **Example:**
  - Data: 2 × 2 counts [[12, 5], [7, 15]] (rows = groups, columns = outcome).
  - Risk 0.706 vs. 0.318: relative risk 2.2 (95% CI 1.1 to 4.4).
  - χ²(1, N = 39) = 5.77, p = .016, Cramér's V = .38; Fisher p = .025 shown alongside.
  - Expected counts pass (smallest 8.28).

## fisher_exact — Fisher's exact test

- **When:** a 2 × 2 table, especially with small counts; r × c tables by the
  Fisher–Freeman–Halton test within its work limit.
- **Tables:** contingency (2 or more data sets).
- **Options:** `alternative` `"two_sided"` (`"greater"`: odds ratio > 1; 2 × 2 only).
- **Checks:** counts are whole numbers, fixed margins stated, independence.
- **Outputs:** Observed counts, Test (two-sided and one-sided P, sample and conditional MLE
  odds ratios with the exact CI), Effect size (relative risk, odds ratio, risk difference).
  r × c: exact P, probability of the observed table, number of tables, Cramér's V; a clear
  message when the table is too large. Graph: `column`.
- **Method:** [contingency `fisher_exact_2x2`, `fisher_exact_rxc`](METHODS.md#contingency).
- **Example:**
  - Data: the lady tasting tea, [[3, 1], [1, 3]].
  - Two-sided p = .486.
  - Conditional MLE odds ratio 6.41 (95% CI 0.21 to 626).
  - Relative risk 3.0 (95% CI 0.50 to 18.0).

## correlation — Correlation

- **When:** association between two variables, or a matrix of several.
- **Tables:** XY (X with each data set by default), variables (numeric variables), column
  (data sets paired by row).
- **Options:** `columns` [] (all), `method` `"pearson"` (also `"spearman"`, `"kendall"`),
  `ci_level` 0.95.
- **Checks:** values used (pairwise deletion counted), sample size, normality of each
  variable (Pearson), linear or monotonic relationship stated, outliers, independence.
- **Outputs:** Correlation table (pair, n, r, CI, r², P, Holm-adjusted P for several pairs),
  the matrix for more than two variables, Normality. Graph: `scatter` of the pair, or
  `correlation_matrix`.
- **Method:** [correlation](METHODS.md#correlation).
- **Example:**
  - Data: X = 1…10, Y = 2.1, 3.9, …, 19.7.
  - r(8) = .999 (95% CI .998 to 1.000), p < .001, n = 10.
  - Spearman on request: ρ with the Bonett–Wright CI.
  - Linearity is stated, not tested: look at the scatter plot.

## linear_regression — Simple linear regression

- **When:** a straight line through XY data, each data set separately.
- **Tables:** XY (replicates are separate points; mean/SD/N rows give their means).
- **Options:** `datasets` [] (all), `through_origin` false, `bands` `"none"` (also
  `"confidence"`, `"prediction"`, `"both"`, for graphs), `ci_level` 0.95, `residuals`
  false (adds a residuals table).
- **Checks:** linearity (runs test), replicates test, normality of the residuals, equal
  scatter (|residual| against X), influential points (standardized residual, Cook's
  distance), X without error stated, independence (Durbin–Watson shown).
- **Outputs:** Best-fit values (slope, intercepts with SE, CI, t, P), Goodness of fit (n,
  R², Sy.x, SS, F, P, Durbin–Watson), Runs test, Replicates test, Normality, Residuals.
  Graph: `scatter` with the line and its band (linear and log X).
- **Method:** [regression `simple_linear_regression`](METHODS.md#regression).
- **Example:**
  - Data: X = 1…10, Y = 2.1, 3.9, …, 19.7.
  - Slope 1.99 (95% CI 1.94 to 2.05), Y-intercept 0.060 (95% CI -0.280 to 0.400).
  - R² = .999, F(1, 8) = 7052.15, p < .001.
  - Runs test passes.

## nonlinear_regression — Nonlinear regression (curve fit)

- **When:** fit a model (dose-response, exponential, enzyme kinetics, binding, growth, or a
  formula) to XY data.
- **Tables:** XY.
- **Options:** `datasets` [] (all, each fitted separately), `equation` `{"name":
  "four_parameter_logistic"}` (snake_case names such as `one_phase_decay`,
  `michaelis_menten`, `log_inhibitor_variable_slope`, Prism names and the aliases 4PL, 3PL,
  5PL, line, MM; or `{"name": "...", "formula": "Y = A*exp(-K*X) + C"}`), `weighting`
  `"none"` (also `"relative_y2"` = 1/Y², `"poisson_y"` = 1/Y, `"inverse_sd2"` = 1/SD² of
  the replicates), `robust` false, `constraints` [] (`{"kind": "fixed" | "min" | "max",
  "param", "value"}`, `{"kind": "shared", "param"}` for a global fit), `initial` {}
  (by parameter name), `compare_with` null (a second equation), `profile_ci` false,
  `rout_q` null (ROUT outlier removal at Q percent), `ci_level` 0.95, `bands`
  `"confidence"` (for graphs).
- **Checks:** values used, convergence (hit constraint, ambiguous, not converged), runs
  test, replicates test, normality of the residuals, outliers (ROUT when asked, otherwise
  stated), X as log(concentration) for the log equations, scatter of Y stated,
  independence.
- **Outputs:** Best-fit values (value, SE, CI, profile CI when asked, status, dependency),
  Derived values (EC50 from LogEC50, half-life, Span…), Goodness of fit (n, df, R², adjusted
  R², SS, Sy.x, AICc), Convergence, Runs test, Replicates test, Outliers (ROUT), Model
  comparison (extra-sum-of-squares F test and AICc with probabilities and the preferred
  model), Normality. Graph: `scatter` with each curve and its band (linear and log X).
- **Method:** [nonlinear](METHODS.md#nonlinear-curve-fitting), [equations](METHODS.md#equations-curve-fit-equation-library).
- **Example:**
  - Data: 4PL dose-response, 11 log concentrations in duplicate.
  - LogEC50 = -6.45 (95% CI -6.54 to -6.35), so EC50 = 3.58E-7 (95% CI 2.88E-7 to 4.45E-7).
  - HillSlope = 0.75 (95% CI 0.63 to 0.87); R² = .995.
  - Against the fixed-slope model: F(1, 18) = 14.52, p = .001; AICc prefers the variable slope (99.2%).

## normality — Normality tests

- **When:** look at the distribution of each data set.
- **Tables:** column, XY, grouped, variables, nested.
- **Options:** `columns` [] (all), `tests` `["shapiro_wilk", "dagostino_pearson"]` (also
  `"anderson_darling"`, `"kolmogorov_smirnov"` = Lilliefors).
- **Checks:** values used (tests that cannot run are named), power of the tests (n < 8 or
  n > 500), what the tests mean.
- **Outputs:** Normality tests (data set, test, n, statistic, P, passed), Descriptive (mean,
  SD, skewness, excess kurtosis). Graph: `qq`.
- **Method:** [normality](METHODS.md#normality).
- **Example:**
  - Data: PlantGrowth.
  - ctrl: Shapiro–Wilk W = 0.96, p = .747; D'Agostino–Pearson K² = 0.23, p = .890.
  - All three groups are consistent with a Gaussian distribution.
  - That is absence of evidence, not proof.

## outliers — Identify outliers

- **When:** look for values that may be errors, one data set at a time.
- **Tables:** column, XY, grouped, variables, nested.
- **Options:** `columns` [] (all), `method` `{"method": "rout", "q": 1}` (Q in percent) or
  `{"method": "grubbs", "alpha": 0.05}`.
- **Checks:** values used, normality of the values without the flagged ones, decision to
  exclude (flagged values are never removed).
- **Outputs:** Flagged values (data set, row, cell, value), Before and after (n, flagged,
  mean and SD with and without), Grubbs' steps, Normality. Graph: `strip`.
- **Method:** [outliers](METHODS.md#outliers).
- **Example:**
  - Data: 10.1, 9.8, 10.3, 10.0, 9.9, 10.2, 25, 10.1.
  - ROUT (Q = 1%) flags 25 (row 7); so does Grubbs at α = 0.05.
  - Without it M = 10.1, SD = 0.172 (with it M = 11.9, SD = 5.29).
  - The value stays in the table until you exclude it.

## survival — Survival curves (Kaplan–Meier)

- **When:** time to an event with censoring, one or more groups.
- **Tables:** survival (X = time; each data set 1 = event, 0 = censored).
- **Options:** `compare` `"log_rank"` (also `"gehan_breslow_wilcoxon"`, which leads the
  sentence; both are always shown), `ci_level` 0.95.
- **Checks:** values used, event codes (the bad cell is named), number of events,
  proportional hazards (stated, not tested), uninformative censoring, independence.
- **Outputs:** a Kaplan–Meier table per group (time, at risk, events, censored, percent
  survival, SE, CI), Median survival with the Brookmeyer–Crowley CI, Curve comparison
  (log-rank, Gehan–Breslow–Wilcoxon, trend for three or more groups), Observed and expected
  events, Hazard ratio both ways (Mantel–Haenszel and log-rank methods). Graph:
  `kaplan_meier` (step curves, censor ticks, CI bands, numbers at risk).
- **Method:** [survival](METHODS.md#survival).
- **Example:**
  - Data: Gehan's leukaemia trial, 6-MP (21 patients, 9 relapses) and placebo (21, 21).
  - Median survival 23 weeks with 6-MP (95% CI 13 to undefined), 8 with placebo (95% CI 4 to 11).
  - Log-rank χ²(1) = 16.79, p < .001.
  - Hazard ratio (6-MP / placebo, Mantel–Haenszel) 0.19 (95% CI 0.09 to 0.43).

## roc — ROC curve

- **When:** how well a measurement separates patients from controls; choose a cut-off.
- **Tables:** column (controls first, then patients), variables (a score variable and a
  category variable with `positive_label`).
- **Options:** `columns` (first two), `positive_label` null, `ci_level` 0.95.
- **Checks:** direction (AUC < 0.5 means the columns are the wrong way round), sample size,
  representative samples stated, independence.
- **Outputs:** Area under the curve (DeLong and Hanley–McNeil SEs and CIs), test of AUC =
  0.5, Cut-offs (sensitivity and specificity with Wilson CIs, LR+, LR−, Youden J), Best
  cut-offs (Youden, closest to (0, 1)). Graph: `roc`.
- **Method:** [roc](METHODS.md#roc).
- **Example:**
  - Data: 10 controls (1.2 … 2.0) and 10 patients (3.4 … 5.2).
  - AUC = .90 (95% CI .77 to 1.00, DeLong), p < .001 against chance.
  - Youden cut-off ≥ 3.3: sensitivity 80.0%, specificity 90.0%.
  - Direction check passes.

## bland_altman — Bland–Altman

- **When:** agreement between two methods measured on the same subjects.
- **Tables:** column, variables (two data sets paired by row).
- **Options:** `columns` (first two), `method` `"difference"` (also `"ratio"`,
  `"percent_difference"`), `ci_level` 0.95.
- **Checks:** values used (unpaired rows dropped), sample size (30 pairs), normality of the
  differences, proportional bias, acceptable agreement stated, independence.
- **Outputs:** Bias and limits of agreement (bias, SD, lower and upper limits, each with
  CI), Proportional bias (slope of the difference on the mean), Normality. Graph:
  `bland_altman` (differences), `scatter` with bias and limit lines (ratio, percent).
- **Method:** [agreement `bland_altman`](METHODS.md#agreement).
- **Example:**
  - Data: the ROC example's two columns read as methods A and B.
  - Bias (A − B) -1.6 (95% CI -2.3 to -0.9).
  - Limits of agreement -3.4 (95% CI -4.6 to -2.2) to 0.20 (95% CI -0.96 to 1.36).
  - No proportional bias (p = .458).

## pca — Principal component analysis

- **When:** summarise several correlated numeric variables by a few components.
- **Tables:** variables.
- **Options:** `columns` [] (all numeric), `standardize` true (correlation matrix),
  `components` null (chosen by Horn's parallel analysis).
- **Checks:** values used (incomplete rows dropped), rows per variable (5 or more),
  comparable scales when not standardized, linear relationships stated, independence.
- **Outputs:** Eigenvalues (% of variance, cumulative %, parallel-analysis 95th percentile,
  retained), Loadings, Eigenvectors, Scores. Graph: `scatter` (scores on PC1 and PC2);
  `column` gives the scree plot.
- **Method:** [multivariate](METHODS.md#multivariate).
- **Example:**
  - Data: three variables on 10 cases, two of them strongly correlated with each other and
    negatively with the third.
  - The first component explains 97.1% of the variance.
  - Parallel analysis and Kaiser's rule both retain one component.
