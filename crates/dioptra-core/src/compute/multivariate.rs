//! Principal component analysis of numeric variables: eigenvalues, variance explained,
//! loadings, scores; components chosen by Horn's parallel analysis (or as asked), with
//! Kaiser's rule reported.

use dioptra_stats::multivariate::{parallel_analysis, pca as pca_fit, PcaScaling};

use super::checks;
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec};
use crate::graph::{GraphView, Series};
use crate::results::{CheckStatus, ResultCell};

/// Principal component analysis.
pub fn pca(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Pca(spec) = ctx.spec else {
        return Err("not a PCA spec".into());
    };
    let t = ctx.table;
    let keys = extract::pick(t, &spec.columns, None)?;
    if keys.len() < 2 {
        return Err("PCA needs at least two numeric variables.".into());
    }
    let cols = extract::aligned(t, &keys)?;
    let names: Vec<String> = cols.iter().map(|c| c.0.clone()).collect();
    let mut rows = Vec::new();
    let mut row_ids = Vec::new();
    let mut dropped = 0;
    for r in 0..t.rows {
        let v: Vec<f64> = cols.iter().map(|c| c.1[r]).collect();
        if v.iter().all(|x| x.is_finite()) {
            rows.push(v);
            row_ids.push(r);
        } else if v.iter().any(|x| x.is_finite()) {
            dropped += 1;
        }
    }
    if rows.len() < 3 {
        return Err("PCA needs at least three complete rows.".into());
    }
    let scaling = if spec.standardize {
        PcaScaling::Standardize
    } else {
        PcaScaling::Center
    };
    let f = pca_fit(&rows, scaling).map_err(|e| e.to_string())?;
    // Fewer random data sets for large tables keep the analysis interactive.
    let reps = if rows.len() * names.len() > 20_000 {
        100
    } else {
        500
    };
    let pa = parallel_analysis(&f, reps, 0.95, 20_261_011).ok();
    let k_all = f.eigenvalues.len();
    let retained = match spec.components {
        Some(k) => k.clamp(1, k_all),
        None => pa.as_ref().map_or(f.kaiser_retained, |p| p.retained).max(1),
    };
    let mut s = sheet("pca", "Principal component analysis");
    s.methods = methods(&format!(
        "Principal component analysis of {} variables ({}), {} complete rows; {}",
        names.len(),
        if spec.standardize { "standardized: correlation matrix" } else { "centred: covariance matrix" },
        f.n,
        match spec.components {
            Some(_) => format!("{retained} components kept as specified"),
            None => format!("components kept by Horn's parallel analysis ({reps} random data sets, 95th percentile)"),
        }
    ));
    let ev_rows: Vec<Vec<ResultCell>> = (0..k_all)
        .map(|i| {
            vec![
                txt(format!("PC{}", i + 1)),
                num(f.eigenvalues[i]),
                num(100.0 * f.explained_variance_ratio[i]),
                num(100.0 * f.cumulative_ratio[i]),
                pa.as_ref()
                    .map_or(ResultCell::Empty, |p| num(p.percentile_eigenvalues[i])),
                yes_no(i < retained),
            ]
        })
        .collect();
    s.sections.push(section(
        "Eigenvalues",
        &[
            "Component",
            "Eigenvalue",
            "% of variance",
            "Cumulative %",
            "Parallel analysis (95th percentile)",
            "Retained?",
        ],
        ev_rows,
    ));
    let mut lcols = vec!["Variable".to_string()];
    lcols.extend((0..retained).map(|i| format!("PC{}", i + 1)));
    s.sections.push(section_owned(
        "Loadings",
        lcols.clone(),
        names
            .iter()
            .enumerate()
            .map(|(j, n)| {
                let mut row = vec![txt(n.clone())];
                row.extend((0..retained).map(|i| num(f.loadings[i][j])));
                row
            })
            .collect(),
    ));
    s.sections.push(section_owned(
        "Eigenvectors",
        lcols,
        names
            .iter()
            .enumerate()
            .map(|(j, n)| {
                let mut row = vec![txt(n.clone())];
                row.extend((0..retained).map(|i| num(f.eigenvectors[i][j])));
                row
            })
            .collect(),
    ));
    let mut scols = vec!["Row".to_string()];
    scols.extend((0..retained).map(|i| format!("PC{}", i + 1)));
    s.sections.push(section_owned(
        "Scores",
        scols,
        f.scores
            .iter()
            .zip(&row_ids)
            .map(|(sc, r)| {
                let mut row = vec![txt(extract::row_name(t, *r))];
                row.extend((0..retained).map(|i| num(sc[i])));
                row
            })
            .collect(),
    ));
    let parts: Vec<String> = (0..retained.min(4))
        .map(|i| {
            format!(
                "PC{} {}%",
                i + 1,
                format_sig(100.0 * f.explained_variance_ratio[i], Some(1), 4)
            )
        })
        .collect();
    s.summary.push(format!(
        "The first {} explain{} {}% of the variance ({}).",
        if retained == 1 {
            "component".to_string()
        } else {
            format!("{retained} components")
        },
        if retained == 1 { "s" } else { "" },
        format_sig(100.0 * f.cumulative_ratio[retained - 1], Some(1), 4),
        parts.join(", ")
    ));
    s.summary.push(format!(
        "Parallel analysis retains {}; Kaiser's rule (eigenvalue > {}) retains {}.",
        pa.as_ref()
            .map_or("—".to_string(), |p| p.retained.to_string()),
        if spec.standardize {
            "1"
        } else {
            "the mean eigenvalue"
        },
        f.kaiser_retained
    ));
    // Checks.
    let mut notes = Vec::new();
    if dropped > 0 {
        notes.push(format!(
            "{dropped} row{} with a missing value left out (complete cases only).",
            if dropped == 1 { "" } else { "s" }
        ));
    }
    s.assumptions.push(checks::values_used(&notes, dropped > 0));
    let ratio = f.n as f64 / names.len() as f64;
    s.assumptions.push(if ratio < 5.0 {
        AssumptionCheck::new(
            "Rows per variable",
            CheckStatus::Caution,
            format!("{} rows for {} variables (fewer than 5 per variable): the components and loadings are unstable.", f.n, names.len()),
        )
    } else {
        AssumptionCheck::new("Rows per variable", CheckStatus::Pass, format!("{} rows for {} variables.", f.n, names.len()))
    });
    if !spec.standardize {
        let (lo, hi) = f
            .column_sds
            .iter()
            .fold((f64::INFINITY, 0.0f64), |a, s| (a.0.min(*s), a.1.max(*s)));
        if lo > 0.0 && hi / lo > 10.0 {
            s.assumptions.push(
                AssumptionCheck::new(
                    "Comparable scales",
                    CheckStatus::Caution,
                    format!("Unstandardized PCA with SDs differing {}-fold: the variables with the largest spread dominate the first components.", g(hi / lo)),
                )
                .or_else("Standardize the variables (option \"standardize\").".to_string()),
            );
        }
    }
    s.assumptions.push(AssumptionCheck::new(
        "Linear relationships",
        CheckStatus::NotChecked,
        "PCA summarises linear correlations; curved relationships and outliers distort the components.",
    ));
    s.assumptions
        .push(independence("Each row must be an independent case."));
    // Graph: scores (scatter) and scree (column).
    let mut ag = graph_for("scatter", &keys);
    if retained >= 2 || k_all >= 2 {
        let (xs, ys): (Vec<f64>, Vec<f64>) = f.scores.iter().map(|r| (r[0], r[1])).unzip();
        let mut sr = Series::xy("Scores", 0, xs, ys);
        if !t.row_titles.is_empty() {
            sr.labels = row_ids.iter().map(|r| extract::row_name(t, *r)).collect();
        }
        ag.views.push(GraphView {
            family: "scatter".into(),
            replace: true,
            series: vec![sr],
            x_label: Some(format!(
                "PC1 ({}%)",
                format_sig(100.0 * f.explained_variance_ratio[0], Some(1), 4)
            )),
            y_label: Some(format!(
                "PC2 ({}%)",
                format_sig(100.0 * f.explained_variance_ratio[1], Some(1), 4)
            )),
            ..Default::default()
        });
    }
    ag.views.push(GraphView {
        family: "column".into(),
        replace: true,
        series: vec![Series::xy(
            "Eigenvalue",
            0,
            (1..=k_all).map(|i| i as f64).collect(),
            f.eigenvalues.clone(),
        )],
        categories: (1..=k_all).map(|i| format!("PC{i}")).collect(),
        y_label: Some("Eigenvalue".into()),
        ..Default::default()
    });
    s.graph_family = Some("scatter".into());
    s.graph = Some(ag);
    Ok(s)
}
