//! t tests: one-sample, unpaired (Student, pooled variance), Welch, paired and ratio
//! paired, with confidence intervals and standardized effect sizes.
//!
//! Sign convention: differences are `x - y` (mean of the first argument minus the mean of
//! the second; paired differences x_i - y_i). Prism reports "B - A" (later column minus
//! earlier); the presentation layer flips signs where it mimics Prism.
//!
//! Missing values: `NaN`s are skipped in unpaired data; in paired data a pair with a `NaN`
//! on either side is dropped.

use crate::descriptive::compensated_sum;
use crate::dist::StudentT;
use crate::special;
use crate::{Result, StatsError};

/// Alternative hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Alternative {
    /// H1: the effect is not zero.
    #[default]
    TwoSided,
    /// H1: the effect (x - y, or mean - mu) is negative.
    Less,
    /// H1: the effect is positive.
    Greater,
}

/// Options shared by the t tests.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TTestOptions {
    /// Alternative hypothesis selecting [`TTestResult::p`].
    pub alternative: Alternative,
    /// Confidence level of the (two-sided) CI, e.g. 0.95.
    pub ci_level: f64,
}

impl Default for TTestOptions {
    fn default() -> Self {
        TTestOptions {
            alternative: Alternative::TwoSided,
            ci_level: 0.95,
        }
    }
}

/// Result of a t test.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TTestResult {
    /// Which test produced this.
    pub method: String,
    /// t statistic.
    pub t: f64,
    /// Degrees of freedom (fractional for Welch).
    pub df: f64,
    /// p for the requested alternative.
    pub p: f64,
    /// Two-sided p.
    pub p_two_sided: f64,
    /// One-sided p for H1: effect < 0.
    pub p_less: f64,
    /// One-sided p for H1: effect > 0.
    pub p_greater: f64,
    /// Alternative used for `p`.
    pub alternative: Alternative,
    /// Estimate: mean - mu (one-sample) or mean difference x - y.
    pub estimate: f64,
    /// Two-sided CI of `estimate` at `ci_level`.
    pub ci: (f64, f64),
    /// Confidence level.
    pub ci_level: f64,
    /// Standard error of `estimate`.
    pub se: f64,
    /// Size of the first sample (or number of pairs).
    pub n1: usize,
    /// Size of the second sample (0 for one-sample; number of pairs for paired tests).
    pub n2: usize,
    /// Cohen's d: difference / pooled SD (two samples, with n-1 weights) or mean / SD
    /// (one-sample and paired, the d_z form).
    pub cohens_d: f64,
    /// Hedges' g = d J(df), J(df) = Gamma(df/2)/(sqrt(df/2) Gamma((df-1)/2)) exactly
    /// (Hedges & Olkin 1985).
    pub hedges_g: f64,
    /// Hedges' g with the approximation J ~ 1 - 3/(4 df - 1).
    pub hedges_g_approx: f64,
    /// R squared = t^2/(t^2 + df) (Prism's "eta squared").
    pub r_squared: f64,
    /// Pooled SD (two-sample tests) or SD of the values/differences.
    pub sd: f64,
    /// Cohen's d with the average-variance denominator sqrt((s1^2+s2^2)/2) (two-sample only).
    pub cohens_d_av: Option<f64>,
}

/// Result of the ratio paired t test.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RatioPairedResult {
    /// Paired t test on log10(x) - log10(y); its estimate is the mean log10 ratio.
    pub log_test: TTestResult,
    /// Geometric mean of x/y = 10^(mean log10 ratio).
    pub geometric_mean_ratio: f64,
    /// Back-transformed CI of the ratio.
    pub ci_ratio: (f64, f64),
}

/// The exact small-sample correction J(df) = Gamma(df/2) / (sqrt(df/2) Gamma((df-1)/2))
/// of Hedges & Olkin (1985), via ln B((df-1)/2, 1/2) to keep accuracy for large df.
/// NaN for df <= 1.
pub fn hedges_j(df: f64) -> f64 {
    if !(df > 1.0) {
        return f64::NAN;
    }
    // Gamma(a)/Gamma(a - 1/2) = Gamma(1/2)/B(a - 1/2, 1/2), a = df/2.
    (special::LN_SQRT_PI - special::ln_beta(0.5 * (df - 1.0), 0.5)).exp() / (0.5 * df).sqrt()
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

fn mean_ss(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m0 = compensated_sum(v.iter().copied()) / n;
    let m = m0 + compensated_sum(v.iter().map(|x| x - m0)) / n;
    let s1 = compensated_sum(v.iter().map(|x| x - m));
    let s2 = compensated_sum(v.iter().map(|x| (x - m) * (x - m)));
    (m, (s2 - s1 * s1 / n).max(0.0))
}

fn check_opts(o: &TTestOptions) -> Result<()> {
    if !(o.ci_level > 0.0 && o.ci_level < 1.0) {
        return Err(StatsError::InvalidArgument(format!(
            "ci_level {} not in (0, 1)",
            o.ci_level
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish(
    method: &str,
    estimate: f64,
    se: f64,
    df: f64,
    n1: usize,
    n2: usize,
    d: f64,
    sd: f64,
    d_av: Option<f64>,
    o: &TTestOptions,
) -> Result<TTestResult> {
    if !(se > 0.0) {
        return Err(StatsError::InvalidArgument(format!(
            "{method}: standard error is zero (all values identical)"
        )));
    }
    let t = estimate / se;
    let dist = StudentT::new(df)?;
    let (p_less, p_greater) = dist.cdf_sf(t);
    let p_two = dist.two_sided_p(t);
    let alpha = 1.0 - o.ci_level;
    let tc = dist.quantile_pq(1.0 - 0.5 * alpha, 0.5 * alpha);
    let j = hedges_j(df);
    let p = match o.alternative {
        Alternative::TwoSided => p_two,
        Alternative::Less => p_less,
        Alternative::Greater => p_greater,
    };
    Ok(TTestResult {
        method: method.to_string(),
        t,
        df,
        p,
        p_two_sided: p_two,
        p_less,
        p_greater,
        alternative: o.alternative,
        estimate,
        ci: (estimate - tc * se, estimate + tc * se),
        ci_level: o.ci_level,
        se,
        n1,
        n2,
        cohens_d: d,
        hedges_g: d * j,
        hedges_g_approx: d * (1.0 - 3.0 / (4.0 * df - 1.0)),
        r_squared: t * t / (t * t + df),
        sd,
        cohens_d_av: d_av,
    })
}

/// One-sample t test of H0: mean = mu. t = (mean - mu)/SEM, df = n - 1 (Student 1908).
/// The estimate is mean - mu ("discrepancy" in Prism) and the CI is that of mean - mu.
pub fn one_sample(x: &[f64], mu: f64, opts: &TTestOptions) -> Result<TTestResult> {
    check_opts(opts)?;
    if !mu.is_finite() {
        return Err(StatsError::InvalidArgument(
            "hypothetical mean must be finite".into(),
        ));
    }
    let v = clean(x, "x")?;
    if v.len() < 2 {
        return Err(StatsError::NotEnoughData(
            "one-sample t test needs at least 2 values".into(),
        ));
    }
    let n = v.len() as f64;
    let (m, ss) = mean_ss(&v);
    let sd = (ss / (n - 1.0)).sqrt();
    let est = m - mu;
    finish(
        "one-sample t test",
        est,
        sd / n.sqrt(),
        n - 1.0,
        v.len(),
        0,
        est / sd,
        sd,
        None,
        opts,
    )
}

/// Unpaired t test with pooled variance (Student): s_p^2 = ((n1-1)s1^2 + (n2-1)s2^2)/(n1+n2-2),
/// SE = s_p sqrt(1/n1 + 1/n2), df = n1 + n2 - 2. Estimate mean(x) - mean(y).
pub fn unpaired(x: &[f64], y: &[f64], opts: &TTestOptions) -> Result<TTestResult> {
    two_sample(x, y, opts, false)
}

/// Welch's unequal-variance t test: SE = sqrt(s1^2/n1 + s2^2/n2) with the
/// Welch–Satterthwaite df = SE^4 / ((s1^2/n1)^2/(n1-1) + (s2^2/n2)^2/(n2-1)), kept
/// fractional (Welch 1947; Satterthwaite 1946). Effect sizes use the pooled SD as in the
/// Student test.
pub fn welch(x: &[f64], y: &[f64], opts: &TTestOptions) -> Result<TTestResult> {
    two_sample(x, y, opts, true)
}

fn two_sample(x: &[f64], y: &[f64], opts: &TTestOptions, welch: bool) -> Result<TTestResult> {
    check_opts(opts)?;
    let a = clean(x, "x")?;
    let b = clean(y, "y")?;
    if a.len() < 2 || b.len() < 2 {
        return Err(StatsError::NotEnoughData(
            "two-sample t test needs at least 2 values per group".into(),
        ));
    }
    let (n1, n2) = (a.len() as f64, b.len() as f64);
    let (m1, ss1) = mean_ss(&a);
    let (m2, ss2) = mean_ss(&b);
    let v1 = ss1 / (n1 - 1.0);
    let v2 = ss2 / (n2 - 1.0);
    let sp = ((ss1 + ss2) / (n1 + n2 - 2.0)).sqrt();
    let diff = m1 - m2;
    let d = diff / sp;
    let d_av = diff / (0.5 * (v1 + v2)).sqrt();
    if welch {
        let a1 = v1 / n1;
        let a2 = v2 / n2;
        let se2 = a1 + a2;
        let df = se2 * se2 / (a1 * a1 / (n1 - 1.0) + a2 * a2 / (n2 - 1.0));
        let mut r = finish(
            "Welch t test",
            diff,
            se2.sqrt(),
            df,
            a.len(),
            b.len(),
            d,
            sp,
            Some(d_av),
            opts,
        )?;
        // Effect sizes keep the pooled-variance df, as in the Student test.
        let j = hedges_j(n1 + n2 - 2.0);
        r.hedges_g = d * j;
        r.hedges_g_approx = d * (1.0 - 3.0 / (4.0 * (n1 + n2 - 2.0) - 1.0));
        Ok(r)
    } else {
        let se = sp * (1.0 / n1 + 1.0 / n2).sqrt();
        finish(
            "unpaired t test (pooled variance)",
            diff,
            se,
            n1 + n2 - 2.0,
            a.len(),
            b.len(),
            d,
            sp,
            Some(d_av),
            opts,
        )
    }
}

fn paired_diffs(x: &[f64], y: &[f64]) -> Result<Vec<f64>> {
    if x.len() != y.len() {
        return Err(StatsError::InvalidArgument(format!(
            "paired data need equal lengths ({} vs {})",
            x.len(),
            y.len()
        )));
    }
    let mut d = Vec::with_capacity(x.len());
    for (&a, &b) in x.iter().zip(y) {
        if a.is_nan() || b.is_nan() {
            continue;
        }
        if a.is_infinite() || b.is_infinite() {
            return Err(StatsError::InvalidArgument(
                "infinite value in paired data".into(),
            ));
        }
        d.push(a - b);
    }
    Ok(d)
}

fn paired_on(d: &[f64], method: &str, opts: &TTestOptions) -> Result<TTestResult> {
    if d.len() < 2 {
        return Err(StatsError::NotEnoughData(format!(
            "{method} needs at least 2 complete pairs"
        )));
    }
    let n = d.len() as f64;
    let (m, ss) = mean_ss(d);
    let sd = (ss / (n - 1.0)).sqrt();
    finish(
        method,
        m,
        sd / n.sqrt(),
        n - 1.0,
        d.len(),
        d.len(),
        m / sd,
        sd,
        None,
        opts,
    )
}

/// Paired t test: a one-sample t test on the differences x_i - y_i. Cohen's d is the
/// d_z form mean(d)/SD(d).
pub fn paired(x: &[f64], y: &[f64], opts: &TTestOptions) -> Result<TTestResult> {
    check_opts(opts)?;
    let d = paired_diffs(x, y)?;
    paired_on(&d, "paired t test", opts)
}

/// Ratio paired t test: a paired t test on log10(x) - log10(y); reports the geometric
/// mean ratio 10^mean and its back-transformed CI (Prism's "ratio paired t test",
/// GraphPad guide; log base 10 as Prism uses; t and p do not depend on the base).
/// Every value must be > 0.
pub fn ratio_paired(x: &[f64], y: &[f64], opts: &TTestOptions) -> Result<RatioPairedResult> {
    check_opts(opts)?;
    if x.len() != y.len() {
        return Err(StatsError::InvalidArgument(
            "paired data need equal lengths".into(),
        ));
    }
    let mut d = Vec::with_capacity(x.len());
    for (&a, &b) in x.iter().zip(y) {
        if a.is_nan() || b.is_nan() {
            continue;
        }
        if !(a > 0.0 && b > 0.0) || a.is_infinite() || b.is_infinite() {
            return Err(StatsError::InvalidArgument(
                "ratio paired t test needs every value finite and > 0".into(),
            ));
        }
        d.push(a.log10() - b.log10());
    }
    let r = paired_on(&d, "ratio paired t test (log10)", opts)?;
    let g = 10f64.powf(r.estimate);
    let ci = (10f64.powf(r.ci.0), 10f64.powf(r.ci.1));
    Ok(RatioPairedResult {
        log_test: r,
        geometric_mean_ratio: g,
        ci_ratio: ci,
    })
}
