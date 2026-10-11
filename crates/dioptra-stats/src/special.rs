//! Special functions: gamma and beta families, regularised incomplete beta and gamma
//! functions with their inverses, the error function family and the normal quantile.
//!
//! Every p-value in Dioptra goes through this module, so each function is written to keep
//! *relative* accuracy in the tails: the upper and lower tails of the incomplete beta and
//! gamma functions are computed directly (never as `1 - big`), and the exponential
//! prefactors are evaluated in log space with the Stirling-series correction so that large
//! parameters (a = b = 5000, a = 1e5) keep their digits.
//!
//! Conventions: invalid arguments return `NaN` (these are low-level kernels; the
//! distribution constructors in [`crate::dist`] validate parameters and return
//! [`crate::StatsError`]).

use crate::roots::{halley_bracketed, HalleyEval};

/// ln(sqrt(2 pi)).
pub const LN_SQRT_2PI: f64 = 0.9189385332046728;
/// sqrt(2 pi).
pub const SQRT_2PI: f64 = 2.5066282746310007;
/// ln(sqrt(pi)) = ln Gamma(1/2).
pub const LN_SQRT_PI: f64 = 0.5723649429247001;
/// 1/sqrt(2 pi).
pub const INV_SQRT_2PI: f64 = 0.3989422804014327;

const EPS: f64 = f64::EPSILON;
const FPMIN: f64 = 1e-300;

// ---------------------------------------------------------------------------------------
// Elementary wrappers
// ---------------------------------------------------------------------------------------

/// Natural log of |Gamma(x)|. Reference: fdlibm `lgamma_r` (Sun Microsystems), via the
/// `libm` crate; about 1 ulp.
pub fn ln_gamma(x: f64) -> f64 {
    libm::lgamma_r(x).0
}

/// Gamma(x). Reference: `libm::tgamma` (musl, Lanczos-type approximation).
pub fn gamma(x: f64) -> f64 {
    libm::tgamma(x)
}

/// Error function. Reference: fdlibm `erf` (rational approximations), via `libm`.
pub fn erf(x: f64) -> f64 {
    libm::erf(x)
}

/// Complementary error function, accurate in relative terms in the upper tail.
/// Reference: fdlibm `erfc`, via `libm`.
pub fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

/// ln(1 + x), accurate for small x (std, IEEE `log1p`).
pub fn log1p(x: f64) -> f64 {
    x.ln_1p()
}

/// exp(x) - 1, accurate for small x (std, IEEE `expm1`).
pub fn expm1(x: f64) -> f64 {
    x.exp_m1()
}

/// ln(1 + x) - x without cancellation for small |x|.
///
/// Reference: the series in r = x/(2+x) used by R's `log1pmx` (Morten Welinder, R
/// `pgamma.c`): ln(1+x) - x = r (2 sum_{k>=1} r^{2k}/(2k+1) - x).
pub fn log1pmx(x: f64) -> f64 {
    if !(x > -1.0) {
        return if x == -1.0 {
            f64::NEG_INFINITY
        } else {
            f64::NAN
        };
    }
    if x.abs() < 0.5 {
        let r = x / (2.0 + x);
        let r2 = r * r;
        let mut term = r2;
        let mut sum = 0.0;
        let mut k = 3.0;
        loop {
            let add = term / k;
            sum += add;
            if add.abs() <= sum.abs() * 1e-17 {
                break;
            }
            term *= r2;
            k += 2.0;
            if k > 400.0 {
                break;
            }
        }
        r * (2.0 * sum - x)
    } else {
        x.ln_1p() - x
    }
}

/// The Stirling-series error ln Gamma(z) - [(z - 1/2) ln z - z + ln sqrt(2 pi)].
///
/// For z >= 8 the asymptotic series with Bernoulli numbers B_2..B_18 (absolute error below
/// 1e-17); for smaller z the exact upward recurrence
/// s(z) = s(z+1) + (z + 1/2) ln(1 + 1/z) - 1. Reference: Loader (2000), "Fast and accurate
/// computation of binomial probabilities" (R's `stirlerr`); Abramowitz & Stegun 6.1.40.
pub fn stirling_error(z: f64) -> f64 {
    if !(z > 0.0) {
        return f64::NAN;
    }
    if z >= 8.0 {
        let iz = 1.0 / z;
        let iz2 = iz * iz;
        // B_{2k} / (2k (2k-1))
        const C: [f64; 9] = [
            1.0 / 12.0,
            -1.0 / 360.0,
            1.0 / 1260.0,
            -1.0 / 1680.0,
            1.0 / 1188.0,
            -691.0 / 360360.0,
            1.0 / 156.0,
            -3617.0 / 122400.0,
            43867.0 / 244188.0,
        ];
        let mut s = 0.0;
        for c in C.iter().rev() {
            s = s * iz2 + c;
        }
        s * iz
    } else {
        let mut acc = 0.0;
        let mut zz = z;
        while zz < 8.0 {
            acc += (zz + 0.5) * (1.0 / zz).ln_1p() - 1.0;
            zz += 1.0;
        }
        acc + stirling_error(zz)
    }
}

/// Digamma psi(x) = d/dx ln Gamma(x).
///
/// Upward recurrence to x >= 10, then the asymptotic expansion ln x - 1/(2x) -
/// sum B_2k/(2k x^2k); reflection psi(1-x) - psi(x) = pi cot(pi x) for x < 0.
/// Reference: Abramowitz & Stegun 6.3.5, 6.3.7, 6.3.18.
pub fn digamma(x: f64) -> f64 {
    if x.is_nan() || x == f64::NEG_INFINITY {
        return f64::NAN;
    }
    if x <= 0.0 {
        if x == x.floor() {
            return f64::NAN;
        }
        let pi = std::f64::consts::PI;
        return digamma(1.0 - x) - pi / (pi * x).tan();
    }
    let mut acc = 0.0;
    let mut z = x;
    while z < 10.0 {
        acc -= 1.0 / z;
        z += 1.0;
    }
    let iz2 = 1.0 / (z * z);
    // B_2k/(2k)
    let series = iz2
        * (1.0 / 12.0
            - iz2
                * (1.0 / 120.0
                    - iz2
                        * (1.0 / 252.0
                            - iz2
                                * (1.0 / 240.0
                                    - iz2
                                        * (1.0 / 132.0 - iz2 * (691.0 / 32760.0 - iz2 / 12.0))))));
    acc + z.ln() - 0.5 / z - series
}

/// ln B(a, b) = ln Gamma(a) + ln Gamma(b) - ln Gamma(a+b), without cancellation for large
/// arguments.
///
/// Reference: R's `lbeta` (Ross Ihaka): the Stirling-correction form when the larger
/// argument is >= 8, plain log-gammas otherwise.
pub fn ln_beta(a: f64, b: f64) -> f64 {
    if !(a > 0.0 && b > 0.0) {
        return f64::NAN;
    }
    let p = a.min(b);
    let q = a.max(b);
    if q.is_infinite() {
        return f64::NEG_INFINITY;
    }
    if p >= 8.0 {
        let corr = stirling_error(p) + stirling_error(q) - stirling_error(p + q);
        -0.5 * q.ln()
            + LN_SQRT_2PI
            + corr
            + (p - 0.5) * (p / (p + q)).ln()
            + q * (-p / (p + q)).ln_1p()
    } else if q >= 8.0 {
        let corr = stirling_error(q) - stirling_error(p + q);
        ln_gamma(p) + corr + p - p * (p + q).ln() + (q - 0.5) * (-p / (p + q)).ln_1p()
    } else {
        ln_gamma(p) + ln_gamma(q) - ln_gamma(p + q)
    }
}

/// B(a, b) = Gamma(a) Gamma(b) / Gamma(a+b), computed as exp(ln_beta).
pub fn beta(a: f64, b: f64) -> f64 {
    ln_beta(a, b).exp()
}

/// ln C(n, k) for real n >= k >= 0, via [`ln_beta`]: ln C(n,k) = -ln(n+1) - ln B(n-k+1, k+1).
pub fn ln_choose(n: f64, k: f64) -> f64 {
    if !(n >= 0.0 && k >= 0.0 && k <= n) {
        return f64::NAN;
    }
    if k == 0.0 || k == n {
        return 0.0;
    }
    -(n + 1.0).ln() - ln_beta(n - k + 1.0, k + 1.0)
}

/// The binomial deviance term bd0(x, np) = x ln(x/np) + np - x, computed without
/// cancellation when x is close to np.
///
/// Reference: Loader (2000), "Fast and accurate computation of binomial probabilities",
/// as in R's `bd0`.
pub fn binomial_deviance(x: f64, np: f64) -> f64 {
    if !(np > 0.0) || !(x >= 0.0) {
        return f64::NAN;
    }
    if x == 0.0 {
        return np;
    }
    if (x - np).abs() < 0.1 * (x + np) {
        let mut v = (x - np) / (x + np);
        let mut s = (x - np) * v;
        if s.abs() < f64::MIN_POSITIVE {
            return s;
        }
        let mut ej = 2.0 * x * v;
        v *= v;
        for j in 1..1000 {
            ej *= v;
            let s1 = s + ej / (2 * j + 1) as f64;
            if s1 == s {
                return s1;
            }
            s = s1;
        }
        s
    } else {
        x * (x / np).ln() + np - x
    }
}

/// Binomial probability mass b(x; n, p) with q = 1 - p supplied separately, by Loader's
/// saddle-point expansion (accurate to a few ulps everywhere; no lgamma cancellation).
///
/// Reference: Loader (2000); R's `dbinom_raw`.
pub fn binomial_pmf_raw(x: f64, n: f64, p: f64, q: f64) -> f64 {
    if p == 0.0 {
        return if x == 0.0 { 1.0 } else { 0.0 };
    }
    if q == 0.0 {
        return if x == n { 1.0 } else { 0.0 };
    }
    if x < 0.0 || x > n {
        return 0.0;
    }
    if x == 0.0 {
        if n == 0.0 {
            return 1.0;
        }
        let lc = if p < 0.1 {
            -binomial_deviance(n, n * q) - n * p
        } else {
            n * q.ln()
        };
        return lc.exp();
    }
    if x == n {
        let lc = if q < 0.1 {
            -binomial_deviance(n, n * p) - n * q
        } else {
            n * p.ln()
        };
        return lc.exp();
    }
    let lc = stirling_error(n)
        - stirling_error(x)
        - stirling_error(n - x)
        - binomial_deviance(x, n * p)
        - binomial_deviance(n - x, n * q);
    let lf = 2.0 * LN_SQRT_2PI + x.ln() + (-x / n).ln_1p();
    (lc - 0.5 * lf).exp()
}

// ---------------------------------------------------------------------------------------
// Regularised incomplete gamma
// ---------------------------------------------------------------------------------------

/// ln( x^a e^{-x} / Gamma(a) ), the prefactor of the incomplete gamma function, computed
/// with ln(1+t) - t and the Stirling error for a >= 8 so that a = 1e5 keeps full accuracy.
/// Reference: Temme (1987), DiDonato & Morris (1986) `rcomp`.
pub fn ln_gamma_prefactor(a: f64, x: f64) -> f64 {
    if a >= 8.0 {
        let t = (x - a) / a;
        // ln(1+t) - t; when x is far from a, 1 + t = x/a must not be formed from t.
        let l = if t.abs() < 0.5 {
            log1pmx(t)
        } else {
            (x / a).ln() - t
        };
        a * l + 0.5 * a.ln() - LN_SQRT_2PI - stirling_error(a)
    } else {
        a * x.ln() - x - ln_gamma(a)
    }
}

/// Regularised lower and upper incomplete gamma functions (P(a,x), Q(a,x)), with the
/// smaller of the two computed directly.
///
/// Power series for x < a + 1, Lentz's continued fraction for Q otherwise
/// (Numerical Recipes 3rd ed. §6.2; DiDonato & Morris 1986, TOMS 654), with the
/// log-space prefactor [`ln_gamma_prefactor`].
pub fn gammainc_pair(a: f64, x: f64) -> (f64, f64) {
    if !(a > 0.0) || x.is_nan() || x < 0.0 {
        return (f64::NAN, f64::NAN);
    }
    if x == 0.0 {
        return (0.0, 1.0);
    }
    if x.is_infinite() {
        return (1.0, 0.0);
    }
    if a.is_infinite() {
        return (0.0, 1.0);
    }
    let lp = ln_gamma_prefactor(a, x);
    if x < a + 1.0 {
        let mut ap = a;
        let mut term = 1.0 / a;
        let mut sum = term;
        for _ in 0..1_000_000 {
            ap += 1.0;
            term *= x / ap;
            sum += term;
            if term <= sum * EPS * 0.25 {
                break;
            }
        }
        let p = (lp + sum.ln()).exp();
        let p = p.min(1.0);
        (p, 1.0 - p)
    } else {
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / FPMIN;
        let mut d = 1.0 / b;
        let mut h = d;
        let mut i = 1.0;
        loop {
            let an = -i * (i - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < FPMIN {
                d = FPMIN;
            }
            c = b + an / c;
            if c.abs() < FPMIN {
                c = FPMIN;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() <= EPS || i > 1e6 {
                break;
            }
            i += 1.0;
        }
        let q = (lp + h.ln()).exp();
        let q = q.min(1.0);
        (1.0 - q, q)
    }
}

/// Regularised lower incomplete gamma P(a, x) = gamma(a, x)/Gamma(a). See [`gammainc_pair`].
pub fn gammainc_p(a: f64, x: f64) -> f64 {
    gammainc_pair(a, x).0
}

/// Regularised upper incomplete gamma Q(a, x) = 1 - P(a, x), computed directly.
/// See [`gammainc_pair`].
pub fn gammainc_q(a: f64, x: f64) -> f64 {
    gammainc_pair(a, x).1
}

/// Density of the standard gamma distribution x^{a-1} e^{-x}/Gamma(a), in log space.
fn ln_gamma_density(a: f64, x: f64) -> f64 {
    ln_gamma_prefactor(a, x) - x.ln()
}

/// Inverse of the regularised incomplete gamma: x with P(a, x) = p, given both p and its
/// complement q = 1 - p (the smaller is used as the target so tails keep relative
/// accuracy).
///
/// Starting value: the small-x series x = (p Gamma(a+1))^{1/a}, the Wilson–Hilferty cube
/// (Wilson & Hilferty 1931), or the large-x asymptote of Q; refined by bracketed Halley
/// iteration (DiDonato & Morris 1986, TOMS 654, §5).
pub fn gammainc_inv_pq(a: f64, p: f64, q: f64) -> f64 {
    if !(a > 0.0) || !(0.0..=1.0).contains(&p) || !(0.0..=1.0).contains(&q) {
        return f64::NAN;
    }
    if p == 0.0 {
        return 0.0;
    }
    if q == 0.0 {
        return f64::INFINITY;
    }
    let lower = p <= q;
    // Initial guess.
    let lg1 = ln_gamma(a + 1.0);
    let x_small = ((p.ln() + lg1) / a).exp();
    let z = norm_quantile_pq(p, q);
    let s = 1.0 / (9.0 * a);
    let wh_base = 1.0 - s + z * s.sqrt();
    let x_wh = if wh_base > 0.0 {
        a * wh_base * wh_base * wh_base
    } else {
        f64::NAN
    };
    let mut x0 = if x_small < 0.3 * (a + 1.0) && lower {
        x_small
    } else if x_wh.is_finite() && x_wh > 0.0 {
        x_wh
    } else {
        x_small
    };
    if !lower && (!x0.is_finite() || x0 < a) {
        // Large-x asymptote: Q ~ x^{a-1} e^{-x} / Gamma(a).
        let lga = ln_gamma(a);
        let mut x = a.max(1.0);
        for _ in 0..5 {
            x = (-(q.ln()) - lga + (a - 1.0) * x.ln()).max(a.max(1e-3));
        }
        if x.is_finite() {
            x0 = x0.max(x);
        }
    }
    if !(x0.is_finite() && x0 > 0.0) {
        x0 = a.max(1e-3);
    }
    halley_bracketed(
        |x| {
            let (pp, qq) = gammainc_pair(a, x);
            let f = if lower { pp - p } else { q - qq };
            let dens = ln_gamma_density(a, x).exp();
            HalleyEval {
                f,
                df: dens,
                d2_over_d1: (a - 1.0) / x - 1.0,
            }
        },
        x0,
        0.0,
        f64::INFINITY,
        400,
    )
}

/// Inverse of P(a, x): returns x with P(a, x) = p. See [`gammainc_inv_pq`].
pub fn gammainc_inv(a: f64, p: f64) -> f64 {
    gammainc_inv_pq(a, p, 1.0 - p)
}

/// Inverse of Q(a, x): returns x with Q(a, x) = q, accurate for tiny q. See
/// [`gammainc_inv_pq`].
pub fn gammaincc_inv(a: f64, q: f64) -> f64 {
    gammainc_inv_pq(a, 1.0 - q, q)
}

// ---------------------------------------------------------------------------------------
// Regularised incomplete beta
// ---------------------------------------------------------------------------------------

/// ln( x^a y^b / B(a, b) ) with y = 1 - x supplied separately.
///
/// For a, b >= 8 the expansion around the mean x0 = a/(a+b):
/// x^a y^b / B(a,b) = sqrt(ab/(2 pi (a+b))) exp(a lnp1mx(u) + b lnp1mx(v) - Delta),
/// u = (x b - y a)/a, v = -a u/b, Delta the Stirling corrections; this is free of the
/// cancellation that ruins `a ln x + b ln y - ln B(a,b)` when a = b = 5000.
/// Reference: DiDonato & Morris (1992), TOMS 708 `brcomp`; Loader (2000).
pub fn ln_beta_prefactor(a: f64, b: f64, x: f64, y: f64) -> f64 {
    if a >= 8.0 && b >= 8.0 {
        let d = x * b - y * a;
        let u = d / a;
        let v = -d / b;
        // ln(1+u) - u with 1 + u = x (a+b)/a; far from the mean, use the logs directly.
        let lu = if u.abs() < 0.5 {
            log1pmx(u)
        } else {
            x.ln() + (b / a).ln_1p() - u
        };
        let lv = if v.abs() < 0.5 {
            log1pmx(v)
        } else {
            y.ln() + (a / b).ln_1p() - v
        };
        let e = a * lu + b * lv;
        let corr = stirling_error(a) + stirling_error(b) - stirling_error(a + b);
        e + 0.5 * (a * (b / (a + b))).ln() - LN_SQRT_2PI - corr
    } else {
        let lx = if x < 0.5 { x.ln() } else { (-y).ln_1p() };
        let ly = if y < 0.5 { y.ln() } else { (-x).ln_1p() };
        a * lx + b * ly - ln_beta(a, b)
    }
}

/// Continued fraction for I_x(a,b) (modified Lentz), Numerical Recipes 3rd ed. §6.4.
fn betacf(a: f64, b: f64, x: f64, y: f64) -> f64 {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    // 1 - (a+b)x/(a+1), written with y = 1 - x when x is close to 1 to avoid cancellation.
    let mut d = if x > 0.5 {
        (1.0 - b + qab * y) / qap
    } else {
        1.0 - qab * x / qap
    };
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    let mut m = 1.0;
    while m < 1e6 {
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() <= EPS {
            break;
        }
        m += 1.0;
    }
    h
}

/// Power series for I_x(a,b) when b x is small and x <= 1/2 (TOMS 708 `bpser`):
/// I = x^a y^b/(a B) * 2F1(a+b, 1; a+1; x) written as a positive series. Used for
/// tiny a, where the continued fraction's complement would lose digits.
fn beta_series_lower(a: f64, b: f64, x: f64, y: f64) -> f64 {
    // I_x(a,b) = x^a y^b / (a B(a,b)) * sum_{n>=0} (a+b)_n / (a+1)_n x^n
    let lp = ln_beta_prefactor(a, b, x, y);
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut n = 0.0;
    while n < 1e6 {
        term *= (a + b + n) / (a + 1.0 + n) * x;
        sum += term;
        if term <= sum * EPS * 0.25 {
            break;
        }
        n += 1.0;
    }
    (lp + sum.ln() - a.ln()).exp()
}

/// Regularised incomplete beta I_x(a, b) and its complement 1 - I_x(a, b), with
/// y = 1 - x supplied by the caller (so that, e.g., the t distribution can pass
/// x = nu/(nu+t^2) and y = t^2/(nu+t^2) without rounding).
///
/// Algorithm: the continued fraction of Numerical Recipes §6.4 (modified Lentz) on whichever
/// side of the switch point x = (a+1)/(a+b+2) converges, the symmetry
/// I_x(a,b) = 1 - I_y(b,a), and the log-space prefactor [`ln_beta_prefactor`]
/// (TOMS 708 `brcomp` ideas). The tail on the convergent side is computed directly; the
/// other value is its complement, which is the larger of the two except near the median.
pub fn betainc_pair_xy(a: f64, b: f64, x: f64, y: f64) -> (f64, f64) {
    if !(a > 0.0 && b > 0.0) || !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
        return (f64::NAN, f64::NAN);
    }
    if x == 0.0 {
        return (0.0, 1.0);
    }
    if y == 0.0 {
        return (1.0, 0.0);
    }
    if x < (a + 1.0) / (a + b + 2.0) {
        let w = if a < 1.0 && b * x <= 1.0 && x <= 0.5 {
            beta_series_lower(a, b, x, y)
        } else {
            (ln_beta_prefactor(a, b, x, y) + betacf(a, b, x, y).ln() - a.ln()).exp()
        };
        let w = w.min(1.0);
        (w, 1.0 - w)
    } else {
        let w = if b < 1.0 && a * y <= 1.0 && y <= 0.5 {
            beta_series_lower(b, a, y, x)
        } else {
            (ln_beta_prefactor(b, a, y, x) + betacf(b, a, y, x).ln() - b.ln()).exp()
        };
        let w = w.min(1.0);
        (1.0 - w, w)
    }
}

/// (I_x(a,b), 1 - I_x(a,b)). See [`betainc_pair_xy`].
pub fn betainc_pair(a: f64, b: f64, x: f64) -> (f64, f64) {
    betainc_pair_xy(a, b, x, 1.0 - x)
}

/// Regularised incomplete beta I_x(a, b). See [`betainc_pair_xy`].
pub fn betainc(a: f64, b: f64, x: f64) -> f64 {
    betainc_pair(a, b, x).0
}

/// Complement 1 - I_x(a, b) = I_{1-x}(b, a), computed directly. See [`betainc_pair_xy`].
pub fn betaincc(a: f64, b: f64, x: f64) -> f64 {
    betainc_pair(a, b, x).1
}

/// Inverse of the regularised incomplete beta: (x, y = 1 - x) with I_x(a,b) = p, given p
/// and q = 1 - p. Whichever of x and y is smaller is solved for directly, so solutions
/// near 0 or 1 keep relative accuracy in the small variable.
///
/// Starting value from Numerical Recipes 3rd ed. §6.14 (`invbetai`, after Abramowitz &
/// Stegun 26.5.22 and the small-x power approximations), refined by bracketed Halley
/// iteration (as Boost's `ibeta_inv` and TOMS 708 users do).
pub fn betainc_inv_pq(a: f64, b: f64, p: f64, q: f64) -> (f64, f64) {
    if !(a > 0.0 && b > 0.0) || !(0.0..=1.0).contains(&p) || !(0.0..=1.0).contains(&q) {
        return (f64::NAN, f64::NAN);
    }
    if p == 0.0 {
        return (0.0, 1.0);
    }
    if q == 0.0 {
        return (1.0, 0.0);
    }
    let x0 = betainc_inv_guess(a, b, p, q);
    if x0 > 0.5 {
        let (y, x) = betainc_inv_solve(b, a, q, p, 1.0 - x0);
        (x, y)
    } else {
        betainc_inv_solve(a, b, p, q, x0)
    }
}

fn betainc_inv_guess(a: f64, b: f64, p: f64, q: f64) -> f64 {
    let lnb = ln_beta(a, b);
    // Tail approximations: I ~ x^a/(a B) near 0, 1 - I ~ y^b/(b B) near 1.
    let x_lo = ((p.ln() + a.ln() + lnb) / a).exp();
    let y_hi = ((q.ln() + b.ln() + lnb) / b).exp();
    let mean = a / (a + b);
    let mut x;
    if a >= 1.0 && b >= 1.0 {
        let pp = p.min(q);
        let t = (-2.0 * pp.ln()).sqrt();
        let mut z = (2.30753 + t * 0.27061) / (1.0 + t * (0.99229 + t * 0.04481)) - t;
        if p < q {
            z = -z;
        }
        let al = (z * z - 3.0) / 6.0;
        let h = 2.0 / (1.0 / (2.0 * a - 1.0) + 1.0 / (2.0 * b - 1.0));
        let w = z * (al + h).sqrt() / h
            - (1.0 / (2.0 * b - 1.0) - 1.0 / (2.0 * a - 1.0)) * (al + 5.0 / 6.0 - 2.0 / (3.0 * h));
        x = a / (a + b * (2.0 * w).exp());
    } else {
        let lna = (a / (a + b)).ln();
        let lnbb = (b / (a + b)).ln();
        let t = (a * lna).exp() / a;
        let u = (b * lnbb).exp() / b;
        let w = t + u;
        if p < t / w {
            x = (a * w * p).powf(1.0 / a);
        } else {
            x = 1.0 - (b * w * q).powf(1.0 / b);
        }
    }
    if x_lo < 0.1 * mean && p < q {
        x = x_lo;
    } else if y_hi < 0.1 * (1.0 - mean) && q < p {
        x = 1.0 - y_hi;
    }
    if !(x.is_finite() && x > 0.0 && x < 1.0) {
        x = if p < q {
            x_lo.clamp(1e-300, 0.5)
        } else {
            (1.0 - y_hi).clamp(0.5, 1.0 - 1e-16)
        };
    }
    x
}

/// Solve I_x(a,b) = p (complement q) for x in (0, 1); returns (x, 1-x).
fn betainc_inv_solve(a: f64, b: f64, p: f64, q: f64, x0: f64) -> (f64, f64) {
    let lower = p <= q;
    let x = halley_bracketed(
        |x| {
            let y = 1.0 - x;
            let (i, ic) = betainc_pair_xy(a, b, x, y);
            let f = if lower { i - p } else { q - ic };
            let dens = (ln_beta_prefactor(a, b, x, y) - x.ln() - y.ln()).exp();
            HalleyEval {
                f,
                df: dens,
                d2_over_d1: (a - 1.0) / x - (b - 1.0) / y,
            }
        },
        x0,
        0.0,
        1.0,
        400,
    );
    (x, 1.0 - x)
}

/// Inverse of I_x(a,b): x with I_x(a,b) = p. See [`betainc_inv_pq`].
pub fn betainc_inv(a: f64, b: f64, p: f64) -> f64 {
    betainc_inv_pq(a, b, p, 1.0 - p).0
}

/// Inverse of the complement: x with 1 - I_x(a,b) = q, accurate for tiny q.
/// See [`betainc_inv_pq`].
pub fn betaincc_inv(a: f64, b: f64, q: f64) -> f64 {
    betainc_inv_pq(a, b, 1.0 - q, q).0
}

// ---------------------------------------------------------------------------------------
// Normal quantile and inverse error functions
// ---------------------------------------------------------------------------------------

/// AS 241 PPND16 central rational function for |q| = |p - 1/2| <= 0.425.
fn ppnd16_central(q: f64) -> f64 {
    let r = 0.180625 - q * q;
    q * (((((((r * 2509.0809287301227 + 33430.57558358813) * r + 67265.7709270087) * r
        + 45921.95393154987)
        * r
        + 13731.69376550946)
        * r
        + 1971.5909503065513)
        * r
        + 133.14166789178438)
        * r
        + 3.3871328727963665)
        / (((((((r * 5226.495278852546 + 28729.085735721943) * r + 39307.89580009271) * r
            + 21213.794301586597)
            * r
            + 5394.196021424751)
            * r
            + 687.1870074920579)
            * r
            + 42.31333070160091)
            * r
            + 1.0)
}

/// Wichura's PPND16 core: the standard normal quantile for lower-tail probability `p`
/// (when `upper` is false) or upper-tail probability `p` (when true).
fn ppnd16(p: f64, upper: bool) -> f64 {
    if upper {
        // Phi^{-1}(1 - q) = -Phi^{-1}(q): the symmetry keeps tiny upper tails exact.
        return -ppnd16(p, false);
    }
    let q = p - 0.5;
    if q.abs() <= 0.425 {
        return ppnd16_central(q);
    }
    let lower_tail = q < 0.0;
    let tail = if lower_tail { p } else { 1.0 - p };
    let mut r = (-tail.ln()).sqrt();
    let val = if r <= 5.0 {
        r -= 1.6;
        (((((((r * 0.0007745450142783414 + 0.022723844989269184) * r + 0.2417807251774506) * r
            + 1.2704582524523684)
            * r
            + 3.6478483247632045)
            * r
            + 5.769497221460691)
            * r
            + 4.630337846156545)
            * r
            + 1.4234371107496835)
            / (((((((r * 1.0507500716444169e-9 + 0.0005475938084995345) * r
                + 0.015198666563616457)
                * r
                + 0.14810397642748008)
                * r
                + 0.6897673349851)
                * r
                + 1.6763848301838038)
                * r
                + 2.053191626637759)
                * r
                + 1.0)
    } else {
        r -= 5.0;
        (((((((r * 2.010334399292288e-7 + 2.7115555687434876e-5) * r + 0.0012426609473880784)
            * r
            + 0.026532189526576124)
            * r
            + 0.2965605718285049)
            * r
            + 1.7848265399172913)
            * r
            + 5.463784911164114)
            * r
            + 6.657904643501103)
            / (((((((r * 2.0442631033899397e-15 + 1.421511758316446e-7) * r
                + 1.8463183175100548e-5)
                * r
                + 0.0007868691311456133)
                * r
                + 0.014875361290850615)
                * r
                + 0.1369298809227358)
                * r
                + 0.599832206555888)
                * r
                + 1.0)
    };
    if lower_tail {
        -val
    } else {
        val
    }
}

/// Standard normal quantile Phi^{-1}(p).
///
/// Reference: Wichura (1988), Algorithm AS 241 `PPND16`, Applied Statistics 37:477
/// (about 1e-16 relative), the algorithm of R's `qnorm`.
pub fn norm_quantile(p: f64) -> f64 {
    if p.is_nan() || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    ppnd16(p, false)
}

/// Standard normal upper quantile: z with 1 - Phi(z) = q, accurate for tiny q
/// (AS 241 applied to the upper tail directly).
pub fn norm_quantile_upper(q: f64) -> f64 {
    if q.is_nan() || !(0.0..=1.0).contains(&q) {
        return f64::NAN;
    }
    if q == 0.0 {
        return f64::INFINITY;
    }
    if q == 1.0 {
        return f64::NEG_INFINITY;
    }
    ppnd16(q, true)
}

/// Normal quantile from whichever of p or q = 1 - p is smaller (keeps tail accuracy).
pub fn norm_quantile_pq(p: f64, q: f64) -> f64 {
    if p <= q {
        norm_quantile(p)
    } else {
        norm_quantile_upper(q)
    }
}

/// Wichura's single-precision PPND7 (AS 241, about 1e-7 relative). Royston's AS R94
/// Shapiro–Wilk was published with it; [`crate::normality::shapiro_wilk`] uses the
/// full-precision PPND16 instead, which reproduces SciPy's W to about 1e-13.
pub fn norm_quantile_ppnd7(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        return norm_quantile(p);
    }
    let q = p - 0.5;
    if q.abs() <= 0.425 {
        let r = 0.180625 - q * q;
        return q * (((59.10937472 * r + 159.29113202) * r + 50.434271938) * r + 3.3871327179)
            / (((67.1875636 * r + 78.757757664) * r + 17.895169469) * r + 1.0);
    }
    let mut r = if q < 0.0 { p } else { 1.0 - p };
    r = (-r.ln()).sqrt();
    let val = if r <= 5.0 {
        r -= 1.6;
        (((0.17023821103 * r + 1.3067284816) * r + 2.75681539) * r + 1.4234372777)
            / ((0.12021132975 * r + 0.7370016425) * r + 1.0)
    } else {
        r -= 5.0;
        (((0.017337203997 * r + 0.42868294337) * r + 3.081226386) * r + 6.657905115)
            / ((0.012258202635 * r + 0.24197894225) * r + 1.0)
    };
    if q < 0.0 {
        -val
    } else {
        val
    }
}

/// Inverse complementary error function: erfc(erfc_inv(y)) = y for 0 < y < 2.
/// Computed as -Phi^{-1}(y/2)/sqrt(2) via AS 241 (Wichura 1988).
pub fn erfc_inv(y: f64) -> f64 {
    if !(0.0..=2.0).contains(&y) {
        return f64::NAN;
    }
    if y <= 1.0 {
        norm_quantile_upper(0.5 * y) * std::f64::consts::FRAC_1_SQRT_2
    } else {
        -norm_quantile_upper(0.5 * (2.0 - y)) * std::f64::consts::FRAC_1_SQRT_2
    }
}

/// Inverse error function: erf(erf_inv(x)) = x for -1 < x < 1.
/// Computed as Phi^{-1}((1+x)/2)/sqrt(2) via AS 241.
pub fn erf_inv(x: f64) -> f64 {
    if !(-1.0..=1.0).contains(&x) {
        return f64::NAN;
    }
    if x.abs() <= 0.85 {
        return ppnd16_central(0.5 * x) * std::f64::consts::FRAC_1_SQRT_2;
    }
    if x >= 0.0 {
        erfc_inv(1.0 - x)
    } else {
        -erfc_inv(1.0 + x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stirling_error_continuity() {
        // Both branches agree at the switch point.
        let below = {
            let z = 7.999999999;
            ln_gamma(z) - ((z - 0.5) * z.ln() - z + LN_SQRT_2PI)
        };
        assert!((stirling_error(7.999999999) - below).abs() < 1e-14);
        assert!((stirling_error(1.0) - (1.0 - LN_SQRT_2PI)).abs() < 1e-15);
    }

    #[test]
    fn ppnd7_close_to_ppnd16() {
        let mut worst: f64 = 0.0;
        for i in 1..1000 {
            let p = i as f64 / 1000.0;
            let a = norm_quantile(p);
            let b = norm_quantile_ppnd7(p);
            worst = worst.max((a - b).abs() / a.abs().max(1e-3));
        }
        assert!(worst < 2e-7, "ppnd7 deviates {worst}");
    }

    #[test]
    fn erf_inv_roundtrip() {
        for &x in &[-0.999, -0.5, -1e-9, 0.0, 1e-9, 0.3, 0.9, 0.999999] {
            let y = erf_inv(x);
            assert!(
                (erf(y) - x).abs() <= 1e-15 * x.abs().max(1e-300) + 1e-300,
                "x={x}"
            );
        }
    }

    #[test]
    fn log1pmx_matches_taylor_and_direct() {
        for &x in &[-0.4_f64, -1e-3, 1e-8, 0.2, 0.49] {
            // Alternating Taylor series sum_{k>=2} (-1)^{k+1} x^k / k, summed to convergence.
            let mut s = 0.0;
            let mut p = x * x;
            for k in 2..400 {
                let t = p / k as f64;
                s += if k % 2 == 0 { -t } else { t };
                p *= x;
            }
            assert!((log1pmx(x) - s).abs() <= 4e-16 * s.abs(), "x={x}");
        }
        for &x in &[0.6_f64, 3.0] {
            let d = x.ln_1p() - x;
            assert!((log1pmx(x) - d).abs() <= 1e-15 * d.abs());
        }
    }
}
