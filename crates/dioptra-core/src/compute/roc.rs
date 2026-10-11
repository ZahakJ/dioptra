//! ROC curve: sensitivity and specificity at every cut-off with Wilson CIs, likelihood
//! ratios, AUC with DeLong and Hanley–McNeil CIs, and the Youden cut-off.

use dioptra_stats::roc::{roc as roc_curve, ProportionCi, RocOptions};

use super::checks;
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec};
use crate::api::{TableKind, VariableType};
use crate::graph::{Curve, GraphView};
use crate::results::CheckStatus;

/// ROC curve.
pub fn roc(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Roc(spec) = ctx.spec else {
        return Err("not an ROC spec".into());
    };
    check_level(spec.ci_level)?;
    let style = ctx.p_style;
    let t = ctx.table;
    let (neg, pos, nname, pname, keys) = if let (TableKind::Variables, Some(label)) =
        (t.kind, &spec.positive_label)
    {
        // Scores in one variable, class labels in another.
        let score_d = spec
            .columns
            .first()
            .and_then(|c| extract::dataset_of(t, *c))
            .or_else(|| {
                (0..t.datasets.len())
                    .find(|&d| matches!(t.datasets[d].var_type, None | Some(VariableType::Number)))
            })
            .ok_or("Choose the variable that holds the test values.")?;
        let label_d = spec
            .columns
            .get(1)
            .and_then(|c| extract::dataset_of(t, *c))
            .or_else(|| {
                (0..t.datasets.len())
                    .find(|&d| t.datasets[d].var_type == Some(VariableType::Category))
            })
            .ok_or("Choose the category variable that says which cases are positive.")?;
        let sc = &t.datasets[score_d].subcolumns[0];
        let lc = &t.datasets[label_d].subcolumns[0];
        let (mut p, mut n) = (Vec::new(), Vec::new());
        for r in 0..t.rows {
            let v = sc.usable_at(r);
            if !v.is_finite() || lc.excluded.contains(&r) {
                continue;
            }
            let lab = lc.text.get(&r).cloned().or_else(|| {
                let x = lc.values[r];
                x.is_finite().then(|| crate::util::format_sig(x, None, 15))
            });
            match lab {
                Some(l) if l.trim() == label.trim() => p.push(v),
                Some(_) => n.push(v),
                None => {}
            }
        }
        (
            n,
            p,
            "other cases".to_string(),
            format!("\"{label}\""),
            vec![Some(score_d), Some(label_d)],
        )
    } else {
        let gs = extract::groups(t, &spec.columns, Some(2))?;
        extract::require_raw(&gs, "An ROC curve")?;
        if gs.len() < 2 {
            return Err("An ROC curve needs two data sets: controls first, then patients.".into());
        }
        (
            gs[0].values.clone(),
            gs[1].values.clone(),
            gs[0].name.clone(),
            gs[1].name.clone(),
            vec![gs[0].dataset, gs[1].dataset],
        )
    };
    if neg.is_empty() || pos.is_empty() {
        return Err("Both the controls and the patients need at least one value.".into());
    }
    let r = roc_curve(
        &pos,
        &neg,
        RocOptions {
            ci_level: spec.ci_level,
            proportion_ci: ProportionCi::Wilson,
        },
    )
    .map_err(|e| e.to_string())?;
    let lvl = pct(spec.ci_level);
    let mut s = sheet("roc", "ROC curve");
    s.methods = methods(&format!(
        "ROC curve ({pname} positive, higher values indicate positive), AUC by the Mann–Whitney identity with DeLong {lvl} CI, sensitivity and specificity with Wilson CIs, Youden cut-off"
    ));
    s.sections.push(section_owned(
        "Area under the curve",
        vec![
            "Quantity".into(),
            "Value".into(),
            "SE".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
        ],
        vec![
            vec![
                txt("AUC (DeLong)"),
                num(r.auc),
                num(r.se_delong),
                num(r.ci_delong.0),
                num(r.ci_delong.1),
            ],
            vec![
                txt("AUC (Hanley–McNeil)"),
                num(r.auc),
                num(r.se_hanley_mcneil),
                num(r.ci_hanley_mcneil.0),
                num(r.ci_hanley_mcneil.1),
            ],
        ],
    ));
    s.sections.push(kv(
        "Test of AUC = 0.5",
        vec![
            ("z (DeLong SE)".into(), num(r.z)),
            ("P (two-sided)".into(), pv(r.p)),
            (format!("Controls ({nname})"), int(r.n_negative)),
            (format!("Patients ({pname})"), int(r.n_positive)),
        ],
    ));
    let rows: Vec<Vec<_>> = r
        .points
        .iter()
        .filter(|p| p.threshold.is_finite())
        .map(|p| {
            vec![
                num(p.threshold),
                num(100.0 * p.sensitivity),
                num(100.0 * p.sensitivity_ci.0),
                num(100.0 * p.sensitivity_ci.1),
                num(100.0 * p.specificity),
                num(100.0 * p.specificity_ci.0),
                num(100.0 * p.specificity_ci.1),
                num(p.lr_positive),
                num(p.lr_negative),
                num(p.youden),
            ]
        })
        .collect();
    s.sections.push(section_owned(
        "Cut-offs",
        vec![
            "Cut-off (≥)".into(),
            "Sensitivity %".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
            "Specificity %".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
            "LR+".into(),
            "LR−".into(),
            "Youden J".into(),
        ],
        rows,
    ));
    let yb = &r.points[r.youden_best_index];
    let ct = &r.points[r.closest_topleft_index];
    s.sections.push(section(
        "Best cut-offs",
        &[
            "Criterion",
            "Cut-off (≥)",
            "Sensitivity %",
            "Specificity %",
            "Youden J",
        ],
        vec![
            vec![
                txt("Youden (max J)"),
                num(yb.threshold),
                num(100.0 * yb.sensitivity),
                num(100.0 * yb.specificity),
                num(yb.youden),
            ],
            vec![
                txt("Closest to (0, 1)"),
                num(ct.threshold),
                num(100.0 * ct.sensitivity),
                num(100.0 * ct.specificity),
                num(ct.youden),
            ],
        ],
    ));
    s.summary.push(format!(
        "AUC = {} ({lvl} CI {} to {}, DeLong), {} against chance (AUC = 0.5).",
        unit(r.auc, style),
        unit(r.ci_delong.0, style),
        unit(r.ci_delong.1, style),
        p_phrase(r.p, style)
    ));
    s.summary.push(format!(
        "The Youden cut-off ≥ {} gives sensitivity {}% and specificity {}% (J = {}).",
        g(yb.threshold),
        format_sig(100.0 * yb.sensitivity, Some(1), 4),
        format_sig(100.0 * yb.specificity, Some(1), 4),
        unit(yb.youden, style)
    ));
    // Checks.
    s.assumptions.push(if r.auc < 0.5 {
        AssumptionCheck::new(
            "Direction",
            CheckStatus::Caution,
            format!("AUC is below 0.5: in these data higher values point to {nname}, not {pname}."),
        )
        .or_else("Swap the two data sets (controls first) if higher values are expected in the controls.")
    } else {
        AssumptionCheck::new("Direction", CheckStatus::Pass, format!("Higher values point to {pname}, as assumed."))
    });
    s.assumptions.push(checks::sample_size(
        &[(nname.clone(), neg.len()), (pname.clone(), pos.len())],
        10,
    ));
    s.assumptions.push(AssumptionCheck::new(
        "Representative samples",
        CheckStatus::NotChecked,
        "Sensitivity and specificity transfer to practice only if the controls and patients resemble those who will be tested; the predictive values also depend on prevalence, which these data do not give.",
    ));
    s.assumptions.push(independence(
        "Each value must come from a different subject.",
    ));
    // Graph.
    let (xs, ys): (Vec<f64>, Vec<f64>) = r.points.iter().map(|p| (p.fpr, p.sensitivity)).unzip();
    let mut ag = graph_for("roc", &keys);
    ag.views.push(GraphView {
        family: "roc".into(),
        curves: vec![Curve {
            name: format!("{pname} (AUC {})", unit(r.auc, style)),
            dataset: keys.get(1).copied().flatten(),
            x: xs,
            y: ys,
            ..Default::default()
        }],
        ..Default::default()
    });
    s.graph_family = Some("roc".into());
    s.graph = Some(ag);
    Ok(s)
}
