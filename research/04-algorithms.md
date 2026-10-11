# 04 - Algorithms: what the Rust engine should implement, and from which source

Each section names the method the Rust code should follow, the paper or algorithm
it comes from, and where SciPy (our oracle) and GraphPad Prism (the program we
replace) disagree on conventions. The fixtures in `fixtures/oracle/` use exactly
the formulas written here. Where a formula is written out, it is the one the
fixture generator `scripts/oracle/gen_fixtures.py` implements.

Two rules hold throughout:

- Every test computes the two-sided p first. Prism's one-tailed p is half of
  the two-sided p when the effect goes in the predicted direction, and
  `1 - p2/2` when it goes the other way. SciPy's `alternative='less'/'greater'`
  computes the same quantity directly from the distribution, so the two agree,
  but our API should take a "predicted direction" the way Prism does, rather
  than SciPy's less/greater.
- Prism prints p-values as `<0.0001` and adjusted p-values as `>0.9999`. That is
  a display rule only. The engine returns full-precision numbers, and the
  formatting layer clips them.

---

## 0. Foundations: special functions and inverses

Every p-value in the engine depends on this layer. If it is wrong, everything
above it is wrong too.

- **Regularized incomplete beta I_x(a,b)**: follow TOMS 708 `bratio`
  (DiDonato & Morris 1992, doi:10.1145/131766.131776). This is what R's
  `pbeta` and SciPy's Boost backend use. It switches among a power series, a
  continued fraction (Numerical Recipes 6.4, Lentz's method), asymptotic
  expansions for large a and b, and the symmetry I_x(a,b) = 1 - I_{1-x}(b,a).
  The awkward fixture cases (a=0.5, b=1e3, x near 0 and 1) exist because a
  single continued fraction loses digits there. Return both I and 1-I without
  computing the second by subtraction. That is how a tail like p = 1e-12 keeps
  its significant digits.
- **Regularized incomplete gamma P(a,x), Q(a,x)**: DiDonato & Morris 1986
  (doi:10.1145/22721.23109, TOMS 654). Use the series for x < a+1 and the
  continued fraction otherwise, plus Temme's uniform asymptotic expansion for
  large a. The chi-square distribution is P(k/2, x/2).
- **log-gamma / log-beta**: Lanczos approximation, or the `gamln`/`betaln` from
  TOMS 708. Compute the binomial and hypergeometric pmfs in log space through
  `lgamma`, or use Loader's saddle-point method (the one R's `dbinom` uses,
  https://www.r-project.org/doc/reports/CLoader-dbinom-2002.pdf).
- **Normal cdf/ppf**: the cdf via `erfc` (W. J. Cody's rational Chebyshev
  approximations). The ppf via Wichura 1988 AS 241 (doi:10.2307/2347330), which
  is accurate to about 1e-16 and is the algorithm R's `qnorm` uses.
- **Inverse functions (t, F, chi2, beta, gamma ppf)**: start from a closed-form
  approximation (Cornish-Fisher, or the AS 109 / AS 64 starting points for the
  inverse beta), then refine with Halley's method on the log scale. Protect the
  iteration with bisection, so that a step that leaves the bracket falls back
  to a bisection step. Stop when the relative step is below 4 machine epsilons.
- **t, F, binomial cdfs** reduce to I_x: t with nu degrees of freedom uses
  x = nu/(nu+t^2); F uses x = d1 F/(d1 F + d2); the binomial uses
  `P(X<=k) = I_{1-p}(n-k, k+1)`.
- **Hypergeometric cdf**: sum the pmf from the mode outward using the term
  ratio. For the extreme tails, use log-space summation.
- **Noncentral t**: Lenth 1989, AS 243 (doi:10.2307/2347113). This is a Poisson
  mixture of incomplete betas. R's `pt(ncp=)` uses it. Its accuracy is about
  1e-12 in the body, and it is weaker in the far tails, so the tolerance there
  is looser (see doc 05). SciPy uses Boost's `non_central_t`, which is more
  accurate, so where AS 243 and SciPy disagree in the tail, trust SciPy.
- **Studentized range**: the cdf is the double integral
  `P(q; k, nu) = int_0^inf f_nu(s) * k int phi(z)[Phi(z) - Phi(z - q s)]^{k-1} dz ds`.
  R's `ptukey`/`qtukey` follow Copenhaver & Holland 1988
  (doi:10.1080/00949658808811068). They use Gauss-Legendre quadrature over
  sub-intervals, with the outer integral dropped when nu > 25000. AS 190
  (Lund & Lund 1983, doi:10.2307/2347300) is the older, less accurate
  algorithm. SciPy's `studentized_range` integrates the same double integral
  adaptively and is the oracle here. Make `qtukey` a secant/Newton iteration on
  `ptukey`.
- **Multivariate t (Dunnett)**: Genz & Bretz 2002 (doi:10.1198/106186002394)
  and Genz & Bretz 2009, *Computation of Multivariate Normal and t
  Probabilities* (doi:10.1007/978-3-642-01689-9). SciPy's `dunnett` uses a
  randomized quasi-Monte Carlo integration, so its p-values are reproducible
  only to about 1e-4 to 1e-5 (see the Dunnett section). Dunnett's correlation
  structure (rho_ij = sqrt(n_i n_j / ((n_0+n_i)(n_0+n_j))), with control group
  0) also allows a 1-D integral form: condition on the control mean, integrate
  over the chi variable, and compute the product of normal cdfs. That gives a
  deterministic, 1e-10 answer with Gauss-Legendre quadrature. Prefer it.

## 1. Descriptive statistics

- **Mean**: compute in two passes. Take the naive mean, then add the mean of
  the residuals as a correction (Higham, *Accuracy and Stability of Numerical
  Algorithms*, ch. 4). The NIST NumAcc1-4 sets exist to catch the one-pass
  `sum(x^2) - n*mean^2` formula.
- **SD** with an n-1 denominator: Welford's update or the two-pass corrected
  sum (Chan, Golub & LeVeque 1983, doi:10.1080/00031305.1983.10483115).
  SEM = SD/sqrt(n). Prism always uses n-1, as SciPy does with `ddof=1`. Note that
  NumPy's default is ddof=0, a common source of oracle bugs.
- **CV** = SD/mean, reported as a percent in Prism.
- **Geometric mean** = exp(mean(ln x)), defined only when every x > 0. Prism
  also reports the geometric SD *factor* exp(SD(ln x)).
- **95% CI of mean** = mean +/- t_{0.975,n-1} * SEM.
- **Quartiles and percentiles**: Prism computes the rank R = P(n+1)/100,
  interpolates between the bracketing order statistics, and clamps to min/max
  when R falls outside [1,n]. This is Hyndman & Fan 1996 **definition 6**
  (doi:10.1080/00031305.1996.10473566), the same as NumPy's
  `method='weibull'`. (Source: GraphPad KB #501 and the guide page "Percentiles
  and the median",
  https://graphpad.com/guides/prism/latest/statistics/stat_percentiles_and_the_median.htm.)
  The median is the usual middle value or average of the two middle values;
  every definition agrees on it.
  - SciPy's and NumPy's default is `linear`, which is **definition 7**, the same
    as Excel's `PERCENTILE.INC`. It is not Prism's method. Prism's guide
    explicitly warns that Prism and Excel differ.
  - Tukey hinges are the medians of the lower and upper halves, with the median
    included in both halves when n is odd. They are what Prism's box-plot
    "Tukey" whiskers are built from, so the engine needs them.
  - The fixtures give all three (type 6, type 7, Tukey hinges). The engine's
    default must be type 6.
- **Skewness and kurtosis**:
  g1 = m3/m2^{3/2} and g2 = m4/m2^2 - 3 are the biased, moment versions (SciPy's
  default `bias=True`). The bias-corrected Fisher versions are
  G1 = g1 sqrt(n(n-1))/(n-2) and
  G2 = ((n+1) g2 + 6)(n-1)/((n-2)(n-3)). SciPy computes them with `bias=False`,
  and they are what Excel's `SKEW`/`KURT` give.
  Prism reports G1 and G2 (Joanes & Gill 1998, doi:10.1111/1467-9884.00122).
  G2 is "excess" kurtosis, so it is 0 for a normal distribution.

## 2. t tests

- **One-sample**: t = (mean - mu0)/SEM, df = n-1.
- **Unpaired Student** (pooled variance): s_p^2 = ((n1-1)s1^2 + (n2-1)s2^2)/(n1+n2-2),
  SE = s_p sqrt(1/n1+1/n2), df = n1+n2-2.
- **Welch**: SE = sqrt(s1^2/n1 + s2^2/n2). The Welch-Satterthwaite df is
  SE^4 / ((s1^2/n1)^2/(n1-1) + (s2^2/n2)^2/(n2-1)). Keep the df non-integer
  and pass it straight to the t cdf. Prism reports the fractional df.
- **Paired**: a one-sample t on the differences d = x - y.
- **Ratio paired t**: a paired t on ln(x) and ln(y). The reported ratio is the
  geometric mean of x/y, exp(mean(ln x - ln y)), and its CI is the
  back-transformed CI. All values must be > 0. Prism uses the natural log.
- **Effect sizes**: Cohen's d = (mean1 - mean2)/s_p for two samples, and
  mean(d)/sd(d) for paired and one-sample data (the "d_z" form). Hedges' g = d * J,
  with the exact J = Gamma(df/2)/(sqrt(df/2) Gamma((df-1)/2)). The approximation
  1 - 3/(4 df - 1) is close but not the same, and the fixtures record which one
  they use. Hedges & Olkin 1985, *Statistical Methods for Meta-Analysis*.
- **Sign convention**: Prism's mean difference is column B minus column A
  (later minus earlier). SciPy's `ttest_ind(a, b)` uses a - b. The fixtures use
  x - y. The engine must keep its own convention and flip signs when it presents
  results the way Prism does.
- **CI of the difference**: diff +/- t_{0.975,df} SE, using Welch's df for Welch.

## 3. Mann-Whitney U

- **U statistic**: U1 = R1 - n1(n1+1)/2 and U2 = n1 n2 - U1, using midranks for
  ties. SciPy returns U1, the value for the first sample. Prism reports a single
  U, the smaller of the two.
- **Exact null distribution with no ties**: the frequency of U for sizes (m, n)
  follows the recurrence f(u; m, n) = f(u-n; m-1, n) + f(u; m, n-1), or the
  faster generating-function update of Harding 1984 (doi:10.2307/2347503)
  (Löffler 1983 is the related recurrence that SciPy's `_mwu_state` follows).
  Store the counts in f64, or u128 for large m n, and divide by C(m+n, m).
  The two-sided p is 2 * min(P(U<=u), P(U>=u)), capped at 1.
- **Asymptotic method**: z = (U - m n/2 - cc)/sigma, with the tie-corrected
  sigma^2 = (m n/12)((N+1) - sum(t^3-t)/(N(N-1))) and the continuity
  correction cc = 0.5 * sign(U - mn/2). This matches SciPy with
  `use_continuity=True, method='asymptotic'`.
- **Hodges-Lehmann shift**: the median of all m*n pairwise differences
  x_i - y_j (Hodges & Lehmann 1963, doi:10.1214/aoms/1177704172). An O(n log n)
  selection algorithm exists (Monahan 1984, doi:10.1145/1271.319414), but at
  Prism's sizes the plain O(mn) sort is fine. Prism's CI for the shift comes
  from the exact U distribution's critical values (Bauer 1972).
- **Prism vs SciPy**: Prism computes an **exact p when the smaller group has
  <= 100 values, even with ties**. In other words, with ties it enumerates the
  permutation distribution of the actual midrank sums
  (https://graphpad.com/guides/prism/latest/statistics/how_the_mann-whitney_test_works.htm).
  SciPy's exact method ignores ties, and its `auto` mode picks exact only when
  there are no ties and a group is small. To match Prism with ties, implement
  the exact permutation distribution of the rank sum with midranks: a DP over
  the multiset of ranks, the Streitberg & Röhmel 1986 shift algorithm. Our
  oracle covers only the no-ties exact case and the asymptotic case. The
  tied-exact case needs a separate oracle (R `coin::wilcox_test`,
  distribution="exact").

## 4. Wilcoxon signed-rank

- **W** (SciPy, two-sided) = min(W+, W-), where W+ is the sum of the ranks of
  |d| for d > 0.
- **Exact distribution with no ties or zeros**: the number of subsets of
  {1..n} with sum w is the coefficient of q^w in prod_{i=1..n}(1+q^i). Build it
  as a DP array of length n(n+1)/2+1 and divide by 2^n.
- **Zero handling**:
  - `wilcox` drops the zero differences before ranking. This is Wilcoxon's
    original rule and Prism's default (Prism 5 and earlier had no other).
  - `pratt` ranks the zeros with everything else, then discards their ranks
    from W+ and W- (Pratt 1959, doi:10.1080/01621459.1959.10501526). Prism 6
    and later offer it as an option
    (https://graphpad.com/guides/prism/latest/statistics/stat_pratt.htm).
  - The asymptotic variance changes with each rule. For Pratt,
    var = (n(n+1)(2n+1) - n0(n0+1)(2n0+1))/24 - tie term, where n0 is the
    number of zeros. The tie term is sum(t^3-t)/48.
- **Hodges-Lehmann median difference**: the median of the n(n+1)/2 Walsh
  averages (d_i + d_j)/2 with i <= j.
- **Prism vs SciPy**: Prism computes an **exact p for fewer than 200 pairs,
  even with ties** (Prism 6+), and uses the normal approximation above that
  (https://graphpad.com/guides/prism/latest/statistics/how_the_wilcoxon_matched_pairs_test_works.htm).
  SciPy's `auto` picks exact for n <= 50 with no ties or zeros. With ties or
  zeros it does an exhaustive permutation test for n <= 13 and uses the
  asymptotic method otherwise. For tied exact p, use the same multiset DP
  (over doubled midranks, so that all values are integers). The asymptotic
  fixture cases use `correction=False`, which is SciPy's default. Prism does
  not document a continuity correction.

## 5. One-way ANOVA

- **Partition**: SS_between = sum n_i (mean_i - grand)^2 and SS_within =
  sum sum (x - mean_i)^2. F = MS_b/MS_w, with df (k-1, N-k). Compute the group
  means in two passes; the NIST SmLs07-09 sets carry 13 constant leading
  digits.
- **Effect sizes**: eta^2 = SS_b/SS_t. omega^2 = (SS_b - (k-1)MS_w)/(SS_t + MS_w)
  (Hays 1963). Prism reports R^2, which is the same number as eta^2.
- **Welch ANOVA** (Welch 1951, doi:10.2307/2332579): use weights
  w_i = n_i/s_i^2. Then
  F_W = [sum w_i (m_i - m_w)^2/(k-1)] / [1 + 2(k-2)/(k^2-1) * Lambda],
  with Lambda = sum (1 - w_i/W)^2/(n_i-1) and m_w the weighted mean. The df are
  (k-1, (k^2-1)/(3 Lambda)).
- **Brown-Forsythe F\* for means** (Brown & Forsythe 1974,
  doi:10.2307/1267501): F* = SS_b / sum (1 - n_i/N) s_i^2. The denominator df
  come from Satterthwaite:
  1/df2 = sum c_i^2/(n_i-1), with c_i = (1-n_i/N)s_i^2 / sum (1-n_j/N)s_j^2.
  **This is what Prism calls "Brown-Forsythe ANOVA"**: a test of equal *means*
  that tolerates unequal variances. It is a different test from the
  Brown-Forsythe test of *equal variances*, which is Levene's test with the
  median as center and is SciPy's `levene(center='median')`. Prism reports both
  (the second as a homogeneity-of-variance check). The fixtures carry both,
  labelled `brown_forsythe_means_Fstar` and `brown_forsythe_levene_median`.
  The F* p-value uses numerator df k-1 (the 1974 test). statsmodels'
  `anova_oneway(use_var='bf')` reports by default a p with Mehrotra's 1997
  modified numerator df (doi:10.1080/03610919708813431) and keeps the 1974 one
  as `pvalue2`; the fixture stores both (`mehrotra_1997_*`).
- **Bartlett's test** is also printed by Prism as a variance check
  (`scipy.stats.bartlett`).

## 6. Two-way ANOVA

- Fit it as a linear model with dummy coding through QR, never with the normal
  equations, and get the sums of squares from model comparisons.
  - **Type I** (sequential): the SS for each term is the drop in RSS when the
    term is added in order (A, then B, then AxB).
  - **Type II**: each main effect is adjusted for the other main effect but not
    for the interaction.
  - **Type III**: each term is adjusted for every other term, including the
    interaction. This needs **sum-to-zero (deviation) contrasts**, which is
    patsy's `C(a, Sum)`. With treatment coding, type III main effects test a
    meaningless hypothesis about one reference level. The fixtures fit
    `y ~ C(A, Sum) * C(B, Sum)` and call statsmodels `anova_lm(typ=3)`.
- When the design is balanced, all three types agree. The fixtures check this.
- **Prism**: two-way ANOVA with unequal n uses type III sums of squares. A cell
  with no data at all is treated differently in Prism and is out of scope here.
  Reference: Searle 1987, *Linear Models for Unbalanced Data*; Langsrud 2003,
  doi:10.1023/A:1023260610025.

## 7. Repeated-measures one-way ANOVA, sphericity

- The F test is the standard subjects x treatments decomposition:
  SS_treat, SS_subjects, SS_error = SS_total - SS_treat - SS_subj. The df are
  (k-1, (n-1)(k-1)), with n subjects and k levels. The oracle is statsmodels
  `AnovaRM`.
- **Orthonormal contrasts**: C is the (k-1) x k orthonormal Helmert matrix. Its
  rows are orthogonal to the vector of ones and have unit norm. S is the k x k
  sample covariance of the subjects' rows, with an n-1 denominator. Then
  S_c = C S C'.
- **Greenhouse-Geisser** (Greenhouse & Geisser 1959, doi:10.1007/BF02289823;
  Box 1954): eps_GG = (tr S_c)^2 / ((k-1) tr(S_c^2)), bounded in [1/(k-1), 1].
  The same value comes from the double-centred-S formula
  eps = k^2 (sbar_dd - sbar)^2 / ((k-1)(sum s_ij^2 - 2k sum sbar_i.^2 + k^2 sbar^2)).
  The fixtures check that the two forms agree.
- **Huynh-Feldt** with the Lecoutre 1991 correction
  (doi:10.3102/10769986016004371), as R's `car` and recent SPSS use it. With
  N subjects in g groups:
  eps_HF = ((N-g+1)(k-1) eps_GG - 2) / ((k-1)(N - g - (k-1) eps_GG)), capped at 1.
  The original Huynh & Feldt 1976 numerator has N where Lecoutre has N-g+1.
  For the one-group design Prism runs (g=1, N=n) the two forms are identical:
  eps_HF = (n(k-1) eps_GG - 2) / ((k-1)(n - 1 - (k-1) eps_GG)). The fixture
  records both the capped and the uncapped value.
- **Mauchly's W** (Mauchly 1940, doi:10.1214/aoms/1177731915):
  W = det(S_c) / (tr(S_c)/(k-1))^{k-1}. The test statistic is
  chi2 = -(n - 1 - (2(k-1)^2 + (k-1) + 2)/(6(k-1))) ln W, with
  df = k(k-1)/2 - 1.
- The corrected tests multiply both df by epsilon.
- **Prism** applies the Geisser-Greenhouse correction when the user unticks
  "assume sphericity", and reports eps_GG. It does not run Mauchly's test,
  because GraphPad considers it unreliable. Huynh-Feldt and Mauchly are
  computed for completeness and for SPSS/R parity.

## 8. Kruskal-Wallis, Friedman, Dunn

- **Kruskal-Wallis**: H = (12/(N(N+1))) sum R_i^2/n_i - 3(N+1), divided by the
  tie factor 1 - sum(t^3-t)/(N^3-N). p comes from chi2 with k-1 df
  (Kruskal & Wallis 1952, doi:10.1080/01621459.1952.10483441). SciPy `kruskal`
  is the oracle. Prism can compute an exact p for small samples by
  permutation. That threshold is not covered by the fixtures.
- **Friedman**: rank within each subject (block). Then
  chi2_F = 12/(n k(k+1)) sum R_j^2 - 3n(k+1), with the within-block tie
  correction divisor 1 - sum(t^3-t)/(n(k^3-k)). The df are k-1. SciPy
  `friedmanchisquare` is the oracle.
- **Dunn's test** (Dunn 1964, doi:10.1080/00401706.1964.10490181; Prism cites
  Daniel, *Applied Nonparametric Statistics*, pp. 240-241). Rank all values
  together. Then
  z_ij = (Rbar_i - Rbar_j) / sqrt((N(N+1)/12 - sum(t^3-t)/(12(N-1))) (1/n_i + 1/n_j)),
  and the two-sided p is 2(1 - Phi(|z|)). Bonferroni multiplies p by
  m = k(k-1)/2 and caps it at 1. Holm uses the step-down rule from section 9.
  scikit-posthocs is not installed in the oracle venv, so the fixtures
  implement this formula directly.
  (https://graphpad.com/guides/prism/latest/statistics/stat_how_the_dunn_method_for_nonpar.htm)
- Post hoc tests after Friedman are not in the fixtures.

## 9. Multiple comparisons after ANOVA

- **Tukey HSD / Tukey-Kramer** (Tukey 1953; Kramer 1956,
  doi:10.2307/3001469): q_ij = |m_i - m_j| / sqrt(MS_w/2 (1/n_i + 1/n_j)).
  The adjusted p is 1 - ptukey(q, k, df_w), and the CI is
  diff +/- qtukey(0.95,k,df)/sqrt(2) * sqrt(MS_w(1/n_i+1/n_j)). Prism's "Tukey"
  is always Tukey-Kramer when the n differ, as is statsmodels
  `pairwise_tukeyhsd`. Note that statsmodels reports the CI half-width with the
  `q/sqrt(2)` factor already applied. Prism reports q itself, defined *with*
  the sqrt(2), i.e. q = diff/sqrt(MS_w/2 (1/n_i+1/n_j)).
- **Dunnett** (Dunnett 1955, doi:10.1080/01621459.1955.10501294): compare each
  treatment with the control,
  t_i = (m_i - m_0)/sqrt(MS_w (1/n_i + 1/n_0)), and compute the adjusted p from
  the (k-1)-variate t with the correlations in section 0. Because the
  correlation has product form rho_ij = lam_i lam_j, lam_i = sqrt(n_i/(n_i+n_0)),
  the probability reduces to a double integral:
  P(max|T_i| <= c) = int f_u(u) int phi(z) prod_i [Phi((cu - lam_i z)/sqrt(1-lam_i^2))
  - Phi((-cu - lam_i z)/sqrt(1-lam_i^2))] dz du, with u = s/sigma ~ chi_nu/sqrt(nu).
  The fixture computes it that way (400-point Gauss-Legendre in z, adaptive in u,
  about 1e-12), so the expected Dunnett p and critical value are exact to
  near double precision. SciPy's `stats.dunnett` integrates with randomized QMC
  and is good to only about 1e-4, even with a fixed rng; its values are
  recorded with a loose tolerance as a sanity check. The Rust code can use the
  same 1-D-in-z, 1-D-in-u integral for many-to-one comparisons; Genz-Bretz is
  needed only for general correlation structures.
  Prism 6 and later compute exact multiplicity-adjusted p for Dunnett, Tukey and
  Sidak.
- **Bonferroni**: p_adj = min(1, m p). **Sidak**: 1 - (1-p)^m. Holm 1979
  (https://www.jstor.org/stable/4615733) is a step-down: sort ascending, take
  p_(i)(m-i+1), then a running max, then cap at 1. **Holm-Sidak** is the same
  step-down with 1-(1-p)^(m-i+1). This is Prism's recommended default for
  "compare selected pairs".
- **Benjamini-Hochberg FDR** (BH 1995, doi:10.1111/j.2517-6161.1995.tb02031.x):
  sort ascending, take p_(i) m/i, then a running min from the top, then cap at 1.
  Prism also offers Benjamini-Krieger-Yekutieli two-stage (2006,
  doi:10.1093/biomet/93.3.491), which statsmodels calls `fdr_tsbky`. Prism's
  BKY is the recommended FDR method, so add it when FDR fixtures are extended.
- **Oracle**: statsmodels `multipletests`. Note that statsmodels `holm-sidak` is
  the step-down Sidak, not Holm.

## 10. Contingency tables

- **Chi-square of independence**: X^2 = sum (O-E)^2/E, with df (r-1)(c-1).
  The Yates correction (2x2 only) is sum (|O-E| - 0.5)^2/E, with |O-E| - 0.5
  floored at 0 as SciPy does. SciPy's `chi2_contingency` applies Yates *by
  default* for 2x2 tables (`correction=True`). Prism asks the user, and its
  default 2x2 test is Fisher's exact.
- **Goodness of fit**: `scipy.stats.chisquare`, df = k-1-(number of estimated
  params).
- **Fisher exact 2x2**: the hypergeometric distribution of a given the
  margins. "less" is P(X<=a) and "greater" is P(X>=a). The two-sided p sums
  all tables with probability <= p_obs * (1 + 1e-7). This is R's relative
  tolerance, and SciPy does the same. Prism uses this "sum of small p" two-sided
  rule as well.
- **Odds ratio**: the sample OR = ad/(bc). The **conditional MLE** OR and its
  exact CI invert the noncentral hypergeometric distribution (Cornfield 1956;
  R `fisher.test`; SciPy `stats.contingency.odds_ratio(kind='conditional')`).
  Solve the score equation E[X; psi] = a with Brent's method on log psi. The
  CI ends come from the tail probabilities, each equal to 0.025.
- **Woolf (log) CI for the OR**: exp(ln OR +/- z sqrt(1/a+1/b+1/c+1/d))
  (Woolf 1955, doi:10.1111/j.1469-1809.1955.tb01285.x).
- **Relative risk, Katz log CI**:
  exp(ln RR +/- z sqrt(1/a - 1/(a+b) + 1/c - 1/(c+d)))
  (Katz et al. 1978, doi:10.2307/2530610). Prism's default RR CI is the
  Koopman asymptotic score interval (Koopman 1984, doi:10.2307/2531405), and
  its OR CI is Baptista-Pike. Both are on the to-do list for Prism parity. The
  fixtures give Woolf and Katz as the textbook references.
- **Fisher exact r x c**: the reference algorithm is the network algorithm of
  Mehta & Patel 1983 (doi:10.1080/01621459.1983.10477989), implemented as
  FEXACT (TOMS 643, Mehta & Patel 1986, doi:10.1145/6497.214326), which is what
  R's `fisher.test` uses. For small tables the fixtures use plain enumeration:
  every table with the observed margins, weighted by the multivariate
  hypergeometric probability prod(r_i!) prod(c_j!)/(N! prod n_ij!). p is the sum
  over tables with probability <= p_obs(1+1e-7). The Rust implementation can
  start with the same enumeration plus a cap on table count, then add the
  network algorithm for larger tables.
- **McNemar** (McNemar 1947, doi:10.1007/BF02295996): chi2 = (b-c)^2/(b+c), or
  (|b-c|-1)^2/(b+c) with continuity correction. The exact version is binomial
  with p = 0.5 on b+c. statsmodels `mcnemar` is the oracle. Prism uses the
  continuity-corrected chi-square.
- **Cochran-Armitage trend** (Cochran 1954, doi:10.2307/3001616; Armitage 1955,
  doi:10.2307/3001775):
  Z = sum t_i (x_i - n_i pbar) / sqrt(pbar(1-pbar)[sum n_i t_i^2 - (sum n_i t_i)^2/N]).
  Without the N/(N-1) factor this matches R's `prop.trend.test` (whose
  chi-square equals Z^2). Some texts use the N/(N-1) variant, so the engine
  should state which it uses.

## 11. Correlation

- **Pearson r**: compute from the centred cross-products (two-pass).
  t = r sqrt((n-2)/(1-r^2)), df n-2. The Fisher-z CI is
  tanh(atanh r +/- z_{0.975}/sqrt(n-3)). Prism uses this CI.
- **Spearman rho**: Pearson r on midranks, which handles ties correctly. The
  shortcut 1 - 6 sum d^2/(n(n^2-1)) is wrong with ties, so do not use it. For
  p, the fixtures enumerate all n! permutations exactly for n <= 8 and use the
  t approximation t = rho sqrt((n-2)/(1-rho^2)) with n-2 df otherwise. SciPy's
  `spearmanr` uses only the t approximation. The exact distribution for larger
  n without ties comes from the Edgeworth series or AS 89 (Best & Roberts 1975,
  doi:10.2307/2347111), which is what R's `cor.test(method='spearman')` uses up
  to n=1290.
- **Kendall tau-b**: tau_b = (C - D)/sqrt((n0-n1)(n0-n2)), with n0 = n(n-1)/2
  and n1, n2 the tie sums. Compute it in O(n log n) with Knight's merge-sort
  algorithm (Knight 1966, doi:10.1080/01621459.1966.10480879), as SciPy does.
  For the exact p with no ties, use the Kendall recurrence on the number of
  inversions, N(n, k) = sum_{j=0}^{min(k, n-1)} N(n-1, k-j). SciPy's
  `kendalltau(method='auto')` uses exact for small n with no ties and the
  normal approximation with the tie-corrected variance otherwise.

## 12. Linear regression

- **Fit**: Householder QR (or SVD when the matrix is rank-deficient) on the
  design matrix. Never form X'X. The NIST Longley, Filip and Wampler4-5 sets
  exist to break the normal equations, and Filip needs column scaling plus QR
  with extended precision, or pivoting, to reach 7+ digits.
- **Standard outputs**: SE(b) = s sqrt(diag((X'X)^{-1})), taken from R^{-1}.
  Then t = b/SE with df = n-p, then R^2, adjusted R^2 = 1 - (1-R^2)(n-1)/(n-p),
  Sy.x = sqrt(SSE/(n-p)), and F = (SSR/(p-1))/(SSE/(n-p)).
- **Confidence band at x0**: yhat +/- t s sqrt(x0'(X'X)^{-1}x0).
  **Prediction band**: the same with 1 + x0'(X'X)^{-1}x0. For simple
  regression this is s sqrt(1/n + (x0-xbar)^2/Sxx) (+1).
- **Runs test** (Wald-Wolfowitz on the signs of the residuals in x order):
  mean = 2 n+ n-/N + 1 and var = 2n+n-(2n+n- - N)/(N^2(N-1)). Prism computes
  an exact one-sided p (too few runs) for small n via the runs distribution
  (Swed & Eisenhart 1943, doi:10.1214/aoms/1177731178). The fixture gives the
  number of runs and the exact p from the Swed-Eisenhart distribution.
- **Deming regression**: lambda is the error-variance ratio
  sigma_y^2/sigma_x^2 (default 1, which is orthogonal regression).
  slope = (syy - lambda sxx + sqrt((syy - lambda sxx)^2 + 4 lambda sxy^2))/(2 sxy)
  and intercept = ybar - slope xbar, where sxx, syy, sxy are the centred sums
  (Deming 1943; Linnet 1993, doi:10.1093/clinchem/39.3.424). SEs come from the
  jackknife (leave one out, pseudo-values), which is the approach Linnet
  recommends and what NCSS/mcr use. Prism's Deming documentation states the
  SEs without naming the method, so treat the SE fixture as the jackknife
  reference and check Prism separately. Prism's input is the SD of the X and Y
  errors, so lambda = (SD_y/SD_x)^2.
- **Multiple linear regression**: the same QR path. statsmodels `OLS` is the
  oracle (coefficients, SE, t, p, R^2, adjusted R^2, F).

## 13. Nonlinear regression

- **Optimizer**: Levenberg-Marquardt following Moré 1978
  (doi:10.1007/BFb0067700), as implemented in MINPACK `lmder`/`lmdif`
  (https://www.netlib.org/minpack/). It uses scaled trust regions, a QR of the
  Jacobian with column pivoting, and the Moré update for the damping
  parameter. Geodesic acceleration (Transtrum & Sethna 2012, arXiv:1201.5885)
  is optional and helps on MGH09, MGH10 and Bennett5. SciPy's
  `curve_fit(method='lm')` wraps MINPACK `lmdif`, which uses forward-difference
  Jacobians. Analytic or dual-number Jacobians in Rust will therefore agree with
  it to roughly 1e-7 relative, not to 1e-12. Doc 05 has the tolerance rules.
- **Starting values**: Prism computes initial values with rules specific to
  each model. The NIST sets give two starting points each, and the engine must
  converge from both.
- **Asymptotic SE**: SE_j = sqrt(s^2 [(J'J)^{-1}]_jj), with s^2 = SS/(n-p),
  computed from the QR of J. CI = b +/- t_{0.975,n-p} SE. That is Prism's
  default "asymmetric: no" CI.
- **Profile-likelihood CI** (Prism's "asymmetric" CI option; Venzon &
  Moolgavkar 1988, doi:10.2307/2347496; Bates & Watts 1988, *Nonlinear
  Regression Analysis*, ch. 6). Fix b_j on a grid, refit the others, and find
  where SS(b_j) = SS_min (1 + F_{0.95;1,n-p}/(n-p)). Prism uses this F-based
  threshold. Not in the first fixture batch.
- **Goodness of fit**: R^2 = 1 - SS/SS_tot (with no intercept adjustment), and
  Sy.x = sqrt(SS/(n-p)).
- **AICc** for least squares: AICc = N ln(SS/N) + 2K + 2K(K+1)/(N-K-1), where
  **K = number of fitted parameters + 1**. Prism counts the residual variance
  as a parameter
  (https://graphpad.com/guides/prism/latest/curve-fitting/reg_how_the_aicc_computations_work.htm).
  The fixtures use this K. Prism reports delta AICc as
  AICc(simpler) - AICc(more complex), with the Akaike weight
  exp(-0.5 delta)/(1+exp(-0.5 delta)).
- **Extra-sum-of-squares F** (nested models; model 1 is simpler):
  F = ((SS1 - SS2)/(df1 - df2)) / (SS2/df2), with df (df1-df2, df2).
- **Weighting**: 1/Y^2 weighting minimizes sum ((y - f)/y)^2, so pass
  sigma = |y| to curve_fit. Prism also offers 1/Y^K and 1/X^K. With
  `absolute_sigma=False`, SciPy rescales the covariance by the reduced chi^2,
  which is what Prism does.
- **Robust fitting**: Prism's robust fit minimizes the **Lorentzian** merit
  sum ln(1 + (r_i/RSDR)^2), with RSDR recomputed every iteration (section 16).
  The fixture's robust case uses SciPy `least_squares(loss='cauchy',
  f_scale=c)`, which minimizes sum c^2 ln(1 + (r/c)^2), the same Lorentzian
  shape but with a *fixed* scale c. The fixture records c. A second case uses
  `loss='soft_l1'` (rho(z) = 2(sqrt(1+z)-1)) as a smooth-L1 reference. Neither
  case reproduces Prism's adaptive-scale fit exactly. The Rust ROUT
  implementation must add the per-iteration RSDR update itself.

## 14. Normality tests

- **Shapiro-Wilk**: Royston 1992 (doi:10.1007/BF01891203) for the
  approximations to the coefficients and the normalising transform of W, and
  Royston 1995 AS R94 (doi:10.2307/2986146), valid for 3 <= n <= 5000. SciPy
  wraps the AS R94 Fortran, and R's `shapiro.test` is the same code.
- **D'Agostino-Pearson omnibus K^2** (D'Agostino, Belanger & D'Agostino 1990,
  doi:10.1080/00031305.1990.10475751): Z(sqrt b1) uses D'Agostino 1970's
  transform, and Z(b2) uses Anscombe & Glynn 1983. K^2 = Z1^2 + Z2^2, compared
  with chi2(2). It needs n >= 8 (SciPy `skewtest` warns below 8 and
  `kurtosistest` below 20). SciPy `normaltest` is the oracle. Prism needs n >= 8.
- **Anderson-Darling** against a normal with estimated mean and SD:
  A^2 = -n - (1/n) sum (2i-1)[ln Phi(z_(i)) + ln(1 - Phi(z_(n+1-i)))].
  The modified statistic is A*^2 = A^2 (1 + 0.75/n + 2.25/n^2). The p-value
  comes from D'Agostino & Stephens 1986, *Goodness-of-Fit Techniques*,
  Table 4.9:
  - A* >= 0.6: p = exp(1.2937 - 5.709 A* + 0.0186 A*^2)
  - 0.34 <= A* < 0.6: p = exp(0.9177 - 4.279 A* - 1.38 A*^2)
  - 0.2 <= A* < 0.34: p = 1 - exp(-8.318 + 42.796 A* - 59.938 A*^2)
  - A* < 0.2: p = 1 - exp(-13.436 + 101.14 A* - 223.73 A*^2)

  This is the formula used by R `nortest::ad.test` and, as far as GraphPad
  documents it, by Prism. SciPy `anderson` returns critical values. Since 1.17
  it also offers `method='interpolated'` p-values from its own tables, which
  are a different method. The fixtures record the statistic, SciPy's critical
  values, and the Table 4.9 p.
- **Kolmogorov-Smirnov with estimated parameters (Lilliefors)**: the statistic
  is D = max |F_n - Phi((x-xbar)/s)|. statsmodels `lilliefors(pvaluemethod=
  'table')` interpolates its own tables. Prism uses the Dallal & Wilkinson 1986
  approximation (doi:10.1080/00031305.1986.10475419) and reports "P > 0.10"
  above that. The two can differ in the third digit, so treat Lilliefors p as a
  loose fixture.
- **KS with specified parameters**: the exact distribution of D_n by
  Marsaglia, Tsang & Wang 2003 (doi:10.18637/jss.v008.i18), with Simard &
  L'Ecuyer 2011 (doi:10.18637/jss.v039.i11) for the switch-over regions. SciPy
  `kstest(method='exact')` is the oracle.

## 15. Grubbs' test

Grubbs 1969 (doi:10.1080/00401706.1969.10490657); NIST e-Handbook 1.3.5.17
(https://www.itl.nist.gov/div898/handbook/eda/section3/eda35h1.htm), which is
the source GraphPad's QuickCalc follows.

- G = max_i |x_i - xbar|/s, with s using n-1 and computed from *all* values,
  including the suspect one.
- Two-sided critical value:
  G_crit = ((n-1)/sqrt(n)) sqrt(t^2/(n-2+t^2)), with t = t_{1 - alpha/(2n), n-2}.
- Approximate p: invert the critical value. Set
  t_obs = sqrt(n(n-2)G^2/((n-1)^2 - nG^2)), then p = min(1, 2n(1 - T_{n-2}(t_obs))).
  This is a Bonferroni bound, and it is accurate whenever p is small.
- **Iteration** (Prism's "iterative Grubbs"): remove the most extreme point
  while p < alpha (0.05), recompute, and stop at the first non-significant
  step. The fixture records the indices removed, in order, plus G and p at each
  step.

## 16. ROUT (robust regression + outlier removal)

Motulsky & Brown 2006, BMC Bioinformatics 7:123, doi:10.1186/1471-2105-7-123
(open access at https://pmc.ncbi.nlm.nih.gov/articles/PMC1472692/). Prism
guide: https://graphpad.com/guides/prism/latest/statistics/stat_how_it_works_rout_method.htm.

1. **Robust fit.** Minimize sum ln(1 + (D_i/RSDR)^2), the Lorentzian merit
   (paper Eq. 8/14). RSDR is recomputed at every iteration. The optimizer is a
   modified Marquardt whose gradient and Hessian terms are the least-squares
   ones weighted by 1/(1+RR_i^2), with RR_i = D_i/RSDR. To judge whether a step
   is accepted, re-evaluate the previous merit with the new RSDR. For "identify
   outliers in a column", the model is Y = M, a constant. One parameter, so K=1.
   Start from the median.
2. **RSDR.** Take P68, the 68.27th percentile of |residuals| (interpolated), and
   apply the small-sample correction with N points and K parameters. Prism's
   guide states the correction as **RSDR = P68 * N/(N-K)**
   (https://graphpad.com/guides/prism/latest/curve-fitting/reg_standard_deviation_of_the_resi.htm).
   Some renderings of the paper's Eq. 1 show sqrt(N/(N-K)). The fixture follows
   the Prism guide (no square root) and records P68 so that either form can be
   checked. The task brief mentioned 1.4826*MAD. That is a different robust
   scale (the MAD scaled to sigma), not the one the paper uses. The fixture
   also records 1.4826*MAD*N/(N-K) for comparison, but the flagged indices come
   from P68.
3. **FDR test** (paper Eqs. 17-18). Rank |residuals| from low to high, so that
   index N is the farthest point. For i from int(0.70 N) to N:
   alpha_i = Q (N - (i-1))/N, with t = |r_i|/RSDR, and p is the two-tailed t
   probability with **N-K df**. At the *first* i with p < alpha_i, declare
   point i and every point farther out to be outliers, then stop. Only the
   farthest 30% can ever be flagged. Q = 1% is Prism's default.
4. **Refit** by ordinary least squares without the outliers. For the
   column case this is just the mean of the kept values.

## 17. Survival analysis

- **Kaplan-Meier**: S(t) = prod_{t_i <= t} (1 - d_i/n_i), with events before
  censorings at tied times (the standard convention, and lifelines').
  - **Greenwood variance**: Var S = S^2 sum d_i/(n_i(n_i - d_i)).
  - **Log-log CI** ("exponential Greenwood", lifelines default, Kalbfleisch &
    Prentice 1980): theta = ln(-ln S), SE_theta = sqrt(sum d/(n(n-d)))/|ln S|,
    and CI = S^{exp(+/- z SE_theta)}. **This is Prism's default
    "asymmetrical" CI.** GraphPad states that it matches R `survfit(...,
    conf.type="log-log")`
    (https://graphpad.com/guides/prism/latest/statistics/the_fraction_(or_percent)_survival_at_each_time.htm).
    Note that R's own default is `"log"`, not log-log.
  - **Linear ("symmetrical")**: S +/- z sqrt(Var S), clipped to [0,1]. Prism
    keeps it only for backward compatibility.
- **Median survival**: the smallest t with S(t) <= 0.5. Prism, lifelines and
  R all use this rule. Where S stays at exactly 0.5 over an interval, some
  programs average the two ends, so the fixtures avoid that case. In Prism
  10.5+ the CI of the median uses the complementary log-log method (Brookmeyer &
  Crowley 1982).
- **Log-rank (Mantel-Cox)**: at each distinct event time, take
  E_1j = d_j n_1j/n_j and V_j = d_j (n_1j/n_j)(1 - n_1j/n_j)(n_j - d_j)/(n_j - 1).
  Then chi2 = (O_1 - E_1)^2/V with 1 df. Prism uses this Mantel-Haenszel form
  and calls it "logrank"
  (https://graphpad.com/guides/prism/latest/statistics/stat_analysis_choices_for_survival.htm).
  lifelines `logrank_test` is the oracle.
- **Gehan-Breslow-Wilcoxon**: the same sums with weight w_j = n_j at each time
  (lifelines `weightings='wilcoxon'`).
- **Hazard ratio**: Prism reports two estimates.
  - *Logrank method*: HR = (O_1/E_1)/(O_2/E_2), SE(ln HR) = sqrt(1/E_1 + 1/E_2),
    and CI = exp(ln HR +/- 1.96 SE).
  - *Mantel-Haenszel method*: L = (O_1 - E_1)/V, HR = exp(L), and
    CI = exp(L +/- 1.96/sqrt(V)).
  - (https://graphpad.com/guides/prism/latest/statistics/stat_the_hazard_ratio.htm.)
    The two agree closely when HR is near 1 and there are few ties.
  - The Cox proportional-hazards HR (Breslow ties in R's default; lifelines
    uses Efron ties) is a third estimate and comes from lifelines
    `CoxPHFitter`. Prism's Cox regression module is separate.
  - The fixtures give all three, labelled.

## 18. ROC curves

- **Curve**: sort the scores descending. At each distinct threshold, record
  sensitivity = TP/P and 1 - specificity = FP/N. Ties in score move both
  coordinates in one diagonal step. sklearn `roc_curve` (with
  `drop_intermediate=False`) is the oracle.
- **AUC** = the Mann-Whitney U/(n1 n0), with ties counted as 1/2.
- **DeLong SE** (DeLong, DeLong & Clarke-Pearson 1988, doi:10.2307/2531595),
  computed with the O(n log n) midrank algorithm of Sun & Xu 2014
  (doi:10.1109/LSP.2014.2337313). Take the midranks of the positives in the
  pooled set, then within the positives, and likewise for the negatives. The
  structural components are
  V10_i = (R_pooled(x_i) - R_pos(x_i))/n0 and
  V01_j = 1 - (R_pooled(y_j) - R_neg(y_j))/n1.
  Then Var = var(V10)/n1 + var(V01)/n0, with n-1 denominators. The CI is
  AUC +/- 1.959964 SE, clipped to [0,1].
- **Hanley-McNeil 1982** (doi:10.1148/radiology.143.1.7063747):
  Q1 = A/(2-A) and Q2 = 2A^2/(1+A), with
  SE^2 = (A(1-A) + (n1-1)(Q1-A^2) + (n0-1)(Q2-A^2))/(n1 n0).
  Prism's ROC analysis reports this SE, and it is the second reference in the
  fixture. (Its "Wilson/Brown" choice applies to the CIs of sensitivity and
  specificity at each threshold.)
- **Sensitivity/specificity CIs**: Prism uses the Wilson-Brown hybrid
  (Brown, Cai & DasGupta 2001, doi:10.1214/ss/1009213286). The fixtures give
  the plain Wilson score interval. Clopper-Pearson is the alternative.
- **Youden J** = sens + spec - 1. The fixture reports the threshold that
  maximizes J. When thresholds tie on J, it takes the first in the descending
  threshold order.

## 19. Bland-Altman

Bland & Altman 1986 (doi:10.1016/S0140-6736(86)90837-8) and 1999
(doi:10.1177/096228029900800204).

- d = A - B. Prism lets the user choose A-B, B-A, ratio, or % difference, and
  the fixtures use A-B. bias = mean(d) and sd = SD(d) with n-1.
- LoA = bias +/- 1.96 sd. Prism uses 1.96, not t.
- CI of the bias = bias +/- t_{0.975,n-1} sd/sqrt(n).
- SE of each limit = sd sqrt(1/n + 1.96^2/(2(n-1))), with CI = limit +/- t SE
  (Bland & Altman 1999, section 4.2). The exact method of Carkeet 2015
  (doi:10.1097/OPX.0000000000000513) is tighter, and is not used here.
- **Proportional bias**: OLS of d on (A+B)/2, giving the slope, its p and its
  CI. Prism does not report this, but it is a standard companion.

## 20. PCA

- Standardize the columns (z-scores with n-1 SD) and take the SVD
  Z = U S V'. Then eigenvalue_k = S_k^2/(n-1), the explained variance ratio is
  eigenvalue/sum, and the loadings are V's columns (eigenvectors).
  sklearn `PCA` on the standardized matrix is the oracle. sklearn's
  `explained_variance_` already uses n-1.
- **Sign convention**: an eigenvector's sign is arbitrary. The fixtures flip
  each component so that its entry with the largest absolute value is
  positive. When two entries tie in |.|, the first one wins. The Rust code must
  apply the same rule before comparing. The scores flip with the components.
- Prism's PCA also offers "loadings" scaled by sqrt(eigenvalue) (correlations
  of the variables with the PCs). The fixtures give both, labelled
  `eigenvectors` and `loadings_scaled`.

## 21. Distribution tables

The fixture `distributions.json` holds the cdf, sf and ppf of t, F, chi2,
normal, beta, gamma, binomial, hypergeometric and noncentral t on grids that
reach p = 1e-12 in both tails, plus regularized I_x(a,b) and P(a,x)/Q(a,x) at
awkward arguments (a=0.5, b=1000; a=b=5000; a=b=1e-3; x within 1e-6 of 0 or 1;
a up to 1e5). The expected values are **not** SciPy output. They are
32-digit mpmath references rounded to double:

- I_x(a,b) from the positive-term series x^a(1-x)^b/(a B(a,b)) 2F1(a+b,1;a+1;x)
  on whichever side of the mean avoids cancellation (80 working digits),
  checked against mpmath's `betainc` in the body.
- P(a,x), Q(a,x) from the power series (x < a+1) and the Lentz continued
  fraction for Q (otherwise), at 80 digits, checked against mpmath's `gammainc`.
- t, F, chi2 through those; quantiles by a 32-digit secant root of cdf or sf.
- Noncentral t by quadrature of int Phi(xu - nc) f_u(u) du, confirmed in the
  far tail by a second representation.

SciPy's worst relative error per table is stored in the fixture. Two SciPy
weaknesses turned up: `stats.f.isf` in the far upper tail (sf(isf(q)) is off
by about 5e-9 relative at q = 1e-8 and about 1e-4 at q = 1e-12 when df2 is
small), and `stats.nct.cdf` in the far lower tail (x=-3, df=5, nc=8: SciPy
8.0e-20, true value 1.90e-20). The Rust code must beat SciPy there, not copy
it. The algorithms are in section 0. For the far tails, compute `sf` directly
rather than `1 - cdf`. The Rust API must expose the upper tail directly.

---

## Summary: where SciPy and Prism disagree

| Topic | SciPy / statsmodels default | Prism |
|---|---|---|
| Quartiles | type 7 (`linear`) | type 6, (n+1)p rank, clamp |
| Skew/kurtosis | g1, g2 (biased) | G1, G2 (bias-corrected) |
| Mann-Whitney exact | no ties, small n only | smaller group <= 100, even with ties |
| Wilcoxon exact | n <= 50, no ties/zeros | n < 200, even with ties; zeros: Wilcoxon default, Pratt option |
| U reported | U1 (first sample) | smaller U |
| Mean difference sign | a - b | later column minus earlier |
| Brown-Forsythe | `levene(center='median')` (variances) | F* test of means (plus the variance test) |
| Two-way unbalanced | typ chosen by user | type III |
| RM ANOVA epsilon | (AnovaRM gives none) | Geisser-Greenhouse, no Mauchly |
| 2x2 chi-square | Yates on by default | user choice; Fisher default |
| RR / OR CI | Katz / Woolf (textbook) | Koopman / Baptista-Pike |
| AD p | interpolated tables (1.17+) | D'Agostino-Stephens Table 4.9 |
| Lilliefors p | statsmodels tables | Dallal-Wilkinson |
| KM CI | lifelines log-log; R `log` | log-log ("asymmetrical") |
| Hazard ratio | Cox (lifelines) | logrank and Mantel-Haenszel both |
| Robust fit | fixed-scale Cauchy loss | Lorentzian with adaptive RSDR |
| AICc K | user's choice | parameters + 1 |
| Dunnett p | randomized QMC (~1e-4) | exact (fixture: double integral, ~1e-12) |
| One-tailed p | `alternative=` | p2/2 if direction predicted, else 1 - p2/2 |
