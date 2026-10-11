//! Outlier identification in each data set: iterated Grubbs at α, or ROUT at Q. Flagged
//! values are reported with their rows; nothing is excluded automatically.

use dioptra_stats::outliers::{grubbs, rout_column};

use super::checks;
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec, OutlierMethod};
use crate::results::CheckStatus;

/// Identify outliers.
pub fn outliers(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Outliers(spec) = ctx.spec else {
        return Err("not an outliers spec".into());
    };
    let t = ctx.table;
    let gs = extract::groups(t, &spec.columns, None)?;
    extract::require_raw(&gs, "Outlier identification")?;
    if gs.is_empty() {
        return Err("There are no numeric data sets.".into());
    }
    let (mname, setting) = match spec.method {
        OutlierMethod::Grubbs { alpha } => {
            if !(alpha > 0.0 && alpha < 1.0) {
                return Err(format!("Grubbs' α must be between 0 and 1 (got {alpha})."));
            }
            ("Grubbs' test", format!("α = {}", g(alpha)))
        }
        OutlierMethod::Rout { q } => {
            if !(q > 0.0 && q < 100.0) {
                return Err(format!(
                    "ROUT's Q is a percentage between 0 and 100 (got {q}); 1 is the usual choice."
                ));
            }
            ("ROUT", format!("Q = {}%", g(q)))
        }
    };
    let mut s = sheet("outliers", "Identify outliers");
    s.methods = methods(&format!(
        "{} ({setting}){}",
        match spec.method {
            OutlierMethod::Grubbs { .. } => "Iterated two-sided Grubbs' test",
            OutlierMethod::Rout { .. } =>
                "ROUT (robust fit of a constant with false discovery rate control)",
        },
        "; flagged values are reported, not removed"
    ));
    let mut flagged_rows = Vec::new();
    let mut clean_rows = Vec::new();
    let mut steps_rows = Vec::new();
    let mut skipped = Vec::new();
    let mut cleaned_sets = Vec::new();
    let mut total = 0;
    for gr in &gs {
        let v = &gr.values;
        if v.len() < 3 {
            skipped.push(format!("{}: fewer than 3 values, not tested.", gr.name));
            continue;
        }
        let idx: Vec<usize> = match spec.method {
            OutlierMethod::Grubbs { alpha } => {
                let r = grubbs(v, alpha).map_err(|e| format!("{}: {e}", gr.name))?;
                for (k, st_) in r.steps.iter().enumerate() {
                    steps_rows.push(vec![
                        txt(gr.name.clone()),
                        int(k + 1),
                        int(st_.n),
                        num(st_.suspect_value),
                        num(st_.g),
                        num(st_.g_critical),
                        pv(st_.p),
                        yes_no(st_.significant),
                    ]);
                }
                r.outlier_indices
            }
            OutlierMethod::Rout { q } => {
                rout_column(v, q / 100.0)
                    .map_err(|e| format!("{}: {e}", gr.name))?
                    .outlier_indices
            }
        };
        let mut sorted = idx.clone();
        sorted.sort_unstable();
        for &i in &sorted {
            let row = gr.rows.get(i).copied().unwrap_or(i);
            let addr = match gr.dataset {
                Some(d) if t.kind != crate::api::TableKind::Nested => {
                    t.address(row, t.flat_index(d, 0).unwrap_or(0))
                }
                _ => format!("{}", row + 1),
            };
            flagged_rows.push(vec![
                txt(gr.name.clone()),
                int(row + 1),
                txt(addr),
                num(v[i]),
            ]);
        }
        let kept: Vec<f64> = v
            .iter()
            .enumerate()
            .filter(|(i, _)| !idx.contains(i))
            .map(|(_, x)| *x)
            .collect();
        let (m0, s0) = mean_sd(v);
        let (m1, s1) = mean_sd(&kept);
        clean_rows.push(vec![
            txt(gr.name.clone()),
            int(v.len()),
            int(idx.len()),
            num(m0),
            num(s0),
            num(m1),
            num(s1),
        ]);
        total += idx.len();
        s.summary.push(if idx.is_empty() {
            format!("{}: no outlier flagged by {mname} ({setting}), n = {}.", gr.name, v.len())
        } else {
            format!(
                "{}: {} flagged by {mname} ({setting}): {}. Without {}, M = {}, SD = {} (with: M = {}, SD = {}).",
                gr.name,
                if idx.len() == 1 { "1 value".to_string() } else { format!("{} values", idx.len()) },
                sorted.iter().map(|&i| format!("{} (row {})", g(v[i]), gr.rows.get(i).copied().unwrap_or(i) + 1)).collect::<Vec<_>>().join(", "),
                if idx.len() == 1 { "it" } else { "them" },
                g(m1),
                g(s1),
                g(m0),
                g(s0)
            )
        });
        cleaned_sets.push((gr.name.clone(), kept));
    }
    s.sections.push(section(
        "Flagged values",
        &["Data set", "Row", "Cell", "Value"],
        flagged_rows,
    ));
    s.sections.push(section(
        "Before and after",
        &[
            "Data set",
            "n",
            "Flagged",
            "Mean (all)",
            "SD (all)",
            "Mean (without flagged)",
            "SD (without flagged)",
        ],
        clean_rows,
    ));
    if !steps_rows.is_empty() {
        s.sections.push(section(
            "Grubbs' steps",
            &[
                "Data set",
                "Step",
                "n",
                "Suspect value",
                "G",
                "Critical G",
                "P (approx.)",
                "Outlier?",
            ],
            steps_rows,
        ));
    }
    let mut notes: Vec<String> = gs.iter().flat_map(|g| g.notes.iter().cloned()).collect();
    notes.extend(skipped.iter().cloned());
    s.assumptions
        .push(checks::values_used(&notes, !skipped.is_empty()));
    let (c, sec) = checks::normality(
        &cleaned_sets,
        "the values without the flagged ones",
        "Outlier tests assume the bulk of the data is Gaussian; with skewed data, extreme values are expected and should not be called outliers. Consider a transformation or a rank-based analysis.",
    );
    s.assumptions.push(c);
    s.sections.push(sec);
    s.assumptions.push(AssumptionCheck::new(
        "Decision to exclude",
        if total > 0 { CheckStatus::Caution } else { CheckStatus::Pass },
        if total > 0 {
            "Values are flagged, not removed. Exclude a value (it stays in the table, marked) only with a reason beyond the test, such as a recording error, and say so in the methods."
        } else {
            "No value was flagged."
        },
    ));
    if matches!(spec.method, OutlierMethod::Grubbs { .. }) {
        s.checklist.push("Grubbs' test removes one value at a time; a second outlier can mask the first. ROUT handles several outliers at once.".into());
    }
    let keys: Vec<Option<usize>> = gs.iter().map(|g| g.dataset).collect();
    s.graph_family = Some("strip".into());
    s.graph = Some(graph_for("strip", &keys));
    Ok(s)
}
