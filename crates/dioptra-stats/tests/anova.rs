//! One-way, two-way, repeated-measures ANOVA, Kruskal-Wallis and Friedman against
//! fixtures/oracle/{anova_oneway, anova_twoway, anova_rm, kruskal_friedman_dunn}.json.
mod common;
use common::families;

#[test]
fn anova_oneway_fixture() {
    families::anova_oneway_family().finish();
}

#[test]
fn anova_twoway_fixture() {
    families::anova_twoway_family().finish();
}

#[test]
fn anova_rm_fixture() {
    families::anova_rm_family().finish();
}

#[test]
fn kruskal_friedman_dunn_fixture() {
    families::kruskal_friedman_dunn_family().finish();
}
