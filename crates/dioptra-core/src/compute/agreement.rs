//! Bland–Altman agreement of two methods: bias and 95% limits of agreement with CIs,
//! and proportional bias.

use dioptra_stats::agreement::{bland_altman as ba, BaKind};

use super::checks;
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec, BlandAltmanMethod};
use crate::graph::{Curve, GraphView, Series};
use crate::results::CheckStatus;

/// Bland–Altman.
pub fn bland_altman(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::BlandAltman(spec) = ctx.spec else {
        return Err("not a Bland–Altman spec".into());
    };
    check_level(spec.ci_level)?;
    let style = ctx.p_style;
    let t = ctx.table;
    let keys = extract::pick(t, &spec.columns, Some(2))?;
    if keys.len() < 2 {
        return Err("Bland–Altman compares two methods; choose two data sets measured on the same subjects (rows).".into());
    }
    let (ka, kb) = (keys[0], keys[1]);
    let (an, bn) = (extract::key_name(t, ka), extract::key_name(t, kb));
    let p = extract::paired(t, ka, kb)?;
    let kind = match spec.method {
        BlandAltmanMethod::Difference => BaKind::Difference,
        BlandAltmanMethod::Ratio => BaKind::Ratio,
        BlandAltmanMethod::PercentDifference => BaKind::PercentDifference,
    };
    let r = ba(&p.x, &p.y, kind, spec.ci_level).map_err(|e| e.to_string())?;
    let what = match spec.method {
        BlandAltmanMethod::Difference => format!("{an} − {bn}"),
        BlandAltmanMethod::Ratio => format!("{an} / {bn}"),
        BlandAltmanMethod::PercentDifference => format!("100 × ({an} − {bn}) / mean"),
    };
    let lvl = pct(spec.ci_level);
    let mut s = sheet("bland_altman", "Bland–Altman");
    s.methods = methods(&format!(
        "Bland–Altman analysis of {what} against the mean of the two methods, {} pairs, bias ± 1.96 SD limits of agreement with {lvl} CIs",
        r.n
    ));
    s.sections.push(section_owned(
        "Bias and limits of agreement",
        vec![
            "Quantity".into(),
            "Value".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
        ],
        vec![
            vec![
                txt(format!("Bias (mean of {what})")),
                num(r.bias),
                num(r.bias_ci.0),
                num(r.bias_ci.1),
            ],
            vec![
                txt("SD of the differences"),
                num(r.sd),
                crate::results::ResultCell::Empty,
                crate::results::ResultCell::Empty,
            ],
            vec![
                txt("Lower limit of agreement"),
                num(r.loa_lower),
                num(r.loa_lower_ci.0),
                num(r.loa_lower_ci.1),
            ],
            vec![
                txt("Upper limit of agreement"),
                num(r.loa_upper),
                num(r.loa_upper_ci.0),
                num(r.loa_upper_ci.1),
            ],
        ],
    ));
    if let Some(pb) = &r.proportional_bias {
        s.sections.push(section_owned(
            "Proportional bias",
            vec![
                "Quantity".into(),
                "Slope".into(),
                "SE".into(),
                format!("Lower {lvl} CI"),
                format!("Upper {lvl} CI"),
                "P (slope ≠ 0)".into(),
            ],
            vec![vec![
                txt("Difference against mean"),
                num(pb.slope),
                num(pb.slope_se),
                num(pb.slope_ci.0),
                num(pb.slope_ci.1),
                pv(pb.p),
            ]],
        ));
    }
    s.summary.push(format!(
        "The bias ({what}) was {}, with {lvl} limits of agreement from {} to {}, n = {} pairs.",
        est_ci(r.bias, r.bias_ci.0, r.bias_ci.1, spec.ci_level),
        est_ci(
            r.loa_lower,
            r.loa_lower_ci.0,
            r.loa_lower_ci.1,
            spec.ci_level
        ),
        est_ci(
            r.loa_upper,
            r.loa_upper_ci.0,
            r.loa_upper_ci.1,
            spec.ci_level
        ),
        r.n
    ));
    // Checks.
    let mut notes = Vec::new();
    if p.dropped > 0 {
        notes.push(format!(
            "{} row{} with only one method measured left out.",
            p.dropped,
            if p.dropped == 1 { "" } else { "s" }
        ));
    }
    s.assumptions
        .push(checks::values_used(&notes, p.dropped > 0));
    s.assumptions
        .push(checks::sample_size(&[("Pairs".into(), r.n)], 30));
    let (c, sec) = checks::normality(
        &[("Differences".into(), r.differences.clone())],
        "the differences",
        "The limits of agreement assume Gaussian differences; try the ratio or percent difference, or report nonparametric limits (2.5th and 97.5th percentiles of the differences).",
    );
    s.assumptions.push(c);
    s.sections.push(sec);
    if let Some(pb) = &r.proportional_bias {
        s.assumptions.push(if pb.p < 0.05 {
            AssumptionCheck::new(
                "Proportional bias",
                CheckStatus::Caution,
                format!("The difference changes with the size of the measurement (slope {}, {}); a single bias and one pair of limits do not describe it.", g(pb.slope), p_phrase(pb.p, style)),
            )
            .or_else("Use the ratio or percent-difference method (option \"method\"), or report the regression of the difference on the mean.")
        } else {
            AssumptionCheck::new(
                "Proportional bias",
                CheckStatus::Pass,
                format!("The difference does not change detectably with the size of the measurement ({}).", p_phrase(pb.p, style)),
            )
        });
    }
    s.assumptions.push(AssumptionCheck::new(
        "Acceptable agreement",
        CheckStatus::NotChecked,
        "Whether limits this wide are acceptable is a clinical or scientific judgement, not a statistical one; decide the acceptable difference before looking.",
    ));
    s.assumptions.push(independence("Each pair must come from a different subject (repeated pairs per subject need a different method)."));
    // Graph.
    let mut ag = graph_for("bland_altman", &[ka, kb]);
    if spec.method != BlandAltmanMethod::Difference {
        let (lo, hi) = r
            .averages
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |a, x| {
                (a.0.min(*x), a.1.max(*x))
            });
        let line = |name: &str, y: f64| Curve {
            name: name.into(),
            dataset: None,
            x: vec![lo, hi],
            y: vec![y, y],
            ..Default::default()
        };
        ag.family = "scatter".into();
        ag.views.push(GraphView {
            family: "scatter".into(),
            replace: true,
            series: vec![Series::xy(
                &what,
                ka.unwrap_or(0),
                r.averages.clone(),
                r.differences.clone(),
            )],
            curves: vec![
                line("Bias", r.bias),
                line("Lower limit", r.loa_lower),
                line("Upper limit", r.loa_upper),
            ],
            x_label: Some(format!("Mean of {an} and {bn}")),
            y_label: Some(what.clone()),
            ..Default::default()
        });
    }
    s.graph_family = Some(ag.family.clone());
    s.graph = Some(ag);
    Ok(s)
}
