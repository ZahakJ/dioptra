//! Outlier identification in one column: iterative Grubbs and ROUT (Q).
//!
//! Indices always refer to positions in the caller's slice (missing values keep their
//! positions and are never flagged).

use crate::descriptive::{compensated_sum, quantile_sorted, QuantileMethod};
use crate::dist::StudentT;
use crate::{Result, StatsError};

/// One step of the iterative Grubbs test.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GrubbsStep {
    /// Number of values tested at this step.
    pub n: usize,
    /// Mean of those values.
    pub mean: f64,
    /// SD (n - 1) of those values, the suspect included.
    pub sd: f64,
    /// Index (in the input) of the value farthest from the mean.
    pub suspect_index: usize,
    /// Its value.
    pub suspect_value: f64,
    /// G = |x - mean| / sd.
    pub g: f64,
    /// Two-sided critical value at `alpha`.
    pub g_critical: f64,
    /// Approximate p-value (Bonferroni bound, accurate when small).
    pub p: f64,
    /// G > G_critical.
    pub significant: bool,
}

/// Result of [`grubbs`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GrubbsResult {
    /// Every step taken, the last one non-significant unless the data ran out.
    pub steps: Vec<GrubbsStep>,
    /// Indices removed as outliers, in removal order.
    pub outlier_indices: Vec<usize>,
    /// Significance level used.
    pub alpha: f64,
}

/// Iterative two-sided Grubbs test (Grubbs 1969, Technometrics 11:1; NIST/SEMATECH
/// e-Handbook §1.3.5.17, the source GraphPad's QuickCalc follows). At each step
/// G = max |x_i - mean|/s with s computed from all remaining values,
/// G_crit = ((n-1)/sqrt(n)) sqrt(t^2/(n-2+t^2)) with t = t_{1-alpha/(2n), n-2}, and
/// p = min(1, 2n P(T_{n-2} > t_obs)) with t_obs = sqrt(n(n-2)G^2/((n-1)^2 - nG^2)).
/// The suspect is removed and the test repeated while G > G_crit (and n >= 3).
pub fn grubbs(data: &[f64], alpha: f64) -> Result<GrubbsResult> {
    if !(alpha > 0.0 && alpha < 1.0) {
        return Err(StatsError::InvalidArgument(
            "alpha must be in (0, 1)".into(),
        ));
    }
    let mut vals = Vec::new();
    let mut idx = Vec::new();
    for (i, &x) in data.iter().enumerate() {
        if x.is_nan() {
            continue;
        }
        if x.is_infinite() {
            return Err(StatsError::InvalidArgument("infinite value in data".into()));
        }
        vals.push(x);
        idx.push(i);
    }
    if vals.len() < 3 {
        return Err(StatsError::NotEnoughData(
            "Grubbs' test needs at least 3 values".into(),
        ));
    }
    let mut steps = Vec::new();
    while vals.len() >= 3 {
        let n = vals.len();
        let nf = n as f64;
        let m0 = compensated_sum(vals.iter().copied()) / nf;
        let m = m0 + compensated_sum(vals.iter().map(|x| x - m0)) / nf;
        let s1 = compensated_sum(vals.iter().map(|x| x - m));
        let ss = compensated_sum(vals.iter().map(|x| (x - m) * (x - m))) - s1 * s1 / nf;
        let s = (ss.max(0.0) / (nf - 1.0)).sqrt();
        if !(s > 0.0) {
            break;
        }
        let mut k = 0;
        let mut dmax = -1.0;
        for (i, &v) in vals.iter().enumerate() {
            let d = (v - m).abs();
            if d > dmax {
                dmax = d;
                k = i;
            }
        }
        let g = dmax / s;
        let td = StudentT::new(nf - 2.0)?;
        let q = alpha / (2.0 * nf);
        let tc = td.quantile_pq(1.0 - q, q);
        let gcrit = (nf - 1.0) / nf.sqrt() * (tc * tc / (nf - 2.0 + tc * tc)).sqrt();
        let den = (nf - 1.0) * (nf - 1.0) - nf * g * g;
        let p = if den > 0.0 {
            let tobs = (nf * (nf - 2.0) * g * g / den).sqrt();
            (2.0 * nf * td.cdf_sf(tobs).1).min(1.0)
        } else {
            0.0
        };
        let significant = g > gcrit;
        steps.push(GrubbsStep {
            n,
            mean: m,
            sd: s,
            suspect_index: idx[k],
            suspect_value: vals[k],
            g,
            g_critical: gcrit,
            p,
            significant,
        });
        if !significant {
            break;
        }
        vals.remove(k);
        idx.remove(k);
    }
    let outlier_indices = steps
        .iter()
        .filter(|s| s.significant)
        .map(|s| s.suspect_index)
        .collect();
    Ok(GrubbsResult {
        steps,
        outlier_indices,
        alpha,
    })
}

/// Result of [`rout_column`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoutResult {
    /// Robust centre: minimiser of the Lorentzian merit with adaptive RSDR.
    pub robust_center: f64,
    /// 68.27th percentile of |residuals| about the robust centre.
    pub p68_abs_residual: f64,
    /// RSDR = P68 N/(N-K), K = 1.
    pub rsdr: f64,
    /// 1.4826 MAD N/(N-K), for comparison only (not used for flagging).
    pub rsdr_alt_mad: f64,
    /// Indices of the outliers (ascending).
    pub outlier_indices: Vec<usize>,
    /// Mean of the values kept.
    pub mean_after_removal: f64,
    /// SD (n - 1) of the values kept (NaN if fewer than 2 remain).
    pub sd_after_removal: f64,
    /// Q (a fraction: 0.01 = Prism's default 1%).
    pub q: f64,
}

/// ROUT's false-discovery-rate test (Motulsky & Brown 2006, eqs. 17-18): rank |r|
/// ascending; for i = floor(0.7 N)+1..N, alpha_i = Q (N - (i-1))/N and p_i the two-tailed
/// t probability of |r_i|/RSDR on N-K df; the first i with p_i < alpha_i flags point i and
/// every point farther out. Returns the flagged positions in `resid`, ascending.
pub fn rout_fdr_flags(resid: &[f64], rsdr: f64, k: f64, q: f64) -> Result<Vec<usize>> {
    let nn = resid.len();
    let nf = nn as f64;
    let mut order: Vec<usize> = (0..nn).collect();
    order.sort_by(|&a, &b| resid[a].abs().total_cmp(&resid[b].abs()));
    let mut flagged: Vec<usize> = Vec::new();
    if rsdr > 0.0 && nf - k > 0.0 {
        let td = StudentT::new(nf - k)?;
        let start = (0.7 * nf).floor() as usize + 1;
        for i in start..=nn {
            let j = order[i - 1];
            let t = resid[j].abs() / rsdr;
            let p = td.two_sided_p(t);
            let alpha_i = q * (nf - (i as f64 - 1.0)) / nf;
            if p < alpha_i {
                flagged = order[i - 1..].to_vec();
                break;
            }
        }
    }
    flagged.sort_unstable();
    Ok(flagged)
}

/// RSDR (Prism): the 68.27th percentile of |residuals| (linear interpolation, Hyndman-Fan
/// type 7) times N/(N-K).
pub fn rsdr(resid: &[f64], k: f64) -> f64 {
    let mut a: Vec<f64> = resid
        .iter()
        .map(|x| x.abs())
        .filter(|x| !x.is_nan())
        .collect();
    if a.is_empty() {
        return f64::NAN;
    }
    a.sort_by(f64::total_cmp);
    let nf = a.len() as f64;
    quantile_sorted(&a, 68.27 / 100.0, QuantileMethod::Linear).unwrap_or(f64::NAN) * nf / (nf - k)
}

/// Lorentzian merit minimiser for fixed scale, by iteratively reweighted least squares
/// (a majorise–minimise iteration that decreases sum ln(1 + (r/s)^2) monotonically).
fn lorentzian_center(v: &[f64], start: f64, scale: f64) -> f64 {
    let mut m = start;
    let s2 = scale * scale;
    for _ in 0..100_000 {
        let mut sw = 0.0;
        let mut swx = 0.0;
        for &x in v {
            let r = x - m;
            let w = 1.0 / (1.0 + r * r / s2);
            sw += w;
            swx += w * r;
        }
        let step = swx / sw;
        let mn = m + step;
        if (mn - m).abs() <= 1e-15 * mn.abs().max(1e-300) || step == 0.0 {
            return mn;
        }
        m = mn;
    }
    m
}

/// ROUT outlier identification for a column ("fit a constant"), Motulsky & Brown (2006),
/// BMC Bioinformatics 7:123, with the RSDR of the Prism guide:
///
/// 1. Robust fit: minimise sum ln(1 + (r_i/RSDR)^2) over the centre M (K = 1), RSDR
///    recomputed from the residuals at each outer iteration (start: the median), until M
///    changes by < 1e-13 relative.
/// 2. RSDR = P68 N/(N-K), P68 the 68.27th percentile of |residuals| (linear interpolation,
///    Hyndman–Fan type 7).
/// 3. FDR test (paper eqs. 17-18): rank |r| ascending; for i = floor(0.7 N)+1..N,
///    alpha_i = Q (N - (i-1))/N and p_i the two-tailed t probability of |r_i|/RSDR on N-K
///    df; the first i with p_i < alpha_i flags point i and all farther points.
/// 4. Mean and SD of the values kept.
pub fn rout_column(data: &[f64], q: f64) -> Result<RoutResult> {
    if !(q > 0.0 && q < 1.0) {
        return Err(StatsError::InvalidArgument("Q must be in (0, 1)".into()));
    }
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for (i, &x) in data.iter().enumerate() {
        if x.is_nan() {
            continue;
        }
        if x.is_infinite() {
            return Err(StatsError::InvalidArgument("infinite value in data".into()));
        }
        v.push(x);
        idx.push(i);
    }
    let nn = v.len();
    if nn < 3 {
        return Err(StatsError::NotEnoughData(
            "ROUT needs at least 3 values".into(),
        ));
    }
    let k = 1.0;
    let nf = nn as f64;
    let corr = nf / (nf - k);
    let p68 = |m: f64| -> f64 {
        let mut a: Vec<f64> = v.iter().map(|x| (x - m).abs()).collect();
        a.sort_by(f64::total_cmp);
        quantile_sorted(&a, 68.27 / 100.0, QuantileMethod::Linear).unwrap_or(f64::NAN)
    };
    let mut sorted = v.clone();
    sorted.sort_by(f64::total_cmp);
    let median = quantile_sorted(&sorted, 0.5, QuantileMethod::Linear)?;
    let mut m = median;
    for _ in 0..500 {
        let rsdr = p68(m) * corr;
        if !(rsdr > 0.0) {
            break;
        }
        let mn = lorentzian_center(&v, m, rsdr);
        if (mn - m).abs() <= 1e-13 * m.abs().max(1.0) {
            m = mn;
            break;
        }
        m = mn;
    }
    let p68v = p68(m);
    let rsdr = p68v * corr;
    let mad = {
        let mut a: Vec<f64> = v.iter().map(|x| (x - median).abs()).collect();
        a.sort_by(f64::total_cmp);
        quantile_sorted(&a, 0.5, QuantileMethod::Linear)?
    };
    let resid: Vec<f64> = v.iter().map(|x| x - m).collect();
    let flagged_local = rout_fdr_flags(&resid, rsdr, k, q)?;
    let kept: Vec<f64> = (0..nn)
        .filter(|i| !flagged_local.contains(i))
        .map(|i| v[i])
        .collect();
    let km = compensated_sum(kept.iter().copied()) / kept.len() as f64;
    let ksd = if kept.len() >= 2 {
        let s1 = compensated_sum(kept.iter().map(|x| x - km));
        let ss =
            compensated_sum(kept.iter().map(|x| (x - km) * (x - km))) - s1 * s1 / kept.len() as f64;
        (ss.max(0.0) / (kept.len() as f64 - 1.0)).sqrt()
    } else {
        f64::NAN
    };
    Ok(RoutResult {
        robust_center: m,
        p68_abs_residual: p68v,
        rsdr,
        rsdr_alt_mad: 1.4826 * mad * corr,
        outlier_indices: flagged_local.iter().map(|&i| idx[i]).collect(),
        mean_after_removal: km,
        sd_after_removal: ksd,
        q,
    })
}

/// ROUT for a fitted curve (Motulsky & Brown 2006): robust fit with the adaptive
/// Lorentzian merit, RSDR, the FDR test of [`rout_fdr_flags`] on N - K df, least-squares
/// refit without the outliers. See [`crate::nonlinear::rout_fit`].
pub fn rout_curve(
    eq: &crate::equations::Equation,
    data: &crate::nonlinear::Dataset,
    q: f64,
    opts: &crate::nonlinear::FitOptions,
) -> Result<crate::nonlinear::RoutFit> {
    crate::nonlinear::rout_fit(eq, data, q, opts)
}
