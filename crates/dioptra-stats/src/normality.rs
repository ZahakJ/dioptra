//! Normality tests: Shapiro–Wilk, D'Agostino–Pearson K^2, Anderson–Darling,
//! Kolmogorov–Smirnov (specified parameters, exact p) and Lilliefors (estimated
//! parameters).
//!
//! Missing values (`NaN`) are skipped; infinities are rejected.

use crate::descriptive::{clean, compensated_sum};
use crate::dist::{std_normal_cdf, std_normal_ln_cdf, std_normal_ln_sf, std_normal_sf};
use crate::lilliefors_table as lt;
use crate::special;
use crate::{Result, StatsError};

fn need(v: &[f64], n: usize, what: &str) -> Result<()> {
    if v.len() < n {
        return Err(StatsError::NotEnoughData(format!(
            "{what} needs at least {n} values, got {}",
            v.len()
        )));
    }
    Ok(())
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m0 = compensated_sum(v.iter().copied()) / n;
    let m = m0 + compensated_sum(v.iter().map(|x| x - m0)) / n;
    let s1 = compensated_sum(v.iter().map(|x| x - m));
    let s2 = compensated_sum(v.iter().map(|x| (x - m) * (x - m)));
    (m, ((s2 - s1 * s1 / n).max(0.0) / (n - 1.0)).sqrt())
}

// =======================================================================================
// Shapiro–Wilk
// =======================================================================================

/// Result of [`shapiro_wilk`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShapiroWilkResult {
    /// W statistic.
    pub w: f64,
    /// p-value.
    pub p: f64,
    /// Sample size.
    pub n: usize,
}

fn poly(c: &[f64], x: f64) -> f64 {
    let mut r = 0.0;
    for &ci in c.iter().rev() {
        r = r * x + ci;
    }
    r
}

/// Shapiro–Wilk W test for 3 <= n <= 5000 by Royston's algorithm AS R94 (Royston 1995,
/// Applied Statistics 44:547; approximations of Royston 1992, Statistics and Computing
/// 2:117): coefficients from Blom-type normal scores m_i = Phi^{-1}((i - 3/8)/(n + 1/4))
/// with polynomial corrections for the two extreme coefficients, W as the squared
/// correlation of the ordered data with the coefficients, and p from Royston's normalising
/// transformations of ln(1 - W) (n <= 11: gamma-shifted log; n >= 12: log-normal).
/// The normal scores use the full-precision AS 241 quantile (as R's `swilk.c`); SciPy's
/// translation agrees to the fixture tolerance.
pub fn shapiro_wilk(data: &[f64]) -> Result<ShapiroWilkResult> {
    let mut x = clean(data)?;
    let n = x.len();
    need(&x, 3, "Shapiro–Wilk")?;
    if n > 5000 {
        return Err(StatsError::InvalidArgument(
            "Shapiro–Wilk (AS R94) is valid for n <= 5000".into(),
        ));
    }
    x.sort_by(f64::total_cmp);
    let range = x[n - 1] - x[0];
    if !(range > 0.0) {
        return Err(StatsError::InvalidArgument(
            "Shapiro–Wilk: all values are identical".into(),
        ));
    }
    let an = n as f64;
    let nn2 = n / 2;
    let mut a = vec![0.0; nn2 + 1]; // 1-based
    const C1: [f64; 6] = [0.0, 0.221157, -0.147981, -2.071190, 4.434685, -2.706056];
    const C2: [f64; 6] = [0.0, 0.042981, -0.293762, -1.752461, 5.682633, -3.582633];
    if n == 3 {
        a[1] = std::f64::consts::FRAC_1_SQRT_2;
    } else {
        let an25 = an + 0.25;
        let mut m = vec![0.0; nn2 + 1];
        let mut summ2 = 0.0;
        for (i, mi) in m.iter_mut().enumerate().skip(1) {
            *mi = special::norm_quantile((i as f64 - 0.375) / an25);
            summ2 += *mi * *mi;
        }
        summ2 *= 2.0;
        let ssumm2 = summ2.sqrt();
        let rsn = 1.0 / an.sqrt();
        let a1 = poly(&C1, rsn) - m[1] / ssumm2;
        let (i1, fac);
        if n > 5 {
            i1 = 3;
            let a2 = -m[2] / ssumm2 + poly(&C2, rsn);
            fac = ((summ2 - 2.0 * m[1] * m[1] - 2.0 * m[2] * m[2])
                / (1.0 - 2.0 * a1 * a1 - 2.0 * a2 * a2))
                .sqrt();
            a[2] = a2;
        } else {
            i1 = 2;
            fac = ((summ2 - 2.0 * m[1] * m[1]) / (1.0 - 2.0 * a1 * a1)).sqrt();
        }
        a[1] = a1;
        for i in i1..=nn2 {
            a[i] = -m[i] / fac;
        }
    }
    // W as the squared correlation between the ordered data and the coefficients.
    let coef = |i: usize| -> f64 {
        // signed coefficient for order statistic i (1-based)
        let j = n + 1 - i;
        if i < j {
            -a[i]
        } else if i > j {
            a[j]
        } else {
            0.0
        }
    };
    let xs: Vec<f64> = x.iter().map(|v| v / range).collect();
    let sa = compensated_sum((1..=n).map(coef)) / an;
    let sx = compensated_sum(xs.iter().copied()) / an;
    let mut ssa = 0.0;
    let mut ssx = 0.0;
    let mut sax = 0.0;
    for (i, xi) in xs.iter().enumerate() {
        let asa = coef(i + 1) - sa;
        let xsx = xi - sx;
        ssa += asa * asa;
        ssx += xsx * xsx;
        sax += asa * xsx;
    }
    let ssassx = (ssa * ssx).sqrt();
    let w1 = (ssassx - sax) * (ssassx + sax) / (ssa * ssx);
    let w = 1.0 - w1;

    if n == 3 {
        let pi6 = 6.0 / std::f64::consts::PI;
        let stqr = std::f64::consts::PI / 3.0;
        let p = (pi6 * (w.sqrt().asin() - stqr)).max(0.0);
        return Ok(ShapiroWilkResult {
            w,
            p: p.min(1.0),
            n,
        });
    }
    let y = w1.ln();
    let xx = an.ln();
    let (mm, s, yy);
    if n <= 11 {
        let gamma = poly(&[-2.273, 0.459], an);
        if y >= gamma {
            return Ok(ShapiroWilkResult { w, p: 1e-99, n });
        }
        yy = -(gamma - y).ln();
        mm = poly(&[0.5440, -0.39978, 0.025054, -6.714e-4], an);
        s = poly(&[1.3822, -0.77857, 0.062767, -0.0020322], an).exp();
    } else {
        yy = y;
        mm = poly(&[-1.5861, -0.31082, -0.083751, 0.0038915], xx);
        s = poly(&[-0.4803, -0.082676, 0.0030302], xx).exp();
    }
    let p = std_normal_sf((yy - mm) / s);
    Ok(ShapiroWilkResult { w, p, n })
}

// =======================================================================================
// D'Agostino–Pearson
// =======================================================================================

/// Result of [`dagostino_pearson`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DagostinoPearsonResult {
    /// Omnibus K^2 = z_skew^2 + z_kurt^2.
    pub k2: f64,
    /// p = P(chi^2_2 >= K^2) = exp(-K^2/2).
    pub p: f64,
    /// Skewness z (D'Agostino 1970).
    pub z_skewness: f64,
    /// Kurtosis z (Anscombe & Glynn 1983).
    pub z_kurtosis: f64,
    /// Sample size.
    pub n: usize,
}

/// D'Agostino–Pearson omnibus K^2 test (D'Agostino, Belanger & D'Agostino 1990): the
/// skewness z of D'Agostino (1970) and the kurtosis z of Anscombe & Glynn (1983), exactly
/// as SciPy `normaltest`/`skewtest`/`kurtosistest`. Requires n >= 8 (as Prism).
pub fn dagostino_pearson(data: &[f64]) -> Result<DagostinoPearsonResult> {
    let v = clean(data)?;
    need(&v, 8, "D'Agostino–Pearson")?;
    let n = v.len() as f64;
    let m = {
        let m0 = compensated_sum(v.iter().copied()) / n;
        m0 + compensated_sum(v.iter().map(|x| x - m0)) / n
    };
    let m2 = compensated_sum(v.iter().map(|x| (x - m).powi(2))) / n;
    let m3 = compensated_sum(v.iter().map(|x| (x - m).powi(3))) / n;
    let m4 = compensated_sum(v.iter().map(|x| (x - m).powi(4))) / n;
    if !(m2 > 0.0) {
        return Err(StatsError::InvalidArgument(
            "D'Agostino–Pearson: all values are identical".into(),
        ));
    }
    // skewtest
    let b2 = m3 / m2.powf(1.5);
    let mut y = b2 * (((n + 1.0) * (n + 3.0)) / (6.0 * (n - 2.0))).sqrt();
    let beta2 = 3.0 * (n * n + 27.0 * n - 70.0) * (n + 1.0) * (n + 3.0)
        / ((n - 2.0) * (n + 5.0) * (n + 7.0) * (n + 9.0));
    let w2 = -1.0 + (2.0 * (beta2 - 1.0)).sqrt();
    let delta = 1.0 / (0.5 * w2.ln()).sqrt();
    let alpha = (2.0 / (w2 - 1.0)).sqrt();
    if y == 0.0 {
        y = 1.0;
    }
    let ya = y / alpha;
    let z1 = delta * (ya + (ya * ya + 1.0).sqrt()).ln();
    // kurtosistest
    let b2k = m4 / (m2 * m2);
    let e = 3.0 * (n - 1.0) / (n + 1.0);
    let varb2 = 24.0 * n * (n - 2.0) * (n - 3.0) / ((n + 1.0) * (n + 1.0) * (n + 3.0) * (n + 5.0));
    let x = (b2k - e) / varb2.sqrt();
    let sqrtbeta1 = 6.0 * (n * n - 5.0 * n + 2.0) / ((n + 7.0) * (n + 9.0))
        * ((6.0 * (n + 3.0) * (n + 5.0)) / (n * (n - 2.0) * (n - 3.0))).sqrt();
    let a =
        6.0 + 8.0 / sqrtbeta1 * (2.0 / sqrtbeta1 + (1.0 + 4.0 / (sqrtbeta1 * sqrtbeta1)).sqrt());
    let term1 = 1.0 - 2.0 / (9.0 * a);
    let denom = 1.0 + x * (2.0 / (a - 4.0)).sqrt();
    if denom == 0.0 {
        return Err(StatsError::InvalidArgument(
            "D'Agostino–Pearson: kurtosis transform undefined".into(),
        ));
    }
    let term2 = denom.signum() * ((1.0 - 2.0 / a) / denom.abs()).powf(1.0 / 3.0);
    let z2 = (term1 - term2) / (2.0 / (9.0 * a)).sqrt();
    let k2 = z1 * z1 + z2 * z2;
    Ok(DagostinoPearsonResult {
        k2,
        p: (-0.5 * k2).exp(),
        z_skewness: z1,
        z_kurtosis: z2,
        n: v.len(),
    })
}

// =======================================================================================
// Anderson–Darling
// =======================================================================================

/// Result of [`anderson_darling`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AndersonDarlingResult {
    /// A^2 against a normal with estimated mean and SD (n - 1).
    pub a2: f64,
    /// Small-sample adjusted A*^2 = A^2 (1 + 0.75/n + 2.25/n^2) (Stephens 1986).
    pub a2_star: f64,
    /// p from D'Agostino & Stephens (1986) Table 4.9 (the formula of R `nortest::ad.test`).
    pub p: f64,
    /// SciPy's critical values of A^2 at `significance_levels`.
    pub critical_values: [f64; 5],
    /// Significance levels in percent: 15, 10, 5, 2.5, 1.
    pub significance_levels: [f64; 5],
    /// Sample size.
    pub n: usize,
}

/// Anderson–Darling normality test with estimated parameters (Anderson & Darling 1954;
/// Stephens 1974). A^2 = -n - (1/n) sum (2i-1)[ln Phi(z_(i)) + ln(1 - Phi(z_(n+1-i)))] with
/// log tails computed directly; p from the piecewise formulas of D'Agostino & Stephens
/// (1986), *Goodness-of-Fit Techniques*, Table 4.9. Requires n >= 4 (as Prism needs
/// n >= 8 for its normality suite, the caller may impose more).
pub fn anderson_darling(data: &[f64]) -> Result<AndersonDarlingResult> {
    let mut v = clean(data)?;
    need(&v, 4, "Anderson–Darling")?;
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let nf = n as f64;
    let (m, s) = mean_sd(&v);
    if !(s > 0.0) {
        return Err(StatsError::InvalidArgument(
            "Anderson–Darling: all values are identical".into(),
        ));
    }
    let z: Vec<f64> = v.iter().map(|x| (x - m) / s).collect();
    let mut acc = 0.0;
    for i in 0..n {
        let coef = (2.0 * (i as f64 + 1.0) - 1.0) / nf;
        acc += coef * (std_normal_ln_cdf(z[i]) + std_normal_ln_sf(z[n - 1 - i]));
    }
    let a2 = -nf - acc;
    let a = a2 * (1.0 + 0.75 / nf + 2.25 / (nf * nf));
    let p = if a >= 0.6 {
        (1.2937 - 5.709 * a + 0.0186 * a * a).exp()
    } else if a >= 0.34 {
        (0.9177 - 4.279 * a - 1.38 * a * a).exp()
    } else if a >= 0.2 {
        1.0 - (-8.318 + 42.796 * a - 59.938 * a * a).exp()
    } else {
        1.0 - (-13.436 + 101.14 * a - 223.73 * a * a).exp()
    };
    const AVALS: [f64; 5] = [0.561, 0.631, 0.752, 0.873, 1.035];
    let scale = 1.0 + 0.75 / nf + 2.25 / (nf * nf);
    let mut cv = [0.0; 5];
    for (c, av) in cv.iter_mut().zip(AVALS) {
        *c = (av / scale * 1000.0).round_ties_even() / 1000.0;
    }
    Ok(AndersonDarlingResult {
        a2,
        a2_star: a,
        p: p.clamp(0.0, 1.0),
        critical_values: cv,
        significance_levels: [15.0, 10.0, 5.0, 2.5, 1.0],
        n,
    })
}

// =======================================================================================
// Kolmogorov–Smirnov
// =======================================================================================

/// Two-sided one-sample KS statistics (D, D+, D-) of sorted data against a cdf.
fn ks_stats(sorted: &[f64], cdf: impl Fn(f64) -> f64) -> (f64, f64, f64) {
    let n = sorted.len() as f64;
    let mut dp: f64 = 0.0;
    let mut dm: f64 = 0.0;
    for (i, &x) in sorted.iter().enumerate() {
        let f = cdf(x);
        dp = dp.max((i as f64 + 1.0) / n - f);
        dm = dm.max(f - i as f64 / n);
    }
    (dp.max(dm), dp, dm)
}

/// Exact one-sided Smirnov upper tail P(D_n^+ >= d) by the Birnbaum & Tingey (1951) sum
/// d sum_{j=0}^{floor(n(1-d))} C(n,j) (1 - d - j/n)^{n-j} (d + j/n)^{j-1}, in log space.
pub fn smirnov_sf(n: usize, d: f64) -> f64 {
    if d.is_nan() {
        return f64::NAN;
    }
    if d <= 0.0 {
        return 1.0;
    }
    if d >= 1.0 {
        return 0.0;
    }
    let nf = n as f64;
    let jmax = (nf * (1.0 - d)).floor() as usize;
    let mut sum = 0.0;
    for j in 0..=jmax.min(n) {
        let jf = j as f64;
        let base = 1.0 - d - jf / nf;
        if base <= 0.0 && n - j > 0 {
            continue;
        }
        let lt = special::ln_choose(nf, jf)
            + if n - j > 0 {
                (nf - jf) * base.ln()
            } else {
                0.0
            }
            + (jf - 1.0) * (d + jf / nf).ln();
        sum += lt.exp();
    }
    (d * sum).clamp(0.0, 1.0)
}

/// Kolmogorov's limiting distribution: P(K > z), K = lim sqrt(n) D_n.
/// For z >= 1.18: 2 sum (-1)^{k-1} exp(-2 k^2 z^2); below, 1 minus the Jacobi-theta form
/// sqrt(2 pi)/z sum exp(-(2k-1)^2 pi^2/(8 z^2)) (Kolmogorov 1933; SciPy `kstwobign`).
pub fn kolmogorov_sf(z: f64) -> f64 {
    if z.is_nan() {
        return f64::NAN;
    }
    if z <= 0.0 {
        return 1.0;
    }
    if z < 1.18 {
        let pi2 = std::f64::consts::PI * std::f64::consts::PI;
        let mut s = 0.0;
        for k in 1..100 {
            let m = (2 * k - 1) as f64;
            let t = (-m * m * pi2 / (8.0 * z * z)).exp();
            s += t;
            if t < 1e-17 * s {
                break;
            }
        }
        (1.0 - (2.0 * std::f64::consts::PI).sqrt() / z * s).clamp(0.0, 1.0)
    } else {
        let mut s = 0.0;
        for k in 1..100 {
            let kf = k as f64;
            let t = (-2.0 * kf * kf * z * z).exp();
            s += if k % 2 == 1 { t } else { -t };
            if t < 1e-17 * s.abs() {
                break;
            }
        }
        (2.0 * s).clamp(0.0, 1.0)
    }
}

/// P(D_n <= d) by Durbin's matrix method in the formulation of Marsaglia, Tsang & Wang
/// (2003, J. Stat. Softw. 8(18)), with power-of-two rescaling against overflow.
fn kolmogn_durbin_cdf(n: usize, d: f64) -> f64 {
    let nd = n as f64 * d;
    if nd <= 0.5 {
        return 0.0;
    }
    let k = nd.ceil() as usize;
    let h = k as f64 - nd;
    let m = 2 * k - 1;
    let mut hm = vec![0.0; m * m];
    let mut v = vec![0.0; m];
    let mut w = vec![0.0; m];
    let mut fac = 1.0;
    for j in 1..=m {
        w[j - 1] = fac;
        fac /= j as f64;
        v[j - 1] = (1.0 - h.powi(j as i32)) * fac;
    }
    let tt = (2.0 * h - 1.0).max(0.0).powi(m as i32) - 2.0 * h.powi(m as i32);
    v[m - 1] = (1.0 + tt) * fac;
    for i in 1..m {
        for r in (i - 1)..m {
            hm[r * m + i] = w[r - (i - 1)];
        }
    }
    for r in 0..m {
        hm[r * m] = v[r];
    }
    for c in 0..m {
        hm[(m - 1) * m + c] = v[m - 1 - c];
    }
    let mul = |a: &[f64], b: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; m * m];
        for i in 0..m {
            for kk in 0..m {
                let aik = a[i * m + kk];
                if aik == 0.0 {
                    continue;
                }
                for j in 0..m {
                    out[i * m + j] += aik * b[kk * m + j];
                }
            }
        }
        out
    };
    let mut pw = vec![0.0; m * m];
    for i in 0..m {
        pw[i * m + i] = 1.0;
    }
    let mut hcur = hm;
    let mut nn = n;
    let mut expnt: i32 = 0;
    let mut hexp: i32 = 0;
    let big = 2f64.powi(128);
    let small = 2f64.powi(-128);
    while nn > 0 {
        if nn % 2 == 1 {
            pw = mul(&pw, &hcur);
            expnt += hexp;
        }
        hcur = mul(&hcur, &hcur);
        hexp *= 2;
        if hcur[(k - 1) * m + (k - 1)].abs() > big {
            for x in hcur.iter_mut() {
                *x /= big;
            }
            hexp += 128;
        }
        nn /= 2;
    }
    let mut p = pw[(k - 1) * m + (k - 1)];
    for i in 1..=n {
        p = i as f64 * p / n as f64;
        if p.abs() < small {
            p *= big;
            expnt -= 128;
        }
    }
    if expnt != 0 {
        p *= 2f64.powi(expnt);
    }
    p.clamp(0.0, 1.0)
}

/// Pelz–Good (1976) asymptotic series for P(D_n <= d), used for large n.
fn kolmogn_pelz_good_cdf(n: usize, x: f64) -> f64 {
    use std::f64::consts::PI;
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let nf = n as f64;
    let z = nf.sqrt() * x;
    let (z2, z3, z4, z6) = (z * z, z * z * z, z.powi(4), z.powi(6));
    let pi2 = PI * PI;
    let pi4 = pi2 * pi2;
    let pi6 = pi4 * pi2;
    let qlog = -pi2 / 8.0 / z2;
    if qlog < -708.0 {
        return 0.0;
    }
    let q = qlog.exp();
    let k1a = -z2;
    let k1b = pi2 / 4.0;
    let k2a = 6.0 * z6 + 2.0 * z4;
    let k2b = (2.0 * z4 - 5.0 * z2) * pi2 / 4.0;
    let k2c = pi4 * (1.0 - 2.0 * z2) / 16.0;
    let k3d = pi6 * (5.0 - 30.0 * z2) / 64.0;
    let k3c = pi4 * (-60.0 * z2 + 212.0 * z4) / 16.0;
    let k3b = pi2 * (135.0 * z4 - 96.0 * z6) / 4.0;
    let k3a = -30.0 * z6 - 90.0 * z.powi(8);
    let mut k = [0.0f64; 4];
    let maxk = (16.0 * z / PI).ceil() as i64;
    for kk in (1..=maxk).rev() {
        let m = (2 * kk - 1) as f64;
        let (m2, m4, m6) = (m * m, m.powi(4), m.powi(6));
        let qp = q.powf(8.0 * kk as f64);
        let coeffs = [
            1.0,
            k1a + k1b * m2,
            k2a + k2b * m2 + k2c * m4,
            k3a + k3b * m2 + k3c * m4 + k3d * m6,
        ];
        for i in 0..4 {
            k[i] = k[i] * qp + coeffs[i];
        }
    }
    let s2pi = (2.0 * PI).sqrt();
    let den = [z, 6.0 * z4, 72.0 * z.powi(7), 6480.0 * z.powi(10)];
    for i in 0..4 {
        k[i] = k[i] * q * s2pi / den[i];
    }
    let q2 = (-pi2 / 2.0 / z2).exp();
    let sqrt3z = 3f64.sqrt() * z;
    let mut k2extra = 0.0;
    let mut k3extra = 0.0;
    for kk in (1..=maxk).rev() {
        let kf = kk as f64;
        let qp = q2.powf(kf * kf);
        k2extra += kf * kf * qp;
        let kspi = PI * kf;
        k3extra += (sqrt3z + kspi) * (sqrt3z - kspi) * kf * kf * qp;
    }
    k[2] += k2extra * pi2 * s2pi / (-36.0 * z3);
    k[3] += k3extra * pi2 * s2pi / (216.0 * z6);
    let mut sum = 0.0;
    for (i, ki) in k.iter().enumerate() {
        sum += ki / nf.powf(i as f64 / 2.0);
    }
    sum.clamp(0.0, 1.0)
}

/// Exact upper tail P(D_n >= d) of the two-sided one-sample KS statistic, following the
/// method selection of Simard & L'Ecuyer (2011, J. Stat. Softw. 39(11)) as implemented in
/// SciPy's `kstwo`: Ruben–Gambino closed forms at the extremes; for n <= 140, Durbin's
/// matrix (Marsaglia–Tsang–Wang 2003) when n d^2 <= 4 and 2 x the exact Smirnov tail
/// otherwise; for larger n, the Smirnov tail when n d^2 >= 2.2, else Durbin's matrix
/// (n <= 1e5, n d^1.5 <= 1.4) or the Pelz–Good series. (SciPy uses Pomeranz's recursion
/// for 0.754693 < n d^2 <= 4; Durbin's matrix is exact there too.)
pub fn kolmogn_sf(n: usize, d: f64) -> f64 {
    if d.is_nan() || n == 0 {
        return f64::NAN;
    }
    if d >= 1.0 {
        return 0.0;
    }
    if d <= 0.0 {
        return 1.0;
    }
    let nf = n as f64;
    let t = nf * d;
    if t <= 1.0 {
        if t <= 0.5 {
            return 1.0;
        }
        let mut prob = 1.0;
        for i in 1..=n {
            prob *= i as f64 / nf * (2.0 * t - 1.0);
        }
        return (1.0 - prob).clamp(0.0, 1.0);
    }
    if t >= nf - 1.0 {
        return (2.0 * (1.0 - d).powi(n as i32)).clamp(0.0, 1.0);
    }
    if d >= 0.5 {
        return (2.0 * smirnov_sf(n, d)).clamp(0.0, 1.0);
    }
    let nx2 = t * d;
    if n <= 140 {
        if nx2 <= 4.0 {
            return (1.0 - kolmogn_durbin_cdf(n, d)).clamp(0.0, 1.0);
        }
        return (2.0 * smirnov_sf(n, d)).clamp(0.0, 1.0);
    }
    if nx2 >= 370.0 {
        return 0.0;
    }
    if nx2 >= 2.2 {
        return (2.0 * smirnov_sf(n, d)).clamp(0.0, 1.0);
    }
    let cdf = if n <= 100_000 && nf * d.powf(1.5) <= 1.4 {
        kolmogn_durbin_cdf(n, d)
    } else {
        kolmogn_pelz_good_cdf(n, d)
    };
    (1.0 - cdf).clamp(0.0, 1.0)
}

/// Result of [`kolmogorov_smirnov_normal`] and [`ks_one_sample`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KsResult {
    /// D = max(D+, D-).
    pub d: f64,
    /// D+ = max(i/n - F(x_(i))).
    pub d_plus: f64,
    /// D- = max(F(x_(i)) - (i-1)/n).
    pub d_minus: f64,
    /// Exact p = P(D_n >= D) ([`kolmogn_sf`]).
    pub p_exact: f64,
    /// Asymptotic p = P(K > sqrt(n) D) ([`kolmogorov_sf`]).
    pub p_asymptotic: f64,
    /// Sample size.
    pub n: usize,
}

/// One-sample two-sided KS test against a fully specified continuous cdf
/// (Kolmogorov 1933; exact p as [`kolmogn_sf`], SciPy `kstest(method='exact')`).
pub fn ks_one_sample(data: &[f64], cdf: impl Fn(f64) -> f64) -> Result<KsResult> {
    let mut v = clean(data)?;
    need(&v, 1, "Kolmogorov–Smirnov")?;
    v.sort_by(f64::total_cmp);
    let (d, dp, dm) = ks_stats(&v, cdf);
    let n = v.len();
    Ok(KsResult {
        d,
        d_plus: dp,
        d_minus: dm,
        p_exact: kolmogn_sf(n, d),
        p_asymptotic: kolmogorov_sf((n as f64).sqrt() * d),
        n,
    })
}

/// KS test against N(mean, sd^2) with parameters given a priori (not estimated from the
/// data; for estimated parameters use [`lilliefors`]).
pub fn kolmogorov_smirnov_normal(data: &[f64], mean: f64, sd: f64) -> Result<KsResult> {
    if !(sd > 0.0 && sd.is_finite() && mean.is_finite()) {
        return Err(StatsError::InvalidArgument(
            "KS: mean must be finite and sd > 0".into(),
        ));
    }
    ks_one_sample(data, |x| std_normal_cdf((x - mean) / sd))
}

/// Result of [`lilliefors`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LillieforsResult {
    /// KS distance to N(mean, sd^2) with mean and SD (n - 1) estimated.
    pub d: f64,
    /// p by interpolation in the statsmodels simulation table
    /// (`lilliefors(pvalmethod='table')`), clipped to [0.001, 0.99].
    pub p_table: f64,
    /// p by the Dallal & Wilkinson (1986) approximation when that is <= 0.1, otherwise the
    /// table value (statsmodels `pvalmethod='approx'`; Prism reports "P > 0.10" there).
    pub p_dallal_wilkinson: f64,
    /// Sample size.
    pub n: usize,
}

/// Dallal & Wilkinson (1986, Am. Stat. 40:294) analytic approximation to the Lilliefors
/// p-value, valid for p < 0.1 (statsmodels `pval_lf`).
pub fn lilliefors_p_dallal_wilkinson(d: f64, n: usize) -> f64 {
    let (mut d, mut nf) = (d, n as f64);
    if nf > 100.0 {
        d *= (nf / 100.0).powf(0.49);
        nf = 100.0;
    }
    (-7.01256 * d * d * (nf + 2.78019) + 2.99587 * d * (nf + 2.78019).sqrt() - 0.122119
        + 0.974598 / nf.sqrt()
        + 1.67997 / nf)
        .exp()
}

/// Lilliefors p-value by the statsmodels table: critical values linearly interpolated in n
/// (asymptotic log-quadratic model above n = 1600), then p linearly interpolated in the
/// critical values; outside the table, the bounding probabilities 0.99 or 0.001.
pub fn lilliefors_p_table(d: f64, n: usize) -> f64 {
    let nf = n as f64;
    let ncol = lt::PERCENTILES.len();
    let mut cv = vec![0.0; ncol];
    let max_size = lt::SIZES[lt::SIZES.len() - 1];
    if nf > max_size {
        let ln = nf.ln();
        for (c, b) in cv.iter_mut().zip(lt::ASYMP.iter()) {
            *c = (b[0] + b[1] * ln + b[2] * ln * ln).exp();
        }
    } else {
        let nf = nf.max(lt::SIZES[0]);
        let mut r = 0;
        while r + 1 < lt::SIZES.len() && lt::SIZES[r + 1] < nf {
            r += 1;
        }
        let (s0, s1) = (lt::SIZES[r], lt::SIZES[(r + 1).min(lt::SIZES.len() - 1)]);
        let t = if s1 > s0 { (nf - s0) / (s1 - s0) } else { 0.0 };
        for (j, c) in cv.iter_mut().enumerate() {
            let a = lt::CRIT[r][j];
            let b = lt::CRIT[(r + 1).min(lt::SIZES.len() - 1)][j];
            *c = a + t * (b - a);
        }
    }
    // Columns are percentiles ascending (critical values ascending); upper-tail prob 1 - pct/100.
    if d < cv[0] {
        return 1.0 - lt::PERCENTILES[0] / 100.0;
    }
    if d > cv[ncol - 1] {
        return 1.0 - lt::PERCENTILES[ncol - 1] / 100.0;
    }
    for j in 0..ncol - 1 {
        if d <= cv[j + 1] {
            let a0 = 1.0 - lt::PERCENTILES[j] / 100.0;
            let a1 = 1.0 - lt::PERCENTILES[j + 1] / 100.0;
            let t = (d - cv[j]) / (cv[j + 1] - cv[j]);
            return a0 + t * (a1 - a0);
        }
    }
    1.0 - lt::PERCENTILES[ncol - 1] / 100.0
}

/// Lilliefors (1967) test: KS distance to the normal with estimated mean and SD (n - 1),
/// p-values from the statsmodels simulation table and from Dallal & Wilkinson (1986).
/// Requires n >= 4.
pub fn lilliefors(data: &[f64]) -> Result<LillieforsResult> {
    let mut v = clean(data)?;
    need(&v, 4, "Lilliefors")?;
    v.sort_by(f64::total_cmp);
    let (m, s) = mean_sd(&v);
    if !(s > 0.0) {
        return Err(StatsError::InvalidArgument(
            "Lilliefors: all values are identical".into(),
        ));
    }
    let (d, _, _) = ks_stats(&v, |x| std_normal_cdf((x - m) / s));
    let n = v.len();
    let pt = lilliefors_p_table(d, n);
    let dw = lilliefors_p_dallal_wilkinson(d, n);
    Ok(LillieforsResult {
        d,
        p_table: pt,
        p_dallal_wilkinson: if dw > 0.1 { pt } else { dw },
        n,
    })
}
