//! ROC curves against fixtures/oracle/roc.json (sklearn, DeLong by Sun & Xu, Hanley-McNeil).
mod common;

#[test]
fn roc_fixture() {
    common::families::roc_family().finish();
}
