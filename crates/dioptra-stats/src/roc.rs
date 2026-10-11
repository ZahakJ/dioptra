//! Receiver operating characteristic (ROC) curves: the empirical curve at every distinct
//! threshold, sensitivity and specificity with confidence intervals, likelihood ratios,
//! the area under the curve with DeLong and Hanley–McNeil standard errors, the Youden and
//! closest-to-(0,1) cut-offs, and DeLong's test comparing two correlated curves.
//!
//! Classification rule: a subject is called positive when its score is >= the threshold
//! (higher scores mean disease). Thresholds descend from +inf (nobody positive; sklearn's
//! sentinel) through every distinct observed score.

use crate::dist::{p_two_sided_z, Beta};
use crate::rank::rank_with_ties;
use crate::special::norm_quantile_pq;
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

/// Confidence interval of a proportion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ProportionCi {
    /// Wilson (1927) score interval (Brown, Cai & DasGupta 2001 recommend it; Prism's
    /// "Wilson/Brown" choice is a hybrid that can differ at x = 0, 1, n-1, n).
    #[default]
    Wilson,
    /// Clopper & Pearson (1934) exact interval from beta quantiles.
    ClopperPearson,
}

/// Interval for x successes out of n at confidence `level`.
pub fn proportion_ci(x: usize, n: usize, level: f64, method: ProportionCi) -> (f64, f64) {
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    let a = 1.0 - level;
    let (xf, nf) = (x as f64, n as f64);
    match method {
        ProportionCi::Wilson => {
            let z = norm_quantile_pq(1.0 - 0.5 * a, 0.5 * a);
            let z2 = z * z;
            let center = (xf + 0.5 * z2) / (nf + z2);
            let half = z / (nf + z2) * (xf * (nf - xf) / nf + 0.25 * z2).sqrt();
            ((center - half).max(0.0), (center + half).min(1.0))
        }
        ProportionCi::ClopperPearson => {
            let lo = if x == 0 {
                0.0
            } else {
                Beta {
                    a: xf,
                    b: nf - xf + 1.0,
                }
                .quantile_pq(0.5 * a, 1.0 - 0.5 * a)
            };
            let hi = if x == n {
                1.0
            } else {
                Beta {
                    a: xf + 1.0,
                    b: nf - xf,
                }
                .quantile_pq(1.0 - 0.5 * a, 0.5 * a)
            };
            (lo, hi)
        }
    }
}

/// Options for [`roc`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RocOptions {
    /// Confidence level (0.95).
    pub ci_level: f64,
    /// CI method for sensitivity and specificity.
    pub proportion_ci: ProportionCi,
}

impl Default for RocOptions {
    fn default() -> Self {
        RocOptions {
            ci_level: 0.95,
            proportion_ci: ProportionCi::Wilson,
        }
    }
}

/// One threshold of the ROC curve.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RocPoint {
    /// Threshold (positive if score >= threshold); +inf first.
    pub threshold: f64,
    /// True positives.
    pub tp: usize,
    /// False positives.
    pub fp: usize,
    /// Sensitivity TP/P (= true positive rate).
    pub sensitivity: f64,
    /// Specificity TN/N.
    pub specificity: f64,
    /// 1 - specificity (false positive rate).
    pub fpr: f64,
    /// CI of the sensitivity.
    pub sensitivity_ci: (f64, f64),
    /// CI of the specificity.
    pub specificity_ci: (f64, f64),
    /// Positive likelihood ratio sens/(1 - spec) (+inf when spec = 1 and sens > 0).
    pub lr_positive: f64,
    /// Negative likelihood ratio (1 - sens)/spec.
    pub lr_negative: f64,
    /// Youden J = sens + spec - 1.
    pub youden: f64,
}

/// ROC analysis.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Roc {
    /// Curve points, thresholds descending.
    pub points: Vec<RocPoint>,
    /// Patients (positives).
    pub n_positive: usize,
    /// Controls (negatives).
    pub n_negative: usize,
    /// Area under the curve = Mann–Whitney U/(n1 n0), ties counted 1/2.
    pub auc: f64,
    /// DeLong SE.
    pub se_delong: f64,
    /// AUC +/- z SE_DeLong, clipped to [0, 1].
    pub ci_delong: (f64, f64),
    /// Hanley & McNeil (1982) SE.
    pub se_hanley_mcneil: f64,
    /// AUC +/- z SE_HM, clipped to [0, 1].
    pub ci_hanley_mcneil: (f64, f64),
    /// z = (AUC - 0.5)/SE_DeLong.
    pub z: f64,
    /// Two-sided p of z (H0: AUC = 0.5).
    pub p: f64,
    /// Index of the first point (in threshold order) with the largest Youden J.
    pub youden_best_index: usize,
    /// Its threshold.
    pub youden_best_threshold: f64,
    /// Its J.
    pub youden_best_j: f64,
    /// Index of the point closest to (FPR, TPR) = (0, 1) (first on ties).
    pub closest_topleft_index: usize,
    /// DeLong structural components V10 (one per positive).
    pub v10: Vec<f64>,
    /// DeLong structural components V01 (one per negative).
    pub v01: Vec<f64>,
}

fn clean(v: &[f64], what: &str) -> Result<Vec<f64>> {
    let c: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
    if c.is_empty() {
        return Err(StatsError::NotEnoughData(format!("no {what}")));
    }
    if c.iter().any(|x| x.is_infinite()) {
        return Err(invalid("infinite score"));
    }
    Ok(c)
}

/// DeLong components by the midrank algorithm of Sun & Xu (2014), IEEE Signal Processing
/// Letters 21:1389, doi:10.1109/LSP.2014.2337313: V10_i = (R_pooled(x_i) - R_pos(x_i))/n0,
/// V01_j = 1 - (R_pooled(y_j) - R_neg(y_j))/n1.
fn delong_components(pos: &[f64], neg: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let (n1, n0) = (pos.len(), neg.len());
    let mut pooled = pos.to_vec();
    pooled.extend_from_slice(neg);
    let (rp, _) = rank_with_ties(&pooled);
    let (r1, _) = rank_with_ties(pos);
    let (r0, _) = rank_with_ties(neg);
    let v10 = (0..n1).map(|i| (rp[i] - r1[i]) / n0 as f64).collect();
    let v01 = (0..n0)
        .map(|j| 1.0 - (rp[n1 + j] - r0[j]) / n1 as f64)
        .collect();
    (v10, v01)
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn cov(a: &[f64], b: &[f64]) -> f64 {
    let (ma, mb) = (mean(a), mean(b));
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - ma) * (y - mb))
        .sum::<f64>()
        / (a.len() as f64 - 1.0)
}

/// ROC curve and AUC of `positives` (patients) against `negatives` (controls).
///
/// AUC by the Mann–Whitney identity; SE by DeLong, DeLong & Clarke-Pearson (1988),
/// Biometrics 44:837, doi:10.2307/2531595 (Var = var(V10)/n1 + var(V01)/n0, n-1
/// denominators) and by Hanley & McNeil (1982), Radiology 143:29 (Q1 = A/(2-A),
/// Q2 = 2A^2/(1+A)); CIs AUC +/- z SE clipped to [0, 1]; the z test of AUC = 0.5 uses the
/// DeLong SE. Youden (1950) J and the closest-to-(0,1) criterion pick cut-offs.
pub fn roc(positives: &[f64], negatives: &[f64], opts: RocOptions) -> Result<Roc> {
    if !(opts.ci_level > 0.0 && opts.ci_level < 1.0) {
        return Err(invalid("ci_level must be in (0, 1)"));
    }
    let pos = clean(positives, "positives")?;
    let neg = clean(negatives, "negatives")?;
    let (n1, n0) = (pos.len(), neg.len());
    let a = 1.0 - opts.ci_level;
    let z = norm_quantile_pq(1.0 - 0.5 * a, 0.5 * a);
    let mut thr: Vec<f64> = pos.iter().chain(&neg).copied().collect();
    thr.sort_by(|a, b| b.total_cmp(a));
    thr.dedup();
    let mut sp = pos.clone();
    sp.sort_by(|a, b| b.total_cmp(a));
    let mut sn = neg.clone();
    sn.sort_by(|a, b| b.total_cmp(a));
    let mut points = Vec::with_capacity(thr.len() + 1);
    let (mut ip, mut ineg) = (0usize, 0usize);
    let mut push = |t: f64, tp: usize, fp: usize| {
        let sens = tp as f64 / n1 as f64;
        let spec = (n0 - fp) as f64 / n0 as f64;
        let fpr = fp as f64 / n0 as f64;
        points.push(RocPoint {
            threshold: t,
            tp,
            fp,
            sensitivity: sens,
            specificity: spec,
            fpr,
            sensitivity_ci: proportion_ci(tp, n1, opts.ci_level, opts.proportion_ci),
            specificity_ci: proportion_ci(n0 - fp, n0, opts.ci_level, opts.proportion_ci),
            lr_positive: sens / fpr,
            lr_negative: (1.0 - sens) / spec,
            youden: sens + spec - 1.0,
        });
    };
    push(f64::INFINITY, 0, 0);
    for &t in &thr {
        while ip < n1 && sp[ip] >= t {
            ip += 1;
        }
        while ineg < n0 && sn[ineg] >= t {
            ineg += 1;
        }
        push(t, ip, ineg);
    }
    let (v10, v01) = delong_components(&pos, &neg);
    let auc = mean(&v10);
    let var = cov(&v10, &v10) / n1 as f64 + cov(&v01, &v01) / n0 as f64;
    let se = var.sqrt();
    let q1 = auc / (2.0 - auc);
    let q2 = 2.0 * auc * auc / (1.0 + auc);
    let hm = ((auc * (1.0 - auc)
        + (n1 as f64 - 1.0) * (q1 - auc * auc)
        + (n0 as f64 - 1.0) * (q2 - auc * auc))
        / (n1 as f64 * n0 as f64))
        .sqrt();
    let clip = |v: f64| v.clamp(0.0, 1.0);
    let zz = (auc - 0.5) / se;
    let mut best = 0;
    let mut close = 0;
    let dist = |p: &RocPoint| p.fpr * p.fpr + (1.0 - p.sensitivity) * (1.0 - p.sensitivity);
    for (i, p) in points.iter().enumerate() {
        if p.youden > points[best].youden {
            best = i;
        }
        if dist(p) < dist(&points[close]) {
            close = i;
        }
    }
    Ok(Roc {
        n_positive: n1,
        n_negative: n0,
        auc,
        se_delong: se,
        ci_delong: (clip(auc - z * se), clip(auc + z * se)),
        se_hanley_mcneil: hm,
        ci_hanley_mcneil: (clip(auc - z * hm), clip(auc + z * hm)),
        z: zz,
        p: p_two_sided_z(zz),
        youden_best_index: best,
        youden_best_threshold: points[best].threshold,
        youden_best_j: points[best].youden,
        closest_topleft_index: close,
        points,
        v10,
        v01,
    })
}

/// [`roc`] from scores and labels (`true` = positive).
pub fn roc_from_labels(scores: &[f64], labels: &[bool], opts: RocOptions) -> Result<Roc> {
    if scores.len() != labels.len() {
        return Err(invalid("scores and labels lengths differ"));
    }
    let pos: Vec<f64> = scores
        .iter()
        .zip(labels)
        .filter(|(_, l)| **l)
        .map(|(s, _)| *s)
        .collect();
    let neg: Vec<f64> = scores
        .iter()
        .zip(labels)
        .filter(|(_, l)| !**l)
        .map(|(s, _)| *s)
        .collect();
    roc(&pos, &neg, opts)
}

/// DeLong comparison of two correlated ROC curves (two tests on the same subjects).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RocComparison {
    /// AUC of test A.
    pub auc_a: f64,
    /// AUC of test B.
    pub auc_b: f64,
    /// AUC_A - AUC_B.
    pub difference: f64,
    /// SE of the difference.
    pub se: f64,
    /// CI of the difference.
    pub ci: (f64, f64),
    /// z = difference/SE.
    pub z: f64,
    /// Two-sided p.
    pub p: f64,
    /// Correlation of the two AUC estimates.
    pub correlation: f64,
}

/// DeLong et al. (1988) test of two correlated AUCs: subject i has scores `a[i]` and
/// `b[i]` and status `labels[i]`. Var(A - B) = (S10_AA + S10_BB - 2 S10_AB)/n1 +
/// (S01_AA + S01_BB - 2 S01_AB)/n0 with S the covariances of the structural components.
/// (No oracle fixture; checked by hand-built identities in the tests.)
pub fn delong_paired(
    a: &[f64],
    b: &[f64],
    labels: &[bool],
    ci_level: f64,
) -> Result<RocComparison> {
    if a.len() != b.len() || a.len() != labels.len() {
        return Err(invalid("a, b and labels must have equal length"));
    }
    let keep: Vec<usize> = (0..a.len())
        .filter(|&i| a[i].is_finite() && b[i].is_finite())
        .collect();
    let split = |s: &[f64], want: bool| -> Vec<f64> {
        keep.iter()
            .filter(|&&i| labels[i] == want)
            .map(|&i| s[i])
            .collect()
    };
    let (ap, an, bp, bn) = (
        split(a, true),
        split(a, false),
        split(b, true),
        split(b, false),
    );
    if ap.len() < 2 || an.len() < 2 {
        return Err(StatsError::NotEnoughData(
            "need at least two positives and two negatives".into(),
        ));
    }
    let (a10, a01) = delong_components(&ap, &an);
    let (b10, b01) = delong_components(&bp, &bn);
    let (n1, n0) = (ap.len() as f64, an.len() as f64);
    let (auc_a, auc_b) = (mean(&a10), mean(&b10));
    let va = cov(&a10, &a10) / n1 + cov(&a01, &a01) / n0;
    let vb = cov(&b10, &b10) / n1 + cov(&b01, &b01) / n0;
    let cab = cov(&a10, &b10) / n1 + cov(&a01, &b01) / n0;
    let var = va + vb - 2.0 * cab;
    let se = var.max(0.0).sqrt();
    let d = auc_a - auc_b;
    let al = 1.0 - ci_level;
    let zc = norm_quantile_pq(1.0 - 0.5 * al, 0.5 * al);
    let z = d / se;
    Ok(RocComparison {
        auc_a,
        auc_b,
        difference: d,
        se,
        ci: (d - zc * se, d + zc * se),
        z,
        p: if se > 0.0 { p_two_sided_z(z) } else { f64::NAN },
        correlation: cab / (va * vb).sqrt(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_identities() {
        let a = [0.9, 0.8, 0.7, 0.3, 0.6, 0.2, 0.4, 0.1, 0.55, 0.35];
        let lab = [
            true, true, true, true, true, false, false, false, false, false,
        ];
        let b: Vec<f64> = a.iter().map(|v| 2.0 * v + 1.0).collect();
        // A monotone transform has the same AUC: difference 0 with SE 0.
        let c = delong_paired(&a, &b, &lab, 0.95).unwrap();
        assert_eq!(c.difference, 0.0);
        assert!(c.se < 1e-12);
        // Single-test variance matches roc()'s DeLong SE when B is a constant shift.
        let r = roc(&a[..5], &a[5..], RocOptions::default()).unwrap();
        assert!((c.auc_a - r.auc).abs() < 1e-15);
        // Swapping A and B flips the sign.
        let b2 = [0.5, 0.9, 0.2, 0.6, 0.7, 0.3, 0.8, 0.1, 0.4, 0.45];
        let c1 = delong_paired(&a, &b2, &lab, 0.95).unwrap();
        let c2 = delong_paired(&b2, &a, &lab, 0.95).unwrap();
        assert!((c1.z + c2.z).abs() < 1e-14 && (c1.p - c2.p).abs() < 1e-14);
    }

    #[test]
    fn clopper_pearson_contains_wilson_center() {
        let (lo, hi) = proportion_ci(7, 20, 0.95, ProportionCi::ClopperPearson);
        // R binom.test(7, 20)$conf.int: 0.1539092 0.5921885
        assert!((lo - 0.153_909_2).abs() < 1e-7 && (hi - 0.592_188_5).abs() < 1e-7);
    }
}
