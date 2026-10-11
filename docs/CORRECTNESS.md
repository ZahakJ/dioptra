# Correctness

Generated from `cargo run -p dioptra-stats --example correctness_report` (the same
comparisons as `cargo test -p dioptra-stats`), then annotated. This page covers special
functions, distributions, descriptive statistics, t tests, Mann–Whitney, Wilcoxon,
normality tests, outliers, ANOVA (one-way, two-way, repeated measures, Kruskal–Wallis,
Friedman), multiple comparisons, contingency tables, correlation and linear regression.
Survival, ROC, agreement, PCA and nonlinear regression are appended by later stages.

Every comparison uses the tolerance stored with its fixture case
(`research/05-nist-and-oracle-notes.md` §3); exact discrete p-values (Mann–Whitney,
Wilcoxon) are tightened to 1e-12 relative as that policy asks.

## Oracle fixtures

| Family | Cases | Comparisons | Failures | Worst error / tolerance | Max rel. error | Min digits |
|---|---:|---:|---:|---:|---:|---:|
| incomplete beta + inverse | 10 | 315 | 0 | 4.5e-1 | 4.5e-13 | 12.3 |
| incomplete gamma + inverse, lgamma, digamma | 8 | 184 | 0 | 1.7e-1 | 1.7e-13 | 12.8 |
| normal | 1 | 82 | 0 | 4.0e-3 | 4.0e-16 | 15.4 |
| t, F, chi-square | 27 | 1086 | 0 | 1.5e-1 | 1.5e-13 | 12.8 |
| binomial, hypergeometric | 9 | 177 | 0 | 4.9e-2 | 7.3e-15 | 14.1 |
| noncentral t | 6 | 66 | 0 | 7.8e-3 | 7.8e-13 | 12.1 |
| studentized range | 90 | 855 | 0 | 5.1e-2 | 8.4e-6 | 5.1 |
| Dunnett distribution | 2 | 12 | 0 | 2.7e-5 | 2.0e-9 | 8.7 |
| descriptive | 12 | 385 | 0 | 1.8e-1 | 2.4e-13 | 12.6 |
| t tests | 21 | 235 | 0 | 9.5e-5 | 3.6e-12 | 11.4 |
| Mann-Whitney | 7 | 96 | 0 | 2.2e-4 | 1.4e-15 | 14.8 |
| Wilcoxon signed-rank | 6 | 165 | 0 | 1.6e-7 | 2.2e-15 | 14.7 |
| normality | 8 | 180 | 0 | 1.1e-5 | 1.3e-13 | 12.9 |
| outliers (Grubbs, ROUT) | 5 | 207 | 0 | 2.9e-1 | 2.9e-9 | 8.5 |
| one-way ANOVA (+ Welch, Brown-Forsythe, Bartlett, Levene) | 5 | 194 | 0 | 3.5e-7 | 5.1e-15 | 14.3 |
| two-way ANOVA (types I, II, III) | 2 | 140 | 0 | 4.9e-6 | 4.9e-14 | 13.3 |
| repeated-measures ANOVA, sphericity, Friedman | 3 | 81 | 0 | 3.9e-6 | 3.9e-14 | 13.4 |
| Kruskal-Wallis, Friedman, Dunn | 7 | 198 | 0 | 4.9e-6 | 4.9e-14 | 13.3 |
| Tukey-Kramer, Dunnett, p-value adjustment | 8 | 377 | 0 | 2.3e-4 | 2.0e-9 | 8.7 |
| sanity: statsmodels qsturng, SciPy QMC Dunnett | 8 | 48 | 0 | 1.9e-1 | 2.9e-1 | 0.5 |
| contingency (chi-square, Fisher, McNemar, trend) | 21 | 175 | 0 | 7.6e-6 | 7.6e-14 | 13.1 |
| correlation (Pearson, Spearman, Kendall) | 6 | 89 | 0 | 3.7e-7 | 1.8e0 | -0.3 |
| linear regression (simple, bands, runs, Deming, multiple) | 5 | 254 | 0 | 4.0e-3 | 8.7e-14 | 13.1 |

Worst error / tolerance: the largest |got - expected| / max(abs, rel |expected|) over
the family, with each case's own tolerance (< 1 passes). Max rel. error is over expected
values with |value| > 1e-290, including tiny values where the abs tolerance governs;
min digits = -log10(max rel. error).

## NIST StRD univariate (LRE, McCullough 1998)

| Dataset | Difficulty | Mean | SD | Lag-1 r | Mean* | SD* | Lag-1 r* |
|---|---|---:|---:|---:|---:|---:|---:|
| PiDigits | Lower | 15.0 | 15.0 | 14.9 | 15.0 | 15.0 | 14.7 |
| Lottery | Lower | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| Lew | Lower | 15.0 | 15.0 | 14.8 | 15.0 | 15.0 | 15.0 |
| Mavro | Lower | 15.0 | 13.1 | 13.9 | 15.0 | 15.0 | 15.0 |
| Michelso | Lower | 15.0 | 13.8 | 13.4 | 15.0 | 15.0 | 15.0 |
| NumAcc1 | Lower | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| NumAcc2 | Average | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| NumAcc3 | Average | 15.0 | 9.5 | 12.2 | 15.0 | 15.0 | 15.0 |
| NumAcc4 | Higher | 15.0 | 8.3 | 11.0 | 15.0 | 15.0 | 15.0 |

LRE capped at 15 (NIST certifies 15 digits). Columns without a star score against the NIST
certified values; starred columns against the exact statistics of the inputs as parsed into
doubles (rational arithmetic), which separates the algorithm from input rounding: the
NumAcc3/NumAcc4 values (1e6 + 0.1 and 1e7 + 0.1 steps) are not representable in binary,
which caps any double-precision SD at 9.5 and 8.3 digits against the certified 0.1.
Minimum LRE: 8.25 against certified values, 14.74 against the exact statistics of the double inputs.

## NIST StRD linear regression (LRE)

| Dataset | Difficulty | Parameters | SDs | R^2 | Residual SD | ANOVA table | Min | Min* |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Norris | Lower | 14.1 | 13.9 | 15.0 | 14.0 | 13.7 | 13.7 | 15.0 |
| Pontius | Lower | 13.5 | 13.8 | 15.0 | 13.8 | 13.5 | 13.5 | 15.0 |
| NoInt1 | Average | 14.7 | 15.0 | 15.0 | 15.0 | 14.7 | 14.7 | 15.0 |
| NoInt2 | Average | 15.0 | 14.9 | 15.0 | 15.0 | 14.5 | 14.5 | 15.0 |
| Filip | Higher | 14.0 | 14.8 | 15.0 | 14.8 | 14.4 | 14.0 | 15.0 |
| Longley | Higher | 14.6 | 14.9 | 15.0 | 15.0 | 14.8 | 14.6 | 15.0 |
| Wampler1 | Higher | 15.0 | 15.0 | 15.0 | 15.0 | 14.7 | 14.7 | 15.0 |
| Wampler2 | Higher | 13.2 | 15.0 | 15.0 | 15.0 | 14.6 | 13.2 | 15.0 |
| Wampler3 | Higher | 15.0 | 14.5 | 15.0 | 14.8 | 14.7 | 14.5 | 15.0 |
| Wampler4 | Higher | 15.0 | 14.5 | 15.0 | 14.8 | 14.7 | 14.5 | 15.0 |
| Wampler5 | Higher | 15.0 | 14.4 | 13.8 | 14.8 | 13.7 | 13.7 | 13.8 |

Parameters and SDs: minimum over the coefficients. ANOVA table: minimum over the
regression and residual SS and MS and F (Wampler1's F is infinite and not scored).
Min*: minimum LRE against the exact least-squares solution for the inputs as parsed
into doubles (rational arithmetic; values below 1e-12 scored by absolute error).

## NIST StRD one-way ANOVA (LRE)

| Dataset | Difficulty | SS between | SS within | MS between | MS within | F | R^2 | Residual SD | Min | Min* |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| SiRstv | Lower | 14.0 | 13.1 | 14.0 | 13.1 | 13.1 | 13.2 | 13.4 | 13.1 | 15.0 |
| SmLs01 | Lower | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| SmLs02 | Lower | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| SmLs03 | Lower | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| AtmWtAg | Average | 10.2 | 10.9 | 10.2 | 10.9 | 10.2 | 10.3 | 11.2 | 10.2 | 15.0 |
| SmLs04 | Average | 10.1 | 10.3 | 10.1 | 10.3 | 10.4 | 10.7 | 10.6 | 10.1 | 15.0 |
| SmLs05 | Average | 9.9 | 10.3 | 9.9 | 10.3 | 10.2 | 10.5 | 10.6 | 9.9 | 15.0 |
| SmLs06 | Average | 9.9 | 10.3 | 9.9 | 10.3 | 10.2 | 10.5 | 10.6 | 9.9 | 15.0 |
| SmLs07 | Higher | 4.0 | 4.3 | 4.0 | 4.3 | 4.4 | 4.7 | 4.6 | 4.0 | 15.0 |
| SmLs08 | Higher | 3.9 | 4.3 | 3.9 | 4.3 | 4.2 | 4.5 | 4.6 | 3.9 | 15.0 |
| SmLs09 | Higher | 3.9 | 4.3 | 3.9 | 4.3 | 4.2 | 4.4 | 4.6 | 3.9 | 15.0 |

Min*: minimum LRE against the exact statistics of the inputs as parsed into doubles.

On the linear sets the refined QR (equilibrated, pivoted Householder plus double-double
refinement of the augmented system) returns the correctly rounded least-squares solution
of the double inputs everywhere (Min* = 15) except Wampler5's R^2 = 0.0022, which is formed
as a difference (13.8). Against the certified values the remaining gap is input rounding:
Wampler2's y values and Pontius's x values are decimal fractions that binary cannot hold.
Filip reaches 14.0 (McCullough's "acceptable" bar is 7; normal equations score 0, and
plain Householder QR about 7-8). The simple-regression routine reaches 13.9 on Norris and
14.7 on NoInt1/NoInt2 through the same solver (`tests/nist_linear.rs::nist_simple_path`).

On the ANOVA sets every statistic is correctly rounded for the double inputs (Min* = 15:
sums of squares on data shifted by the first observation, two-pass means, compensated
sums). Against the certified values, SmLs04-09 are limited by their inputs: 1000000.4 and
1000000000000.4 are not representable in binary. On the parsed doubles the exact
SS_between of SmLs05 is 16.080000001825 (certified 16.08, so at most LRE 9.94) and of SmLs07
1.6801563 (certified 1.68, at most 4.03); no double-precision program can do better on these
inputs. AtmWtAg (7 constant digits) reaches 10.2 for the same reason. The tests hold the
lower sets to 13, the average sets to 10 (9.9 for the input-capped SmLs05/06) and every
set to 13 against the exact statistics of its double inputs.

## Notes on the fixtures

- **Studentized range, far upper tail.** The worst relative errors in that row (8e-6) are
  in the SciPy-generated reference, not in Dioptra. For k = 2 the studentized range has a
  closed form, P(Q > q) = P(|T_df| > q/sqrt 2). At q = 8, df = 1000 that is
  2.0119785627859e-8 (40-digit mpmath and our t distribution agree); the fixture holds
  2.0119947042e-8 (SciPy `studentized_range.sf`, off by 8.0e-6 relative), and Dioptra gives
  2.01197856277e-8 (1e-11). The case passes on the fixture's absolute tolerance (1e-11).
  `tests/distributions.rs::quadrature_identities` pins k = 2 to the t distribution at
  1e-10 relative, and the ignored test `studentized_range_vs_nested_quadrature` checks
  k = 2..20, df = 1..1000 and inf against nested adaptive quadrature (worst: 1e-13
  absolute on the cdf, 1e-11 relative on the upper tail).
- **Dunnett.** The fixture's own double integral is accurate to about 2e-9 relative
  (p_greater = 1.3658528033e-5 for the unequal-n case); nested adaptive quadrature gives
  1.36585280593e-5, which Dioptra matches to 3e-14. Dunnett with one treatment equals the
  t test exactly and is tested that way.
- **ROUT robust centre.** The fixture's centre comes from SciPy's `minimize_scalar`; its
  merit gradient there is about 1e-8, ours about 1e-13, and the two centres differ by up to
  3e-10 relative (tolerance 1e-9). Flagged indices agree everywhere.
- **Noncentral t.** All six tables pass at 1e-10, including x = -3, df = 5, nc = 8, where
  the reference is 1.899970783e-20 and SciPy returns 8.0e-20.
- **Shapiro–Wilk.** W and p agree with SciPy to 1.3e-13 even though the fixture tolerance
  is 1e-7 / 1e-6 (the AS R94 polynomials are single precision; we use the full-precision
  normal scores, as R does, and so does SciPy).
- **NIST NumAcc3/NumAcc4.** The SD LREs of 9.5 and 8.3 against the certified 0.1 are the
  limit set by the inputs: 1000000.1 and 10000000.1 are not representable in binary, and
  the exact SD of the parsed doubles is 0.10000000003492 and 0.10000000055879. Dioptra
  reproduces those exact values to 15 digits (starred columns).

- **Spearman, perfect correlation (correlation row).** For `n5` the ranks agree exactly,
  so rho = 1 and the t-approximation p is 0. SciPy reports 1.40e-24 because its rho
  rounds to 0.9999999999999999; Dioptra's rounds to a slightly different neighbour of 1
  and gives a p of the same order. The case passes on the absolute tolerance (1e-10); it
  is the only reason the correlation row shows a relative error above 1. Every other
  correlation comparison, including the exact Spearman and Kendall p-values, agrees to
  2e-13 relative or better.
- **Tukey–Kramer, Dunnett, p-value adjustment.** The 2.0e-9 relative error is the
  fixture's own Dunnett double integral (see the Dunnett note above); Tukey p-values agree
  to 8e-12 (the worst is a tail p of 5.3e-5), Tukey critical values and CIs to 1e-13, and
  all seven p-adjustment methods to 1e-13. The loose
  library cross-checks stored in the same fixture (statsmodels' `qsturng` interpolation,
  SciPy's randomized-QMC `dunnett` p and CI, tolerance 2e-3) are reported on their own
  "sanity" row so that they do not mask the exact comparisons; SciPy's QMC Dunnett p for
  the unequal-n case is 3.8e-5 against the exact 2.73e-5.
- **Two-way ANOVA trap.** The fixture's `type3_treatment_coding_WRONG_for_reference`
  (type III with dummy coding) is checked to *differ* from Dioptra's sum-to-zero type III
  on the unbalanced case; matching it would be the classic error.
- **Fisher r x c.** The network algorithm is checked against plain enumeration of every
  table on 60 pseudo-random tables (p to 1e-10, table counts exactly,
  `contingency::tests::network_matches_enumeration`) and matches all five fixture tables,
  including the transposed 3 x 2 and the 2 x 2 that equals `fisher_exact`. The conditional
  MLE odds ratio and its exact CI agree with SciPy to 1e-13.
- **Exact discrete p-values.** Runs-test p (Swed–Eisenhart), Kendall and Spearman exact p
  and Fisher p are compared at 1e-12 relative or tighter where the fixture tolerance is
  looser, as the tolerance policy asks for rational quantities.

## Performance (release build, one core)

From `cargo test -p dioptra-stats --release --test benches -- --ignored --nocapture`:

| Operation | Time |
|---|---:|
| betainc(2.5, 7.5, x) body | 0.169 us |
| gammainc_p(4.5, x) body | 0.095 us |
| betainc(50, 50, x) body | 0.198 us |
| betainc_inv(2, 3, p) | 0.845 us |
| norm_quantile(p) | 0.008 us |
| Shapiro-Wilk n=5000 | 95.727 us |
| Mann-Whitney exact 50x50 | 1125.738 us |
| Mann-Whitney exact 100x100 | 14340.008 us |
| StudentizedRange(k=5, df=20).cdf_sf | 1139.164 us |
| StudentizedRange(k=10, df=60).cdf_sf | 1469.011 us |
| StudentizedRange::new(5, 20) (node setup) | 16.721 us |
| StudentizedRange(k=5, df=20).ppf(0.95) | 17279.177 us |
| Dunnett(k=3, unequal).p_value | 7146.145 us |
| Dunnett(k=3, unequal).quantile(0.95) | 76594.293 us |
| NoncentralT(10, 1.5).cdf (AS 243) | 0.331 us |
| NoncentralT(5, 8).cdf(-3) (tail quadrature) | 27.356 us |
| Fisher r x c 3x5, N = 150 | 13.6 ms |
| Fisher r x c 4x5, N = 200 (near independence) | 353 ms |
| Fisher r x c 3x10, N = 200 | 31.7 ms |
| Fisher r x c 5x6, N = 200 (near independence) | 561 ms |
| Fisher r x c 5x6, N = 200, strongly associated | exceeds the work/path limits; returns `NoConvergence` after 1.0 s, 154 MB peak |
| Tukey-Kramer, k = 10 (45 pairs, critical q) | 75 ms |

The r x c limits are the `FISHER_RXC_WORK_LIMIT` (5e7 path-child expansions) and
`FISHER_RXC_PATH_LIMIT` (4e6 stored paths per stage). Tables near independence finish well
inside them up to 5 x 6 with N = 200; strongly associated tables of that size do not (R's
FEXACT also needs an enlarged workspace or simulation there), and the chi-square test is
the fallback. Prism offers Fisher's test for 2 x 2 tables only. The Tukey time is almost all
the studentized-range quantile for the simultaneous CI.
