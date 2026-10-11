//! Mann–Whitney U test (Wilcoxon rank-sum) with the Hodges–Lehmann shift estimate.

use crate::dist::{std_normal_cdf, std_normal_sf};
use crate::rank::{rank_with_ties, tie_term};
use crate::special;
use crate::ttest::Alternative;
use crate::{Result, StatsError};

/// How to compute the p-value of a rank test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Method {
    /// Exact when there are no ties (and, for Mann–Whitney, n1 n2 <= 10,000; for Wilcoxon,
    /// no zeros and n <= 50), asymptotic otherwise.
    #[default]
    Auto,
    /// Exact permutation distribution; an error if ties make it unavailable.
    Exact,
    /// Normal approximation with tie correction.
    Asymptotic,
}

/// Options for [`mann_whitney`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MannWhitneyOptions {
    /// Alternative for x vs y ("greater": x tends to be larger).
    pub alternative: Alternative,
    /// Exact or asymptotic.
    pub method: Method,
    /// Continuity correction (0.5) in the asymptotic p-value (default true, as SciPy).
    pub continuity: bool,
    /// Confidence level of the Hodges–Lehmann CI.
    pub ci_level: f64,
}

impl Default for MannWhitneyOptions {
    fn default() -> Self {
        MannWhitneyOptions {
            alternative: Alternative::TwoSided,
            method: Method::Auto,
            continuity: true,
            ci_level: 0.95,
        }
    }
}

/// Result of [`mann_whitney`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MannWhitneyResult {
    /// Human-readable description of how p was computed.
    pub method: String,
    /// Whether the p-value is exact.
    pub exact: bool,
    /// U for x: number of pairs with x_i > y_j, ties counting 1/2 (SciPy's statistic).
    pub u1: f64,
    /// U for y = n1 n2 - U1.
    pub u2: f64,
    /// min(U1, U2), the U Prism reports.
    pub u: f64,
    /// Sum of the ranks of x in the pooled sample.
    pub rank_sum_x: f64,
    /// Sum of the ranks of y.
    pub rank_sum_y: f64,
    /// Sizes.
    pub n1: usize,
    /// Size of y.
    pub n2: usize,
    /// Whether the pooled sample has ties.
    pub has_ties: bool,
    /// Tie-corrected z = (U1 - n1 n2/2)/sigma, without continuity correction.
    pub z: f64,
    /// p for the requested alternative.
    pub p: f64,
    /// Two-sided p.
    pub p_two_sided: f64,
    /// One-sided p, H1: x tends to be smaller.
    pub p_less: f64,
    /// One-sided p, H1: x tends to be larger.
    pub p_greater: f64,
    /// Hodges–Lehmann estimate of the shift x - y: the median of all n1 n2 differences.
    pub hodges_lehmann: f64,
    /// CI of the shift (Moses' order-statistic method), if attainable at the level.
    pub ci: Option<(f64, f64)>,
    /// Requested confidence level.
    pub ci_level: f64,
    /// Actual coverage of the exact CI (discrete), or the nominal level when asymptotic.
    pub ci_achieved_level: Option<f64>,
}

/// Exact null distribution of U for sample sizes (m, n) without ties: counts c(u),
/// u = 0..=mn, by the recurrence c_{m,n}(u) = c_{m-1,n}(u-n) + c_{m,n-1}(u)
/// (Mann & Whitney 1947; the frequency recursion used by Löffler 1983 and SciPy's
/// `_mwu_state`). Additions only, so tail counts are exact up to f64 rounding.
/// Returns probabilities (counts / C(m+n, m)).
pub fn u_distribution(m: usize, n: usize) -> Vec<f64> {
    let (m, n) = if m <= n { (m, n) } else { (n, m) };
    // prev[j] = distribution for (i-1, j)
    let mut prev: Vec<Vec<f64>> = (0..=n).map(|_| vec![1.0]).collect();
    for i in 1..=m {
        let mut cur: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
        cur.push(vec![1.0]);
        for j in 1..=n {
            let len = i * j + 1;
            let mut c = vec![0.0; len];
            // c_{i,j-1}(u)
            for (u, v) in cur[j - 1].iter().enumerate() {
                c[u] += v;
            }
            // c_{i-1,j}(u - j)
            for (u, v) in prev[j].iter().enumerate() {
                c[u + j] += v;
            }
            cur.push(c);
        }
        prev = cur;
    }
    let counts = prev.pop().unwrap_or_else(|| vec![1.0]);
    let total: f64 = counts.iter().sum();
    counts.iter().map(|c| c / total).collect()
}

fn clean(x: &[f64], what: &str) -> Result<Vec<f64>> {
    let mut v = Vec::with_capacity(x.len());
    for &a in x {
        if a.is_nan() {
            continue;
        }
        if a.is_infinite() {
            return Err(StatsError::InvalidArgument(format!(
                "infinite value in {what}"
            )));
        }
        v.push(a);
    }
    Ok(v)
}

fn median_sorted(s: &[f64]) -> f64 {
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// Mann–Whitney U test of x vs y (Mann & Whitney 1947; Wilcoxon 1945).
///
/// * Exact p (no ties, n1 n2 <= 10,000 under [`Method::Auto`]) from [`u_distribution`];
///   two-sided p = min(1, 2 min(P(U <= U1), P(U >= U1))).
/// * Asymptotic p from z = (U1 - n1 n2/2 -/+ cc)/sigma with the tie-corrected
///   sigma^2 = (n1 n2/12)((N+1) - sum(t^3 - t)/(N(N-1))) and cc = 0.5 when `continuity`
///   (SciPy `mannwhitneyu(method='asymptotic')`).
/// * Hodges–Lehmann shift: median of all x_i - y_j (Hodges & Lehmann 1963); CI by Moses'
///   method: the k-th smallest and k-th largest differences, k the largest integer with
///   P(U <= k-1) <= alpha/2 under the exact distribution, or
///   k = round(n1 n2/2 - z_{alpha/2} sigma) asymptotically (Hollander & Wolfe 1999, §4.3).
///
/// Prism note: Prism computes an exact p with ties for groups up to 100 values; this
/// implementation switches to the asymptotic method whenever ties are present and says so
/// in `method`.
pub fn mann_whitney(x: &[f64], y: &[f64], opts: &MannWhitneyOptions) -> Result<MannWhitneyResult> {
    let a = clean(x, "x")?;
    let b = clean(y, "y")?;
    if a.is_empty() || b.is_empty() {
        return Err(StatsError::NotEnoughData(
            "Mann–Whitney needs at least one value per group".into(),
        ));
    }
    if !(opts.ci_level > 0.0 && opts.ci_level < 1.0) {
        return Err(StatsError::InvalidArgument(
            "ci_level must be in (0, 1)".into(),
        ));
    }
    let (n1, n2) = (a.len(), b.len());
    let pooled: Vec<f64> = a.iter().chain(b.iter()).copied().collect();
    let (ranks, ties) = rank_with_ties(&pooled);
    let r1: f64 = ranks[..n1].iter().sum();
    let r2: f64 = ranks[n1..].iter().sum();
    let (n1f, n2f) = (n1 as f64, n2 as f64);
    let u1 = r1 - n1f * (n1f + 1.0) / 2.0;
    let u2 = n1f * n2f - u1;
    let has_ties = !ties.is_empty();
    let nn = n1f + n2f;
    let mu = n1f * n2f / 2.0;
    let var = n1f * n2f / 12.0 * ((nn + 1.0) - tie_term(&ties) / (nn * (nn - 1.0)).max(1.0));
    let sigma = var.sqrt();
    let z = if sigma > 0.0 { (u1 - mu) / sigma } else { 0.0 };

    let exact_ok = !has_ties && n1 * n2 <= 10_000;
    let use_exact = match opts.method {
        Method::Auto => exact_ok,
        Method::Exact => {
            if has_ties {
                return Err(StatsError::InvalidArgument(
                    "exact Mann–Whitney p is not available with ties; use the asymptotic method"
                        .into(),
                ));
            }
            if n1 * n2 > 40_000 {
                return Err(StatsError::InvalidArgument(
                    "exact Mann–Whitney p is limited to n1 n2 <= 40,000".into(),
                ));
            }
            true
        }
        Method::Asymptotic => false,
    };

    let mut dist = None;
    let (p_less, p_greater, p_two, method) = if use_exact {
        let d = u_distribution(n1, n2);
        let k = u1.round() as usize;
        let lower: f64 = d[..=k.min(d.len() - 1)].iter().sum();
        let upper: f64 = d[k.min(d.len() - 1)..].iter().sum();
        let (pl, pg) = (lower.min(1.0), upper.min(1.0));
        dist = Some(d);
        (
            pl,
            pg,
            (2.0 * pl.min(pg)).min(1.0),
            "exact (no ties)".to_string(),
        )
    } else if sigma > 0.0 {
        let cc = if opts.continuity { 0.5 } else { 0.0 };
        let pl = std_normal_cdf((u1 - mu + cc) / sigma);
        let pg = std_normal_sf((u1 - mu - cc) / sigma);
        let p2 = (2.0 * std_normal_sf(((u1 - mu).abs() - cc) / sigma)).min(1.0);
        let why = if has_ties {
            "asymptotic, tie-corrected (ties present; Prism would enumerate the exact distribution)"
        } else {
            "asymptotic"
        };
        (
            pl,
            pg,
            p2,
            format!(
                "{why}{}",
                if opts.continuity {
                    ", continuity-corrected"
                } else {
                    ""
                }
            ),
        )
    } else {
        (1.0, 1.0, 1.0, "degenerate: all values tied".to_string())
    };
    let p = match opts.alternative {
        Alternative::TwoSided => p_two,
        Alternative::Less => p_less,
        Alternative::Greater => p_greater,
    };

    // Hodges–Lehmann and Moses CI.
    let mut diffs = Vec::with_capacity(n1 * n2);
    for &xi in &a {
        for &yj in &b {
            diffs.push(xi - yj);
        }
    }
    diffs.sort_by(f64::total_cmp);
    let hl = median_sorted(&diffs);
    let mn = diffs.len();
    let alpha = 1.0 - opts.ci_level;
    let (ci, achieved) = if let Some(d) = dist.as_ref() {
        // largest k with P(U <= k-1) <= alpha/2
        let mut cum = 0.0;
        let mut k = 0usize;
        for (u, pu) in d.iter().enumerate() {
            if cum + pu > alpha / 2.0 + 1e-12 {
                k = u;
                break;
            }
            cum += pu;
        }
        if k >= 1 {
            (Some((diffs[k - 1], diffs[mn - k])), Some(1.0 - 2.0 * cum))
        } else {
            (None, None)
        }
    } else {
        let zc = special::norm_quantile_upper(alpha / 2.0);
        let kf = (mu - zc * sigma).round();
        if kf >= 1.0 && (kf as usize) <= mn {
            let k = kf as usize;
            (Some((diffs[k - 1], diffs[mn - k])), Some(opts.ci_level))
        } else {
            (None, None)
        }
    };

    Ok(MannWhitneyResult {
        method,
        exact: use_exact,
        u1,
        u2,
        u: u1.min(u2),
        rank_sum_x: r1,
        rank_sum_y: r2,
        n1,
        n2,
        has_ties,
        z,
        p,
        p_two_sided: p_two,
        p_less,
        p_greater,
        hodges_lehmann: hl,
        ci,
        ci_level: opts.ci_level,
        ci_achieved_level: achieved,
    })
}
