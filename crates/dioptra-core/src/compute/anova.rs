//! ANOVA: one-way (with Welch and Brown–Forsythe), repeated-measures one-way (with
//! sphericity), and two-way (type I/II/III; one repeated factor as a split-plot design),
//! with the multiple-comparison families the spec asks for.

use dioptra_stats::anova::{
    one_way_anova, rm_anova, two_way_anova, SsType as StatsSs, TwoWayOptions,
};
use dioptra_stats::dist::{
    p_two_sided_t, p_upper_f, t_critical_two_sided, Continuous, StudentizedRange,
};
use dioptra_stats::posthoc::{dunnett, p_adjust, pairwise_t, tukey_kramer, Adjust, PostHoc};
use dioptra_stats::ttest;

use super::checks;
use super::extract::{self, Group};
use super::*;
use crate::analysis::{
    AnalysisContext, AnalysisSpec, Comparisons, McMethod, MultipleComparisons, RepeatedFactor,
    SsType,
};
use crate::results::{CheckStatus, ResultCell};
use crate::table::Table;

const KW_ALT: &str =
    "Consider the Kruskal–Wallis test (Analyze → Kruskal–Wallis test), or report both.";

/// How a multiple-comparison request is carried out.
#[derive(Debug, Clone, PartialEq)]
enum Plan {
    Tukey,
    Dunnett(usize),
    Pairwise(Option<Vec<(usize, usize)>>, Adjust),
}

fn adjust_of(m: McMethod) -> Adjust {
    match m {
        McMethod::Sidak => Adjust::Sidak,
        McMethod::Holm => Adjust::HolmSidak,
        McMethod::Bonferroni => Adjust::Bonferroni,
        McMethod::BenjaminiHochberg => Adjust::BenjaminiHochberg,
        _ => Adjust::HolmSidak,
    }
}

fn adjust_name(a: Adjust) -> &'static str {
    match a {
        Adjust::None => "Fisher's LSD (no adjustment)",
        Adjust::Bonferroni => "Bonferroni",
        Adjust::Sidak => "Šidák",
        Adjust::Holm => "Holm",
        Adjust::HolmSidak => "Holm–Šidák",
        Adjust::Hochberg => "Hochberg",
        Adjust::Hommel => "Hommel",
        Adjust::BenjaminiHochberg => "Benjamini–Hochberg (FDR)",
        Adjust::BenjaminiYekutieli => "Benjamini–Yekutieli (FDR)",
        Adjust::TwoStageBky { .. } => "two-stage Benjamini–Krieger–Yekutieli (FDR)",
    }
}

/// Map dataset indices in the spec to group positions; resolve the plan.
fn plan(
    mc: &MultipleComparisons,
    keys: &[Option<usize>],
    names: &[String],
) -> Result<(Plan, Vec<String>), String> {
    let pos = |d: usize| -> Result<usize, String> {
        keys.iter().position(|k| *k == Some(d)).ok_or_else(|| {
            format!(
                "Data set {} is not among the data sets analysed ({}).",
                crate::util::letters(d),
                names.join(", ")
            )
        })
    };
    let mut notes = Vec::new();
    let p = match (&mc.method, &mc.comparisons) {
        (McMethod::Dunnett { control }, _) => Plan::Dunnett(pos(*control)?),
        (McMethod::Tukey | McMethod::Dunn, Comparisons::VsControl { control }) => {
            if mc.method == McMethod::Tukey {
                notes.push("Comparisons with a control use Dunnett's test (Tukey's test is for all pairs).".into());
            }
            Plan::Dunnett(pos(*control)?)
        }
        (McMethod::Tukey | McMethod::Dunn, Comparisons::AllPairs) => Plan::Tukey,
        (McMethod::Tukey | McMethod::Dunn, Comparisons::Selected { pairs }) => {
            notes.push(
                "Selected pairs use t tests on the pooled variance with Holm–Šidák adjustment."
                    .into(),
            );
            let pr = pairs
                .iter()
                .map(|(a, b)| Ok((pos(*a)?, pos(*b)?)))
                .collect::<Result<Vec<_>, String>>()?;
            Plan::Pairwise(Some(pr), Adjust::HolmSidak)
        }
        (m, Comparisons::AllPairs) => Plan::Pairwise(None, adjust_of(*m)),
        (m, Comparisons::VsControl { control }) => {
            let c = pos(*control)?;
            Plan::Pairwise(
                Some(
                    (0..keys.len())
                        .filter(|&i| i != c)
                        .map(|i| (i, c))
                        .collect(),
                ),
                adjust_of(*m),
            )
        }
        (m, Comparisons::Selected { pairs }) => {
            let pr = pairs
                .iter()
                .map(|(a, b)| Ok((pos(*a)?, pos(*b)?)))
                .collect::<Result<Vec<_>, String>>()?;
            Plan::Pairwise(Some(pr), adjust_of(*m))
        }
    };
    if mc.method == McMethod::Dunn {
        notes.push("Dunn's test belongs after rank tests; after ANOVA the matching parametric family is used.".into());
    }
    if let Plan::Pairwise(Some(pr), _) = &p {
        if pr.iter().any(|(a, b)| a == b) {
            return Err("A comparison must involve two different data sets.".into());
        }
    }
    Ok((p, notes))
}

fn run_plan(p: &Plan, groups: &[&[f64]]) -> Result<(PostHoc, String, &'static str), String> {
    Ok(match p {
        Plan::Tukey => (
            tukey_kramer(groups, 0.05).map_err(|e| e.to_string())?,
            "Tukey's multiple comparisons".into(),
            "q",
        ),
        Plan::Dunnett(c) => (
            dunnett(
                groups,
                *c,
                dioptra_stats::ttest::Alternative::TwoSided,
                0.05,
            )
            .map_err(|e| e.to_string())?,
            "Dunnett's multiple comparisons".into(),
            "t",
        ),
        Plan::Pairwise(pairs, adj) => (
            pairwise_t(groups, pairs.as_deref(), *adj, 0.05).map_err(|e| e.to_string())?,
            format!("Multiple comparisons ({})", adjust_name(*adj)),
            "t",
        ),
    })
}

fn notes(gs: &[Group]) -> Vec<String> {
    gs.iter().flat_map(|g| g.notes.iter().cloned()).collect()
}

/// One-way ANOVA.
pub fn one_way(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::OneWayAnova(spec) = ctx.spec else {
        return Err("not a one-way ANOVA spec".into());
    };
    let style = ctx.p_style;
    let gs: Vec<Group> = extract::groups(ctx.table, &spec.columns, None)?
        .into_iter()
        .filter(|g| g.n() > 0)
        .collect();
    if gs.len() < 2 {
        return Err("One-way ANOVA needs at least two data sets with values.".into());
    }
    let summary_data = gs.iter().any(|g| g.summary.is_some());
    let eff: Vec<Vec<f64>> = gs.iter().map(|g| g.effective()).collect();
    let refs: Vec<&[f64]> = eff.iter().map(|v| v.as_slice()).collect();
    let ow = one_way_anova(&refs).map_err(|e| e.to_string())?;
    let names: Vec<String> = gs.iter().map(|g| g.name.clone()).collect();
    let keys: Vec<Option<usize>> = gs.iter().map(|g| g.dataset).collect();
    let mut s = sheet("one_way_anova", "One-way ANOVA");
    let mut m = format!(
        "Ordinary one-way ANOVA{}{}",
        if spec.welch {
            " with Welch's ANOVA (unequal variances)"
        } else {
            ""
        },
        if spec.brown_forsythe {
            " with the Brown–Forsythe test of means"
        } else {
            ""
        }
    );
    // Descriptive and ANOVA table.
    s.sections.push(section(
        "Descriptive",
        &DESC_COLS,
        gs.iter().map(desc_row).collect(),
    ));
    s.sections.push(section(
        "ANOVA table",
        &["Source", "SS", "df", "MS", "F", "P"],
        vec![
            vec![
                txt("Treatment (between groups)"),
                num(ow.ss_between),
                num(ow.df_between),
                num(ow.ms_between),
                num(ow.f),
                pv(ow.p),
            ],
            vec![
                txt("Residual (within groups)"),
                num(ow.ss_within),
                num(ow.df_within),
                num(ow.ms_within),
                ResultCell::Empty,
                ResultCell::Empty,
            ],
            vec![
                txt("Total"),
                num(ow.ss_total),
                num(ow.df_total),
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
            ],
        ],
    ));
    let mut robust = Vec::new();
    if spec.welch {
        if let Some(w) = &ow.welch {
            robust.push(vec![
                txt("Welch's ANOVA"),
                num(w.f),
                num(w.df1),
                num(w.df2),
                pv(w.p),
            ]);
        }
    }
    if spec.brown_forsythe {
        if let Some(b) = &ow.brown_forsythe {
            robust.push(vec![
                txt("Brown–Forsythe ANOVA"),
                num(b.f),
                num(b.df1),
                num(b.df2),
                pv(b.p),
            ]);
        }
    }
    if !robust.is_empty() {
        s.sections.push(section(
            "Tests not assuming equal variances",
            &["Test", "F", "DFn", "DFd", "P"],
            robust,
        ));
    }
    let cohen_f = (ow.r_squared / (1.0 - ow.r_squared)).sqrt();
    s.sections.push(section(
        "Effect size",
        &["Measure", "Value"],
        vec![
            vec![txt("η² (R²)"), num(ow.r_squared)],
            vec![txt("ω²"), num(ow.omega_squared)],
            vec![txt("Cohen's f"), num(cohen_f)],
        ],
    ));
    // Lead sentence.
    if let (true, Some(w)) = (spec.welch, &ow.welch) {
        s.summary.push(format!(
            "Welch's ANOVA: F({}, {}) = {}, {}; ordinary ANOVA F({}, {}) = {}, {}, η² = {}.",
            df(w.df1),
            df(w.df2),
            st(w.f),
            p_phrase(w.p, style),
            df(ow.df_between),
            df(ow.df_within),
            st(ow.f),
            p_phrase(ow.p, style),
            unit(ow.r_squared, style)
        ));
    } else {
        s.summary.push(format!(
            "One-way ANOVA: F({}, {}) = {}, {}, η² = {} (ω² = {}).",
            df(ow.df_between),
            df(ow.df_within),
            st(ow.f),
            p_phrase(ow.p, style),
            unit(ow.r_squared, style),
            unit(ow.omega_squared, style)
        ));
    }
    if let (true, Some(b)) = (spec.brown_forsythe, &ow.brown_forsythe) {
        s.summary.push(format!(
            "Brown–Forsythe ANOVA: F({}, {}) = {}, {}.",
            df(b.df1),
            df(b.df2),
            st(b.f),
            p_phrase(b.p, style)
        ));
    }
    // Multiple comparisons.
    let mut ag = graph_for("column", &keys);
    let mut extra_notes = Vec::new();
    if let Some(mc) = &spec.post_hoc {
        let (p, n) = plan(mc, &keys, &names)?;
        extra_notes.extend(n);
        let (ph, title, stat) = run_plan(&p, &refs)?;
        let (sec, rows) = posthoc_table(&title, &ph, &names, "Mean diff.", stat);
        s.sections.push(sec);
        s.summary.push(family_sentence(
            &title,
            &rows,
            "mean difference",
            0.95,
            style,
        ));
        for c in &ph.comparisons {
            add_bracket(&mut ag, keys[c.i], keys[c.j], c.p_adj);
        }
        prune_brackets(&mut ag);
        m.push_str(&format!(
            ", {} (family-wise α = 0.05)",
            title
                .replace("Multiple comparisons (", "multiple comparisons with ")
                .trim_end_matches(')')
        ));
    }
    s.methods = methods(&format!(
        "{m}{}",
        if summary_data {
            ", computed from the entered mean, SD and N"
        } else {
            ""
        }
    ));
    // Assumption checks.
    let mut nv = notes(&gs);
    nv.extend(extra_notes);
    s.assumptions.push(checks::values_used(&nv, false));
    s.assumptions.push(checks::sample_size(
        &gs.iter()
            .map(|g| (g.name.clone(), g.n()))
            .collect::<Vec<_>>(),
        5,
    ));
    if !summary_data {
        let resid: Vec<f64> = gs
            .iter()
            .flat_map(|g| {
                let (mu, _) = mean_sd(&g.values);
                g.values.iter().map(move |v| v - mu)
            })
            .collect();
        let (c, sec) = checks::normality(&[("Residuals".into(), resid)], "the residuals", KW_ALT);
        s.assumptions.push(c);
        s.sections.push(sec);
        let sets: Vec<(String, Vec<f64>)> = gs
            .iter()
            .map(|g| (g.name.clone(), g.values.clone()))
            .collect();
        if let Some((c, sec)) = checks::equal_variances(
            &sets,
            spec.welch || spec.brown_forsythe,
            "Turn on Welch's ANOVA (option \"welch\"), which does not assume equal variances.",
        ) {
            s.assumptions.push(c);
            s.sections.push(sec);
        }
        s.assumptions.push(checks::outlier_hint(&sets));
    } else {
        s.assumptions.push(AssumptionCheck::new(
            "Gaussian distribution of the residuals",
            CheckStatus::NotChecked,
            "Summary data (mean, SD and N) cannot show the distribution; ANOVA assumes it is roughly Gaussian.",
        ));
        if let Some(b) = &ow.bartlett {
            let status = if b.p < 0.01 {
                CheckStatus::Fail
            } else if b.p < 0.05 {
                CheckStatus::Caution
            } else {
                CheckStatus::Pass
            };
            let mut c = AssumptionCheck::new(
                "Equal variances",
                status,
                format!(
                    "Bartlett's test on the entered SDs: χ²({}) = {}, {}.",
                    df(b.df1),
                    st(b.statistic),
                    p_phrase(b.p, style)
                ),
            );
            if status != CheckStatus::Pass {
                c = c.or_else("Turn on Welch's ANOVA (option \"welch\"), which does not assume equal variances.");
            }
            s.assumptions.push(c);
        }
    }
    s.assumptions.push(independence(
        "Every value must be independent, and the groups unrelated (for matched data use repeated-measures ANOVA).",
    ));
    s.graph_family = Some("column".into());
    s.graph = Some(ag);
    Ok(s)
}

/// The table, the family sentence rows, and (i, j, adjusted p) for brackets.
type RmOut = (Section, Vec<CompRow>, Vec<(usize, usize, f64)>);

/// Pairwise paired t tests (for repeated measures), adjusted over the family.
fn rm_pairwise(
    m: &[Vec<f64>],
    pairs: &[(usize, usize)],
    adj: Adjust,
    names: &[String],
) -> Result<RmOut, String> {
    let mut res = Vec::new();
    for &(i, j) in pairs {
        let x: Vec<f64> = m.iter().map(|r| r[i]).collect();
        let y: Vec<f64> = m.iter().map(|r| r[j]).collect();
        res.push(ttest::paired(&x, &y, &Default::default()).map_err(|e| e.to_string())?);
    }
    let raw: Vec<f64> = res.iter().map(|r| r.p_two_sided).collect();
    let adjp = p_adjust(&raw, adj);
    let k = pairs.len() as f64;
    let ci_alpha = match adj {
        Adjust::Bonferroni => Some(0.05 / k),
        Adjust::Sidak => Some(1.0 - 0.95f64.powf(1.0 / k)),
        _ => None,
    };
    let mut rows = Vec::new();
    let mut comp = Vec::new();
    let mut br = Vec::new();
    for (n, ((i, j), r)) in pairs.iter().zip(&res).enumerate() {
        let label = format!("{} vs. {}", names[*i], names[*j]);
        let ci = ci_alpha.map(|a| {
            let tc = t_critical_two_sided(a, r.df);
            (r.estimate - tc * r.se, r.estimate + tc * r.se)
        });
        let sig = adjp[n] < 0.05;
        rows.push(vec![
            txt(label.clone()),
            num(r.estimate),
            ci.map_or(ResultCell::Empty, |c| num(c.0)),
            ci.map_or(ResultCell::Empty, |c| num(c.1)),
            num(r.t),
            num(r.df),
            pv(r.p_two_sided),
            pv(adjp[n]),
            yes_no(sig),
            txt(crate::results::stars(adjp[n])),
        ]);
        comp.push((label, r.estimate, ci, adjp[n], sig));
        br.push((*i, *j, adjp[n]));
    }
    Ok((
        section(
            format!(
                "Multiple comparisons (paired t tests, {})",
                adjust_name(adj)
            ),
            &[
                "Comparison",
                "Mean diff.",
                "Lower 95% CI (adjusted)",
                "Upper 95% CI (adjusted)",
                "t",
                "df",
                "P (unadjusted)",
                "Adjusted P",
                "Significant (α = 0.05)?",
                "Summary",
            ],
            rows,
        ),
        comp,
        br,
    ))
}

/// Repeated-measures one-way ANOVA.
pub fn repeated_measures(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::RepeatedMeasuresAnova(spec) = ctx.spec else {
        return Err("not a repeated-measures ANOVA spec".into());
    };
    let style = ctx.p_style;
    let t = ctx.table;
    let keys = extract::pick(t, &spec.columns, None)?;
    if keys.len() < 2 {
        return Err("Repeated-measures ANOVA needs at least two matched data sets.".into());
    }
    let mx = extract::matrix(t, &keys)?;
    if mx.m.len() < 2 {
        return Err("Repeated-measures ANOVA needs at least two complete rows (subjects).".into());
    }
    let r = rm_anova(&mx.m).map_err(|e| e.to_string())?;
    let names: Vec<String> = keys.iter().map(|k| extract::key_name(t, *k)).collect();
    let k = keys.len();
    let mut s = sheet("repeated_measures_anova", "Repeated-measures one-way ANOVA");
    // Descriptive.
    let desc: Vec<Vec<ResultCell>> = (0..k)
        .map(|j| {
            let col: Vec<f64> = mx.m.iter().map(|row| row[j]).collect();
            let (mu, sd) = mean_sd(&col);
            desc_cells(&names[j], col.len(), mu, sd)
        })
        .collect();
    s.sections.push(section("Descriptive", &DESC_COLS, desc));
    s.sections.push(section(
        "ANOVA table",
        &["Source", "SS", "df", "MS", "F", "P"],
        vec![
            vec![
                txt("Treatment (between columns)"),
                num(r.ss_treatment),
                num(r.df_treatment),
                num(r.ms_treatment),
                num(r.f),
                pv(r.p),
            ],
            vec![
                txt("Subjects (between rows)"),
                num(r.ss_subjects),
                num(r.df_subjects),
                num(r.ms_subjects),
                num(r.f_subjects),
                pv(r.p_subjects),
            ],
            vec![
                txt("Residual"),
                num(r.ss_error),
                num(r.df_error),
                num(r.ms_error),
                ResultCell::Empty,
                ResultCell::Empty,
            ],
            vec![
                txt("Total"),
                num(r.ss_total),
                num(r.df_treatment + r.df_subjects + r.df_error),
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
            ],
        ],
    ));
    s.sections.push(section(
        "Corrected and uncorrected tests",
        &["Correction", "ε", "DFn", "DFd", "F", "P"],
        vec![
            vec![
                txt("None (assumes sphericity)"),
                num(1.0),
                num(r.df_treatment),
                num(r.df_error),
                num(r.f),
                pv(r.p),
            ],
            vec![
                txt("Greenhouse–Geisser"),
                num(r.gg_epsilon),
                num(r.df1_gg),
                num(r.df2_gg),
                num(r.f),
                pv(r.p_gg),
            ],
            vec![
                txt("Huynh–Feldt"),
                num(r.hf_epsilon),
                num(r.df1_hf),
                num(r.df2_hf),
                num(r.f),
                pv(r.p_hf),
            ],
            vec![
                txt("Lower bound"),
                num(r.lower_bound_epsilon),
                num(r.df_treatment * r.lower_bound_epsilon),
                num(r.df_error * r.lower_bound_epsilon),
                num(r.f),
                pv(r.p_lower_bound),
            ],
        ],
    ));
    if k > 2 {
        s.sections.push(kv(
            "Sphericity (Mauchly)",
            vec![
                ("Mauchly's W".into(), num(r.mauchly_w)),
                ("χ²".into(), num(r.mauchly_chi2)),
                ("df".into(), num(r.mauchly_df)),
                ("P".into(), pv(r.mauchly_p)),
            ],
        ));
    }
    s.sections.push(section(
        "Effect size",
        &["Measure", "Value"],
        vec![vec![txt("Partial η²"), num(r.partial_eta_squared)]],
    ));
    let (d1, d2, p) = if spec.gg_correction {
        (r.df1_gg, r.df2_gg, r.p_gg)
    } else {
        (r.df_treatment, r.df_error, r.p)
    };
    s.summary.push(format!(
        "Repeated-measures ANOVA{}: F({}, {}) = {}, {}, partial η² = {}{}.",
        if spec.gg_correction {
            " (Greenhouse–Geisser corrected)"
        } else {
            ""
        },
        df(d1),
        df(d2),
        st(r.f),
        p_phrase(p, style),
        unit(r.partial_eta_squared, style),
        if spec.gg_correction {
            format!(", ε = {}", unit(r.gg_epsilon, style))
        } else {
            String::new()
        }
    ));
    let mut ag = graph_for("paired", &keys);
    let mut mnote = String::new();
    if let Some(mc) = &spec.post_hoc {
        let (p, mut pn) = plan(mc, &keys, &names)?;
        let (pairs, adj): (Vec<(usize, usize)>, Adjust) = match p {
            Plan::Tukey => {
                pn.push("For repeated measures Dioptra compares pairs with paired t tests (they do not assume sphericity) and Holm–Šidák adjustment instead of Tukey's test.".into());
                (
                    (0..k)
                        .flat_map(|i| (i + 1..k).map(move |j| (i, j)))
                        .collect(),
                    Adjust::HolmSidak,
                )
            }
            Plan::Dunnett(c) => {
                pn.push("For repeated measures, comparisons with the control are paired t tests with Holm–Šidák adjustment.".into());
                (
                    (0..k).filter(|&i| i != c).map(|i| (i, c)).collect(),
                    Adjust::HolmSidak,
                )
            }
            Plan::Pairwise(pr, a) => (
                pr.unwrap_or_else(|| {
                    (0..k)
                        .flat_map(|i| (i + 1..k).map(move |j| (i, j)))
                        .collect()
                }),
                a,
            ),
        };
        let (sec, rows, br) = rm_pairwise(&mx.m, &pairs, adj, &names)?;
        let title = format!("Paired multiple comparisons ({})", adjust_name(adj));
        s.sections.push(sec);
        s.summary.push(family_sentence(
            &title,
            &rows,
            "mean difference",
            0.95,
            style,
        ));
        for (i, j, p) in br {
            add_bracket(&mut ag, keys[i], keys[j], p);
        }
        prune_brackets(&mut ag);
        mnote = format!(", paired t tests with {} adjustment", adjust_name(adj));
        for n in pn {
            s.checklist.push(n);
        }
    }
    s.methods = methods(&format!(
        "Repeated-measures one-way ANOVA ({} subjects, {k} conditions){}{mnote}",
        r.n_subjects,
        if spec.gg_correction {
            " with the Greenhouse–Geisser correction"
        } else {
            ", no sphericity correction"
        }
    ));
    // Checks.
    let mut nv = Vec::new();
    if mx.dropped > 0 {
        nv.push(format!(
            "{} row{} with a missing condition left out (listwise deletion; mixed models are deferred).",
            mx.dropped,
            if mx.dropped == 1 { "" } else { "s" }
        ));
    }
    s.assumptions.push(checks::values_used(&nv, mx.dropped > 0));
    s.assumptions
        .push(checks::sample_size(&[("Subjects".into(), mx.m.len())], 5));
    if k > 2 {
        let status = if r.mauchly_p < 0.05 {
            CheckStatus::Caution
        } else {
            CheckStatus::Pass
        };
        let detail = format!(
            "Mauchly's W = {}, {}; Greenhouse–Geisser ε = {}. {}",
            st(r.mauchly_w),
            p_phrase(r.mauchly_p, style),
            unit(r.gg_epsilon, style),
            if spec.gg_correction {
                "The reported P uses the Greenhouse–Geisser correction, so it does not rely on sphericity."
            } else if status == CheckStatus::Pass {
                "No evidence against sphericity; Mauchly's test has little power in small samples."
            } else {
                "Sphericity is doubtful, and the uncorrected P is too small when it fails."
            }
        );
        let mut c = AssumptionCheck::new(
            "Sphericity",
            if spec.gg_correction {
                CheckStatus::Pass
            } else {
                status
            },
            detail,
        );
        if !spec.gg_correction && status != CheckStatus::Pass {
            c = c.or_else("Turn on the Greenhouse–Geisser correction (option \"gg_correction\").");
        }
        s.assumptions.push(c);
    } else {
        s.assumptions.push(AssumptionCheck::new(
            "Sphericity",
            CheckStatus::Pass,
            "With two conditions sphericity holds automatically.",
        ));
    }
    let gm: f64 = mx.m.iter().flatten().sum::<f64>() / (mx.m.len() * k) as f64;
    let colm: Vec<f64> = (0..k)
        .map(|j| mx.m.iter().map(|r| r[j]).sum::<f64>() / mx.m.len() as f64)
        .collect();
    let resid: Vec<f64> =
        mx.m.iter()
            .flat_map(|row| {
                let rm = row.iter().sum::<f64>() / k as f64;
                let colm = colm.clone();
                row.iter()
                    .enumerate()
                    .map(move |(j, v)| v - rm - colm[j] + gm)
                    .collect::<Vec<_>>()
            })
            .collect();
    let (c, sec) = checks::normality(
        &[("Residuals".into(), resid)],
        "the residuals",
        "Consider the Friedman test (Analyze → Friedman test), or report both.",
    );
    s.assumptions.push(c);
    s.sections.push(sec);
    s.assumptions.push(independence(
        "Each row must be one subject (or matched set), independent of the other rows.",
    ));
    s.graph_family = Some("paired".into());
    s.graph = Some(ag);
    Ok(s)
}

/// Two-way ANOVA (ordinary, or with one repeated factor).
pub fn two_way(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::TwoWayAnova(spec) = ctx.spec else {
        return Err("not a two-way ANOVA spec".into());
    };
    if let Some(rf) = spec.repeated {
        let mut s = two_way_rm(ctx, rf)?;
        if spec.post_hoc.is_some() {
            s.checklist.push(
                "Multiple comparisons are not offered yet for the two-way design with a repeated factor; compare simple effects with paired t tests if you need them."
                    .into(),
            );
        }
        return Ok(s);
    }
    let style = ctx.p_style;
    let t = ctx.table;
    let c = extract::cells(t)?;
    if c.cells.len() < 2 || c.col_names.len() < 2 {
        return Err("Two-way ANOVA needs at least two rows (levels of the row factor) and two data sets (levels of the column factor) with values.".into());
    }
    let ss = match spec.ss_type {
        SsType::I => StatsSs::I,
        SsType::Ii => StatsSs::II,
        SsType::Iii => StatsSs::III,
    };
    let r = two_way_anova(
        &c.cells,
        TwoWayOptions {
            ss_type: ss,
            interaction: true,
        },
    )
    .map_err(|e| e.to_string())?;
    let ssname = match spec.ss_type {
        SsType::I => "type I (sequential)",
        SsType::Ii => "type II",
        SsType::Iii => "type III",
    };
    let term_name = |n: &str| match n {
        "A" => "Row factor".to_string(),
        "B" => "Column factor".to_string(),
        "A:B" => "Interaction".to_string(),
        other => other.to_string(),
    };
    let mut s = sheet("two_way_anova", "Two-way ANOVA");
    let mut rows: Vec<Vec<ResultCell>> = r
        .terms
        .iter()
        .map(|tm| {
            vec![
                txt(term_name(&tm.name)),
                num(tm.ss),
                num(tm.df),
                num(tm.ms),
                num(tm.f),
                pv(tm.p),
                num(tm.partial_eta_squared),
            ]
        })
        .collect();
    rows.push(vec![
        txt("Residual"),
        num(r.residual.ss),
        num(r.residual.df),
        num(r.residual.ms),
        ResultCell::Empty,
        ResultCell::Empty,
        ResultCell::Empty,
    ]);
    s.sections.push(section(
        format!("ANOVA table ({ssname} sums of squares)"),
        &["Source", "SS", "df", "MS", "F", "P", "Partial η²"],
        rows,
    ));
    // Cell means.
    let mut cols = vec!["Row".to_string()];
    for n in &c.col_names {
        cols.push(format!("{n}: mean"));
        cols.push(format!("{n}: n"));
    }
    let cm: Vec<Vec<ResultCell>> = (0..c.cells.len())
        .map(|i| {
            let mut row = vec![txt(c.row_names[i].clone())];
            for j in 0..c.col_names.len() {
                row.push(num(r.cell_means[i][j]));
                row.push(int(r.cell_n[i][j]));
            }
            row
        })
        .collect();
    s.sections.push(section_owned("Cell means", cols, cm));
    // Sentences: interaction first.
    let find = |n: &str| r.terms.iter().find(|tm| tm.name == n);
    let fsent = |label: &str, tm: &dioptra_stats::anova::AnovaTerm| {
        format!(
            "{label}: F({}, {}) = {}, {}, partial η² = {}",
            df(tm.df),
            df(r.residual.df),
            st(tm.f),
            p_phrase(tm.p, style),
            unit(tm.partial_eta_squared, style)
        )
    };
    if let Some(i) = find("A:B") {
        s.summary.push(format!(
            "{}; {}.",
            fsent("Interaction", i),
            if i.p < 0.05 {
                "the effect of the column factor depends on the row factor, so interpret the main effects with care and look at the simple effects"
            } else {
                "no detectable interaction, so the main effects can be read on their own"
            }
        ));
    } else {
        s.summary.push("Each cell has at most one value, so the interaction cannot be separated from the residual; an additive model was fitted.".into());
    }
    if let Some(a) = find("A") {
        s.summary.push(format!("{}.", fsent("Row factor", a)));
    }
    if let Some(b) = find("B") {
        s.summary.push(format!("{}.", fsent("Column factor", b)));
    }
    let mut mline = format!(
        "Two-way ANOVA with {ssname} sums of squares (sum-to-zero coding), {} rows × {} columns, N = {}",
        c.cells.len(),
        c.col_names.len(),
        r.n
    );
    // Simple-effects post hoc within rows.
    if let Some(mc) = &spec.post_hoc {
        let (sec, comp, title) = simple_effects(&r, &c, mc)?;
        s.sections.push(sec);
        s.summary.push(family_sentence(
            &title,
            &comp,
            "mean difference",
            0.95,
            style,
        ));
        mline.push_str(&format!("; {title}"));
    }
    s.methods = methods(&mline);
    // Checks.
    let mut nv = c.notes.clone();
    if r.empty_cells > 0 {
        nv.push(format!(
            "{} empty cell{}.",
            r.empty_cells,
            if r.empty_cells == 1 { "" } else { "s" }
        ));
    }
    s.assumptions
        .push(checks::values_used(&nv, r.empty_cells > 0));
    let single = c.cells.iter().flatten().filter(|v| v.len() < 2).count();
    s.assumptions.push(if single > 0 {
        AssumptionCheck::new(
            "Replicates in every cell",
            CheckStatus::Caution,
            format!("{single} cell{} fewer than two values; the residual variance rests on the other cells, and with no replicates at all the interaction cannot be tested.", if single == 1 { " has" } else { "s have" }),
        )
    } else {
        AssumptionCheck::new("Replicates in every cell", CheckStatus::Pass, "Every cell has at least two values.")
    });
    if !c.from_summary {
        let resid: Vec<f64> = c
            .cells
            .iter()
            .flatten()
            .flat_map(|v| {
                let (mu, _) = mean_sd(v);
                v.iter().map(move |x| x - mu).collect::<Vec<_>>()
            })
            .collect();
        let (ck, sec) = checks::normality(
            &[("Residuals".into(), resid)],
            "the residuals",
            "Consider transforming the data (e.g. logarithms); a rank-based two-way analysis is not offered.",
        );
        s.assumptions.push(ck);
        s.sections.push(sec);
        let sets: Vec<(String, Vec<f64>)> = c
            .cells
            .iter()
            .enumerate()
            .flat_map(|(i, row)| {
                let rn = c.row_names[i].clone();
                let cn = c.col_names.clone();
                row.iter()
                    .enumerate()
                    .filter(|(_, v)| v.len() >= 2)
                    .map(move |(j, v)| (format!("{rn}, {}", cn[j]), v.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        if sets.len() >= 2 {
            if let Some((ck, sec)) = checks::equal_variances(
                &sets,
                false,
                "Consider transforming the data (logarithms often equalise variances), and report the cell SDs.",
            ) {
                s.assumptions.push(ck);
                s.sections.push(sec);
            }
        }
    } else {
        s.assumptions.push(AssumptionCheck::new(
            "Gaussian distribution of the residuals",
            CheckStatus::NotChecked,
            "Some cells were entered as mean, SD and N, so the residuals cannot be examined.",
        ));
    }
    s.assumptions.push(independence(
        "Every value must be independent; if the same subjects appear in several cells, set the repeated factor.",
    ));
    let all: Vec<Option<usize>> = (0..t.datasets.len()).map(Some).collect();
    s.graph_family = Some("column".into());
    s.graph = Some(graph_for("column", &all));
    Ok(s)
}

/// Compare cell means within each row (Prism's "compare each cell mean with the other
/// cell means in that row") on the two-way ANOVA's residual variance.
fn simple_effects(
    r: &dioptra_stats::anova::TwoWayAnova,
    c: &extract::Cells,
    mc: &MultipleComparisons,
) -> Result<(Section, Vec<CompRow>, String), String> {
    let nb = c.col_names.len();
    let ms = r.residual.ms;
    let dfr = r.residual.df;
    let in_range = |d: usize| -> Result<usize, String> {
        if d < nb {
            Ok(d)
        } else {
            Err(format!(
                "Data set {} does not exist.",
                crate::util::letters(d)
            ))
        }
    };
    let pairs: Vec<(usize, usize)> = match &mc.comparisons {
        Comparisons::AllPairs => (0..nb)
            .flat_map(|i| (i + 1..nb).map(move |j| (i, j)))
            .collect(),
        Comparisons::VsControl { control } => {
            let c0 = in_range(*control)?;
            (0..nb).filter(|&i| i != c0).map(|i| (i, c0)).collect()
        }
        Comparisons::Selected { pairs } => pairs
            .iter()
            .map(|(a, b)| Ok((in_range(*a)?, in_range(*b)?)))
            .collect::<Result<Vec<_>, String>>()?,
    };
    let control = match (&mc.method, &mc.comparisons) {
        (McMethod::Dunnett { control }, _) => Some(in_range(*control)?),
        _ => None,
    };
    let pairs = match control {
        Some(c0) => (0..nb).filter(|&i| i != c0).map(|i| (i, c0)).collect(),
        None => pairs,
    };
    let tukey = mc.method == McMethod::Tukey && matches!(mc.comparisons, Comparisons::AllPairs);
    let adj = if tukey {
        Adjust::None
    } else {
        adjust_of(mc.method)
    };
    let sr = if tukey {
        StudentizedRange::new(nb, dfr).ok()
    } else {
        None
    };
    let qcrit = sr.as_ref().map(|d| d.quantile_pq(0.95, 0.05));
    struct Raw {
        label: String,
        est: f64,
        se: f64,
        stat: f64,
        p_raw: f64,
        p_tukey: Option<f64>,
    }
    let mut raw = Vec::new();
    for (i, rn) in c.row_names.iter().enumerate() {
        for &(a, b) in &pairs {
            let (na, nbb) = (r.cell_n[i][a], r.cell_n[i][b]);
            if na == 0 || nbb == 0 {
                continue;
            }
            let est = r.cell_means[i][a] - r.cell_means[i][b];
            let se = (ms * (1.0 / na as f64 + 1.0 / nbb as f64)).sqrt();
            let tv = est / se;
            let p_tukey = sr
                .as_ref()
                .map(|d| d.sf(tv.abs() * std::f64::consts::SQRT_2));
            raw.push(Raw {
                label: format!("{rn}: {} vs. {}", c.col_names[a], c.col_names[b]),
                est,
                se,
                stat: if tukey {
                    tv.abs() * std::f64::consts::SQRT_2
                } else {
                    tv
                },
                p_raw: p_two_sided_t(tv, dfr),
                p_tukey,
            });
        }
    }
    let raw_p: Vec<f64> = raw.iter().map(|x| x.p_raw).collect();
    let adjp: Vec<f64> = if tukey {
        raw.iter().map(|x| x.p_tukey.unwrap_or(f64::NAN)).collect()
    } else {
        p_adjust(&raw_p, adj)
    };
    let m = raw.len().max(1) as f64;
    let ci_t = if tukey {
        qcrit.map(|q| q / std::f64::consts::SQRT_2)
    } else {
        match adj {
            Adjust::Bonferroni => Some(t_critical_two_sided(0.05 / m, dfr)),
            Adjust::Sidak => Some(t_critical_two_sided(1.0 - 0.95f64.powf(1.0 / m), dfr)),
            _ => None,
        }
    };
    let mut rows = Vec::new();
    let mut comp = Vec::new();
    for (x, p) in raw.iter().zip(&adjp) {
        let ci = ci_t.map(|tc| (x.est - tc * x.se, x.est + tc * x.se));
        let sig = *p < 0.05;
        rows.push(vec![
            txt(x.label.clone()),
            num(x.est),
            ci.map_or(ResultCell::Empty, |c| num(c.0)),
            ci.map_or(ResultCell::Empty, |c| num(c.1)),
            num(x.stat),
            pv(x.p_raw),
            pv(*p),
            yes_no(sig),
            txt(crate::results::stars(*p)),
        ]);
        comp.push((x.label.clone(), x.est, ci, *p, sig));
    }
    let title = if tukey {
        "Tukey's multiple comparisons within each row".to_string()
    } else {
        format!(
            "Multiple comparisons within each row ({})",
            adjust_name(adj)
        )
    };
    Ok((
        section(
            &title,
            &[
                "Comparison",
                "Mean diff.",
                "Lower 95% CI",
                "Upper 95% CI",
                if tukey { "q" } else { "t" },
                "P (unadjusted)",
                "Adjusted P",
                "Significant (α = 0.05)?",
                "Summary",
            ],
            rows,
        ),
        comp,
        title,
    ))
}

/// Two-way ANOVA with one repeated factor (split-plot): subjects nested in the levels of
/// the between factor, measured at every level of the within factor. `Rows`: each row's
/// subcolumn is one subject measured in every data set (data sets repeated); `Columns`:
/// each subcolumn of a data set is one subject measured in every row (rows repeated).
fn two_way_rm(ctx: &AnalysisContext<'_>, rf: RepeatedFactor) -> Result<ResultsSheet, String> {
    let style = ctx.p_style;
    let t: &Table = ctx.table;
    if t.datasets.iter().any(|d| extract::is_summary(d.format)) {
        return Err("Repeated-measures two-way ANOVA needs the individual values; it is not possible from summary data.".into());
    }
    let rows: Vec<usize> = (0..t.rows)
        .filter(|&r| {
            t.datasets
                .iter()
                .any(|d| d.subcolumns.iter().any(|c| c.usable_at(r).is_finite()))
        })
        .collect();
    let nsub = t
        .datasets
        .iter()
        .map(|d| d.subcolumns.len())
        .max()
        .unwrap_or(0);
    let (between_names, within_names): (Vec<String>, Vec<String>) = match rf {
        RepeatedFactor::Rows => (
            rows.iter().map(|&r| extract::row_name(t, r)).collect(),
            t.datasets.iter().map(|d| d.name.clone()).collect(),
        ),
        RepeatedFactor::Columns => (
            t.datasets.iter().map(|d| d.name.clone()).collect(),
            rows.iter().map(|&r| extract::row_name(t, r)).collect(),
        ),
    };
    let (ng, nw) = (between_names.len(), within_names.len());
    if ng < 1 || nw < 2 {
        return Err("The repeated factor needs at least two levels.".into());
    }
    let value = |gi: usize, wi: usize, k: usize| -> f64 {
        let (r, d) = match rf {
            RepeatedFactor::Rows => (rows[gi], wi),
            RepeatedFactor::Columns => (rows[wi], gi),
        };
        t.datasets[d]
            .subcolumns
            .get(k)
            .map_or(f64::NAN, |c| c.usable_at(r))
    };
    // y[g] = subjects (complete) x within levels.
    let mut y: Vec<Vec<Vec<f64>>> = vec![Vec::new(); ng];
    let mut dropped = 0;
    for (gi, yg) in y.iter_mut().enumerate() {
        for k in 0..nsub {
            let v: Vec<f64> = (0..nw).map(|wi| value(gi, wi, k)).collect();
            if v.iter().all(|x| x.is_finite()) {
                yg.push(v);
            } else if v.iter().any(|x| x.is_finite()) {
                dropped += 1;
            }
        }
    }
    let ns: usize = y.iter().map(|g| g.len()).sum();
    let groups_used = y.iter().filter(|g| !g.is_empty()).count();
    if ns < 2 || groups_used < 1 {
        return Err("Too few complete subjects for a repeated-measures two-way ANOVA.".into());
    }
    let wf = nw as f64;
    let all: Vec<f64> = y.iter().flatten().flatten().copied().collect();
    let gm = all.iter().sum::<f64>() / all.len() as f64;
    let ss_total: f64 = all.iter().map(|v| (v - gm).powi(2)).sum();
    let ss_subj: f64 = y
        .iter()
        .flatten()
        .map(|s| {
            let m = s.iter().sum::<f64>() / wf;
            wf * (m - gm).powi(2)
        })
        .sum();
    let gmeans: Vec<f64> = y
        .iter()
        .map(|g| {
            if g.is_empty() {
                f64::NAN
            } else {
                g.iter().flatten().sum::<f64>() / (g.len() as f64 * wf)
            }
        })
        .collect();
    let ss_g: f64 = y
        .iter()
        .zip(&gmeans)
        .filter(|(g, _)| !g.is_empty())
        .map(|(g, m)| wf * g.len() as f64 * (m - gm).powi(2))
        .sum();
    let wmeans: Vec<f64> = (0..nw)
        .map(|w| y.iter().flatten().map(|s| s[w]).sum::<f64>() / ns as f64)
        .collect();
    let ss_w: f64 = wmeans.iter().map(|m| ns as f64 * (m - gm).powi(2)).sum();
    let cmeans: Vec<Vec<f64>> = y
        .iter()
        .map(|g| {
            (0..nw)
                .map(|w| {
                    if g.is_empty() {
                        f64::NAN
                    } else {
                        g.iter().map(|s| s[w]).sum::<f64>() / g.len() as f64
                    }
                })
                .collect()
        })
        .collect();
    let ss_cells: f64 = y
        .iter()
        .zip(&cmeans)
        .filter(|(g, _)| !g.is_empty())
        .map(|(g, cm)| {
            cm.iter()
                .map(|m| g.len() as f64 * (m - gm).powi(2))
                .sum::<f64>()
        })
        .sum();
    let ss_gw = ss_cells - ss_g - ss_w;
    let ss_subj_g = ss_subj - ss_g;
    let ss_err = ss_total - ss_subj - ss_w - ss_gw;
    let gq = groups_used as f64;
    let df_g = gq - 1.0;
    let df_sg = ns as f64 - gq;
    let df_w = wf - 1.0;
    let df_gw = df_g * df_w;
    let df_err = df_sg * df_w;
    let ms = |ss: f64, d: f64| if d > 0.0 { ss / d } else { f64::NAN };
    let (ms_g, ms_sg, ms_w, ms_gw, ms_err) = (
        ms(ss_g, df_g),
        ms(ss_subj_g, df_sg),
        ms(ss_w, df_w),
        ms(ss_gw, df_gw),
        ms(ss_err, df_err),
    );
    let fp = |num_ms: f64, d1: f64, den_ms: f64, d2: f64| -> (f64, f64) {
        if d1 > 0.0 && d2 > 0.0 && den_ms > 0.0 {
            let f = num_ms / den_ms;
            (f, p_upper_f(f, d1, d2))
        } else {
            (f64::NAN, f64::NAN)
        }
    };
    let (f_g, p_g) = fp(ms_g, df_g, ms_sg, df_sg);
    let (f_w, p_w) = fp(ms_w, df_w, ms_err, df_err);
    let (f_gw, p_gw) = fp(ms_gw, df_gw, ms_err, df_err);
    let (bl, wl) = match rf {
        RepeatedFactor::Rows => ("Row factor (between subjects)", "Column factor (repeated)"),
        RepeatedFactor::Columns => ("Column factor (between subjects)", "Row factor (repeated)"),
    };
    let mut s = sheet("two_way_anova", "Two-way repeated-measures ANOVA");
    let pe = |a: f64, b: f64| a / (a + b);
    s.sections.push(section(
        "ANOVA table (split-plot)",
        &["Source", "SS", "df", "MS", "F", "P", "Partial η²"],
        vec![
            vec![
                txt("Interaction"),
                num(ss_gw),
                num(df_gw),
                num(ms_gw),
                num(f_gw),
                pv(p_gw),
                num(pe(ss_gw, ss_err)),
            ],
            vec![
                txt(wl),
                num(ss_w),
                num(df_w),
                num(ms_w),
                num(f_w),
                pv(p_w),
                num(pe(ss_w, ss_err)),
            ],
            vec![
                txt(bl),
                num(ss_g),
                num(df_g),
                num(ms_g),
                num(f_g),
                pv(p_g),
                num(pe(ss_g, ss_subj_g)),
            ],
            vec![
                txt("Subjects (within groups)"),
                num(ss_subj_g),
                num(df_sg),
                num(ms_sg),
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
            ],
            vec![
                txt("Residual"),
                num(ss_err),
                num(df_err),
                num(ms_err),
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
            ],
            vec![
                txt("Total"),
                num(ss_total),
                num(all.len() as f64 - 1.0),
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
            ],
        ],
    ));
    let mut cols = vec!["Between level".to_string(), "Subjects".to_string()];
    cols.extend(within_names.iter().map(|w| format!("{w}: mean")));
    s.sections.push(section_owned(
        "Cell means",
        cols,
        (0..ng)
            .map(|gi| {
                let mut row = vec![txt(between_names[gi].clone()), int(y[gi].len())];
                row.extend(cmeans[gi].iter().map(|m| num(*m)));
                row
            })
            .collect(),
    ));
    let fs = |label: &str, f: f64, d1: f64, d2: f64, p: f64| {
        format!(
            "{label}: F({}, {}) = {}, {}",
            df(d1),
            df(d2),
            st(f),
            p_phrase(p, style)
        )
    };
    s.summary.push(format!(
        "{}; {}.",
        fs("Interaction", f_gw, df_gw, df_err, p_gw),
        if p_gw < 0.05 {
            "the effect of the repeated factor differs between groups"
        } else {
            "no detectable interaction"
        }
    ));
    s.summary
        .push(format!("{}.", fs(wl, f_w, df_w, df_err, p_w)));
    s.summary
        .push(format!("{}.", fs(bl, f_g, df_g, df_sg, p_g)));
    let balanced = y
        .iter()
        .filter(|g| !g.is_empty())
        .map(|g| g.len())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        <= 1;
    s.methods = methods(&format!(
        "Two-way ANOVA with repeated measures on one factor (split-plot), {ns} subjects in {groups_used} groups × {nw} repeated levels, no sphericity correction{}",
        if balanced { "" } else { ", unequal group sizes (sequential sums of squares)" }
    ));
    let mut nv = Vec::new();
    if dropped > 0 {
        nv.push(format!(
            "{dropped} subject{} with a missing repeated value left out (listwise deletion; mixed-effects models are deferred).",
            if dropped == 1 { "" } else { "s" }
        ));
    }
    s.assumptions.push(checks::values_used(&nv, dropped > 0));
    s.assumptions.push(AssumptionCheck::new(
        "Sphericity",
        if nw > 2 { CheckStatus::NotChecked } else { CheckStatus::Pass },
        if nw > 2 {
            "Not tested for the two-way repeated design, and no correction is applied; with more than two repeated levels the P values for the repeated factor and the interaction can be too small if sphericity fails."
        } else {
            "With two repeated levels sphericity holds automatically."
        },
    ));
    if !balanced {
        s.assumptions.push(AssumptionCheck::new(
            "Balanced groups",
            CheckStatus::Caution,
            "The groups have different numbers of subjects; the sums of squares are sequential, so the main effects depend slightly on the order of the terms.",
        ));
    }
    let resid: Vec<f64> = y
        .iter()
        .enumerate()
        .flat_map(|(gi, g)| {
            let cm = cmeans[gi].clone();
            let gmn = gmeans[gi];
            g.iter()
                .flat_map(move |sbj| {
                    let sm = sbj.iter().sum::<f64>() / sbj.len() as f64;
                    let cm = cm.clone();
                    sbj.iter()
                        .enumerate()
                        .map(move |(w, v)| v - cm[w] - (sm - gmn))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let (ck, sec) = checks::normality(
        &[("Residuals".into(), resid)],
        "the residuals",
        "Consider transforming the data (e.g. logarithms).",
    );
    s.assumptions.push(ck);
    s.sections.push(sec);
    s.assumptions
        .push(independence("Subjects must be independent of each other."));
    let all_ds: Vec<Option<usize>> = (0..t.datasets.len()).map(Some).collect();
    s.graph_family = Some("column".into());
    s.graph = Some(graph_for("column", &all_ds));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("G{i}")).collect()
    }

    #[test]
    fn plans_follow_the_request() {
        let keys = vec![Some(0), Some(2), Some(3)];
        let mc = |m: McMethod, c: Comparisons| MultipleComparisons {
            method: m,
            comparisons: c,
        };
        let (p, _) = plan(
            &mc(McMethod::Tukey, Comparisons::AllPairs),
            &keys,
            &names(3),
        )
        .unwrap();
        assert_eq!(p, Plan::Tukey);
        // Data set indices map to group positions; Tukey with a control becomes Dunnett.
        let (p, notes) = plan(
            &mc(McMethod::Tukey, Comparisons::VsControl { control: 2 }),
            &keys,
            &names(3),
        )
        .unwrap();
        assert_eq!(p, Plan::Dunnett(1));
        assert!(!notes.is_empty());
        let (p, _) = plan(
            &mc(
                McMethod::Tukey,
                Comparisons::Selected {
                    pairs: vec![(0, 3)],
                },
            ),
            &keys,
            &names(3),
        )
        .unwrap();
        assert_eq!(p, Plan::Pairwise(Some(vec![(0, 2)]), Adjust::HolmSidak));
        let (p, _) = plan(
            &mc(McMethod::BenjaminiHochberg, Comparisons::AllPairs),
            &keys,
            &names(3),
        )
        .unwrap();
        assert_eq!(p, Plan::Pairwise(None, Adjust::BenjaminiHochberg));
        let e = plan(
            &mc(McMethod::Dunnett { control: 1 }, Comparisons::AllPairs),
            &keys,
            &names(3),
        )
        .unwrap_err();
        assert!(e.contains("Data set B"), "{e}");
    }
}
