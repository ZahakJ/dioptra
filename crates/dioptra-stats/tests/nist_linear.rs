//! NIST StRD linear regression: LRE (McCullough 1998) of the parameters, their SDs, R^2,
//! residual SD and the ANOVA table, against the certified values and against the exact
//! least-squares solution of the inputs as parsed into doubles.
mod common;
use common::families::nist_linear;

#[test]
fn nist_linear_lre() {
    let mut fails = Vec::new();
    for r in nist_linear() {
        println!(
            "{:9} {:8} params {:5.2} SDs {:5.2} R2 {:5.2} resSD {:5.2} ANOVA {:5.2} | min {:5.2} | exact-on-doubles min {:5.2}",
            r.name, r.difficulty, r.params, r.sds, r.r_squared, r.residual_sd, r.anova, r.min, r.min_exact_on_doubles
        );
        // Bars asked for: Filip >= 7 (McCullough's "acceptable"), Longley >= 11, 13 on the
        // lower/average sets and Wampler1-2. Enforced with margin above those bars.
        let need = match r.name.as_str() {
            "Filip" | "Longley" | "Wampler3" | "Wampler4" | "Wampler5" => 12.0,
            _ => 13.0,
        };
        if r.min < need {
            fails.push(format!("{}: min LRE {:.2} < {need}", r.name, r.min));
        }
        if r.min_exact_on_doubles.is_nan() || r.min_exact_on_doubles < 12.0 {
            fails.push(format!(
                "{}: min LRE {:.2} < 12 against the exact solution for the double inputs",
                r.name, r.min_exact_on_doubles
            ));
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// The closed-form simple-regression path (shifted, centred, compensated sums) on the
/// straight-line sets: Norris (intercept) and NoInt1/NoInt2 (through the origin).
#[test]
fn nist_simple_path() {
    use common::families::lre;
    use dioptra_stats::regression::{simple_linear_regression, SimpleRegressionOptions};
    for name in ["Norris", "NoInt1", "NoInt2"] {
        let j = common::load(&format!("nist/{name}.json"));
        let x = common::nums(&j["data"]["x"]);
        let y = common::nums(&j["data"]["y"]);
        let origin = !j["model"]["intercept"].as_bool().unwrap();
        let r = simple_linear_regression(
            &x,
            &y,
            SimpleRegressionOptions {
                through_origin: origin,
                ..Default::default()
            },
        )
        .unwrap();
        let ps = j["parameters"].as_array().unwrap();
        let mut got = vec![];
        if origin {
            got.push((r.slope, r.se_slope));
        } else {
            got.push((r.intercept, r.se_intercept));
            got.push((r.slope, r.se_slope));
        }
        let mut m = f64::INFINITY;
        for (p, (b, s)) in ps.iter().zip(got) {
            let (lb, ls) = (
                lre(b, p["certified"].as_f64().unwrap()),
                lre(s, p["std_dev"].as_f64().unwrap()),
            );
            println!("  {name} {}: estimate {lb:.2} SD {ls:.2}", p["name"]);
            m = m.min(lb).min(ls);
        }
        m = m.min(lre(
            r.sy_x,
            j["residual_std_dev"]["value"].as_f64().unwrap(),
        ));
        m = m.min(lre(r.r_squared, j["r_squared"]["value"].as_f64().unwrap()));
        println!("{name}: simple-regression path min LRE {m:.2}");
        assert!(m >= 13.0, "{name}: {m}");
    }
}
