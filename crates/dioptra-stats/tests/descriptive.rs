//! Descriptive statistics against fixtures/oracle/descriptive.json.
mod common;
use dioptra_stats::descriptive::{self as d, Summary, SummaryOptions};

#[test]
fn descriptive_fixture() {
    common::families::descriptive().finish();
}

#[test]
fn missing_values_are_skipped() {
    let x = [1.0, f64::NAN, 2.0, 3.0, f64::NAN];
    let s = Summary::compute(&x, &SummaryOptions::default()).unwrap();
    assert_eq!(s.n, 3);
    assert_eq!(s.n_missing, 2);
    assert_eq!(s.mean, 2.0);
    assert!(d::mean(&[f64::NAN]).is_err());
    assert!(d::mean(&[1.0, f64::INFINITY]).is_err());
}
