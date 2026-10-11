//! Chi-square, Fisher (2x2 and r x c), McNemar and Cochran-Armitage against
//! fixtures/oracle/contingency.json.
mod common;
use common::families;

#[test]
fn contingency_fixture() {
    families::contingency_family().finish();
}
