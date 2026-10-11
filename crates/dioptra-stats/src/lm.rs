//! Levenberg–Marquardt least squares after Moré (1978), "The Levenberg–Marquardt
//! algorithm: implementation and theory", Lecture Notes in Mathematics 630:105,
//! doi:10.1007/BFb0067700, as implemented in MINPACK `lmder`, `lmpar`, `qrfac` and
//! `qrsolv` (Moré, Garbow & Hillstrom 1980, ANL-80-74).
//!
//! The port keeps MINPACK's scaled trust region (diag from the Jacobian column norms,
//! never decreasing), QR with column pivoting, the Moré update of the damping parameter
//! and its four convergence tests. Three additions:
//!
//! * **Box constraints** by an active-set projection: at each outer iteration a
//!   parameter sitting on a bound whose negative gradient points out of the box is held
//!   fixed; the trust-region step of the remaining parameters is projected onto the box
//!   and its predicted reduction recomputed from the Jacobian for the projected step.
//! * **Geodesic acceleration** (Transtrum & Sethna 2012, arXiv:1201.5885): optional
//!   second-order correction a = -(J'J + lambda D'D)^{-1} J' r_vv with r_vv the second
//!   directional derivative along the LM velocity v by finite differences (h = 0.1);
//!   the step v + a/2 is used when 2|Da|/|Dv| <= 0.75.
//! * **Polishing**: after convergence, Gauss–Newton steps solved by the refined QR of
//!   `linalg` (double-double residual refinement) while the residual sum of squares,
//!   accumulated in compensated arithmetic, keeps decreasing.

use crate::linalg::Acc;

/// A residual vector function with Jacobian.
pub(crate) trait Residuals {
    /// Number of residuals.
    fn m(&self) -> usize;
    /// Fill `r` (length m) and, when given, `jac` (column-major m x n). Returns false if
    /// a value is not finite (the point is then rejected).
    fn eval(&self, p: &[f64], r: &mut [f64], jac: Option<&mut [f64]>) -> bool;
}

/// Stopping reason (MINPACK `info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LmStop {
    /// Relative reduction in the sum of squares at most ftol (info 1, 6).
    Ftol,
    /// Relative step at most xtol (info 2, 3, 7).
    Xtol,
    /// Gradient (cosine of the angle between r and the columns of J) at most gtol (4, 8).
    Gtol,
    /// Iteration limit (info 5).
    MaxIter,
    /// The residuals were not finite at the start, or the Jacobian was not finite.
    NotFinite,
}

impl LmStop {
    pub(crate) fn converged(self) -> bool {
        matches!(self, LmStop::Ftol | LmStop::Xtol | LmStop::Gtol)
    }
}

/// LM controls.
#[derive(Debug, Clone)]
pub(crate) struct LmOptions {
    pub ftol: f64,
    pub xtol: f64,
    pub gtol: f64,
    pub max_iter: usize,
    /// Initial trust-region radius factor (MINPACK `factor`, 100).
    pub factor: f64,
    pub geodesic: bool,
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    pub polish: bool,
}

impl LmOptions {
    pub(crate) fn new(n: usize) -> Self {
        LmOptions {
            ftol: 1e-12,
            xtol: 1e-12,
            gtol: 0.0,
            max_iter: 1000,
            factor: 100.0,
            geodesic: false,
            lower: vec![f64::NEG_INFINITY; n],
            upper: vec![f64::INFINITY; n],
            polish: true,
        }
    }
}

/// LM outcome.
#[derive(Debug, Clone)]
pub(crate) struct LmOutcome {
    pub p: Vec<f64>,
    /// Compensated sum of squared residuals.
    pub ss: f64,
    pub iterations: usize,
    pub nfev: usize,
    pub stop: LmStop,
    /// Parameters on a bound at the end.
    pub at_bound: Vec<bool>,
}

fn enorm(v: &[f64]) -> f64 {
    let scale = v.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    if scale == 0.0 || !scale.is_finite() {
        return scale;
    }
    let s: f64 = v.iter().map(|x| (x / scale) * (x / scale)).sum();
    scale * s.sqrt()
}

pub(crate) fn sum_sq(r: &[f64]) -> f64 {
    let mut a = Acc::default();
    for &x in r {
        a.add_prod(x, x);
    }
    a.value()
}

/// MINPACK `qrfac` with pivoting on an m x n column-major matrix. On return `a` holds R
/// strictly above the diagonal and the Householder vectors on and below it (MINPACK
/// convention: v_j with v_j[j] = 1 + a_jj/norm scaling); `rdiag` the diagonal of R,
/// `acnorm` the input column norms, `ipvt` the permutation.
struct QrFac {
    rdiag: Vec<f64>,
    acnorm: Vec<f64>,
    ipvt: Vec<usize>,
    /// Householder diagonal elements (a_jj after factoring), kept to apply Q' later.
    hdiag: Vec<f64>,
}

fn qrfac(a: &mut [f64], m: usize, n: usize) -> QrFac {
    let mut acnorm = vec![0.0; n];
    let mut rdiag = vec![0.0; n];
    let mut ipvt: Vec<usize> = (0..n).collect();
    for j in 0..n {
        acnorm[j] = enorm(&a[j * m..(j + 1) * m]);
    }
    let steps = m.min(n);
    for j in 0..steps {
        // pivot: largest remaining column norm (recomputed exactly)
        let mut kmax = j;
        let mut best = -1.0;
        for k in j..n {
            let s = enorm(&a[k * m + j..(k + 1) * m]);
            if s > best {
                best = s;
                kmax = k;
            }
        }
        if kmax != j {
            for i in 0..m {
                a.swap(j * m + i, kmax * m + i);
            }
            ipvt.swap(j, kmax);
        }
        let mut ajnorm = enorm(&a[j * m + j..(j + 1) * m]);
        if ajnorm != 0.0 {
            if a[j * m + j] < 0.0 {
                ajnorm = -ajnorm;
            }
            for i in j..m {
                a[j * m + i] /= ajnorm;
            }
            a[j * m + j] += 1.0;
            let ajj = a[j * m + j];
            for k in (j + 1)..n {
                let mut sum = 0.0;
                for i in j..m {
                    sum += a[j * m + i] * a[k * m + i];
                }
                let temp = sum / ajj;
                for i in j..m {
                    a[k * m + i] -= temp * a[j * m + i];
                }
            }
        }
        rdiag[j] = -ajnorm;
    }
    let hdiag = (0..steps).map(|j| a[j * m + j]).collect();
    QrFac {
        rdiag,
        acnorm,
        ipvt,
        hdiag,
    }
}

/// y <- Q' y using the Householder vectors in `a` (diagonal elements from `hdiag`).
fn apply_qt(a: &[f64], hdiag: &[f64], m: usize, y: &mut [f64]) {
    for (j, &hjj) in hdiag.iter().enumerate() {
        if hjj == 0.0 {
            continue;
        }
        let mut sum = hjj * y[j];
        for i in (j + 1)..m {
            sum += a[j * m + i] * y[i];
        }
        let temp = -sum / hjj;
        y[j] += hjj * temp;
        for i in (j + 1)..m {
            y[i] += a[j * m + i] * temp;
        }
    }
}

/// MINPACK `qrsolv`: given R (upper triangle of `r`, n x n column-major, leading dim
/// `ldr`), solve min |A x - b|^2 + |D x|^2 where A P = Q R and qtb = Q'b. On return the
/// strict lower triangle of r holds S' and `sdiag` the diagonal of S, with
/// P'(A'A + DD)P = S'S.
#[allow(clippy::too_many_arguments)]
fn qrsolv(
    n: usize,
    r: &mut [f64],
    ldr: usize,
    ipvt: &[usize],
    diag: &[f64],
    qtb: &[f64],
    x: &mut [f64],
    sdiag: &mut [f64],
) {
    let mut wa = vec![0.0; n];
    for j in 0..n {
        for i in j..n {
            r[j * ldr + i] = r[i * ldr + j];
        }
        x[j] = r[j * ldr + j];
        wa[j] = qtb[j];
    }
    for j in 0..n {
        let l = ipvt[j];
        if diag[l] != 0.0 {
            for s in sdiag[j..n].iter_mut() {
                *s = 0.0;
            }
            sdiag[j] = diag[l];
            let mut qtbpj = 0.0;
            for k in j..n {
                if sdiag[k] == 0.0 {
                    continue;
                }
                let rkk = r[k * ldr + k];
                let (sin, cos);
                if rkk.abs() < sdiag[k].abs() {
                    let cotan = rkk / sdiag[k];
                    sin = 0.5 / (0.25 + 0.25 * cotan * cotan).sqrt();
                    cos = sin * cotan;
                } else {
                    let tan = sdiag[k] / rkk;
                    cos = 0.5 / (0.25 + 0.25 * tan * tan).sqrt();
                    sin = cos * tan;
                }
                r[k * ldr + k] = cos * rkk + sin * sdiag[k];
                let temp = cos * wa[k] + sin * qtbpj;
                qtbpj = -sin * wa[k] + cos * qtbpj;
                wa[k] = temp;
                for i in (k + 1)..n {
                    let rik = r[k * ldr + i];
                    let temp = cos * rik + sin * sdiag[i];
                    sdiag[i] = -sin * rik + cos * sdiag[i];
                    r[k * ldr + i] = temp;
                }
            }
        }
        sdiag[j] = r[j * ldr + j];
        r[j * ldr + j] = x[j];
    }
    let mut nsing = n;
    for j in 0..n {
        if sdiag[j] == 0.0 && nsing == n {
            nsing = j;
        }
        if nsing < n {
            wa[j] = 0.0;
        }
    }
    for k in 0..nsing {
        let j = nsing - 1 - k;
        let mut sum = 0.0;
        for i in (j + 1)..nsing {
            sum += r[j * ldr + i] * wa[i];
        }
        wa[j] = (wa[j] - sum) / sdiag[j];
    }
    for j in 0..n {
        x[ipvt[j]] = wa[j];
    }
}

/// MINPACK `lmpar`: Levenberg–Marquardt parameter for the trust region `delta`.
/// `r` (n x n in the leading part of an m x n array, `ldr` = m) holds R above the
/// diagonal (and `rdiag`), restored from `rdiag` on each call. Returns (par, x) with x
/// the step (to be negated).
#[allow(clippy::too_many_arguments)]
fn lmpar(
    n: usize,
    r: &mut [f64],
    ldr: usize,
    ipvt: &[usize],
    diag: &[f64],
    qtb: &[f64],
    delta: f64,
    par_in: f64,
    x: &mut [f64],
    sdiag: &mut [f64],
) -> f64 {
    let dwarf = f64::MIN_POSITIVE;
    let mut par = par_in;
    let mut wa1 = vec![0.0; n];
    let mut wa2 = vec![0.0; n];
    // Gauss–Newton direction
    let mut nsing = n;
    for j in 0..n {
        wa1[j] = qtb[j];
        if r[j * ldr + j] == 0.0 && nsing == n {
            nsing = j;
        }
        if nsing < n {
            wa1[j] = 0.0;
        }
    }
    for k in 0..nsing {
        let j = nsing - 1 - k;
        wa1[j] /= r[j * ldr + j];
        let temp = wa1[j];
        for i in 0..j {
            wa1[i] -= r[j * ldr + i] * temp;
        }
    }
    for j in 0..n {
        x[ipvt[j]] = wa1[j];
    }
    let mut iter = 0;
    for j in 0..n {
        wa2[j] = diag[j] * x[j];
    }
    let mut dxnorm = enorm(&wa2);
    let mut fp = dxnorm - delta;
    if fp <= 0.1 * delta {
        return 0.0;
    }
    let mut parl = 0.0;
    if nsing >= n {
        for j in 0..n {
            let l = ipvt[j];
            wa1[j] = diag[l] * (wa2[l] / dxnorm);
        }
        for j in 0..n {
            let mut sum = 0.0;
            for i in 0..j {
                sum += r[j * ldr + i] * wa1[i];
            }
            wa1[j] = (wa1[j] - sum) / r[j * ldr + j];
        }
        let temp = enorm(&wa1);
        parl = ((fp / delta) / temp) / temp;
    }
    for j in 0..n {
        let mut sum = 0.0;
        for i in 0..=j {
            sum += r[j * ldr + i] * qtb[i];
        }
        let l = ipvt[j];
        wa1[j] = sum / diag[l];
    }
    let gnorm = enorm(&wa1);
    let mut paru = gnorm / delta;
    if paru == 0.0 {
        paru = dwarf / delta.min(0.1);
    }
    par = par.max(parl).min(paru);
    if par == 0.0 {
        par = gnorm / dxnorm;
    }
    loop {
        iter += 1;
        if par == 0.0 {
            par = dwarf.max(0.001 * paru);
        }
        let temp = par.sqrt();
        for j in 0..n {
            wa1[j] = temp * diag[j];
        }
        qrsolv(n, r, ldr, ipvt, &wa1, qtb, x, sdiag);
        for j in 0..n {
            wa2[j] = diag[j] * x[j];
        }
        dxnorm = enorm(&wa2);
        let temp = fp;
        fp = dxnorm - delta;
        if fp.abs() <= 0.1 * delta || (parl == 0.0 && fp <= temp && temp < 0.0) || iter == 10 {
            break;
        }
        for j in 0..n {
            let l = ipvt[j];
            wa1[j] = diag[l] * (wa2[l] / dxnorm);
        }
        for j in 0..n {
            wa1[j] /= sdiag[j];
            let temp = wa1[j];
            for i in (j + 1)..n {
                wa1[i] -= r[j * ldr + i] * temp;
            }
        }
        let temp = enorm(&wa1);
        let parc = ((fp / delta) / temp) / temp;
        if fp > 0.0 {
            parl = parl.max(par);
        }
        if fp < 0.0 {
            paru = paru.min(par);
        }
        par = parl.max(par + parc);
    }
    par
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// Minimise sum r_i(p)^2 from `p0`.
pub(crate) fn minimize(f: &dyn Residuals, p0: &[f64], opts: &LmOptions) -> LmOutcome {
    let n = p0.len();
    let m = f.m();
    let epsmch = f64::EPSILON;
    let mut x: Vec<f64> = p0
        .iter()
        .enumerate()
        .map(|(j, &v)| clamp(v, opts.lower[j], opts.upper[j]))
        .collect();
    let mut fvec = vec![0.0; m];
    let mut nfev = 1;
    let at_bound = |x: &[f64]| -> Vec<bool> {
        (0..n)
            .map(|j| x[j] <= opts.lower[j] || x[j] >= opts.upper[j])
            .collect()
    };
    if !f.eval(&x, &mut fvec, None) {
        let ss = f64::NAN;
        return LmOutcome {
            at_bound: at_bound(&x),
            p: x,
            ss,
            iterations: 0,
            nfev,
            stop: LmStop::NotFinite,
        };
    }
    let mut fnorm = enorm(&fvec);
    let mut jac = vec![0.0; m * n];
    let mut diag = vec![0.0; n];
    let mut delta = 0.0;
    let mut xnorm = 0.0;
    let mut par = 0.0;
    let mut iter = 0usize;
    let mut stop = LmStop::MaxIter;
    let mut first = true;
    let mut successes = 0usize;
    let mut trial = vec![0.0; n];
    let mut fnew = vec![0.0; m];
    'outer: while iter < opts.max_iter {
        iter += 1;
        if !f.eval(&x, &mut fvec, Some(&mut jac)) || jac.iter().any(|v| !v.is_finite()) {
            stop = LmStop::NotFinite;
            break;
        }
        nfev += 1;
        // Active set: hold parameters on a bound when the descent direction leaves the box.
        let mut grad = vec![0.0; n];
        for (j, g) in grad.iter_mut().enumerate() {
            let c = &jac[j * m..(j + 1) * m];
            *g = c.iter().zip(&fvec).map(|(a, b)| a * b).sum();
        }
        let free: Vec<usize> = (0..n)
            .filter(|&j| {
                !((x[j] <= opts.lower[j] && grad[j] > 0.0)
                    || (x[j] >= opts.upper[j] && grad[j] < 0.0))
            })
            .collect();
        let nf = free.len();
        if nf == 0 {
            stop = LmStop::Gtol;
            break;
        }
        let mut a = Vec::with_capacity(m * nf);
        for &j in &free {
            a.extend_from_slice(&jac[j * m..(j + 1) * m]);
        }
        let qr = qrfac(&mut a, m, nf);
        // Householder vectors, kept intact (lmpar/qrsolv overwrite the lower triangle).
        let hh = if opts.geodesic { a.clone() } else { Vec::new() };
        if first {
            for &j in &free {
                let c = enorm(&jac[j * m..(j + 1) * m]);
                diag[j] = if c == 0.0 { 1.0 } else { c };
            }
            for d in diag.iter_mut() {
                if *d == 0.0 {
                    *d = 1.0;
                }
            }
            let wa: Vec<f64> = (0..n).map(|j| diag[j] * x[j]).collect();
            xnorm = enorm(&wa);
            delta = opts.factor * xnorm;
            if delta == 0.0 {
                delta = opts.factor;
            }
        }
        // qtf = Q' fvec
        let mut qtf_full = fvec.clone();
        apply_qt(&a, &qr.hdiag, m, &mut qtf_full);
        first = false;
        let steps = m.min(nf);
        for j in 0..steps {
            a[j * m + j] = qr.rdiag[j];
        }
        let mut qtf = vec![0.0; nf];
        qtf[..steps].copy_from_slice(&qtf_full[..steps]);
        // gradient norm (scaled cosine)
        let mut gnorm: f64 = 0.0;
        if fnorm != 0.0 {
            for j in 0..nf {
                let l = qr.ipvt[j];
                if qr.acnorm[l] != 0.0 {
                    let mut sum = 0.0;
                    for i in 0..=j.min(steps.saturating_sub(1)) {
                        sum += a[j * m + i] * (qtf[i] / fnorm);
                    }
                    gnorm = gnorm.max((sum / qr.acnorm[l]).abs());
                }
            }
        }
        if gnorm <= opts.gtol {
            stop = LmStop::Gtol;
            break;
        }
        for (k, &j) in free.iter().enumerate() {
            diag[j] = diag[j].max(qr.acnorm[k]);
        }
        let dfree: Vec<f64> = free.iter().map(|&j| diag[j]).collect();
        // inner loop
        loop {
            let mut step_r = vec![0.0; nf];
            let mut sdiag = vec![0.0; nf];
            if m < nf {
                // underdetermined: pad (cannot happen for fits with df >= 0 checked upstream)
                stop = LmStop::NotFinite;
                break 'outer;
            }
            par = lmpar(
                nf,
                &mut a,
                m,
                &qr.ipvt,
                &dfree,
                &qtf,
                delta,
                par,
                &mut step_r,
                &mut sdiag,
            );
            // full step (negated) in parameter space
            let mut s = vec![0.0; n];
            for (k, &j) in free.iter().enumerate() {
                s[j] = -step_r[k];
            }
            let mut exact_step = true;
            if opts.geodesic {
                // second directional derivative along s by finite differences
                let h = 0.1;
                let xh: Vec<f64> = (0..n).map(|j| x[j] + h * s[j]).collect();
                let mut rh = vec![0.0; m];
                nfev += 1;
                if f.eval(&xh, &mut rh, None) {
                    let mut rvv = vec![0.0; m];
                    for i in 0..m {
                        let mut jv = 0.0;
                        for j in 0..n {
                            jv += jac[j * m + i] * s[j];
                        }
                        rvv[i] = (2.0 / h) * ((rh[i] - fvec[i]) / h - jv);
                    }
                    let mut qt = rvv.clone();
                    apply_qt(&hh, &qr.hdiag, m, &mut qt);
                    let mut acc_r = vec![0.0; nf];
                    let mut sd2 = vec![0.0; nf];
                    let sq = par.sqrt();
                    let dpar: Vec<f64> = dfree.iter().map(|d| d * sq).collect();
                    // restore R's diagonal (qrsolv overwrote the lower triangle only)
                    qrsolv(
                        nf,
                        &mut a,
                        m,
                        &qr.ipvt,
                        &dpar,
                        &qt[..nf],
                        &mut acc_r,
                        &mut sd2,
                    );
                    let dv = enorm(&free.iter().map(|&j| diag[j] * s[j]).collect::<Vec<f64>>());
                    let da = enorm(
                        &free
                            .iter()
                            .enumerate()
                            .map(|(k, &j)| diag[j] * acc_r[k])
                            .collect::<Vec<f64>>(),
                    );
                    if dv > 0.0 && 2.0 * da / dv <= 0.75 {
                        for (k, &j) in free.iter().enumerate() {
                            s[j] -= 0.5 * acc_r[k];
                        }
                        exact_step = false;
                    }
                }
            }
            for j in 0..n {
                trial[j] = clamp(x[j] + s[j], opts.lower[j], opts.upper[j]);
                if trial[j] != x[j] + s[j] {
                    exact_step = false;
                }
                s[j] = trial[j] - x[j];
            }
            let ds: Vec<f64> = (0..n).map(|j| diag[j] * s[j]).collect();
            let pnorm = enorm(&ds);
            if successes == 0 {
                delta = delta.min(pnorm);
            }
            nfev += 1;
            let ok = f.eval(&trial, &mut fnew, None);
            let fnorm1 = if ok { enorm(&fnew) } else { f64::INFINITY };
            let actred = if 0.1 * fnorm1 < fnorm {
                1.0 - (fnorm1 / fnorm) * (fnorm1 / fnorm)
            } else {
                -1.0
            };
            // predicted reduction
            let (prered, dirder) = if exact_step {
                let mut wa3 = vec![0.0; nf];
                for j in 0..nf {
                    let l = qr.ipvt[j];
                    let temp = -step_r[l];
                    for i in 0..=j {
                        wa3[i] += a[j * m + i] * temp;
                    }
                }
                // a's upper triangle is R (columns in pivot order); the step is indexed
                // by the original free columns, hence ipvt above.
                let temp1 = enorm(&wa3) / fnorm;
                let temp2 = par.sqrt() * pnorm / fnorm;
                (
                    temp1 * temp1 + temp2 * temp2 / 0.5,
                    -(temp1 * temp1 + temp2 * temp2),
                )
            } else {
                let mut fjs = 0.0;
                let mut js2 = 0.0;
                for i in 0..m {
                    let mut v = 0.0;
                    for j in 0..n {
                        v += jac[j * m + i] * s[j];
                    }
                    fjs += fvec[i] * v;
                    js2 += v * v;
                }
                let f2 = fnorm * fnorm;
                (-(2.0 * fjs + js2) / f2, fjs / f2)
            };
            let ratio = if prered != 0.0 { actred / prered } else { 0.0 };
            if ratio <= 0.25 {
                let mut temp = if actred >= 0.0 {
                    0.5
                } else {
                    0.5 * dirder / (dirder + 0.5 * actred)
                };
                if 0.1 * fnorm1 >= fnorm || temp < 0.1 || !temp.is_finite() {
                    temp = 0.1;
                }
                delta = temp * delta.min(pnorm / 0.1);
                par /= temp;
            } else if par == 0.0 || ratio >= 0.75 {
                delta = pnorm / 0.5;
                par *= 0.5;
            }
            if ratio >= 1e-4 {
                x.copy_from_slice(&trial);
                let wa: Vec<f64> = (0..n).map(|j| diag[j] * x[j]).collect();
                xnorm = enorm(&wa);
                fvec.copy_from_slice(&fnew);
                fnorm = fnorm1;
                successes += 1;
            }
            let ftol_ok = actred.abs() <= opts.ftol && prered <= opts.ftol && 0.5 * ratio <= 1.0;
            let xtol_ok = delta <= opts.xtol * xnorm;
            if ftol_ok {
                stop = LmStop::Ftol;
                break 'outer;
            }
            if xtol_ok {
                stop = LmStop::Xtol;
                break 'outer;
            }
            if actred.abs() <= epsmch && prered <= epsmch && 0.5 * ratio <= 1.0 {
                stop = LmStop::Ftol;
                break 'outer;
            }
            if delta <= epsmch * xnorm {
                stop = LmStop::Xtol;
                break 'outer;
            }
            if gnorm <= epsmch {
                stop = LmStop::Gtol;
                break 'outer;
            }
            if fnorm == 0.0 {
                stop = LmStop::Ftol;
                break 'outer;
            }
            if nfev > 20 * opts.max_iter + 100 {
                stop = LmStop::MaxIter;
                break 'outer;
            }
            if ratio >= 1e-4 {
                break;
            }
        }
    }
    // final residuals at x
    if !f.eval(&x, &mut fvec, None) {
        stop = LmStop::NotFinite;
    }
    let mut ss = sum_sq(&fvec);
    if opts.polish && stop.converged() {
        let (px, _, pss) = polish(f, &x, &fvec, ss, opts);
        x = px;
        ss = pss;
    }
    LmOutcome {
        at_bound: at_bound(&x),
        p: x,
        ss,
        iterations: iter,
        nfev,
        stop,
    }
}

/// Newton polishing over the parameters not on a bound: H d = -g with g = J'r accumulated
/// in compensated arithmetic and H = J'J + sum_i r_i grad^2 f_i, the second derivatives
/// from central differences of the (exact) Jacobian. A step is accepted while the
/// equilibrated gradient norm decreases, the relative step is below 1e-3 and the sum of
/// squares does not rise by more than 1e-10 relative (its rounding noise, about
/// eps |y|/|r| relative, can exceed 1e-13 on small-residual sets). Converges quadratically from the LM solution, so the parameters reach the
/// accuracy the gradient can resolve (about eps cond), not the sqrt(eps) that a
/// sum-of-squares acceptance test can resolve.
fn polish(
    f: &dyn Residuals,
    x0: &[f64],
    r0: &[f64],
    ss0: f64,
    opts: &LmOptions,
) -> (Vec<f64>, Vec<f64>, f64) {
    let n = x0.len();
    let m = r0.len();
    let mut x = x0.to_vec();
    let mut r = r0.to_vec();
    let mut ss = ss0;
    let mut jac = vec![0.0; m * n];
    let mut rt = vec![0.0; m];
    let free: Vec<usize> = (0..n)
        .filter(|&j| x[j] > opts.lower[j] && x[j] < opts.upper[j])
        .collect();
    let nf = free.len();
    if nf == 0 || ss == 0.0 {
        return (x, r, ss);
    }
    // gradient over the free parameters, and J'J
    let grad_at = |x: &[f64], r: &mut [f64], jac: &mut [f64]| -> Option<Vec<f64>> {
        if !f.eval(x, r, Some(jac)) || jac.iter().any(|v| !v.is_finite()) {
            return None;
        }
        Some(
            free.iter()
                .map(|&j| {
                    let mut a = Acc::default();
                    for i in 0..m {
                        a.add_prod(jac[j * m + i], r[i]);
                    }
                    a.value()
                })
                .collect(),
        )
    };
    let Some(mut g) = grad_at(&x, &mut r, &mut jac) else {
        return (x, r, ss);
    };
    let mut gnorm = f64::INFINITY;
    let mut jp = vec![0.0; m * n];
    let mut jm = vec![0.0; m * n];
    let mut rr = vec![0.0; m];
    for _ in 0..12 {
        // Hessian
        let mut h = vec![0.0; nf * nf];
        for a in 0..nf {
            for b in 0..=a {
                let (ja, jb) = (free[a], free[b]);
                let mut acc = Acc::default();
                for i in 0..m {
                    acc.add_prod(jac[ja * m + i], jac[jb * m + i]);
                }
                h[a * nf + b] = acc.value();
            }
        }
        for b in 0..nf {
            let jb = free[b];
            let hstep = f64::EPSILON.cbrt() * x[jb].abs().max(1e-3);
            let mut xp = x.clone();
            xp[jb] += hstep;
            let mut xm = x.clone();
            xm[jb] -= hstep;
            if !f.eval(&xp, &mut rr, Some(&mut jp)) || !f.eval(&xm, &mut rr, Some(&mut jm)) {
                return (x, r, ss);
            }
            let dh = xp[jb] - xm[jb];
            for a in b..nf {
                let ja = free[a];
                let mut s = 0.0;
                for i in 0..m {
                    s += r[i] * (jp[ja * m + i] - jm[ja * m + i]) / dh;
                }
                h[a * nf + b] += s;
            }
        }
        // equilibrated Cholesky of H
        let dsc: Vec<f64> = (0..nf).map(|a| 1.0 / h[a * nf + a].abs().sqrt()).collect();
        if dsc.iter().any(|d| !d.is_finite()) {
            break;
        }
        let gs: f64 = g
            .iter()
            .zip(&dsc)
            .map(|(a, d)| (a * d) * (a * d))
            .sum::<f64>()
            .sqrt();
        if !(gs < gnorm) {
            break;
        }
        gnorm = gs;
        let mut l = vec![0.0; nf * nf];
        let mut ok = true;
        for a in 0..nf {
            for b in 0..=a {
                let mut s = h[a * nf + b] * dsc[a] * dsc[b];
                for k in 0..b {
                    s -= l[a * nf + k] * l[b * nf + k];
                }
                if a == b {
                    if !(s > 1e-14) {
                        ok = false;
                        break;
                    }
                    l[a * nf + a] = s.sqrt();
                } else {
                    l[a * nf + b] = s / l[b * nf + b];
                }
            }
            if !ok {
                break;
            }
        }
        if !ok {
            break;
        }
        let mut z: Vec<f64> = (0..nf).map(|a| -g[a] * dsc[a]).collect();
        for a in 0..nf {
            for k in 0..a {
                z[a] -= l[a * nf + k] * z[k];
            }
            z[a] /= l[a * nf + a];
        }
        for a in (0..nf).rev() {
            for k in (a + 1)..nf {
                z[a] -= l[k * nf + a] * z[k];
            }
            z[a] /= l[a * nf + a];
        }
        let mut xt = x.clone();
        let mut rel: f64 = 0.0;
        for a in 0..nf {
            let j = free[a];
            let v = clamp(x[j] + z[a] * dsc[a], opts.lower[j], opts.upper[j]);
            rel = rel.max(((v - x[j]) / x[j].abs().max(1e-300)).abs());
            xt[j] = v;
        }
        if rel == 0.0 {
            break;
        }
        let mut jt = vec![0.0; m * n];
        let Some(gt) = grad_at(&xt, &mut rt, &mut jt) else {
            break;
        };
        let sst = sum_sq(&rt);
        let gts: f64 = gt
            .iter()
            .zip(&dsc)
            .map(|(a, d)| (a * d) * (a * d))
            .sum::<f64>()
            .sqrt();
        if !(gts < gs) || sst > ss * (1.0 + 1e-10) || rel > 1e-3 {
            break;
        }
        x = xt;
        std::mem::swap(&mut r, &mut rt);
        jac = jt;
        g = gt;
        ss = sst;
        if rel <= 2.0 * f64::EPSILON {
            break;
        }
    }
    (x, r, ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rosen;
    impl Residuals for Rosen {
        fn m(&self) -> usize {
            2
        }
        fn eval(&self, p: &[f64], r: &mut [f64], jac: Option<&mut [f64]>) -> bool {
            r[0] = 10.0 * (p[1] - p[0] * p[0]);
            r[1] = 1.0 - p[0];
            if let Some(j) = jac {
                j[0] = -20.0 * p[0];
                j[1] = -1.0;
                j[2] = 10.0;
                j[3] = 0.0;
            }
            true
        }
    }

    #[test]
    fn rosenbrock() {
        let o = minimize(&Rosen, &[-1.2, 1.0], &LmOptions::new(2));
        assert!(o.stop.converged(), "{:?}", o.stop);
        assert!((o.p[0] - 1.0).abs() < 1e-12 && (o.p[1] - 1.0).abs() < 1e-12);
        let mut g = LmOptions::new(2);
        g.geodesic = true;
        let o = minimize(&Rosen, &[-1.2, 1.0], &g);
        assert!((o.p[0] - 1.0).abs() < 1e-12, "{:?}", o.p);
    }

    #[test]
    fn rosenbrock_bounded() {
        let mut o = LmOptions::new(2);
        o.upper = vec![0.5, f64::INFINITY];
        let r = minimize(&Rosen, &[-1.2, 1.0], &o);
        assert!((r.p[0] - 0.5).abs() < 1e-14, "{:?}", r.p);
        assert!((r.p[1] - 0.25).abs() < 1e-10, "{:?}", r.p);
        assert!(r.at_bound[0]);
    }
}
