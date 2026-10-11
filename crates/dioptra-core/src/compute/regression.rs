//! Simple linear regression of each XY data set, with the runs test, the replicates test,
//! residual checks, and the line with its confidence or prediction band for graphs.

use dioptra_stats::correlation::spearman;
use dioptra_stats::regression::{
    simple_linear_regression, SimpleRegression, SimpleRegressionOptions,
};

use super::checks;
use super::extract;
use super::*;
use crate::analysis::{AnalysisContext, AnalysisSpec, Bands};
use crate::graph::{Curve, GraphView};
use crate::results::{CheckStatus, ResultCell};

/// `n` X values from `lo` to `hi`, evenly or (when `log` and lo > 0) geometrically spaced.
pub(crate) fn grid(lo: f64, hi: f64, n: usize, log: bool) -> Vec<f64> {
    if !(lo.is_finite() && hi.is_finite()) || n < 2 {
        return vec![];
    }
    if hi <= lo {
        return vec![lo, hi];
    }
    (0..n)
        .map(|i| {
            let f = i as f64 / (n - 1) as f64;
            if log && lo > 0.0 {
                (lo.ln() + f * (hi.ln() - lo.ln())).exp()
            } else {
                lo + f * (hi - lo)
            }
        })
        .collect()
}

fn curves(r: &SimpleRegression, name: &str, ds: usize, xs: &[f64], bands: Bands) -> Vec<Curve> {
    let b = r.bands(xs);
    let mut out = Vec::new();
    let mut main = Curve {
        name: format!("{name} (fit)"),
        dataset: Some(ds),
        x: b.x.clone(),
        y: b.fit.clone(),
        ..Default::default()
    };
    if matches!(bands, Bands::Confidence | Bands::Both) {
        main.lo = b.ci_lower.clone();
        main.hi = b.ci_upper.clone();
    }
    out.push(main);
    if matches!(bands, Bands::Prediction | Bands::Both) {
        out.push(Curve {
            name: String::new(),
            dataset: Some(ds),
            x: b.x.clone(),
            y: b.fit.clone(),
            lo: b.pi_lower,
            hi: b.pi_upper,
            step: false,
        });
    }
    out
}

/// Simple linear regression.
pub fn linear_regression(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::LinearRegression(spec) = ctx.spec else {
        return Err("not a linear regression spec".into());
    };
    check_level(spec.ci_level)?;
    let style = ctx.p_style;
    let data = extract::xy(ctx.table, &spec.datasets)?;
    let data: Vec<_> = data.into_iter().filter(|d| !d.x.is_empty()).collect();
    if data.is_empty() {
        return Err("There are no XY points to fit.".into());
    }
    let lvl = pct(spec.ci_level);
    let opts = SimpleRegressionOptions {
        through_origin: spec.through_origin,
        ci_level: spec.ci_level,
        ..Default::default()
    };
    let mut s = sheet("linear_regression", "Simple linear regression");
    s.methods = methods(&format!(
        "Simple linear regression by least squares{}, {lvl} CIs, runs test for deviation from linearity, each data set fitted separately",
        if spec.through_origin { " through the origin" } else { "" }
    ));
    let mut params = Vec::new();
    let mut gof = Vec::new();
    let mut runs = Vec::new();
    let mut lof = Vec::new();
    let mut resid_rows = Vec::new();
    let mut notes = Vec::new();
    let mut resid_sets = Vec::new();
    let mut runs_bad = Vec::new();
    let mut lof_bad = Vec::new();
    let mut spread_bad = Vec::new();
    let mut influential = Vec::new();
    let mut view = GraphView {
        family: "scatter".into(),
        ..Default::default()
    };
    let mut log_curves = Vec::new();
    let mut can_log = true;
    let mut fitted_any = false;
    for d in &data {
        notes.extend(d.notes.iter().cloned());
        if d.x.len() < 3 {
            notes.push(format!("{}: fewer than three points; not fitted.", d.name));
            continue;
        }
        let r = match simple_linear_regression(&d.x, &d.y, opts) {
            Ok(r) => r,
            Err(e) => {
                notes.push(format!("{}: could not be fitted ({e}).", d.name));
                continue;
            }
        };
        fitted_any = true;
        let nm = d.name.as_str();
        params.push(vec![
            txt(nm),
            txt("Slope"),
            num(r.slope),
            num(r.se_slope),
            num(r.ci_slope.0),
            num(r.ci_slope.1),
            num(r.t_slope),
            pv(r.p_slope),
        ]);
        if !spec.through_origin {
            params.push(vec![
                txt(nm),
                txt("Y-intercept"),
                num(r.intercept),
                num(r.se_intercept),
                num(r.ci_intercept.0),
                num(r.ci_intercept.1),
                num(r.t_intercept),
                pv(r.p_intercept),
            ]);
            params.push(vec![
                txt(nm),
                txt("X-intercept"),
                num(r.x_intercept),
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
                ResultCell::Empty,
            ]);
        }
        gof.push(vec![
            txt(nm),
            int(r.n),
            num(if spec.through_origin {
                r.r_squared_centered
            } else {
                r.r_squared
            }),
            num(r.sy_x),
            num(r.ss_residual),
            num(r.f),
            num(1.0),
            num(r.df_residual),
            pv(r.p_f),
            num(r.durbin_watson),
        ]);
        runs.push(vec![
            txt(nm),
            int(r.runs_test.runs),
            num(r.runs_test.expected_runs),
            pv(r.runs_test.p_exact_too_few_runs),
        ]);
        if r.runs_test.p_exact_too_few_runs < 0.05 {
            runs_bad.push(format!(
                "{nm} ({})",
                p_phrase(r.runs_test.p_exact_too_few_runs, style)
            ));
        }
        if let Some(l) = &r.lack_of_fit {
            lof.push(vec![
                txt(nm),
                num(l.ss_lack_of_fit),
                num(l.df_lack_of_fit),
                num(l.ss_pure_error),
                num(l.df_pure_error),
                num(l.f),
                pv(l.p),
            ]);
            if l.p < 0.05 {
                lof_bad.push(format!("{nm} ({})", p_phrase(l.p, style)));
            }
        }
        // Spread against X: Spearman of |residual| with X.
        let absr: Vec<f64> = r.residuals.iter().map(|v| v.abs()).collect();
        if let Ok(sp) = spearman(&d.x, &absr, 0.95) {
            let p = sp.p_exact_two_sided.unwrap_or(sp.p_t_two_sided);
            if p < 0.05 {
                spread_bad.push(format!(
                    "{nm} (Spearman ρ = {}, {})",
                    unit(sp.rho, style),
                    p_phrase(p, style)
                ));
            }
        }
        for (i, sr) in r.standardized_residuals.iter().enumerate() {
            if sr.abs() > 3.0 || r.cooks_distance.get(i).is_some_and(|c| *c > 1.0) {
                influential.push(format!("{nm} at X = {}", g(d.x[i])));
            }
        }
        resid_sets.push((nm.to_string(), r.residuals.clone()));
        if spec.residuals {
            for i in 0..r.residuals.len() {
                resid_rows.push(vec![
                    txt(nm),
                    num(d.x[i]),
                    num(d.y[i]),
                    num(r.fitted[i]),
                    num(r.residuals[i]),
                    num(r.standardized_residuals[i]),
                    num(r.leverage[i]),
                    num(r.cooks_distance[i]),
                ]);
            }
        }
        s.summary.push(format!(
            "{nm}: slope {}{}, R² = {}, F(1, {}) = {}, {}.",
            est_ci(r.slope, r.ci_slope.0, r.ci_slope.1, spec.ci_level),
            if spec.through_origin {
                " (line through the origin)".to_string()
            } else {
                format!(
                    ", Y-intercept {}",
                    est_ci(
                        r.intercept,
                        r.ci_intercept.0,
                        r.ci_intercept.1,
                        spec.ci_level
                    )
                )
            },
            unit(
                if spec.through_origin {
                    r.r_squared_centered
                } else {
                    r.r_squared
                },
                style
            ),
            df(r.df_residual),
            st(r.f),
            p_phrase(r.p_f, style)
        ));
        let (lo, hi) = d.x.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, x| {
            (a.0.min(*x), a.1.max(*x))
        });
        view.curves.extend(curves(
            &r,
            nm,
            d.dataset,
            &grid(lo, hi, 200, false),
            spec.bands,
        ));
        if lo > 0.0 {
            log_curves.extend(curves(
                &r,
                nm,
                d.dataset,
                &grid(lo, hi, 200, true),
                spec.bands,
            ));
        } else {
            can_log = false;
        }
    }
    if !fitted_any {
        return Err(format!("No data set could be fitted. {}", notes.join(" ")));
    }
    s.sections.push(section_owned(
        "Best-fit values",
        vec![
            "Data set".into(),
            "Parameter".into(),
            "Estimate".into(),
            "SE".into(),
            format!("Lower {lvl} CI"),
            format!("Upper {lvl} CI"),
            "t".into(),
            "P (≠ 0)".into(),
        ],
        params,
    ));
    s.sections.push(section(
        "Goodness of fit",
        &[
            "Data set",
            "n",
            "R²",
            "Sy.x",
            "SS (residual)",
            "F",
            "DFn",
            "DFd",
            "P (slope ≠ 0)",
            "Durbin–Watson",
        ],
        gof,
    ));
    s.sections.push(section(
        "Runs test",
        &["Data set", "Runs", "Expected runs", "P (too few runs)"],
        runs,
    ));
    let has_lof = !lof.is_empty();
    if has_lof {
        s.sections.push(section(
            "Replicates test (lack of fit)",
            &[
                "Data set",
                "SS lack of fit",
                "df",
                "SS pure error",
                "df",
                "F",
                "P",
            ],
            lof,
        ));
    }
    if spec.through_origin {
        s.checklist.push("Through the origin, R² is Prism's centred R² (comparing with a horizontal line at the mean); the uncentred R² of R and statsmodels is larger and not comparable.".into());
    }
    // Checks.
    s.assumptions.push(checks::values_used(
        &notes,
        notes
            .iter()
            .any(|n| n.contains("not fitted") || n.contains("could not")),
    ));
    let nonlinear_alt =
        "Fit a curve instead (Analyze → Nonlinear regression), or transform X or Y.";
    s.assumptions.push(if runs_bad.is_empty() {
        AssumptionCheck::new(
            "Linearity (runs test)",
            CheckStatus::Pass,
            "The residuals alternate above and below the line about as often as chance predicts; no sign of curvature.",
        )
    } else {
        AssumptionCheck::new(
            "Linearity (runs test)",
            CheckStatus::Caution,
            format!(
                "Too few runs of residuals for {}: points lie systematically above and below the line, which suggests a curve.",
                runs_bad.join(", ")
            ),
        )
        .or_else(nonlinear_alt)
    });
    if has_lof {
        s.assumptions.push(if lof_bad.is_empty() {
            AssumptionCheck::new(
                "Replicates test",
                CheckStatus::Pass,
                "The replicates scatter about the line no more than about their own means.",
            )
        } else {
            AssumptionCheck::new(
                "Replicates test",
                CheckStatus::Caution,
                format!(
                    "The line misses the replicate means more than their scatter explains for {}.",
                    lof_bad.join(", ")
                ),
            )
            .or_else(nonlinear_alt)
        });
    }
    let (c, sec) = checks::normality(&resid_sets, "the residuals", "Transform Y (e.g. logarithms) or use a robust nonlinear fit; with large samples the slope's CI is robust to mild non-normality.");
    s.assumptions.push(c);
    s.sections.push(sec);
    s.assumptions.push(if spread_bad.is_empty() {
        AssumptionCheck::new("Equal scatter", CheckStatus::Pass, "The size of the residuals does not change detectably with X.")
    } else {
        AssumptionCheck::new(
            "Equal scatter",
            CheckStatus::Caution,
            format!("The size of the residuals changes with X for {}; least squares weights every point equally, so the CIs may be off.", spread_bad.join(", ")),
        )
        .or_else("Use weighted nonlinear regression (Analyze → Nonlinear regression, straight line, weighting 1/Y²), or transform Y.")
    });
    s.assumptions.push(if influential.is_empty() {
        AssumptionCheck::new("Influential points", CheckStatus::Pass, "No point has a standardized residual beyond ±3 or a Cook's distance above 1.")
    } else {
        AssumptionCheck::new(
            "Influential points",
            CheckStatus::Caution,
            format!("Points with a large standardized residual or Cook's distance: {}. Check them for errors; the line may depend on them.", influential.join(", ")),
        )
    });
    s.assumptions.push(AssumptionCheck::new(
        "X measured without error",
        CheckStatus::NotChecked,
        "Least squares assumes X is known exactly; if both variables carry error, Deming regression fits better.",
    ));
    s.assumptions.push(independence("Each point must be independent; the Durbin–Watson statistic (near 2 when independent) is shown for ordered data."));
    if spec.residuals {
        s.sections.push(section(
            "Residuals",
            &[
                "Data set",
                "X",
                "Y",
                "Fitted",
                "Residual",
                "Standardized residual",
                "Leverage",
                "Cook's distance",
            ],
            resid_rows,
        ));
    }
    if can_log {
        view.curves_log = Some(log_curves);
    }
    let keys: Vec<Option<usize>> = data.iter().map(|d| Some(d.dataset)).collect();
    let mut ag = graph_for("scatter", &keys);
    ag.views.push(view);
    s.graph_family = Some("scatter".into());
    s.graph = Some(ag);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_are_linear_or_geometric() {
        let g = grid(1.0, 100.0, 3, true);
        assert!((g[1] - 10.0).abs() < 1e-12);
        assert_eq!(grid(0.0, 10.0, 3, false), vec![0.0, 5.0, 10.0]);
        // log spacing needs lo > 0
        assert_eq!(grid(0.0, 10.0, 3, true), vec![0.0, 5.0, 10.0]);
        assert_eq!(grid(f64::NAN, 1.0, 3, false), Vec::<f64>::new());
    }
}
