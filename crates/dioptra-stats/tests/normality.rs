//! Normality tests against fixtures/oracle/normality.json.
mod common;

#[test]
fn normality_fixture() {
    common::families::normality_family().finish();
}
