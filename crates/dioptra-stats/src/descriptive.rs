//! Descriptive statistics of one column.
//!
//! Missing values: every function here skips `NaN` (the engine's representation of a
//! blank or excluded cell). Infinite values are rejected with
//! [`StatsError::InvalidArgument`].
//!
//! Accuracy: sums use Neumaier's compensated summation; the mean is the two-pass corrected
//! mean and the variance the corrected two-pass algorithm of Chan, Golub & LeVeque (1983),
//! so the NIST StRD NumAcc sets keep all the digits their inputs carry.

use crate::dist::StudentT;
use crate::{Result, StatsError};

/// Quantile (percentile) definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum QuantileMethod {
    /// Hyndman & Fan (1996) definition 7: h = (n-1)p, linear interpolation
    /// (NumPy/SciPy default, Excel `PERCENTILE.INC`).
    Linear,
    /// Hyndman & Fan (1996) definition 6: rank (n+1)p, clamped to [min, max]
    /// (GraphPad Prism, Minitab, SPSS, Excel `PERCENTILE.EXC`). The engine default.
    #[default]
    Prism,
    /// Tukey's hinges: medians of the lower and upper halves, the median included in both
    /// halves when n is odd (R `fivenum`). Defined only for p in {0, 0.25, 0.5, 0.75, 1}.
    TukeyHinges,
}

/// Neumaier's improved Kahan summation (Neumaier 1974, ZAMM 54:39).
pub fn compensated_sum<I: IntoIterator<Item = f64>>(it: I) -> f64 {
    let mut sum = 0.0;
    let mut c = 0.0;
    for x in it {
        let t = sum + x;
        if sum.abs() >= x.abs() {
            c += (sum - t) + x;
        } else {
            c += (x - t) + sum;
        }
        sum = t;
    }
    sum + c
}

/// The non-missing values (NaN skipped); error on infinities.
pub fn clean(data: &[f64]) -> Result<Vec<f64>> {
    let mut v = Vec::with_capacity(data.len());
    for &x in data {
        if x.is_nan() {
            continue;
        }
        if x.is_infinite() {
            return Err(StatsError::InvalidArgument("infinite value in data".into()));
        }
        v.push(x);
    }
    Ok(v)
}

fn need(v: &[f64], n: usize, what: &str) -> Result<()> {
    if v.len() < n {
        return Err(StatsError::NotEnoughData(format!(
            "{what} needs at least {n} non-missing values, got {}",
            v.len()
        )));
    }
    Ok(())
}

pub(crate) fn mean_of(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    let m0 = compensated_sum(v.iter().copied()) / n;
    // Second pass: add the mean residual (Higham 2002, §1.9 / ch. 4).
    m0 + compensated_sum(v.iter().map(|x| x - m0)) / n
}

/// Sum of squared deviations about the mean, by the corrected two-pass algorithm
/// sum (x - m)^2 - (sum (x - m))^2 / n (Chan, Golub & LeVeque 1983).
pub(crate) fn ss_of(v: &[f64], m: f64) -> f64 {
    let n = v.len() as f64;
    let s1 = compensated_sum(v.iter().map(|x| x - m));
    let s2 = compensated_sum(v.iter().map(|x| (x - m) * (x - m)));
    (s2 - s1 * s1 / n).max(0.0)
}

/// Number of non-missing values.
pub fn count(data: &[f64]) -> usize {
    data.iter().filter(|x| !x.is_nan()).count()
}

/// Sum (compensated).
pub fn sum(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    Ok(compensated_sum(v))
}

/// Arithmetic mean (two-pass corrected; Higham 2002).
pub fn mean(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 1, "mean")?;
    Ok(mean_of(&v))
}

/// Sample variance with the n - 1 denominator (Chan, Golub & LeVeque 1983 two-pass).
pub fn variance(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 2, "variance")?;
    let m = mean_of(&v);
    Ok(ss_of(&v, m) / (v.len() as f64 - 1.0))
}

/// Sample standard deviation (n - 1).
pub fn sd(data: &[f64]) -> Result<f64> {
    variance(data).map(f64::sqrt)
}

/// Population standard deviation (n denominator).
pub fn sd_population(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 1, "population SD")?;
    let m = mean_of(&v);
    Ok((ss_of(&v, m) / v.len() as f64).sqrt())
}

/// Standard error of the mean SD/sqrt(n).
pub fn sem(data: &[f64]) -> Result<f64> {
    let n = count(data);
    sd(data).map(|s| s / (n as f64).sqrt())
}

/// Coefficient of variation SD/mean as a fraction (Prism shows it as a percent).
pub fn cv(data: &[f64]) -> Result<f64> {
    Ok(sd(data)? / mean(data)?)
}

/// Minimum.
pub fn min(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 1, "min")?;
    Ok(v.iter().copied().fold(f64::INFINITY, f64::min))
}

/// Maximum.
pub fn max(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 1, "max")?;
    Ok(v.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// Range max - min.
pub fn range(data: &[f64]) -> Result<f64> {
    Ok(max(data)? - min(data)?)
}

fn sorted(data: &[f64]) -> Result<Vec<f64>> {
    let mut v = clean(data)?;
    v.sort_by(f64::total_cmp);
    Ok(v)
}

/// NumPy's `_lerp`: a + t (b - a), or b - (1 - t)(b - a) for t >= 1/2 (monotone, exact at ends).
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    let d = b - a;
    if t >= 0.5 {
        b - d * (1.0 - t)
    } else {
        a + d * t
    }
}

/// Quantile of already sorted data at probability p in [0, 1].
pub fn quantile_sorted(s: &[f64], p: f64, method: QuantileMethod) -> Result<f64> {
    need(s, 1, "quantile")?;
    if !(0.0..=1.0).contains(&p) {
        return Err(StatsError::InvalidArgument(format!(
            "quantile probability {p} not in [0, 1]"
        )));
    }
    let n = s.len();
    match method {
        QuantileMethod::Linear | QuantileMethod::Prism => {
            let nf = n as f64;
            // NumPy's virtual index: (n-1)p for type 7, n p + p - 1 for type 6.
            let h = match method {
                QuantileMethod::Linear => (nf - 1.0) * p,
                _ => nf * p + p - 1.0,
            };
            let h = h.clamp(0.0, nf - 1.0);
            let lo = h.floor();
            let g = h - lo;
            let i = lo as usize;
            if i + 1 >= n {
                return Ok(s[n - 1]);
            }
            Ok(lerp(s[i], s[i + 1], g))
        }
        QuantileMethod::TukeyHinges => {
            let half = n.div_ceil(2);
            if p == 0.0 {
                Ok(s[0])
            } else if p == 1.0 {
                Ok(s[n - 1])
            } else if p == 0.5 {
                Ok(median_sorted(s))
            } else if p == 0.25 {
                Ok(median_sorted(&s[..half]))
            } else if p == 0.75 {
                Ok(median_sorted(&s[n - half..]))
            } else {
                Err(StatsError::InvalidArgument(
                    "Tukey hinges are defined only for p in {0, 0.25, 0.5, 0.75, 1}".into(),
                ))
            }
        }
    }
}

fn median_sorted(s: &[f64]) -> f64 {
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        // NumPy's mean of the two middle values.
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// Quantile at probability p (0..1) under `method` (Hyndman & Fan 1996).
pub fn quantile(data: &[f64], p: f64, method: QuantileMethod) -> Result<f64> {
    quantile_sorted(&sorted(data)?, p, method)
}

/// Percentile at `pct` in [0, 100].
pub fn percentile(data: &[f64], pct: f64, method: QuantileMethod) -> Result<f64> {
    quantile(data, pct / 100.0, method)
}

/// Median (the same under every quantile definition).
pub fn median(data: &[f64]) -> Result<f64> {
    let s = sorted(data)?;
    need(&s, 1, "median")?;
    Ok(median_sorted(&s))
}

/// (Q1, median, Q3) under `method`.
pub fn quartiles(data: &[f64], method: QuantileMethod) -> Result<(f64, f64, f64)> {
    let s = sorted(data)?;
    Ok((
        quantile_sorted(&s, 0.25, method)?,
        quantile_sorted(&s, 0.5, method)?,
        quantile_sorted(&s, 0.75, method)?,
    ))
}

/// Central moments m2, m3, m4 (n denominators) about the corrected mean.
fn moments(v: &[f64]) -> (f64, f64, f64, f64) {
    let m = mean_of(v);
    let n = v.len() as f64;
    let m2 = compensated_sum(v.iter().map(|x| (x - m).powi(2))) / n;
    let m3 = compensated_sum(v.iter().map(|x| (x - m).powi(3))) / n;
    let m4 = compensated_sum(v.iter().map(|x| (x - m).powi(4))) / n;
    (m, m2, m3, m4)
}

/// Biased (moment) skewness g1 = m3 / m2^{3/2} (SciPy `skew(bias=True)`).
pub fn skewness_g1(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 2, "skewness")?;
    let (_, m2, m3, _) = moments(&v);
    Ok(m3 / m2.powf(1.5))
}

/// Bias-corrected skewness G1 = g1 sqrt(n(n-1))/(n-2), the value Prism and Excel `SKEW`
/// report (Joanes & Gill 1998).
pub fn skewness(data: &[f64]) -> Result<f64> {
    let n = count(data) as f64;
    if n < 3.0 {
        return Err(StatsError::NotEnoughData(
            "skewness G1 needs at least 3 values".into(),
        ));
    }
    Ok(skewness_g1(data)? * (n * (n - 1.0)).sqrt() / (n - 2.0))
}

/// Biased excess kurtosis g2 = m4/m2^2 - 3 (SciPy `kurtosis(bias=True)`).
pub fn kurtosis_g2(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 2, "kurtosis")?;
    let (_, m2, _, m4) = moments(&v);
    Ok(m4 / (m2 * m2) - 3.0)
}

/// Bias-corrected excess kurtosis G2 = ((n+1) g2 + 6)(n-1)/((n-2)(n-3)), the value Prism
/// and Excel `KURT` report (Joanes & Gill 1998).
pub fn kurtosis(data: &[f64]) -> Result<f64> {
    let n = count(data) as f64;
    if n < 4.0 {
        return Err(StatsError::NotEnoughData(
            "kurtosis G2 needs at least 4 values".into(),
        ));
    }
    let g2 = kurtosis_g2(data)?;
    Ok(((n + 1.0) * g2 + 6.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0)))
}

fn logs(data: &[f64], what: &str) -> Result<Vec<f64>> {
    let v = clean(data)?;
    if v.iter().any(|&x| x <= 0.0) {
        return Err(StatsError::InvalidArgument(format!(
            "{what} needs every value > 0"
        )));
    }
    Ok(v.iter().map(|x| x.ln()).collect())
}

/// Geometric mean exp(mean(ln x)); all values must be > 0.
pub fn geometric_mean(data: &[f64]) -> Result<f64> {
    let l = logs(data, "geometric mean")?;
    need(&l, 1, "geometric mean")?;
    Ok(mean_of(&l).exp())
}

/// Geometric SD factor exp(SD(ln x)) (Kirkwood 1979; Prism's "geometric SD factor").
pub fn geometric_sd_factor(data: &[f64]) -> Result<f64> {
    let l = logs(data, "geometric SD")?;
    need(&l, 2, "geometric SD")?;
    let m = mean_of(&l);
    Ok((ss_of(&l, m) / (l.len() as f64 - 1.0)).sqrt().exp())
}

/// Harmonic mean n / sum(1/x); all values must be > 0.
pub fn harmonic_mean(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    if v.iter().any(|&x| x <= 0.0) {
        return Err(StatsError::InvalidArgument(
            "harmonic mean needs every value > 0".into(),
        ));
    }
    need(&v, 1, "harmonic mean")?;
    Ok(v.len() as f64 / compensated_sum(v.iter().map(|x| 1.0 / x)))
}

fn t_crit(level: f64, df: f64) -> Result<f64> {
    if !(level > 0.0 && level < 1.0) {
        return Err(StatsError::InvalidArgument(format!(
            "confidence level {level} not in (0, 1)"
        )));
    }
    let alpha = 1.0 - level;
    Ok(StudentT::new(df)?.quantile_pq(1.0 - 0.5 * alpha, 0.5 * alpha))
}

/// Confidence interval of the mean: mean +/- t_{(1+level)/2, n-1} SEM.
pub fn ci_mean(data: &[f64], level: f64) -> Result<(f64, f64)> {
    let v = clean(data)?;
    need(&v, 2, "CI of the mean")?;
    let n = v.len() as f64;
    let m = mean_of(&v);
    let se = (ss_of(&v, m) / (n - 1.0)).sqrt() / n.sqrt();
    let t = t_crit(level, n - 1.0)?;
    Ok((m - t * se, m + t * se))
}

/// Confidence interval of the geometric mean: exp(mean(ln x) +/- t SEM(ln x)).
pub fn ci_geometric_mean(data: &[f64], level: f64) -> Result<(f64, f64)> {
    let l = logs(data, "CI of the geometric mean")?;
    let (a, b) = ci_mean(&l, level)?;
    Ok((a.exp(), b.exp()))
}

/// Lag-1 autocorrelation r1 = sum_{i<n} (x_i - m)(x_{i+1} - m) / sum (x_i - m)^2, the
/// statistic certified by NIST StRD for the univariate sets. Missing values are skipped
/// (the series is the non-missing values in order).
pub fn lag1_autocorrelation(data: &[f64]) -> Result<f64> {
    let v = clean(data)?;
    need(&v, 2, "autocorrelation")?;
    let m = mean_of(&v);
    // Deviations a_i = x_i - m from the rounded mean, then the exact correction for the
    // mean's rounding delta = sum(a)/n (as in the corrected two-pass variance):
    // sum (a_i - delta)(a_{i+1} - delta) = sum a_i a_{i+1} - delta sum(a_i + a_{i+1}) + (n-1) delta^2.
    let n = v.len() as f64;
    let delta = compensated_sum(v.iter().map(|x| x - m)) / n;
    let cross = compensated_sum(v.windows(2).map(|w| (w[0] - m) * (w[1] - m)));
    let pair_sum = compensated_sum(v.windows(2).map(|w| (w[0] - m) + (w[1] - m)));
    let num = cross - delta * pair_sum + (n - 1.0) * delta * delta;
    let den = ss_of(&v, m);
    Ok(num / den)
}

/// Options for [`Summary::compute`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SummaryOptions {
    /// Quartile/percentile definition (default: Prism's type 6).
    pub quantile_method: QuantileMethod,
    /// Confidence level of the CI of the mean (default 0.95).
    pub ci_level: f64,
}

impl Default for SummaryOptions {
    fn default() -> Self {
        SummaryOptions {
            quantile_method: QuantileMethod::Prism,
            ci_level: 0.95,
        }
    }
}

/// Column statistics as Prism's "Descriptive statistics" table reports them.
/// Fields that are undefined for the data (e.g. a geometric mean of data with
/// negatives, or a kurtosis with n < 4) are `None`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Summary {
    /// Number of non-missing values.
    pub n: usize,
    /// Number of missing (NaN) values skipped.
    pub n_missing: usize,
    /// Sum.
    pub sum: f64,
    /// Mean.
    pub mean: f64,
    /// Sample variance (n - 1).
    pub variance: Option<f64>,
    /// Sample SD (n - 1).
    pub sd: Option<f64>,
    /// SEM.
    pub sem: Option<f64>,
    /// Coefficient of variation (fraction).
    pub cv: Option<f64>,
    /// Minimum.
    pub min: f64,
    /// Maximum.
    pub max: f64,
    /// Range.
    pub range: f64,
    /// Median.
    pub median: f64,
    /// 25th percentile under `quantile_method`.
    pub q1: f64,
    /// 75th percentile under `quantile_method`.
    pub q3: f64,
    /// 10th percentile (`None` for Tukey hinges).
    pub p10: Option<f64>,
    /// 90th percentile (`None` for Tukey hinges).
    pub p90: Option<f64>,
    /// Quantile definition used.
    pub quantile_method: QuantileMethod,
    /// CI of the mean at `ci_level`.
    pub ci_mean: Option<(f64, f64)>,
    /// Confidence level.
    pub ci_level: f64,
    /// Bias-corrected skewness G1 (Prism).
    pub skewness: Option<f64>,
    /// Bias-corrected excess kurtosis G2 (Prism).
    pub kurtosis: Option<f64>,
    /// Biased skewness g1.
    pub skewness_g1: Option<f64>,
    /// Biased excess kurtosis g2.
    pub kurtosis_g2: Option<f64>,
    /// Geometric mean (all values > 0).
    pub geometric_mean: Option<f64>,
    /// Geometric SD factor.
    pub geometric_sd_factor: Option<f64>,
    /// CI of the geometric mean.
    pub ci_geometric_mean: Option<(f64, f64)>,
    /// Harmonic mean (all values > 0).
    pub harmonic_mean: Option<f64>,
}

impl Summary {
    /// Compute every column statistic in one call. Errors only when there is no
    /// non-missing value or an infinity is present.
    pub fn compute(data: &[f64], opts: &SummaryOptions) -> Result<Summary> {
        let v = clean(data)?;
        need(&v, 1, "descriptive statistics")?;
        let n = v.len();
        let mut s = v.clone();
        s.sort_by(f64::total_cmp);
        let m = mean_of(&v);
        let var = if n >= 2 {
            Some(ss_of(&v, m) / (n as f64 - 1.0))
        } else {
            None
        };
        let sdv = var.map(f64::sqrt);
        let all_pos = v.iter().all(|&x| x > 0.0);
        let ok = |r: Result<f64>| r.ok();
        let (p10, p90) = if opts.quantile_method == QuantileMethod::TukeyHinges {
            (None, None)
        } else {
            (
                ok(quantile_sorted(&s, 0.1, opts.quantile_method)),
                ok(quantile_sorted(&s, 0.9, opts.quantile_method)),
            )
        };
        Ok(Summary {
            n,
            n_missing: data.len() - n,
            sum: compensated_sum(v.iter().copied()),
            mean: m,
            variance: var,
            sd: sdv,
            sem: sdv.map(|x| x / (n as f64).sqrt()),
            cv: sdv.map(|x| x / m),
            min: s[0],
            max: s[n - 1],
            range: s[n - 1] - s[0],
            median: median_sorted(&s),
            q1: quantile_sorted(&s, 0.25, opts.quantile_method)?,
            q3: quantile_sorted(&s, 0.75, opts.quantile_method)?,
            p10,
            p90,
            quantile_method: opts.quantile_method,
            ci_mean: ci_mean(&v, opts.ci_level).ok(),
            ci_level: opts.ci_level,
            skewness: ok(skewness(&v)),
            kurtosis: ok(kurtosis(&v)),
            skewness_g1: ok(skewness_g1(&v)),
            kurtosis_g2: ok(kurtosis_g2(&v)),
            geometric_mean: if all_pos {
                ok(geometric_mean(&v))
            } else {
                None
            },
            geometric_sd_factor: if all_pos {
                ok(geometric_sd_factor(&v))
            } else {
                None
            },
            ci_geometric_mean: if all_pos {
                ci_geometric_mean(&v, opts.ci_level).ok()
            } else {
                None
            },
            harmonic_mean: if all_pos { ok(harmonic_mean(&v)) } else { None },
        })
    }
}
