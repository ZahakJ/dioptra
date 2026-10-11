//! Bland-Altman against fixtures/oracle/bland_altman.json; ICC and kappa are hand-tested
//! in the module (no oracle fixture).
mod common;

#[test]
fn bland_altman_fixture() {
    common::families::bland_altman_family().finish();
}
