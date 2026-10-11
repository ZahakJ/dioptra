//! NIST StRD one-way ANOVA: LRE (McCullough 1998) of SS, MS, F, R^2 and residual SD
//! against the certified values and against the exact statistics of the inputs as parsed
//! into doubles (rational arithmetic, tests/common/families.rs).
//!
//! The SmLs04-09 responses (1e6 + 0.x and 1e12 + 0.x) are not representable in binary.
//! On the parsed doubles the exact SS_between of SmLs05 is 16.080000001825 (certified
//! 16.08) and of SmLs07 1.680156 (certified 1.68): no double-precision method can exceed
//! LRE 9.94 on SmLs05/06 or about 4 on SmLs07-09 against the certified values. Those sets
//! are held to that cap against the certified values and to 13 digits against the exact
//! statistics of their double inputs.
mod common;
use common::families::nist_anova;

#[test]
fn nist_anova_lre() {
    let mut fails = Vec::new();
    for r in nist_anova() {
        println!(
            "{:8} {:8} SSB {:5.2} SSW {:5.2} MSB {:5.2} MSW {:5.2} F {:5.2} R2 {:5.2} resSD {:5.2} | min {:5.2} | exact-on-doubles min {:5.2}",
            r.name, r.difficulty, r.ss_between, r.ss_within, r.ms_between, r.ms_within, r.f, r.r_squared, r.residual_sd, r.min, r.min_exact_on_doubles
        );
        // research/05 section 1 asks 12/9/7 by difficulty; the task asks >= 10 on lower and
        // average. Input-capped sets: see the module comment.
        let need = match r.name.as_str() {
            "SmLs05" | "SmLs06" => 9.9,
            "SmLs07" | "SmLs08" | "SmLs09" => 3.9,
            _ => match r.difficulty.as_str() {
                "Lower" => 13.0,
                _ => 10.0,
            },
        };
        if r.min < need {
            fails.push(format!(
                "{}: min LRE {:.2} < {need} (certified)",
                r.name, r.min
            ));
        }
        if r.min_exact_on_doubles.is_nan() || r.min_exact_on_doubles < 13.0 {
            fails.push(format!(
                "{}: min LRE {:.2} < 13 (exact on doubles)",
                r.name, r.min_exact_on_doubles
            ));
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
