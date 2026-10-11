//! Small statistics the plot engine needs to draw summaries: means and spreads, the t and
//! normal quantiles for confidence intervals, kernel densities, a seeded generator for
//! jitter and bootstrap, Kaplan–Meier, ROC, average-linkage clustering and correlations.
//!
//! These exist so the plot crate stands alone (it must not depend on the analysis crate,
//! which depends on nothing graphical). Analyses that report numbers use `dioptra-stats`;
//! the graph only draws them.

use crate::detmath::DetMath;
/// Arithmetic mean of the finite values (NaN when there are none).
pub fn mean(v: &[f64]) -> f64 {
    let (mut s, mut n) = (0.0, 0usize);
    for x in v.iter().filter(|x| x.is_finite()) {
        s += x;
        n += 1;
    }
    if n == 0 {
        f64::NAN
    } else {
        s / n as f64
    }
}

/// Sample standard deviation (n − 1) of the finite values.
pub fn sd(v: &[f64]) -> f64 {
    let m = mean(v);
    let f: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if f.len() < 2 {
        return f64::NAN;
    }
    let ss: f64 = f.iter().map(|x| (x - m) * (x - m)).sum();
    (ss / (f.len() - 1) as f64).sqrt()
}

/// Finite values, sorted ascending.
pub fn sorted(v: &[f64]) -> Vec<f64> {
    let mut f: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    f.sort_by(f64::total_cmp);
    f
}

/// Quantile of sorted data, Hyndman–Fan type 7 (the R and NumPy default).
pub fn quantile_sorted(s: &[f64], p: f64) -> f64 {
    if s.is_empty() {
        return f64::NAN;
    }
    let h = (s.len() - 1) as f64 * p.clamp(0.0, 1.0);
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(s.len() - 1);
    s[lo] + (h - lo as f64) * (s[hi] - s[lo])
}

/// Median of the finite values.
pub fn median(v: &[f64]) -> f64 {
    quantile_sorted(&sorted(v), 0.5)
}

/// Natural log of the gamma function.
fn ln_gamma(x: f64) -> f64 {
    libm::lgamma(x)
}

/// Continued fraction for the incomplete beta (modified Lentz).
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    let tiny = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..400 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// Regularised incomplete beta I_x(a, b).
pub fn inc_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let lbt = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln_d() + b * (1.0 - x).ln_d();
    let bt = lbt.exp_d();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * beta_cf(a, b, x) / a
    } else {
        1.0 - bt * beta_cf(b, a, 1.0 - x) / b
    }
}

/// Student t cumulative distribution.
pub fn t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let tail = 0.5 * inc_beta(df / 2.0, 0.5, x);
    if t >= 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

/// Student t quantile by bisection on the CDF (accurate to ~1e-12; used for error bars).
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if p.is_nan() || p <= 0.0 || p >= 1.0 || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    if p < 0.5 {
        return -t_quantile(1.0 - p, df);
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    while t_cdf(hi, df) < p {
        hi *= 2.0;
        if hi > 1e12 {
            return f64::INFINITY;
        }
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if t_cdf(mid, df) < p {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 1e-13 * hi.max(1.0) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// Standard normal cumulative distribution.
pub fn norm_cdf(z: f64) -> f64 {
    0.5 * libm::erfc(-z / std::f64::consts::SQRT_2)
}

/// Standard normal quantile (Acklam's rational approximation refined by one Newton step).
pub fn norm_quantile(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let pl = 0.02425;
    let x = if p < pl {
        let q = (-2.0 * p.ln_d()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln_d()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    // One Halley step.
    let e = norm_cdf(x) - p;
    let u = e * (2.0 * std::f64::consts::PI).sqrt() * (x * x / 2.0).exp_d();
    x - u / (1.0 + x * u / 2.0)
}

/// Silverman's rule-of-thumb bandwidth: 0.9 · min(SD, IQR/1.34) · n^(−1/5).
pub fn silverman(v: &[f64]) -> f64 {
    let s = sorted(v);
    let n = s.len();
    if n < 2 {
        return 1.0;
    }
    let sdv = sd(&s);
    let iqr = quantile_sorted(&s, 0.75) - quantile_sorted(&s, 0.25);
    let spread = if iqr > 0.0 { sdv.min(iqr / 1.34) } else { sdv };
    let h = 0.9 * spread * (n as f64).powf_d(-0.2);
    if h > 0.0 && h.is_finite() {
        h
    } else {
        (s[n - 1] - s[0]).abs().max(1.0) * 0.1
    }
}

/// Gaussian kernel density of `v` at each grid point.
pub fn kde(v: &[f64], bw: f64, grid: &[f64]) -> Vec<f64> {
    let s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    let n = s.len() as f64;
    let c = 1.0 / (n * bw * (2.0 * std::f64::consts::PI).sqrt());
    grid.iter()
        .map(|g| {
            s.iter()
                .map(|x| {
                    let z = (g - x) / bw;
                    (-0.5 * z * z).exp_d()
                })
                .sum::<f64>()
                * c
        })
        .collect()
}

/// A density over an even grid: `(grid, density)`. With `truncate`, the grid spans the
/// data range; otherwise it extends three bandwidths beyond it.
pub fn density_curve(
    v: &[f64],
    bw: Option<f64>,
    truncate: bool,
    points: usize,
) -> (Vec<f64>, Vec<f64>) {
    let s = sorted(v);
    if s.is_empty() {
        return (vec![], vec![]);
    }
    let h = bw.filter(|b| *b > 0.0).unwrap_or_else(|| silverman(&s));
    let (mut a, mut b) = (s[0], s[s.len() - 1]);
    if !truncate {
        a -= 3.0 * h;
        b += 3.0 * h;
    }
    if b <= a {
        a -= h;
        b += h;
    }
    let m = points.max(2);
    let grid: Vec<f64> = (0..m)
        .map(|i| a + (b - a) * i as f64 / (m - 1) as f64)
        .collect();
    let d = kde(&s, h, &grid);
    (grid, d)
}

/// SplitMix64: a small, fast, seedable generator. Output is identical on every platform.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// Seeded generator.
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform index below `n` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_u64() as u128 * n as u128) >> 64) as usize
    }
}

/// Result of a bootstrap of the difference of two means.
#[derive(Debug, Clone, PartialEq)]
pub struct Bootstrap {
    /// Observed difference (test − control).
    pub estimate: f64,
    /// Lower 95% bound (bias-corrected and accelerated).
    pub lo: f64,
    /// Upper 95% bound.
    pub hi: f64,
    /// Bootstrap replicates, sorted.
    pub replicates: Vec<f64>,
}

/// Bias-corrected and accelerated (BCa) bootstrap 95% CI of mean(test) − mean(control),
/// resampling each group independently (Efron 1987; the DABEST default).
pub fn bootstrap_mean_diff(
    control: &[f64],
    test: &[f64],
    resamples: usize,
    seed: u64,
) -> Option<Bootstrap> {
    let a: Vec<f64> = control.iter().copied().filter(|v| v.is_finite()).collect();
    let b: Vec<f64> = test.iter().copied().filter(|v| v.is_finite()).collect();
    if a.len() < 2 || b.len() < 2 {
        return None;
    }
    let est = mean(&b) - mean(&a);
    let mut rng = Rng::new(seed);
    let n = resamples.max(100);
    let mut reps = Vec::with_capacity(n);
    for _ in 0..n {
        let ma = (0..a.len()).map(|_| a[rng.below(a.len())]).sum::<f64>() / a.len() as f64;
        let mb = (0..b.len()).map(|_| b[rng.below(b.len())]).sum::<f64>() / b.len() as f64;
        reps.push(mb - ma);
    }
    reps.sort_by(f64::total_cmp);
    let below = reps.iter().filter(|r| **r < est).count() as f64;
    let ties = reps.iter().filter(|r| **r == est).count() as f64;
    let z0 = norm_quantile(((below + 0.5 * ties) / n as f64).clamp(1e-10, 1.0 - 1e-10));
    // Jackknife over both groups for the acceleration.
    let (sa, sb) = (a.iter().sum::<f64>(), b.iter().sum::<f64>());
    let mut jack = Vec::with_capacity(a.len() + b.len());
    for x in &a {
        jack.push(sb / b.len() as f64 - (sa - x) / (a.len() - 1) as f64);
    }
    for x in &b {
        jack.push((sb - x) / (b.len() - 1) as f64 - sa / a.len() as f64);
    }
    let jm = mean(&jack);
    let (num, den) = jack.iter().fold((0.0, 0.0), |(n3, d2), j| {
        let d = jm - j;
        (n3 + d * d * d, d2 + d * d)
    });
    let acc = if den > 0.0 {
        num / (6.0 * den.powf_d(1.5))
    } else {
        0.0
    };
    let adj = |alpha: f64| {
        let z = norm_quantile(alpha);
        norm_cdf(z0 + (z0 + z) / (1.0 - acc * (z0 + z)))
    };
    let (p_lo, p_hi) = (adj(0.025), adj(0.975));
    Some(Bootstrap {
        estimate: est,
        lo: quantile_sorted(&reps, p_lo),
        hi: quantile_sorted(&reps, p_hi),
        replicates: reps,
    })
}

/// A Kaplan–Meier estimate.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KaplanMeier {
    /// Step times, starting at 0 (or the earliest time when negative).
    pub times: Vec<f64>,
    /// Survival from each time until the next.
    pub survival: Vec<f64>,
    /// Lower 95% bound (exponential Greenwood, log(−log) scale).
    pub lo: Vec<f64>,
    /// Upper 95% bound.
    pub hi: Vec<f64>,
    /// Censored observations as (time, survival there).
    pub censored: Vec<(f64, f64)>,
    /// All observed times (for numbers at risk).
    pub observed: Vec<f64>,
}

impl KaplanMeier {
    /// Number still at risk at time `t` (observed time ≥ t).
    pub fn at_risk(&self, t: f64) -> usize {
        self.observed.iter().filter(|o| **o >= t).count()
    }

    /// Survival at time `t`.
    pub fn at(&self, t: f64) -> f64 {
        let mut s = 1.0;
        for (ti, si) in self.times.iter().zip(&self.survival) {
            if *ti <= t {
                s = *si;
            } else {
                break;
            }
        }
        s
    }
}

/// Kaplan–Meier estimator for (time, event) pairs; an event code of 0 means censored.
pub fn kaplan_meier(time: &[f64], event: &[f64]) -> KaplanMeier {
    let mut obs: Vec<(f64, bool)> = time
        .iter()
        .zip(event)
        .filter(|(t, e)| t.is_finite() && e.is_finite())
        .map(|(t, e)| (*t, *e != 0.0))
        .collect();
    obs.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let start = obs.first().map_or(0.0, |o| o.0.min(0.0));
    let mut km = KaplanMeier {
        times: vec![start],
        survival: vec![1.0],
        lo: vec![1.0],
        hi: vec![1.0],
        observed: obs.iter().map(|o| o.0).collect(),
        ..Default::default()
    };
    let z = 1.959_963_984_540_054;
    let (mut s, mut gw) = (1.0f64, 0.0f64);
    let mut i = 0;
    while i < obs.len() {
        let t = obs[i].0;
        let at_risk = obs.len() - i;
        let mut d = 0usize;
        let mut c = 0usize;
        while i < obs.len() && obs[i].0 == t {
            if obs[i].1 {
                d += 1;
            } else {
                c += 1;
            }
            i += 1;
        }
        if d > 0 {
            let (n, df) = (at_risk as f64, d as f64);
            s *= 1.0 - df / n;
            if n > df {
                gw += df / (n * (n - df));
            }
            let (lo, hi) = if s > 0.0 && s < 1.0 {
                let ls = s.ln_d();
                let a = z * (gw / (ls * ls)).sqrt();
                (s.powf_d(a.exp_d()), s.powf_d((-a).exp_d()))
            } else {
                (s, s)
            };
            km.times.push(t);
            km.survival.push(s);
            km.lo.push(lo);
            km.hi.push(hi);
        }
        for _ in 0..c {
            km.censored.push((t, s));
        }
    }
    // Extend the last step to the last observation.
    if let Some(last) = obs.last() {
        if last.0 > *km.times.last().expect("start") {
            km.times.push(last.0);
            km.survival.push(s);
            km.lo.push(*km.lo.last().expect("lo"));
            km.hi.push(*km.hi.last().expect("hi"));
        }
    }
    km
}

/// An empirical ROC curve: `(false positive rate, true positive rate)` from (0,0) to (1,1)
/// and the area under it. Higher values are taken to indicate the patient group.
pub fn roc(controls: &[f64], patients: &[f64]) -> (Vec<(f64, f64)>, f64) {
    let c = sorted(controls);
    let p = sorted(patients);
    if c.is_empty() || p.is_empty() {
        return (vec![], f64::NAN);
    }
    let mut thresholds: Vec<f64> = c.iter().chain(&p).copied().collect();
    thresholds.sort_by(|a, b| b.total_cmp(a));
    thresholds.dedup();
    let (nc, np) = (c.len() as f64, p.len() as f64);
    let mut pts = vec![(0.0, 0.0)];
    for t in thresholds {
        let fp = c.iter().filter(|v| **v >= t).count() as f64 / nc;
        let tp = p.iter().filter(|v| **v >= t).count() as f64 / np;
        pts.push((fp, tp));
    }
    if pts.last() != Some(&(1.0, 1.0)) {
        pts.push((1.0, 1.0));
    }
    let auc = pts
        .windows(2)
        .map(|w| (w[1].0 - w[0].0) * (w[1].1 + w[0].1) / 2.0)
        .sum();
    (pts, auc)
}

/// Pearson correlation over pairs where both values are finite.
pub fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b)
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| (*x, *y))
        .collect();
    if pairs.len() < 3 {
        return f64::NAN;
    }
    let n = pairs.len() as f64;
    let (mx, my) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in &pairs {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
    }
    sxy / (sxx * syy).sqrt()
}

/// Ordinary least squares line `(intercept, slope)` over finite pairs.
pub fn linear_fit(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
    let pairs: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .map(|(a, b)| (*a, *b))
        .collect();
    if pairs.len() < 2 {
        return None;
    }
    let n = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = pairs.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx == 0.0 {
        return None;
    }
    let sxy: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let b = sxy / sxx;
    Some((my - b * mx, b))
}

/// One merge in an agglomerative clustering: clusters `a` and `b` (indices: leaves are
/// `0..n`, merge `k` creates cluster `n + k`) joined at `height`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Merge {
    /// First cluster.
    pub a: usize,
    /// Second cluster.
    pub b: usize,
    /// Average Euclidean distance at which they joined.
    pub height: f64,
}

/// Average-linkage (UPGMA) clustering of the rows of `m` by Euclidean distance over
/// pairwise-complete columns. Returns the merges and the leaf order for drawing.
#[allow(clippy::needless_range_loop)]
pub fn cluster(m: &[Vec<f64>]) -> (Vec<Merge>, Vec<usize>) {
    let n = m.len();
    if n < 2 {
        return (vec![], (0..n).collect());
    }
    let dist = |i: usize, j: usize| {
        let (mut s, mut k) = (0.0, 0usize);
        for (a, b) in m[i].iter().zip(&m[j]) {
            if a.is_finite() && b.is_finite() {
                s += (a - b) * (a - b);
                k += 1;
            }
        }
        if k == 0 {
            0.0
        } else {
            (s * m[i].len() as f64 / k as f64).sqrt()
        }
    };
    let mut d = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in i + 1..n {
            let v = dist(i, j);
            d[i][j] = v;
            d[j][i] = v;
        }
    }
    // active clusters: (id, members, size)
    let mut active: Vec<(usize, usize)> = (0..n).map(|i| (i, 1)).collect();
    // distance between active clusters by position in `active`
    let mut cd = d;
    let mut merges = Vec::with_capacity(n - 1);
    while active.len() > 1 {
        let (mut bi, mut bj, mut best) = (0, 1, f64::INFINITY);
        for i in 0..active.len() {
            for j in i + 1..active.len() {
                if cd[i][j] < best {
                    best = cd[i][j];
                    bi = i;
                    bj = j;
                }
            }
        }
        let (ia, sa) = active[bi];
        let (ib, sb) = active[bj];
        merges.push(Merge {
            a: ia,
            b: ib,
            height: best,
        });
        let new_id = n + merges.len() - 1;
        // new distances (UPGMA)
        let newrow: Vec<f64> = (0..active.len())
            .map(|k| (cd[bi][k] * sa as f64 + cd[bj][k] * sb as f64) / (sa + sb) as f64)
            .collect();
        active[bi] = (new_id, sa + sb);
        for k in 0..active.len() {
            cd[bi][k] = newrow[k];
            cd[k][bi] = newrow[k];
        }
        cd[bi][bi] = 0.0;
        active.remove(bj);
        cd.remove(bj);
        for row in cd.iter_mut() {
            row.remove(bj);
        }
    }
    // Leaf order: depth-first from the root.
    let mut order = Vec::with_capacity(n);
    let mut stack = vec![n + merges.len() - 1];
    while let Some(c) = stack.pop() {
        if c < n {
            order.push(c);
        } else {
            let mg = merges[c - n];
            stack.push(mg.b);
            stack.push(mg.a);
        }
    }
    (merges, order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_quantiles_match_tables() {
        assert!((t_quantile(0.975, 1.0) - 12.706_204_736).abs() < 1e-6);
        assert!((t_quantile(0.975, 5.0) - 2.570_581_836).abs() < 1e-8);
        assert!((t_quantile(0.975, 30.0) - 2.042_272_456).abs() < 1e-8);
        assert!((norm_quantile(0.975) - 1.959_963_985).abs() < 1e-9);
    }

    #[test]
    fn km_textbook() {
        // times 1,2,2(c),3,4 ; events 1,1,0,1,0
        let km = kaplan_meier(&[1., 2., 2., 3., 4.], &[1., 1., 0., 1., 0.]);
        assert_eq!(km.survival[1], 0.8);
        assert!((km.survival[2] - 0.6).abs() < 1e-12);
        assert!((km.survival[3] - 0.3).abs() < 1e-12);
        assert_eq!(km.at_risk(2.5), 2);
    }

    #[test]
    fn roc_auc_is_mann_whitney() {
        let (_, auc) = roc(&[1., 2., 3.], &[2., 4., 5.]);
        // P(p > c) + 0.5 P(p = c) = (1.5 + 3 + 3) / 9
        assert!((auc - 7.5 / 9.0).abs() < 1e-12);
    }

    #[test]
    fn clustering_groups_near_rows() {
        let m = vec![
            vec![0.0, 0.0],
            vec![10.0, 10.0],
            vec![0.1, 0.0],
            vec![10.0, 9.9],
        ];
        let (merges, order) = cluster(&m);
        assert_eq!(merges.len(), 3);
        let p0 = order.iter().position(|v| *v == 0).unwrap();
        let p2 = order.iter().position(|v| *v == 2).unwrap();
        assert_eq!((p0 as i64 - p2 as i64).abs(), 1);
    }

    #[test]
    fn bootstrap_is_deterministic() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0];
        let b = [3.0, 4.0, 5.0, 6.0, 7.5];
        let x = bootstrap_mean_diff(&a, &b, 2000, 7).unwrap();
        let y = bootstrap_mean_diff(&a, &b, 2000, 7).unwrap();
        assert_eq!(x, y);
        assert!(x.lo < x.estimate && x.estimate < x.hi);
    }
}
