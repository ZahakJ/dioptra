//! Dense linear least squares for the regression and ANOVA modules.
//!
//! The design is equilibrated (each column divided by its Euclidean norm), factored by
//! Householder QR with column pivoting (Businger & Golub 1965; Golub & Van Loan 2013,
//! §5.4.2), and the solution is refined on the augmented system
//! `[I X; X' 0] [r; b] = [y; 0]` with residuals accumulated in double-double arithmetic
//! (Björck 1967, "Iterative refinement of linear least squares solutions I", BIT 7:257;
//! Ogita, Rump & Oishi 2005 `Dot2` for the compensated dot products). Refinement on the
//! augmented system converges whenever cond(X) eps < 1 and returns the least-squares
//! solution and residual vector of the (double) design to nearly full precision, which is
//! what the NIST StRD Filip and Wampler sets require. The normal equations are never
//! formed. The unscaled covariance (X'X)^{-1} is obtained column by column from the same
//! refined augmented solver (right-hand side `[0; -e_j]`).
//!
//! A design may carry a low-order part (`lo`), so that columns such as x^10 can be
//! represented to about 32 digits; the refinement residuals then use hi + lo.

/// Error-free sum (Knuth 1969 TwoSum).
#[inline]
pub(crate) fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// Compensated accumulator (Ogita, Rump & Oishi 2005, `Sum2`/`Dot2`): the result is as
/// accurate as if computed in twice the working precision.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Acc {
    hi: f64,
    lo: f64,
}

impl Acc {
    #[inline]
    pub(crate) fn add(&mut self, x: f64) {
        let (s, e) = two_sum(self.hi, x);
        self.hi = s;
        self.lo += e;
    }
    #[inline]
    pub(crate) fn add_prod(&mut self, a: f64, b: f64) {
        let p = a * b;
        let e = a.mul_add(b, -p);
        self.add(p);
        self.lo += e;
    }
    #[inline]
    pub(crate) fn value(&self) -> f64 {
        self.hi + self.lo
    }
}

/// Exact product a*b as (hi, lo).
#[inline]
pub(crate) fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    (p, a.mul_add(b, -p))
}

/// Double-double product (ah + al)(bh + bl) -> (hi, lo).
#[inline]
pub(crate) fn dd_mul(ah: f64, al: f64, bh: f64, bl: f64) -> (f64, f64) {
    let (p, e) = two_prod(ah, bh);
    let e = e + (ah * bl + al * bh);
    two_sum(p, e)
}

/// A column-major design matrix, optionally with a low-order part per entry.
#[derive(Clone, Debug)]
pub(crate) struct Design {
    pub n: usize,
    pub p: usize,
    pub hi: Vec<f64>,
    pub lo: Option<Vec<f64>>,
}

impl Design {
    pub(crate) fn from_columns(cols: &[Vec<f64>]) -> Self {
        let n = cols.first().map_or(0, |c| c.len());
        let mut hi = Vec::with_capacity(n * cols.len());
        for c in cols {
            hi.extend_from_slice(c);
        }
        Design {
            n,
            p: cols.len(),
            hi,
            lo: None,
        }
    }
    #[inline]
    pub(crate) fn col(&self, j: usize) -> &[f64] {
        &self.hi[j * self.n..(j + 1) * self.n]
    }
    #[inline]
    fn col_lo(&self, j: usize) -> Option<&[f64]> {
        self.lo.as_ref().map(|l| &l[j * self.n..(j + 1) * self.n])
    }
    /// The sub-design with the given columns.
    pub(crate) fn select(&self, cols: &[usize]) -> Design {
        let mut hi = Vec::with_capacity(self.n * cols.len());
        let mut lo = self
            .lo
            .as_ref()
            .map(|_| Vec::with_capacity(self.n * cols.len()));
        for &j in cols {
            hi.extend_from_slice(self.col(j));
            if let (Some(l), Some(src)) = (lo.as_mut(), self.col_lo(j)) {
                l.extend_from_slice(src);
            }
        }
        Design {
            n: self.n,
            p: cols.len(),
            hi,
            lo,
        }
    }
}

/// Householder QR of an n x p column-major matrix with optional column pivoting.
struct Qr {
    n: usize,
    p: usize,
    /// Householder vectors below/on the diagonal (v_k stored in rows k..n of column k),
    /// R strictly above the diagonal.
    a: Vec<f64>,
    tau: Vec<f64>,
    rdiag: Vec<f64>,
    /// Column j of the factored matrix is column perm[j] of the input.
    perm: Vec<usize>,
}

impl Qr {
    fn new(mut a: Vec<f64>, n: usize, p: usize, pivot: bool) -> Qr {
        let mut perm: Vec<usize> = (0..p).collect();
        let mut tau = vec![0.0; p];
        let mut rdiag = vec![0.0; p];
        let steps = p.min(n);
        for k in 0..steps {
            if pivot {
                // Choose the remaining column with the largest norm (rows k..n), recomputed
                // exactly each step (p is small; no downdating error).
                let mut best = k;
                let mut best_norm = -1.0;
                for j in k..p {
                    let c = &a[j * n + k..(j + 1) * n];
                    let s = norm2(c);
                    if s > best_norm {
                        best_norm = s;
                        best = j;
                    }
                }
                if best != k {
                    for i in 0..n {
                        a.swap(k * n + i, best * n + i);
                    }
                    perm.swap(k, best);
                }
            }
            let (head, tail) = a.split_at_mut((k + 1) * n);
            let col = &mut head[k * n + k..(k + 1) * n];
            let norm = norm2(col);
            if norm == 0.0 {
                tau[k] = 0.0;
                rdiag[k] = 0.0;
                continue;
            }
            let x0 = col[0];
            let alpha = if x0 >= 0.0 { -norm } else { norm };
            col[0] = x0 - alpha;
            let vtv: f64 = col.iter().map(|v| v * v).sum();
            let t = 2.0 / vtv;
            tau[k] = t;
            rdiag[k] = alpha;
            for j in (k + 1)..p {
                let cj = &mut tail[(j - k - 1) * n + k..(j - k) * n];
                let s: f64 = col.iter().zip(cj.iter()).map(|(v, c)| v * c).sum();
                let f = t * s;
                for (c, v) in cj.iter_mut().zip(col.iter()) {
                    *c -= f * v;
                }
            }
        }
        Qr {
            n,
            p,
            a,
            tau,
            rdiag,
            perm,
        }
    }

    fn r(&self, i: usize, j: usize) -> f64 {
        if i == j {
            self.rdiag[i]
        } else {
            self.a[j * self.n + i]
        }
    }

    /// y <- Q' y
    fn qt_mul(&self, y: &mut [f64]) {
        for k in 0..self.p.min(self.n) {
            let t = self.tau[k];
            if t == 0.0 {
                continue;
            }
            let v = &self.a[k * self.n + k..(k + 1) * self.n];
            let s: f64 = v.iter().zip(&y[k..]).map(|(a, b)| a * b).sum();
            let f = t * s;
            for (yi, vi) in y[k..].iter_mut().zip(v) {
                *yi -= f * vi;
            }
        }
    }

    /// y <- Q y
    fn q_mul(&self, y: &mut [f64]) {
        for k in (0..self.p.min(self.n)).rev() {
            let t = self.tau[k];
            if t == 0.0 {
                continue;
            }
            let v = &self.a[k * self.n + k..(k + 1) * self.n];
            let s: f64 = v.iter().zip(&y[k..]).map(|(a, b)| a * b).sum();
            let f = t * s;
            for (yi, vi) in y[k..].iter_mut().zip(v) {
                *yi -= f * vi;
            }
        }
    }

    /// Solve R z = b (first p entries), in place.
    #[allow(clippy::needless_range_loop)]
    fn solve_r(&self, b: &mut [f64]) {
        for i in (0..self.p).rev() {
            let mut s = b[i];
            for j in (i + 1)..self.p {
                s -= self.r(i, j) * b[j];
            }
            b[i] = s / self.rdiag[i];
        }
    }

    /// Solve R' u = b, in place.
    #[allow(clippy::needless_range_loop)]
    fn solve_rt(&self, b: &mut [f64]) {
        for i in 0..self.p {
            let mut s = b[i];
            for j in 0..i {
                s -= self.r(j, i) * b[j];
            }
            b[i] = s / self.rdiag[i];
        }
    }
}

fn norm2(v: &[f64]) -> f64 {
    let scale = v.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    if scale == 0.0 || !scale.is_finite() {
        return scale;
    }
    let s: f64 = v.iter().map(|x| (x / scale) * (x / scale)).sum();
    scale * s.sqrt()
}

/// Relative threshold on |R_kk| / |R_11| of the equilibrated design below which a column is
/// declared aliased (linearly dependent on the columns before it in pivot order).
pub(crate) const RANK_TOL: f64 = 1e-11;

/// Columns of `d` kept and aliased, deciding in column order as R's `lm` does (a column is
/// aliased when its component orthogonal to the kept columns before it, by twice-iterated
/// modified Gram–Schmidt on the equilibrated design, has relative norm <= [`RANK_TOL`]).
/// Kept columns are returned in their original order.
pub(crate) fn rank_select(d: &Design) -> (Vec<usize>, Vec<usize>) {
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let mut kept = Vec::new();
    let mut aliased = Vec::new();
    for j in 0..d.p {
        let c = d.col(j);
        let s = norm2(c);
        if !(s > 0.0) || basis.len() >= d.n {
            aliased.push(j);
            continue;
        }
        let mut v: Vec<f64> = c.iter().map(|x| x / s).collect();
        for _ in 0..2 {
            for q in &basis {
                let dot: f64 = q.iter().zip(&v).map(|(a, b)| a * b).sum();
                for (vi, qi) in v.iter_mut().zip(q) {
                    *vi -= dot * qi;
                }
            }
        }
        let r = norm2(&v);
        if r > RANK_TOL {
            for x in v.iter_mut() {
                *x /= r;
            }
            basis.push(v);
            kept.push(j);
        } else {
            aliased.push(j);
        }
    }
    (kept, aliased)
}

/// Least-squares solver for a full-column-rank design.
pub(crate) struct LeastSquares<'a> {
    d: &'a Design,
    scale: Vec<f64>,
    qr: Qr,
}

/// Solution of a refined least-squares problem.
pub(crate) struct LsSolution {
    pub coef: Vec<f64>,
    pub resid: Vec<f64>,
    /// Residual sum of squares from the refined residual vector (compensated).
    pub rss: f64,
}

const MAX_REFINE: usize = 12;

impl<'a> LeastSquares<'a> {
    /// Factor `d` (assumed full column rank; use [`rank_select`] first).
    pub(crate) fn new(d: &'a Design) -> Self {
        let mut a = d.hi.clone();
        let mut scale = vec![1.0; d.p];
        for (j, sc) in scale.iter_mut().enumerate() {
            let c = &mut a[j * d.n..(j + 1) * d.n];
            let s = norm2(c);
            if s > 0.0 {
                *sc = s;
                for x in c.iter_mut() {
                    *x /= s;
                }
            }
        }
        let qr = Qr::new(a, d.n, d.p, true);
        LeastSquares { d, scale, qr }
    }

    /// One solve of the augmented system for corrections: dr + X db = f, X' dr = g.
    fn solve_aug(&self, f: &[f64], g: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let (n, p) = (self.d.n, self.d.p);
        let mut w = f.to_vec();
        self.qr.qt_mul(&mut w);
        // u1 = R'^{-1} P' D g  (Xs = X D, D = diag(1/scale))
        let mut u1: Vec<f64> = (0..p)
            .map(|k| {
                let j = self.qr.perm[k];
                g[j] / self.scale[j]
            })
            .collect();
        self.qr.solve_rt(&mut u1);
        let mut z: Vec<f64> = (0..p).map(|k| w[k] - u1[k]).collect();
        self.qr.solve_r(&mut z);
        let mut db = vec![0.0; p];
        for (&j, zk) in self.qr.perm.iter().zip(&z) {
            db[j] = zk / self.scale[j];
        }
        let mut dr = vec![0.0; n];
        dr[..p].copy_from_slice(&u1);
        dr[p..].copy_from_slice(&w[p..]);
        self.qr.q_mul(&mut dr);
        (dr, db)
    }

    /// Residuals of the augmented system in compensated arithmetic:
    /// (f - r - X b, g - X' r).
    fn residuals(&self, f: &[f64], g: &[f64], r: &[f64], b: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let (n, p) = (self.d.n, self.d.p);
        let mut fr = Vec::with_capacity(n);
        for i in 0..n {
            let mut acc = Acc::default();
            acc.add(f[i]);
            acc.add(-r[i]);
            for (j, &bj) in b.iter().enumerate() {
                acc.add_prod(-self.d.hi[j * n + i], bj);
                if let Some(lo) = &self.d.lo {
                    acc.add_prod(-lo[j * n + i], bj);
                }
            }
            fr.push(acc.value());
        }
        let mut gr = Vec::with_capacity(p);
        for (j, &gj) in g.iter().enumerate() {
            let mut acc = Acc::default();
            acc.add(gj);
            let c = self.d.col(j);
            for i in 0..n {
                acc.add_prod(-c[i], r[i]);
            }
            if let Some(lo) = self.d.col_lo(j) {
                for i in 0..n {
                    acc.add_prod(-lo[i], r[i]);
                }
            }
            gr.push(acc.value());
        }
        (fr, gr)
    }

    /// Refined solution of r + X b = f, X' r = g.
    fn solve_refined(&self, f: &[f64], g: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let (mut r, mut b) = self.solve_aug(f, g);
        let mut last = f64::INFINITY;
        for _ in 0..MAX_REFINE {
            let (fr, gr) = self.residuals(f, g, &r, &b);
            let (dr, db) = self.solve_aug(&fr, &gr);
            let mut change = 0.0f64;
            for (bj, dj) in b.iter_mut().zip(&db) {
                let rel = if *bj != 0.0 {
                    (dj / *bj).abs()
                } else {
                    dj.abs()
                };
                change = change.max(rel);
                *bj += dj;
            }
            for (ri, di) in r.iter_mut().zip(&dr) {
                *ri += di;
            }
            // Stop when converged to rounding level, or when refinement stalls.
            if change <= 2.0 * f64::EPSILON || change >= 0.5 * last {
                break;
            }
            last = change;
        }
        (r, b)
    }

    /// Least-squares fit of `y`.
    pub(crate) fn fit(&self, y: &[f64]) -> LsSolution {
        let g = vec![0.0; self.d.p];
        let (r, b) = self.solve_refined(y, &g);
        let mut acc = Acc::default();
        for &x in &r {
            acc.add_prod(x, x);
        }
        LsSolution {
            coef: b,
            resid: r,
            rss: acc.value(),
        }
    }

    /// (X'X)^{-1}, p x p row-major, each column refined.
    pub(crate) fn xtx_inverse(&self) -> Vec<f64> {
        let p = self.d.p;
        let f = vec![0.0; self.d.n];
        let mut out = vec![0.0; p * p];
        for j in 0..p {
            let mut g = vec![0.0; p];
            g[j] = -1.0;
            let (_, b) = self.solve_refined(&f, &g);
            for i in 0..p {
                out[i * p + j] = b[i];
            }
        }
        // symmetrise
        for i in 0..p {
            for j in (i + 1)..p {
                let m = 0.5 * (out[i * p + j] + out[j * p + i]);
                out[i * p + j] = m;
                out[j * p + i] = m;
            }
        }
        out
    }
}

/// Residual sum of squares of the least-squares fit of `y` on the columns `cols` of `d`
/// (rank-deficient column sets are reduced first), and the rank used.
pub(crate) fn rss_of(d: &Design, cols: &[usize], y: &[f64]) -> (f64, usize) {
    if cols.is_empty() {
        let mut acc = Acc::default();
        for &v in y {
            acc.add_prod(v, v);
        }
        return (acc.value(), 0);
    }
    let sub = d.select(cols);
    let (kept, _) = rank_select(&sub);
    let sub = sub.select(&kept);
    let ls = LeastSquares::new(&sub);
    (ls.fit(y).rss, kept.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_line() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y: Vec<f64> = x.iter().map(|v| 2.0 + 3.0 * v).collect();
        let d = Design::from_columns(&[vec![1.0; 5], x.to_vec()]);
        let ls = LeastSquares::new(&d);
        let s = ls.fit(&y);
        assert!((s.coef[0] - 2.0).abs() < 1e-15 && (s.coef[1] - 3.0).abs() < 1e-15);
        assert!(s.rss < 1e-28);
        let inv = ls.xtx_inverse();
        // X'X = [[5, 15], [15, 55]] -> inverse = [[1.1, -0.3], [-0.3, 0.1]]
        assert!((inv[0] - 1.1).abs() < 1e-15);
        assert!((inv[1] + 0.3).abs() < 1e-15);
        assert!((inv[3] - 0.1).abs() < 1e-15);
    }

    #[test]
    fn detects_aliasing() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![2.0, 4.0, 6.0, 8.0];
        let c = vec![1.0, 0.0, 1.0, 0.0];
        let d = Design::from_columns(&[a, b, c]);
        let (kept, aliased) = rank_select(&d);
        assert_eq!(kept.len(), 2);
        assert_eq!(aliased.len(), 1);
    }
}
