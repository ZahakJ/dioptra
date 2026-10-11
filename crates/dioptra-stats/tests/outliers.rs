//! Grubbs and ROUT against fixtures/oracle/outliers.json.
mod common;

#[test]
fn outliers_fixture() {
    common::families::outliers_family().finish();
}
