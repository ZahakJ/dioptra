//! Rank tests: Mann–Whitney, Wilcoxon signed-rank, Kruskal–Wallis and Friedman, with
//! Hodges–Lehmann estimates and Dunn's multiple comparisons.

use dioptra_stats::anova::{friedman as friedman_test, kruskal_wallis as kw_test};
use dioptra_stats::descriptive::{median, quartiles, QuantileMethod};
use dioptra_stats::mannwhitney::{self, MannWhitneyOptions, Method};
use dioptra_stats::posthoc::{dunn_friedman, dunn_kruskal, Adjust};
use dioptra_stats::wilcoxon::{self, WilcoxonOptions};

use super::checks;
use super::extract::{self, Group};
use super::*;
use crate::analysis::{Alternative, AnalysisContext, AnalysisSpec};
use crate::results::{CheckStatus, ResultCell};

fn median_row(name: &str, v: &[f64]) -> Vec<ResultCell> {
    let (q1, md, q3) =
        quartiles(v, QuantileMethod::Prism).unwrap_or((f64::NAN, f64::NAN, f64::NAN));
    vec![txt(name), int(v.len()), num(md), num(q1), num(q3)]
}

const MED_COLS: [&str; 5] = [
    "Data set",
    "n",
    "Median",
    "25th percentile",
    "75th percentile",
];

fn notes(gs: &[Group]) -> Vec<String> {
    gs.iter().flat_map(|g| g.notes.iter().cloned()).collect()
}

/// Mann–Whitney U test.
pub fn mann_whitney(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::MannWhitney(spec) = ctx.spec else {
        return Err("not a Mann–Whitney spec".into());
    };
    check_level(spec.ci_level)?;
    let style = ctx.p_style;
    let gs = extract::groups(ctx.table, &spec.columns, Some(2))?;
    extract::require_raw(&gs, "The Mann–Whitney test")?;
    if gs.len() < 2 {
        return Err("The Mann–Whitney test compares two data sets; choose two.".into());
    }
    let (a, b) = (&gs[0], &gs[1]);
    if a.values.is_empty() || b.values.is_empty() {
        return Err("Each data set needs at least one value.".into());
    }
    let opts = MannWhitneyOptions {
        alternative: alt(spec.alternative),
        method: match spec.exact {
            None => Method::Auto,
            Some(true) => Method::Exact,
            Some(false) => Method::Asymptotic,
        },
        continuity: true,
        ci_level: spec.ci_level,
    };
    let r = mannwhitney::mann_whitney(&a.values, &b.values, &opts).map_err(|e| e.to_string())?;
    let (an, bn) = (a.name.as_str(), b.name.as_str());
    let gt = format!("{an} tends to be larger than {bn}");
    let lt = format!("{an} tends to be smaller than {bn}");
    let mut s = sheet("mann_whitney", "Mann–Whitney test");
    s.methods = methods(&format!(
        "Mann–Whitney U test ({}), {}, Hodges–Lehmann shift with {} CI",
        if r.exact {
            "exact p"
        } else {
            "normal approximation with tie and continuity correction"
        },
        sidedness(spec.alternative, &gt, &lt),
        pct(spec.ci_level)
    ));
    s.sections.push(section(
        "Descriptive",
        &MED_COLS,
        vec![median_row(an, &a.values), median_row(bn, &b.values)],
    ));
    let mut rows = vec![
        ("U (Prism's, the smaller)".to_string(), num(r.u)),
        (format!("U for {an}"), num(r.u1)),
        (format!("Rank sum of {an}"), num(r.rank_sum_x)),
        (format!("Rank sum of {bn}"), num(r.rank_sum_y)),
        ("z (tie-corrected)".to_string(), num(r.z)),
        ("P value (two-sided)".to_string(), pv(r.p_two_sided)),
    ];
    match spec.alternative {
        Alternative::TwoSided => {}
        Alternative::Greater => {
            rows.push((format!("P value (one-sided, H1: {gt})"), pv(r.p_greater)))
        }
        Alternative::Less => rows.push((format!("P value (one-sided, H1: {lt})"), pv(r.p_less))),
    }
    rows.push(("Exact P".into(), yes_no(r.exact)));
    rows.push(("Method".into(), txt(r.method.clone())));
    s.sections.push(kv("Test", rows));
    let lvl = pct(spec.ci_level);
    s.sections.push(section_owned(
        "Difference",
        vec![
            "Comparison".into(),
            "Hodges–Lehmann shift".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
            "Achieved level".into(),
        ],
        vec![vec![
            txt(format!("{an} − {bn}")),
            num(r.hodges_lehmann),
            r.ci.map_or(ResultCell::Empty, |c| num(c.0)),
            r.ci.map_or(ResultCell::Empty, |c| num(c.1)),
            r.ci_achieved_level.map_or(ResultCell::Empty, num),
        ]],
    ));
    // Effect size: rank-biserial r = 1 - 2U1'/(n1 n2) with U1 counting x > y.
    let rb = 2.0 * r.u1 / (r.n1 as f64 * r.n2 as f64) - 1.0;
    s.sections.push(section(
        "Effect size",
        &["Measure", "Value"],
        vec![vec![
            txt(format!("Rank-biserial correlation ({an} > {bn})")),
            num(rb),
        ]],
    ));
    s.assumptions
        .push(checks::values_used(&notes(&gs), gs.len() > 2));
    s.assumptions.push(checks::sample_size(
        &[
            (a.name.clone(), a.values.len()),
            (b.name.clone(), b.values.len()),
        ],
        5,
    ));
    let sets = vec![
        (a.name.clone(), a.values.clone()),
        (b.name.clone(), b.values.clone()),
    ];
    if let Some((c, sec)) = checks::equal_variances(&sets, false, "Report the medians and the rank-biserial correlation, and do not read the Hodges–Lehmann shift as a difference in medians; if the data are roughly Gaussian, Welch's t test is an option.") {
        let c = AssumptionCheck {
            name: "Similar shapes (spread)".into(),
            detail: if c.status == CheckStatus::Pass {
                format!("{} The Hodges–Lehmann shift reads as a difference in location when the two distributions have the same shape.", c.detail)
            } else {
                format!("{} With different spreads a small p can reflect the spread rather than a shift, and the Hodges–Lehmann shift is not a difference of medians.", c.detail)
            },
            status: if c.status == CheckStatus::Fail { CheckStatus::Caution } else { c.status },
            alternative: c.alternative,
        };
        s.assumptions.push(c);
        s.sections.push(sec);
    }
    s.assumptions.push(AssumptionCheck::new(
        "Exact or approximate P",
        if r.exact { CheckStatus::Pass } else { CheckStatus::Caution },
        if r.exact {
            "The P value is exact (no ties).".to_string()
        } else if r.has_ties {
            "The data have ties, so P comes from the tie-corrected normal approximation; with small samples it is approximate.".to_string()
        } else {
            "P comes from the normal approximation (large samples).".to_string()
        },
    ));
    s.assumptions.push(independence(
        "Every value must be independent, and the two groups unrelated.",
    ));
    let ci = r.ci.unwrap_or((f64::NAN, f64::NAN));
    s.summary.push(format!(
        "The Hodges–Lehmann shift ({an} − {bn}) was {}, U = {}, {}{}.",
        est_ci(r.hodges_lehmann, ci.0, ci.1, spec.ci_level),
        g(r.u),
        p_dir(r.p, spec.alternative, style),
        if r.exact { " (exact)" } else { "" }
    ));
    s.summary.push(format!(
        "Medians: {an} {} (n = {}), {bn} {} (n = {}).",
        g(median(&a.values).unwrap_or(f64::NAN)),
        a.values.len(),
        g(median(&b.values).unwrap_or(f64::NAN)),
        b.values.len()
    ));
    let mut ag = graph_for("box", &[a.dataset, b.dataset]);
    add_bracket(&mut ag, a.dataset, b.dataset, r.p_two_sided);
    s.graph_family = Some("box".into());
    s.graph = Some(ag);
    Ok(s)
}

/// Wilcoxon signed-rank test (paired, or one column against a hypothetical median).
pub fn wilcoxon(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Wilcoxon(spec) = ctx.spec else {
        return Err("not a Wilcoxon spec".into());
    };
    check_level(spec.ci_level)?;
    let style = ctx.p_style;
    let t = ctx.table;
    let keys = extract::pick(t, &spec.columns, Some(2))?;
    if keys.is_empty() {
        return Err("There is no data set to test.".into());
    }
    let opts = WilcoxonOptions {
        alternative: alt(spec.alternative),
        ci_level: spec.ci_level,
        ..Default::default()
    };
    let mut s = sheet("wilcoxon", "Wilcoxon signed-rank test");
    let mut notes_v = Vec::new();
    let (r, label, d, n_pairs, one) = if keys.len() == 1 {
        let gr = extract::group(t, keys[0])?;
        extract::require_raw(std::slice::from_ref(&gr), "The Wilcoxon signed-rank test")?;
        notes_v.extend(gr.notes.clone());
        let r = wilcoxon::one_sample(&gr.values, spec.mu, &opts).map_err(|e| e.to_string())?;
        s.sections.push(section(
            "Descriptive",
            &MED_COLS,
            vec![median_row(&gr.name, &gr.values)],
        ));
        let d: Vec<f64> = gr.values.iter().map(|v| v - spec.mu).collect();
        let n = d.len();
        (r, format!("{} − {}", gr.name, g(spec.mu)), d, n, true)
    } else {
        let (ka, kb) = (keys[0], keys[1]);
        let (an, bn) = (extract::key_name(t, ka), extract::key_name(t, kb));
        let p = extract::paired(t, ka, kb)?;
        if p.dropped > 0 {
            notes_v.push(format!(
                "{} row{} with only one value of the pair left out.",
                p.dropped,
                if p.dropped == 1 { "" } else { "s" }
            ));
        }
        if p.x.is_empty() {
            return Err("There are no complete pairs.".into());
        }
        let r = wilcoxon::paired(&p.x, &p.y, &opts).map_err(|e| e.to_string())?;
        let d: Vec<f64> = p.x.iter().zip(&p.y).map(|(a, b)| a - b).collect();
        s.sections.push(section(
            "Descriptive",
            &MED_COLS,
            vec![
                median_row(&an, &p.x),
                median_row(&bn, &p.y),
                median_row(&format!("Differences ({an} − {bn})"), &d),
            ],
        ));
        let n = d.len();
        (r, format!("{an} − {bn}"), d, n, false)
    };
    let gt = format!("{label} > 0");
    let lt = format!("{label} < 0");
    s.methods = methods(&format!(
        "Wilcoxon signed-rank test ({}, zeros dropped), {}, Hodges–Lehmann pseudo-median with {} CI",
        if r.exact { "exact p" } else { "normal approximation with tie correction" },
        sidedness(spec.alternative, &gt, &lt),
        pct(spec.ci_level)
    ));
    let mut rows = vec![
        ("W+ (sum of positive ranks)".to_string(), num(r.w_plus)),
        ("W− (sum of negative ranks)".to_string(), num(r.w_minus)),
        (
            "W (sum of signed ranks)".to_string(),
            num(r.w_plus - r.w_minus),
        ),
        ("Nonzero differences".to_string(), int(r.n - r.n_zero)),
        ("Zero differences (dropped)".to_string(), int(r.n_zero)),
        ("z".to_string(), num(r.z)),
        ("P value (two-sided)".to_string(), pv(r.p_two_sided)),
    ];
    match spec.alternative {
        Alternative::TwoSided => {}
        Alternative::Greater => {
            rows.push((format!("P value (one-sided, H1: {gt})"), pv(r.p_greater)))
        }
        Alternative::Less => rows.push((format!("P value (one-sided, H1: {lt})"), pv(r.p_less))),
    }
    rows.push(("Exact P".into(), yes_no(r.exact)));
    rows.push(("Method".into(), txt(r.method.clone())));
    s.sections.push(kv("Test", rows));
    let lvl = pct(spec.ci_level);
    s.sections.push(section_owned(
        "Pseudo-median",
        vec![
            "Comparison".into(),
            "Hodges–Lehmann estimate".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
            "Achieved level".into(),
        ],
        vec![vec![
            txt(label.clone()),
            num(r.hodges_lehmann),
            r.ci.map_or(ResultCell::Empty, |c| num(c.0)),
            r.ci.map_or(ResultCell::Empty, |c| num(c.1)),
            r.ci_achieved_level.map_or(ResultCell::Empty, num),
        ]],
    ));
    s.assumptions.push(checks::values_used(
        &notes_v,
        !one && notes_v.iter().any(|n| n.contains("pair")),
    ));
    s.assumptions.push(checks::sample_size(
        &[(if one { "Values" } else { "Pairs" }.into(), n_pairs)],
        6,
    ));
    // Symmetry of the differences about their median (what the signed-rank test assumes).
    let sk = dioptra_stats::descriptive::skewness(&d).unwrap_or(f64::NAN);
    s.assumptions.push(if !sk.is_finite() {
        AssumptionCheck::new(
            "Symmetric differences",
            CheckStatus::NotChecked,
            "Too few differences to judge symmetry.",
        )
    } else if sk.abs() > 1.0 {
        AssumptionCheck::new(
            "Symmetric differences",
            CheckStatus::Caution,
            format!("The differences are skewed (skewness {}); the signed-rank test assumes they are symmetric about their median, so read P as a test of location only cautiously.", st(sk)),
        )
        .or_else("The sign test makes no symmetry assumption; a transformation (e.g. logarithms) may restore symmetry.")
    } else {
        AssumptionCheck::new(
            "Symmetric differences",
            CheckStatus::Pass,
            format!("The differences look roughly symmetric (skewness {}), as the test assumes.", st(sk)),
        )
    });
    s.assumptions.push(AssumptionCheck::new(
        "Exact or approximate P",
        if r.exact { CheckStatus::Pass } else { CheckStatus::Caution },
        if r.exact {
            "The P value is exact (no ties or zeros).".to_string()
        } else {
            "Ties or zero differences (or more than 50 pairs) mean P comes from the normal approximation.".to_string()
        },
    ));
    s.assumptions.push(independence(if one {
        "Every value must be independent."
    } else {
        "Each pair must be independent of the other pairs."
    }));
    let ci = r.ci.unwrap_or((f64::NAN, f64::NAN));
    s.summary.push(format!(
        "The Hodges–Lehmann median of the differences ({label}) was {}, W+ = {}, W− = {}, {}{}.",
        est_ci(r.hodges_lehmann, ci.0, ci.1, spec.ci_level),
        g(r.w_plus),
        g(r.w_minus),
        p_dir(r.p, spec.alternative, style),
        if r.exact { " (exact)" } else { "" }
    ));
    let fam = if one { "box" } else { "paired" };
    let mut ag = graph_for(fam, &keys);
    if !one {
        add_bracket(&mut ag, keys[0], keys[1], r.p_two_sided);
    }
    s.graph_family = Some(fam.into());
    s.graph = Some(ag);
    Ok(s)
}

/// Kruskal–Wallis test with Dunn's comparisons.
pub fn kruskal_wallis(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::KruskalWallis(spec) = ctx.spec else {
        return Err("not a Kruskal–Wallis spec".into());
    };
    let style = ctx.p_style;
    let gs = extract::groups(ctx.table, &spec.columns, None)?;
    extract::require_raw(&gs, "The Kruskal–Wallis test")?;
    let gs: Vec<Group> = gs.into_iter().filter(|g| !g.values.is_empty()).collect();
    if gs.len() < 2 {
        return Err("The Kruskal–Wallis test needs at least two data sets with values.".into());
    }
    let refs: Vec<&[f64]> = gs.iter().map(|g| g.values.as_slice()).collect();
    let r = kw_test(&refs).map_err(|e| e.to_string())?;
    let names: Vec<String> = gs.iter().map(|g| g.name.clone()).collect();
    let mut s = sheet("kruskal_wallis", "Kruskal–Wallis test");
    s.methods = methods(&format!(
        "Kruskal–Wallis test (tie-corrected H, chi-square approximation){}",
        if spec.dunn {
            ", Dunn's multiple comparisons with Holm adjustment, α = 0.05"
        } else {
            ""
        }
    ));
    let mut desc = Vec::new();
    for (i, gr) in gs.iter().enumerate() {
        let mut row = median_row(&gr.name, &gr.values);
        row.push(num(r.mean_ranks[i]));
        desc.push(row);
    }
    s.sections.push(section(
        "Descriptive",
        &[
            "Data set",
            "n",
            "Median",
            "25th percentile",
            "75th percentile",
            "Mean rank",
        ],
        desc,
    ));
    s.sections.push(kv(
        "Test",
        vec![
            ("H (tie-corrected)".into(), num(r.h)),
            ("df".into(), num(r.df)),
            ("P value".into(), pv(r.p)),
            ("ε² (epsilon squared)".into(), num(r.epsilon_squared)),
            ("Tie correction".into(), num(r.tie_correction)),
        ],
    ));
    s.summary.push(format!(
        "Kruskal–Wallis test: H({}) = {}, {}, ε² = {}.",
        df(r.df),
        st(r.h),
        p_phrase(r.p, style),
        unit(r.epsilon_squared, style)
    ));
    let keys: Vec<Option<usize>> = gs.iter().map(|g| g.dataset).collect();
    let mut ag = graph_for("box", &keys);
    if spec.dunn {
        let ph = dunn_kruskal(&refs, None, Adjust::Holm, 0.05).map_err(|e| e.to_string())?;
        let (sec, rows) = posthoc_table(
            "Dunn's multiple comparisons (Holm)",
            &ph,
            &names,
            "Mean rank diff.",
            "z",
        );
        s.sections.push(sec);
        s.summary.push(family_sentence(
            "Dunn's multiple comparisons (Holm)",
            &rows,
            "mean rank difference",
            0.95,
            style,
        ));
        for c in &ph.comparisons {
            add_bracket(&mut ag, keys[c.i], keys[c.j], c.p_adj);
        }
        prune_brackets(&mut ag);
    }
    s.assumptions.push(checks::values_used(&notes(&gs), false));
    let small = gs.iter().any(|g| g.values.len() < 5);
    s.assumptions.push(if small {
        AssumptionCheck::new(
            "Chi-square approximation",
            CheckStatus::Caution,
            "A group has fewer than 5 values; the chi-square approximation to H is rough there, and Dioptra does not compute the exact small-sample P.",
        )
        .or_else("Report the P value as approximate; with very small groups no rank test has much power.")
    } else {
        AssumptionCheck::new(
            "Chi-square approximation",
            CheckStatus::Pass,
            "Every group has at least 5 values, so the chi-square approximation to H is adequate.",
        )
    });
    s.assumptions.push(AssumptionCheck::new(
        "Similar shapes",
        CheckStatus::NotChecked,
        "Read as a test of medians, Kruskal–Wallis assumes the distributions have the same shape; otherwise it tests whether values in one group tend to be larger.",
    ));
    s.assumptions.push(independence(
        "Every value must be independent, and the groups unrelated.",
    ));
    s.graph_family = Some("box".into());
    s.graph = Some(ag);
    Ok(s)
}

/// Friedman test with Dunn's comparisons.
pub fn friedman(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Friedman(spec) = ctx.spec else {
        return Err("not a Friedman spec".into());
    };
    let style = ctx.p_style;
    let t = ctx.table;
    let keys = extract::pick(t, &spec.columns, None)?;
    if keys.len() < 2 {
        return Err("The Friedman test needs at least two matched data sets.".into());
    }
    let mx = extract::matrix(t, &keys)?;
    if mx.m.len() < 2 {
        return Err("The Friedman test needs at least two complete rows (subjects).".into());
    }
    let r = friedman_test(&mx.m).map_err(|e| e.to_string())?;
    let names: Vec<String> = keys.iter().map(|k| extract::key_name(t, *k)).collect();
    let mut s = sheet("friedman", "Friedman test");
    s.methods = methods(&format!(
        "Friedman test (ranks within each subject, tie-corrected, chi-square approximation), {} subjects{}",
        r.n_subjects,
        if spec.dunn { ", Dunn's multiple comparisons with Holm adjustment, α = 0.05" } else { "" }
    ));
    let desc: Vec<Vec<ResultCell>> = (0..keys.len())
        .map(|j| {
            let col: Vec<f64> = mx.m.iter().map(|row| row[j]).collect();
            let mut row = median_row(&names[j], &col);
            row.push(num(r.mean_ranks[j]));
            row
        })
        .collect();
    s.sections.push(section(
        "Descriptive",
        &[
            "Data set",
            "n",
            "Median",
            "25th percentile",
            "75th percentile",
            "Mean rank",
        ],
        desc,
    ));
    s.sections.push(kv(
        "Test",
        vec![
            ("Friedman χ² (tie-corrected)".into(), num(r.chi2)),
            ("df".into(), num(r.df)),
            ("P value".into(), pv(r.p)),
            ("Kendall's W".into(), num(r.kendall_w)),
            ("Subjects used".into(), int(r.n_subjects)),
            ("Subjects dropped (incomplete)".into(), int(mx.dropped)),
        ],
    ));
    s.summary.push(format!(
        "Friedman test: χ²({}, N = {}) = {}, {}, Kendall's W = {}.",
        df(r.df),
        r.n_subjects,
        st(r.chi2),
        p_phrase(r.p, style),
        unit(r.kendall_w, style)
    ));
    let mut ag = graph_for("paired", &keys);
    if spec.dunn {
        let ph = dunn_friedman(&mx.m, None, Adjust::Holm, 0.05).map_err(|e| e.to_string())?;
        let (sec, rows) = posthoc_table(
            "Dunn's multiple comparisons (Holm)",
            &ph,
            &names,
            "Mean rank diff.",
            "z",
        );
        s.sections.push(sec);
        s.summary.push(family_sentence(
            "Dunn's multiple comparisons (Holm)",
            &rows,
            "mean rank difference",
            0.95,
            style,
        ));
        for c in &ph.comparisons {
            add_bracket(&mut ag, keys[c.i], keys[c.j], c.p_adj);
        }
        prune_brackets(&mut ag);
    }
    let mut nv = Vec::new();
    if mx.dropped > 0 {
        nv.push(format!(
            "{} row{} with a missing condition left out (Friedman needs every condition for a subject).",
            mx.dropped,
            if mx.dropped == 1 { "" } else { "s" }
        ));
    }
    s.assumptions.push(checks::values_used(&nv, mx.dropped > 0));
    s.assumptions.push(if r.n_subjects < 10 {
        AssumptionCheck::new(
            "Chi-square approximation",
            CheckStatus::Caution,
            format!("Only {} subjects: the chi-square approximation is rough, and Dioptra does not compute the exact P.", r.n_subjects),
        )
    } else {
        AssumptionCheck::new(
            "Chi-square approximation",
            CheckStatus::Pass,
            "At least 10 subjects, so the chi-square approximation is adequate.",
        )
    });
    s.assumptions.push(independence(
        "Each row must be one subject (or matched set), independent of the other rows.",
    ));
    s.graph_family = Some("paired".into());
    s.graph = Some(ag);
    Ok(s)
}
