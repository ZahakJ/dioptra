//! Timing checks (run with `cargo test -p dioptra-stats --release --test benches -- --ignored --nocapture`).
use dioptra_stats::dist::{Continuous, Dunnett, NoncentralT, StudentizedRange};
use dioptra_stats::mannwhitney::{mann_whitney, MannWhitneyOptions, Method};
use dioptra_stats::{normality, special};
use std::hint::black_box;
use std::time::Instant;

fn time<F: FnMut() -> f64>(label: &str, reps: usize, mut f: F) -> f64 {
    let t0 = Instant::now();
    let mut acc = 0.0;
    for _ in 0..reps {
        acc += f();
    }
    let per = t0.elapsed().as_secs_f64() / reps as f64;
    black_box(acc);
    println!("{label:48} {:>12.3} us/call", per * 1e6);
    per
}

/// Deterministic pseudo-normal data (Box–Muller on a fixed LCG) so benches need no RNG crate.
fn data(n: usize, seed: u64) -> Vec<f64> {
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

#[test]
#[ignore]
fn bench_special_functions() {
    let mut x = 0.0;
    let b = time("betainc(2.5, 7.5, x) body", 200_000, || {
        x = (x + 0.37) % 1.0;
        special::betainc(2.5, 7.5, x)
    });
    let g = time("gammainc_p(4.5, x) body", 200_000, || {
        x = (x + 0.37) % 10.0;
        special::gammainc_p(4.5, x)
    });
    time("betainc(50, 50, x) body", 100_000, || {
        x = (x + 0.0371) % 1.0;
        special::betainc(50.0, 50.0, x)
    });
    time("betainc_inv(2, 3, p)", 50_000, || {
        x = (x + 0.0371) % 1.0;
        special::betainc_inv(2.0, 3.0, x.max(1e-9))
    });
    time("norm_quantile(p)", 1_000_000, || {
        x = (x + 0.0371) % 1.0;
        special::norm_quantile(x.max(1e-300))
    });
    assert!(
        b < 1e-6 * 2.0 && g < 1e-6 * 2.0,
        "special functions slower than ~1 us"
    );
}

#[test]
#[ignore]
fn bench_tukey_dunnett_nct() {
    let d = StudentizedRange::new(5, 20.0).unwrap();
    let mut q = 1.0;
    let per = time("StudentizedRange(k=5, df=20).cdf_sf", 2_000, || {
        q = 1.0 + (q + 0.731) % 6.0;
        d.cdf_sf(q).1
    });
    let d10 = StudentizedRange::new(10, 60.0).unwrap();
    time("StudentizedRange(k=10, df=60).cdf_sf", 2_000, || {
        q = 1.0 + (q + 0.731) % 6.0;
        d10.cdf_sf(q).1
    });
    time("StudentizedRange::new(5, 20) (node setup)", 2_000, || {
        StudentizedRange::new(5, 20.0).unwrap().df
    });
    time("StudentizedRange(k=5, df=20).ppf(0.95)", 50, || d.ppf(0.95));
    let du = Dunnett::new(vec![0.77, 0.67, 0.75], 24.0).unwrap();
    time("Dunnett(k=3, unequal).p_value", 200, || {
        du.p_value(2.5, true)
    });
    time("Dunnett(k=3, unequal).quantile(0.95)", 10, || {
        du.quantile(0.95, true)
    });
    let nct = NoncentralT::new(10.0, 1.5).unwrap();
    time("NoncentralT(10, 1.5).cdf (AS 243)", 20_000, || {
        nct.cdf_sf(2.0).0
    });
    let nct2 = NoncentralT::new(5.0, 8.0).unwrap();
    time("NoncentralT(5, 8).cdf(-3) (tail quadrature)", 2_000, || {
        nct2.cdf_sf(-3.0).0
    });
    assert!(per < 2e-3, "studentized range cdf slower than 2 ms");
}

#[test]
#[ignore]
fn bench_tests() {
    let x = data(5000, 7);
    let per = time("Shapiro-Wilk n=5000", 50, || {
        normality::shapiro_wilk(&x).unwrap().p
    });
    assert!(per < 5e-3, "Shapiro–Wilk n=5000 slower than 5 ms");
    let a = data(50, 1);
    let b: Vec<f64> = data(50, 2).iter().map(|v| v + 0.3).collect();
    let per = time("Mann-Whitney exact 50x50", 20, || {
        mann_whitney(
            &a,
            &b,
            &MannWhitneyOptions {
                method: Method::Exact,
                ..Default::default()
            },
        )
        .unwrap()
        .p
    });
    assert!(per < 20e-3, "Mann–Whitney exact 50x50 slower than 20 ms");
    let a = data(100, 3);
    let b = data(100, 4);
    time("Mann-Whitney exact 100x100", 3, || {
        mann_whitney(
            &a,
            &b,
            &MannWhitneyOptions {
                method: Method::Exact,
                ..Default::default()
            },
        )
        .unwrap()
        .p
    });
}

#[test]
#[ignore]
fn bench_fisher_rxc_and_tukey() {
    use dioptra_stats::contingency::fisher_exact_rxc;
    use dioptra_stats::posthoc::tukey_kramer;
    // Largest supported shapes (N <= 200, r c <= 30), moderately structured counts.
    let tables: Vec<(&str, Vec<Vec<u64>>)> = vec![
        (
            "Fisher r x c 3x3 N=60",
            vec![vec![8, 5, 7], vec![4, 9, 7], vec![6, 6, 8]],
        ),
        (
            "Fisher r x c 2x10 N=100",
            vec![
                vec![3, 6, 4, 7, 5, 6, 4, 5, 3, 7],
                vec![7, 4, 6, 3, 5, 4, 6, 5, 7, 3],
            ],
        ),
        (
            "Fisher r x c 3x5 N=150",
            vec![
                vec![12, 9, 11, 8, 10],
                vec![8, 12, 10, 11, 9],
                vec![10, 9, 9, 11, 11],
            ],
        ),
        (
            "Fisher r x c 5x6 N=60",
            vec![
                vec![3, 2, 1, 2, 2, 2],
                vec![1, 3, 2, 2, 2, 2],
                vec![2, 2, 3, 1, 2, 2],
                vec![2, 2, 2, 3, 1, 2],
                vec![2, 1, 2, 2, 3, 2],
            ],
        ),
        (
            "Fisher r x c 4x5 N=200",
            vec![
                vec![12, 8, 10, 9, 11],
                vec![9, 11, 10, 12, 8],
                vec![10, 10, 11, 9, 10],
                vec![9, 11, 9, 10, 11],
            ],
        ),
        (
            "Fisher r x c 3x10 N=200",
            vec![
                vec![7, 6, 8, 5, 7, 6, 8, 7, 6, 7],
                vec![6, 7, 5, 8, 6, 7, 6, 7, 7, 6],
                vec![7, 7, 7, 7, 7, 7, 6, 6, 7, 7],
            ],
        ),
        (
            "Fisher r x c 5x6 N=200",
            vec![
                vec![8, 6, 7, 6, 7, 6],
                vec![6, 8, 6, 7, 6, 7],
                vec![7, 6, 8, 6, 7, 6],
                vec![6, 7, 6, 8, 6, 7],
                vec![7, 6, 7, 6, 8, 6],
            ],
        ),
        (
            "Fisher r x c 5x6 N=200 associated",
            vec![
                vec![16, 9, 6, 4, 3, 2],
                vec![9, 14, 8, 5, 3, 1],
                vec![5, 8, 14, 7, 4, 2],
                vec![3, 4, 7, 13, 9, 4],
                vec![2, 3, 4, 7, 12, 12],
            ],
        ),
        (
            "Fisher r x c 2x15 N=200",
            vec![
                vec![7, 6, 8, 5, 7, 6, 8, 7, 6, 7, 5, 8, 6, 7, 7],
                vec![6, 7, 5, 8, 6, 7, 6, 7, 7, 6, 8, 6, 7, 7, 7],
            ],
        ),
    ];
    for (label, t) in &tables {
        let t0 = Instant::now();
        let r = fisher_exact_rxc(t);
        let el = t0.elapsed().as_secs_f64();
        match r {
            Ok(r) => println!(
                "{label:48} {:>12.3} ms  p = {:.6e}",
                el * 1e3,
                r.p_two_sided
            ),
            Err(e) => println!("{label:48} {:>12.3} ms  {e}", el * 1e3),
        }
    }
    let groups: Vec<Vec<f64>> = (0..10).map(|k| data(12 + k, 100 + k as u64)).collect();
    let refs: Vec<&[f64]> = groups.iter().map(|g| g.as_slice()).collect();
    time("Tukey-Kramer k=10 (45 pairs + q_crit)", 3, || {
        tukey_kramer(&refs, 0.05).unwrap().comparisons[0].p_adj
    });
}
