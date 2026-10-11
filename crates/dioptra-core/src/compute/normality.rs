//! Normality tests of each data set: Shapiro–Wilk, D'Agostino–Pearson, Anderson–Darling
//! and Kolmogorov–Smirnov (Lilliefors), as the spec asks.

use dioptra_stats::descriptive::{kurtosis, skewness};
use dioptra_stats::normality::{anderson_darling, dagostino_pearson, lilliefors, shapiro_wilk};

use super::checks::{self, ALPHA};
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec, NormalityTest};
use crate::results::{CheckStatus, ResultCell};

fn tname(t: NormalityTest) -> &'static str {
    match t {
        NormalityTest::ShapiroWilk => "Shapiro–Wilk",
        NormalityTest::DagostinoPearson => "D'Agostino–Pearson omnibus",
        NormalityTest::AndersonDarling => "Anderson–Darling",
        NormalityTest::KolmogorovSmirnov => "Kolmogorov–Smirnov (Lilliefors)",
    }
}

/// Run one test: (statistic name, statistic, p) or why it could not run.
fn run(t: NormalityTest, v: &[f64]) -> Result<(&'static str, f64, f64), String> {
    let e = |e: dioptra_stats::StatsError| e.to_string();
    match t {
        NormalityTest::ShapiroWilk => {
            if v.len() > 5000 {
                return Err("Shapiro–Wilk is defined for at most 5000 values".into());
            }
            shapiro_wilk(v).map(|r| ("W", r.w, r.p)).map_err(e)
        }
        NormalityTest::DagostinoPearson => {
            dagostino_pearson(v).map(|r| ("K²", r.k2, r.p)).map_err(e)
        }
        NormalityTest::AndersonDarling => anderson_darling(v)
            .map(|r| ("A²*", r.a2_star, r.p))
            .map_err(e),
        NormalityTest::KolmogorovSmirnov => lilliefors(v)
            .map(|r| ("D", r.d, r.p_dallal_wilkinson))
            .map_err(e),
    }
}

/// Normality tests.
pub fn normality(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Normality(spec) = ctx.spec else {
        return Err("not a normality spec".into());
    };
    let style = ctx.p_style;
    let gs = extract::groups(ctx.table, &spec.columns, None)?;
    extract::require_raw(&gs, "A normality test")?;
    if gs.is_empty() {
        return Err("There are no numeric data sets to test.".into());
    }
    let tests = if spec.tests.is_empty() {
        vec![NormalityTest::ShapiroWilk, NormalityTest::DagostinoPearson]
    } else {
        spec.tests.clone()
    };
    let mut s = sheet("normality", "Normality tests");
    s.methods = methods(&format!(
        "Normality tests ({}) at α = 0.05",
        tests
            .iter()
            .map(|t| tname(*t))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    let mut rows = Vec::new();
    let mut desc = Vec::new();
    let mut not_run = Vec::new();
    let mut fails = Vec::new();
    for gr in &gs {
        let v = &gr.values;
        let (m, sd) = mean_sd(v);
        desc.push(vec![
            txt(gr.name.clone()),
            int(v.len()),
            num(m),
            num(sd),
            skewness(v).map_or(ResultCell::Empty, num),
            kurtosis(v).map_or(ResultCell::Empty, num),
        ]);
        let mut parts = Vec::new();
        for &t in &tests {
            match run(t, v) {
                Ok((stat_name, stat, p)) => {
                    let ok = p >= ALPHA;
                    rows.push(vec![
                        txt(gr.name.clone()),
                        txt(tname(t)),
                        int(v.len()),
                        txt(stat_name),
                        num(stat),
                        pv(p),
                        yes_no(ok),
                    ]);
                    parts.push(format!(
                        "{} {stat_name} = {}, {}",
                        tname(t),
                        st(stat),
                        p_phrase(p, style)
                    ));
                    if !ok {
                        fails.push(format!("{} ({})", gr.name, tname(t)));
                    }
                }
                Err(e) => {
                    rows.push(vec![
                        txt(gr.name.clone()),
                        txt(tname(t)),
                        int(v.len()),
                        ResultCell::Empty,
                        ResultCell::Empty,
                        ResultCell::Empty,
                        txt("not run"),
                    ]);
                    not_run.push(format!("{} ({}: {e})", gr.name, tname(t)));
                }
            }
        }
        if parts.is_empty() {
            s.summary.push(format!(
                "{}: too few values (n = {}) for the chosen tests.",
                gr.name,
                v.len()
            ));
        } else {
            let any_fail = rows
                .iter()
                .rev()
                .take(parts.len())
                .any(|r| r[6] == txt("No"));
            s.summary.push(format!(
                "{}: {}; {}.",
                gr.name,
                parts.join("; "),
                if any_fail {
                    "the data depart detectably from a Gaussian distribution"
                } else {
                    "consistent with a Gaussian distribution (a test cannot prove normality)"
                }
            ));
        }
    }
    s.sections.push(section(
        "Normality tests",
        &[
            "Data set",
            "Test",
            "n",
            "Statistic",
            "Value",
            "P",
            "Passed (α = .05)?",
        ],
        rows,
    ));
    s.sections.push(section(
        "Descriptive",
        &["Data set", "n", "Mean", "SD", "Skewness", "Excess kurtosis"],
        desc,
    ));
    let notes: Vec<String> = gs
        .iter()
        .flat_map(|g| g.notes.iter().cloned())
        .chain(not_run.iter().map(|n| format!("Not run: {n}.")))
        .collect();
    s.assumptions
        .push(checks::values_used(&notes, !not_run.is_empty()));
    let small: Vec<String> = gs
        .iter()
        .filter(|g| g.values.len() < 8)
        .map(|g| format!("{} (n = {})", g.name, g.values.len()))
        .collect();
    let large: Vec<String> = gs
        .iter()
        .filter(|g| g.values.len() > 500)
        .map(|g| g.name.clone())
        .collect();
    s.assumptions.push(if !small.is_empty() {
        AssumptionCheck::new(
            "Power of the tests",
            CheckStatus::Caution,
            format!("{}: with fewer than 8 values a normality test rarely detects even a clearly skewed distribution, so passing means little.", small.join(", ")),
        )
    } else if !large.is_empty() {
        AssumptionCheck::new(
            "Power of the tests",
            CheckStatus::Caution,
            format!("{}: with hundreds of values a normality test flags departures too small to matter for t tests or ANOVA; look at the QQ plot.", large.join(", ")),
        )
    } else {
        AssumptionCheck::new("Power of the tests", CheckStatus::Pass, "Sample sizes are in the range where these tests are informative.")
    });
    s.assumptions.push(if fails.is_empty() {
        AssumptionCheck::new(
            "What the tests mean",
            CheckStatus::Pass,
            "No test rejects a Gaussian distribution. That is not proof of normality, only absence of evidence against it.",
        )
    } else {
        AssumptionCheck::new(
            "What the tests mean",
            CheckStatus::Caution,
            format!("Rejected for {}. A parametric test may still be fine for mild departures and moderate samples; a QQ plot shows how the data depart.", fails.join(", ")),
        )
        .or_else("Use a rank-based test (Mann–Whitney, Kruskal–Wallis, Wilcoxon), or transform the data (e.g. logarithms for right-skewed values).")
    });
    s.assumptions.push(independence(
        "Each value must be an independent measurement.",
    ));
    let keys: Vec<Option<usize>> = gs.iter().map(|g| g.dataset).collect();
    s.graph_family = Some("qq".into());
    s.graph = Some(graph_for("qq", &keys));
    Ok(s)
}
