//! Nonlinear regression against fixtures/oracle/nonlinear_regression.json (SciPy
//! `curve_fit` / `least_squares` optima, polished).
mod common;

#[test]
fn nonlinear_fixture() {
    common::families::nonlinear_family().finish();
}

use dioptra_stats::equations::{self, Equation};
use dioptra_stats::nonlinear::{self as nl, Dataset, FitOptions, ParamConstraint, ParamStatus};
use dioptra_stats::regression::{simple_linear_regression, SimpleRegressionOptions};

/// Deterministic standard-normal noise (Box–Muller on a fixed LCG).
fn noise(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    let mut next = || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    (0..n)
        .map(|_| (-2.0 * next().ln()).sqrt() * (2.0 * std::f64::consts::PI * next()).cos())
        .collect()
}

fn grid(a: f64, b: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| a + (b - a) * i as f64 / (n - 1) as f64)
        .collect()
}

#[test]
fn four_pl_noiseless_recovered() {
    let eq = equations::log_agonist_variable_slope();
    let truth = [5.0, 95.0, -6.5, 0.8];
    let x = grid(-9.0, -4.0, 21);
    let y: Vec<f64> = x.iter().map(|&v| eq.eval(&[v], &truth)).collect();
    let f = nl::fit(&eq, &Dataset::new(&x, &y), &FitOptions::default()).unwrap();
    for (g, t) in f.params().iter().zip(truth) {
        assert!(((g - t) / t).abs() < 1e-10, "{g} vs {t}");
    }
    assert!(f.ss < 1e-20);
    // EC50 derived from LogEC50
    let ec50 = &f.datasets[0].derived[0];
    assert_eq!(ec50.name, "EC50");
    assert!((ec50.value / 10f64.powf(-6.5) - 1.0).abs() < 1e-9);
}

#[test]
fn global_fit_shares_top_and_bottom() {
    let eq = equations::log_agonist_variable_slope();
    let x = grid(-9.0, -4.0, 16);
    let truths = [
        [2.0, 100.0, -7.0, 1.0],
        [2.0, 100.0, -6.0, 0.7],
        [2.0, 100.0, -5.5, 1.3],
    ];
    let data: Vec<Dataset> = truths
        .iter()
        .enumerate()
        .map(|(d, t)| {
            let e = noise(x.len(), 11 + d as u64);
            let y: Vec<f64> = x
                .iter()
                .zip(&e)
                .map(|(&v, n)| eq.eval(&[v], t) + 1.5 * n)
                .collect();
            Dataset::new(&x, &y)
        })
        .collect();
    let opts = FitOptions {
        constraints: Some(vec![
            ParamConstraint::shared(),
            ParamConstraint::shared(),
            ParamConstraint::free(),
            ParamConstraint::free(),
        ]),
        ..FitOptions::default()
    };
    let g = nl::fit_global(&eq, &data, &opts).unwrap();
    assert_eq!(g.k, 2 + 2 * 3);
    assert_eq!(g.datasets.len(), 3);
    for (ds, t) in g.datasets.iter().zip(&truths) {
        assert_eq!(ds.params[0].status, ParamStatus::Shared);
        assert_eq!(ds.params[0].value, g.datasets[0].params[0].value);
        assert_eq!(ds.params[2].status, ParamStatus::Fitted);
        assert!((ds.params[2].value - t[2]).abs() < 0.1);
    }
    assert!((g.datasets[0].params[0].value - 2.0).abs() < 2.0);
    assert!((g.datasets[0].params[1].value - 100.0).abs() < 2.0);
    // Noiseless version recovers exactly.
    let exact: Vec<Dataset> = truths
        .iter()
        .map(|t| {
            let y: Vec<f64> = x.iter().map(|&v| eq.eval(&[v], t)).collect();
            Dataset::new(&x, &y)
        })
        .collect();
    let ge = nl::fit_global(&eq, &exact, &opts).unwrap();
    for (d, t) in truths.iter().enumerate() {
        for (k, tv) in t.iter().enumerate() {
            let v = ge.datasets[d].params[k].value;
            assert!(
                ((v - tv) / tv).abs() < 1e-9,
                "set {d} param {k}: {v} vs {tv}"
            );
        }
    }
    // Shared vs separate: the truth is shared, so the F test must not reject.
    let fam = nl::fit_family(&eq, &data, &opts).unwrap();
    assert_eq!(fam.separate.len(), 3);
    assert!(fam.comparison.p > 0.05, "{:?}", fam.comparison);
    assert_eq!(fam.comparison.df_num, 4.0);
}

#[test]
fn profile_ci_equals_t_ci_for_a_line() {
    let x = grid(0.0, 10.0, 12);
    let e = noise(12, 3);
    let y: Vec<f64> = x.iter().zip(&e).map(|(v, n)| 1.5 + 0.7 * v + n).collect();
    let opts = FitOptions {
        profile_ci: true,
        ..FitOptions::default()
    };
    let f = nl::fit(&equations::straight_line(), &Dataset::new(&x, &y), &opts).unwrap();
    for p in &f.datasets[0].params {
        let (lo, hi) = (p.profile_low.unwrap(), p.profile_high.unwrap());
        assert!(
            ((lo - p.ci_low) / p.ci_low).abs() < 1e-7,
            "{} {lo} {}",
            p.name,
            p.ci_low
        );
        assert!(
            ((hi - p.ci_high) / p.ci_high).abs() < 1e-7,
            "{} {hi} {}",
            p.name,
            p.ci_high
        );
    }
    // and with the linear-regression module
    let r = simple_linear_regression(&x, &y, SimpleRegressionOptions::default()).unwrap();
    let p = f.params();
    assert!((p[0] - r.intercept).abs() < 1e-12 && (p[1] - r.slope).abs() < 1e-12);
    let se = f.se();
    assert!((se[1] / r.se_slope - 1.0).abs() < 1e-10);
    // Bands agree with the linear-regression bands.
    let opts = FitOptions::default();
    let xs = [0.0, 3.3, 10.0];
    let b = nl::bands(&equations::straight_line(), &f, 0, &xs, &opts).unwrap();
    let rb = r.bands(&xs);
    for i in 0..3 {
        assert!(
            (b.ci_lower[i] - rb.ci_lower[i]).abs() < 1e-10,
            "{} {}",
            b.ci_lower[i],
            rb.ci_lower[i]
        );
        assert!((b.pi_upper[i] - rb.pi_upper[i]).abs() < 1e-10);
    }
}

#[test]
fn profile_ci_is_asymmetric_for_a_rate() {
    let eq = equations::one_phase_decay();
    let x = grid(0.0, 8.0, 12);
    let e = noise(12, 5);
    let y: Vec<f64> = x
        .iter()
        .zip(&e)
        .map(|(v, n)| eq.eval(&[*v], &[100.0, 10.0, 0.4]) + 3.0 * n)
        .collect();
    let opts = FitOptions {
        profile_ci: true,
        ..FitOptions::default()
    };
    let f = nl::fit(&eq, &Dataset::new(&x, &y), &opts).unwrap();
    let k = &f.datasets[0].params[2];
    let (lo, hi) = (k.profile_low.unwrap(), k.profile_high.unwrap());
    assert!(lo < k.value && k.value < hi);
    // The profile CI of a rate constant is skewed to the right.
    assert!(hi - k.value > k.value - lo, "{lo} {} {hi}", k.value);
}

#[test]
fn constraints_hold() {
    let eq = equations::log_agonist_variable_slope();
    let x = grid(-9.0, -4.0, 21);
    let y: Vec<f64> = x
        .iter()
        .map(|&v| eq.eval(&[v], &[5.0, 95.0, -6.5, 0.8]))
        .collect();
    let d = Dataset::new(&x, &y);
    let opts = FitOptions {
        constraints: Some(vec![
            ParamConstraint::between(10.0, 50.0),
            ParamConstraint::free(),
            ParamConstraint::free(),
            ParamConstraint::fixed(1.0),
        ]),
        ..FitOptions::default()
    };
    let f = nl::fit(&eq, &d, &opts).unwrap();
    let p = &f.datasets[0].params;
    assert_eq!(p[0].value, 10.0);
    assert_eq!(p[0].status, ParamStatus::AtBound);
    assert!(p[0].se.is_nan());
    assert_eq!(p[3].value, 1.0);
    assert_eq!(p[3].status, ParamStatus::Fixed);
    assert_eq!(f.k, 3);
    assert_eq!(f.convergence.status, nl::FitStatus::HitConstraint);
    // An upper bound that is not active leaves the fit alone.
    let opts = FitOptions {
        constraints: Some(vec![
            ParamConstraint::free(),
            ParamConstraint::between(0.0, 200.0),
            ParamConstraint::free(),
            ParamConstraint::free(),
        ]),
        ..FitOptions::default()
    };
    let f = nl::fit(&eq, &d, &opts).unwrap();
    assert_eq!(f.convergence.status, nl::FitStatus::Converged);
    assert!((f.params()[1] - 95.0).abs() < 1e-9);
}

#[test]
fn rout_flags_planted_outliers_on_a_curve() {
    let eq = equations::log_agonist_variable_slope();
    let truth = [5.0, 95.0, -6.5, 0.8];
    let x = grid(-9.0, -4.0, 26);
    let e = noise(26, 7);
    let mut y: Vec<f64> = x
        .iter()
        .zip(&e)
        .map(|(&v, n)| eq.eval(&[v], &truth) + 2.0 * n)
        .collect();
    y[6] += 40.0;
    y[17] -= 35.0;
    let d = Dataset::new(&x, &y);
    let r = nl::rout_fit(&eq, &d, 0.01, &FitOptions::default()).unwrap();
    assert_eq!(r.outliers[0], vec![6, 17]);
    let p = r.fit.params();
    for (g, t) in p.iter().zip(truth) {
        assert!((g - t).abs() < 0.1 * t.abs().max(1.0), "{g} vs {t}");
    }
    assert_eq!(r.fit.n, 24);
    assert!(r.rsdr > 1.0 && r.rsdr < 4.0, "{}", r.rsdr);
    // The robust fit itself is close to the truth despite the outliers.
    let rp = r.robust.params();
    assert!((rp[2] - truth[2]).abs() < 0.1);
    // Through FitOptions as well.
    let opts = FitOptions {
        outlier_q: Some(0.01),
        ..FitOptions::default()
    };
    let f = nl::fit(&eq, &d, &opts).unwrap();
    assert_eq!(f.datasets[0].outliers, vec![6, 17]);
    assert!(!f.datasets[0].residuals[6].is_nan());
    // No outliers in clean data.
    let yc: Vec<f64> = x
        .iter()
        .zip(&e)
        .map(|(&v, n)| eq.eval(&[v], &truth) + 2.0 * n)
        .collect();
    let rc = nl::rout_fit(&eq, &Dataset::new(&x, &yc), 0.01, &FitOptions::default()).unwrap();
    assert!(rc.outliers[0].is_empty());
}

#[test]
fn polynomial_uses_the_linear_solution() {
    let x = grid(-2.0, 3.0, 15);
    let e = noise(15, 9);
    let y: Vec<f64> = x
        .iter()
        .zip(&e)
        .map(|(v, n)| 1.0 - 2.0 * v + 0.5 * v * v * v + 0.1 * n)
        .collect();
    let f = nl::fit(
        &equations::polynomial(3).unwrap(),
        &Dataset::new(&x, &y),
        &FitOptions::default(),
    )
    .unwrap();
    let r =
        dioptra_stats::regression::polynomial_regression(&x, &y, 3, Default::default()).unwrap();
    for k in 0..4 {
        assert!(
            (f.params()[k] - r.coefficients[k]).abs() < 1e-12 * (1.0 + r.coefficients[k].abs())
        );
        assert!((f.se()[k] / r.se[k] - 1.0).abs() < 1e-9);
    }
}

#[test]
fn finite_difference_and_closure_jacobians_agree() {
    let eq = equations::michaelis_menten();
    let x = [0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
    let y = [10.2, 18.1, 30.5, 45.0, 60.3, 70.1, 76.0];
    let d = Dataset::new(&x, &y);
    let a = nl::fit(&eq, &d, &FitOptions::default()).unwrap();
    let opts = FitOptions {
        jacobian: nl::JacobianMode::FiniteDifference { richardson: true },
        ..FitOptions::default()
    };
    let b = nl::fit(&eq, &d, &opts).unwrap();
    let c = nl::fit(
        &Equation::from_closure("mm", &["Vmax", "Km"], |x, p| p[0] * x[0] / (p[1] + x[0])),
        &d,
        &FitOptions {
            initial: Some(vec![vec![80.0, 3.0]]),
            ..FitOptions::default()
        },
    )
    .unwrap();
    for k in 0..2 {
        assert!((a.params()[k] / b.params()[k] - 1.0).abs() < 1e-8);
        assert!((a.params()[k] / c.params()[k] - 1.0).abs() < 1e-8);
        assert!((a.se()[k] / b.se()[k] - 1.0).abs() < 1e-6);
    }
}

#[test]
fn poisson_and_weighting_variants_run() {
    let eq = equations::one_phase_decay();
    let x = grid(0.5, 10.5, 11);
    let y: Vec<f64> = x
        .iter()
        .map(|v| (eq.eval(&[*v], &[200.0, 20.0, 0.3])).round())
        .collect();
    let d = Dataset::new(&x, &y);
    for w in [
        nl::Weighting::Poisson,
        nl::Weighting::InverseY,
        nl::Weighting::InverseYK(1.5),
        nl::Weighting::InverseX,
    ] {
        let f = nl::fit(
            &eq,
            &d,
            &FitOptions {
                weighting: w,
                ..FitOptions::default()
            },
        )
        .unwrap();
        assert_eq!(f.convergence.status, nl::FitStatus::Converged, "{w:?}");
        assert!(
            (f.params()[2] - 0.3).abs() < 0.02,
            "{w:?}: {:?}",
            f.params()
        );
    }
    // 1/SD^2 needs SDs.
    let sd = vec![2.0; 11];
    let f = nl::fit(
        &eq,
        &d.clone().with_sd(&sd),
        &FitOptions {
            weighting: nl::Weighting::InverseSD2,
            ..FitOptions::default()
        },
    )
    .unwrap();
    let g = nl::fit(&eq, &d, &FitOptions::default()).unwrap();
    // Equal SDs give the unweighted parameters and SEs.
    for k in 0..3 {
        assert!((f.params()[k] / g.params()[k] - 1.0).abs() < 1e-9);
        assert!((f.se()[k] / g.se()[k] - 1.0).abs() < 1e-8);
    }
    // impossible weights
    let mut yz = y.clone();
    yz[3] = 0.0;
    assert!(nl::fit(
        &eq,
        &Dataset::new(&x, &yz),
        &FitOptions {
            weighting: nl::Weighting::InverseY2,
            weight_base: nl::WeightBase::Data,
            ..FitOptions::default()
        }
    )
    .is_err());
}

#[test]
fn curve_table_and_diagnostics() {
    let eq = equations::michaelis_menten();
    let x = [0.5, 0.5, 1.0, 1.0, 2.0, 2.0, 4.0, 4.0, 8.0, 8.0];
    let y = [10.0, 11.0, 18.0, 19.5, 31.0, 29.5, 44.0, 46.5, 61.0, 59.0];
    let opts = FitOptions {
        curve_points: 50,
        ..FitOptions::default()
    };
    let f = nl::fit(&eq, &Dataset::new(&x, &y), &opts).unwrap();
    let ds = &f.datasets[0];
    let c = ds.curve.as_ref().unwrap();
    assert_eq!(c.x.len(), 50);
    for i in 0..50 {
        assert!(c.pi_lower[i] < c.ci_lower[i] && c.ci_lower[i] < c.y[i] && c.y[i] < c.ci_upper[i]);
    }
    assert!(ds.replicates_test.is_some());
    assert!(ds.runs_test.is_some());
    assert!(f.r_squared > 0.99 && f.adj_r_squared < f.r_squared);
    assert_eq!(f.correlation.len(), 4);
    assert!((f.correlation[0] - 1.0).abs() < 1e-14);
    assert!(ds
        .params
        .iter()
        .all(|p| p.dependency > 0.0 && p.dependency < 1.0));
}

#[test]
fn custom_equation_errors_and_names() {
    assert!(Equation::custom("1 + 2").is_err());
    assert!(Equation::custom("A*exp(-k*X").is_err());
    let e = Equation::custom("Top*X/(Kd + X) + NS*X").unwrap();
    assert_eq!(e.param_names(), ["Top", "Kd", "NS"]);
    assert!(Equation::custom_with("a*q", &["X"], Some(&["a"])).is_err());
    // Model comparison sanity: identical fits.
    let s = nl::FitSummary {
        n: 10,
        k: 2,
        ss: 5.0,
        df: 8.0,
    };
    let c = nl::compare_summaries(
        s,
        nl::FitSummary {
            n: 10,
            k: 3,
            ss: 5.0,
            df: 7.0,
        },
        0.05,
    )
    .unwrap();
    assert_eq!(c.preferred_f, nl::Preferred::Simpler);
    assert_eq!(c.preferred_aicc, nl::Preferred::Simpler);
    assert!(nl::compare_summaries(s, nl::FitSummary { n: 11, ..s }, 0.05).is_err());
}
