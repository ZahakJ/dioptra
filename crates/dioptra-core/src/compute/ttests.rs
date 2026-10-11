//! t tests: one-sample, unpaired (Welch by default, Student on request), paired and ratio
//! paired. Unpaired and one-sample tests also run from mean, SD and N.

use dioptra_stats::correlation::pearson;
use dioptra_stats::ttest::{self, TTestOptions, TTestResult};

use super::checks::{self, ALPHA};
use super::extract::{self, Group};
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec, TTestSpec, TTestVariant};
use crate::results::{CheckStatus, ResultCell};

const MW_ALT: &str =
    "Consider the Mann–Whitney test (Analyze → Mann–Whitney test), or report both.";
const WILCOXON_ALT: &str =
    "Consider the Wilcoxon signed-rank test (Analyze → Wilcoxon signed-rank test), or report both.";

/// Compute a t test.
pub fn t_test(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::TTest(spec) = ctx.spec else {
        return Err("not a t test spec".into());
    };
    check_level(spec.ci_level)?;
    match spec.variant {
        TTestVariant::OneSample => one_sample(ctx, spec),
        TTestVariant::Unpaired | TTestVariant::Welch => unpaired(ctx, spec),
        TTestVariant::Paired | TTestVariant::RatioPaired => paired(ctx, spec),
    }
}

fn opts(spec: &TTestSpec) -> TTestOptions {
    TTestOptions {
        alternative: alt(spec.alternative),
        ci_level: spec.ci_level,
    }
}

fn test_rows(
    r: &TTestResult,
    spec: &TTestSpec,
    h1_greater: &str,
    h1_less: &str,
) -> Vec<(String, ResultCell)> {
    let mut rows = vec![
        ("t".to_string(), num(r.t)),
        ("df".to_string(), num(r.df)),
        ("P value (two-sided)".to_string(), pv(r.p_two_sided)),
    ];
    match spec.alternative {
        crate::analysis::Alternative::TwoSided => {}
        crate::analysis::Alternative::Greater => rows.push((
            format!("P value (one-sided, H1: {h1_greater})"),
            pv(r.p_greater),
        )),
        crate::analysis::Alternative::Less => {
            rows.push((format!("P value (one-sided, H1: {h1_less})"), pv(r.p_less)))
        }
    }
    rows.push(("Method".into(), txt(r.method.clone())));
    rows
}

fn effect_section(r: &TTestResult, n1: usize, n2: usize, level: f64, paired: bool) -> Section {
    let (dl, dh) = d_ci(r.cohens_d, n1, n2, level);
    let j = if r.cohens_d != 0.0 {
        r.hedges_g / r.cohens_d
    } else {
        1.0
    };
    let lvl = pct(level);
    let mut rows = vec![
        vec![
            txt(if paired || n2 == 0 {
                "Cohen's d (d_z)"
            } else {
                "Cohen's d"
            }),
            num(r.cohens_d),
            num(dl),
            num(dh),
        ],
        vec![txt("Hedges' g"), num(r.hedges_g), num(dl * j), num(dh * j)],
    ];
    if let Some(av) = r.cohens_d_av {
        rows.push(vec![
            txt("Cohen's d (average SD)"),
            num(av),
            ResultCell::Empty,
            ResultCell::Empty,
        ]);
    }
    rows.push(vec![
        txt("R² (η²)"),
        num(r.r_squared),
        ResultCell::Empty,
        ResultCell::Empty,
    ]);
    section_owned(
        "Effect size",
        vec![
            "Measure".into(),
            "Value".into(),
            format!("Lower {lvl} CI (approx.)"),
            format!("Upper {lvl} CI (approx.)"),
        ],
        rows,
    )
}

fn diff_section(label: &str, r: &TTestResult) -> Section {
    let lvl = pct(r.ci_level);
    section_owned(
        "Difference",
        vec![
            "Comparison".into(),
            "Estimate".into(),
            "SE".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
        ],
        vec![vec![
            txt(label),
            num(r.estimate),
            num(r.se),
            num(r.ci.0),
            num(r.ci.1),
        ]],
    )
}

fn notes_of(gs: &[&Group]) -> Vec<String> {
    gs.iter().flat_map(|g| g.notes.iter().cloned()).collect()
}

fn one_sample(ctx: &AnalysisContext<'_>, spec: &TTestSpec) -> Result<ResultsSheet, String> {
    let style = ctx.p_style;
    let gs = extract::groups(ctx.table, &spec.columns, Some(1))?;
    let g0 = gs.first().ok_or("There is no data set to test.")?;
    let mut notes = notes_of(&[g0]);
    if gs.len() > 1 {
        notes.push(format!(
            "A one-sample test uses one data set; only {} was tested.",
            g0.name
        ));
    }
    let x = g0.effective();
    if x.len() < 2 {
        return Err(format!(
            "{} needs at least two values for a t test.",
            g0.name
        ));
    }
    let r = ttest::one_sample(&x, spec.mu, &opts(spec)).map_err(|e| e.to_string())?;
    let mu = g(spec.mu);
    let mut s = sheet("t_test", "One-sample t test");
    s.methods = methods(&format!(
        "One-sample t test against a hypothetical mean of {mu}, {}, {} CI{}",
        sidedness(
            spec.alternative,
            &format!("mean > {mu}"),
            &format!("mean < {mu}")
        ),
        pct(spec.ci_level),
        if g0.summary.is_some() {
            ", computed from the entered mean, SD and N"
        } else {
            ""
        }
    ));
    s.sections
        .push(section("Descriptive", &DESC_COLS, vec![desc_row(g0)]));
    let mut rows = vec![("Hypothetical mean".to_string(), num(spec.mu))];
    rows.extend(test_rows(
        &r,
        spec,
        &format!("mean > {mu}"),
        &format!("mean < {mu}"),
    ));
    s.sections.push(kv("Test", rows));
    s.sections.push(diff_section(&format!("Mean − {mu}"), &r));
    s.sections
        .push(effect_section(&r, r.n1, 0, spec.ci_level, false));
    s.assumptions
        .push(checks::values_used(&notes, gs.len() > 1));
    s.assumptions
        .push(checks::sample_size(&[(g0.name.clone(), g0.n())], 5));
    let one_alt = "Consider the Wilcoxon signed-rank test against a hypothetical median (Analyze → Wilcoxon signed-rank test), or report both.";
    if g0.summary.is_none() {
        let sets = vec![(g0.name.clone(), g0.values.clone())];
        let (c, sec) = checks::normality(&sets, "the values", one_alt);
        s.assumptions.push(c);
        s.sections.push(sec);
        s.assumptions.push(checks::outlier_hint(&sets));
    } else {
        s.assumptions.push(AssumptionCheck::new(
            "Gaussian distribution of the values",
            CheckStatus::NotChecked,
            "The data were entered as mean, SD and N, so their distribution cannot be checked.",
        ));
    }
    s.assumptions.push(independence(
        "Each value must be an independent measurement.",
    ));
    s.summary.push(format!(
        "The mean of {} was {}, which differs from {mu} by {}, t({}) = {}, {}, Hedges' g = {}.",
        g0.name,
        g(r.estimate + spec.mu),
        est_ci(r.estimate, r.ci.0, r.ci.1, spec.ci_level),
        df(r.df),
        st(r.t),
        p_dir(r.p, spec.alternative, style),
        st(r.hedges_g)
    ));
    let keys = [g0.dataset];
    s.graph_family = Some("column".into());
    s.graph = Some(graph_for("column", &keys));
    Ok(s)
}

fn unpaired(ctx: &AnalysisContext<'_>, spec: &TTestSpec) -> Result<ResultsSheet, String> {
    let style = ctx.p_style;
    let welch = spec.variant == TTestVariant::Welch;
    let gs = extract::groups(ctx.table, &spec.columns, Some(2))?;
    if gs.len() < 2 {
        return Err("An unpaired t test compares two data sets; choose two.".into());
    }
    let (a, b) = (&gs[0], &gs[1]);
    let mut notes = notes_of(&[a, b]);
    if gs.len() > 2 {
        notes.push(format!(
            "A t test compares two data sets; only {} and {} were used. For more groups use one-way ANOVA.",
            a.name, b.name
        ));
    }
    let (x, y) = (a.effective(), b.effective());
    if x.len() < 2 || y.len() < 2 {
        return Err("Each data set needs at least two values for a t test.".into());
    }
    let r = if welch {
        ttest::welch(&x, &y, &opts(spec))
    } else {
        ttest::unpaired(&x, &y, &opts(spec))
    }
    .map_err(|e| e.to_string())?;
    let (an, bn) = (a.name.as_str(), b.name.as_str());
    let gt = format!("{an} > {bn}");
    let lt = format!("{an} < {bn}");
    let summary_data = a.summary.is_some() || b.summary.is_some();
    let mut s = sheet(
        "t_test",
        if welch {
            "Welch's t test"
        } else {
            "Unpaired t test"
        },
    );
    s.methods = methods(&format!(
        "{}, {}, {} CI{}",
        if welch {
            "Welch's unpaired t test"
        } else {
            "Unpaired t test (Student, pooled variance)"
        },
        sidedness(spec.alternative, &gt, &lt),
        pct(spec.ci_level),
        if summary_data {
            ", computed from the entered mean, SD and N"
        } else {
            ""
        }
    ));
    s.sections.push(section(
        "Descriptive",
        &DESC_COLS,
        vec![desc_row(a), desc_row(b)],
    ));
    s.sections.push(kv("Test", test_rows(&r, spec, &gt, &lt)));
    s.sections.push(diff_section(&format!("{an} − {bn}"), &r));
    s.sections
        .push(effect_section(&r, r.n1, r.n2, spec.ci_level, false));
    s.assumptions
        .push(checks::values_used(&notes, gs.len() > 2));
    s.assumptions.push(checks::sample_size(
        &[(a.name.clone(), a.n()), (b.name.clone(), b.n())],
        5,
    ));
    let vsets = [(a.name.clone(), x.clone()), (b.name.clone(), y.clone())];
    if !summary_data {
        let sets = vec![
            (a.name.clone(), a.values.clone()),
            (b.name.clone(), b.values.clone()),
        ];
        let (c, sec) = checks::normality(&sets, "each group", MW_ALT);
        s.assumptions.push(c);
        s.sections.push(sec);
        if let Some((c, sec)) = checks::equal_variances(
            &sets,
            welch,
            "Use Welch's t test (variant \"welch\"), which does not assume equal variances.",
        ) {
            s.assumptions.push(c);
            s.sections.push(sec);
        }
        s.assumptions.push(checks::outlier_hint(&sets));
    } else {
        s.assumptions.push(AssumptionCheck::new(
            "Gaussian distribution of each group",
            CheckStatus::NotChecked,
            "Summary data (mean, SD and N) cannot show the distribution; the test assumes it is roughly Gaussian.",
        ));
        if !welch {
            let (sa, sb) = (super::mean_sd(&vsets[0].1).1, super::mean_sd(&vsets[1].1).1);
            let ratio = sa.max(sb) / sa.min(sb);
            s.assumptions.push(
                AssumptionCheck::new(
                    "Equal variances",
                    if ratio > 2.0 { CheckStatus::Caution } else { CheckStatus::Pass },
                    format!(
                        "From the entered SDs: the larger SD is {} times the smaller. Student's test pools the variances; a ratio above 2 makes that doubtful.",
                        g(ratio)
                    ),
                )
                .or_else("Use Welch's t test (variant \"welch\"), which does not assume equal variances."),
            );
        }
    }
    s.assumptions.push(independence(
        "Every value must be independent of the others, and the two groups must be unrelated (unpaired).",
    ));
    s.summary.push(format!(
        "The mean difference ({an} − {bn}) was {}, t({}) = {}, {}, Hedges' g = {}.",
        est_ci(r.estimate, r.ci.0, r.ci.1, spec.ci_level),
        df(r.df),
        st(r.t),
        p_dir(r.p, spec.alternative, style),
        st(r.hedges_g)
    ));
    let (ma, sa) = super::mean_sd(&x);
    let (mb, sb) = super::mean_sd(&y);
    s.summary.push(format!(
        "{an}: M = {}, SD = {}, n = {}; {bn}: M = {}, SD = {}, n = {}.",
        g(ma),
        g(sa),
        a.n(),
        g(mb),
        g(sb),
        b.n()
    ));
    let keys = [a.dataset, b.dataset];
    let mut ag = graph_for("column", &keys);
    add_bracket(&mut ag, a.dataset, b.dataset, r.p_two_sided);
    s.graph_family = Some("column".into());
    s.graph = Some(ag);
    Ok(s)
}

fn paired(ctx: &AnalysisContext<'_>, spec: &TTestSpec) -> Result<ResultsSheet, String> {
    let style = ctx.p_style;
    let t = ctx.table;
    let ratio = spec.variant == TTestVariant::RatioPaired;
    let keys = extract::pick(t, &spec.columns, Some(2))?;
    if keys.len() < 2 {
        return Err("A paired t test needs two data sets whose rows are pairs.".into());
    }
    let (ka, kb) = (keys[0], keys[1]);
    let (an, bn) = (extract::key_name(t, ka), extract::key_name(t, kb));
    let p = extract::paired(t, ka, kb)?;
    if p.x.len() < 2 {
        return Err(format!(
            "Only {} complete pair{}; a paired t test needs at least two.",
            p.x.len(),
            if p.x.len() == 1 { "" } else { "s" }
        ));
    }
    if ratio {
        if let Some(i) = (0..p.x.len()).find(|&i| !(p.x[i] > 0.0 && p.y[i] > 0.0)) {
            return Err(format!(
                "The ratio paired t test works on logarithms, so every value must be positive; row {} has {} and {}.",
                p.rows[i] + 1,
                g(p.x[i]),
                g(p.y[i])
            ));
        }
    }
    let mut notes = Vec::new();
    for k in [ka, kb] {
        if let Ok(gr) = extract::group(t, k) {
            notes.extend(gr.notes.iter().filter(|n| !n.contains("summary")).cloned());
        }
    }
    if p.dropped > 0 {
        notes.push(format!(
            "{} row{} with only one value of the pair {} left out.",
            p.dropped,
            if p.dropped == 1 { "" } else { "s" },
            if p.dropped == 1 { "was" } else { "were" }
        ));
    }
    if keys.len() > 2 {
        notes.push(format!(
            "A paired test compares two data sets; only {an} and {bn} were used."
        ));
    }
    let d: Vec<f64> = if ratio {
        p.x.iter().zip(&p.y).map(|(a, b)| (a / b).log10()).collect()
    } else {
        p.x.iter().zip(&p.y).map(|(a, b)| a - b).collect()
    };
    let o = opts(spec);
    let (r, ratio_res) = if ratio {
        let rr = ttest::ratio_paired(&p.x, &p.y, &o).map_err(|e| e.to_string())?;
        (rr.log_test.clone(), Some(rr))
    } else {
        (
            ttest::paired(&p.x, &p.y, &o).map_err(|e| e.to_string())?,
            None,
        )
    };
    let gt = format!("{an} > {bn}");
    let lt = format!("{an} < {bn}");
    let mut s = sheet(
        "t_test",
        if ratio {
            "Ratio paired t test"
        } else {
            "Paired t test"
        },
    );
    s.methods = methods(&format!(
        "{}, {}, {} CI, {} pairs",
        if ratio {
            "Ratio paired t test (paired t test on log10 ratios)"
        } else {
            "Paired t test"
        },
        sidedness(spec.alternative, &gt, &lt),
        pct(spec.ci_level),
        p.x.len()
    ));
    let n = p.x.len();
    let (mx, sx) = super::mean_sd(&p.x);
    let (my, sy) = super::mean_sd(&p.y);
    let (md, sd_d) = super::mean_sd(&d);
    s.sections.push(section(
        "Descriptive",
        &DESC_COLS,
        vec![
            desc_cells(&an, n, mx, sx),
            desc_cells(&bn, n, my, sy),
            desc_cells(
                &if ratio {
                    format!("log10({an} / {bn})")
                } else {
                    format!("Differences ({an} − {bn})")
                },
                n,
                md,
                sd_d,
            ),
        ],
    ));
    s.sections.push(kv("Test", test_rows(&r, spec, &gt, &lt)));
    s.sections.push(diff_section(
        &if ratio {
            format!("Mean log10({an} / {bn})")
        } else {
            format!("Mean of {an} − {bn}")
        },
        &r,
    ));
    if let Some(rr) = &ratio_res {
        let lvl = pct(spec.ci_level);
        s.sections.push(section_owned(
            "Ratio",
            vec![
                "Quantity".into(),
                "Estimate".into(),
                format!("Lower {lvl} CI"),
                format!("Upper {lvl} CI"),
            ],
            vec![vec![
                txt(format!("Geometric mean of {an} / {bn}")),
                num(rr.geometric_mean_ratio),
                num(rr.ci_ratio.0),
                num(rr.ci_ratio.1),
            ]],
        ));
    }
    s.sections
        .push(effect_section(&r, n, 0, spec.ci_level, true));
    // Was pairing effective?
    let pr = pearson(&p.x, &p.y, 0.95).ok();
    if let Some(c) = &pr {
        s.sections.push(section(
            "Pairing",
            &["Quantity", "r", "P (one-sided, r > 0)"],
            vec![vec![
                txt(format!("Correlation of {an} and {bn}")),
                num(c.estimate),
                pv(c.p_greater),
            ]],
        ));
    }
    s.assumptions
        .push(checks::values_used(&notes, p.dropped > 0));
    s.assumptions
        .push(checks::sample_size(&[("Pairs".into(), n)], 5));
    let subject = if ratio {
        "the log ratios"
    } else {
        "the paired differences"
    };
    let sets = vec![(subject.to_string(), d.clone())];
    let (c, sec) = checks::normality(
        &sets,
        subject,
        if ratio {
            "Consider the paired t test on differences if the ratios are not log-normal, or the Wilcoxon signed-rank test."
        } else {
            WILCOXON_ALT
        },
    );
    s.assumptions.push(c);
    s.sections.push(sec);
    s.assumptions.push(checks::outlier_hint(&sets));
    if let Some(c) = &pr {
        s.assumptions.push(if c.p_greater < ALPHA {
            AssumptionCheck::new(
                "Effective pairing",
                CheckStatus::Pass,
                format!(
                    "The paired values are correlated (r = {}, one-sided {}), so pairing removed variation between subjects.",
                    unit(c.estimate, style),
                    p_phrase(c.p_greater, style)
                ),
            )
        } else {
            AssumptionCheck::new(
                "Effective pairing",
                CheckStatus::Caution,
                format!(
                    "The paired values are not detectably correlated (r = {}, one-sided {}). Pairing should still follow the design of the experiment, but it removed little variation here.",
                    unit(c.estimate, style),
                    p_phrase(c.p_greater, style)
                ),
            )
        });
    }
    s.assumptions.push(independence(
        "Each pair must be independent of the other pairs.",
    ));
    if let Some(rr) = &ratio_res {
        s.summary.push(format!(
            "The geometric mean ratio ({an} / {bn}) was {}, t({}) = {}, {}.",
            est_ci(
                rr.geometric_mean_ratio,
                rr.ci_ratio.0,
                rr.ci_ratio.1,
                spec.ci_level
            ),
            df(r.df),
            st(r.t),
            p_dir(r.p, spec.alternative, style)
        ));
    } else {
        s.summary.push(format!(
            "The mean of the paired differences ({an} − {bn}) was {}, t({}) = {}, {}, Hedges' g = {}.",
            est_ci(r.estimate, r.ci.0, r.ci.1, spec.ci_level),
            df(r.df),
            st(r.t),
            p_dir(r.p, spec.alternative, style),
            st(r.hedges_g)
        ));
    }
    s.summary.push(format!(
        "{n} pairs; {an}: M = {}, SD = {}; {bn}: M = {}, SD = {}.",
        g(mx),
        g(sx),
        g(my),
        g(sy)
    ));
    let mut ag = graph_for("before_after", &[ka, kb]);
    add_bracket(&mut ag, ka, kb, r.p_two_sided);
    s.graph_family = Some("before_after".into());
    s.graph = Some(ag);
    Ok(s)
}
