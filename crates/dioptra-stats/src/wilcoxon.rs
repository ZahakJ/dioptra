//! Wilcoxon signed-rank test with Hodges–Lehmann pseudo-median.

use crate::dist::{std_normal_cdf, std_normal_sf};
use crate::mannwhitney::Method;
use crate::rank::{rank_with_ties, tie_term};
use crate::special;
use crate::ttest::Alternative;
use crate::{Result, StatsError};

/// Treatment of zero differences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ZeroMethod {
    /// Drop zeros before ranking (Wilcoxon 1945; Prism's default).
    #[default]
    Wilcoxon,
    /// Rank zeros with the rest, then drop their ranks from W+ and W- (Pratt 1959; Prism option).
    Pratt,
}

/// Options for [`signed_rank`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WilcoxonOptions {
    /// Alternative ("greater": the differences tend to be positive).
    pub alternative: Alternative,
    /// Exact or asymptotic.
    pub method: Method,
    /// Zero handling.
    pub zero_method: ZeroMethod,
    /// Continuity correction in the normal approximation (default false, SciPy's default).
    pub continuity: bool,
    /// Confidence level of the Hodges–Lehmann CI.
    pub ci_level: f64,
}

impl Default for WilcoxonOptions {
    fn default() -> Self {
        WilcoxonOptions {
            alternative: Alternative::TwoSided,
            method: Method::Auto,
            zero_method: ZeroMethod::Wilcoxon,
            continuity: false,
            ci_level: 0.95,
        }
    }
}

/// Result of [`signed_rank`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WilcoxonResult {
    /// How p was computed.
    pub method: String,
    /// Whether p is exact.
    pub exact: bool,
    /// Sum of ranks of positive differences.
    pub w_plus: f64,
    /// Sum of ranks of negative differences.
    pub w_minus: f64,
    /// Number of differences (after dropping missing values).
    pub n: usize,
    /// Number of zero differences.
    pub n_zero: usize,
    /// Whether the nonzero |differences| have ties.
    pub has_ties: bool,
    /// z = (W+ - E W+)/sd, tie- and zero-adjusted, without continuity correction.
    pub z: f64,
    /// p for the requested alternative.
    pub p: f64,
    /// Two-sided p.
    pub p_two_sided: f64,
    /// One-sided p, H1: differences tend to be negative.
    pub p_less: f64,
    /// One-sided p, H1: differences tend to be positive.
    pub p_greater: f64,
    /// Hodges–Lehmann pseudo-median: median of the Walsh averages (d_i + d_j)/2, i <= j,
    /// zeros kept.
    pub hodges_lehmann: f64,
    /// The same with zero differences dropped first.
    pub hodges_lehmann_nonzero: f64,
    /// CI of the pseudo-median from the Walsh averages (Tukey; Hollander & Wolfe §3.2).
    pub ci: Option<(f64, f64)>,
    /// Requested level.
    pub ci_level: f64,
    /// Actual coverage of the exact CI, or the nominal level when asymptotic.
    pub ci_achieved_level: Option<f64>,
}

/// Exact null distribution of W+ for n untied, nonzero differences: the coefficients of
/// prod_{i=1}^n (1 + q^i) / 2^n, built by a subset-sum recurrence (additions only).
/// Index w = 0..=n(n+1)/2.
pub fn signed_rank_distribution(n: usize) -> Vec<f64> {
    let max = n * (n + 1) / 2;
    let mut c = vec![0.0; max + 1];
    c[0] = 1.0;
    let mut top = 0;
    for i in 1..=n {
        for w in (0..=top).rev() {
            let v = c[w];
            if v != 0.0 {
                c[w + i] += v;
            }
        }
        top += i;
    }
    let total: f64 = c.iter().sum();
    c.iter().map(|v| v / total).collect()
}

fn median_sorted(s: &[f64]) -> f64 {
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

fn walsh(d: &[f64]) -> Vec<f64> {
    let mut w = Vec::with_capacity(d.len() * (d.len() + 1) / 2);
    for i in 0..d.len() {
        for j in i..d.len() {
            w.push(0.5 * (d[i] + d[j]));
        }
    }
    w.sort_by(f64::total_cmp);
    w
}

/// Wilcoxon signed-rank test on paired data (differences x_i - y_i; incomplete pairs dropped).
pub fn paired(x: &[f64], y: &[f64], opts: &WilcoxonOptions) -> Result<WilcoxonResult> {
    if x.len() != y.len() {
        return Err(StatsError::InvalidArgument(
            "paired data need equal lengths".into(),
        ));
    }
    let d: Vec<f64> = x
        .iter()
        .zip(y)
        .filter(|(a, b)| !a.is_nan() && !b.is_nan())
        .map(|(a, b)| a - b)
        .collect();
    signed_rank(&d, opts)
}

/// One-sample Wilcoxon signed-rank test of H0: median = mu (differences x_i - mu).
pub fn one_sample(x: &[f64], mu: f64, opts: &WilcoxonOptions) -> Result<WilcoxonResult> {
    let d: Vec<f64> = x.iter().filter(|a| !a.is_nan()).map(|a| a - mu).collect();
    signed_rank(&d, opts)
}

/// Wilcoxon signed-rank test on differences `d` (Wilcoxon 1945).
///
/// * Exact p by the generating function prod (1 + q^i) (no ties, no zeros, n <= 50 under
///   [`Method::Auto`]); two-sided p = min(1, 2 min(P(W+ <= w), P(W+ >= w))).
/// * Asymptotic: mean n(n+1)/4 and variance n(n+1)(2n+1)/24 - sum(t^3-t)/48; with Pratt
///   zeros both are reduced by the zeros' share n0(n0+1)/4 and n0(n0+1)(2n0+1)/24
///   (Cureton 1967), ties counted among the nonzero ranks, as SciPy `wilcoxon`.
///   The continuity correction (if requested) moves W+ by 0.5 toward the mean.
/// * Hodges–Lehmann pseudo-median of the Walsh averages with the CI of Tukey/Hollander &
///   Wolfe: the k-th smallest and largest Walsh averages, k the largest integer with
///   P(W+ <= k-1) <= alpha/2 (exact, n <= 50 and no ties) or
///   k = round(n(n+1)/4 - z_{alpha/2} sd) otherwise.
///
/// Prism note: Prism computes exact p-values with ties for fewer than 200 pairs; with ties
/// or zeros this implementation uses the asymptotic method and says so in `method`.
pub fn signed_rank(d: &[f64], opts: &WilcoxonOptions) -> Result<WilcoxonResult> {
    let mut all = Vec::with_capacity(d.len());
    for &v in d {
        if v.is_nan() {
            continue;
        }
        if v.is_infinite() {
            return Err(StatsError::InvalidArgument("infinite difference".into()));
        }
        all.push(v);
    }
    if !(opts.ci_level > 0.0 && opts.ci_level < 1.0) {
        return Err(StatsError::InvalidArgument(
            "ci_level must be in (0, 1)".into(),
        ));
    }
    let n_zero = all.iter().filter(|&&v| v == 0.0).count();
    let nonzero: Vec<f64> = all.iter().copied().filter(|&v| v != 0.0).collect();
    if nonzero.is_empty() {
        return Err(StatsError::NotEnoughData(
            "Wilcoxon signed-rank test needs at least one nonzero difference".into(),
        ));
    }
    let (ranked, n_rank, n0) = match opts.zero_method {
        ZeroMethod::Wilcoxon => (nonzero.clone(), nonzero.len(), 0usize),
        ZeroMethod::Pratt => (all.clone(), all.len(), n_zero),
    };
    let absd: Vec<f64> = ranked.iter().map(|v| v.abs()).collect();
    let (ranks, _) = rank_with_ties(&absd);
    let mut wp = 0.0;
    let mut wm = 0.0;
    let mut nz_ranks = Vec::with_capacity(ranks.len());
    for (r, v) in ranks.iter().zip(&ranked) {
        if *v > 0.0 {
            wp += r;
        } else if *v < 0.0 {
            wm += r;
        }
        if *v != 0.0 {
            nz_ranks.push(*r);
        }
    }
    let (_, ties) = rank_with_ties(&nz_ranks);
    let has_ties = !ties.is_empty();
    let nf = n_rank as f64;
    let n0f = n0 as f64;
    let mean = nf * (nf + 1.0) / 4.0 - n0f * (n0f + 1.0) / 4.0;
    let var = (nf * (nf + 1.0) * (2.0 * nf + 1.0) - n0f * (n0f + 1.0) * (2.0 * n0f + 1.0)) / 24.0
        - tie_term(&ties) / 48.0;
    let sd = var.max(0.0).sqrt();
    let z = if sd > 0.0 { (wp - mean) / sd } else { 0.0 };

    let exact_ok = !has_ties && n_zero == 0 && nonzero.len() <= 50;
    let use_exact = match opts.method {
        Method::Auto => exact_ok,
        Method::Exact => {
            if has_ties || n_zero > 0 {
                return Err(StatsError::InvalidArgument("exact Wilcoxon p is not available with ties or zero differences; use the asymptotic method".into()));
            }
            if nonzero.len() > 1000 {
                return Err(StatsError::InvalidArgument(
                    "exact Wilcoxon p limited to n <= 1000".into(),
                ));
            }
            true
        }
        Method::Asymptotic => false,
    };
    let mut dist = None;
    let (p_less, p_greater, p_two, method) = if use_exact {
        let dd = signed_rank_distribution(nonzero.len());
        let k = (wp.round() as usize).min(dd.len() - 1);
        let pl: f64 = dd[..=k].iter().sum::<f64>().min(1.0);
        let pg: f64 = dd[k..].iter().sum::<f64>().min(1.0);
        dist = Some(dd);
        (
            pl,
            pg,
            (2.0 * pl.min(pg)).min(1.0),
            "exact (no ties, no zeros)".to_string(),
        )
    } else if sd > 0.0 {
        let cc = if opts.continuity { 0.5 } else { 0.0 };
        let pl = std_normal_cdf((wp - mean + cc) / sd);
        let pg = std_normal_sf((wp - mean - cc) / sd);
        let p2 = (2.0 * std_normal_sf(((wp - mean).abs() - cc) / sd)).min(1.0);
        let why = if has_ties || n_zero > 0 {
            "asymptotic, tie/zero-adjusted (Prism would enumerate the exact distribution)"
        } else {
            "asymptotic"
        };
        let zm = match opts.zero_method {
            ZeroMethod::Wilcoxon => "zeros dropped",
            ZeroMethod::Pratt => "Pratt zeros",
        };
        (
            pl,
            pg,
            p2,
            format!(
                "{why}, {zm}{}",
                if opts.continuity {
                    ", continuity-corrected"
                } else {
                    ""
                }
            ),
        )
    } else {
        (1.0, 1.0, 1.0, "degenerate".to_string())
    };
    let p = match opts.alternative {
        Alternative::TwoSided => p_two,
        Alternative::Less => p_less,
        Alternative::Greater => p_greater,
    };

    let w_all = walsh(&all);
    let hl = median_sorted(&w_all);
    let hl_nz = median_sorted(&walsh(&nonzero));
    let m = w_all.len();
    let alpha = 1.0 - opts.ci_level;
    let n_all = all.len();
    let (ci, achieved) = match dist.as_ref() {
        Some(dd) if n_all == nonzero.len() => {
            let mut cum = 0.0;
            let mut k = 0usize;
            for (w, pw) in dd.iter().enumerate() {
                if cum + pw > alpha / 2.0 + 1e-12 {
                    k = w;
                    break;
                }
                cum += pw;
            }
            if k >= 1 {
                (Some((w_all[k - 1], w_all[m - k])), Some(1.0 - 2.0 * cum))
            } else {
                (None, None)
            }
        }
        _ => {
            let na = n_all as f64;
            let sd_all = (na * (na + 1.0) * (2.0 * na + 1.0) / 24.0).sqrt();
            let zc = special::norm_quantile_upper(alpha / 2.0);
            let kf = (na * (na + 1.0) / 4.0 - zc * sd_all).round();
            if kf >= 1.0 && (kf as usize) <= m {
                let k = kf as usize;
                (Some((w_all[k - 1], w_all[m - k])), Some(opts.ci_level))
            } else {
                (None, None)
            }
        }
    };

    Ok(WilcoxonResult {
        method,
        exact: use_exact,
        w_plus: wp,
        w_minus: wm,
        n: all.len(),
        n_zero,
        has_ties,
        z,
        p,
        p_two_sided: p_two,
        p_less,
        p_greater,
        hodges_lehmann: hl,
        hodges_lehmann_nonzero: hl_nz,
        ci,
        ci_level: opts.ci_level,
        ci_achieved_level: achieved,
    })
}
