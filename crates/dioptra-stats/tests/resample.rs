//! Bootstrap and permutation sanity checks (no oracle fixture).
use dioptra_stats::resample::{
    bootstrap_ci, bootstrap_mean_difference, permutation_test, BootstrapMethod, Xoshiro256,
};

fn normal_sample(n: usize, mean: f64, sd: f64, seed: u64) -> Vec<f64> {
    let mut r = Xoshiro256::new(seed);
    (0..n).map(|_| mean + sd * r.normal()).collect()
}

#[test]
fn bca_matches_t_interval_for_a_normal_mean() {
    let x = normal_sample(200, 10.0, 2.0, 1);
    let mean = |s: &[&[f64]]| s[0].iter().sum::<f64>() / s[0].len() as f64;
    let ci = bootstrap_ci(&[&x], mean, 4000, 0.95, BootstrapMethod::Bca, 9).unwrap();
    let m = mean(&[&x]);
    let sd = (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / 199.0).sqrt();
    let half = 1.972 * sd / (200f64).sqrt();
    assert!(
        (ci.lower - (m - half)).abs() < 0.06,
        "{} vs {}",
        ci.lower,
        m - half
    );
    assert!(
        (ci.upper - (m + half)).abs() < 0.06,
        "{} vs {}",
        ci.upper,
        m + half
    );
    assert!(ci.acceleration.abs() < 0.05 && ci.z0.abs() < 0.1);
    assert!((ci.se - sd / (200f64).sqrt()).abs() < 0.02);
    // Determinism.
    let again = bootstrap_ci(&[&x], mean, 4000, 0.95, BootstrapMethod::Bca, 9).unwrap();
    assert_eq!(ci, again);
}

#[test]
fn bca_corrects_skew_of_a_variance() {
    // Exponential data: the variance's bootstrap distribution is right-skewed; BCa shifts
    // the interval up relative to the percentile interval (Efron & Tibshirani 1993 ch. 14).
    let mut r = Xoshiro256::new(3);
    let x: Vec<f64> = (0..60).map(|_| -(1.0 - r.next_f64()).ln()).collect();
    let var = |s: &[&[f64]]| {
        let v = s[0];
        let m = v.iter().sum::<f64>() / v.len() as f64;
        v.iter().map(|a| (a - m) * (a - m)).sum::<f64>() / (v.len() as f64 - 1.0)
    };
    let p = bootstrap_ci(&[&x], var, 4000, 0.90, BootstrapMethod::Percentile, 4).unwrap();
    let b = bootstrap_ci(&[&x], var, 4000, 0.90, BootstrapMethod::Bca, 4).unwrap();
    assert!(b.acceleration > 0.0);
    assert!(b.upper > p.upper && b.lower > p.lower);
    assert!(b.lower < b.estimate && b.estimate < b.upper);
}

#[test]
fn mean_difference_and_permutation_agree() {
    let a = normal_sample(12, 5.0, 1.0, 21);
    let b = normal_sample(12, 6.5, 1.0, 22);
    let ci = bootstrap_mean_difference(&a, &b, 3000, 0.95, BootstrapMethod::Bca, 5).unwrap();
    assert!(ci.upper < 0.0, "{ci:?}");
    let mc = permutation_test(&a, &b, 0, 20_000, 8).unwrap();
    let t = dioptra_stats::ttest::unpaired(&a, &b, &Default::default()).unwrap();
    assert!(
        !mc.exact && (mc.p - t.p).abs() < 0.3 * t.p + 0.002,
        "{} vs t test {}",
        mc.p,
        t.p
    );
    let small = permutation_test(&a[..6], &b[..6], 1_000, 0, 0).unwrap();
    assert!(small.exact && small.permutations == 924);
    assert!((small.difference - ci.estimate).abs() > 0.0);
}
