//! NIST StRD nonlinear regression: all 27 sets from both certified starting points, LRE
//! (McCullough 1998) of the parameters, their SDs, the residual SS and the residual SD.
mod common;
use common::families::nist_nonlinear;

#[test]
fn nist_nonlinear_lre() {
    let mut fails = Vec::new();
    for r in nist_nonlinear() {
        for (k, s) in r.starts.iter().enumerate() {
            println!(
                "{:9} {:7} start{} {:13} it {:4} params {:5.2} SDs {:5.2} SS {:5.2} resSD {:5.2}",
                r.name,
                r.difficulty,
                k + 1,
                s.status,
                s.iterations,
                s.params,
                s.sds,
                s.ss,
                s.residual_sd
            );
            if !s.converged {
                fails.push(format!("{} start {}: {}", r.name, k + 1, s.status));
                continue;
            }
            // Parameters: 9 for every set (asked: lower >= 7, average >= 6, higher >= 4).
            if s.params < 9.0 {
                fails.push(format!(
                    "{} start {}: parameter LRE {:.2}",
                    r.name,
                    k + 1,
                    s.params
                ));
            }
            // SDs, SS and residual SD: 9, except Lanczos1, whose certified SS (1.43e-25)
            // is below what the double-rounded data can represent (see CORRECTNESS.md).
            let need = if r.name == "Lanczos1" { 2.5 } else { 9.0 };
            for (what, v) in [("SD", s.sds), ("SS", s.ss), ("residual SD", s.residual_sd)] {
                if v < need {
                    fails.push(format!("{} start {}: {what} LRE {v:.2}", r.name, k + 1));
                }
            }
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
