//! Correlation: Pearson r (Fisher-z CI), Spearman ρ (Bonett–Wright CI), Kendall τ-b; a
//! matrix with Holm-adjusted p values when more than two variables are chosen.

use dioptra_stats::correlation::{kendall, pearson, spearman};
use dioptra_stats::posthoc::{p_adjust, Adjust};

use super::checks;
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec, CorrelationMethod};
use crate::api::TableKind;
use crate::graph::{GraphView, Series};
use crate::results::{CheckStatus, ResultCell};

struct Pair {
    r: f64,
    ci: Option<(f64, f64)>,
    p: f64,
    n: usize,
    exact: bool,
}

fn corr(x: &[f64], y: &[f64], m: CorrelationMethod, level: f64) -> Result<Pair, String> {
    match m {
        CorrelationMethod::Pearson => {
            let c = pearson(x, y, level).map_err(|e| e.to_string())?;
            Ok(Pair {
                r: c.estimate,
                ci: c.ci,
                p: c.p_two_sided,
                n: c.n,
                exact: false,
            })
        }
        CorrelationMethod::Spearman => {
            let c = spearman(x, y, level).map_err(|e| e.to_string())?;
            Ok(Pair {
                r: c.rho,
                ci: c.ci,
                p: c.p_exact_two_sided.unwrap_or(c.p_t_two_sided),
                n: c.n,
                exact: c.p_exact_two_sided.is_some(),
            })
        }
        CorrelationMethod::Kendall => {
            let c = kendall(x, y).map_err(|e| e.to_string())?;
            Ok(Pair {
                r: c.tau_b,
                ci: None,
                p: c.p_auto,
                n: c.n,
                exact: c.p_exact.is_some() && !c.has_ties,
            })
        }
    }
}

fn symbol(m: CorrelationMethod) -> &'static str {
    match m {
        CorrelationMethod::Pearson => "r",
        CorrelationMethod::Spearman => "rₛ",
        CorrelationMethod::Kendall => "τb",
    }
}

fn mname(m: CorrelationMethod) -> &'static str {
    match m {
        CorrelationMethod::Pearson => "Pearson correlation (Fisher z CI)",
        CorrelationMethod::Spearman => "Spearman rank correlation (Bonett–Wright CI)",
        CorrelationMethod::Kendall => "Kendall's τ-b",
    }
}

/// Correlation between two or more variables.
pub fn correlation(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Correlation(spec) = ctx.spec else {
        return Err("not a correlation spec".into());
    };
    check_level(spec.ci_level)?;
    let style = ctx.p_style;
    let t = ctx.table;
    let mut keys = extract::pick(t, &spec.columns, None)?;
    if spec.columns.is_empty() && t.kind == TableKind::Xy {
        keys.insert(0, None);
    }
    if keys.len() < 2 {
        return Err(
            "A correlation needs two variables (columns) with values in the same rows.".into(),
        );
    }
    let cols = extract::aligned(t, &keys)?;
    let m = spec.method;
    let sy = symbol(m);
    let lvl = pct(spec.ci_level);
    let mut s = sheet("correlation", "Correlation");
    s.methods = methods(&format!(
        "{}, two-sided, {lvl} CI, pairwise deletion of missing values{}",
        mname(m),
        if keys.len() > 2 {
            ", p values Holm-adjusted over all pairs"
        } else {
            ""
        }
    ));
    let mut pairs = Vec::new();
    for i in 0..cols.len() {
        for j in i + 1..cols.len() {
            let p = corr(&cols[i].1, &cols[j].1, m, spec.ci_level)
                .map_err(|e| format!("{} and {}: {e}", cols[i].0, cols[j].0))?;
            pairs.push((i, j, p));
        }
    }
    let raw: Vec<f64> = pairs.iter().map(|p| p.2.p).collect();
    let adj = if pairs.len() > 1 {
        p_adjust(&raw, Adjust::Holm)
    } else {
        raw.clone()
    };
    let mut rows = Vec::new();
    for (k, (i, j, p)) in pairs.iter().enumerate() {
        let mut row = vec![
            txt(format!("{} vs. {}", cols[*i].0, cols[*j].0)),
            int(p.n),
            num(p.r),
            p.ci.map_or(ResultCell::Empty, |c| num(c.0)),
            p.ci.map_or(ResultCell::Empty, |c| num(c.1)),
            num(p.r * p.r),
            pv(p.p),
        ];
        if pairs.len() > 1 {
            row.push(pv(adj[k]));
        }
        rows.push(row);
    }
    let mut colnames = vec![
        "Pair".to_string(),
        "n".into(),
        sy.to_string(),
        format!("Lower {lvl} CI"),
        format!("Upper {lvl} CI"),
        format!("{sy}²"),
        "P (two-sided)".into(),
    ];
    if pairs.len() > 1 {
        colnames.push("P (Holm-adjusted)".into());
    }
    s.sections
        .push(section_owned("Correlation", colnames, rows));
    if cols.len() > 2 {
        let k = cols.len();
        let mut mr = vec![vec![f64::NAN; k]; k];
        for (i, j, p) in &pairs {
            mr[*i][*j] = p.r;
            mr[*j][*i] = p.r;
        }
        let mut cn = vec!["Variable".to_string()];
        cn.extend(cols.iter().map(|c| c.0.clone()));
        s.sections.push(section_owned(
            format!("Matrix of {sy}"),
            cn,
            (0..k)
                .map(|i| {
                    let mut row = vec![txt(cols[i].0.clone())];
                    row.extend((0..k).map(|j| if i == j { num(1.0) } else { num(mr[i][j]) }));
                    row
                })
                .collect(),
        ));
    }
    for (k, (i, j, p)) in pairs.iter().enumerate() {
        let ci = p.ci.map_or(String::new(), |c| {
            format!(" ({lvl} CI {} to {})", unit(c.0, style), unit(c.1, style))
        });
        let df_part = if m == CorrelationMethod::Kendall {
            String::new()
        } else {
            format!("({})", p.n.saturating_sub(2))
        };
        s.summary.push(format!(
            "{} and {}: {sy}{df_part} = {}{ci}, {}{}, n = {}.",
            cols[*i].0,
            cols[*j].0,
            unit(p.r, style),
            if pairs.len() > 1 {
                format!("Holm-adjusted {}", p_phrase(adj[k], style))
            } else {
                p_phrase(p.p, style)
            },
            if p.exact { " (exact)" } else { "" },
            p.n
        ));
        if k >= 9 {
            s.summary.push(format!(
                "{} further pairs are in the table.",
                pairs.len() - 10
            ));
            break;
        }
    }
    // Checks.
    let mut notes = Vec::new();
    for (i, j, p) in &pairs {
        let both = cols[*i]
            .1
            .iter()
            .filter(|v| v.is_finite())
            .count()
            .max(cols[*j].1.iter().filter(|v| v.is_finite()).count());
        if p.n < both {
            notes.push(format!(
                "{} vs. {}: {} rows with one value missing left out.",
                cols[*i].0,
                cols[*j].0,
                both - p.n
            ));
        }
    }
    s.assumptions.push(checks::values_used(&notes, false));
    let complete: Vec<(String, Vec<f64>)> = cols
        .iter()
        .map(|(n, v)| {
            (
                n.clone(),
                v.iter().copied().filter(|x| x.is_finite()).collect(),
            )
        })
        .collect();
    s.assumptions.push(checks::sample_size(
        &complete
            .iter()
            .map(|(n, v)| (n.clone(), v.len()))
            .collect::<Vec<_>>(),
        10,
    ));
    if m == CorrelationMethod::Pearson {
        let (c, sec) = checks::normality(
            &complete,
            "each variable",
            "Use Spearman's rank correlation (method \"spearman\"), which assumes neither normality nor linearity.",
        );
        s.assumptions.push(c);
        s.sections.push(sec);
        s.assumptions.push(AssumptionCheck::new(
            "Linear relationship",
            CheckStatus::NotChecked,
            "Pearson's r measures linear association only; look at the scatter plot. A curved relationship can give r near 0.",
        ));
    } else {
        s.assumptions.push(AssumptionCheck::new(
            "Monotonic relationship",
            CheckStatus::NotChecked,
            "Rank correlation measures monotonic association; a U-shaped relationship can give a coefficient near 0.",
        ));
    }
    s.assumptions.push(checks::outlier_hint(&complete));
    s.assumptions.push(independence(
        "Each row must be an independent case (not repeated measurements of one subject).",
    ));
    // Graph.
    let mut ag = graph_for(
        if cols.len() == 2 {
            "scatter"
        } else {
            "correlation_matrix"
        },
        &keys,
    );
    if cols.len() == 2 {
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        for (a, b) in cols[0].1.iter().zip(&cols[1].1) {
            if a.is_finite() && b.is_finite() {
                xs.push(*a);
                ys.push(*b);
            }
        }
        ag.views.push(GraphView {
            family: "scatter".into(),
            replace: true,
            series: vec![Series::xy(&cols[1].0, keys[1].unwrap_or(0), xs, ys)],
            x_label: Some(cols[0].0.clone()),
            y_label: Some(cols[1].0.clone()),
            ..Default::default()
        });
    } else {
        let series = cols
            .iter()
            .enumerate()
            .map(|(i, (n, v))| {
                let (mut xs, mut ys) = (Vec::new(), Vec::new());
                for (r, x) in v.iter().enumerate() {
                    if x.is_finite() {
                        xs.push((r + 1) as f64);
                        ys.push(*x);
                    }
                }
                Series::xy(n, keys[i].unwrap_or(i), xs, ys)
            })
            .collect();
        ag.views.push(GraphView {
            family: "correlation_matrix".into(),
            replace: true,
            series,
            categories: (0..t.rows).map(|r| extract::row_name(t, r)).collect(),
            ..Default::default()
        });
    }
    s.graph_family = Some(ag.family.clone());
    s.graph = Some(ag);
    Ok(s)
}
