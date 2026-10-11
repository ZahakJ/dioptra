# Correctness

Generated from `cargo run -p dioptra-stats --example correctness_report` (the same
comparisons as `cargo test -p dioptra-stats`), then annotated. This page covers the first
half of the engine: special functions, distributions, descriptive statistics, t tests,
Mann–Whitney, Wilcoxon, normality tests and outliers. ANOVA, multiple comparisons,
contingency, correlation, regression, survival, ROC, agreement and PCA are added by the
next stage and appended to the same report.

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
