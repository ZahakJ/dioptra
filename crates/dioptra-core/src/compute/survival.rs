//! Kaplan–Meier survival curves with Greenwood log-log CIs, median survival with the
//! Brookmeyer–Crowley CI, log-rank (Mantel–Cox) and Gehan–Breslow–Wilcoxon comparisons,
//! the test for trend, and hazard ratios both ways.

use dioptra_stats::survival::{
    hazard_ratio, kaplan_meier, logrank, KmCi, KmOptions, LogRankWeight,
};

use super::checks;
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec, SurvivalCompare};
use crate::graph::{AtRiskTable, Curve, GraphView};
use crate::results::{CheckStatus, ResultCell};

fn med(v: f64) -> String {
    if v.is_infinite() {
        "undefined (survival never fell to 50%)".into()
    } else {
        g(v)
    }
}

/// Survival analysis.
pub fn survival(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Survival(spec) = ctx.spec else {
        return Err("not a survival spec".into());
    };
    check_level(spec.ci_level)?;
    let style = ctx.p_style;
    let t = ctx.table;
    let keys: Vec<usize> = (0..t.datasets.len()).collect();
    let (groups, notes) = extract::survival(t, &keys)?;
    let groups: Vec<_> = groups.into_iter().filter(|g| !g.time.is_empty()).collect();
    if groups.is_empty() {
        return Err(
            "There are no subjects: enter times in X and 1 (event) or 0 (censored) in a data set."
                .into(),
        );
    }
    let lvl = pct(spec.ci_level);
    let kmo = KmOptions {
        ci_level: spec.ci_level,
        ci: KmCi::LogLog,
    };
    let mut s = sheet("survival", "Survival curves");
    let mut medians = Vec::new();
    let mut view = GraphView {
        family: "kaplan_meier".into(),
        ..Default::default()
    };
    let mut kms = Vec::new();
    for gr in &groups {
        let km = kaplan_meier(&gr.time, &gr.event, kmo).map_err(|e| format!("{}: {e}", gr.name))?;
        let rows: Vec<Vec<ResultCell>> = km
            .rows
            .iter()
            .map(|r| {
                vec![
                    num(r.time),
                    int(r.n_at_risk),
                    int(r.events),
                    int(r.censored),
                    num(100.0 * r.survival),
                    num(100.0 * r.se),
                    num(100.0 * r.ci_lower),
                    num(100.0 * r.ci_upper),
                ]
            })
            .collect();
        s.sections.push(section_owned(
            format!("Kaplan–Meier: {}", gr.name),
            vec![
                "Time".into(),
                "At risk".into(),
                "Events".into(),
                "Censored".into(),
                "Percent survival".into(),
                "SE (%)".into(),
                format!("Lower {lvl} CI (%)"),
                format!("Upper {lvl} CI (%)"),
            ],
            rows,
        ));
        medians.push(vec![
            txt(gr.name.clone()),
            int(km.n),
            int(km.events),
            num_or(km.median, "undefined"),
            num_or(km.median_ci.0, "undefined"),
            num_or(km.median_ci.1, "undefined"),
        ]);
        // Curve with band (fractions), at-risk table.
        let (xs, ys): (Vec<f64>, Vec<f64>) = km.curve.iter().copied().unzip();
        let (lo, hi): (Vec<f64>, Vec<f64>) = xs
            .iter()
            .zip(&ys)
            .map(|(x, y)| {
                let (a, b) = km.ci_at(*x);
                (
                    if a.is_finite() { a } else { *y },
                    if b.is_finite() { b } else { *y },
                )
            })
            .unzip();
        let mut censor: Vec<f64> = gr
            .time
            .iter()
            .zip(&gr.event)
            .filter(|(_, e)| !**e)
            .map(|(t, _)| *t)
            .collect();
        censor.sort_by(f64::total_cmp);
        censor.dedup();
        view.curves.push(Curve {
            name: gr.name.clone(),
            dataset: Some(gr.dataset),
            x: xs,
            y: ys,
            lo,
            hi,
            step: true,
            censor,
        });
        kms.push(km);
    }
    let tmax = groups
        .iter()
        .flat_map(|g| g.time.iter().copied())
        .fold(0.0f64, f64::max);
    let ticks = super::regression::grid(0.0, tmax, 5, false);
    for (gr, km) in groups.iter().zip(&kms) {
        view.tables.push(AtRiskTable {
            name: gr.name.clone(),
            dataset: gr.dataset,
            times: ticks.clone(),
            counts: ticks.iter().map(|x| km.at_risk_at(*x)).collect(),
        });
    }
    s.sections.push(section_owned(
        "Median survival",
        vec![
            "Data set".into(),
            "n".into(),
            "Events".into(),
            "Median".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
        ],
        medians,
    ));
    let meds: Vec<String> = groups
        .iter()
        .zip(&kms)
        .map(|(gr, km)| {
            if km.median.is_finite() {
                format!(
                    "{} in {} ({lvl} CI {} to {})",
                    g(km.median),
                    gr.name,
                    g(km.median_ci.0),
                    if km.median_ci.1.is_finite() {
                        g(km.median_ci.1)
                    } else {
                        "undefined".into()
                    }
                )
            } else {
                format!("{} in {}", med(km.median), gr.name)
            }
        })
        .collect();
    s.summary
        .push(format!("Median survival was {}.", meds.join("; ")));
    let mut mline = format!(
        "Kaplan–Meier survival curves with Greenwood log-log {lvl} CIs; median survival with the Brookmeyer–Crowley CI"
    );
    if groups.len() >= 2 {
        let gs: Vec<(&[f64], &[bool])> = groups
            .iter()
            .map(|g| (g.time.as_slice(), g.event.as_slice()))
            .collect();
        let lr = logrank(&gs, LogRankWeight::LogRank, None).map_err(|e| e.to_string())?;
        let gb = logrank(&gs, LogRankWeight::Wilcoxon, None).map_err(|e| e.to_string())?;
        let mut rows = vec![
            vec![
                txt("Log-rank (Mantel–Cox)"),
                num(lr.chi2),
                num(lr.df),
                pv(lr.p),
            ],
            vec![
                txt("Gehan–Breslow–Wilcoxon"),
                num(gb.chi2),
                num(gb.df),
                pv(gb.p),
            ],
        ];
        if let Some((c2, p)) = lr.trend {
            rows.push(vec![
                txt("Log-rank test for trend"),
                num(c2),
                num(1.0),
                pv(p),
            ]);
        }
        s.sections.push(section(
            "Curve comparison",
            &["Test", "χ²", "df", "P"],
            rows,
        ));
        s.sections.push(section(
            "Observed and expected events",
            &["Data set", "Observed", "Expected", "O/E"],
            groups
                .iter()
                .enumerate()
                .map(|(i, gr)| {
                    vec![
                        txt(gr.name.clone()),
                        num(lr.observed[i]),
                        num(lr.expected[i]),
                        num(lr.o_over_e[i]),
                    ]
                })
                .collect(),
        ));
        let (lead, lname) = match spec.compare {
            SurvivalCompare::LogRank => (&lr, "Log-rank (Mantel–Cox)"),
            SurvivalCompare::GehanBreslowWilcoxon => (&gb, "Gehan–Breslow–Wilcoxon"),
        };
        let mut sent = format!(
            "{lname} χ²({}) = {}, {}",
            df(lead.df),
            st(lead.chi2),
            p_phrase(lead.p, style)
        );
        mline.push_str(&format!("; curves compared by the {lname} test"));
        if groups.len() == 2 {
            let (a, b) = (&groups[0], &groups[1]);
            let h1 = hazard_ratio((&a.time, &a.event), (&b.time, &b.event), spec.ci_level)
                .map_err(|e| e.to_string())?;
            let h2 = hazard_ratio((&b.time, &b.event), (&a.time, &a.event), spec.ci_level)
                .map_err(|e| e.to_string())?;
            s.sections.push(section_owned(
                "Hazard ratio",
                vec![
                    "Ratio".into(),
                    "Method".into(),
                    "Hazard ratio".into(),
                    format!("Lower {lvl} CI"),
                    format!("Upper {lvl} CI"),
                ],
                vec![
                    vec![
                        txt(format!("{} / {}", a.name, b.name)),
                        txt("Mantel–Haenszel"),
                        num(h1.hr_mantel_haenszel),
                        num(h1.hr_mantel_haenszel_ci.0),
                        num(h1.hr_mantel_haenszel_ci.1),
                    ],
                    vec![
                        txt(format!("{} / {}", a.name, b.name)),
                        txt("Log-rank (O/E)"),
                        num(h1.hr_logrank),
                        num(h1.hr_logrank_ci.0),
                        num(h1.hr_logrank_ci.1),
                    ],
                    vec![
                        txt(format!("{} / {}", b.name, a.name)),
                        txt("Mantel–Haenszel"),
                        num(h2.hr_mantel_haenszel),
                        num(h2.hr_mantel_haenszel_ci.0),
                        num(h2.hr_mantel_haenszel_ci.1),
                    ],
                    vec![
                        txt(format!("{} / {}", b.name, a.name)),
                        txt("Log-rank (O/E)"),
                        num(h2.hr_logrank),
                        num(h2.hr_logrank_ci.0),
                        num(h2.hr_logrank_ci.1),
                    ],
                ],
            ));
            sent.push_str(&format!(
                "; hazard ratio ({} / {}, Mantel–Haenszel) {}",
                a.name,
                b.name,
                est_ci(
                    h1.hr_mantel_haenszel,
                    h1.hr_mantel_haenszel_ci.0,
                    h1.hr_mantel_haenszel_ci.1,
                    spec.ci_level
                )
            ));
            mline.push_str("; hazard ratios by the Mantel–Haenszel and log-rank methods");
        }
        sent.push('.');
        s.summary.push(sent);
    }
    s.methods = methods(&mline);
    // Checks.
    let events: Vec<(String, usize)> = groups
        .iter()
        .map(|g| (g.name.clone(), g.event.iter().filter(|e| **e).count()))
        .collect();
    s.assumptions
        .push(checks::values_used(&notes, !notes.is_empty()));
    s.assumptions.push(AssumptionCheck::new(
        "Event codes",
        CheckStatus::Pass,
        format!(
            "Every used cell is 1 (event) or 0 (censored): {}.",
            groups
                .iter()
                .zip(&events)
                .map(|(gr, (_, e))| format!("{} {} subjects, {e} events", gr.name, gr.time.len()))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    ));
    let few: Vec<String> = events
        .iter()
        .filter(|(_, e)| *e < 5)
        .map(|(n, e)| format!("{n} ({e})"))
        .collect();
    s.assumptions.push(if few.is_empty() {
        AssumptionCheck::new("Number of events", CheckStatus::Pass, "Every group has at least 5 events.")
    } else {
        AssumptionCheck::new(
            "Number of events",
            CheckStatus::Caution,
            format!("Few events in {}: survival estimates and CIs are imprecise, and the log-rank test is approximate.", few.join(", ")),
        )
    });
    if groups.len() >= 2 {
        s.assumptions.push(AssumptionCheck::new(
            "Proportional hazards",
            CheckStatus::NotChecked,
            "The log-rank test and the hazard ratio assume the ratio of hazards is constant over time; this is not tested. Crossing survival curves are a sign it fails.",
        ).or_else("If the curves cross, the Gehan–Breslow–Wilcoxon test (which weights early differences) or a comparison of medians may describe the data better than a single hazard ratio."));
    }
    s.assumptions.push(AssumptionCheck::new(
        "Uninformative censoring",
        CheckStatus::NotChecked,
        "Subjects censored (lost or still alive) must have the same outlook as those followed; the data cannot show this.",
    ));
    s.assumptions.push(independence(
        "Each subject must appear once and be independent of the others.",
    ));
    // Graph: the analysis's own curves (its CI level and transform, its censoring
    // times, its at-risk table), so a graph of the analysis shows what the sheet reports.
    let mut ag = graph_for(
        "kaplan_meier",
        &groups.iter().map(|g| Some(g.dataset)).collect::<Vec<_>>(),
    );
    ag.views.push(view);
    s.graph_family = Some("kaplan_meier".into());
    s.graph = Some(ag);
    Ok(s)
}
