//! Wilcoxon signed-rank against fixtures/oracle/wilcoxon_signed_rank.json.
mod common;
use dioptra_stats::wilcoxon::signed_rank_distribution;

#[test]
fn wilcoxon_fixture() {
    common::families::wilcoxon_family().finish();
}

#[test]
fn signed_rank_distribution_small() {
    // n = 3: W+ in {0,1,2,3,3,4,5,6} -> counts 1,1,1,2,1,1,1 over 8
    let d = signed_rank_distribution(3);
    let want = [1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0].map(|c| c / 8.0);
    assert_eq!(d.len(), 7);
    for (a, b) in d.iter().zip(want) {
        assert!((a - b).abs() < 1e-16);
    }
}
