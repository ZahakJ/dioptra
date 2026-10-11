//! Probability distributions: density, cdf, survival function (upper tail computed
//! directly), quantile (`ppf`) and inverse survival function (`isf`).
//!
//! Every distribution validates its parameters in its constructor and returns
//! [`StatsError::InvalidArgument`] otherwise; the evaluation methods are total (they
//! return `NaN` for a `NaN` argument and the appropriate limit outside the support).
//!
//! Tail rule: `sf` never computes `1 - cdf`. The t, F and beta families pass both
//! x and 1 - x to [`special::betainc_pair_xy`]; chi-square and gamma use
//! [`special::gammainc_pair`]; the normal uses erfc and, beyond |z| = 5, the Laplace
//! continued fraction for Mills' ratio. Quantiles solve against whichever tail is smaller.

use crate::quad;
use crate::roots;
use crate::special::{self, INV_SQRT_2PI, LN_SQRT_2PI};
use crate::{Result, StatsError};

/// A continuous univariate distribution.
pub trait Continuous {
    /// Probability density at x.
    fn pdf(&self, x: f64) -> f64;
    /// P(X <= x).
    fn cdf(&self, x: f64) -> f64;
    /// P(X > x), computed directly (not as 1 - cdf).
    fn sf(&self, x: f64) -> f64;
    /// Quantile: smallest x with cdf(x) >= p.
    fn ppf(&self, p: f64) -> f64;
    /// Inverse survival function: x with sf(x) = q (accurate for tiny q).
    fn isf(&self, q: f64) -> f64;
}

fn invalid(msg: &str) -> StatsError {
    StatsError::InvalidArgument(msg.to_string())
}

fn check_prob(p: f64) -> bool {
    (0.0..=1.0).contains(&p)
}

// =======================================================================================
// Standard normal kernels
// =======================================================================================

/// Mills' ratio R(t) = (1 - Phi(t)) / phi(t) for t >= 5 by Laplace's continued fraction
/// R(t) = 1/(t + 1/(t + 2/(t + 3/(t + ...)))) (Abramowitz & Stegun 26.2.14), evaluated
/// with the modified Lentz method.
fn mills_ratio(t: f64) -> f64 {
    let tiny = 1e-300;
    let mut f = t;
    let mut c = t;
    let mut d = 0.0;
    let mut k = 1.0;
    while k < 5000.0 {
        d = t + k * d;
        if d.abs() < tiny {
            d = tiny;
        }
        d = 1.0 / d;
        c = t + k / c;
        if c.abs() < tiny {
            c = tiny;
        }
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() <= f64::EPSILON {
            break;
        }
        k += 1.0;
    }
    1.0 / f
}

/// Standard normal density phi(z), with the argument split (z = z1 + z2, z1 having 16
/// fractional bits) so that exp(-z^2/2) keeps full relative accuracy for large |z|
/// (as in R's `dnorm`).
pub fn std_normal_pdf(z: f64) -> f64 {
    let x = z.abs();
    if x < 5.0 {
        return INV_SQRT_2PI * (-0.5 * x * x).exp();
    }
    if x > (2.0 * f64::MAX.ln()).sqrt() {
        return 0.0;
    }
    let x1 = (x * 65536.0).round() / 65536.0;
    let x2 = x - x1;
    INV_SQRT_2PI * (-0.5 * x1 * x1).exp() * ((-0.5 * x2 - x1) * x2).exp()
}

/// ln phi(z).
pub fn std_normal_ln_pdf(z: f64) -> f64 {
    -0.5 * z * z - LN_SQRT_2PI
}

/// Upper tail Q(t) = 1 - Phi(t) for t >= 0.
///
/// Q(t) = erfc(t/sqrt 2)/2, with the rounding of the argument t/sqrt 2 corrected:
/// z = fl(t/sqrt 2), delta = t/sqrt 2 - z from an exact residual (fma with a two-term
/// sqrt 2), and erfc(z + delta) = erfc(z) exp(-delta (2z + delta)) to first order in the
/// (tiny) delta. Without the correction the relative error grows like t^2 eps. Beyond
/// t = 37, where erfc underflows, phi(t) times Mills' ratio.
fn upper_tail_nonneg(t: f64) -> f64 {
    const SQRT2_HI: f64 = std::f64::consts::SQRT_2;
    const SQRT2_LO: f64 = -9.667293313452913e-17;
    if t < 37.0 {
        let z = t * std::f64::consts::FRAC_1_SQRT_2;
        let r = (-z).mul_add(SQRT2_HI, t) - z * SQRT2_LO; // t - z sqrt 2, nearly exact
        let delta = r * std::f64::consts::FRAC_1_SQRT_2;
        0.5 * special::erfc(z) * (-delta * (2.0 * z + delta)).exp()
    } else {
        let v = std_normal_pdf(t) * mills_ratio(t);
        if v.is_finite() {
            v
        } else {
            0.0
        }
    }
}

/// Standard normal cdf Phi(z) via erfc (fdlibm's rational approximations, about 1 ulp)
/// with the argument-rounding correction of `upper_tail_nonneg`; phi(z) times Mills'
/// ratio beyond |z| = 37. Relative accuracy is kept to the underflow limit.
pub fn std_normal_cdf(z: f64) -> f64 {
    if z.is_nan() {
        return f64::NAN;
    }
    if z < 0.0 {
        upper_tail_nonneg(-z)
    } else {
        1.0 - upper_tail_nonneg(z)
    }
}

/// (Phi(z), 1 - Phi(z)) from a single erfc evaluation (the smaller tail is computed
/// directly, the other as its complement).
fn normal_cdf_sf(z: f64) -> (f64, f64) {
    if z < 0.0 {
        let c = upper_tail_nonneg(-z);
        (c, 1.0 - c)
    } else {
        let u = upper_tail_nonneg(z);
        (1.0 - u, u)
    }
}

/// Standard normal survival function 1 - Phi(z), computed directly.
pub fn std_normal_sf(z: f64) -> f64 {
    std_normal_cdf(-z)
}

/// ln Phi(z), accurate in the far lower tail (ln phi(z) + ln R(-z) for z < -5).
pub fn std_normal_ln_cdf(z: f64) -> f64 {
    if z.is_nan() {
        return f64::NAN;
    }
    if z < -5.0 {
        std_normal_ln_pdf(z) + mills_ratio(-z).ln()
    } else if z < 0.5 {
        std_normal_cdf(z).ln()
    } else {
        (-std_normal_sf(z)).ln_1p()
    }
}

/// ln(1 - Phi(z)).
pub fn std_normal_ln_sf(z: f64) -> f64 {
    std_normal_ln_cdf(-z)
}

// =======================================================================================
// Normal
// =======================================================================================

/// Normal distribution N(mean, sd^2).
///
/// cdf/sf via erfc and Mills' ratio (see [`std_normal_cdf`]); quantile by Wichura's
/// AS 241 PPND16 (1988), the algorithm of R's `qnorm`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Normal {
    /// Mean.
    pub mean: f64,
    /// Standard deviation (> 0).
    pub sd: f64,
}

impl Normal {
    /// N(mean, sd^2); sd must be finite and > 0.
    pub fn new(mean: f64, sd: f64) -> Result<Self> {
        if !mean.is_finite() || !(sd > 0.0) || !sd.is_finite() {
            return Err(invalid("Normal: mean must be finite and sd > 0"));
        }
        Ok(Normal { mean, sd })
    }

    /// The standard normal N(0, 1).
    pub fn standard() -> Self {
        Normal { mean: 0.0, sd: 1.0 }
    }

    /// ln P(X <= x).
    pub fn ln_cdf(&self, x: f64) -> f64 {
        std_normal_ln_cdf((x - self.mean) / self.sd)
    }

    /// ln P(X > x).
    pub fn ln_sf(&self, x: f64) -> f64 {
        std_normal_ln_sf((x - self.mean) / self.sd)
    }

    /// ln density.
    pub fn ln_pdf(&self, x: f64) -> f64 {
        std_normal_ln_pdf((x - self.mean) / self.sd) - self.sd.ln()
    }
}

impl Continuous for Normal {
    fn pdf(&self, x: f64) -> f64 {
        std_normal_pdf((x - self.mean) / self.sd) / self.sd
    }
    fn cdf(&self, x: f64) -> f64 {
        std_normal_cdf((x - self.mean) / self.sd)
    }
    fn sf(&self, x: f64) -> f64 {
        std_normal_sf((x - self.mean) / self.sd)
    }
    fn ppf(&self, p: f64) -> f64 {
        self.mean + self.sd * special::norm_quantile(p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.mean + self.sd * special::norm_quantile_upper(q)
    }
}

// =======================================================================================
// Student t
// =======================================================================================

/// Student's t distribution with `df` > 0 degrees of freedom (non-integer allowed;
/// `df = inf` is the standard normal).
///
/// cdf: P(T <= -|t|) = I_w(df/2, 1/2)/2 with w = df/(df+t^2) and 1 - w = t^2/(df+t^2)
/// passed separately (Abramowitz & Stegun 26.7.1; the incomplete beta of
/// [`special::betainc_pair_xy`]). Quantile by inverting the incomplete beta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StudentT {
    /// Degrees of freedom.
    pub df: f64,
}

impl StudentT {
    /// t distribution; df > 0 (may be +inf).
    pub fn new(df: f64) -> Result<Self> {
        if !(df > 0.0) {
            return Err(invalid("StudentT: df must be > 0"));
        }
        Ok(StudentT { df })
    }

    /// (P(T <= t), P(T > t)).
    pub fn cdf_sf(&self, t: f64) -> (f64, f64) {
        if t.is_nan() {
            return (f64::NAN, f64::NAN);
        }
        if self.df.is_infinite() {
            return (std_normal_cdf(t), std_normal_sf(t));
        }
        if t.is_infinite() {
            return if t > 0.0 { (1.0, 0.0) } else { (0.0, 1.0) };
        }
        let df = self.df;
        let t2 = t * t;
        let (w, y) = if t2 > 1e300 {
            (df / t2, 1.0)
        } else if t2 > df {
            (df / (df + t2), 1.0 / (1.0 + df / t2))
        } else {
            let y = t2 / (df + t2);
            (1.0 / (1.0 + t2 / df), y)
        };
        // I_w(df/2, 1/2) = P(|T| > |t|), and its complement P(|T| <= |t|).
        let (iw, icw) = special::betainc_pair_xy(0.5 * df, 0.5, w, y);
        let tail = 0.5 * iw;
        let body = 0.5 + 0.5 * icw;
        if t < 0.0 {
            (tail, body)
        } else {
            (body, tail)
        }
    }

    /// Two-sided p-value P(|T| >= |t|) = I_w(df/2, 1/2), computed directly.
    pub fn two_sided_p(&self, t: f64) -> f64 {
        if t.is_nan() {
            return f64::NAN;
        }
        if self.df.is_infinite() {
            return 2.0 * std_normal_sf(t.abs());
        }
        let (c, _) = self.cdf_sf(-t.abs());
        (2.0 * c).min(1.0)
    }

    /// Quantile from a lower-tail p and upper-tail q = 1 - p (the smaller drives the
    /// inversion).
    pub fn quantile_pq(&self, p: f64, q: f64) -> f64 {
        if !check_prob(p) || !check_prob(q) {
            return f64::NAN;
        }
        if self.df.is_infinite() {
            return special::norm_quantile_pq(p, q);
        }
        if p == 0.0 {
            return f64::NEG_INFINITY;
        }
        if q == 0.0 {
            return f64::INFINITY;
        }
        if p == q {
            return 0.0;
        }
        let (tail, sign) = if p < q { (p, -1.0) } else { (q, 1.0) };
        // I_w(df/2, 1/2) = 2 tail.
        let two_tail = 2.0 * tail;
        let rest = if p < q { q - p } else { p - q };
        let (w, y) = special::betainc_inv_pq(0.5 * self.df, 0.5, two_tail, rest);
        if w == 0.0 {
            return sign * f64::INFINITY;
        }
        sign * (self.df * y / w).sqrt()
    }

    /// ln density.
    pub fn ln_pdf(&self, t: f64) -> f64 {
        if self.df.is_infinite() {
            return std_normal_ln_pdf(t);
        }
        let df = self.df;
        -0.5 * (df + 1.0) * (t * t / df).ln_1p() - 0.5 * df.ln() - special::ln_beta(0.5 * df, 0.5)
    }
}

impl Continuous for StudentT {
    fn pdf(&self, t: f64) -> f64 {
        self.ln_pdf(t).exp()
    }
    fn cdf(&self, t: f64) -> f64 {
        self.cdf_sf(t).0
    }
    fn sf(&self, t: f64) -> f64 {
        self.cdf_sf(t).1
    }
    fn ppf(&self, p: f64) -> f64 {
        self.quantile_pq(p, 1.0 - p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.quantile_pq(1.0 - q, q)
    }
}

// =======================================================================================
// Chi-squared and Gamma
// =======================================================================================

/// Gamma distribution with shape k > 0 and scale theta > 0.
///
/// cdf = P(k, x/theta) and sf = Q(k, x/theta), the regularised incomplete gamma functions
/// of [`special::gammainc_pair`]; quantile by [`special::gammainc_inv_pq`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gamma {
    /// Shape k.
    pub shape: f64,
    /// Scale theta.
    pub scale: f64,
}

impl Gamma {
    /// Gamma(shape, scale); both must be finite and > 0.
    pub fn new(shape: f64, scale: f64) -> Result<Self> {
        if !(shape > 0.0 && shape.is_finite() && scale > 0.0 && scale.is_finite()) {
            return Err(invalid("Gamma: shape and scale must be finite and > 0"));
        }
        Ok(Gamma { shape, scale })
    }
    /// Quantile from p and q = 1 - p.
    pub fn quantile_pq(&self, p: f64, q: f64) -> f64 {
        self.scale * special::gammainc_inv_pq(self.shape, p, q)
    }
}

impl Continuous for Gamma {
    fn pdf(&self, x: f64) -> f64 {
        if x < 0.0 {
            return 0.0;
        }
        if x == 0.0 {
            return if self.shape < 1.0 {
                f64::INFINITY
            } else if self.shape == 1.0 {
                1.0 / self.scale
            } else {
                0.0
            };
        }
        let z = x / self.scale;
        (special::ln_gamma_prefactor(self.shape, z)).exp() / x
    }
    fn cdf(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return if x.is_nan() { f64::NAN } else { 0.0 };
        }
        special::gammainc_pair(self.shape, x / self.scale).0
    }
    fn sf(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return if x.is_nan() { f64::NAN } else { 1.0 };
        }
        special::gammainc_pair(self.shape, x / self.scale).1
    }
    fn ppf(&self, p: f64) -> f64 {
        self.quantile_pq(p, 1.0 - p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.quantile_pq(1.0 - q, q)
    }
}

/// Chi-squared distribution with `df` > 0 degrees of freedom: Gamma(df/2, scale 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiSquared {
    /// Degrees of freedom.
    pub df: f64,
}

impl ChiSquared {
    /// Chi-squared(df), df finite and > 0.
    pub fn new(df: f64) -> Result<Self> {
        if !(df > 0.0 && df.is_finite()) {
            return Err(invalid("ChiSquared: df must be finite and > 0"));
        }
        Ok(ChiSquared { df })
    }
    fn g(&self) -> Gamma {
        Gamma {
            shape: 0.5 * self.df,
            scale: 2.0,
        }
    }
    /// Quantile from p and q = 1 - p.
    pub fn quantile_pq(&self, p: f64, q: f64) -> f64 {
        self.g().quantile_pq(p, q)
    }
}

impl Continuous for ChiSquared {
    fn pdf(&self, x: f64) -> f64 {
        self.g().pdf(x)
    }
    fn cdf(&self, x: f64) -> f64 {
        self.g().cdf(x)
    }
    fn sf(&self, x: f64) -> f64 {
        self.g().sf(x)
    }
    fn ppf(&self, p: f64) -> f64 {
        self.g().ppf(p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.g().isf(q)
    }
}

// =======================================================================================
// F and Beta
// =======================================================================================

/// Fisher's F distribution with (d1, d2) degrees of freedom.
///
/// cdf = I_w(d1/2, d2/2) with w = d1 x/(d1 x + d2), 1 - w = d2/(d1 x + d2) passed
/// separately (Abramowitz & Stegun 26.6.2); quantile by inverting the incomplete beta,
/// solving for whichever of w and 1 - w is smaller (this is where SciPy's `f.isf` loses
/// digits for q < 1e-8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FisherF {
    /// Numerator degrees of freedom.
    pub d1: f64,
    /// Denominator degrees of freedom.
    pub d2: f64,
}

impl FisherF {
    /// F(d1, d2); both finite and > 0.
    pub fn new(d1: f64, d2: f64) -> Result<Self> {
        if !(d1 > 0.0 && d2 > 0.0 && d1.is_finite() && d2.is_finite()) {
            return Err(invalid("FisherF: d1 and d2 must be finite and > 0"));
        }
        Ok(FisherF { d1, d2 })
    }

    fn wy(&self, x: f64) -> (f64, f64) {
        let a = self.d1 * x;
        if a > self.d2 {
            (1.0 / (1.0 + self.d2 / a), 1.0 / (a / self.d2 + 1.0))
        } else {
            (a / (a + self.d2), self.d2 / (a + self.d2))
        }
    }

    /// (P(F <= x), P(F > x)).
    pub fn cdf_sf(&self, x: f64) -> (f64, f64) {
        if x.is_nan() {
            return (f64::NAN, f64::NAN);
        }
        if x <= 0.0 {
            return (0.0, 1.0);
        }
        if x.is_infinite() {
            return (1.0, 0.0);
        }
        let (w, y) = self.wy(x);
        special::betainc_pair_xy(0.5 * self.d1, 0.5 * self.d2, w, y)
    }

    /// Quantile from p and q = 1 - p.
    pub fn quantile_pq(&self, p: f64, q: f64) -> f64 {
        if !check_prob(p) || !check_prob(q) {
            return f64::NAN;
        }
        if p == 0.0 {
            return 0.0;
        }
        if q == 0.0 {
            return f64::INFINITY;
        }
        let (w, y) = special::betainc_inv_pq(0.5 * self.d1, 0.5 * self.d2, p, q);
        if y == 0.0 {
            return f64::INFINITY;
        }
        self.d2 * w / (self.d1 * y)
    }
}

impl Continuous for FisherF {
    fn pdf(&self, x: f64) -> f64 {
        if x < 0.0 {
            return 0.0;
        }
        if x == 0.0 {
            return if self.d1 < 2.0 {
                f64::INFINITY
            } else if self.d1 == 2.0 {
                1.0
            } else {
                0.0
            };
        }
        let (w, y) = self.wy(x);
        special::ln_beta_prefactor(0.5 * self.d1, 0.5 * self.d2, w, y).exp() / x
    }
    fn cdf(&self, x: f64) -> f64 {
        self.cdf_sf(x).0
    }
    fn sf(&self, x: f64) -> f64 {
        self.cdf_sf(x).1
    }
    fn ppf(&self, p: f64) -> f64 {
        self.quantile_pq(p, 1.0 - p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.quantile_pq(1.0 - q, q)
    }
}

/// Beta distribution Beta(a, b) on [0, 1]: cdf I_x(a, b) ([`special::betainc_pair`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beta {
    /// First shape a > 0.
    pub a: f64,
    /// Second shape b > 0.
    pub b: f64,
}

impl Beta {
    /// Beta(a, b); both finite and > 0.
    pub fn new(a: f64, b: f64) -> Result<Self> {
        if !(a > 0.0 && b > 0.0 && a.is_finite() && b.is_finite()) {
            return Err(invalid("Beta: a and b must be finite and > 0"));
        }
        Ok(Beta { a, b })
    }
    /// Quantile from p and q = 1 - p.
    pub fn quantile_pq(&self, p: f64, q: f64) -> f64 {
        special::betainc_inv_pq(self.a, self.b, p, q).0
    }
}

impl Continuous for Beta {
    fn pdf(&self, x: f64) -> f64 {
        if !(0.0..=1.0).contains(&x) {
            return if x.is_nan() { f64::NAN } else { 0.0 };
        }
        if x == 0.0 || x == 1.0 {
            let e = if x == 0.0 { self.a } else { self.b };
            return if e < 1.0 {
                f64::INFINITY
            } else if e == 1.0 {
                (-special::ln_beta(self.a, self.b)).exp()
            } else {
                0.0
            };
        }
        let y = 1.0 - x;
        (special::ln_beta_prefactor(self.a, self.b, x, y) - x.ln() - y.ln()).exp()
    }
    fn cdf(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        special::betainc_pair(self.a, self.b, x.clamp(0.0, 1.0)).0
    }
    fn sf(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        special::betainc_pair(self.a, self.b, x.clamp(0.0, 1.0)).1
    }
    fn ppf(&self, p: f64) -> f64 {
        self.quantile_pq(p, 1.0 - p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.quantile_pq(1.0 - q, q)
    }
}

// =======================================================================================
// Discrete: Binomial and Hypergeometric
// =======================================================================================

/// Binomial distribution B(n, p).
///
/// pmf by Loader's saddle-point method ([`special::binomial_pmf_raw`], R's `dbinom`);
/// cdf P(X <= k) = I_{1-p}(n-k, k+1) and sf P(X > k) = I_p(k+1, n-k)
/// (Abramowitz & Stegun 26.5.24), each computed directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Binomial {
    /// Number of trials.
    pub n: u64,
    /// Success probability.
    pub p: f64,
}

impl Binomial {
    /// B(n, p) with 0 <= p <= 1.
    pub fn new(n: u64, p: f64) -> Result<Self> {
        if !check_prob(p) {
            return Err(invalid("Binomial: p must be in [0, 1]"));
        }
        Ok(Binomial { n, p })
    }
    /// P(X = k).
    pub fn pmf(&self, k: i64) -> f64 {
        if k < 0 || k as u64 > self.n {
            return 0.0;
        }
        special::binomial_pmf_raw(k as f64, self.n as f64, self.p, 1.0 - self.p)
    }
    /// (P(X <= k), P(X > k)).
    pub fn cdf_sf(&self, k: i64) -> (f64, f64) {
        if k < 0 {
            return (0.0, 1.0);
        }
        if k as u64 >= self.n {
            return (1.0, 0.0);
        }
        let n = self.n as f64;
        let kf = k as f64;
        let q = 1.0 - self.p;
        if self.p == 0.0 {
            return (1.0, 0.0);
        }
        if q == 0.0 {
            return (0.0, 1.0);
        }
        special::betainc_pair_xy(n - kf, kf + 1.0, q, self.p)
    }
    /// P(X <= k).
    pub fn cdf(&self, k: i64) -> f64 {
        self.cdf_sf(k).0
    }
    /// P(X > k).
    pub fn sf(&self, k: i64) -> f64 {
        self.cdf_sf(k).1
    }
    /// Smallest k with P(X <= k) >= prob.
    pub fn quantile(&self, prob: f64) -> f64 {
        if !check_prob(prob) {
            return f64::NAN;
        }
        discrete_quantile(|k| self.cdf(k), 0, self.n as i64, prob)
    }
}

fn discrete_quantile<F: Fn(i64) -> f64>(cdf: F, lo: i64, hi: i64, prob: f64) -> f64 {
    let (mut a, mut b) = (lo, hi);
    if cdf(a) >= prob {
        return a as f64;
    }
    while b - a > 1 {
        let m = a + (b - a) / 2;
        if cdf(m) >= prob {
            b = m;
        } else {
            a = m;
        }
    }
    b as f64
}

/// Hypergeometric distribution: `draws` items drawn without replacement from a population
/// of `population` items of which `successes` are successes (SciPy's `hypergeom(M, n, N)`
/// with M = population, n = successes, N = draws).
///
/// pmf = b(k; K, p) b(N-k; M-K, p) / b(N; M, p) with p = N/M and Loader's binomial
/// kernel (R's `dhyper`); cdf and sf by summing the pmf from k outward with the term
/// ratio (R's `pdhyper`), always on the side away from the mode so that both tails keep
/// relative accuracy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hypergeometric {
    /// Population size M.
    pub population: u64,
    /// Number of successes in the population K.
    pub successes: u64,
    /// Number of draws N.
    pub draws: u64,
}

impl Hypergeometric {
    /// Hypergeometric(M, K, N) with K <= M and N <= M.
    pub fn new(population: u64, successes: u64, draws: u64) -> Result<Self> {
        if successes > population || draws > population {
            return Err(invalid(
                "Hypergeometric: successes and draws must not exceed the population",
            ));
        }
        Ok(Hypergeometric {
            population,
            successes,
            draws,
        })
    }
    fn support(&self) -> (i64, i64) {
        let (m, k, n) = (
            self.population as i64,
            self.successes as i64,
            self.draws as i64,
        );
        ((n + k - m).max(0), k.min(n))
    }
    /// P(X = k).
    pub fn pmf(&self, k: i64) -> f64 {
        let (lo, hi) = self.support();
        if k < lo || k > hi {
            return 0.0;
        }
        let (m, kk, n) = (
            self.population as f64,
            self.successes as f64,
            self.draws as f64,
        );
        if n == 0.0 || n == m {
            return 1.0;
        }
        let p = n / m;
        let q = (m - n) / m;
        let x = k as f64;
        let d1 = special::binomial_pmf_raw(x, kk, p, q);
        let d2 = special::binomial_pmf_raw(n - x, m - kk, p, q);
        let d3 = special::binomial_pmf_raw(n, m, p, q);
        d1 * d2 / d3
    }

    /// Sum of pmf(j) for j <= k (downward from k).
    fn sum_down(&self, k: i64) -> f64 {
        let (lo, _) = self.support();
        let (m, kk, n) = (
            self.population as f64,
            self.successes as f64,
            self.draws as f64,
        );
        let mut term = self.pmf(k);
        let mut sum = term;
        let mut j = k;
        while j > lo {
            let jf = j as f64;
            term *= jf * (m - kk - n + jf) / ((kk - jf + 1.0) * (n - jf + 1.0));
            sum += term;
            if term <= sum * 1e-17 {
                break;
            }
            j -= 1;
        }
        sum
    }

    /// Sum of pmf(j) for j > k (upward from k+1).
    fn sum_up(&self, k: i64) -> f64 {
        let (_, hi) = self.support();
        let (m, kk, n) = (
            self.population as f64,
            self.successes as f64,
            self.draws as f64,
        );
        let mut j = k + 1;
        let mut term = self.pmf(j);
        let mut sum = term;
        while j < hi {
            let jf = j as f64;
            term *= (kk - jf) * (n - jf) / ((jf + 1.0) * (m - kk - n + jf + 1.0));
            sum += term;
            if term <= sum * 1e-17 {
                break;
            }
            j += 1;
        }
        sum
    }

    /// (P(X <= k), P(X > k)).
    pub fn cdf_sf(&self, k: i64) -> (f64, f64) {
        let (lo, hi) = self.support();
        if k < lo {
            return (0.0, 1.0);
        }
        if k >= hi {
            return (1.0, 0.0);
        }
        let mean = self.draws as f64 * self.successes as f64 / self.population as f64;
        if (k as f64) < mean {
            let c = self.sum_down(k).min(1.0);
            (c, 1.0 - c)
        } else {
            let s = self.sum_up(k).min(1.0);
            (1.0 - s, s)
        }
    }
    /// P(X <= k).
    pub fn cdf(&self, k: i64) -> f64 {
        self.cdf_sf(k).0
    }
    /// P(X > k).
    pub fn sf(&self, k: i64) -> f64 {
        self.cdf_sf(k).1
    }
    /// Smallest k with P(X <= k) >= prob.
    pub fn quantile(&self, prob: f64) -> f64 {
        if !check_prob(prob) {
            return f64::NAN;
        }
        let (lo, hi) = self.support();
        discrete_quantile(|k| self.cdf(k), lo, hi, prob)
    }
}

// =======================================================================================
// Integration over the studentising chi variable
// =======================================================================================

/// The studentising scale U = sqrt(chi^2_nu / nu) and adaptive integration against its
/// density f(u) = 2 exp(ln_gamma_prefactor(nu/2, nu u^2/2)) / u (the prefactor's
/// Stirling-corrected form keeps it accurate for large nu).
#[derive(Debug, Clone)]
struct ChiScale {
    nu: f64,
    breaks: Vec<f64>,
}

impl ChiScale {
    fn new(nu: f64) -> Self {
        if nu.is_infinite() {
            return ChiScale { nu, breaks: vec![] };
        }
        let a = 0.5 * nu;
        let q = |p: f64| (2.0 * special::gammainc_inv_pq(a, p, 1.0 - p) / nu).sqrt();
        let qu = |p: f64| (2.0 * special::gammainc_inv_pq(a, 1.0 - p, p) / nu).sqrt();
        let mut breaks = vec![q(1e-18), q(0.02), qu(0.02), qu(1e-18)];
        breaks.dedup_by(|x, y| !(*x > *y));
        ChiScale { nu, breaks }
    }

    fn density(&self, u: f64) -> f64 {
        if u <= 0.0 {
            return 0.0;
        }
        let a = 0.5 * self.nu;
        2.0 * special::ln_gamma_prefactor(a, a * u * u).exp() / u
    }

    /// (E g0(U), E g1(U)): g0 to an estimated absolute error of 1e-10 and g1 (an upper
    /// tail) to an estimated relative error of 1e-8 (QUADPACK's scaled K15 error
    /// estimates, which are conservative here: the achieved errors against nested
    /// adaptive quadrature are below 1e-13 absolute and 1e-10 relative; see the ignored
    /// test `studentized_range_vs_nested_quadrature`).
    fn expect_pair<G: FnMut(f64) -> (f64, f64)>(&self, mut g: G) -> (f64, f64) {
        const ABS_TOL: f64 = 1e-10;
        const REL_TOL: f64 = 1e-8;
        if self.nu.is_infinite() {
            return g(1.0);
        }
        quad::integrate_pair(
            |u| {
                let d = self.density(u);
                if d == 0.0 {
                    return (0.0, 0.0);
                }
                let (a, b) = g(u);
                (a * d, b * d)
            },
            &self.breaks,
            ABS_TOL,
            REL_TOL,
            1e-300,
            400,
        )
    }
}

/// Inner nodes for integrals against phi(z) over [lo, hi]: (z, w phi(z), Phi(z), 1-Phi(z)).
fn normal_nodes(lo: f64, hi: f64, panels: usize, per_panel: usize) -> Vec<[f64; 4]> {
    quad::composite_nodes(lo, hi, panels, per_panel)
        .into_iter()
        .map(|(z, w)| {
            [
                z,
                w * std_normal_pdf(z),
                std_normal_cdf(z),
                std_normal_sf(z),
            ]
        })
        .collect()
}

// =======================================================================================
// Studentized range
// =======================================================================================

/// Studentized range distribution of k means with `df` error degrees of freedom
/// (the distribution behind Tukey's HSD).
///
/// P(Q <= q) = integral_0^inf f(u) R(q u) du with R(w) = k integral phi(z)
/// [Phi(z) - Phi(z - w)]^{k-1} dz, the double integral of Copenhaver & Holland (1988,
/// J. Stat. Comput. Simul. 30:1) and Lund & Lund (1983, AS 190). The inner integral uses a
/// composite Gauss–Legendre rule (z in [-9.5, 14.5], 24 panels x 10 points, Phi(z)
/// precomputed; the range extends upward because the upper-tail integrand peaks near
/// z = w/2); the outer integral over u = s/sigma ~ chi_nu/sqrt(nu) is adaptive
/// Gauss–Kronrod (G7/K15, QUADPACK error control) between the 1e-18 quantiles of u. The
/// upper tail is integrated directly as k integral phi(z) Phi(z-w) sum_j Phi(z)^j
/// D^{k-2-j} dz, so small p-values keep relative accuracy. Accuracy against nested
/// adaptive quadrature: < 1e-13 absolute on the cdf and < 1e-10 relative on the upper tail
/// (k <= 20, df >= 1). `df = inf` uses R(q) directly. Quantiles by Brent's method on the
/// cdf (or the upper tail for p > 1/2).
#[derive(Debug, Clone)]
pub struct StudentizedRange {
    /// Number of groups k >= 2.
    pub k: usize,
    /// Error degrees of freedom (> 0, may be inf).
    pub df: f64,
    chi: ChiScale,
    inner: Vec<[f64; 4]>,
}

impl StudentizedRange {
    /// Studentized range for k >= 2 groups and df > 0 (inf allowed).
    pub fn new(k: usize, df: f64) -> Result<Self> {
        if k < 2 {
            return Err(invalid("StudentizedRange: k must be >= 2"));
        }
        if !(df > 0.0) {
            return Err(invalid("StudentizedRange: df must be > 0"));
        }
        Ok(StudentizedRange {
            k,
            df,
            chi: ChiScale::new(df),
            inner: normal_nodes(-9.5, 14.5, 24, 10),
        })
    }

    /// (R(w), 1 - R(w)) for the range of k standard normals.
    fn range_cdf_sf(&self, w: f64) -> (f64, f64) {
        if w <= 0.0 {
            return (0.0, 1.0);
        }
        let km1 = (self.k - 1) as i32;
        let mut c = 0.0;
        let mut s = 0.0;
        for nd in &self.inner {
            let [z, wphi, a, qz] = *nd;
            let zw = z - w;
            // One erfc per node: Phi(z - w) or its complement, whichever is accurate.
            let (lowc, d) = if zw > 0.0 {
                let up = std_normal_sf(zw);
                (1.0 - up, up - qz)
            } else {
                let lc = std_normal_cdf(zw);
                (lc, a - lc)
            };
            c += wphi * d.powi(km1);
            // sum_{j=0}^{k-2} A^j D^{k-2-j}
            let mut pd = 1.0;
            let mut acc = 1.0;
            for _ in 1..km1 {
                pd *= d;
                acc = pd + a * acc;
            }
            s += wphi * lowc * acc;
        }
        let kf = self.k as f64;
        ((kf * c).clamp(0.0, 1.0), (kf * s).clamp(0.0, 1.0))
    }

    /// (P(Q <= q), P(Q > q)).
    pub fn cdf_sf(&self, q: f64) -> (f64, f64) {
        if q.is_nan() {
            return (f64::NAN, f64::NAN);
        }
        if q <= 0.0 {
            return (0.0, 1.0);
        }
        if q.is_infinite() {
            return (1.0, 0.0);
        }
        let (c, s) = self.chi.expect_pair(|u| self.range_cdf_sf(q * u));
        (c.clamp(0.0, 1.0), s.clamp(0.0, 1.0))
    }

    /// Quantile from p and q = 1 - p.
    pub fn quantile_pq(&self, p: f64, q: f64) -> f64 {
        if !check_prob(p) || !check_prob(q) {
            return f64::NAN;
        }
        if p == 0.0 {
            return 0.0;
        }
        if q == 0.0 {
            return f64::INFINITY;
        }
        let lower = p <= q;
        let f = |x: f64| {
            let (c, s) = self.cdf_sf(x);
            if lower {
                c - p
            } else {
                q - s
            }
        };
        // Start near the normal-range quantile scaled by a t-like inflation.
        let start = (2.0 * special::norm_quantile_pq(0.5 * p.powf(1.0 / self.k as f64), 1.0))
            .abs()
            .max(0.5);
        let start = if start.is_finite() { start } else { 3.0 };
        match roots::expand_bracket(f, start, 1.0, 0.0, 200) {
            Some((a, b)) if a == b => a,
            Some((a, b)) => roots::brent(f, a, b, 0.0, 1e-13, 200).unwrap_or(f64::NAN),
            None => f64::NAN,
        }
    }
}

impl Continuous for StudentizedRange {
    /// Density by central difference of the cdf (rarely needed; Tukey tables use cdf/ppf).
    fn pdf(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        let h = 1e-5 * x.max(1.0);
        let lo = (x - h).max(0.0);
        (self.cdf(x + h) - self.cdf(lo)) / (x + h - lo)
    }
    fn cdf(&self, x: f64) -> f64 {
        self.cdf_sf(x).0
    }
    fn sf(&self, x: f64) -> f64 {
        self.cdf_sf(x).1
    }
    fn ppf(&self, p: f64) -> f64 {
        self.quantile_pq(p, 1.0 - p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.quantile_pq(1.0 - q, q)
    }
}

// =======================================================================================
// Dunnett
// =======================================================================================

/// Joint distribution of Dunnett's many-to-one statistics T_i = (mean_i - mean_0) /
/// (s sqrt(1/n_i + 1/n_0)), i = 1..k, with correlations rho_ij = lambda_i lambda_j,
/// lambda_i = sqrt(n_i / (n_i + n_0)) (Dunnett 1955, JASA 50:1096).
///
/// Conditioning on the control mean (z) and the studentising scale u = s/sigma,
/// P(max |T_i| <= c) = E_u integral phi(z) prod_i [Phi((cu - lambda_i z)/s_i) -
/// Phi((-cu - lambda_i z)/s_i)] dz with s_i = sqrt(1 - lambda_i^2); the one-sided version
/// drops the lower term. Inner integral by composite Gauss–Legendre on z in [-12, 12]
/// (panel width 1.5 min_i s_i, 10 points per panel), outer integral adaptive Gauss–Kronrod
/// over the chi distribution of u (the same deterministic double integral as
/// `fixtures/oracle/multiple_comparisons.json`, not SciPy's randomized QMC). Upper tails are
/// integrated directly as sum_i (1 - a_i) prod_{j<i} a_j. Accuracy against nested adaptive
/// quadrature: ~1e-13 relative.
#[derive(Debug, Clone)]
pub struct Dunnett {
    /// lambda_i = sqrt(n_i/(n_i + n_0)) for each treatment.
    pub lambdas: Vec<f64>,
    /// Error degrees of freedom (> 0, may be inf).
    pub df: f64,
    chi: ChiScale,
    inner: Vec<[f64; 4]>,
}

impl Dunnett {
    /// From explicit lambdas (each in (0, 1)).
    pub fn new(lambdas: Vec<f64>, df: f64) -> Result<Self> {
        if lambdas.is_empty() {
            return Err(invalid("Dunnett: need at least one treatment"));
        }
        if lambdas.iter().any(|&l| !(l > 0.0 && l < 1.0)) {
            return Err(invalid("Dunnett: each lambda must be in (0, 1)"));
        }
        if !(df > 0.0) {
            return Err(invalid("Dunnett: df must be > 0"));
        }
        let min_s = lambdas
            .iter()
            .map(|l| (1.0 - l * l).sqrt())
            .fold(1.0, f64::min);
        let panels = ((24.0 / (1.5 * min_s.min(1.0))).ceil() as usize).clamp(24, 400);
        Ok(Dunnett {
            lambdas,
            df,
            chi: ChiScale::new(df),
            inner: normal_nodes(-12.0, 12.0, panels, 10),
        })
    }

    /// Balanced design: k treatments, each of the same size as the control
    /// (lambda = sqrt(1/2), correlation 1/2).
    pub fn balanced(k: usize, df: f64) -> Result<Self> {
        Self::new(vec![std::f64::consts::FRAC_1_SQRT_2; k], df)
    }

    /// From the control size n0 and the treatment sizes.
    pub fn from_group_sizes(n0: usize, treatments: &[usize], df: f64) -> Result<Self> {
        if n0 == 0 || treatments.contains(&0) {
            return Err(invalid("Dunnett: group sizes must be positive"));
        }
        let l = treatments
            .iter()
            .map(|&n| (n as f64 / (n as f64 + n0 as f64)).sqrt())
            .collect();
        Self::new(l, df)
    }

    /// (P(max_i |T_i| <= c), P(max_i |T_i| > c)) if `two_sided`, else
    /// (P(max_i T_i <= c), P(max_i T_i > c)).
    pub fn cdf_sf(&self, c: f64, two_sided: bool) -> (f64, f64) {
        if c.is_nan() {
            return (f64::NAN, f64::NAN);
        }
        if c.is_infinite() {
            return if c > 0.0 { (1.0, 0.0) } else { (0.0, 1.0) };
        }
        if two_sided && c <= 0.0 {
            return (0.0, 1.0);
        }
        let scales: Vec<(f64, f64)> = self
            .lambdas
            .iter()
            .map(|&l| (l, (1.0 - l * l).sqrt()))
            .collect();
        let (cdf, sf) = self.chi.expect_pair(|u| {
            let cu = c * u;
            let mut ci = 0.0;
            let mut si = 0.0;
            for nd in &self.inner {
                let z = nd[0];
                let wphi = nd[1];
                let mut prod = 1.0;
                let mut tail = 0.0;
                for &(l, s) in &scales {
                    let hi = (cu - l * z) / s;
                    let (ch, sh) = normal_cdf_sf(hi);
                    let (a, one_minus_a) = if two_sided {
                        let lo = (-cu - l * z) / s;
                        let (cl, sl) = normal_cdf_sf(lo);
                        // P(lo < X <= hi) without cancellation on either side.
                        let a = if lo > 0.0 {
                            sl - sh
                        } else if hi < 0.0 {
                            ch - cl
                        } else {
                            1.0 - cl - sh
                        };
                        (a, cl + sh)
                    } else {
                        (ch, sh)
                    };
                    tail += one_minus_a * prod;
                    prod *= a;
                }
                ci += wphi * prod;
                si += wphi * tail;
            }
            (ci, si)
        });
        (cdf.clamp(0.0, 1.0), sf.clamp(0.0, 1.0))
    }

    /// Adjusted p-value P(max |T_i| >= |t|) (two-sided) or P(max T_i >= t) (one-sided).
    pub fn p_value(&self, t: f64, two_sided: bool) -> f64 {
        self.cdf_sf(if two_sided { t.abs() } else { t }, two_sided)
            .1
    }

    /// Critical value c with P(max |T_i| <= c) = p (two-sided) or P(max T_i <= c) = p.
    pub fn quantile(&self, p: f64, two_sided: bool) -> f64 {
        if !(p > 0.0 && p < 1.0) {
            return if p == 1.0 { f64::INFINITY } else { f64::NAN };
        }
        let q = 1.0 - p;
        let f = |x: f64| {
            let (c, s) = self.cdf_sf(x, two_sided);
            if p <= 0.5 {
                c - p
            } else {
                q - s
            }
        };
        let start = special::norm_quantile_upper(0.5 * q / self.lambdas.len() as f64).max(0.5);
        let lower_limit = if two_sided { 0.0 } else { -40.0 };
        match roots::expand_bracket(f, start, 0.5, lower_limit, 200) {
            Some((a, b)) if a == b => a,
            Some((a, b)) => roots::brent(f, a, b, 0.0, 1e-13, 200).unwrap_or(f64::NAN),
            None => f64::NAN,
        }
    }
}

// =======================================================================================
// Noncentral t
// =======================================================================================

/// Noncentral t distribution with `df` degrees of freedom and noncentrality `nc`.
///
/// cdf by Lenth's algorithm AS 243 (1989, Applied Statistics 38:185): a Poisson mixture of
/// incomplete beta functions summed with the recurrences of the paper, plus Phi(-delta),
/// with the reflection F(t; nu, delta) = 1 - F(-t; nu, -delta) for t < 0 (Guenther's
/// treatment of negative t). Wherever that reflection or the final complement would leave
/// a tail below 1e-6 (where AS 243, and SciPy's `nct`, lose relative accuracy: at x = -3,
/// df = 5, nc = 8 SciPy returns 8.0e-20 against the true 1.90e-20), the tail is computed
/// instead by adaptive Gauss–Kronrod quadrature of
/// P(T <= t) = integral_0^inf Phi(t u - delta) f(u) du, u = sqrt(chi^2_nu/nu), whose
/// integrand is positive so relative accuracy is kept. The same quadrature is used for
/// |delta| > 37 or df > 4e5 (outside AS 243's range). The density is the integral
/// integral u phi(t u - delta) f(u) du. Quantiles by Brent's method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoncentralT {
    /// Degrees of freedom.
    pub df: f64,
    /// Noncentrality parameter.
    pub nc: f64,
}

impl NoncentralT {
    /// Noncentral t; df > 0 finite, nc finite.
    pub fn new(df: f64, nc: f64) -> Result<Self> {
        if !(df > 0.0 && df.is_finite() && nc.is_finite()) {
            return Err(invalid("NoncentralT: df must be finite and > 0, nc finite"));
        }
        Ok(NoncentralT { df, nc })
    }

    /// AS 243 for t >= 0 (returns P(T <= t)).
    fn as243_nonneg(&self, t: f64, del: f64) -> f64 {
        let df = self.df;
        let mut tnc = 0.0;
        if t > 0.0 {
            let x = t * t / (t * t + df);
            let lambda = del * del;
            let mut p = 0.5 * (-0.5 * lambda).exp();
            let mut q = (2.0 / std::f64::consts::PI).sqrt() * p * del;
            let mut s = 0.5 - p;
            let mut a = 0.5;
            let b = 0.5 * df;
            // (1-x)^b = (df/(t^2+df))^b
            let ln_rxb = -b * (t * t / df).ln_1p();
            let rxb = ln_rxb.exp();
            let albeta = special::ln_beta(a, b);
            let y = df / (t * t + df);
            let mut xodd = special::betainc_pair_xy(a, b, x, y).0;
            let mut godd = 2.0 * rxb * (a * x.ln() - albeta).exp();
            let mut xeven = -ln_rxb.exp_m1();
            let mut geven = b * x * rxb;
            tnc = p * xodd + q * xeven;
            let mut en = 1.0;
            loop {
                a += 1.0;
                xodd -= godd;
                xeven -= geven;
                godd *= x * (a + b - 1.0) / a;
                geven *= x * (a + b - 0.5) / (a + 0.5);
                p *= lambda / (2.0 * en);
                q *= lambda / (2.0 * en + 1.0);
                s -= p;
                en += 1.0;
                tnc += p * xodd + q * xeven;
                let errbd = 2.0 * s * (xodd - godd);
                if errbd.abs() <= 1e-15 || en > 10000.0 {
                    break;
                }
            }
        }
        tnc + std_normal_cdf(-del)
    }

    /// Lower tail by quadrature: integral Phi(t u - delta) f(u) du (or the upper tail
    /// integral Phi(delta - t u) f(u) du when `upper`).
    fn tail_by_quadrature(&self, t: f64, upper: bool) -> f64 {
        let nu = self.df;
        let a = 0.5 * nu;
        let glo = special::gammainc_inv_pq(a, 1e-30, 1.0);
        let ghi = special::gammainc_inv_pq(a, 1.0, 1e-30);
        let lo = (2.0 * glo / nu).sqrt();
        let hi = (2.0 * ghi / nu).sqrt();
        let del = self.nc;
        let g = |u: f64| {
            if u <= 0.0 {
                return 0.0;
            }
            let dens = 2.0 * special::ln_gamma_prefactor(a, a * u * u).exp() / u;
            let z = t * u - del;
            let pr = if upper {
                std_normal_sf(z)
            } else {
                std_normal_cdf(z)
            };
            pr * dens
        };
        let mode = ((nu - 1.0).max(0.0) / nu).sqrt().clamp(lo, hi);
        let (r1, _) = quad::integrate(g, lo, mode, 1e-14, 0.0, 2000);
        let (r2, _) = quad::integrate(g, mode, hi, 1e-14, 0.0, 2000);
        (r1 + r2).clamp(0.0, 1.0)
    }

    /// (P(T <= t), P(T > t)).
    pub fn cdf_sf(&self, t: f64) -> (f64, f64) {
        if t.is_nan() {
            return (f64::NAN, f64::NAN);
        }
        if t.is_infinite() {
            return if t > 0.0 { (1.0, 0.0) } else { (0.0, 1.0) };
        }
        let use_as243 = self.nc.abs() <= 37.0 && self.df <= 4e5;
        if !use_as243 {
            let c = self.tail_by_quadrature(t, false);
            let s = self.tail_by_quadrature(t, true);
            return (c, s);
        }
        let cdf = if t >= 0.0 {
            self.as243_nonneg(t, self.nc)
        } else {
            1.0 - self.as243_nonneg(-t, -self.nc)
        };
        let cdf = cdf.clamp(0.0, 1.0);
        const TAIL: f64 = 1e-6;
        if cdf < TAIL {
            let c = self.tail_by_quadrature(t, false);
            (c, 1.0 - c)
        } else if cdf > 1.0 - TAIL {
            let s = self.tail_by_quadrature(t, true);
            (1.0 - s, s)
        } else {
            (cdf, 1.0 - cdf)
        }
    }

    /// Quantile from p and q = 1 - p by Brent's method on the cdf (or sf).
    pub fn quantile_pq(&self, p: f64, q: f64) -> f64 {
        if !check_prob(p) || !check_prob(q) {
            return f64::NAN;
        }
        if p == 0.0 {
            return f64::NEG_INFINITY;
        }
        if q == 0.0 {
            return f64::INFINITY;
        }
        let lower = p <= q;
        let f = |x: f64| {
            let (c, s) = self.cdf_sf(x);
            if lower {
                c - p
            } else {
                q - s
            }
        };
        // Start: normal approximation (Abramowitz & Stegun 26.7.10 inverted roughly).
        let z = special::norm_quantile_pq(p, q);
        let start = self.nc + z * (1.0 + self.nc * self.nc / (2.0 * self.df)).sqrt();
        let step = 0.5 * (1.0 + self.nc.abs() / self.df.sqrt());
        match roots::expand_bracket(f, start, step, f64::NEG_INFINITY, 400) {
            Some((a, b)) if a == b => a,
            Some((a, b)) => roots::brent(f, a, b, 1e-300, 1e-14, 300).unwrap_or(f64::NAN),
            None => f64::NAN,
        }
    }
}

impl Continuous for NoncentralT {
    fn pdf(&self, t: f64) -> f64 {
        if t.is_nan() {
            return f64::NAN;
        }
        let nu = self.df;
        let a = 0.5 * nu;
        let glo = special::gammainc_inv_pq(a, 1e-30, 1.0);
        let ghi = special::gammainc_inv_pq(a, 1.0, 1e-30);
        let lo = (2.0 * glo / nu).sqrt();
        let hi = (2.0 * ghi / nu).sqrt();
        let del = self.nc;
        let g = |u: f64| {
            if u <= 0.0 {
                return 0.0;
            }
            let dens = 2.0 * special::ln_gamma_prefactor(a, a * u * u).exp() / u;
            u * std_normal_pdf(t * u - del) * dens
        };
        let mode = ((nu - 1.0).max(0.0) / nu).sqrt().clamp(lo, hi);
        let (r1, _) = quad::integrate(g, lo, mode, 1e-13, 0.0, 2000);
        let (r2, _) = quad::integrate(g, mode, hi, 1e-13, 0.0, 2000);
        r1 + r2
    }
    fn cdf(&self, t: f64) -> f64 {
        self.cdf_sf(t).0
    }
    fn sf(&self, t: f64) -> f64 {
        self.cdf_sf(t).1
    }
    fn ppf(&self, p: f64) -> f64 {
        self.quantile_pq(p, 1.0 - p)
    }
    fn isf(&self, q: f64) -> f64 {
        self.quantile_pq(1.0 - q, q)
    }
}

// =======================================================================================
// p-value helpers
// =======================================================================================

/// Two-sided p-value of a t statistic: P(|T_df| >= |t|).
pub fn p_two_sided_t(t: f64, df: f64) -> f64 {
    StudentT { df }.two_sided_p(t)
}

/// One-sided p-value P(T_df <= t) (alternative "less").
pub fn p_less_t(t: f64, df: f64) -> f64 {
    StudentT { df }.cdf_sf(t).0
}

/// One-sided p-value P(T_df >= t) (alternative "greater").
pub fn p_greater_t(t: f64, df: f64) -> f64 {
    StudentT { df }.cdf_sf(t).1
}

/// Two-sided p-value of a z statistic: 2 (1 - Phi(|z|)).
pub fn p_two_sided_z(z: f64) -> f64 {
    (2.0 * std_normal_sf(z.abs())).min(1.0)
}

/// P(Z <= z).
pub fn p_less_z(z: f64) -> f64 {
    std_normal_cdf(z)
}

/// P(Z >= z).
pub fn p_greater_z(z: f64) -> f64 {
    std_normal_sf(z)
}

/// Upper-tail p-value of a chi-square statistic: P(chi^2_df >= x).
pub fn p_upper_chi2(x: f64, df: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    special::gammainc_q(0.5 * df, 0.5 * x)
}

/// Upper-tail p-value of an F statistic: P(F_{d1,d2} >= f).
pub fn p_upper_f(f: f64, d1: f64, d2: f64) -> f64 {
    FisherF { d1, d2 }.cdf_sf(f).1
}

/// Two-sided critical value t_{1-alpha/2, df} (e.g. for confidence intervals).
pub fn t_critical_two_sided(alpha: f64, df: f64) -> f64 {
    StudentT { df }.quantile_pq(1.0 - 0.5 * alpha, 0.5 * alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mills_ratio_agrees_with_erfc() {
        for &t in &[5.0, 5.5, 6.0, 7.0, 8.0] {
            let a = upper_tail_nonneg(t);
            let b = 0.5 * special::erfc(t / std::f64::consts::SQRT_2);
            assert!(((a - b) / b).abs() < 3e-14, "t={t}: {a} vs {b}");
        }
    }

    #[test]
    fn hypergeometric_sums_to_one() {
        let h = Hypergeometric::new(30, 12, 9).unwrap();
        let s: f64 = (0..=9).map(|k| h.pmf(k)).sum();
        assert!((s - 1.0).abs() < 1e-14);
    }
}
