//! The statistical engine wired into the document model: one computation per analysis
//! kind, each producing a results sheet with a methods line, assumption checks, APA-style
//! summary sentences, typed sections, a checklist and graph material (DESIGN §4.3, §5).
//!
//! The numerics are `dioptra-stats`; this module extracts clean numbers from tables
//! ([`extract`]), runs the checks a test rests on ([`checks`]), and writes the sheet.

use crate::analysis::AnalysisEngine;
use crate::api::{Id, PValueStyle, TableKind};
use crate::graph::{AnalysisGraph, Annotation, GraphSpec};
use crate::results::{
    p_phrase, stars, AssumptionCheck, ResultCell, ResultTable, ResultsSheet, Section,
};
use crate::util::format_sig;

mod agreement;
mod anova;
pub(crate) mod checks;
mod contingency;
mod correlation;
pub mod extract;
mod multivariate;
mod nonlinear;
pub use nonlinear::{builtin_equation, EQUATION_KEYS};
mod nonparametric;
mod normality;
mod outliers;
mod regression;
mod roc;
mod survival;
mod ttests;

/// Version written into every methods line.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Register every analysis kind with an engine (descriptive statistics are built in).
pub fn register_all(e: &mut AnalysisEngine) {
    e.register("t_test", ttests::t_test);
    e.register("mann_whitney", nonparametric::mann_whitney);
    e.register("wilcoxon", nonparametric::wilcoxon);
    e.register("kruskal_wallis", nonparametric::kruskal_wallis);
    e.register("friedman", nonparametric::friedman);
    e.register("one_way_anova", anova::one_way);
    e.register("repeated_measures_anova", anova::repeated_measures);
    e.register("two_way_anova", anova::two_way);
    e.register("chi_square", contingency::chi_square);
    e.register("fisher_exact", contingency::fisher_exact);
    e.register("correlation", correlation::correlation);
    e.register("linear_regression", regression::linear_regression);
    e.register("nonlinear_regression", nonlinear::nonlinear_regression);
    e.register("normality", normality::normality);
    e.register("outliers", outliers::outliers);
    e.register("survival", survival::survival);
    e.register("roc", roc::roc);
    e.register("bland_altman", agreement::bland_altman);
    e.register("pca", multivariate::pca);
}

/// The family a new graph of a table gets when none is named.
pub fn default_table_family(kind: TableKind) -> &'static str {
    match kind {
        TableKind::Xy => "scatter",
        TableKind::Survival => "kaplan_meier",
        TableKind::Nested => "superplot",
        TableKind::Column
        | TableKind::Grouped
        | TableKind::Contingency
        | TableKind::Parts
        | TableKind::Variables => "column",
    }
}

/// Bracket label in the project's p-value style: asterisks for GP, else the p phrase.
pub fn bracket_text(p: f64, style: PValueStyle) -> String {
    match style {
        PValueStyle::Gp => stars(p).to_string(),
        _ => p_phrase(p, style),
    }
}

fn categorical(family: &str) -> bool {
    matches!(
        family,
        "column" | "bar" | "box" | "violin" | "strip" | "raincloud" | "before_after" | "superplot"
    )
}

/// The spec of a new graph of an analysis: the family named (or the analysis's default
/// when the name is empty or `auto`), the analysed data sets, and significance brackets
/// from the analysis in the project's p-value style, stacked by the plot engine.
pub fn graph_spec_for_analysis(
    source: Id,
    family: &str,
    ag: Option<&AnalysisGraph>,
    style: PValueStyle,
) -> GraphSpec {
    let family = if family.trim().is_empty() || family == "auto" {
        ag.map(|g| g.family.as_str())
            .filter(|f| !f.is_empty())
            .unwrap_or("column")
            .to_string()
    } else {
        family.to_string()
    };
    let mut spec = GraphSpec::new(source, &family);
    if let Some(g) = ag {
        spec.mapping.datasets = g.datasets.clone();
        if categorical(&family) {
            for b in &g.brackets {
                spec.annotations.push(Annotation::SignificanceBracket {
                    from: b.from,
                    to: b.to,
                    text: bracket_text(b.p, style),
                    y: None,
                });
            }
        }
    }
    spec
}

// ---------------------------------------------------------------------------
// Sheet building
// ---------------------------------------------------------------------------

/// An empty sheet for a kind.
pub(crate) fn sheet(kind: &str, title: &str) -> ResultsSheet {
    ResultsSheet {
        title: title.into(),
        analysis_kind: kind.into(),
        ..Default::default()
    }
}

/// A methods line with the software version.
pub(crate) fn methods(text: &str) -> Option<String> {
    Some(format!(
        "{}. Dioptra {VERSION}.",
        text.trim_end_matches('.')
    ))
}

/// A typed section.
pub(crate) fn section(
    title: impl Into<String>,
    columns: &[&str],
    rows: Vec<Vec<ResultCell>>,
) -> Section {
    Section {
        title: title.into(),
        table: ResultTable {
            columns: columns.iter().map(|c| c.to_string()).collect(),
            rows,
        },
    }
}

/// A section with owned column names.
pub(crate) fn section_owned(
    title: impl Into<String>,
    columns: Vec<String>,
    rows: Vec<Vec<ResultCell>>,
) -> Section {
    Section {
        title: title.into(),
        table: ResultTable { columns, rows },
    }
}

/// A two-column (quantity, value) section.
pub(crate) fn kv(title: impl Into<String>, rows: Vec<(String, ResultCell)>) -> Section {
    section(
        title,
        &["Quantity", "Value"],
        rows.into_iter()
            .map(|(k, v)| vec![ResultCell::text(k), v])
            .collect(),
    )
}

/// Number cell.
pub(crate) fn num(v: f64) -> ResultCell {
    ResultCell::num(v)
}

/// Count cell.
pub(crate) fn int(n: usize) -> ResultCell {
    ResultCell::count(n)
}

/// p-value cell.
pub(crate) fn pv(p: f64) -> ResultCell {
    if p.is_finite() {
        ResultCell::PValue { p }
    } else {
        ResultCell::Empty
    }
}

/// Text cell.
pub(crate) fn txt(s: impl Into<String>) -> ResultCell {
    ResultCell::text(s)
}

/// Yes / No cell.
pub(crate) fn yes_no(b: bool) -> ResultCell {
    txt(if b { "Yes" } else { "No" })
}

/// A number cell that shows infinities as text ("Infinity", "undefined").
pub(crate) fn num_or(v: f64, inf: &str) -> ResultCell {
    if v.is_infinite() {
        txt(inf)
    } else {
        num(v)
    }
}

/// Validate a confidence level.
pub(crate) fn check_level(level: f64) -> Result<(), String> {
    if level > 0.0 && level < 1.0 {
        Ok(())
    } else {
        Err(format!(
            "The confidence level must be between 0 and 1 (got {level})."
        ))
    }
}

/// "95%".
pub(crate) fn pct(level: f64) -> String {
    format!("{}%", format_sig(level * 100.0, None, 4))
}

/// Shared decimals for an estimate and its interval: two significant digits of the
/// interval's width.
fn decimals_for(width: f64, v: f64) -> usize {
    let w = if width.is_finite() && width > 0.0 {
        width
    } else if v != 0.0 && v.is_finite() {
        v.abs()
    } else {
        1.0
    };
    (1 - w.log10().floor() as i64).clamp(0, 10) as usize
}

fn fixed(v: f64, d: usize) -> String {
    if v.is_infinite() {
        return if v > 0.0 {
            "infinity".into()
        } else {
            "-infinity".into()
        };
    }
    if v.is_nan() {
        return "undefined".into();
    }
    let s = format!("{v:.d$}");
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

fn tiny_or_huge(vs: &[f64]) -> bool {
    let m = vs
        .iter()
        .filter(|v| v.is_finite())
        .fold(0.0f64, |a, v| a.max(v.abs()));
    m != 0.0 && !(1e-3..1e6).contains(&m)
}

/// A number in a sentence (three significant digits, scientific when tiny or huge).
pub(crate) fn g(v: f64) -> String {
    if v.is_nan() {
        return "undefined".into();
    }
    if v.is_infinite() {
        return fixed(v, 0);
    }
    format_sig(v, None, 3)
}

/// "1.58 (95% CI 0.70 to 2.46)".
pub(crate) fn est_ci(v: f64, lo: f64, hi: f64, level: f64) -> String {
    if !(lo.is_finite() || hi.is_finite()) && lo.is_nan() && hi.is_nan() {
        return g(v);
    }
    let (a, b, c) = if tiny_or_huge(&[v, lo, hi]) {
        (g(v), g(lo), g(hi))
    } else {
        let d = decimals_for(hi - lo, v).max(decimals_for(f64::NAN, v));
        (fixed(v, d), fixed(lo, d), fixed(hi, d))
    };
    format!("{a} ({} CI {b} to {c})", pct(level))
}

/// "1.58, 95% CI 0.70 to 2.46" (for use inside parentheses).
pub(crate) fn est_ci_inline(v: f64, lo: f64, hi: f64, level: f64) -> String {
    est_ci(v, lo, hi, level)
        .replacen(" (", ", ", 1)
        .trim_end_matches(')')
        .to_string()
}

/// A test statistic with two decimals.
pub(crate) fn st(v: f64) -> String {
    if v.abs() >= 1e5 {
        return g(v);
    }
    fixed(v, 2)
}

/// Degrees of freedom: whole numbers as integers, fractional ones with one or two decimals.
pub(crate) fn df(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else if v >= 10.0 {
        fixed(v, 1)
    } else {
        fixed(v, 2)
    }
}

/// A quantity bounded by 1 (r, eta squared, R squared, AUC): two decimals, no leading zero
/// in APA style.
pub(crate) fn unit(v: f64, style: PValueStyle) -> String {
    let a = v.abs();
    let d = if (0.995..1.0).contains(&a) || (a > 0.0 && a < 0.005) {
        3
    } else {
        2
    };
    let s = fixed(v, d);
    if style == PValueStyle::Apa {
        if let Some(r) = s.strip_prefix("0.") {
            return format!(".{r}");
        }
        if let Some(r) = s.strip_prefix("-0.") {
            return format!("-.{r}");
        }
    }
    s
}

/// A sentence summarising a family of comparisons: which differ, and how many do not.
pub(crate) fn family_sentence(
    title: &str,
    rows: &[CompRow],
    what: &str,
    level: f64,
    style: PValueStyle,
) -> String {
    let sig: Vec<String> = rows
        .iter()
        .filter(|r| r.4)
        .map(|(name, est, ci, p, _)| match ci {
            Some((lo, hi)) if lo.is_finite() || hi.is_finite() => format!(
                "{name} ({what} {}, {})",
                est_ci_inline(*est, *lo, *hi, level),
                p_phrase(*p, style)
            ),
            _ => format!("{name} ({what} {}, {})", g(*est), p_phrase(*p, style)),
        })
        .collect();
    let rest = rows.len() - sig.len();
    if rows.is_empty() {
        return format!("{title}: no comparisons.");
    }
    if sig.is_empty() {
        return format!(
            "{title}: none of the {} comparison{} reached adjusted p < .05.",
            rows.len(),
            if rows.len() == 1 { "" } else { "s" }
        );
    }
    let mut s = format!(
        "{title}: {} differ{}",
        sig.join("; "),
        if sig.len() == 1 { "s" } else { "" }
    );
    if sig.len() > 1 {
        s = format!("{title}: these differ: {}", sig.join("; "));
    }
    if rest == 1 {
        s.push_str("; the remaining comparison did not reach adjusted p < .05");
    } else if rest > 1 {
        s.push_str(&format!(
            "; the other {rest} comparisons did not reach adjusted p < .05"
        ));
    }
    s.push('.');
    s
}

/// Assumption check stating that the values are assumed independent.
pub(crate) fn independence(what: &str) -> AssumptionCheck {
    AssumptionCheck::new(
        "Independent observations",
        crate::results::CheckStatus::NotChecked,
        format!("{what} Only the design of the experiment can tell; the data cannot."),
    )
}

/// Mean and SD (n - 1) of finite values.
pub(crate) fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    if v.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let m = v.iter().sum::<f64>() / n;
    let sd = if v.len() > 1 {
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        f64::NAN
    };
    (m, sd)
}

/// Columns of a descriptive row.
pub(crate) const DESC_COLS: [&str; 5] = ["Data set", "n", "Mean", "SD", "SEM"];

/// A descriptive row: name, n, mean, SD, SEM (from raw values or entered summaries).
pub(crate) fn desc_row(gr: &extract::Group) -> Vec<ResultCell> {
    let (n, m, sd) = match gr.summary {
        Some(s) => (s.n, s.mean, s.sd),
        None => {
            let (m, sd) = mean_sd(&gr.values);
            (gr.values.len(), m, sd)
        }
    };
    desc_cells(&gr.name, n, m, sd)
}

/// A descriptive row from numbers.
pub(crate) fn desc_cells(name: &str, n: usize, m: f64, sd: f64) -> Vec<ResultCell> {
    vec![
        txt(name),
        int(n),
        num(m),
        num(sd),
        num(sd / (n as f64).sqrt()),
    ]
}

/// The core direction as the stats crate's.
pub(crate) fn alt(a: crate::analysis::Alternative) -> dioptra_stats::ttest::Alternative {
    match a {
        crate::analysis::Alternative::TwoSided => dioptra_stats::ttest::Alternative::TwoSided,
        crate::analysis::Alternative::Greater => dioptra_stats::ttest::Alternative::Greater,
        crate::analysis::Alternative::Less => dioptra_stats::ttest::Alternative::Less,
    }
}

/// "two-sided" or "one-sided (H1: A > B)".
pub(crate) fn sidedness(a: crate::analysis::Alternative, greater: &str, less: &str) -> String {
    match a {
        crate::analysis::Alternative::TwoSided => "two-sided".into(),
        crate::analysis::Alternative::Greater => format!("one-sided (H1: {greater})"),
        crate::analysis::Alternative::Less => format!("one-sided (H1: {less})"),
    }
}

/// The p phrase for the requested direction: "p = .005" or "one-sided p = .003".
pub(crate) fn p_dir(p: f64, a: crate::analysis::Alternative, style: PValueStyle) -> String {
    match a {
        crate::analysis::Alternative::TwoSided => p_phrase(p, style),
        _ => format!("one-sided {}", p_phrase(p, style)),
    }
}

/// Graph material for the data sets `keys` (X entries dropped), in table order.
pub(crate) fn graph_for(family: &str, keys: &[Option<usize>]) -> AnalysisGraph {
    let mut ds: Vec<usize> = keys.iter().flatten().copied().collect();
    ds.sort_unstable();
    ds.dedup();
    AnalysisGraph {
        family: family.into(),
        datasets: ds,
        ..Default::default()
    }
}

/// Add a bracket between two table data sets (positions follow `ag.datasets`).
pub(crate) fn add_bracket(ag: &mut AnalysisGraph, a: Option<usize>, b: Option<usize>, p: f64) {
    let pos = |d: Option<usize>| d.and_then(|d| ag.datasets.iter().position(|x| *x == d));
    if let (Some(i), Some(j)) = (pos(a), pos(b)) {
        if p.is_finite() {
            ag.brackets.push(crate::graph::Bracket {
                from: i.min(j),
                to: i.max(j),
                p,
            });
        }
    }
}

/// Keep at most six brackets: all when there are six or fewer, else the significant ones.
pub(crate) fn prune_brackets(ag: &mut AnalysisGraph) {
    if ag.brackets.len() > 6 {
        ag.brackets.retain(|b| b.p < 0.05);
    }
}

/// Approximate CI of a standardized mean difference (Hedges & Olkin 1985, normal
/// approximation): `se` of d for sizes n1, n2 (n2 = 0: one-sample or paired d_z).
pub(crate) fn d_ci(d: f64, n1: usize, n2: usize, level: f64) -> (f64, f64) {
    let (a, b) = (n1 as f64, n2 as f64);
    let se = if n2 == 0 {
        (1.0 / a + d * d / (2.0 * a)).sqrt()
    } else {
        ((a + b) / (a * b) + d * d / (2.0 * (a + b))).sqrt()
    };
    let z = dioptra_stats::special::norm_quantile_upper(0.5 * (1.0 - level));
    (d - z * se, d + z * se)
}

/// One comparison as the family sentence needs it: label, estimate, CI, adjusted p,
/// significant.
pub(crate) type CompRow = (String, f64, Option<(f64, f64)>, f64, bool);

/// A multiple-comparisons table from a stats `PostHoc` family. `names` are the group
/// names; `est` names the estimate ("Mean diff.", "Mean rank diff."), `stat` the statistic.
pub(crate) fn posthoc_table(
    title: &str,
    ph: &dioptra_stats::posthoc::PostHoc,
    names: &[String],
    est: &str,
    stat: &str,
) -> (Section, Vec<CompRow>) {
    let lvl = pct(1.0 - ph.alpha);
    let mut rows = Vec::new();
    let mut comp = Vec::new();
    for c in &ph.comparisons {
        let label = format!(
            "{} vs. {}",
            names.get(c.i).map_or("?", |s| s.as_str()),
            names.get(c.j).map_or("?", |s| s.as_str())
        );
        rows.push(vec![
            txt(label.clone()),
            num(c.estimate),
            c.ci.map_or(ResultCell::Empty, |ci| num(ci.0)),
            c.ci.map_or(ResultCell::Empty, |ci| num(ci.1)),
            num(c.statistic),
            pv(c.p_raw),
            pv(c.p_adj),
            yes_no(c.significant),
            txt(stars(c.p_adj)),
        ]);
        comp.push((label, c.estimate, c.ci, c.p_adj, c.significant));
    }
    (
        section_owned(
            title,
            vec![
                "Comparison".into(),
                est.into(),
                format!("Lower {lvl} CI"),
                format!("Upper {lvl} CI"),
                stat.into(),
                "P (unadjusted)".into(),
                "Adjusted P".into(),
                format!("Significant (α = {})?", format_sig(ph.alpha, None, 3)),
                "Summary".into(),
            ],
            rows,
        ),
        comp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_numbers() {
        assert_eq!(
            est_ci(1.58, 0.7001, 2.4599, 0.95),
            "1.6 (95% CI 0.7 to 2.5)"
        );
        assert_eq!(
            est_ci(-0.8651, -1.556, -0.174, 0.95),
            "-0.87 (95% CI -1.56 to -0.17)"
        );
        assert_eq!(
            est_ci_inline(-0.8651, -1.556, -0.174, 0.95),
            "-0.87, 95% CI -1.56 to -0.17"
        );
        assert_eq!(est_ci(3.21, 1.1, 5.3, 0.95), "3.2 (95% CI 1.1 to 5.3)");
        assert_eq!(unit(0.9991, PValueStyle::Apa), ".999");
        assert_eq!(df(9.0), "9");
        assert_eq!(df(17.43), "17.4");
        assert_eq!(unit(0.264, PValueStyle::Apa), ".26");
        assert_eq!(unit(0.264, PValueStyle::Nejm), "0.26");
        assert_eq!(st(4.062), "4.06");
        assert_eq!(bracket_text(0.012, PValueStyle::Gp), "*");
        assert_eq!(bracket_text(0.012, PValueStyle::Apa), "p = .012");
    }
}
