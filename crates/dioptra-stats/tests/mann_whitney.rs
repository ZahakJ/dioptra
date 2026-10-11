//! Mann–Whitney U against fixtures/oracle/mann_whitney.json.
mod common;
use dioptra_stats::mannwhitney::{mann_whitney, u_distribution, MannWhitneyOptions};

#[test]
fn mann_whitney_fixture() {
    common::families::mann_whitney_family().finish();
}

#[test]
fn u_distribution_is_symmetric_and_normalised() {
    let d = u_distribution(7, 9);
    assert_eq!(d.len(), 64);
    let s: f64 = d.iter().sum();
    assert!((s - 1.0).abs() < 1e-15);
    for u in 0..d.len() {
        assert!((d[u] - d[d.len() - 1 - u]).abs() < 1e-17);
    }
    // P(U = 0) = 1/C(16, 7) = 1/11440
    assert!((d[0] - 1.0 / 11440.0).abs() < 1e-18);
}

#[test]
fn moses_ci_brackets_the_shift() {
    let x = [0.8, 0.83, 1.89, 1.04, 1.45, 1.38, 1.91, 1.64, 0.73, 1.46];
    let y = [1.15, 0.88, 0.9, 0.74, 1.21];
    let r = mann_whitney(&x, &y, &MannWhitneyOptions::default()).unwrap();
    let (lo, hi) = r.ci.unwrap();
    assert!(lo <= r.hodges_lehmann && r.hodges_lehmann <= hi);
    assert!(r.ci_achieved_level.unwrap() >= 0.95);
}
