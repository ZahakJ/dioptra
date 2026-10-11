//! NIST StRD univariate summary statistics: log relative error (LRE, McCullough 1998)
//! of mean, SD (n-1) and lag-1 autocorrelation.
//!
//! Two references are used:
//! * the NIST certified values (computed on the decimal data), scored against the
//!   difficulty-based minimums of research/05-nist-and-oracle-notes.md §1;
//! * the same statistics computed in exact rational arithmetic on the data *as parsed into
//!   doubles*. These isolate the algorithm from the input rounding: NumAcc3/4 (1e6 + 0.1
//!   and 1e7 + 0.1 steps) are not representable in binary, which caps any double-precision
//!   SD at 9.46 and 8.25 digits against the certified 0.1 (and r1 at 12.2 and 11.0).
//!   Against the double-input reference we require 13 digits everywhere.
mod common;
use common::families::nist_univariate;

#[test]
fn nist_univariate_lre() {
    let mut fails = Vec::new();
    let mut worst_cert = f64::INFINITY;
    let mut worst_exact = f64::INFINITY;
    for row in nist_univariate() {
        let [lm, ls, lr] = row.certified;
        let [xm, xs, xr] = row.exact_on_doubles;
        println!(
            "{:9} {:8} certified LRE: mean {lm:5.2} SD {ls:5.2} r1 {lr:5.2} | exact-on-doubles: mean {xm:5.2} SD {xs:5.2} r1 {xr:5.2}",
            row.name, row.difficulty
        );
        worst_cert = worst_cert.min(lm.min(ls).min(lr));
        worst_exact = worst_exact.min(xm.min(xs).min(xr));
        // research/05 §1 minimums by difficulty: mean 14; SD 13/10/8; r1 12/9/7.
        let (sd_min, r1_min) = match row.difficulty.as_str() {
            "Lower" => (13.0, 12.0),
            "Average" => (10.0, 9.0),
            _ => (8.0, 7.0),
        };
        // NumAcc3 is "Average" but its inputs cap the SD at 9.46 digits (see above).
        let sd_min = if row.name == "NumAcc3" { 9.0 } else { sd_min };
        for (what, l, need) in [
            ("mean", lm, 14.0),
            ("SD", ls, sd_min),
            ("r1", lr, r1_min),
            ("mean (exact-on-doubles)", xm, 13.0),
            ("SD (exact-on-doubles)", xs, 13.0),
            ("r1 (exact-on-doubles)", xr, 13.0),
        ] {
            if l < need {
                fails.push(format!("{}: {what} LRE {l:.2} < {need}", row.name));
            }
        }
    }
    println!("minimum LRE vs NIST certified: {worst_cert:.2}; vs exact statistics of the double inputs: {worst_exact:.2}");
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
