//! Contingency tables: chi-square tests (independence, goodness of fit, trend),
//! Fisher's exact test (2 x 2 with the conditional MLE odds ratio, and r x c by a network
//! algorithm), McNemar's test, relative risk and odds ratio with their CIs, diagnostic
//! accuracy (sensitivity, specificity, predictive values, likelihood ratios) and the
//! Wilson score interval.
//!
//! Tables are given as rows of counts. 2 x 2 tables are `[[a, b], [c, d]]` with rows =
//! groups (exposed, unexposed) and columns = outcome (event, no event); OR = ad/bc and
//! RR = (a/(a+b)) / (c/(c+d)), as in SciPy, statsmodels and Prism's default layout.

use std::collections::HashMap;

use crate::descriptive::compensated_sum;
use crate::dist::{
    p_two_sided_z, p_upper_chi2, std_normal_cdf, std_normal_sf, Binomial, Hypergeometric,
};
use crate::roots;
use crate::special;
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

fn z_two_sided(level: f64) -> Result<f64> {
    if !(level > 0.0 && level < 1.0) {
        return Err(invalid(format!("confidence level {level} not in (0, 1)")));
    }
    Ok(special::norm_quantile_upper(0.5 * (1.0 - level)))
}

fn check_table(t: &[Vec<u64>], what: &str) -> Result<(usize, usize)> {
    let r = t.len();
    let c = t.first().map_or(0, |x| x.len());
    if r < 2 || c < 2 {
        return Err(StatsError::NotEnoughData(format!(
            "{what} needs at least a 2 x 2 table"
        )));
    }
    if t.iter().any(|row| row.len() != c) {
        return Err(invalid(format!("{what}: ragged table")));
    }
    Ok((r, c))
}

fn margins(t: &[Vec<u64>]) -> (Vec<u64>, Vec<u64>, u64) {
    let rs: Vec<u64> = t.iter().map(|r| r.iter().sum()).collect();
    let c = t[0].len();
    let cs: Vec<u64> = (0..c).map(|j| t.iter().map(|r| r[j]).sum()).collect();
    let n = rs.iter().sum();
    (rs, cs, n)
}

// =======================================================================================
// Chi-square tests
// =======================================================================================

/// A chi-square statistic with its df and upper-tail p.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChiSquareStat {
    /// Statistic.
    pub statistic: f64,
    /// Degrees of freedom.
    pub df: f64,
    /// P(chi^2_df >= statistic).
    pub p: f64,
}

fn chi_stat(x: f64, df: f64) -> ChiSquareStat {
    ChiSquareStat {
        statistic: x,
        df,
        p: p_upper_chi2(x, df),
    }
}

/// Chi-square test of independence on an r x c table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChiSquareIndependence {
    /// Pearson X^2 = sum (O - E)^2 / E.
    pub pearson: ChiSquareStat,
    /// Yates-corrected X^2 (2 x 2 tables only).
    pub yates: Option<ChiSquareStat>,
    /// Likelihood-ratio G = 2 sum O ln(O/E).
    pub g_test: ChiSquareStat,
    /// Expected counts r_i c_j / N.
    pub expected: Vec<Vec<f64>>,
    /// Pearson residuals (O - E)/sqrt(E).
    pub pearson_residuals: Vec<Vec<f64>>,
    /// Adjusted standardised residuals (O - E)/sqrt(E (1 - r_i/N)(1 - c_j/N)) (Haberman 1973).
    pub adjusted_residuals: Vec<Vec<f64>>,
    /// Cramér's V = sqrt(X^2 / (N (min(r, c) - 1))).
    pub cramers_v: f64,
    /// Signed phi = (ad - bc)/sqrt(r1 r2 c1 c2) for 2 x 2 tables.
    pub phi: Option<f64>,
    /// Total count.
    pub n: u64,
    /// Number of cells with expected count < 5.
    pub cells_expected_below_5: usize,
    /// Smallest expected count.
    pub min_expected: f64,
    /// True when any expected count is < 1 or more than 20% are < 5 (Cochran 1954): the
    /// chi-square approximation is doubtful; prefer Fisher's exact test.
    pub approximation_warning: bool,
}

/// Chi-square test of independence (Pearson 1900; Yates 1934 continuity correction for
/// 2 x 2, as SciPy `chi2_contingency(correction=True)` with |O - E| reduced by
/// min(0.5, |O - E|); G test by Wilks 1938). df = (r-1)(c-1). Every row and column must
/// have a positive total.
pub fn chi_square_independence(table: &[Vec<u64>]) -> Result<ChiSquareIndependence> {
    let (r, c) = check_table(table, "chi-square test")?;
    let (rs, cs, n) = margins(table);
    if rs.contains(&0) || cs.contains(&0) {
        return Err(invalid("chi-square test: a row or column total is zero"));
    }
    let nf = n as f64;
    let expected: Vec<Vec<f64>> = rs
        .iter()
        .map(|&ri| cs.iter().map(|&cj| ri as f64 * cj as f64 / nf).collect())
        .collect();
    let mut x2 = Vec::new();
    let mut g = Vec::new();
    let mut yates = Vec::new();
    let mut pres = vec![vec![0.0; c]; r];
    let mut ares = vec![vec![0.0; c]; r];
    for i in 0..r {
        for j in 0..c {
            let o = table[i][j] as f64;
            let e = expected[i][j];
            let d = o - e;
            x2.push(d * d / e);
            let dy = (d.abs() - d.abs().min(0.5)).max(0.0);
            yates.push(dy * dy / e);
            if o > 0.0 {
                g.push(2.0 * o * (o / e).ln());
            }
            pres[i][j] = d / e.sqrt();
            ares[i][j] = d / (e * (1.0 - rs[i] as f64 / nf) * (1.0 - cs[j] as f64 / nf)).sqrt();
        }
    }
    let df = ((r - 1) * (c - 1)) as f64;
    let chi2 = compensated_sum(x2);
    let below5 = expected.iter().flatten().filter(|&&e| e < 5.0).count();
    let min_e = expected
        .iter()
        .flatten()
        .fold(f64::INFINITY, |m, &e| m.min(e));
    let two = r == 2 && c == 2;
    let phi = if two {
        let (a, b, cc, d) = (
            table[0][0] as f64,
            table[0][1] as f64,
            table[1][0] as f64,
            table[1][1] as f64,
        );
        Some((a * d - b * cc) / (rs[0] as f64 * rs[1] as f64 * cs[0] as f64 * cs[1] as f64).sqrt())
    } else {
        None
    };
    Ok(ChiSquareIndependence {
        pearson: chi_stat(chi2, df),
        yates: if two {
            Some(chi_stat(compensated_sum(yates), df))
        } else {
            None
        },
        g_test: chi_stat(compensated_sum(g), df),
        cramers_v: (chi2 / (nf * (r.min(c) - 1) as f64)).sqrt(),
        phi,
        expected,
        pearson_residuals: pres,
        adjusted_residuals: ares,
        n,
        cells_expected_below_5: below5,
        min_expected: min_e,
        approximation_warning: min_e < 1.0 || below5 as f64 > 0.2 * (r * c) as f64,
    })
}

/// Chi-square goodness of fit.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChiSquareGof {
    /// X^2 = sum (O - E)^2/E.
    pub chi2: f64,
    /// k - 1 - ddof.
    pub df: f64,
    /// Upper-tail p.
    pub p: f64,
    /// Expected counts.
    pub expected: Vec<f64>,
    /// Number of categories with expected count < 5.
    pub cells_expected_below_5: usize,
}

/// Chi-square goodness-of-fit test of observed counts against expected proportions
/// (normalised to sum 1; `None` = equal proportions). `ddof` = number of parameters
/// estimated from the data (df = k - 1 - ddof). Pearson (1900); SciPy `chisquare`.
pub fn chi_square_gof(
    observed: &[u64],
    expected_proportions: Option<&[f64]>,
    ddof: usize,
) -> Result<ChiSquareGof> {
    let k = observed.len();
    if k < 2 || k <= 1 + ddof {
        return Err(StatsError::NotEnoughData(
            "goodness of fit needs more categories than 1 + ddof".into(),
        ));
    }
    let n: u64 = observed.iter().sum();
    let props: Vec<f64> = match expected_proportions {
        Some(p) => {
            if p.len() != k || p.iter().any(|&x| !(x > 0.0) || !x.is_finite()) {
                return Err(invalid(
                    "goodness of fit: proportions must be positive, one per category",
                ));
            }
            let s: f64 = compensated_sum(p.iter().copied());
            p.iter().map(|x| x / s).collect()
        }
        None => vec![1.0 / k as f64; k],
    };
    let expected: Vec<f64> = props.iter().map(|p| p * n as f64).collect();
    let chi2 = compensated_sum(
        observed
            .iter()
            .zip(&expected)
            .map(|(&o, &e)| (o as f64 - e).powi(2) / e),
    );
    let df = (k - 1 - ddof) as f64;
    Ok(ChiSquareGof {
        chi2,
        df,
        p: p_upper_chi2(chi2, df),
        cells_expected_below_5: expected.iter().filter(|&&e| e < 5.0).count(),
        expected,
    })
}

// =======================================================================================
// Fisher's exact test, 2 x 2
// =======================================================================================

/// Fisher's exact test on a 2 x 2 table and the conditional maximum-likelihood odds ratio.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Fisher2x2 {
    /// Two-sided p: sum of the probabilities of all tables no more likely than the
    /// observed one (relative tolerance 1e-7, as R and SciPy).
    pub p_two_sided: f64,
    /// P(X <= a): H1 odds ratio < 1.
    pub p_less: f64,
    /// P(X >= a): H1 odds ratio > 1.
    pub p_greater: f64,
    /// Sample odds ratio ad/bc.
    pub odds_ratio_sample: f64,
    /// Conditional MLE of the odds ratio (noncentral hypergeometric).
    pub odds_ratio_conditional_mle: f64,
    /// Exact (Cornfield) CI of the conditional odds ratio at `ci_level`.
    pub odds_ratio_conditional_ci: (f64, f64),
    /// Confidence level.
    pub ci_level: f64,
}

struct NcHyper {
    lo: i64,
    /// ln of the central hypergeometric pmf on lo..=hi.
    logd: Vec<f64>,
}

impl NcHyper {
    fn new(h: &Hypergeometric, lo: i64, hi: i64) -> Self {
        let logd = (lo..=hi).map(|x| h.pmf(x).ln()).collect();
        NcHyper { lo, logd }
    }
    /// Normalised probabilities under odds ratio exp(t).
    fn probs(&self, t: f64) -> Vec<f64> {
        let w: Vec<f64> = self
            .logd
            .iter()
            .enumerate()
            .map(|(i, l)| l + (self.lo + i as i64) as f64 * t)
            .collect();
        let m = w.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let e: Vec<f64> = w.iter().map(|x| (x - m).exp()).collect();
        let s = compensated_sum(e.iter().copied());
        e.into_iter().map(|x| x / s).collect()
    }
    fn mean(&self, t: f64) -> f64 {
        let p = self.probs(t);
        compensated_sum(
            p.iter()
                .enumerate()
                .map(|(i, &pi)| (self.lo + i as i64) as f64 * pi),
        )
    }
    /// (P(X <= x), P(X >= x)) under exp(t).
    fn tails(&self, x: i64, t: f64) -> (f64, f64) {
        let p = self.probs(t);
        let k = (x - self.lo) as usize;
        (
            compensated_sum(p[..=k].iter().copied()),
            compensated_sum(p[k..].iter().copied()),
        )
    }
}

/// Root in t = ln(odds ratio) of an increasing function f.
fn solve_log_or<F: FnMut(f64) -> f64>(mut f: F) -> f64 {
    let (mut a, mut b) = (-1.0, 1.0);
    let mut fa = f(a);
    let mut fb = f(b);
    let mut it = 0;
    while fa > 0.0 && it < 60 {
        b = a;
        fb = fa;
        a *= 2.0;
        fa = f(a);
        it += 1;
    }
    while fb < 0.0 && it < 120 {
        a = b;
        fa = fb;
        b *= 2.0;
        fb = f(b);
        it += 1;
    }
    if fa > 0.0 || fb < 0.0 {
        return f64::NAN;
    }
    roots::brent(f, a, b, 1e-14, 1e-15, 300).unwrap_or(f64::NAN)
}

/// Fisher's exact test for a 2 x 2 table `[[a, b], [c, d]]` (Fisher 1935; two-sided p as
/// R `fisher.test` and SciPy `fisher_exact`), with the conditional MLE of the odds ratio
/// and its exact CI (Cornfield 1956), solved on ln(OR) with Brent's method as R
/// `fisher.test` and SciPy `contingency.odds_ratio(kind='conditional')`: the MLE solves
/// E[X; psi] = a; the CI ends solve P(X >= a; psi_L) = alpha/2 and P(X <= a; psi_U) =
/// alpha/2 (0 and inf at the edges of the support).
pub fn fisher_exact_2x2(table: [[u64; 2]; 2], ci_level: f64) -> Result<Fisher2x2> {
    z_two_sided(ci_level)?;
    let [[a, b], [c, d]] = table;
    let n = a + b + c + d;
    if n == 0 {
        return Err(StatsError::NotEnoughData(
            "Fisher's test: empty table".into(),
        ));
    }
    let h = Hypergeometric::new(n, a + b, a + c)?;
    let lo = (a + c).saturating_sub(c + d) as i64;
    let hi = (a + b).min(a + c) as i64;
    let x = a as i64;
    if hi - lo > 10_000_000 {
        return Err(invalid(
            "Fisher's test: margins too large for exact enumeration; use the chi-square test",
        ));
    }
    let probs: Vec<f64> = (lo..=hi).map(|k| h.pmf(k)).collect();
    let pobs = probs[(x - lo) as usize];
    let p_less = h.cdf(x).min(1.0);
    let p_greater = if x <= lo { 1.0 } else { h.sf(x - 1).min(1.0) };
    let thr = pobs * (1.0 + 1e-7);
    let p_two = compensated_sum(probs.iter().copied().filter(|&p| p <= thr)).min(1.0);
    let alpha = 0.5 * (1.0 - ci_level);
    let nc = NcHyper::new(&h, lo, hi);
    let (mle, ci) = if lo == hi {
        (f64::NAN, (0.0, f64::INFINITY))
    } else {
        let mle = if x == lo {
            0.0
        } else if x == hi {
            f64::INFINITY
        } else {
            solve_log_or(|t| nc.mean(t) - x as f64).exp()
        };
        let lower = if x == lo {
            0.0
        } else {
            solve_log_or(|t| nc.tails(x, t).1 - alpha).exp()
        };
        let upper = if x == hi {
            f64::INFINITY
        } else {
            solve_log_or(|t| alpha - nc.tails(x, t).0).exp()
        };
        (mle, (lower, upper))
    };
    let (af, bf, cf, df) = (a as f64, b as f64, c as f64, d as f64);
    Ok(Fisher2x2 {
        p_two_sided: p_two,
        p_less,
        p_greater,
        odds_ratio_sample: af * df / (bf * cf),
        odds_ratio_conditional_mle: mle,
        odds_ratio_conditional_ci: ci,
        ci_level,
    })
}

/// Odds ratio, relative risk and risk difference of a 2 x 2 table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RiskMeasures {
    /// ad / bc.
    pub odds_ratio: f64,
    /// Woolf (logit) CI exp(ln OR +/- z sqrt(1/a+1/b+1/c+1/d)); None if a cell is 0.
    pub odds_ratio_woolf_ci: Option<(f64, f64)>,
    /// (a/(a+b)) / (c/(c+d)).
    pub relative_risk: f64,
    /// Katz log CI RR exp(+/- z sqrt(1/a - 1/(a+b) + 1/c - 1/(c+d))); None if a or c is 0.
    pub relative_risk_katz_ci: Option<(f64, f64)>,
    /// a/(a+b) - c/(c+d).
    pub risk_difference: f64,
    /// Wald CI of the risk difference.
    pub risk_difference_wald_ci: (f64, f64),
    /// Confidence level.
    pub ci_level: f64,
}

/// Odds ratio with Woolf's CI (Woolf 1955, doi:10.1111/j.1469-1809.1955.tb01285.x),
/// relative risk with the Katz log CI (Katz et al. 1978, doi:10.2307/2530610), and the
/// risk difference with its Wald CI. These are the textbook (and SciPy/statsmodels
/// `Table2x2`) intervals; Prism's defaults (Koopman score CI for RR, Baptista–Pike for OR)
/// are not implemented (no oracle yet).
pub fn risk_measures(table: [[u64; 2]; 2], ci_level: f64) -> Result<RiskMeasures> {
    let z = z_two_sided(ci_level)?;
    let [[a, b], [c, d]] = table;
    if a + b == 0 || c + d == 0 {
        return Err(invalid("risk measures: a row total is zero"));
    }
    let (af, bf, cf, df) = (a as f64, b as f64, c as f64, d as f64);
    let or = af * df / (bf * cf);
    let woolf = if a > 0 && b > 0 && c > 0 && d > 0 {
        let se = (1.0 / af + 1.0 / bf + 1.0 / cf + 1.0 / df).sqrt();
        Some(((or.ln() - z * se).exp(), (or.ln() + z * se).exp()))
    } else {
        None
    };
    let p1 = af / (af + bf);
    let p2 = cf / (cf + df);
    let rr = p1 / p2;
    let katz = if a > 0 && c > 0 {
        let se = (1.0 / af - 1.0 / (af + bf) + 1.0 / cf - 1.0 / (cf + df)).sqrt();
        Some((rr * (-z * se).exp(), rr * (z * se).exp()))
    } else {
        None
    };
    let rd = p1 - p2;
    let se_rd = (p1 * (1.0 - p1) / (af + bf) + p2 * (1.0 - p2) / (cf + df)).sqrt();
    Ok(RiskMeasures {
        odds_ratio: or,
        odds_ratio_woolf_ci: woolf,
        relative_risk: rr,
        relative_risk_katz_ci: katz,
        risk_difference: rd,
        risk_difference_wald_ci: ((rd - z * se_rd).max(-1.0), (rd + z * se_rd).min(1.0)),
        ci_level,
    })
}

// =======================================================================================
// Fisher's exact test, r x c
// =======================================================================================

/// Exact test of independence on an r x c table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FisherRxC {
    /// Sum of the probabilities of all tables with the observed margins that are no more
    /// likely than the observed table (relative tolerance 1e-7, as R `fisher.test`).
    pub p_two_sided: f64,
    /// Probability of the observed table.
    pub p_observed_table: f64,
    /// Number of tables with the observed margins (None if counting them would exceed the
    /// work budget; the p-value does not need it).
    pub n_tables: Option<f64>,
    /// Total probability accounted for (included + excluded mass); 1 up to rounding.
    pub total_probability: f64,
}

/// Work limit (path-value x child expansions) of [`fisher_exact_rxc`].
pub const FISHER_RXC_WORK_LIMIT: u64 = 50_000_000;

/// Largest total count accepted by [`fisher_exact_rxc`].
pub const FISHER_RXC_MAX_N: u64 = 100_000;

/// Limit on the distinct (node, path value) entries held for one stage (memory bound,
/// about 60 bytes each).
pub const FISHER_RXC_PATH_LIMIT: usize = 4_000_000;

struct LnFact(Vec<f64>);

impl LnFact {
    fn new(n: u64) -> Self {
        LnFact((0..=n).map(|k| special::ln_gamma(k as f64 + 1.0)).collect())
    }
    #[inline]
    fn get(&self, k: u64) -> f64 {
        self.0[k as usize]
    }
}

/// Minimum of sum f(n_i) (f = ln n!, convex) over integer n with sum = total and
/// 0 <= n_i <= caps_i: water-filling (greedy on the smallest marginal cost).
fn min_spread(caps: &[u64], total: u64, lf: &LnFact) -> f64 {
    let mut c: Vec<u64> = caps.to_vec();
    c.sort_unstable();
    let mut rem = total;
    let mut s = 0.0;
    let m = c.len();
    for (i, &cap) in c.iter().enumerate() {
        let cnt = (m - i) as u64;
        if cap * cnt <= rem {
            s += lf.get(cap);
            rem -= cap;
        } else {
            let q = rem / cnt;
            let r = rem % cnt;
            s += r as f64 * lf.get(q + 1) + (cnt - r) as f64 * lf.get(q);
            return s;
        }
    }
    s
}

/// Maximum of sum ln n_i! under the same constraints: fill the largest caps first
/// (a convex function is maximised at a vertex; by exchange, the vertex that fills the
/// largest caps).
fn max_concentrate(caps: &[u64], total: u64, lf: &LnFact) -> f64 {
    let mut c: Vec<u64> = caps.to_vec();
    c.sort_unstable_by(|a, b| b.cmp(a));
    let mut rem = total;
    let mut s = 0.0;
    for cap in c {
        if rem == 0 {
            break;
        }
        let x = cap.min(rem);
        s += lf.get(x);
        rem -= x;
    }
    s
}

/// All vectors x with sum x = total, 0 <= x_i <= caps_i.
fn compositions(caps: &[u64], total: u64, out: &mut Vec<Vec<u64>>) {
    fn rec(
        caps: &[u64],
        suffix_cap: &[u64],
        i: usize,
        rem: u64,
        cur: &mut Vec<u64>,
        out: &mut Vec<Vec<u64>>,
    ) {
        if i == caps.len() - 1 {
            if rem <= caps[i] {
                cur.push(rem);
                out.push(cur.clone());
                cur.pop();
            }
            return;
        }
        let rest = suffix_cap[i + 1];
        let lo = rem.saturating_sub(rest);
        let hi = caps[i].min(rem);
        let mut x = lo;
        while x <= hi {
            cur.push(x);
            rec(caps, suffix_cap, i + 1, rem - x, cur, out);
            cur.pop();
            x += 1;
        }
    }
    let mut suffix = vec![0u64; caps.len() + 1];
    for i in (0..caps.len()).rev() {
        suffix[i] = suffix[i + 1] + caps[i];
    }
    if suffix[0] < total {
        return;
    }
    let mut cur = Vec::with_capacity(caps.len());
    rec(caps, &suffix, 0, total, &mut cur, out);
}

/// Fisher's exact test on an r x c table (Freeman & Halton 1951) by a network algorithm in
/// the spirit of Mehta & Patel (1983, JASA 78:427, doi:10.1080/01621459.1983.10477989;
/// FEXACT, TOMS 643): columns are processed one at a time; a node is the multiset of
/// remaining row totals (rows are exchangeable for the remaining sub-table), and paths
/// reaching a node are merged by their accumulated sum of ln n_ij! (to 1e-9). At each
/// node the total probability of all completions is known in closed form
/// (M!/(prod r_i! prod c_j!)), and bounds on the best and worst completion — from the
/// row-relaxed and column-relaxed problems (water-filling and greedy concentration of
/// ln n!) — decide whole sub-networks at once: completely included, completely excluded,
/// or expanded. The p-value is the sum of P over tables with P <= P_obs (1 + 1e-7).
///
/// Limits: the work (merged paths x children) is capped at [`FISHER_RXC_WORK_LIMIT`] and
/// the paths held per stage at [`FISHER_RXC_PATH_LIMIT`]; beyond either a `NoConvergence`
/// error is returned (use the chi-square test). Release-build timings: 2 x 15, 3 x 10 and
/// 4 x 5 tables with N = 200 near independence in 1 to 360 ms, 5 x 6 with N = 200 near
/// independence in 0.6 s; a strongly associated 5 x 6 table with N = 200 (p ~ 1e-20)
/// exceeds the limits (R's FEXACT also needs an enlarged workspace there). Prism itself
/// offers Fisher's test only for 2 x 2 tables.
pub fn fisher_exact_rxc(table: &[Vec<u64>]) -> Result<FisherRxC> {
    let (r0, c0) = check_table(table, "Fisher's exact test")?;
    // Rows = the shorter dimension.
    let t: Vec<Vec<u64>> = if r0 <= c0 {
        table.to_vec()
    } else {
        (0..c0)
            .map(|j| table.iter().map(|row| row[j]).collect())
            .collect()
    };
    let (rs, mut cs, n) = margins(&t);
    if n == 0 {
        return Err(StatsError::NotEnoughData(
            "Fisher's exact test: empty table".into(),
        ));
    }
    if n > FISHER_RXC_MAX_N {
        return Err(StatsError::NoConvergence(format!(
            "Fisher's exact test: N = {n} exceeds {FISHER_RXC_MAX_N}; use the chi-square test"
        )));
    }
    let lf = LnFact::new(n);
    let ln_k = rs.iter().map(|&x| lf.get(x)).sum::<f64>()
        + cs.iter().map(|&x| lf.get(x)).sum::<f64>()
        - lf.get(n);
    let s_obs: f64 = t.iter().flatten().map(|&x| lf.get(x)).sum();
    let p_obs = (ln_k - s_obs).exp();
    let s_needed = s_obs - special::log1p(1e-7);
    // Drop empty rows/columns (they contribute nothing), larger columns first.
    let rows: Vec<u64> = rs.iter().copied().filter(|&x| x > 0).collect();
    cs.retain(|&x| x > 0);
    cs.sort_unstable_by(|a, b| b.cmp(a));
    let ncol = cs.len();
    // suffix log of prod c_k! for k >= j
    let mut col_suffix = vec![0.0; ncol + 1];
    for j in (0..ncol).rev() {
        col_suffix[j] = col_suffix[j + 1] + lf.get(cs[j]);
    }
    let mut start = rows.clone();
    start.sort_unstable_by(|a, b| b.cmp(a));
    // Paths reaching a node, keyed by their sum of ln n! quantised to 1e-9:
    // (sum ln n!, multiplicity).
    type Paths = HashMap<i64, (f64, f64)>;
    let key = |s: f64| (s * 1e9).round() as i64;
    let mut stage: HashMap<Vec<u64>, Paths> = HashMap::new();
    stage.insert(start, HashMap::from([(0, (0.0, 1.0))]));
    let mut included = 0.0;
    let mut excluded = 0.0;
    let mut work: u64 = 0;
    for j in 0..ncol {
        let mut next: HashMap<Vec<u64>, Paths> = HashMap::new();
        let mut stored = 0usize;
        for (rem, paths) in stage {
            let m: u64 = rem.iter().sum();
            let ln_total = lf.get(m) - rem.iter().map(|&x| lf.get(x)).sum::<f64>() - col_suffix[j];
            // Bounds on S_rem = sum ln n! over completions.
            let cols = &cs[j..];
            let lb_c: f64 = cols.iter().map(|&cj| min_spread(&rem, cj, &lf)).sum();
            let lb_r: f64 = rem.iter().map(|&ri| min_spread(cols, ri, &lf)).sum();
            let ub_c: f64 = cols.iter().map(|&cj| max_concentrate(&rem, cj, &lf)).sum();
            let ub_r: f64 = rem.iter().map(|&ri| max_concentrate(cols, ri, &lf)).sum();
            let lb = lb_c.max(lb_r);
            let ub = ub_c.min(ub_r);
            let mut undecided: Vec<(f64, f64)> = Vec::new();
            for (_, (s, w)) in paths {
                let mass = w * (ln_k - s + ln_total).exp();
                if s + lb >= s_needed {
                    included += mass;
                } else if s + ub < s_needed {
                    excluded += mass;
                } else {
                    undecided.push((s, w));
                }
            }
            if undecided.is_empty() {
                continue;
            }
            let mut kids = Vec::new();
            compositions(&rem, cs[j], &mut kids);
            work += (kids.len() * undecided.len()) as u64;
            if work > FISHER_RXC_WORK_LIMIT {
                return Err(StatsError::NoConvergence(
                    "Fisher's exact test: table too large for the exact network algorithm; use the chi-square test".into(),
                ));
            }
            for x in kids {
                let ds: f64 = x.iter().map(|&v| lf.get(v)).sum();
                let mut child: Vec<u64> = rem.iter().zip(&x).map(|(a, b)| a - b).collect();
                child.sort_unstable_by(|a, b| b.cmp(a));
                let entry = next.entry(child).or_default();
                for &(s, w) in &undecided {
                    let sv = s + ds;
                    let e = entry.entry(key(sv)).or_insert_with(|| {
                        stored += 1;
                        (sv, 0.0)
                    });
                    e.1 += w;
                }
                if stored > FISHER_RXC_PATH_LIMIT {
                    return Err(StatsError::NoConvergence(
                        "Fisher's exact test: table too large for the exact network algorithm; use the chi-square test".into(),
                    ));
                }
            }
        }
        stage = next;
    }
    // Every path ends at the empty node after the last column.
    for (_, paths) in stage {
        for (_, (s, w)) in paths {
            let mass = w * (ln_k - s).exp();
            if s >= s_needed {
                included += mass;
            } else {
                excluded += mass;
            }
        }
    }
    Ok(FisherRxC {
        p_two_sided: included.min(1.0),
        p_observed_table: p_obs,
        n_tables: count_tables(&rows, &cs, 2_000_000),
        total_probability: included + excluded,
    })
}

/// Number of tables with the given margins by dynamic programming over the same network,
/// or None if more than `budget` nodes would be visited.
fn count_tables(rows: &[u64], cols: &[u64], budget: usize) -> Option<f64> {
    let mut start = rows.to_vec();
    start.sort_unstable_by(|a, b| b.cmp(a));
    let mut stage: HashMap<Vec<u64>, f64> = HashMap::new();
    stage.insert(start, 1.0);
    let mut visited = 0usize;
    for &cj in cols {
        let mut next: HashMap<Vec<u64>, f64> = HashMap::new();
        for (rem, w) in stage {
            let mut kids = Vec::new();
            compositions(&rem, cj, &mut kids);
            visited += kids.len();
            if visited > budget {
                return None;
            }
            for x in kids {
                let mut child: Vec<u64> = rem.iter().zip(&x).map(|(a, b)| a - b).collect();
                child.sort_unstable_by(|a, b| b.cmp(a));
                *next.entry(child).or_default() += w;
            }
        }
        stage = next;
    }
    Some(stage.values().sum())
}

// =======================================================================================
// McNemar, Cochran–Armitage
// =======================================================================================

/// McNemar's test for paired proportions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McNemar {
    /// Discordant count table[0][1].
    pub b: u64,
    /// Discordant count table[1][0].
    pub c: u64,
    /// (b - c)^2 / (b + c).
    pub chi2: f64,
    /// Its p (1 df).
    pub p: f64,
    /// Continuity-corrected (|b - c| - 1)^2 / (b + c) (Edwards 1948; Prism's choice).
    pub chi2_corrected: f64,
    /// Its p.
    pub p_corrected: f64,
    /// Exact two-sided binomial p = min(1, 2 P(X <= min(b, c))), X ~ B(b + c, 1/2).
    pub p_exact: f64,
}

/// McNemar's test (McNemar 1947, Psychometrika 12:153, doi:10.1007/BF02295996) on a 2 x 2
/// table of paired outcomes; the discordant pairs are table[0][1] and table[1][0].
/// statsmodels `mcnemar(exact=True|False, correction=...)`.
pub fn mcnemar(table: [[u64; 2]; 2]) -> Result<McNemar> {
    let (b, c) = (table[0][1], table[1][0]);
    let m = b + c;
    if m == 0 {
        return Err(StatsError::NotEnoughData(
            "McNemar: no discordant pairs".into(),
        ));
    }
    let (bf, cf, mf) = (b as f64, c as f64, m as f64);
    let chi2 = (bf - cf).powi(2) / mf;
    let cc = ((bf - cf).abs() - 1.0).max(0.0).powi(2) / mf;
    let bin = Binomial::new(m, 0.5)?;
    Ok(McNemar {
        b,
        c,
        chi2,
        p: p_upper_chi2(chi2, 1.0),
        chi2_corrected: cc,
        p_corrected: p_upper_chi2(cc, 1.0),
        p_exact: (2.0 * bin.cdf(b.min(c) as i64)).min(1.0),
    })
}

/// Cochran–Armitage test for trend in proportions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CochranArmitage {
    /// z = sum t_i (x_i - n_i pbar) / sqrt(pbar (1 - pbar) [sum n_i t_i^2 - (sum n_i t_i)^2/N]).
    pub z: f64,
    /// z^2 (R `prop.trend.test`; Prism's "chi-square test for trend").
    pub chi2: f64,
    /// Two-sided p.
    pub p_two_sided: f64,
    /// P(Z <= z): proportions decrease with the score.
    pub p_less: f64,
    /// P(Z >= z): proportions increase with the score.
    pub p_greater: f64,
    /// The variant with the variance multiplied by N/(N-1) (some texts).
    pub z_n_over_n_minus_1: f64,
}

/// Cochran–Armitage trend test (Cochran 1954, doi:10.2307/3001616; Armitage 1955,
/// doi:10.2307/3001775) for successes out of totals at ordered scores (default 0, 1, 2,
/// ...). Without the N/(N-1) factor, so chi2 = z^2 equals R's `prop.trend.test`.
pub fn cochran_armitage(
    successes: &[u64],
    totals: &[u64],
    scores: Option<&[f64]>,
) -> Result<CochranArmitage> {
    let k = successes.len();
    if k < 2 || totals.len() != k {
        return Err(invalid(
            "Cochran-Armitage: need at least 2 groups and matching lengths",
        ));
    }
    if successes.iter().zip(totals).any(|(s, t)| s > t) {
        return Err(invalid("Cochran-Armitage: successes exceed totals"));
    }
    let t: Vec<f64> = match scores {
        Some(s) if s.len() == k => s.to_vec(),
        Some(_) => return Err(invalid("Cochran-Armitage: one score per group")),
        None => (0..k).map(|i| i as f64).collect(),
    };
    let nn: f64 = totals.iter().sum::<u64>() as f64;
    let xs: f64 = successes.iter().sum::<u64>() as f64;
    let pbar = xs / nn;
    let num =
        compensated_sum((0..k).map(|i| t[i] * (successes[i] as f64 - totals[i] as f64 * pbar)));
    let s1 = compensated_sum((0..k).map(|i| totals[i] as f64 * t[i]));
    let s2 = compensated_sum((0..k).map(|i| totals[i] as f64 * t[i] * t[i]));
    let var = pbar * (1.0 - pbar) * (s2 - s1 * s1 / nn);
    if !(var > 0.0) {
        return Err(invalid(
            "Cochran-Armitage: zero variance (constant scores or proportions 0/1)",
        ));
    }
    let z = num / var.sqrt();
    Ok(CochranArmitage {
        z,
        chi2: z * z,
        p_two_sided: p_two_sided_z(z),
        p_less: std_normal_cdf(z),
        p_greater: std_normal_sf(z),
        z_n_over_n_minus_1: z * ((nn - 1.0) / nn).sqrt(),
    })
}

// =======================================================================================
// Diagnostic accuracy, Wilson interval
// =======================================================================================

/// Wilson score interval for a binomial proportion (Wilson 1927, JASA 22:209):
/// (p + z^2/2n +/- z sqrt(p(1-p)/n + z^2/4n^2)) / (1 + z^2/n). No continuity correction.
pub fn wilson_ci(successes: u64, n: u64, level: f64) -> Result<(f64, f64)> {
    let z = z_two_sided(level)?;
    if n == 0 || successes > n {
        return Err(invalid("Wilson CI: need 0 <= successes <= n, n > 0"));
    }
    let nf = n as f64;
    let p = successes as f64 / nf;
    let z2 = z * z;
    let centre = p + z2 / (2.0 * nf);
    let half = z * (p * (1.0 - p) / nf + z2 / (4.0 * nf * nf)).sqrt();
    let den = 1.0 + z2 / nf;
    Ok((
        ((centre - half) / den).max(0.0),
        ((centre + half) / den).min(1.0),
    ))
}

/// A proportion with its Wilson CI.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Proportion {
    /// Estimate.
    pub value: f64,
    /// Wilson score CI (None when the denominator is 0).
    pub ci: Option<(f64, f64)>,
}

/// Diagnostic accuracy of a test.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    /// TP / (TP + FN).
    pub sensitivity: Proportion,
    /// TN / (TN + FP).
    pub specificity: Proportion,
    /// TP / (TP + FP).
    pub ppv: Proportion,
    /// TN / (TN + FN).
    pub npv: Proportion,
    /// LR+ = sensitivity / (1 - specificity).
    pub lr_positive: f64,
    /// Log-method CI of LR+ (Simel, Samsa & Matchar 1991), None when undefined.
    pub lr_positive_ci: Option<(f64, f64)>,
    /// LR- = (1 - sensitivity) / specificity.
    pub lr_negative: f64,
    /// Log-method CI of LR-.
    pub lr_negative_ci: Option<(f64, f64)>,
    /// Confidence level.
    pub ci_level: f64,
}

/// Sensitivity, specificity, predictive values (Wilson CIs) and likelihood ratios (log
/// CIs, Simel, Samsa & Matchar 1991, J Clin Epidemiol 44:763) of a 2 x 2 diagnostic table
/// `[[TP, FP], [FN, TN]]` (rows: test positive/negative; columns: disease present/absent,
/// Prism's layout). PPV and NPV assume the table's prevalence.
pub fn diagnostic(table: [[u64; 2]; 2], ci_level: f64) -> Result<Diagnostic> {
    let z = z_two_sided(ci_level)?;
    let [[tp, fp], [fneg, tn]] = table;
    let prop = |x: u64, d: u64| -> Result<Proportion> {
        Ok(if d == 0 {
            Proportion {
                value: f64::NAN,
                ci: None,
            }
        } else {
            Proportion {
                value: x as f64 / d as f64,
                ci: Some(wilson_ci(x, d, ci_level)?),
            }
        })
    };
    let sens = prop(tp, tp + fneg)?;
    let spec = prop(tn, tn + fp)?;
    let (n1, n2) = ((tp + fneg) as f64, (tn + fp) as f64);
    let lrp = sens.value / (1.0 - spec.value);
    let lrn = (1.0 - sens.value) / spec.value;
    let log_ci = |lr: f64, a: u64, b: u64| -> Option<(f64, f64)> {
        // var ln LR = 1/a - 1/n1 + 1/b - 1/n2 with a, b the numerator/denominator counts.
        if a == 0 || b == 0 || !lr.is_finite() {
            return None;
        }
        let se = (1.0 / a as f64 - 1.0 / n1 + 1.0 / b as f64 - 1.0 / n2).sqrt();
        Some((lr * (-z * se).exp(), lr * (z * se).exp()))
    };
    Ok(Diagnostic {
        lr_positive_ci: log_ci(lrp, tp, fp),
        lr_negative_ci: log_ci(lrn, fneg, tn),
        sensitivity: sens,
        specificity: spec,
        ppv: prop(tp, tp + fp)?,
        npv: prop(tn, tn + fneg)?,
        lr_positive: lrp,
        lr_negative: lrn,
        ci_level,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plain enumeration of every table with the margins (reference for the network).
    fn brute(table: &[Vec<u64>]) -> (f64, f64) {
        let (rs, cs, n) = margins(table);
        let lf = LnFact::new(n);
        let ln_k = rs.iter().map(|&x| lf.get(x)).sum::<f64>()
            + cs.iter().map(|&x| lf.get(x)).sum::<f64>()
            - lf.get(n);
        let s_obs: f64 = table.iter().flatten().map(|&x| lf.get(x)).sum();
        let thr = s_obs - special::log1p(1e-7);
        let mut p = 0.0;
        let mut count = 0.0;
        fn rec(j: usize, rem: Vec<u64>, cs: &[u64], s: f64, lf: &LnFact, f: &mut dyn FnMut(f64)) {
            if j == cs.len() {
                f(s);
                return;
            }
            let mut kids = Vec::new();
            compositions(&rem, cs[j], &mut kids);
            for x in kids {
                let ds: f64 = x.iter().map(|&v| lf.get(v)).sum();
                let child: Vec<u64> = rem.iter().zip(&x).map(|(a, b)| a - b).collect();
                rec(j + 1, child, cs, s + ds, lf, f);
            }
        }
        rec(0, rs.clone(), &cs, 0.0, &lf, &mut |s| {
            count += 1.0;
            if s >= thr {
                p += (ln_k - s).exp();
            }
        });
        (p, count)
    }

    #[test]
    fn network_matches_enumeration() {
        // Deterministic pseudo-random tables.
        let mut seed: u64 = 12345;
        let mut next = |m: u64| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) % m
        };
        for _ in 0..60 {
            let r = 2 + next(3) as usize;
            let c = 2 + next(3) as usize;
            let t: Vec<Vec<u64>> = (0..r).map(|_| (0..c).map(|_| next(6)).collect()).collect();
            let (rs, cs, _) = margins(&t);
            if rs.contains(&0) || cs.contains(&0) {
                continue;
            }
            let got = fisher_exact_rxc(&t).unwrap();
            let (p, count) = brute(&t);
            assert!(
                (got.p_two_sided - p).abs() <= 1e-10 * p.max(1e-300) + 1e-15,
                "{t:?}: {} vs {p}",
                got.p_two_sided
            );
            assert_eq!(got.n_tables, Some(count));
            assert!((got.total_probability - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn wilson_hand() {
        // 8/10 at 95%: z = 1.959964; textbook (0.4902, 0.9433).
        let (l, u) = wilson_ci(8, 10, 0.95).unwrap();
        assert!(
            (l - 0.490_162).abs() < 1e-6 && (u - 0.943_317).abs() < 1e-6,
            "{l} {u}"
        );
    }

    #[test]
    fn diagnostic_hand() {
        let d = diagnostic([[90, 20], [10, 80]], 0.95).unwrap();
        assert!((d.sensitivity.value - 0.9).abs() < 1e-15);
        assert!((d.specificity.value - 0.8).abs() < 1e-15);
        assert!((d.lr_positive - 4.5).abs() < 1e-13);
        assert!((d.lr_negative - 0.125).abs() < 1e-14);
        assert!((d.ppv.value - 90.0 / 110.0).abs() < 1e-15);
    }
}
