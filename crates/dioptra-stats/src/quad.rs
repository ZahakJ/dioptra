//! Numerical integration: Gauss–Legendre rules and adaptive Gauss–Kronrod (G7/K15).
//!
//! Used by the studentized range, Dunnett and noncentral-t distributions, and available to
//! later modules (profile likelihood, survival) that need one-dimensional integrals.

use std::sync::OnceLock;

/// Nodes and weights of the n-point Gauss–Legendre rule on [-1, 1], as (x, w) pairs in
/// increasing x. Computed by Newton iteration on the Legendre recurrence
/// (Press et al., *Numerical Recipes* 3rd ed. §4.6, `gauleg`); accurate to machine precision.
pub fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![(0.0, 0.0); n];
    let m = n.div_ceil(2);
    let nf = n as f64;
    for i in 0..m {
        let mut z = (std::f64::consts::PI * (i as f64 + 0.75) / (nf + 0.5)).cos();
        let mut pp = 1.0;
        for _ in 0..100 {
            let mut p1 = 1.0;
            let mut p2 = 0.0;
            for j in 0..n {
                let p3 = p2;
                p2 = p1;
                let jf = j as f64;
                p1 = ((2.0 * jf + 1.0) * z * p2 - jf * p3) / (jf + 1.0);
            }
            pp = nf * (z * p1 - p2) / (z * z - 1.0);
            let z1 = z;
            z = z1 - p1 / pp;
            if (z - z1).abs() <= 1e-16 {
                break;
            }
        }
        let w = 2.0 / ((1.0 - z * z) * pp * pp);
        out[i] = (-z, w);
        out[n - 1 - i] = (z, w);
    }
    if n % 2 == 1 {
        out[n / 2].0 = 0.0;
    }
    out
}

static GL10: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
static GL16: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
static GL20: OnceLock<Vec<(f64, f64)>> = OnceLock::new();

/// Cached Gauss–Legendre rule for n in {10, 16, 20}; other n are computed on demand.
pub fn gauss_legendre_cached(n: usize) -> std::borrow::Cow<'static, [(f64, f64)]> {
    match n {
        10 => std::borrow::Cow::Borrowed(GL10.get_or_init(|| gauss_legendre(10)).as_slice()),
        16 => std::borrow::Cow::Borrowed(GL16.get_or_init(|| gauss_legendre(16)).as_slice()),
        20 => std::borrow::Cow::Borrowed(GL20.get_or_init(|| gauss_legendre(20)).as_slice()),
        _ => std::borrow::Cow::Owned(gauss_legendre(n)),
    }
}

/// Composite Gauss–Legendre nodes on [a, b]: `panels` equal panels of an `n`-point rule.
/// Returns (x, w) pairs whose weights sum to b - a.
pub fn composite_nodes(a: f64, b: f64, panels: usize, n: usize) -> Vec<(f64, f64)> {
    let rule = gauss_legendre_cached(n);
    let h = (b - a) / panels as f64;
    let mut out = Vec::with_capacity(panels * n);
    for p in 0..panels {
        let lo = a + h * p as f64;
        let c = lo + 0.5 * h;
        for &(x, w) in rule.iter() {
            out.push((c + 0.5 * h * x, 0.5 * h * w));
        }
    }
    out
}

// Kronrod 15-point nodes (positive half, descending) and weights; Gauss 7-point weights.
// Reference: Piessens et al., QUADPACK (1983), routine QK15.
const XGK: [f64; 8] = [
    0.9914553711208126,
    0.9491079123427585,
    0.8648644233597691,
    0.7415311855993943,
    0.5860872354676911,
    0.4058451513773972,
    0.2077849550078985,
    0.0,
];
const WGK: [f64; 8] = [
    0.02293532201052922,
    0.06309209262997854,
    0.1047900103222502,
    0.1406532597155259,
    0.1690047266392679,
    0.1903505780647854,
    0.2044329400752989,
    0.2094821410847278,
];
const WG: [f64; 4] = [
    0.1294849661688697,
    0.2797053914892767,
    0.3818300505051189,
    0.4179591836734694,
];

fn gk15<F: FnMut(f64) -> f64>(f: &mut F, a: f64, b: f64) -> (f64, f64) {
    let c = 0.5 * (a + b);
    let h = 0.5 * (b - a);
    let fc = f(c);
    let mut resk = fc * WGK[7];
    let mut resg = fc * WG[3];
    for j in 0..7 {
        let x = h * XGK[j];
        let f1 = f(c - x);
        let f2 = f(c + x);
        resk += WGK[j] * (f1 + f2);
        if j % 2 == 1 {
            resg += WG[j / 2] * (f1 + f2);
        }
    }
    (resk * h, ((resk - resg) * h).abs())
}

/// Adaptive Gauss–Kronrod (G7/K15) integration of `f` over [a, b] (finite), refining the
/// interval with the largest error estimate until the total estimated error is below
/// max(abs_tol, rel_tol * |integral|) or `max_intervals` is reached.
/// Reference: Piessens et al., QUADPACK (1983), routine QAG with the QK15 rule.
/// Returns (integral, error estimate).
pub fn integrate<F: FnMut(f64) -> f64>(
    mut f: F,
    a: f64,
    b: f64,
    rel_tol: f64,
    abs_tol: f64,
    max_intervals: usize,
) -> (f64, f64) {
    if a == b {
        return (0.0, 0.0);
    }
    let (r0, e0) = gk15(&mut f, a, b);
    let mut ivals: Vec<(f64, f64, f64, f64)> = vec![(a, b, r0, e0)];
    let mut total = r0;
    let mut err = e0;
    while err > abs_tol.max(rel_tol * total.abs()) && ivals.len() < max_intervals {
        // pick the worst interval
        let (idx, _) =
            ivals.iter().enumerate().fold(
                (0, -1.0),
                |acc, (i, iv)| if iv.3 > acc.1 { (i, iv.3) } else { acc },
            );
        let (lo, hi, r, e) = ivals.swap_remove(idx);
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            ivals.push((lo, hi, r, 0.0));
            err -= e;
            continue;
        }
        let (r1, e1) = gk15(&mut f, lo, mid);
        let (r2, e2) = gk15(&mut f, mid, hi);
        total += r1 + r2 - r;
        err += e1 + e2 - e;
        ivals.push((lo, mid, r1, e1));
        ivals.push((mid, hi, r2, e2));
    }
    // Re-sum for accuracy.
    let total: f64 = ivals.iter().map(|iv| iv.2).sum();
    let err: f64 = ivals.iter().map(|iv| iv.3).sum();
    (total, err)
}

fn gk15_pair<F: FnMut(f64) -> (f64, f64)>(f: &mut F, a: f64, b: f64) -> ([f64; 2], [f64; 2]) {
    let c = 0.5 * (a + b);
    let h = 0.5 * (b - a);
    let mut fv = [(0.0, 0.0); 15];
    fv[7] = f(c);
    for j in 0..7 {
        let x = h * XGK[j];
        fv[j] = f(c - x);
        fv[14 - j] = f(c + x);
    }
    let mut res = [0.0; 2];
    let mut err = [0.0; 2];
    for comp in 0..2 {
        let val = |i: usize| if comp == 0 { fv[i].0 } else { fv[i].1 };
        let mut rk = val(7) * WGK[7];
        let mut rg = val(7) * WG[3];
        let mut rabs = rk.abs();
        for j in 0..7 {
            let s2 = val(j) + val(14 - j);
            rk += WGK[j] * s2;
            rabs += WGK[j] * (val(j).abs() + val(14 - j).abs());
            if j % 2 == 1 {
                rg += WG[j / 2] * s2;
            }
        }
        let mean = 0.5 * rk;
        let mut rasc = WGK[7] * (val(7) - mean).abs();
        for (j, w) in WGK.iter().take(7).enumerate() {
            rasc += w * ((val(j) - mean).abs() + (val(14 - j) - mean).abs());
        }
        let (r, rabs, rasc) = (rk * h, rabs * h.abs(), rasc * h.abs());
        let mut e = ((rk - rg) * h).abs();
        // QUADPACK QK15 error scaling.
        if rasc != 0.0 && e != 0.0 {
            e = rasc * (200.0 * e / rasc).powf(1.5).min(1.0);
        }
        if rabs > f64::MIN_POSITIVE / (50.0 * f64::EPSILON) {
            e = e.max(50.0 * f64::EPSILON * rabs);
        }
        res[comp] = r;
        err[comp] = e;
    }
    (res, err)
}

/// Adaptive G7/K15 integration of a pair-valued integrand over the consecutive intervals
/// defined by `breaks` (at least two increasing points). Refines the interval with the
/// largest scaled error until component 0 has absolute error <= `abs0` and component 1 has
/// error <= max(`abs1`, `rel1` |integral_1|) (QUADPACK QAG strategy, Piessens et al. 1983).
/// Used where a cdf and its complement are integrated together so that both keep accuracy.
pub fn integrate_pair<F: FnMut(f64) -> (f64, f64)>(
    mut f: F,
    breaks: &[f64],
    abs0: f64,
    rel1: f64,
    abs1: f64,
    max_intervals: usize,
) -> (f64, f64) {
    struct Iv {
        a: f64,
        b: f64,
        r: [f64; 2],
        e: [f64; 2],
    }
    let mut ivs: Vec<Iv> = Vec::new();
    for w in breaks.windows(2) {
        if w[1] > w[0] {
            let (r, e) = gk15_pair(&mut f, w[0], w[1]);
            ivs.push(Iv {
                a: w[0],
                b: w[1],
                r,
                e,
            });
        }
    }
    loop {
        let t1: f64 = ivs.iter().map(|iv| iv.r[1]).sum();
        let e0: f64 = ivs.iter().map(|iv| iv.e[0]).sum();
        let e1: f64 = ivs.iter().map(|iv| iv.e[1]).sum();
        let tol1 = abs1.max(rel1 * t1.abs());
        if (e0 <= abs0 && e1 <= tol1) || ivs.len() >= max_intervals {
            break;
        }
        // worst interval by scaled error
        let score = |iv: &Iv| (iv.e[0] / abs0).max(iv.e[1] / tol1.max(f64::MIN_POSITIVE));
        let mut wi = 0;
        let mut ws = -1.0;
        for (i, iv) in ivs.iter().enumerate() {
            let s = score(iv);
            if s > ws {
                ws = s;
                wi = i;
            }
        }
        let iv = ivs.swap_remove(wi);
        let mid = 0.5 * (iv.a + iv.b);
        if !(mid > iv.a && mid < iv.b) {
            ivs.push(Iv {
                e: [0.0, 0.0],
                ..iv
            });
            continue;
        }
        let (r1, e1) = gk15_pair(&mut f, iv.a, mid);
        let (r2, e2) = gk15_pair(&mut f, mid, iv.b);
        ivs.push(Iv {
            a: iv.a,
            b: mid,
            r: r1,
            e: e1,
        });
        ivs.push(Iv {
            a: mid,
            b: iv.b,
            r: r2,
            e: e2,
        });
    }
    let r0: f64 = ivs.iter().map(|iv| iv.r[0]).sum();
    let r1: f64 = ivs.iter().map(|iv| iv.r[1]).sum();
    (r0, r1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauss_legendre_integrates_polynomials_exactly() {
        let rule = gauss_legendre(10);
        // degree 19 is exact for a 10-point rule
        let s: f64 = rule.iter().map(|&(x, w)| w * x.powi(18)).sum();
        assert!((s - 2.0 / 19.0).abs() < 1e-15);
        let wsum: f64 = rule.iter().map(|&(_, w)| w).sum();
        assert!((wsum - 2.0).abs() < 1e-15);
    }

    #[test]
    fn kronrod_rule_is_consistent() {
        let wsum: f64 = 2.0 * WGK[..7].iter().sum::<f64>() + WGK[7];
        assert!((wsum - 2.0).abs() < 1e-15);
        let gsum: f64 = 2.0 * WG[..3].iter().sum::<f64>() + WG[3];
        assert!((gsum - 2.0).abs() < 1e-15);
        // K15 is exact to degree 22.
        let (r, _) = gk15(&mut |x: f64| x.powi(22), -1.0, 1.0);
        assert!((r - 2.0 / 23.0).abs() < 1e-15);
    }

    #[test]
    fn adaptive_integrates_gaussian() {
        let (r, _) = integrate(|x| (-0.5 * x * x).exp(), -12.0, 12.0, 1e-14, 0.0, 500);
        assert!((r - (2.0 * std::f64::consts::PI).sqrt()).abs() < 1e-13);
    }
}
