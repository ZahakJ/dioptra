//! Contingency tables: chi-square (with Yates on request), Fisher's exact test (2 × 2 and
//! r × c), relative risk, odds ratio and risk difference with CIs, Cramér's V; and the
//! chi-square goodness-of-fit test for a parts-of-whole table.

use dioptra_stats::contingency::{
    chi_square_gof, chi_square_independence, fisher_exact_2x2, fisher_exact_rxc, risk_measures,
};

use super::extract::{self, Counts};
use super::*;
use crate::analysis::{Alternative, AnalysisContext, AnalysisSpec};
use crate::api::TableKind;
use crate::results::{CheckStatus, ResultCell};

const FISHER_ALT: &str = "Use Fisher's exact test (Analyze → Fisher's exact test), which needs no large-sample approximation.";

fn observed_section(c: &Counts) -> Section {
    let mut cols = vec!["Row".to_string()];
    cols.extend(c.col_names.iter().cloned());
    cols.push("Total".into());
    let mut rows: Vec<Vec<ResultCell>> = c
        .counts
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row = vec![txt(c.row_names[i].clone())];
            row.extend(r.iter().map(|v| int(*v as usize)));
            row.push(int(r.iter().sum::<u64>() as usize));
            row
        })
        .collect();
    let mut tot = vec![txt("Total")];
    let mut n = 0;
    for j in 0..c.col_names.len() {
        let s: u64 = c.counts.iter().map(|r| r[j]).sum();
        n += s;
        tot.push(int(s as usize));
    }
    tot.push(int(n as usize));
    rows.push(tot);
    section_owned("Observed counts", cols, rows)
}

fn matrix_section(title: &str, c: &Counts, m: &[Vec<f64>]) -> Section {
    let mut cols = vec!["Row".to_string()];
    cols.extend(c.col_names.iter().cloned());
    section_owned(
        title,
        cols,
        m.iter()
            .enumerate()
            .map(|(i, r)| {
                let mut row = vec![txt(c.row_names[i].clone())];
                row.extend(r.iter().map(|v| num(*v)));
                row
            })
            .collect(),
    )
}

fn as_2x2(c: &Counts) -> Option<[[u64; 2]; 2]> {
    (c.counts.len() == 2 && c.counts[0].len() == 2).then(|| {
        [
            [c.counts[0][0], c.counts[0][1]],
            [c.counts[1][0], c.counts[1][1]],
        ]
    })
}

/// The risk section and sentence for a 2 × 2 table (rows = groups, columns = outcome).
fn risk(c: &Counts, t: [[u64; 2]; 2], level: f64, s: &mut ResultsSheet) {
    let Ok(r) = risk_measures(t, level) else {
        return;
    };
    let lvl = pct(level);
    let (g1, g2, ev) = (&c.row_names[0], &c.row_names[1], &c.col_names[0]);
    let ci = |o: Option<(f64, f64)>| {
        (
            o.map_or(ResultCell::Empty, |x| num(x.0)),
            o.map_or(ResultCell::Empty, |x| num(x.1)),
        )
    };
    let (rl, rh) = ci(r.relative_risk_katz_ci);
    let (ol, oh) = ci(r.odds_ratio_woolf_ci);
    s.sections.push(section_owned(
        "Effect size",
        vec![
            "Measure".into(),
            "Value".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
            "Method".into(),
        ],
        vec![
            vec![
                txt(format!("Relative risk ({g1} / {g2}, outcome \"{ev}\")")),
                num(r.relative_risk),
                rl,
                rh,
                txt("Katz log interval"),
            ],
            vec![
                txt(format!("Odds ratio ({g1} / {g2})")),
                num(r.odds_ratio),
                ol,
                oh,
                txt("Woolf logit interval"),
            ],
            vec![
                txt(format!("Risk difference ({g1} − {g2})")),
                num(r.risk_difference),
                num(r.risk_difference_wald_ci.0),
                num(r.risk_difference_wald_ci.1),
                txt("Wald interval"),
            ],
        ],
    ));
    let rr = match r.relative_risk_katz_ci {
        Some((a, b)) => est_ci(r.relative_risk, a, b, level),
        None => g(r.relative_risk),
    };
    s.summary.insert(
        0,
        format!(
            "The risk of \"{ev}\" was {} in {g1} and {} in {g2}: relative risk {rr}, risk difference {}.",
            g(t[0][0] as f64 / (t[0][0] + t[0][1]) as f64),
            g(t[1][0] as f64 / (t[1][0] + t[1][1]) as f64),
            est_ci(r.risk_difference, r.risk_difference_wald_ci.0, r.risk_difference_wald_ci.1, level)
        ),
    );
}

/// Chi-square test of a contingency table (or goodness of fit of a parts table).
pub fn chi_square(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::ChiSquare(spec) = ctx.spec else {
        return Err("not a chi-square spec".into());
    };
    let style = ctx.p_style;
    let c = extract::counts(ctx.table)?;
    if ctx.table.kind == TableKind::Parts || c.col_names.len() == 1 {
        return goodness_of_fit(ctx, &c);
    }
    if c.counts.len() < 2 {
        return Err("A chi-square test of independence needs at least two rows.".into());
    }
    let r = chi_square_independence(&c.counts).map_err(|e| e.to_string())?;
    let is2 = as_2x2(&c);
    let use_yates = spec.yates && r.yates.is_some();
    let lead = if use_yates {
        r.yates.clone().expect("yates")
    } else {
        r.pearson.clone()
    };
    let mut s = sheet("chi_square", "Chi-square test");
    s.methods = methods(&format!(
        "Chi-square test of independence ({}), {} × {} table, N = {}",
        if use_yates {
            "Pearson with Yates' continuity correction"
        } else {
            "Pearson, no continuity correction"
        },
        c.counts.len(),
        c.col_names.len(),
        r.n
    ));
    s.sections.push(observed_section(&c));
    s.sections
        .push(matrix_section("Expected counts", &c, &r.expected));
    let mut rows = vec![vec![
        txt("Pearson χ²"),
        num(r.pearson.statistic),
        num(r.pearson.df),
        pv(r.pearson.p),
    ]];
    if let Some(y) = &r.yates {
        rows.push(vec![
            txt("Yates-corrected χ²"),
            num(y.statistic),
            num(y.df),
            pv(y.p),
        ]);
    }
    rows.push(vec![
        txt("Likelihood-ratio G"),
        num(r.g_test.statistic),
        num(r.g_test.df),
        pv(r.g_test.p),
    ]);
    let mut fisher_p = None;
    if let Some(t) = is2 {
        if let Ok(f) = fisher_exact_2x2(t, 0.95) {
            rows.push(vec![
                txt("Fisher's exact test"),
                ResultCell::Empty,
                ResultCell::Empty,
                pv(f.p_two_sided),
            ]);
            fisher_p = Some(f.p_two_sided);
        }
    }
    s.sections
        .push(section("Test", &["Test", "Statistic", "df", "P"], rows));
    let mut es = vec![vec![txt("Cramér's V"), num(r.cramers_v)]];
    if let Some(phi) = r.phi {
        es.push(vec![txt("φ (signed)"), num(phi)]);
    }
    s.sections
        .push(section("Association", &["Measure", "Value"], es));
    s.sections.push(matrix_section(
        "Adjusted residuals",
        &c,
        &r.adjusted_residuals,
    ));
    s.summary.push(format!(
        "χ²({}, N = {}) = {}, {}, Cramér's V = {}{}.",
        df(lead.df),
        r.n,
        st(lead.statistic),
        p_phrase(lead.p, style),
        unit(r.cramers_v, style),
        if use_yates { " (Yates-corrected)" } else { "" }
    ));
    if let Some(t) = is2 {
        risk(&c, t, 0.95, &mut s);
    }
    // Checks.
    let cells = c.counts.len() * c.col_names.len();
    s.assumptions.push(if r.approximation_warning {
        AssumptionCheck::new(
            "Expected counts",
            if r.min_expected < 1.0 { CheckStatus::Fail } else { CheckStatus::Caution },
            format!(
                "{} of {cells} expected counts are below 5 (smallest {}); the chi-square approximation is doubtful (Cochran's rule).{}",
                r.cells_expected_below_5,
                g(r.min_expected),
                fisher_p.map_or(String::new(), |p| format!(" Fisher's exact test gives {}.", p_phrase(p, style)))
            ),
        )
        .or_else(FISHER_ALT)
    } else {
        AssumptionCheck::new(
            "Expected counts",
            CheckStatus::Pass,
            format!(
                "Every expected count is at least 1 and at most 20% are below 5 (smallest {}), so the chi-square approximation holds.",
                g(r.min_expected)
            ),
        )
    });
    s.assumptions.push(AssumptionCheck::new(
        "Counts of subjects",
        CheckStatus::Pass,
        "Every cell is a non-negative whole number.",
    ));
    s.assumptions.push(independence(
        "Each subject must be counted once, in one cell; repeated observations of the same subject are not independent.",
    ));
    let all: Vec<Option<usize>> = (0..ctx.table.datasets.len()).map(Some).collect();
    s.graph_family = Some("column".into());
    s.graph = Some(graph_for("column", &all));
    Ok(s)
}

fn goodness_of_fit(ctx: &AnalysisContext<'_>, c: &Counts) -> Result<ResultsSheet, String> {
    let style = ctx.p_style;
    let obs: Vec<u64> = c.counts.iter().map(|r| r[0]).collect();
    if obs.len() < 2 {
        return Err("A goodness-of-fit test needs at least two categories.".into());
    }
    let r = chi_square_gof(&obs, None, 0).map_err(|e| e.to_string())?;
    let n: u64 = obs.iter().sum();
    let mut s = sheet("chi_square", "Chi-square goodness of fit");
    s.methods = methods(&format!(
        "Chi-square goodness-of-fit test against equal proportions, {} categories, N = {n}",
        obs.len()
    ));
    s.sections.push(section(
        "Observed and expected",
        &["Category", "Observed", "Expected", "Observed − expected"],
        obs.iter()
            .enumerate()
            .map(|(i, o)| {
                vec![
                    txt(c.row_names[i].clone()),
                    int(*o as usize),
                    num(r.expected[i]),
                    num(*o as f64 - r.expected[i]),
                ]
            })
            .collect(),
    ));
    s.sections.push(kv(
        "Test",
        vec![
            ("χ²".into(), num(r.chi2)),
            ("df".into(), num(r.df)),
            ("P".into(), pv(r.p)),
        ],
    ));
    s.summary.push(format!(
        "Chi-square goodness of fit against equal proportions: χ²({}, N = {n}) = {}, {}.",
        df(r.df),
        st(r.chi2),
        p_phrase(r.p, style)
    ));
    s.assumptions.push(if r.cells_expected_below_5 > 0 {
        AssumptionCheck::new(
            "Expected counts",
            CheckStatus::Caution,
            format!("{} categories have expected counts below 5; the chi-square approximation is rough.", r.cells_expected_below_5),
        )
        .or_else("Merge sparse categories or collect more data.")
    } else {
        AssumptionCheck::new("Expected counts", CheckStatus::Pass, "Every expected count is at least 5.")
    });
    s.assumptions
        .push(independence("Each subject must be counted once."));
    s.graph_family = Some("column".into());
    s.graph = Some(graph_for("column", &[Some(0)]));
    Ok(s)
}

/// Fisher's exact test.
pub fn fisher_exact(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::FisherExact(spec) = ctx.spec else {
        return Err("not a Fisher's exact test spec".into());
    };
    let style = ctx.p_style;
    let c = extract::counts(ctx.table)?;
    if c.counts.len() < 2 || c.col_names.len() < 2 {
        return Err("Fisher's exact test needs at least a 2 × 2 table.".into());
    }
    let mut s = sheet("fisher_exact", "Fisher's exact test");
    s.sections.push(observed_section(&c));
    let all: Vec<Option<usize>> = (0..ctx.table.datasets.len()).map(Some).collect();
    s.graph_family = Some("column".into());
    s.graph = Some(graph_for("column", &all));
    s.assumptions.push(AssumptionCheck::new(
        "Counts of subjects",
        CheckStatus::Pass,
        "Every cell is a non-negative whole number.",
    ));
    if let Some(t) = as_2x2(&c) {
        let f = fisher_exact_2x2(t, 0.95).map_err(|e| e.to_string())?;
        let (g1, g2) = (&c.row_names[0], &c.row_names[1]);
        let (p, dir) = match spec.alternative {
            Alternative::TwoSided => (f.p_two_sided, "two-sided".to_string()),
            Alternative::Greater => (f.p_greater, "one-sided (H1: odds ratio > 1)".to_string()),
            Alternative::Less => (f.p_less, "one-sided (H1: odds ratio < 1)".to_string()),
        };
        s.methods = methods(&format!(
            "Fisher's exact test, {dir} (two-sided P sums tables no more likely than the observed one), conditional maximum-likelihood odds ratio with exact 95% CI"
        ));
        let mut rows = vec![("P value (two-sided)".to_string(), pv(f.p_two_sided))];
        match spec.alternative {
            Alternative::TwoSided => {}
            Alternative::Greater => {
                rows.push(("P value (one-sided, OR > 1)".into(), pv(f.p_greater)))
            }
            Alternative::Less => rows.push(("P value (one-sided, OR < 1)".into(), pv(f.p_less))),
        }
        rows.push(("Odds ratio (sample)".into(), num(f.odds_ratio_sample)));
        rows.push((
            "Odds ratio (conditional MLE)".into(),
            num(f.odds_ratio_conditional_mle),
        ));
        rows.push((
            "Lower 95% CI (exact)".into(),
            num(f.odds_ratio_conditional_ci.0),
        ));
        rows.push((
            "Upper 95% CI (exact)".into(),
            num_or(f.odds_ratio_conditional_ci.1, "Infinity"),
        ));
        s.sections.push(kv("Test", rows));
        s.summary.push(format!(
            "The odds ratio ({g1} / {g2}, conditional MLE) was {}, Fisher's exact test {}{}.",
            est_ci(
                f.odds_ratio_conditional_mle,
                f.odds_ratio_conditional_ci.0,
                f.odds_ratio_conditional_ci.1,
                0.95
            ),
            if spec.alternative == Alternative::TwoSided {
                String::new()
            } else {
                "one-sided ".into()
            },
            p_phrase(p, style)
        ));
        risk(&c, t, 0.95, &mut s);
        let mut ag = graph_for("column", &all);
        ag.brackets.clear();
        s.graph = Some(ag);
    } else {
        s.methods = methods(&format!(
            "Fisher–Freeman–Halton exact test, {} × {} table (network algorithm), two-sided",
            c.counts.len(),
            c.col_names.len()
        ));
        match fisher_exact_rxc(&c.counts) {
            Ok(f) => {
                let mut rows = vec![
                    ("P value (two-sided, exact)".to_string(), pv(f.p_two_sided)),
                    (
                        "Probability of the observed table".into(),
                        num(f.p_observed_table),
                    ),
                ];
                if let Some(n) = f.n_tables {
                    rows.push(("Tables with these margins".into(), num(n)));
                }
                s.sections.push(kv("Test", rows));
                s.summary.push(format!(
                    "Fisher–Freeman–Halton exact test of independence: {}.",
                    p_phrase(f.p_two_sided, style)
                ));
                if spec.alternative != Alternative::TwoSided {
                    s.checklist.push("A one-sided alternative applies only to 2 × 2 tables; the r × c test is two-sided.".into());
                }
            }
            Err(e) => {
                s.message = Some(format!(
                    "The exact r × c test did not finish within its work limit ({e}); the table is too large for exact enumeration. Use the chi-square test instead if its expected counts allow."
                ));
            }
        }
        if let Ok(r) = chi_square_independence(&c.counts) {
            s.sections.push(section(
                "Association",
                &["Measure", "Value"],
                vec![vec![txt("Cramér's V"), num(r.cramers_v)]],
            ));
        }
    }
    s.assumptions.push(AssumptionCheck::new(
        "Fixed margins",
        CheckStatus::NotChecked,
        "Fisher's test conditions on the row and column totals; it is exact and conservative when the totals were not fixed by design, which is the usual case.",
    ));
    s.assumptions.push(independence(
        "Each subject must be counted once, in one cell.",
    ));
    Ok(s)
}
