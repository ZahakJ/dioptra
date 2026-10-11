//! Bracketed root finding: safeguarded Newton/Halley and Brent's method.
//!
//! These are the workhorses behind every quantile (inverse cdf) in [`crate::dist`] and the
//! inverse special functions in [`crate::special`].

/// Machine epsilon.
const EPS: f64 = f64::EPSILON;

/// Result of evaluating a monotone increasing function for [`halley_bracketed`].
#[derive(Debug, Clone, Copy)]
pub struct HalleyEval {
    /// f(x) (target already subtracted; f must be increasing in x).
    pub f: f64,
    /// f'(x) > 0.
    pub df: f64,
    /// f''(x)/f'(x), or 0 to fall back to Newton.
    pub d2_over_d1: f64,
}

/// Solve f(x) = 0 for an increasing function on the open bracket `(lo, hi)` by Halley's
/// method, falling back to bisection (geometric when the bracket spans many decades)
/// whenever a step leaves the bracket. `hi` may be `+inf`; `lo` may be 0.
///
/// Reference: the safeguarded Newton iteration of Press et al., *Numerical Recipes* 3rd ed.
/// §9.4 (`rtsafe`), with the Halley correction of §9.4.1. Stops when the relative step is
/// below four machine epsilons, as recommended in `research/04-algorithms.md` §0.
pub fn halley_bracketed<F>(mut f: F, x0: f64, mut lo: f64, mut hi: f64, max_iter: usize) -> f64
where
    F: FnMut(f64) -> HalleyEval,
{
    let mut x = if x0.is_finite() && x0 > lo && x0 < hi {
        x0
    } else {
        bisect_point(lo, hi, f64::NAN)
    };
    for _ in 0..max_iter {
        let e = f(x);
        if e.f == 0.0 {
            return x;
        }
        if e.f.is_nan() {
            return f64::NAN;
        }
        if e.f < 0.0 {
            lo = x;
        } else {
            hi = x;
        }
        let mut dx = e.f / e.df;
        if e.d2_over_d1.is_finite() && e.d2_over_d1 != 0.0 {
            let corr = 0.5 * dx * e.d2_over_d1;
            if corr.abs() < 0.8 {
                dx /= 1.0 - corr;
            }
        }
        let mut xn = x - dx;
        if !(xn.is_finite() && xn > lo && xn < hi) || !dx.is_finite() {
            xn = bisect_point(lo, hi, x);
        }
        if (xn - x).abs() <= 4.0 * EPS * xn.abs() {
            return xn;
        }
        if hi.is_finite() && lo > 0.0 && (hi - lo) <= 4.0 * EPS * lo {
            return xn;
        }
        if hi.is_finite() && (hi - lo) <= f64::MIN_POSITIVE {
            return xn;
        }
        x = xn;
    }
    x
}

fn bisect_point(lo: f64, hi: f64, x: f64) -> f64 {
    if !hi.is_finite() {
        let base = if x.is_finite() && x > lo {
            x
        } else {
            lo.abs().max(1.0)
        };
        return if base > 0.0 { base * 4.0 } else { 1.0 };
    }
    if lo > 0.0 && hi / lo > 16.0 {
        (lo.ln() * 0.5 + hi.ln() * 0.5).exp()
    } else if lo == 0.0 && hi > 0.0 {
        // The root may lie many decades below `hi`: step down geometrically.
        let t = if x.is_finite() && x > 0.0 && x < hi {
            x
        } else {
            hi
        };
        let c = t * 1e-3;
        if c > 0.0 {
            c
        } else {
            0.5 * hi
        }
    } else {
        lo + 0.5 * (hi - lo)
    }
}

/// Brent's method for a root of `f` in `[a, b]` where `f(a)` and `f(b)` have opposite signs.
/// Returns `None` if the bracket is invalid.
///
/// Reference: R. P. Brent, *Algorithms for Minimization without Derivatives* (1973), ch. 4
/// (`zeroin`). `xtol_rel` is the relative tolerance on x (absolute floor `xtol_abs`).
pub fn brent<F>(
    mut f: F,
    a: f64,
    b: f64,
    xtol_abs: f64,
    xtol_rel: f64,
    max_iter: usize,
) -> Option<f64>
where
    F: FnMut(f64) -> f64,
{
    let (mut a, mut b) = (a, b);
    let mut fa = f(a);
    let mut fb = f(b);
    if fa == 0.0 {
        return Some(a);
    }
    if fb == 0.0 {
        return Some(b);
    }
    if fa.is_nan() || fb.is_nan() || fa.signum() == fb.signum() {
        return None;
    }
    let mut c = a;
    let mut fc = fa;
    let mut d = b - a;
    let mut e = d;
    for _ in 0..max_iter {
        if fb.signum() == fc.signum() {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let tol = 2.0 * EPS * b.abs() + 0.5 * (xtol_abs + xtol_rel * b.abs());
        let m = 0.5 * (c - b);
        if m.abs() <= tol || fb == 0.0 {
            return Some(b);
        }
        if e.abs() >= tol && fa.abs() > fb.abs() {
            let s = fb / fa;
            let (mut p, mut q);
            if a == c {
                p = 2.0 * m * s;
                q = 1.0 - s;
            } else {
                let qq = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * m * qq * (qq - r) - (b - a) * (r - 1.0));
                q = (qq - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }
            if 2.0 * p < (3.0 * m * q - (tol * q).abs()).min((e * q).abs()) {
                e = d;
                d = p / q;
            } else {
                d = m;
                e = m;
            }
        } else {
            d = m;
            e = m;
        }
        a = b;
        fa = fb;
        if d.abs() > tol {
            b += d;
        } else {
            b += if m > 0.0 { tol } else { -tol };
        }
        fb = f(b);
        if fb.is_nan() {
            return None;
        }
    }
    Some(b)
}

/// Find a bracket `[lo, hi]` around the root of an increasing function by expanding
/// geometrically from `x0` with step `step` (additive while small, multiplicative after).
/// Returns `None` if no sign change is found within `max_expand` expansions.
pub fn expand_bracket<F>(
    mut f: F,
    x0: f64,
    step: f64,
    lower_limit: f64,
    max_expand: usize,
) -> Option<(f64, f64)>
where
    F: FnMut(f64) -> f64,
{
    let f0 = f(x0);
    if f0.is_nan() {
        return None;
    }
    if f0 == 0.0 {
        return Some((x0, x0));
    }
    let mut s = step.abs().max(1e-300);
    let mut prev = x0;
    if f0 < 0.0 {
        for _ in 0..max_expand {
            let x = prev + s;
            let fx = f(x);
            if fx.is_nan() {
                return None;
            }
            if fx >= 0.0 {
                return Some((prev, x));
            }
            prev = x;
            s *= 2.0;
        }
    } else {
        for _ in 0..max_expand {
            let mut x = prev - s;
            if x <= lower_limit {
                x = lower_limit + 0.5 * (prev - lower_limit);
            }
            let fx = f(x);
            if fx.is_nan() {
                return None;
            }
            if fx <= 0.0 {
                return Some((x, prev));
            }
            prev = x;
            s *= 2.0;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brent_finds_sqrt2() {
        let r = brent(|x| x * x - 2.0, 0.0, 2.0, 0.0, 1e-15, 200).unwrap();
        assert!((r - 2f64.sqrt()).abs() < 1e-14);
    }

    #[test]
    fn halley_finds_cube_root() {
        let r = halley_bracketed(
            |x| HalleyEval {
                f: x * x * x - 10.0,
                df: 3.0 * x * x,
                d2_over_d1: 2.0 / x,
            },
            1.0,
            0.0,
            f64::INFINITY,
            100,
        );
        assert!((r - 10f64.cbrt()).abs() < 1e-14);
    }
}
