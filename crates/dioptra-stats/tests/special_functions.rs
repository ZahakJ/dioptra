//! Incomplete beta and gamma functions, their inverses, lgamma and digamma against the
//! 32-digit mpmath references in fixtures/oracle/distributions.json.
mod common;

#[test]
fn incomplete_beta_and_inverse() {
    common::families::incomplete_beta().finish();
}

#[test]
fn incomplete_gamma_and_inverse() {
    common::families::incomplete_gamma().finish();
}
