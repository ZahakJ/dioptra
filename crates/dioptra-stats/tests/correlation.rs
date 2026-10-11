//! Pearson, Spearman (t approximation and exact) and Kendall (exact and asymptotic)
//! against fixtures/oracle/correlation.json.
mod common;
use common::families;
use dioptra_stats::correlation::{correlation_matrix, CorrelationMethod};
use dioptra_stats::posthoc::Adjust;

#[test]
fn correlation_fixture() {
    families::correlation_family().finish();
}

#[test]
fn matrix_is_symmetric_and_adjusted() {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [2.0, 1.0, 4.0, 3.0, 6.0, 5.0];
    let c = [6.0, 4.0, 5.0, 2.0, 3.0, f64::NAN];
    let m = correlation_matrix(
        &[&a, &b, &c],
        CorrelationMethod::Pearson,
        Adjust::Bonferroni,
    )
    .unwrap();
    assert_eq!(m.n[0][2], 5);
    for i in 0..3 {
        assert_eq!(m.r[i][i], 1.0);
        for j in 0..3 {
            assert_eq!(m.r[i][j].to_bits(), m.r[j][i].to_bits());
            if i != j {
                assert!((m.p_adjusted[i][j] - (3.0 * m.p[i][j]).min(1.0)).abs() < 1e-15);
            }
        }
    }
}
