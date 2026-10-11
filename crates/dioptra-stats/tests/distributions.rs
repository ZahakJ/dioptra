//! Distribution functions against fixtures/oracle/distributions.json (32-digit mpmath
//! references), studentized_range.json and the Dunnett integrals in
//! multiple_comparisons.json, plus closed-form identities.
mod common;
use common::*;
use dioptra_stats::dist::*;

#[test]
fn normal() {
    families::normal().finish();
}

#[test]
fn t_f_chi_squared() {
    families::t_f_chi2().finish();
}

#[test]
fn binomial_and_hypergeometric() {
    families::binomial_hypergeometric().finish();
}

#[test]
fn noncentral_t() {
    families::noncentral_t().finish();
}

#[test]
fn studentized_range() {
    families::studentized_range().finish();
}

#[test]
fn dunnett() {
    families::dunnett().finish();
}

/// Identities that pin the quadrature-based distributions to closed forms, independently
/// of the SciPy-generated studentized range table (which is itself off by up to 8e-6
/// relative in the far upper tail at df = 1000; see `studentized_range_k2_is_t`).
#[test]
fn quadrature_identities() {
    let mut ck = Checker::new("identities");
    // Range of two normals: P(Q > q) = P(|T_df| > q / sqrt 2).
    for df in [1.0, 2.0, 5.0, 20.0, 1000.0, f64::INFINITY] {
        let d = StudentizedRange::new(2, df).unwrap();
        for q in [0.5, 1.0, 3.0, 6.0, 8.0, 12.0] {
            let exact = p_two_sided_t(q / std::f64::consts::SQRT_2, df);
            ck.check(
                &format!("k=2 df={df} q={q}"),
                "sf",
                exact,
                d.cdf_sf(q).1,
                Tol::new(1e-10, 1e-300),
            );
        }
    }
    // Dunnett with one treatment: P(|T| > t) is the two-sided t p-value; one-sided is the t tail.
    for (lam, df) in [
        (0.5_f64, 7.0),
        (std::f64::consts::FRAC_1_SQRT_2, 27.0),
        (0.95, 3.0),
    ] {
        let d = Dunnett::new(vec![lam], df).unwrap();
        for t in [0.3, 1.5, 2.5, 4.0, 7.0] {
            let ctx = format!("Dunnett k=1 lambda={lam} df={df} t={t}");
            ck.check(
                &ctx,
                "two-sided",
                p_two_sided_t(t, df),
                d.p_value(t, true),
                Tol::new(1e-10, 1e-300),
            );
            ck.check(
                &ctx,
                "one-sided",
                p_greater_t(t, df),
                d.p_value(t, false),
                Tol::new(1e-10, 1e-300),
            );
        }
    }
    // Noncentral t with nc = 0 is Student's t; the density integrates the cdf.
    for df in [3.0, 10.0, 50.0] {
        let n0 = NoncentralT::new(df, 0.0).unwrap();
        let t = StudentT::new(df).unwrap();
        for x in [-4.0, -1.0, 0.5, 3.0] {
            ck.check(
                &format!("nct(df={df},0) x={x}"),
                "cdf",
                t.cdf(x),
                n0.cdf(x),
                Tol::new(1e-12, 1e-300),
            );
            ck.check(
                &format!("nct(df={df},0) x={x}"),
                "pdf",
                t.pdf(x),
                n0.pdf(x),
                Tol::new(1e-10, 1e-300),
            );
        }
        let n1 = NoncentralT::new(df, 1.7).unwrap();
        for x in [-1.0, 0.5, 2.0, 4.0] {
            let h = 1e-4;
            let num = (n1.cdf(x + h) - n1.cdf(x - h)) / (2.0 * h);
            ck.check(
                &format!("nct(df={df},1.7) x={x}"),
                "pdf~dcdf",
                num,
                n1.pdf(x),
                Tol::new(1e-7, 1e-12),
            );
        }
    }
    // Densities are derivatives of the cdfs.
    let dists: Vec<(String, Box<dyn Continuous>)> = vec![
        ("t(4.5)".into(), Box::new(StudentT::new(4.5).unwrap())),
        ("chi2(3)".into(), Box::new(ChiSquared::new(3.0).unwrap())),
        ("F(3,7)".into(), Box::new(FisherF::new(3.0, 7.0).unwrap())),
        (
            "Beta(2.5,0.7)".into(),
            Box::new(Beta::new(2.5, 0.7).unwrap()),
        ),
        ("Gamma(2,3)".into(), Box::new(Gamma::new(2.0, 3.0).unwrap())),
        ("N(1,2)".into(), Box::new(Normal::new(1.0, 2.0).unwrap())),
    ];
    for (name, d) in &dists {
        for p in [0.05, 0.3, 0.5, 0.8, 0.97] {
            let x = d.ppf(p);
            ck.check(
                name,
                &format!("cdf(ppf({p}))"),
                p,
                d.cdf(x),
                Tol::new(1e-13, 1e-300),
            );
            let h = if name.starts_with("Beta") {
                1e-6 * x.min(1.0 - x)
            } else {
                1e-5 * x.abs().max(1e-3)
            };
            let num = (d.cdf(x + h) - d.cdf(x - h)) / (2.0 * h);
            ck.check(
                name,
                &format!("pdf at ppf({p})"),
                num,
                d.pdf(x),
                Tol::new(1e-7, 1e-12),
            );
        }
    }
    // Discrete quantiles invert the cdf.
    let b = Binomial::new(37, 0.3).unwrap();
    for p in [0.01, 0.5, 0.99] {
        let k = b.quantile(p) as i64;
        assert!(b.cdf(k) >= p && (k == 0 || b.cdf(k - 1) < p));
    }
    let h = Hypergeometric::new(60, 25, 17).unwrap();
    for p in [0.01, 0.5, 0.99] {
        let k = h.quantile(p) as i64;
        assert!(h.cdf(k) >= p && h.cdf(k - 1) < p);
    }
    ck.finish();
}

#[test]
fn constructors_reject_bad_parameters() {
    assert!(Normal::new(0.0, 0.0).is_err());
    assert!(StudentT::new(-1.0).is_err());
    assert!(ChiSquared::new(f64::NAN).is_err());
    assert!(FisherF::new(1.0, 0.0).is_err());
    assert!(Beta::new(0.0, 1.0).is_err());
    assert!(Binomial::new(5, 1.5).is_err());
    assert!(Hypergeometric::new(5, 6, 1).is_err());
    assert!(StudentizedRange::new(1, 10.0).is_err());
    assert!(Dunnett::new(vec![1.0], 10.0).is_err());
    assert!(NoncentralT::new(0.0, 1.0).is_err());
    // NaN arguments propagate rather than panic.
    assert!(StudentT::new(3.0).unwrap().cdf(f64::NAN).is_nan());
    assert!(Normal::standard().ppf(1.5).is_nan());
}

/// Slow cross-check of the studentized range against nested adaptive Gauss–Kronrod
/// quadrature (run with `--ignored`; about a minute in release).
#[test]
#[ignore]
fn studentized_range_vs_nested_quadrature() {
    use dioptra_stats::{quad, special};
    fn reference(k: usize, df: f64, q: f64) -> (f64, f64) {
        let km1 = (k - 1) as i32;
        let r = |w: f64| -> (f64, f64) {
            let dd = |z: f64| {
                let zw = z - w;
                let lc = std_normal_cdf(zw);
                let d = if zw > 0.0 {
                    std_normal_sf(zw) - std_normal_sf(z)
                } else {
                    std_normal_cdf(z) - lc
                };
                (lc, d)
            };
            let fc = |z: f64| std_normal_pdf(z) * dd(z).1.powi(km1);
            let fs = |z: f64| {
                let a = std_normal_cdf(z);
                let (lc, d) = dd(z);
                let mut pd = 1.0;
                let mut acc = 1.0;
                for _ in 1..km1 {
                    pd *= d;
                    acc = pd + a * acc;
                }
                std_normal_pdf(z) * lc * acc
            };
            let c = quad::integrate(fc, -12.0, 12.0, 1e-15, 1e-300, 2000).0;
            let s = quad::integrate(fs, -12.0, 12.0 + w, 1e-15, 1e-300, 2000).0;
            (k as f64 * c, k as f64 * s)
        };
        if df.is_infinite() {
            return r(q);
        }
        let a = 0.5 * df;
        let dens = |u: f64| {
            if u <= 0.0 {
                0.0
            } else {
                2.0 * special::ln_gamma_prefactor(a, a * u * u).exp() / u
            }
        };
        let hi = (2.0 * special::gammainc_inv_pq(a, 1.0 - 1e-25, 1e-25) / df).sqrt();
        let c = quad::integrate(|u| dens(u) * r(q * u).0, 0.0, hi, 1e-14, 1e-300, 4000).0;
        let s = quad::integrate(|u| dens(u) * r(q * u).1, 0.0, hi, 1e-13, 1e-300, 4000).0;
        (c, s)
    }
    let mut ck = Checker::new("studentized_range_vs_nested");
    for k in [2usize, 3, 10, 20] {
        for df in [1.0, 2.0, 5.0, 20.0, 1000.0, f64::INFINITY] {
            let d = StudentizedRange::new(k, df).unwrap();
            for q in [0.5, 2.0, 4.0, 8.0, 12.0] {
                let (rc, rs) = reference(k, df, q);
                let (c, s) = d.cdf_sf(q);
                let ctx = format!("k={k} df={df} q={q}");
                ck.check(&ctx, "cdf", rc, c, Tol::new(0.0, 1e-12));
                ck.check(&ctx, "sf", rs, s, Tol::new(1e-9, 1e-15));
            }
        }
    }
    ck.finish();
}
