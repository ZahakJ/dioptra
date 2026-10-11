# Methods

One line per public function of `dioptra-stats`: what it computes, the reference it
implements, and the conventions that differ between Prism and SciPy. Conventions that hold
everywhere:

- `NaN` is a missing value. Descriptive statistics and tests skip it; paired tests drop
  incomplete pairs; outlier indices keep the caller's positions. Infinities are rejected
  with `StatsError::InvalidArgument`.
- Two-sample differences are `x - y`. Prism shows "B - A" (later column minus earlier); the
  presentation layer flips the sign where it mimics Prism.
- Every test returns the two-sided p and both one-sided p-values (`p_less`, `p_greater`)
  plus `p` for the requested alternative. Prism's "one-tailed" p is `p_less` or `p_greater`
  for the predicted direction.
- Upper tails are computed directly, never as `1 - cdf`; quantiles invert whichever tail
  is smaller.
- The low-level `special` kernels return `NaN` for invalid arguments; distribution
  constructors and tests return `StatsError`.

## special (special functions)

| Function | Reference | Notes |
|---|---|---|
| `ln_gamma(x)` | fdlibm `lgamma_r` (via `libm`) | ln abs Gamma(x), about 1 ulp. |
| `gamma(x)` | musl `tgamma` (via `libm`) | |
| `digamma(x)` | Abramowitz & Stegun 6.3.5, 6.3.7, 6.3.18 | Recurrence to x >= 10, asymptotic series; reflection for x < 0. |
| `ln_beta(a, b)` | R `lbeta` (Ihaka) | Stirling-correction form when max(a,b) >= 8, no cancellation at a = b = 5000. |
| `beta(a, b)` | exp(`ln_beta`) | |
| `ln_choose(n, k)` | via `ln_beta` | Real n, k. |
| `erf(x)`, `erfc(x)` | fdlibm (via `libm`) | erfc keeps relative accuracy in the tail. |
| `log1p(x)`, `expm1(x)` | IEEE (std) | |
| `log1pmx(x)` | R `log1pmx` (Welinder) | ln(1+x) - x without cancellation. |
| `stirling_error(z)` | Loader (2000), R `stirlerr`; A&S 6.1.40 | ln Gamma(z) minus Stirling's formula. |
| `binomial_deviance(x, np)` | Loader (2000), R `bd0` | |
| `binomial_pmf_raw(x, n, p, q)` | Loader (2000), R `dbinom_raw` | Saddle-point binomial pmf, q supplied separately. |
| `ln_gamma_prefactor(a, x)` | Temme (1987); DiDonato & Morris (1986) `rcomp` | ln(x^a e^-x / Gamma(a)), Stirling form for a >= 8. |
| `gammainc_pair(a, x)` | Numerical Recipes 3rd ed. §6.2; DiDonato & Morris (1986), TOMS 654 | (P, Q): series for x < a+1, Lentz continued fraction for Q otherwise. |
| `gammainc_p`, `gammainc_q` | as `gammainc_pair` | Regularised lower / upper incomplete gamma. |
| `gammainc_inv_pq(a, p, q)`, `gammainc_inv`, `gammaincc_inv` | DiDonato & Morris (1986) §5; Wilson & Hilferty (1931) start | Bracketed Halley iteration on the smaller tail. |
| `ln_beta_prefactor(a, b, x, y)` | DiDonato & Morris (1992), TOMS 708 `brcomp`; Loader (2000) | ln(x^a y^b / B(a,b)), expanded around the mean when a, b >= 8. |
| `betainc_pair_xy(a, b, x, y)` | Numerical Recipes 3rd ed. §6.4 (modified Lentz); TOMS 708 ideas (symmetry, `bpser`, `brcomp`) | (I, 1 - I) with y = 1 - x supplied; switch at x = (a+1)/(a+b+2). |
| `betainc_pair`, `betainc`, `betaincc` | as above | Regularised incomplete beta and its complement. |
| `betainc_inv_pq(a, b, p, q)`, `betainc_inv`, `betaincc_inv` | Numerical Recipes 3rd ed. §6.14 start (A&S 26.5.22); bracketed Halley | Returns (x, 1 - x); solves for the smaller of x and 1 - x. |
| `norm_quantile(p)`, `norm_quantile_upper(q)`, `norm_quantile_pq` | Wichura (1988), AS 241 PPND16 | About 1e-16; R's `qnorm` algorithm. Upper tail by symmetry, so tiny q is exact. |
| `norm_quantile_ppnd7(p)` | Wichura (1988), AS 241 PPND7 | Single-precision variant, kept for reference. |
| `erfc_inv(y)`, `erf_inv(x)` | via AS 241 | Central range uses the PPND16 rational directly (no 1 + x rounding). |

## dist (distributions)

Each continuous distribution implements `Continuous` (`pdf`, `cdf`, `sf`, `ppf`, `isf`) and
has a `cdf_sf` or `quantile_pq` method taking both tails.

| Function | Reference | Notes |
|---|---|---|
| `std_normal_pdf`, `std_normal_ln_pdf` | R `dnorm` argument splitting | Full relative accuracy for large z. |
| `std_normal_cdf`, `std_normal_sf` | erfc (fdlibm) with argument-rounding correction; Laplace continued fraction for Mills' ratio beyond 37 (A&S 26.2.14) | Relative accuracy to the underflow limit. |
| `std_normal_ln_cdf`, `std_normal_ln_sf` | ln phi + ln Mills' ratio for z < -5 | |
| `Normal` | as above; AS 241 quantile | |
| `StudentT` (`cdf_sf`, `two_sided_p`, `quantile_pq`) | A&S 26.7.1 via the incomplete beta | Non-integer df; df = inf is the normal. |
| `ChiSquared`, `Gamma` | regularised incomplete gamma | |
| `FisherF` | A&S 26.6.2 via the incomplete beta | Beats SciPy's `f.isf` in the far upper tail. |
| `Beta` | regularised incomplete beta | |
| `Binomial` | Loader (2000) pmf; A&S 26.5.24 cdf/sf | |
| `Hypergeometric` | R `dhyper` (Loader kernel), R `pdhyper` summation | SciPy's `hypergeom(M, n, N)` parameter order: population, successes, draws. |
| `NoncentralT` | Lenth (1989), AS 243; Guenther-style reflection for t < 0 | Tails below 1e-6 (and nc > 37 or df > 4e5) by adaptive Gauss–Kronrod quadrature of integral Phi(tu - delta) f(u) du, which keeps relative accuracy where AS 243 and SciPy's `nct` fail (x = -3, df = 5, nc = 8: true 1.90e-20, SciPy 8.0e-20). |
| `StudentizedRange` | Copenhaver & Holland (1988); Lund & Lund (1983), AS 190 | Inner composite Gauss–Legendre, outer adaptive Gauss–Kronrod over the chi scale; upper tail integrated directly. About 1.2-1.5 ms per cdf. |
| `Dunnett` (`new`, `balanced`, `from_group_sizes`, `cdf_sf`, `p_value`, `quantile`) | Dunnett (1955); product-correlation reduction rho_ij = lambda_i lambda_j | Deterministic double integral (not SciPy's randomized QMC, which is reproducible only to about 1e-3). Two-sided and one-sided; balanced and unbalanced. |
| `p_two_sided_t`, `p_less_t`, `p_greater_t` | Student t tails | |
| `p_two_sided_z`, `p_less_z`, `p_greater_z` | normal tails | |
| `p_upper_chi2`, `p_upper_f` | upper tails, computed directly | |
| `t_critical_two_sided(alpha, df)` | t quantile | |

## descriptive

| Function | Reference | Notes |
|---|---|---|
| `compensated_sum` | Neumaier (1974) | |
| `clean`, `count` | | Drop NaN; reject infinities. |
| `sum`, `mean` | Higham (2002) two-pass corrected mean | |
| `variance`, `sd`, `sem`, `sd_population`, `cv` | Chan, Golub & LeVeque (1983) corrected two-pass | n - 1 denominators (Prism, SciPy `ddof=1`); NumPy's default is n. CV is a fraction (Prism shows %). |
| `min`, `max`, `range`, `median` | | |
| `quantile`, `quantile_sorted`, `percentile`, `quartiles` | Hyndman & Fan (1996) | `QuantileMethod::Prism` (type 6, (n+1)p, the default) vs `Linear` (type 7, SciPy/NumPy/Excel `PERCENTILE.INC`) vs `TukeyHinges` (R `fivenum`, for box plots). |
| `skewness_g1`, `kurtosis_g2` | moment estimators (SciPy `bias=True`) | |
| `skewness`, `kurtosis` | Joanes & Gill (1998) G1, G2 | Bias-corrected, what Prism and Excel report; excess kurtosis. |
| `geometric_mean`, `geometric_sd_factor`, `ci_geometric_mean` | Kirkwood (1979) | All values > 0. |
| `harmonic_mean` | | All values > 0. |
| `ci_mean` | t interval | Any level. |
| `lag1_autocorrelation` | NIST StRD definition | Corrected for the rounding of the mean. |
| `Summary::compute` | all of the above | Prism's column statistics in one pass. |

## ttest

| Function | Reference | Notes |
|---|---|---|
| `one_sample` | Student (1908) | Estimate and CI are of mean - mu (SciPy's CI is of the mean). |
| `unpaired` | pooled-variance t | |
| `welch` | Welch (1947); Satterthwaite (1946) df | Fractional df. Dioptra's default unpaired test (DESIGN §2); Prism's default is the pooled test. |
| `paired` | one-sample t on x - y | Cohen's d is d_z. |
| `ratio_paired` | GraphPad "ratio paired t test" | Paired t on log10(x) - log10(y); geometric mean ratio and back-transformed CI. |
| `hedges_j(df)` | Hedges & Olkin (1985) | Exact J = Gamma(df/2)/(sqrt(df/2) Gamma((df-1)/2)); `hedges_g_approx` uses 1 - 3/(4df - 1). Two-sample effect sizes use the pooled SD and n1 + n2 - 2 df, also for Welch. R^2 = t^2/(t^2 + df). |

## rank, mannwhitney, wilcoxon

| Function | Reference | Notes |
|---|---|---|
| `rank_with_ties`, `tie_term` | midranks (SciPy `rankdata('average')`) | Tie sizes of groups >= 2; sum(t^3 - t). |
| `u_distribution(m, n)` | Mann & Whitney (1947) recurrence (Löffler 1983) | Exact null distribution of U, additions only. |
| `mann_whitney` | Mann & Whitney (1947); Hodges & Lehmann (1963); Moses CI (Hollander & Wolfe 1999 §4.3) | U1 is SciPy's statistic, `u` = min(U1, U2) is Prism's. Exact when there are no ties and n1 n2 <= 10,000; tie-corrected normal approximation with optional 0.5 continuity correction otherwise. Prism computes an exact p even with ties (up to 100 per group); we switch to the asymptotic method and say so in `method`. |
| `signed_rank_distribution(n)` | generating function prod(1 + q^i) | |
| `signed_rank`, `paired`, `one_sample` (wilcoxon) | Wilcoxon (1945); Pratt (1959) zeros; Cureton (1967) variance | `ZeroMethod::Wilcoxon` (drop zeros, Prism's default) or `Pratt`. Exact for n <= 50 without ties or zeros; otherwise normal approximation with tie (and Pratt-zero) adjustment; continuity correction off by default (SciPy's default). Hodges–Lehmann pseudo-median of the Walsh averages (zeros kept, and a zeros-dropped variant) with Tukey's CI. Prism computes exact p with ties below 200 pairs; we use the asymptotic method there and say so. |

## normality

| Function | Reference | Notes |
|---|---|---|
| `shapiro_wilk` | Royston (1992); Royston (1995), AS R94 | 3 <= n <= 5000. Normal scores from AS 241 PPND16 (as R's `swilk.c`). |
| `dagostino_pearson` | D'Agostino (1970); Anscombe & Glynn (1983); D'Agostino, Belanger & D'Agostino (1990) | Exactly SciPy's `normaltest`; n >= 8. |
| `anderson_darling` | Anderson & Darling (1954); Stephens (1974); D'Agostino & Stephens (1986) Table 4.9 | A*^2 = A^2 (1 + 0.75/n + 2.25/n^2); p from Table 4.9 (R `nortest`, Prism). Also SciPy's critical values. SciPy >= 1.17 `method='interpolated'` p is a different method. |
| `ks_one_sample`, `kolmogorov_smirnov_normal` | Kolmogorov (1933); exact p by `kolmogn_sf` | Parameters specified a priori. |
| `kolmogn_sf(n, d)` | Simard & L'Ecuyer (2011) method selection; Durbin (1968) matrix as Marsaglia, Tsang & Wang (2003); Pelz & Good (1976) | SciPy `kstwo` uses Pomeranz (1974) for 0.754693 < n d^2 <= 4; Durbin's matrix is exact there too. |
| `smirnov_sf(n, d)` | Birnbaum & Tingey (1951) | One-sided exact tail. |
| `kolmogorov_sf(z)` | Kolmogorov (1933) limiting series | SciPy `kstwobign`. |
| `lilliefors` | Lilliefors (1967) | Estimated mean and SD (n - 1). Two p-values: `p_table` (statsmodels simulation table, 10^7 replications, reproduced in `lilliefors_table.rs`, BSD-3) and `p_dallal_wilkinson` (Dallal & Wilkinson 1986 when <= 0.1, else the table, as statsmodels `pvalmethod='approx'`). Prism uses Dallal–Wilkinson and reports "P > 0.10" above. |
| `lilliefors_p_table`, `lilliefors_p_dallal_wilkinson` | as above | |

## outliers

| Function | Reference | Notes |
|---|---|---|
| `grubbs(data, alpha)` | Grubbs (1969); NIST/SEMATECH e-Handbook §1.3.5.17 | Two-sided, iterated while significant; G uses the SD of all remaining values; p is the Bonferroni bound 2n P(T > t_obs). |
| `rout_column(data, q)` | Motulsky & Brown (2006), BMC Bioinformatics 7:123; GraphPad guide | Lorentzian fit of a constant with adaptive RSDR = P68 N/(N-K) (Prism's guide; some renderings of the paper show sqrt(N/(N-K))); FDR flagging of the farthest 30% at Q on N-K df. `rsdr_alt_mad` (1.4826 MAD) is reported for comparison only. |

## quad, roots (numerical infrastructure)

| Function | Reference | Notes |
|---|---|---|
| `gauss_legendre`, `gauss_legendre_cached`, `composite_nodes` | Numerical Recipes 3rd ed. §4.6 `gauleg` | |
| `integrate` | QUADPACK QAG with QK15 (Piessens et al. 1983) | Raw |K15 - G7| error estimate (conservative). |
| `integrate_pair` | QUADPACK QAG/QK15 with QUADPACK's error scaling | Two integrands sharing nodes (a cdf and its complement). |
| `halley_bracketed` | Numerical Recipes 3rd ed. §9.4 `rtsafe` with Halley correction | Bisection (geometric across decades) when a step leaves the bracket. |
| `brent`, `expand_bracket` | Brent (1973), `zeroin` | |
