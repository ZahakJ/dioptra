//! Simple, multiple and Deming regression against fixtures/oracle/linear_regression.json.
mod common;
use common::families;

#[test]
fn linear_regression_fixture() {
    families::linear_regression_family().finish();
}
