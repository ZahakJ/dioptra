//! Stand-in numerics for descriptive statistics until `dioptra-stats` provides them.
//!
//! TODO(stats): replace `t_quantile` with `dioptra_stats::distributions::t_quantile` once it
//! lands; this implementation (regularised incomplete beta by Lentz's continued fraction,
//! bisection on the CDF) is accurate to about 1e-12, which is ample for display.

/// ln Γ(x) for x > 0 (Lanczos, g = 7, n = 9).
pub fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=300 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
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
    let ln_bt = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln();
    if x < (a + 1.0) / (a + b + 2.0) {
        (ln_bt.exp() * beta_cf(a, b, x)) / a
    } else {
        1.0 - (ln_bt.exp() * beta_cf(b, a, 1.0 - x)) / b
    }
}

/// CDF of Student's t with `df` degrees of freedom.
pub fn t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let tail = 0.5 * inc_beta(df / 2.0, 0.5, x);
    if t >= 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

/// Quantile of Student's t: the `t` with `t_cdf(t, df) = p`.
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) || df <= 0.0 {
        return f64::NAN;
    }
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -t_quantile(1.0 - p, df);
    }
    let mut hi = 1.0;
    while t_cdf(hi, df) < p && hi < 1e12 {
        hi *= 2.0;
    }
    let mut lo = 0.0;
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if t_cdf(mid, df) < p {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-15 * hi {
            break;
        }
    }
    0.5 * (lo + hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_quantiles_match_tables() {
        // Reference values: scipy.stats.t.ppf.
        for (p, df, want) in [
            (0.975, 1.0, 12.706204736174698),
            (0.975, 2.0, 4.302652729749464),
            (0.975, 9.0, 2.2621571628540993),
            (0.975, 30.0, 2.0422724563012373),
            (0.995, 5.0, 4.032142983557536),
            (0.95, 1000.0, 1.6463788172854639),
        ] {
            let got = t_quantile(p, df);
            assert!(
                (got - want).abs() < 1e-9 * want,
                "t({p}, {df}) = {got}, want {want}"
            );
        }
        assert!((t_quantile(0.025, 9.0) + 2.2621571628540993).abs() < 1e-9);
    }
}
