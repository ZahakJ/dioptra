//! Prints the correctness table for `docs/CORRECTNESS.md`:
//! `cargo run -p dioptra-stats --example correctness_report`.
//!
//! It runs the same comparisons as the integration tests (tests/common/families.rs) and
//! reports, per fixture family, the number of cases and comparisons, failures against the
//! fixture tolerances, the worst relative error and the minimum number of correct digits
//! (-log10 of the relative error, capped at 17), and the NIST StRD univariate, linear
//! regression and one-way ANOVA LREs.

#[path = "../tests/common/mod.rs"]
mod common;

use common::families;

fn main() {
    println!("## Oracle fixtures\n");
    println!("| Family | Cases | Comparisons | Failures | Worst error / tolerance | Max rel. error | Min digits |");
    println!("|---|---:|---:|---:|---:|---:|---:|");
    let mut total_fail = 0;
    for ck in families::all_families() {
        total_fail += ck.failures.len();
        let digits = if ck.min_digits.is_finite() {
            format!("{:.1}", ck.min_digits)
        } else {
            "-".into()
        };
        println!(
            "| {} | {} | {} | {} | {:.1e} | {:.1e} | {} |",
            ck.family,
            ck.cases,
            ck.count,
            ck.failures.len(),
            ck.max_tol_ratio,
            ck.max_rel,
            digits
        );
        for f in ck.failures.iter().take(5) {
            eprintln!("  FAIL {f}");
        }
    }
    println!(
        "\nWorst error / tolerance: the largest |got - expected| / max(abs, rel |expected|) over"
    );
    println!(
        "the family, with each case's own tolerance (< 1 passes). Max rel. error is over expected"
    );
    println!(
        "values with |value| > 1e-290, including tiny values where the abs tolerance governs;"
    );
    println!("min digits = -log10(max rel. error).\n");

    println!("## NIST StRD univariate (LRE, McCullough 1998)\n");
    println!("| Dataset | Difficulty | Mean | SD | Lag-1 r | Mean* | SD* | Lag-1 r* |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|");
    let mut worst = f64::INFINITY;
    let mut worst_exact = f64::INFINITY;
    for r in families::nist_univariate() {
        let [a, b, c] = r.certified;
        let [x, y, z] = r.exact_on_doubles;
        worst = worst.min(a.min(b).min(c));
        worst_exact = worst_exact.min(x.min(y).min(z));
        println!(
            "| {} | {} | {a:.1} | {b:.1} | {c:.1} | {x:.1} | {y:.1} | {z:.1} |",
            r.name, r.difficulty
        );
    }
    println!("\nLRE capped at 15 (NIST certifies 15 digits). Columns without a star score against the NIST");
    println!("certified values; starred columns against the exact statistics of the inputs as parsed into");
    println!(
        "doubles (rational arithmetic), which separates the algorithm from input rounding: the"
    );
    println!(
        "NumAcc3/NumAcc4 values (1e6 + 0.1 and 1e7 + 0.1 steps) are not representable in binary,"
    );
    println!("which caps any double-precision SD at 9.5 and 8.3 digits against the certified 0.1.");
    println!("Minimum LRE: {worst:.2} against certified values, {worst_exact:.2} against the exact statistics of the double inputs.");

    println!("\n## NIST StRD linear regression (LRE)\n");
    println!("| Dataset | Difficulty | Parameters | SDs | R^2 | Residual SD | ANOVA table | Min | Min* |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|");
    for r in families::nist_linear() {
        println!(
            "| {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |",
            r.name,
            r.difficulty,
            r.params,
            r.sds,
            r.r_squared,
            r.residual_sd,
            r.anova,
            r.min,
            r.min_exact_on_doubles
        );
    }
    println!("\nParameters and SDs: minimum over the coefficients. ANOVA table: minimum over the");
    println!("regression and residual SS and MS and F (Wampler1's F is infinite and not scored).");
    println!("Min*: minimum LRE against the exact least-squares solution for the inputs as parsed");
    println!("into doubles (rational arithmetic; values below 1e-12 scored by absolute error).");

    println!("\n## NIST StRD one-way ANOVA (LRE)\n");
    println!("| Dataset | Difficulty | SS between | SS within | MS between | MS within | F | R^2 | Residual SD | Min | Min* |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in families::nist_anova() {
        println!(
            "| {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |",
            r.name,
            r.difficulty,
            r.ss_between,
            r.ss_within,
            r.ms_between,
            r.ms_within,
            r.f,
            r.r_squared,
            r.residual_sd,
            r.min,
            r.min_exact_on_doubles
        );
    }
    println!(
        "\nMin*: minimum LRE against the exact statistics of the inputs as parsed into doubles."
    );
    if total_fail > 0 {
        eprintln!("{total_fail} comparison(s) outside tolerance");
        std::process::exit(1);
    }
}
