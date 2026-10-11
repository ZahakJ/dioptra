//! Tukey-Kramer, Dunnett and p-value adjustment against
//! fixtures/oracle/multiple_comparisons.json, plus hand-computed checks.
mod common;
use common::families;
use dioptra_stats::posthoc::{p_adjust, pairwise_t, Adjust};

#[test]
fn multiple_comparisons_fixture() {
    families::multiple_comparisons_family().finish();
}

#[test]
fn library_sanity_checks() {
    families::multiple_comparisons_sanity().finish();
}

#[test]
fn p_adjust_hand_values() {
    // Five p-values; every method checked by hand.
    let p = [0.005, 0.011, 0.02, 0.04, 0.13];
    let near = |a: &[f64], b: &[f64]| {
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 1e-15, "{a:?} vs {b:?}");
        }
    };
    near(&p_adjust(&p, Adjust::None), &p);
    near(
        &p_adjust(&p, Adjust::Bonferroni),
        &[0.025, 0.055, 0.1, 0.2, 0.65],
    );
    // Holm: 5p1, 4p2, 3p3, 2p4, p5 = .025 .044 .06 .08 .13 (already increasing)
    near(
        &p_adjust(&p, Adjust::Holm),
        &[0.025, 0.044, 0.06, 0.08, 0.13],
    );
    // Hochberg: min from top of (5-i+1)p_i -> .025 .044 .06 .08 .13
    near(
        &p_adjust(&p, Adjust::Hochberg),
        &[0.025, 0.044, 0.06, 0.08, 0.13],
    );
    // BH: p_i 5/i = .025 .0275 .03333 .05 .13; running min from the top keeps them.
    near(
        &p_adjust(&p, Adjust::BenjaminiHochberg),
        &[0.025, 0.0275, 0.1 / 3.0, 0.05, 0.13],
    );
    // BY = BH * (1 + 1/2 + 1/3 + 1/4 + 1/5) = BH * 137/60
    let c = 137.0 / 60.0;
    near(
        &p_adjust(&p, Adjust::BenjaminiYekutieli),
        &[
            0.025 * c,
            0.0275 * c,
            0.1 / 3.0 * c,
            0.05 * c,
            (0.13 * c).min(1.0),
        ],
    );
    // Holm-Sidak: 1-(1-p_i)^(m-i+1), running max.
    let hs: Vec<f64> = [(0.005, 5), (0.011, 4), (0.02, 3), (0.04, 2), (0.13, 1)]
        .iter()
        .map(|&(x, k): &(f64, i32)| 1.0 - (1.0 - x).powi(k))
        .collect();
    let got = p_adjust(&p, Adjust::HolmSidak);
    for (g, h) in got.iter().zip(&hs) {
        assert!((g - h).abs() < 1e-15);
    }
    // Unsorted input keeps positions.
    let q = p_adjust(&[0.13, 0.005], Adjust::Bonferroni);
    near(&q, &[0.26, 0.01]);
}

#[test]
fn lsd_and_bonferroni_t() {
    let a = [4.17, 5.58, 5.18, 6.11, 4.50, 4.61, 5.17, 4.53, 5.33, 5.14];
    let b = [4.81, 4.17, 4.41, 3.59, 5.87, 3.83, 6.03, 4.89, 4.32, 4.69];
    let c = [6.31, 5.12, 5.54, 5.50, 5.37, 5.29, 4.92, 6.15, 5.80, 5.26];
    let lsd = pairwise_t(&[&a, &b, &c], None, Adjust::None, 0.05).unwrap();
    let bon = pairwise_t(&[&a, &b, &c], None, Adjust::Bonferroni, 0.05).unwrap();
    for (l, b) in lsd.comparisons.iter().zip(&bon.comparisons) {
        assert_eq!(l.p_raw, l.p_adj);
        assert!((b.p_adj - (3.0 * l.p_raw).min(1.0)).abs() < 1e-15);
        // Bonferroni CI is wider.
        assert!(b.ci.unwrap().1 - b.ci.unwrap().0 > l.ci.unwrap().1 - l.ci.unwrap().0);
    }
    // PlantGrowth ctrl vs trt2: t = (5.032 - 5.526)/sqrt(MSW (1/10 + 1/10)), MSW = 0.3885959.
    let t02 = lsd.comparisons[1].statistic;
    assert!((t02 - (-0.494 / (0.3885959 * 0.2f64).sqrt())).abs() < 1e-5);
}
