//! Survival analysis against fixtures/oracle/survival.json (lifelines).
mod common;

#[test]
fn survival_fixture() {
    common::families::survival_family().finish();
}

use dioptra_stats::survival::{self as sv, KmCi, KmOptions, LogRankWeight};

#[test]
fn three_groups_and_trend() {
    let t1 = [6.0, 6.0, 6.0, 7.0, 10.0, 13.0, 16.0, 22.0, 23.0, 25.0];
    let e1 = [true, true, true, true, false, true, true, true, true, false];
    let t2 = [1.0, 1.0, 2.0, 2.0, 3.0, 4.0, 4.0, 5.0, 5.0, 8.0];
    let e2 = [true; 10];
    let t3 = [3.0, 5.0, 7.0, 9.0, 11.0, 12.0, 14.0, 15.0, 17.0, 19.0];
    let e3 = [true, true, false, true, true, true, false, true, true, true];
    let g: [(&[f64], &[bool]); 3] = [(&t1, &e1), (&t2, &e2), (&t3, &e3)];
    let lr = sv::logrank(&g, LogRankWeight::LogRank, None).unwrap();
    assert_eq!(lr.df, 2.0);
    // O - E sums to zero, V rows sum to zero
    let s: f64 = lr
        .observed
        .iter()
        .zip(&lr.expected)
        .map(|(o, e)| o - e)
        .sum();
    assert!(s.abs() < 1e-12);
    for r in 0..3 {
        let rs: f64 = (0..3).map(|c| lr.variance[r * 3 + c]).sum();
        assert!(rs.abs() < 1e-12);
    }
    // The statistic does not depend on which group is dropped.
    let g2: [(&[f64], &[bool]); 3] = [(&t3, &e3), (&t1, &e1), (&t2, &e2)];
    let lr2 = sv::logrank(&g2, LogRankWeight::LogRank, None).unwrap();
    assert!((lr.chi2 - lr2.chi2).abs() < 1e-10 * lr.chi2);
    assert!(lr.trend.is_some());
    // Two groups: the k-group statistic equals the Mantel-Haenszel chi-square.
    let two = sv::logrank(&g[..2], LogRankWeight::LogRank, None).unwrap();
    let hr = sv::hazard_ratio(g[0], g[1], 0.95).unwrap();
    assert!((two.chi2 - hr.chi2).abs() < 1e-12 * hr.chi2);
}

#[test]
fn median_ci_and_queries() {
    let t: Vec<f64> = (1..=20).map(|v| v as f64).collect();
    let e: Vec<bool> = (0..20).map(|i| i % 4 != 3).collect();
    for ci in [KmCi::LogLog, KmCi::Linear, KmCi::Log] {
        let km = sv::kaplan_meier(&t, &e, KmOptions { ci_level: 0.95, ci }).unwrap();
        let (lo, hi) = km.median_ci;
        assert!(
            lo <= km.median && km.median <= hi,
            "{ci:?}: {lo} {} {hi}",
            km.median
        );
        for r in &km.rows {
            if r.survival > 0.0 {
                assert!(r.ci_lower <= r.survival && r.survival <= r.ci_upper);
            }
        }
    }
    let km = sv::kaplan_meier(&t, &e, KmOptions::default()).unwrap();
    assert_eq!(km.at_risk_at(0.0), 20);
    assert_eq!(km.at_risk_at(10.5), 10);
    assert_eq!(km.n, 20);
    assert_eq!(km.events, 15);
}

#[test]
fn cox_two_covariates_and_breslow() {
    // Two covariates: the group indicator and a nuisance covariate; Breslow equals Efron
    // without ties.
    let t: Vec<f64> = (0..30)
        .map(|i| 1.0 + i as f64 * 0.7 + ((i * 7) % 5) as f64 * 0.13)
        .collect();
    let e: Vec<bool> = (0..30).map(|i| i % 5 != 0).collect();
    let x: Vec<Vec<f64>> = (0..30)
        .map(|i| vec![(i % 2) as f64, ((i * 13) % 7) as f64 / 7.0])
        .collect();
    let a = sv::cox_ph(&t, &e, &x, sv::CoxTies::Efron, 0.95).unwrap();
    let b = sv::cox_ph(&t, &e, &x, sv::CoxTies::Breslow, 0.95).unwrap();
    for k in 0..2 {
        assert!((a.coefficients[k].coef - b.coefficients[k].coef).abs() < 1e-12);
    }
    assert!(a.lr_chi2 >= 0.0);
    assert_eq!(a.coefficients.len(), 2);
}
