//! Method comparison and agreement: Bland–Altman analysis, intraclass correlation and
//! Cohen's kappa.

use crate::descriptive::compensated_sum;
use crate::dist::{p_two_sided_z, p_upper_f, FisherF, StudentT};
use crate::regression::{simple_linear_regression, SimpleRegressionOptions};
use crate::special::norm_quantile_pq;
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

fn check_level(l: f64) -> Result<()> {
    if !(l > 0.0 && l < 1.0) {
        return Err(invalid("ci_level must be in (0, 1)"));
    }
    Ok(())
}

// =======================================================================================
// Bland–Altman
// =======================================================================================

/// What is plotted on the Y axis of a Bland–Altman plot (Prism's choices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BaKind {
    /// A - B.
    #[default]
    Difference,
    /// A/B.
    Ratio,
    /// 100 (A - B)/mean(A, B).
    PercentDifference,
}

/// Regression of the differences on the averages (proportional bias).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProportionalBias {
    /// Slope.
    pub slope: f64,
    /// Intercept.
    pub intercept: f64,
    /// SE of the slope.
    pub slope_se: f64,
    /// CI of the slope.
    pub slope_ci: (f64, f64),
    /// p of slope = 0 (t on n - 2 df).
    pub p: f64,
}

/// Bland–Altman analysis.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlandAltman {
    /// What was analysed.
    pub kind: BaKind,
    /// Complete pairs.
    pub n: usize,
    /// Per pair: A - B (or ratio, or % difference).
    pub differences: Vec<f64>,
    /// Per pair: (A + B)/2.
    pub averages: Vec<f64>,
    /// Mean difference.
    pub bias: f64,
    /// SD of the differences (n - 1).
    pub sd: f64,
    /// bias - 1.96 SD (Prism uses the constant 1.96 at 95%; z quantile at other levels).
    pub loa_lower: f64,
    /// bias + 1.96 SD.
    pub loa_upper: f64,
    /// bias - z SD with the exact normal quantile.
    pub loa_lower_exact_z: f64,
    /// bias + z SD with the exact normal quantile.
    pub loa_upper_exact_z: f64,
    /// bias +/- t(n-1) SD/sqrt(n).
    pub bias_ci: (f64, f64),
    /// SE of each limit, SD sqrt(1/n + 1.96^2/(2(n-1))) (Bland & Altman 1999, 4.2).
    pub loa_se: f64,
    /// lower limit +/- t SE.
    pub loa_lower_ci: (f64, f64),
    /// upper limit +/- t SE.
    pub loa_upper_ci: (f64, f64),
    /// OLS of differences on averages.
    pub proportional_bias: Option<ProportionalBias>,
}

/// Bland & Altman (1986, Lancet 327:307; 1999, Stat Methods Med Res 8:135) analysis of
/// paired measurements `a`, `b` (pairs with a missing value dropped).
pub fn bland_altman(a: &[f64], b: &[f64], kind: BaKind, ci_level: f64) -> Result<BlandAltman> {
    check_level(ci_level)?;
    if a.len() != b.len() {
        return Err(invalid("a and b lengths differ"));
    }
    let mut d = Vec::new();
    let mut m = Vec::new();
    for (x, y) in a.iter().zip(b) {
        if x.is_nan() || y.is_nan() {
            continue;
        }
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid("infinite value"));
        }
        let avg = 0.5 * (x + y);
        d.push(match kind {
            BaKind::Difference => x - y,
            BaKind::Ratio => x / y,
            BaKind::PercentDifference => 100.0 * (x - y) / avg,
        });
        m.push(avg);
    }
    let n = d.len();
    if n < 2 {
        return Err(StatsError::NotEnoughData("need at least 2 pairs".into()));
    }
    if d.iter().any(|v| !v.is_finite()) {
        return Err(invalid(
            "ratio or percent difference not finite (zero denominator)",
        ));
    }
    let nf = n as f64;
    let bias = compensated_sum(d.iter().copied()) / nf;
    let ss = compensated_sum(d.iter().map(|v| (v - bias) * (v - bias)));
    let sd = (ss / (nf - 1.0)).sqrt();
    let alpha = 1.0 - ci_level;
    let z = norm_quantile_pq(1.0 - 0.5 * alpha, 0.5 * alpha);
    let k = if (ci_level - 0.95).abs() < 1e-12 {
        1.96
    } else {
        z
    };
    let t = StudentT { df: nf - 1.0 }.quantile_pq(1.0 - 0.5 * alpha, 0.5 * alpha);
    let se_bias = sd / nf.sqrt();
    let loa_se = sd * (1.0 / nf + k * k / (2.0 * (nf - 1.0))).sqrt();
    let (lo, hi) = (bias - k * sd, bias + k * sd);
    let pb = if n >= 3 {
        simple_linear_regression(
            &m,
            &d,
            SimpleRegressionOptions {
                ci_level,
                ..Default::default()
            },
        )
        .ok()
        .map(|r| ProportionalBias {
            slope: r.slope,
            intercept: r.intercept,
            slope_se: r.se_slope,
            slope_ci: r.ci_slope,
            p: r.p_slope,
        })
    } else {
        None
    };
    Ok(BlandAltman {
        kind,
        n,
        bias,
        sd,
        loa_lower: lo,
        loa_upper: hi,
        loa_lower_exact_z: bias - z * sd,
        loa_upper_exact_z: bias + z * sd,
        bias_ci: (bias - t * se_bias, bias + t * se_bias),
        loa_se,
        loa_lower_ci: (lo - t * loa_se, lo + t * loa_se),
        loa_upper_ci: (hi - t * loa_se, hi + t * loa_se),
        proportional_bias: pb,
        differences: d,
        averages: m,
    })
}

// =======================================================================================
// Intraclass correlation
// =======================================================================================

/// One intraclass correlation coefficient.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IccValue {
    /// Shrout & Fleiss name: "ICC(1,1)", "ICC(2,1)", "ICC(3,1)", "ICC(1,k)", ...
    pub name: String,
    /// McGraw & Wong description.
    pub description: String,
    /// The coefficient.
    pub icc: f64,
    /// F statistic of H0: ICC = 0.
    pub f: f64,
    /// Numerator df.
    pub df1: f64,
    /// Denominator df.
    pub df2: f64,
    /// P(F >= f).
    pub p: f64,
    /// CI lower limit.
    pub ci_lower: f64,
    /// CI upper limit.
    pub ci_upper: f64,
}

/// The six Shrout–Fleiss intraclass correlations.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Icc {
    /// Subjects (rows) used.
    pub n: usize,
    /// Raters (columns).
    pub k: usize,
    /// Between-subjects mean square.
    pub ms_rows: f64,
    /// Between-raters mean square.
    pub ms_columns: f64,
    /// Residual mean square (two-way).
    pub ms_error: f64,
    /// Within-subjects mean square (one-way).
    pub ms_within: f64,
    /// ICC(1,1), ICC(2,1), ICC(3,1), ICC(1,k), ICC(2,k), ICC(3,k).
    pub values: Vec<IccValue>,
}

/// Intraclass correlations of an n x k matrix (rows = subjects, columns = raters; rows
/// with a missing value dropped): Shrout & Fleiss (1979), Psychol Bull 86:420, single and
/// average measures, with F tests and the F-based CIs of McGraw & Wong (1996), Psychol
/// Methods 1:30 (Satterthwaite df for ICC(2,.)), as R `psych::ICC` computes them.
/// No oracle fixture: tested against the Shrout–Fleiss worked example.
pub fn icc(rows: &[Vec<f64>], ci_level: f64) -> Result<Icc> {
    check_level(ci_level)?;
    let k = rows.first().map_or(0, |r| r.len());
    if k < 2 {
        return Err(invalid("need at least two raters (columns)"));
    }
    let data: Vec<&Vec<f64>> = rows
        .iter()
        .filter(|r| r.len() == k && r.iter().all(|v| v.is_finite()))
        .collect();
    let n = data.len();
    if n < 2 {
        return Err(StatsError::NotEnoughData(
            "need at least two complete rows".into(),
        ));
    }
    let (nf, kf) = (n as f64, k as f64);
    let grand = compensated_sum(data.iter().flat_map(|r| r.iter().copied())) / (nf * kf);
    let row_means: Vec<f64> = data.iter().map(|r| r.iter().sum::<f64>() / kf).collect();
    let col_means: Vec<f64> = (0..k)
        .map(|j| data.iter().map(|r| r[j]).sum::<f64>() / nf)
        .collect();
    let ssr = kf * compensated_sum(row_means.iter().map(|m| (m - grand) * (m - grand)));
    let ssc = nf * compensated_sum(col_means.iter().map(|m| (m - grand) * (m - grand)));
    let sst = compensated_sum(
        data.iter()
            .flat_map(|r| r.iter().map(|v| (v - grand) * (v - grand))),
    );
    let sse = (sst - ssr - ssc).max(0.0);
    let msr = ssr / (nf - 1.0);
    let msc = ssc / (kf - 1.0);
    let mse = sse / ((nf - 1.0) * (kf - 1.0));
    let msw = (ssc + sse) / (nf * (kf - 1.0));
    let a2 = 0.5 * (1.0 - ci_level);
    let fq = |d1: f64, d2: f64| FisherF { d1, d2 }.quantile_pq(1.0 - a2, a2);
    // one-way
    let f1 = msr / msw;
    let (d11, d12) = (nf - 1.0, nf * (kf - 1.0));
    let f1l = f1 / fq(d11, d12);
    let f1u = f1 * fq(d12, d11);
    // two-way
    let f3 = msr / mse;
    let (d21, d22) = (nf - 1.0, (nf - 1.0) * (kf - 1.0));
    let f3l = f3 / fq(d21, d22);
    let f3u = f3 * fq(d22, d21);
    let icc1 = (msr - msw) / (msr + (kf - 1.0) * msw);
    let icc2 = (msr - mse) / (msr + (kf - 1.0) * mse + kf * (msc - mse) / nf);
    let icc3 = (msr - mse) / (msr + (kf - 1.0) * mse);
    let icc1k = (msr - msw) / msr;
    let icc2k = (msr - mse) / (msr + (msc - mse) / nf);
    let icc3k = (msr - mse) / msr;
    // McGraw & Wong CI for ICC(2,1), Satterthwaite df
    let fj = msc / mse;
    let vn = (kf - 1.0)
        * (nf - 1.0)
        * (kf * icc2 * fj + nf * (1.0 + (kf - 1.0) * icc2) - kf * icc2).powi(2);
    let vd = (nf - 1.0) * kf * kf * icc2 * icc2 * fj * fj
        + (nf * (1.0 + (kf - 1.0) * icc2) - kf * icc2).powi(2);
    let v = vn / vd;
    let fs = fq(nf - 1.0, v);
    let fss = fq(v, nf - 1.0);
    let l2 = nf * (msr - fs * mse) / (fs * (kf * msc + (kf * nf - kf - nf) * mse) + nf * msr);
    let u2 = nf * (fss * msr - mse) / (kf * msc + (kf * nf - kf - nf) * mse + nf * fss * msr);
    let p1 = p_upper_f(f1, d11, d12);
    let p3 = p_upper_f(f3, d21, d22);
    let mk =
        |name: &str, desc: &str, icc: f64, f: f64, d1: f64, d2: f64, p: f64, lo: f64, hi: f64| {
            IccValue {
                name: name.into(),
                description: desc.into(),
                icc,
                f,
                df1: d1,
                df2: d2,
                p,
                ci_lower: lo,
                ci_upper: hi,
            }
        };
    let values = vec![
        mk(
            "ICC(1,1)",
            "one-way random, single measures",
            icc1,
            f1,
            d11,
            d12,
            p1,
            (f1l - 1.0) / (f1l + kf - 1.0),
            (f1u - 1.0) / (f1u + kf - 1.0),
        ),
        mk(
            "ICC(2,1)",
            "two-way random, absolute agreement, single measures",
            icc2,
            f3,
            d21,
            d22,
            p3,
            l2,
            u2,
        ),
        mk(
            "ICC(3,1)",
            "two-way mixed, consistency, single measures",
            icc3,
            f3,
            d21,
            d22,
            p3,
            (f3l - 1.0) / (f3l + kf - 1.0),
            (f3u - 1.0) / (f3u + kf - 1.0),
        ),
        mk(
            "ICC(1,k)",
            "one-way random, average measures",
            icc1k,
            f1,
            d11,
            d12,
            p1,
            1.0 - 1.0 / f1l,
            1.0 - 1.0 / f1u,
        ),
        mk(
            "ICC(2,k)",
            "two-way random, absolute agreement, average measures",
            icc2k,
            f3,
            d21,
            d22,
            p3,
            l2 * kf / (1.0 + l2 * (kf - 1.0)),
            u2 * kf / (1.0 + u2 * (kf - 1.0)),
        ),
        mk(
            "ICC(3,k)",
            "two-way mixed, consistency, average measures",
            icc3k,
            f3,
            d21,
            d22,
            p3,
            1.0 - 1.0 / f3l,
            1.0 - 1.0 / f3u,
        ),
    ];
    Ok(Icc {
        n,
        k,
        ms_rows: msr,
        ms_columns: msc,
        ms_error: mse,
        ms_within: msw,
        values,
    })
}

// =======================================================================================
// Cohen's kappa
// =======================================================================================

/// Disagreement weights for kappa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum KappaWeights {
    /// Unweighted (Cohen 1960).
    #[default]
    None,
    /// w_ij = 1 - |i-j|/(k-1) (Cicchetti & Allison 1971).
    Linear,
    /// w_ij = 1 - ((i-j)/(k-1))^2 (Fleiss & Cohen 1973).
    Quadratic,
}

/// Cohen's kappa.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kappa {
    /// Weights used.
    pub weights: KappaWeights,
    /// Subjects.
    pub n: f64,
    /// Observed (weighted) agreement.
    pub po: f64,
    /// Chance (weighted) agreement.
    pub pe: f64,
    /// kappa = (po - pe)/(1 - pe).
    pub kappa: f64,
    /// Asymptotic SE (Fleiss, Cohen & Everitt 1969), used for the CI.
    pub se: f64,
    /// kappa +/- z SE.
    pub ci: (f64, f64),
    /// SE under H0: kappa = 0.
    pub se0: f64,
    /// z = kappa/SE0.
    pub z: f64,
    /// Two-sided p.
    pub p: f64,
}

/// Cohen's (1960) kappa from a k x k agreement table (rows: rater 1, columns: rater 2),
/// optionally weighted, with the large-sample SEs of Fleiss, Cohen & Everitt (1969),
/// Psychol Bull 72:323 (general weighted form; unweighted is the identity weights).
pub fn cohen_kappa(table: &[Vec<f64>], weights: KappaWeights, ci_level: f64) -> Result<Kappa> {
    check_level(ci_level)?;
    let k = table.len();
    if k < 2 || table.iter().any(|r| r.len() != k) {
        return Err(invalid(
            "kappa needs a square table with at least 2 categories",
        ));
    }
    if table
        .iter()
        .flatten()
        .any(|v| !(v.is_finite() && *v >= 0.0))
    {
        return Err(invalid("counts must be finite and >= 0"));
    }
    let n: f64 = table.iter().flatten().sum();
    if !(n > 0.0) {
        return Err(StatsError::NotEnoughData("empty table".into()));
    }
    let p: Vec<Vec<f64>> = table
        .iter()
        .map(|r| r.iter().map(|v| v / n).collect())
        .collect();
    let row: Vec<f64> = p.iter().map(|r| r.iter().sum()).collect();
    let col: Vec<f64> = (0..k).map(|j| p.iter().map(|r| r[j]).sum()).collect();
    let km1 = (k - 1) as f64;
    let w = |i: usize, j: usize| -> f64 {
        let d = (i as f64 - j as f64).abs() / km1;
        match weights {
            KappaWeights::None => {
                if i == j {
                    1.0
                } else {
                    0.0
                }
            }
            KappaWeights::Linear => 1.0 - d,
            KappaWeights::Quadratic => 1.0 - d * d,
        }
    };
    let mut po = 0.0;
    let mut pe = 0.0;
    for i in 0..k {
        for j in 0..k {
            po += w(i, j) * p[i][j];
            pe += w(i, j) * row[i] * col[j];
        }
    }
    let kappa = (po - pe) / (1.0 - pe);
    // weighted means of the weights
    let wi: Vec<f64> = (0..k)
        .map(|i| (0..k).map(|j| col[j] * w(i, j)).sum())
        .collect();
    let wj: Vec<f64> = (0..k)
        .map(|j| (0..k).map(|i| row[i] * w(i, j)).sum())
        .collect();
    let mut s = 0.0;
    let mut s0 = 0.0;
    for i in 0..k {
        for j in 0..k {
            let a = w(i, j) - (wi[i] + wj[j]) * (1.0 - kappa);
            s += p[i][j] * a * a;
            let b = w(i, j) - (wi[i] + wj[j]);
            s0 += row[i] * col[j] * b * b;
        }
    }
    let t = kappa - pe * (1.0 - kappa);
    let se = ((s - t * t).max(0.0) / (n * (1.0 - pe) * (1.0 - pe))).sqrt();
    let se0 = ((s0 - pe * pe).max(0.0) / (n * (1.0 - pe) * (1.0 - pe))).sqrt();
    let a = 1.0 - ci_level;
    let zc = norm_quantile_pq(1.0 - 0.5 * a, 0.5 * a);
    let z = kappa / se0;
    Ok(Kappa {
        weights,
        n,
        po,
        pe,
        kappa,
        se,
        ci: (kappa - zc * se, kappa + zc * se),
        se0,
        z,
        p: p_two_sided_z(z),
    })
}

/// [`cohen_kappa`] from two vectors of category codes (0..k-1 after sorting the distinct
/// codes of both raters).
pub fn cohen_kappa_ratings(
    r1: &[f64],
    r2: &[f64],
    weights: KappaWeights,
    ci_level: f64,
) -> Result<Kappa> {
    if r1.len() != r2.len() {
        return Err(invalid("rating vectors differ in length"));
    }
    let mut cats: Vec<f64> = r1
        .iter()
        .chain(r2)
        .copied()
        .filter(|v| !v.is_nan())
        .collect();
    cats.sort_by(f64::total_cmp);
    cats.dedup();
    let k = cats.len().max(2);
    let mut t = vec![vec![0.0; k]; k];
    for (a, b) in r1.iter().zip(r2) {
        if a.is_nan() || b.is_nan() {
            continue;
        }
        let i = cats.iter().position(|c| c == a).unwrap_or(0);
        let j = cats.iter().position(|c| c == b).unwrap_or(0);
        t[i][j] += 1.0;
    }
    cohen_kappa(&t, weights, ci_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shrout & Fleiss (1979) Table 2: 6 targets, 4 judges; values as printed by
    /// R psych::ICC (ICC 0.17 0.29 0.71 0.44 0.62 0.91; bounds -0.133/0.72, 0.019/0.76,
    /// 0.342/0.95, -0.884/0.91, 0.071/0.93, 0.676/0.99; F 1.8 and 11; p 0.165, 0.00013).
    #[test]
    fn shrout_fleiss_example() {
        let d = vec![
            vec![9.0, 2.0, 5.0, 8.0],
            vec![6.0, 1.0, 3.0, 2.0],
            vec![8.0, 4.0, 6.0, 8.0],
            vec![7.0, 1.0, 2.0, 6.0],
            vec![10.0, 5.0, 6.0, 9.0],
            vec![6.0, 2.0, 4.0, 7.0],
        ];
        let r = icc(&d, 0.95).unwrap();
        let want = [
            (0.17, -0.133, 0.72),
            (0.29, 0.019, 0.76),
            (0.71, 0.342, 0.95),
            (0.44, -0.884, 0.91),
            (0.62, 0.071, 0.93),
            (0.91, 0.676, 0.99),
        ];
        for (v, w) in r.values.iter().zip(want) {
            assert!((v.icc - w.0).abs() < 0.005, "{}: {}", v.name, v.icc);
            assert!(
                (v.ci_lower - w.1).abs() < 0.0015,
                "{}: {}",
                v.name,
                v.ci_lower
            );
            assert!(
                (v.ci_upper - w.2).abs() < 0.005,
                "{}: {}",
                v.name,
                v.ci_upper
            );
        }
        assert!((r.values[0].f - 1.795).abs() < 0.01);
        assert!((r.values[1].f - 11.03).abs() < 0.01);
        assert!((r.values[0].p - 0.16477).abs() < 5e-5);
        assert!((r.values[1].p - 0.00013).abs() < 5e-6);
    }

    #[test]
    fn kappa_hand_computed() {
        // po = 0.75, pe = 0.35*0.40 + 0.65*0.60 = 0.53, kappa = 0.22/0.47
        let t = vec![vec![25.0, 10.0], vec![15.0, 50.0]];
        let k = cohen_kappa(&t, KappaWeights::None, 0.95).unwrap();
        assert!((k.kappa - 0.22 / 0.47).abs() < 1e-15);
        // SE0 for 2x2: sqrt((pe + pe^2 - sum r_i c_i (r_i + c_i))/(n (1-pe)^2))
        let pe: f64 = 0.53;
        let s: f64 = 0.35 * 0.40 * (0.35 + 0.40) + 0.65 * 0.60 * (0.65 + 0.60);
        let se0 = ((pe + pe * pe - s) / (100.0 * (1.0 - pe) * (1.0 - pe))).sqrt();
        assert!((k.se0 - se0).abs() < 1e-15);
        // Weighted kappa of a 2x2 equals the unweighted one.
        let kl = cohen_kappa(&t, KappaWeights::Linear, 0.95).unwrap();
        assert!((kl.kappa - k.kappa).abs() < 1e-15 && (kl.se - k.se).abs() < 1e-15);
        let kr = cohen_kappa_ratings(
            &[1.0, 1.0, 2.0, 2.0],
            &[1.0, 2.0, 2.0, 2.0],
            KappaWeights::None,
            0.95,
        )
        .unwrap();
        assert!((kr.kappa - 0.5).abs() < 1e-15);
    }
}
