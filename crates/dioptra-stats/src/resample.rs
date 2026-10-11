//! Resampling: a small deterministic PRNG, bootstrap confidence intervals (percentile
//! and BCa) and the two-group permutation test.

use crate::descriptive::{quantile_sorted, QuantileMethod};
use crate::dist::std_normal_cdf;
use crate::special::{norm_quantile, norm_quantile_pq};
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

/// xoshiro256** (Blackman & Vigna 2021, "Scrambled linear pseudorandom number
/// generators", ACM TOMS 47:36), seeded through SplitMix64 (Steele, Lea & Flood 2014).
/// Deterministic across platforms; not cryptographic.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Xoshiro256 {
    s: [u64; 4],
}

impl Xoshiro256 {
    /// Generator seeded from a 64-bit seed (SplitMix64 expansion; never all-zero state).
    pub fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        Xoshiro256 {
            s: [next(), next(), next(), next()],
        }
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform on [0, 1) with 53 random bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform integer in 0..n (Lemire 2019 multiply-and-reject, unbiased). n > 0.
    pub fn below(&mut self, n: u64) -> u64 {
        let mut m = (self.next_u64() as u128) * (n as u128);
        let mut l = m as u64;
        if l < n {
            let t = n.wrapping_neg() % n;
            while l < t {
                m = (self.next_u64() as u128) * (n as u128);
                l = m as u64;
            }
        }
        (m >> 64) as u64
    }

    /// Standard normal deviate (inversion of a 53-bit uniform on (0, 1)).
    pub fn normal(&mut self) -> f64 {
        let u = ((self.next_u64() >> 11) as f64 + 0.5) * (1.0 / (1u64 << 53) as f64);
        norm_quantile(u)
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

/// Bootstrap interval method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BootstrapMethod {
    /// Percentile interval (Efron 1979).
    Percentile,
    /// Bias-corrected and accelerated (Efron 1987, JASA 82:171), acceleration from the
    /// jackknife.
    #[default]
    Bca,
}

/// A bootstrap confidence interval.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BootstrapCi {
    /// The statistic on the data.
    pub estimate: f64,
    /// Lower limit.
    pub lower: f64,
    /// Upper limit.
    pub upper: f64,
    /// Method.
    pub method: BootstrapMethod,
    /// Bootstrap SE (SD of the replicates).
    pub se: f64,
    /// Mean of the replicates minus the estimate.
    pub bias: f64,
    /// BCa bias correction z0 (0 for the percentile method).
    pub z0: f64,
    /// BCa acceleration a (0 for the percentile method).
    pub acceleration: f64,
    /// Replicates used (non-finite replicates are dropped).
    pub replicates: usize,
    /// Seed.
    pub seed: u64,
}

/// Bootstrap CI of `stat` over one or more independent samples, each resampled with
/// replacement within itself. Quantiles of the replicates by linear interpolation
/// (Hyndman–Fan type 7). BCa: z0 = Phi^{-1}(#(theta* < theta) + #(theta* = theta)/2)/B),
/// a = sum (m - theta_(i))^3 / (6 (sum (m - theta_(i))^2)^1.5) over the leave-one-out
/// jackknife of every observation of every sample, limits at
/// Phi(z0 + (z0 + z_alpha)/(1 - a (z0 + z_alpha))).
pub fn bootstrap_ci<F>(
    samples: &[&[f64]],
    stat: F,
    replicates: usize,
    ci_level: f64,
    method: BootstrapMethod,
    seed: u64,
) -> Result<BootstrapCi>
where
    F: Fn(&[&[f64]]) -> f64,
{
    if !(ci_level > 0.0 && ci_level < 1.0) {
        return Err(invalid("ci_level must be in (0, 1)"));
    }
    if replicates < 10 {
        return Err(invalid("need at least 10 replicates"));
    }
    let clean: Vec<Vec<f64>> = samples
        .iter()
        .map(|s| s.iter().copied().filter(|v| !v.is_nan()).collect())
        .collect();
    if clean.iter().any(|s| s.is_empty()) {
        return Err(StatsError::NotEnoughData("empty sample".into()));
    }
    let refs: Vec<&[f64]> = clean.iter().map(|v| v.as_slice()).collect();
    let est = stat(&refs);
    let mut rng = Xoshiro256::new(seed);
    let mut bufs: Vec<Vec<f64>> = clean.iter().map(|s| vec![0.0; s.len()]).collect();
    let mut reps = Vec::with_capacity(replicates);
    for _ in 0..replicates {
        for (b, s) in bufs.iter_mut().zip(&clean) {
            for v in b.iter_mut() {
                *v = s[rng.below(s.len() as u64) as usize];
            }
        }
        let r: Vec<&[f64]> = bufs.iter().map(|v| v.as_slice()).collect();
        let t = stat(&r);
        if t.is_finite() {
            reps.push(t);
        }
    }
    if reps.len() < 10 {
        return Err(StatsError::NoConvergence(
            "statistic not finite on resamples".into(),
        ));
    }
    reps.sort_by(f64::total_cmp);
    let b = reps.len() as f64;
    let mean = reps.iter().sum::<f64>() / b;
    let se = (reps.iter().map(|r| (r - mean) * (r - mean)).sum::<f64>() / (b - 1.0)).sqrt();
    let alpha = 1.0 - ci_level;
    let (mut lo_p, mut hi_p) = (0.5 * alpha, 1.0 - 0.5 * alpha);
    let (mut z0, mut acc) = (0.0, 0.0);
    if method == BootstrapMethod::Bca {
        let below = reps.iter().filter(|&&r| r < est).count() as f64;
        let equal = reps.iter().filter(|&&r| r == est).count() as f64;
        let frac = ((below + 0.5 * equal) / b).clamp(0.5 / b, 1.0 - 0.5 / b);
        z0 = norm_quantile(frac);
        // jackknife
        let mut jack = Vec::new();
        for g in 0..clean.len() {
            if clean[g].len() < 2 {
                continue;
            }
            for i in 0..clean[g].len() {
                let mut loo: Vec<Vec<f64>> = clean.clone();
                loo[g].remove(i);
                let r: Vec<&[f64]> = loo.iter().map(|v| v.as_slice()).collect();
                let t = stat(&r);
                if t.is_finite() {
                    jack.push(t);
                }
            }
        }
        if jack.len() >= 2 {
            let m = jack.iter().sum::<f64>() / jack.len() as f64;
            let num: f64 = jack.iter().map(|t| (m - t).powi(3)).sum();
            let den: f64 = jack.iter().map(|t| (m - t).powi(2)).sum();
            acc = if den > 0.0 {
                num / (6.0 * den.powf(1.5))
            } else {
                0.0
            };
        }
        let za = norm_quantile_pq(0.5 * alpha, 1.0 - 0.5 * alpha);
        let adj = |z: f64| std_normal_cdf(z0 + (z0 + z) / (1.0 - acc * (z0 + z)));
        lo_p = adj(za);
        hi_p = adj(-za);
    }
    let q = |p: f64| {
        quantile_sorted(&reps, p.clamp(0.0, 1.0), QuantileMethod::Linear).unwrap_or(f64::NAN)
    };
    Ok(BootstrapCi {
        estimate: est,
        lower: q(lo_p),
        upper: q(hi_p),
        method,
        se,
        bias: mean - est,
        z0,
        acceleration: acc,
        replicates: reps.len(),
        seed,
    })
}

/// Bootstrap CI of mean(a) - mean(b) (each group resampled separately).
pub fn bootstrap_mean_difference(
    a: &[f64],
    b: &[f64],
    replicates: usize,
    ci_level: f64,
    method: BootstrapMethod,
    seed: u64,
) -> Result<BootstrapCi> {
    bootstrap_ci(
        &[a, b],
        |s| {
            s[0].iter().sum::<f64>() / s[0].len() as f64
                - s[1].iter().sum::<f64>() / s[1].len() as f64
        },
        replicates,
        ci_level,
        method,
        seed,
    )
}

/// Two-group permutation test of the difference in means.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermutationTest {
    /// mean(a) - mean(b).
    pub difference: f64,
    /// Two-sided p: P(|D*| >= |D|).
    pub p: f64,
    /// P(D* <= D).
    pub p_less: f64,
    /// P(D* >= D).
    pub p_greater: f64,
    /// Exact enumeration of all C(n, n_a) splits (true) or Monte Carlo (false).
    pub exact: bool,
    /// Permutations evaluated.
    pub permutations: u64,
}

fn ln_choose(n: u64, k: u64) -> f64 {
    crate::special::ln_choose(n as f64, k as f64)
}

/// Permutation test of mean(a) - mean(b) (Fisher 1935; Pitman 1937): exact enumeration of
/// every split when C(n_a + n_b, n_a) <= `exact_limit`, otherwise `monte_carlo` random
/// relabellings with p = (count + 1)/(B + 1) (Phipson & Smyth 2010). Ties in |D| are
/// counted with a relative tolerance of 1e-12.
pub fn permutation_test(
    a: &[f64],
    b: &[f64],
    exact_limit: u64,
    monte_carlo: u64,
    seed: u64,
) -> Result<PermutationTest> {
    let a: Vec<f64> = a.iter().copied().filter(|v| !v.is_nan()).collect();
    let b: Vec<f64> = b.iter().copied().filter(|v| !v.is_nan()).collect();
    if a.is_empty() || b.is_empty() {
        return Err(StatsError::NotEnoughData("empty group".into()));
    }
    let (na, nb) = (a.len(), b.len());
    let n = na + nb;
    let mut all = a.clone();
    all.extend_from_slice(&b);
    let total: f64 = all.iter().sum();
    let diff_of = |sa: f64| sa / na as f64 - (total - sa) / nb as f64;
    let d0 = diff_of(a.iter().sum());
    let eps = 1e-12 * (d0.abs().max(1e-300) + all.iter().fold(0.0f64, |m, v| m.max(v.abs())));
    let (mut c2, mut cl, mut cg) = (0u64, 0u64, 0u64);
    let mut count = |d: f64| {
        if d.abs() >= d0.abs() - eps {
            c2 += 1;
        }
        if d <= d0 + eps {
            cl += 1;
        }
        if d >= d0 - eps {
            cg += 1;
        }
    };
    let combos = ln_choose(n as u64, na as u64).exp();
    if combos <= exact_limit as f64 {
        // enumerate k-subsets of n in lexicographic order
        let k = na;
        let mut idx: Vec<usize> = (0..k).collect();
        let mut perms = 0u64;
        loop {
            let s: f64 = idx.iter().map(|&i| all[i]).sum();
            count(diff_of(s));
            perms += 1;
            // next combination
            let mut i = k;
            loop {
                if i == 0 {
                    let pt = perms as f64;
                    return Ok(PermutationTest {
                        difference: d0,
                        p: c2 as f64 / pt,
                        p_less: cl as f64 / pt,
                        p_greater: cg as f64 / pt,
                        exact: true,
                        permutations: perms,
                    });
                }
                i -= 1;
                if idx[i] < n - k + i {
                    idx[i] += 1;
                    for j in (i + 1)..k {
                        idx[j] = idx[j - 1] + 1;
                    }
                    break;
                }
            }
        }
    }
    if monte_carlo == 0 {
        return Err(invalid(
            "too many splits for exact enumeration and no Monte Carlo",
        ));
    }
    let mut rng = Xoshiro256::new(seed);
    let mut v = all.clone();
    for _ in 0..monte_carlo {
        // partial Fisher-Yates: the first na positions are a uniform random subset
        for i in 0..na {
            let j = i + rng.below((n - i) as u64) as usize;
            v.swap(i, j);
        }
        let s: f64 = v[..na].iter().sum();
        count(diff_of(s));
    }
    let bb = monte_carlo as f64 + 1.0;
    Ok(PermutationTest {
        difference: d0,
        p: (c2 as f64 + 1.0) / bb,
        p_less: (cl as f64 + 1.0) / bb,
        p_greater: (cg as f64 + 1.0) / bb,
        exact: false,
        permutations: monte_carlo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xoshiro_reference_and_uniformity() {
        // Reference output of xoshiro256** for state [1, 2, 3, 4] (Vigna's C code).
        let mut r = Xoshiro256 { s: [1, 2, 3, 4] };
        let want = [11520u64, 0, 1509978240, 1215971899390074240];
        for w in want {
            assert_eq!(r.next_u64(), w);
        }
        let mut r = Xoshiro256::new(42);
        let n = 100_000;
        let m: f64 = (0..n).map(|_| r.next_f64()).sum::<f64>() / n as f64;
        assert!((m - 0.5).abs() < 0.005);
        let mut counts = [0usize; 7];
        for _ in 0..70_000 {
            counts[r.below(7) as usize] += 1;
        }
        assert!(counts.iter().all(|&c| (c as f64 - 10_000.0).abs() < 400.0));
        let z: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let zm = z.iter().sum::<f64>() / n as f64;
        let zv = z.iter().map(|v| (v - zm) * (v - zm)).sum::<f64>() / (n as f64 - 1.0);
        assert!(zm.abs() < 0.015 && (zv - 1.0).abs() < 0.02);
    }

    #[test]
    fn permutation_exact_small() {
        // a = {1,2}, b = {3,4}: 6 splits, D0 = -2; |D| >= 2 for {1,2} and {3,4} only.
        let r = permutation_test(&[1.0, 2.0], &[3.0, 4.0], 1000, 0, 1).unwrap();
        assert!(r.exact);
        assert_eq!(r.permutations, 6);
        assert!((r.p - 2.0 / 6.0).abs() < 1e-15);
        assert!((r.p_less - 1.0 / 6.0).abs() < 1e-15);
        let mc = permutation_test(&[1.0, 2.0, 2.5], &[3.0, 4.0, 3.5], 1, 20_000, 3).unwrap();
        assert!(!mc.exact);
        // exact two-sided p for these is 2/20
        assert!((mc.p - 0.1).abs() < 0.01, "{}", mc.p);
    }
}
