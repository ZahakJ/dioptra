//! Correlation: Pearson r (t test, Fisher-z CI), Spearman rho (midranks; t approximation
//! and exact permutation p for small n), Kendall tau-b and tau-c (Knight's O(n log n)
//! algorithm; exact p without ties from the inversion distribution, asymptotic p with the
//! tie-corrected variance), partial correlation, and a correlation matrix with
//! multiplicity-adjusted p-values.
//!
//! Missing values: pairs with a `NaN` on either side are dropped (pairwise deletion in
//! the matrix).

use crate::descriptive::{compensated_sum, mean_of};
use crate::dist::{p_two_sided_z, std_normal_cdf, std_normal_sf, StudentT};
use crate::posthoc::{p_adjust, Adjust};
use crate::rank::rank_with_ties;
use crate::special;
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

/// Complete pairs of (x, y).
pub(crate) fn pairs(x: &[f64], y: &[f64], what: &str) -> Result<(Vec<f64>, Vec<f64>)> {
    if x.len() != y.len() {
        return Err(invalid(format!("{what}: x and y have different lengths")));
    }
    let mut a = Vec::with_capacity(x.len());
    let mut b = Vec::with_capacity(x.len());
    for (&u, &v) in x.iter().zip(y) {
        if u.is_nan() || v.is_nan() {
            continue;
        }
        if u.is_infinite() || v.is_infinite() {
            return Err(invalid(format!("{what}: infinite value")));
        }
        a.push(u);
        b.push(v);
    }
    Ok((a, b))
}

/// Pearson r of complete data by centred two-pass sums: (r, Sxx, Syy, Sxy).
fn pearson_r(x: &[f64], y: &[f64]) -> (f64, f64, f64, f64) {
    let (mx, my) = (mean_of(x), mean_of(y));
    let sxx = compensated_sum(x.iter().map(|a| (a - mx) * (a - mx)));
    let syy = compensated_sum(y.iter().map(|b| (b - my) * (b - my)));
    let sxy = compensated_sum(x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)));
    let r = (sxy / (sxx.sqrt() * syy.sqrt())).clamp(-1.0, 1.0);
    (r, sxx, syy, sxy)
}

/// A correlation coefficient with its tests.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Correlation {
    /// Method name.
    pub method: String,
    /// Number of complete pairs.
    pub n: usize,
    /// The coefficient (r, rho or tau-b).
    pub estimate: f64,
    /// Test statistic (t for Pearson and Spearman, z for Kendall).
    pub statistic: f64,
    /// Two-sided p (the method's default p: see each function).
    pub p_two_sided: f64,
    /// One-sided p for H1: positive association.
    pub p_greater: f64,
    /// One-sided p for H1: negative association.
    pub p_less: f64,
    /// Confidence interval of the coefficient, where defined.
    pub ci: Option<(f64, f64)>,
    /// r^2 (Pearson) or rho^2.
    pub r_squared: f64,
}

/// Pearson product-moment correlation (Pearson 1896) with t = r sqrt((n-2)/(1-r^2)) on
/// n - 2 df and the Fisher (1921) z interval tanh(atanh r +/- z_{1-a/2}/sqrt(n-3))
/// (n >= 4; Prism's CI). SciPy `pearsonr`.
pub fn pearson(x: &[f64], y: &[f64], ci_level: f64) -> Result<Correlation> {
    let (x, y) = pairs(x, y, "Pearson")?;
    let n = x.len();
    if n < 3 {
        return Err(StatsError::NotEnoughData(
            "Pearson correlation needs at least 3 pairs".into(),
        ));
    }
    if !(ci_level > 0.0 && ci_level < 1.0) {
        return Err(invalid("ci_level not in (0, 1)"));
    }
    let (r, sxx, syy, _) = pearson_r(&x, &y);
    if !(sxx > 0.0 && syy > 0.0) {
        return Err(invalid("Pearson: x or y is constant"));
    }
    let df = (n - 2) as f64;
    let (t, p2, pl, pg) = t_test_of_r(r, df);
    let ci = if n > 3 {
        let z = special::norm_quantile_upper(0.5 * (1.0 - ci_level));
        let zr = r.atanh();
        let h = z / ((n - 3) as f64).sqrt();
        Some(((zr - h).tanh(), (zr + h).tanh()))
    } else {
        None
    };
    Ok(Correlation {
        method: "Pearson".into(),
        n,
        estimate: r,
        statistic: t,
        p_two_sided: p2,
        p_greater: pg,
        p_less: pl,
        ci,
        r_squared: r * r,
    })
}

/// (t, p two-sided, p less, p greater) of r on df.
fn t_test_of_r(r: f64, df: f64) -> (f64, f64, f64, f64) {
    if r.abs() >= 1.0 {
        let s = r.signum();
        return (
            s * f64::INFINITY,
            0.0,
            if s > 0.0 { 1.0 } else { 0.0 },
            if s > 0.0 { 0.0 } else { 1.0 },
        );
    }
    let t = r * (df / ((1.0 - r) * (1.0 + r))).sqrt();
    let d = StudentT { df };
    let (cdf, sf) = d.cdf_sf(t);
    (t, d.two_sided_p(t), cdf, sf)
}

/// Spearman rank correlation with exact and approximate p-values.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Spearman {
    /// rho, the Pearson r of the midranks.
    pub rho: f64,
    /// Number of complete pairs.
    pub n: usize,
    /// t = rho sqrt((n-2)/(1-rho^2)).
    pub t: f64,
    /// t-approximation two-sided p on n - 2 df (SciPy `spearmanr`).
    pub p_t_two_sided: f64,
    /// t-approximation one-sided p, H1: rho > 0.
    pub p_t_greater: f64,
    /// t-approximation one-sided p, H1: rho < 0.
    pub p_t_less: f64,
    /// Exact permutation p (all n! orderings, ties kept as midranks), n <= 9.
    pub p_exact_two_sided: Option<f64>,
    /// Exact P(rho >= rho_obs).
    pub p_exact_greater: Option<f64>,
    /// Exact P(rho <= rho_obs).
    pub p_exact_less: Option<f64>,
    /// Number of permutations enumerated.
    pub n_permutations: Option<f64>,
    /// Fisher-z CI of rho with the Bonett–Wright (2000) variance (1 + rho^2/2)/(n - 3).
    pub ci: Option<(f64, f64)>,
}

/// Largest n for which [`spearman`] enumerates all n! permutations.
pub const SPEARMAN_EXACT_MAX_N: usize = 9;

/// Spearman's rho (Spearman 1904) as Pearson's r on midranks, which is correct with ties
/// (the 1 - 6 sum d^2/(n(n^2-1)) shortcut is not). p by the t approximation (SciPy
/// `spearmanr`) and, for n <= 9, exactly by enumerating all n! permutations of the y ranks
/// (ties kept), counting |rho| >= |rho_obs| (two-sided) and each tail. CI: Fisher z with
/// the Bonett & Wright (2000, Psychometrika 65:23) variance.
pub fn spearman(x: &[f64], y: &[f64], ci_level: f64) -> Result<Spearman> {
    let (x, y) = pairs(x, y, "Spearman")?;
    let n = x.len();
    if n < 3 {
        return Err(StatsError::NotEnoughData(
            "Spearman correlation needs at least 3 pairs".into(),
        ));
    }
    if !(ci_level > 0.0 && ci_level < 1.0) {
        return Err(invalid("ci_level not in (0, 1)"));
    }
    let (rx, _) = rank_with_ties(&x);
    let (ry, _) = rank_with_ties(&y);
    let (rho, sxx, syy, _) = pearson_r(&rx, &ry);
    if !(sxx > 0.0 && syy > 0.0) {
        return Err(invalid("Spearman: x or y is constant"));
    }
    let (t, p2, pl, pg) = t_test_of_r(rho, (n - 2) as f64);
    let (mut pe2, mut peg, mut pel, mut nperm) = (None, None, None, None);
    if n <= SPEARMAN_EXACT_MAX_N {
        let (a, b, c, total) = spearman_exact(&rx, &ry);
        pe2 = Some(a);
        peg = Some(b);
        pel = Some(c);
        nperm = Some(total);
    }
    let ci = if n > 3 && rho.abs() < 1.0 {
        let z = special::norm_quantile_upper(0.5 * (1.0 - ci_level));
        let h = z * ((1.0 + rho * rho / 2.0) / (n - 3) as f64).sqrt();
        let zr = rho.atanh();
        Some(((zr - h).tanh(), (zr + h).tanh()))
    } else {
        None
    };
    Ok(Spearman {
        rho,
        n,
        t,
        p_t_two_sided: p2,
        p_t_greater: pg,
        p_t_less: pl,
        p_exact_two_sided: pe2,
        p_exact_greater: peg,
        p_exact_less: pel,
        n_permutations: nperm,
        ci,
    })
}

/// Exact permutation tails of S = sum (rx_i - m)(ry_pi(i) - m), which is proportional to
/// rho for fixed rank sets. Heap's algorithm over all n! permutations.
fn spearman_exact(rx: &[f64], ry: &[f64]) -> (f64, f64, f64, f64) {
    let n = rx.len();
    let m = (n as f64 + 1.0) / 2.0;
    // Midranks are multiples of 1/2, so centred products are multiples of 1/4: exact.
    let a: Vec<f64> = rx.iter().map(|r| r - m).collect();
    let mut b: Vec<f64> = ry.iter().map(|r| r - m).collect();
    let s_obs: f64 = a.iter().zip(&b).map(|(u, v)| u * v).sum();
    let eps = 1e-9;
    let (mut ge, mut le, mut two, mut total) = (0u64, 0u64, 0u64, 0u64);
    let mut count = |b: &[f64]| {
        let s: f64 = a.iter().zip(b).map(|(u, v)| u * v).sum();
        total += 1;
        if s >= s_obs - eps {
            ge += 1;
        }
        if s <= s_obs + eps {
            le += 1;
        }
        if s.abs() >= s_obs.abs() - eps {
            two += 1;
        }
    };
    // Heap's algorithm (iterative).
    let mut c = vec![0usize; n];
    count(&b);
    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                b.swap(0, i);
            } else {
                b.swap(c[i], i);
            }
            count(&b);
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    let t = total as f64;
    (two as f64 / t, ge as f64 / t, le as f64 / t, t)
}

/// Kendall rank correlation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kendall {
    /// tau-b = (C - D)/sqrt((n0 - n1)(n0 - n2)).
    pub tau_b: f64,
    /// Stuart's tau-c = 2 (C - D) / (n^2 (m - 1)/m), m = min(#distinct x, #distinct y).
    pub tau_c: f64,
    /// Number of complete pairs.
    pub n: usize,
    /// C - D.
    pub s: f64,
    /// z = (C - D)/sqrt(Var(S)) with the tie-corrected variance.
    pub z: f64,
    /// Asymptotic two-sided p.
    pub p_asymptotic: f64,
    /// Asymptotic one-sided p, H1: positive association.
    pub p_asymptotic_greater: f64,
    /// Asymptotic one-sided p, H1: negative association.
    pub p_asymptotic_less: f64,
    /// Exact two-sided p (no ties only; any n up to [`KENDALL_EXACT_MAX_N`]).
    pub p_exact: Option<f64>,
    /// SciPy's `method='auto'` choice: exact if there are no ties and (n <= 33 or
    /// min(D, n0 - D) <= 1), else asymptotic.
    pub p_auto: f64,
    /// Whether x or y has ties.
    pub has_ties: bool,
}

/// Largest n for the exact Kendall distribution.
pub const KENDALL_EXACT_MAX_N: usize = 170;

/// Kendall's tau-b (Kendall 1945, Biometrika 33:239) by Knight's O(n log n) merge-sort
/// algorithm (Knight 1966, JASA 61:436, doi:10.1080/01621459.1966.10480879), the
/// tie-corrected variance of C - D (Kendall 1970, ch. 4; as SciPy `kendalltau`), and the
/// exact null distribution of the number of discordant pairs without ties (the inversion
/// numbers, recurrence N(n, k) = sum_{j=0}^{min(k, n-1)} N(n-1, k-j), in probabilities):
/// two-sided p = 2 P(D <= min(D, n0 - D)), capped at 1.
pub fn kendall(x: &[f64], y: &[f64]) -> Result<Kendall> {
    let (x, y) = pairs(x, y, "Kendall")?;
    let n = x.len();
    if n < 2 {
        return Err(StatsError::NotEnoughData(
            "Kendall correlation needs at least 2 pairs".into(),
        ));
    }
    // Sort by (x, y).
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| x[a].total_cmp(&x[b]).then(y[a].total_cmp(&y[b])));
    let xs: Vec<f64> = idx.iter().map(|&i| x[i]).collect();
    let mut ys: Vec<f64> = idx.iter().map(|&i| y[i]).collect();
    // Tie counts in x and joint ties.
    let tie_stats = |v: &[f64]| -> (f64, f64, f64, usize) {
        // returns (sum t(t-1)/2, sum t(t-1)(t-2), sum t(t-1)(2t+5), distinct)
        let mut w = v.to_vec();
        w.sort_by(|a, b| a.total_cmp(b));
        let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
        let mut distinct = 0;
        let mut i = 0;
        while i < w.len() {
            let mut j = i + 1;
            while j < w.len() && w[j] == w[i] {
                j += 1;
            }
            let t = (j - i) as f64;
            a += t * (t - 1.0) / 2.0;
            b += t * (t - 1.0) * (t - 2.0);
            c += t * (t - 1.0) * (2.0 * t + 5.0);
            distinct += 1;
            i = j;
        }
        (a, b, c, distinct)
    };
    let (xtie, x0, x1, dx) = tie_stats(&xs);
    let (ytie, y0, y1, dy) = tie_stats(&ys);
    // joint ties: runs equal in both x and y (after sorting by (x, y)).
    let mut ntie = 0.0;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && xs[j] == xs[i] && ys[j] == ys[i] {
            j += 1;
        }
        let t = (j - i) as f64;
        ntie += t * (t - 1.0) / 2.0;
        i = j;
    }
    // Discordant pairs = inversions of y (strict) after sorting by (x, y).
    let swaps = merge_count(&mut ys);
    let n0 = n as f64 * (n as f64 - 1.0) / 2.0;
    // tot = n0; C - D = n0 - xtie - ytie + ntie - 2 swaps
    let s = n0 - xtie - ytie + ntie - 2.0 * swaps;
    let denom = ((n0 - xtie) * (n0 - ytie)).sqrt();
    if !(denom > 0.0) {
        return Err(invalid("Kendall: x or y is constant"));
    }
    let tau_b = s / denom;
    let m = dx.min(dy) as f64;
    let nf = n as f64;
    let tau_c = 2.0 * s / (nf * nf * (m - 1.0) / m);
    let mm = nf * (nf - 1.0);
    let var = (mm * (2.0 * nf + 5.0) - x1 - y1) / 18.0
        + 2.0 * xtie * ytie / mm
        + if n > 2 {
            x0 * y0 / (9.0 * mm * (nf - 2.0))
        } else {
            0.0
        };
    let z = s / var.sqrt();
    let has_ties = xtie > 0.0 || ytie > 0.0;
    let p_exact = if !has_ties && n <= KENDALL_EXACT_MAX_N {
        let d = swaps.round() as usize;
        Some(kendall_exact_two_sided(n, d))
    } else {
        None
    };
    let dis = swaps;
    let use_exact = !has_ties && (n <= 33 || dis.min(n0 - dis) <= 1.0);
    let p_asym = p_two_sided_z(z);
    Ok(Kendall {
        tau_b,
        tau_c,
        n,
        s,
        z,
        p_asymptotic: p_asym,
        p_asymptotic_greater: std_normal_sf(z),
        p_asymptotic_less: std_normal_cdf(z),
        p_auto: if use_exact {
            p_exact.unwrap_or(p_asym)
        } else {
            p_asym
        },
        p_exact,
        has_ties,
    })
}

/// Sort `v` ascending and return the number of strict inversions (pairs i < j with
/// v_i > v_j).
fn merge_count(v: &mut [f64]) -> f64 {
    let n = v.len();
    if n < 2 {
        return 0.0;
    }
    let mut buf = v.to_vec();
    let mut inv = 0.0;
    let mut width = 1;
    while width < n {
        let mut lo = 0;
        while lo < n {
            let mid = (lo + width).min(n);
            let hi = (lo + 2 * width).min(n);
            let (mut i, mut j, mut k) = (lo, mid, lo);
            while i < mid && j < hi {
                if v[j] < v[i] {
                    buf[k] = v[j];
                    inv += (mid - i) as f64;
                    j += 1;
                } else {
                    buf[k] = v[i];
                    i += 1;
                }
                k += 1;
            }
            while i < mid {
                buf[k] = v[i];
                i += 1;
                k += 1;
            }
            while j < hi {
                buf[k] = v[j];
                j += 1;
                k += 1;
            }
            lo = hi;
        }
        v.copy_from_slice(&buf);
        width *= 2;
    }
    inv
}

/// Exact two-sided p of Kendall's statistic without ties: 2 P(D <= min(d, n0 - d)), where
/// D (the number of inversions of a uniform random permutation) has the Mahonian
/// distribution, built by convolving uniform{0..j-1} for j = 2..n in probabilities.
pub fn kendall_exact_two_sided(n: usize, d: usize) -> f64 {
    if n < 2 {
        return 1.0;
    }
    let n0 = n * (n - 1) / 2;
    let c = d.min(n0.saturating_sub(d));
    if 2 * c == n0 {
        return 1.0;
    }
    // probabilities of k inversions, k = 0..=c (only the lower tail is needed).
    let mut p = vec![0.0; c + 1];
    p[0] = 1.0;
    for j in 2..=n {
        // new[k] = (1/j) sum_{i=0}^{min(k, j-1)} old[k-i]
        let mut prefix = vec![0.0; c + 2];
        for k in 0..=c {
            prefix[k + 1] = prefix[k] + p[k];
        }
        for k in 0..=c {
            let lo = k.saturating_sub(j - 1);
            p[k] = (prefix[k + 1] - prefix[lo]) / j as f64;
        }
    }
    (2.0 * compensated_sum(p.iter().copied())).min(1.0)
}

/// Correlation method for [`correlation_matrix`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CorrelationMethod {
    /// Pearson r.
    Pearson,
    /// Spearman rho (t-approximation p).
    Spearman,
    /// Kendall tau-b (SciPy `auto` p).
    Kendall,
}

/// A correlation matrix with p-values.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CorrelationMatrix {
    /// Coefficients (1 on the diagonal).
    pub r: Vec<Vec<f64>>,
    /// Two-sided p-values (NaN on the diagonal).
    pub p: Vec<Vec<f64>>,
    /// Adjusted p-values over the k(k-1)/2 distinct pairs.
    pub p_adjusted: Vec<Vec<f64>>,
    /// Complete pairs used for each entry.
    pub n: Vec<Vec<usize>>,
    /// Adjustment used.
    pub adjust: Adjust,
}

/// All pairwise correlations of the columns (pairwise deletion of missing values), with
/// two-sided p-values and their multiplicity adjustment over the k(k-1)/2 pairs
/// ([`crate::posthoc::p_adjust`]). Entries that cannot be computed (too few pairs, a
/// constant column) are NaN.
#[allow(clippy::needless_range_loop)]
pub fn correlation_matrix(
    columns: &[&[f64]],
    method: CorrelationMethod,
    adjust: Adjust,
) -> Result<CorrelationMatrix> {
    let k = columns.len();
    if k < 2 {
        return Err(StatsError::NotEnoughData(
            "correlation matrix needs at least 2 columns".into(),
        ));
    }
    let mut r = vec![vec![f64::NAN; k]; k];
    let mut p = vec![vec![f64::NAN; k]; k];
    let mut nn = vec![vec![0usize; k]; k];
    let mut flat = Vec::new();
    for i in 0..k {
        r[i][i] = 1.0;
        nn[i][i] = columns[i].iter().filter(|v| !v.is_nan()).count();
        for j in (i + 1)..k {
            let (est, pv, n) =
                match method {
                    CorrelationMethod::Pearson => pearson(columns[i], columns[j], 0.95)
                        .map(|c| (c.estimate, c.p_two_sided, c.n)),
                    CorrelationMethod::Spearman => spearman(columns[i], columns[j], 0.95)
                        .map(|c| (c.rho, c.p_t_two_sided, c.n)),
                    CorrelationMethod::Kendall => {
                        kendall(columns[i], columns[j]).map(|c| (c.tau_b, c.p_auto, c.n))
                    }
                }
                .unwrap_or((f64::NAN, f64::NAN, 0));
            r[i][j] = est;
            r[j][i] = est;
            p[i][j] = pv;
            p[j][i] = pv;
            nn[i][j] = n;
            nn[j][i] = n;
            flat.push(pv);
        }
    }
    let adj = p_adjust(&flat, adjust);
    let mut pa = vec![vec![f64::NAN; k]; k];
    let mut it = adj.into_iter();
    for i in 0..k {
        for j in (i + 1)..k {
            let v = it.next().unwrap_or(f64::NAN);
            pa[i][j] = v;
            pa[j][i] = v;
        }
    }
    Ok(CorrelationMatrix {
        r,
        p,
        p_adjusted: pa,
        n: nn,
        adjust,
    })
}

/// Partial correlation of x and y controlling for the columns of `z`: the Pearson
/// correlation of the residuals of x and y regressed (with intercept) on z, tested with
/// t on n - 2 - q df (Fisher 1924). Rows with a missing value anywhere are dropped.
pub fn partial_correlation(
    x: &[f64],
    y: &[f64],
    z: &[&[f64]],
    ci_level: f64,
) -> Result<Correlation> {
    let n0 = x.len();
    if y.len() != n0 || z.iter().any(|c| c.len() != n0) {
        return Err(invalid("partial correlation: lengths differ"));
    }
    let keep: Vec<usize> = (0..n0)
        .filter(|&i| !x[i].is_nan() && !y[i].is_nan() && z.iter().all(|c| !c[i].is_nan()))
        .collect();
    let q = z.len();
    let n = keep.len();
    if n < q + 4 {
        return Err(StatsError::NotEnoughData(
            "partial correlation needs n >= q + 4".into(),
        ));
    }
    let mut cols = vec![vec![1.0; n]];
    for c in z {
        cols.push(keep.iter().map(|&i| c[i]).collect());
    }
    let d = crate::linalg::Design::from_columns(&cols);
    let (kept, _) = crate::linalg::rank_select(&d);
    let d = d.select(&kept);
    let ls = crate::linalg::LeastSquares::new(&d);
    let xv: Vec<f64> = keep.iter().map(|&i| x[i]).collect();
    let yv: Vec<f64> = keep.iter().map(|&i| y[i]).collect();
    let ex = ls.fit(&xv).resid;
    let ey = ls.fit(&yv).resid;
    let (r, sxx, syy, _) = pearson_r(&ex, &ey);
    if !(sxx > 0.0 && syy > 0.0) {
        return Err(invalid("partial correlation: a residual vector is zero"));
    }
    let df = (n - 2 - (kept.len() - 1)) as f64;
    let (t, p2, pl, pg) = t_test_of_r(r, df);
    let zq = special::norm_quantile_upper(0.5 * (1.0 - ci_level));
    let h = zq / (df - 1.0).sqrt();
    Ok(Correlation {
        method: "partial Pearson".into(),
        n,
        estimate: r,
        statistic: t,
        p_two_sided: p2,
        p_greater: pg,
        p_less: pl,
        ci: Some(((r.atanh() - h).tanh(), (r.atanh() + h).tanh())),
        r_squared: r * r,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kendall_exact_small() {
        // n = 4: inversion counts 0..6 with frequencies 1,3,5,6,5,3,1 (24 total).
        assert!((kendall_exact_two_sided(4, 0) - 2.0 / 24.0).abs() < 1e-15);
        assert!((kendall_exact_two_sided(4, 1) - 8.0 / 24.0).abs() < 1e-15);
        assert!((kendall_exact_two_sided(4, 5) - 8.0 / 24.0).abs() < 1e-15);
        assert_eq!(kendall_exact_two_sided(4, 3), 1.0);
    }

    #[test]
    fn knight_matches_quadratic() {
        let x = [1.0, 2.0, 2.0, 3.0, 4.0, 4.0, 5.0, 6.0];
        let y = [2.0, 1.0, 3.0, 3.0, 5.0, 4.0, 4.0, 7.0];
        let k = kendall(&x, &y).unwrap();
        let mut s = 0.0;
        for i in 0..x.len() {
            for j in (i + 1)..x.len() {
                let prod = (x[i] - x[j]) * (y[i] - y[j]);
                if prod != 0.0 {
                    s += prod.signum();
                }
            }
        }
        assert!((k.s - s).abs() < 1e-12, "{} vs {s}", k.s);
    }

    #[test]
    fn partial_removes_confounder() {
        let z: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let x: Vec<f64> = z.iter().map(|v| 2.0 * v + (v * 1.3).sin()).collect();
        let y: Vec<f64> = z.iter().map(|v| -v + (v * 0.7).cos()).collect();
        let r = partial_correlation(&x, &y, &[&z], 0.95).unwrap();
        assert!(r.estimate.abs() < 0.9);
        assert_eq!(r.n, 20);
    }
}
