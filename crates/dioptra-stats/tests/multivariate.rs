//! PCA against fixtures/oracle/pca.json (sklearn on the standardized matrix).
mod common;

#[test]
fn pca_fixture() {
    common::families::pca_family().finish();
}
