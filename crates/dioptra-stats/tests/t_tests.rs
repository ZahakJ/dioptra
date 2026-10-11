//! t tests against fixtures/oracle/t_tests.json.
mod common;
use dioptra_stats::ttest;

#[test]
fn t_tests_fixture() {
    common::families::t_tests().finish();
}

#[test]
fn hedges_j_exact() {
    // J(18) = Gamma(9)/(3 Gamma(8.5))
    let j = ttest::hedges_j(18.0);
    let direct =
        (dioptra_stats::special::ln_gamma(9.0) - dioptra_stats::special::ln_gamma(8.5)).exp() / 3.0;
    assert!((j - direct).abs() < 1e-14);
    assert!(ttest::hedges_j(1.0).is_nan());
    // large df: J -> 1 - 3/(4 df - 1) + O(df^-2)
    let df = 1e6;
    assert!((ttest::hedges_j(df) - (1.0 - 3.0 / (4.0 * df - 1.0))).abs() < 1e-11);
}
