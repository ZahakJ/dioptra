//! Nonlinear regression (curve fitting): each Y data set fitted separately, or globally
//! when a parameter is shared; constraints, weighting, robust fitting and ROUT; asymptotic
//! and profile CIs; derived values (EC50 from LogEC50 and so on); runs and replicates
//! tests; model comparison by the extra-sum-of-squares F test and AICc; curves with bands
//! for graphs.

use dioptra_stats::equations::{self, Equation, XTransform};
use dioptra_stats::nonlinear::{
    self, compare_summaries, CurveFit, Dataset, FitMethod, FitOptions, FitStatus, ParamConstraint,
    ParamStatus, Preferred, Weighting as SW,
};

use super::checks;
use super::extract::{self, XyData};
use super::regression::grid;
use super::*;
use crate::analysis::{
    AnalysisContext, AnalysisSpec, Bands, Constraint, EquationSpec, NonlinearRegressionSpec,
    Weighting,
};
use crate::graph::{Curve, GraphView};
use crate::results::{CheckStatus, ResultCell};

/// The equation a spec names: a snake_case name used by the UI, a Prism name or alias,
/// or a user formula (`Y = …`).
pub(crate) fn equation(spec: &EquationSpec) -> Result<Equation, String> {
    if let Some(f) = spec.formula.as_deref().filter(|f| !f.trim().is_empty()) {
        let rhs = match f.split_once('=') {
            Some((lhs, rhs)) if lhs.trim().eq_ignore_ascii_case("y") => rhs,
            _ => f,
        };
        let mut e = Equation::custom(rhs.trim())
            .map_err(|e| format!("The formula could not be read: {e}"))?;
        if !spec.name.trim().is_empty() {
            e.name = spec.name.clone();
        }
        return Ok(e);
    }
    let n = spec
        .name
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    let e = match n.as_str() {
        "four_parameter_logistic" | "4pl" | "log_agonist_variable_slope" | "dose_response_variable_slope" => {
            equations::log_agonist_variable_slope()
        }
        "three_parameter_logistic" | "3pl" | "log_agonist_vs_response" => equations::log_agonist_vs_response(),
        "log_agonist_normalized_variable_slope" | "normalized_variable_slope" => {
            equations::log_agonist_normalized_variable_slope()
        }
        "log_inhibitor_variable_slope" | "inhibitor_variable_slope" => equations::log_inhibitor_variable_slope(),
        "log_inhibitor_vs_response" | "inhibitor_vs_response" => equations::log_inhibitor_vs_response(),
        "five_parameter_logistic" | "5pl" | "asymmetric_5pl" => equations::asymmetric_5pl(),
        "agonist_vs_response" => equations::agonist_vs_response(),
        "agonist_variable_slope" => equations::agonist_variable_slope(),
        "bell_shaped" => equations::bell_shaped(),
        "log_logistic" => equations::log_logistic(),
        "one_phase_decay" => equations::one_phase_decay(),
        "two_phase_decay" => equations::two_phase_decay(),
        "plateau_then_one_phase_decay" => equations::plateau_then_one_phase_decay(),
        "one_phase_association" => equations::one_phase_association(),
        "two_phase_association" => equations::two_phase_association(),
        "plateau_then_one_phase_association" => equations::plateau_then_one_phase_association(),
        "exponential_growth" => equations::exponential_growth(),
        "michaelis_menten" | "mm" => equations::michaelis_menten(),
        "substrate_inhibition" => equations::substrate_inhibition(),
        "allosteric_sigmoidal" => equations::allosteric_sigmoidal(),
        "competitive_inhibition" => equations::competitive_inhibition(),
        "one_site_specific" | "one_site_specific_binding" => equations::one_site_specific(),
        "specific_binding_hill" | "specific_binding_with_hill_slope" => equations::specific_binding_hill(),
        "one_site_total" | "one_site_total_binding" => equations::one_site_total(),
        "one_site_fit_logic50" | "one_site_fit_log_ic50" => equations::one_site_fit_logic50(),
        "one_site_fit_ki" => equations::one_site_fit_ki(),
        "two_sites_fit_logic50" | "two_sites_fit_log_ic50" => equations::two_sites_fit_logic50(),
        "straight_line" | "line" | "linear" => equations::straight_line(),
        "line_through_origin" => equations::line_through_origin(),
        "quadratic" | "second_order_polynomial" | "polynomial_2" => equations::polynomial(2).map_err(|e| e.to_string())?,
        "cubic" | "third_order_polynomial" | "polynomial_3" => equations::polynomial(3).map_err(|e| e.to_string())?,
        "polynomial_4" => equations::polynomial(4).map_err(|e| e.to_string())?,
        "polynomial_5" => equations::polynomial(5).map_err(|e| e.to_string())?,
        "polynomial_6" => equations::polynomial(6).map_err(|e| e.to_string())?,
        "segmental_linear" => equations::segmental_linear(),
        "gaussian" => equations::gaussian(),
        "lorentzian" => equations::lorentzian(),
        "sine_wave" => equations::sine_wave(),
        "sine_wave_baseline" => equations::sine_wave_baseline(),
        "power" | "power_function" => equations::power(),
        "power_series" => equations::power_series(),
        "boltzmann_sigmoid" | "boltzmann" => equations::boltzmann_sigmoid(),
        "logistic_growth" => equations::logistic_growth(),
        "gompertz_growth" | "gompertz" => equations::gompertz_growth(),
        _ => equations::by_name(&spec.name).ok_or_else(|| {
            format!(
                "There is no built-in equation called \"{}\". Use a name such as four_parameter_logistic, one_phase_decay or michaelis_menten, or write a formula.",
                spec.name
            )
        })?,
    };
    Ok(e)
}

fn stats_weighting(w: Weighting) -> SW {
    match w {
        Weighting::None => SW::None,
        Weighting::RelativeY2 => SW::InverseY2,
        Weighting::PoissonY => SW::InverseY,
        Weighting::InverseSd2 => SW::InverseSD2,
    }
}

fn weight_name(w: Weighting) -> &'static str {
    match w {
        Weighting::None => "no weighting",
        Weighting::RelativeY2 => "weighting by 1/Y²",
        Weighting::PoissonY => "weighting by 1/Y",
        Weighting::InverseSd2 => "weighting by 1/SD² of the replicates",
    }
}

/// Constraints and initial values for an equation, from the spec.
fn constraints(
    eq: &Equation,
    spec: &NonlinearRegressionSpec,
) -> Result<(Option<Vec<ParamConstraint>>, bool), String> {
    let names = eq.param_names();
    let idx = |p: &str| -> Result<usize, String> {
        names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(p))
            .ok_or_else(|| {
                format!(
                    "The equation has no parameter \"{p}\"; its parameters are {}.",
                    names.join(", ")
                )
            })
    };
    let constant: Vec<&str> = eq
        .params
        .iter()
        .filter(|p| p.constant)
        .map(|p| p.name.as_str())
        .collect();
    if spec.constraints.is_empty() && spec.initial.is_empty() && constant.is_empty() {
        return Ok((None, false));
    }
    let mut cs: Vec<ParamConstraint> = eq
        .params
        .iter()
        .map(|p| ParamConstraint {
            lower: p.lower,
            upper: p.upper,
            ..Default::default()
        })
        .collect();
    let mut shared = false;
    for c in &spec.constraints {
        match c {
            Constraint::Fixed { param, value } => cs[idx(param)?].fixed = Some(*value),
            Constraint::Min { param, value } => cs[idx(param)?].lower = *value,
            Constraint::Max { param, value } => cs[idx(param)?].upper = *value,
            Constraint::Shared { param } => {
                cs[idx(param)?].shared = true;
                shared = true;
            }
        }
    }
    for (p, v) in &spec.initial {
        cs[idx(p)?].initial = Some(*v);
    }
    for (i, p) in eq.params.iter().enumerate() {
        if p.constant && cs[i].fixed.is_none() {
            return Err(format!(
                "{} is a constant of each data set (for example a concentration); give it a value with a \"fixed\" constraint.",
                p.name
            ));
        }
    }
    Ok((Some(cs), shared))
}

fn to_dataset(d: &XyData, w: Weighting) -> Result<Dataset, String> {
    if w == Weighting::InverseSd2 {
        let pts: Vec<_> = d
            .means
            .iter()
            .filter(|m| m.2.is_finite() && m.2 > 0.0)
            .collect();
        if pts.len() < d.means.len() {
            return Err(format!(
                "{}: weighting by 1/SD² needs an SD at every X (two or more replicates, or entered SDs).",
                d.name
            ));
        }
        let x: Vec<f64> = pts.iter().map(|m| m.0).collect();
        let y: Vec<f64> = pts.iter().map(|m| m.1).collect();
        let sd: Vec<f64> = pts.iter().map(|m| m.2).collect();
        return Ok(Dataset::new(&x, &y).with_sd(&sd));
    }
    Ok(Dataset::new(&d.x, &d.y))
}

fn status_text(s: FitStatus) -> &'static str {
    match s {
        FitStatus::Converged => "Converged",
        FitStatus::HitConstraint => "Converged; a parameter hit a constraint",
        FitStatus::Ambiguous => "Ambiguous (a parameter is not identifiable)",
        FitStatus::NotConverged => "Not converged",
        FitStatus::Failed => "Failed",
    }
}

fn pstatus(s: ParamStatus) -> &'static str {
    match s {
        ParamStatus::Fitted => "Fitted",
        ParamStatus::Shared => "Shared",
        ParamStatus::Fixed => "Fixed",
        ParamStatus::AtBound => "At bound",
    }
}

/// (name, table data set, index in `fit.datasets`, X and Y of the points fitted)
type SetInfo = (String, usize, usize, Vec<f64>, Vec<f64>);

struct Fitted {
    fit: CurveFit,
    sets: Vec<SetInfo>,
    rout_flags: Vec<Vec<usize>>,
}

fn fit_all(
    eq: &Equation,
    data: &[XyData],
    spec: &NonlinearRegressionSpec,
    opts: &FitOptions,
    global: bool,
) -> Result<(Vec<Fitted>, Vec<String>), String> {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    let mut sets = Vec::new();
    for d in data {
        match to_dataset(d, spec.weighting) {
            Ok(ds) => sets.push((d, ds)),
            Err(e) => notes.push(e),
        }
    }
    if sets.is_empty() {
        return Err(notes.join(" "));
    }
    let rout = |dsets: &[Dataset]| -> Vec<Vec<usize>> {
        if spec.robust && spec.rout_q.is_none() {
            nonlinear::rout_fit_global(eq, dsets, 0.01, opts)
                .map(|r| r.outliers)
                .unwrap_or_default()
        } else {
            vec![]
        }
    };
    if global {
        let dsets: Vec<Dataset> = sets.iter().map(|(_, d)| d.clone()).collect();
        let fit = nonlinear::fit_global(eq, &dsets, opts).map_err(|e| e.to_string())?;
        let flags = rout(&dsets);
        out.push(Fitted {
            sets: sets
                .iter()
                .enumerate()
                .map(|(i, (d, ds))| (d.name.clone(), d.dataset, i, ds.x.clone(), ds.y.clone()))
                .collect(),
            fit,
            rout_flags: flags,
        });
    } else {
        for (d, ds) in &sets {
            let need = eq.n_params() + 1;
            if ds.x.len() < need {
                notes.push(format!(
                    "{}: {} points are too few to fit {} parameters; not fitted.",
                    d.name,
                    ds.x.len(),
                    eq.n_params()
                ));
                continue;
            }
            match nonlinear::fit(eq, ds, opts) {
                Ok(fit) => {
                    let flags = rout(std::slice::from_ref(ds));
                    out.push(Fitted {
                        sets: vec![(d.name.clone(), d.dataset, 0, ds.x.clone(), ds.y.clone())],
                        fit,
                        rout_flags: flags,
                    })
                }
                Err(e) => notes.push(format!("{}: the fit failed ({e}).", d.name)),
            }
        }
    }
    Ok((out, notes))
}

fn curve_for(
    eq: &Equation,
    f: &CurveFit,
    d: usize,
    (name, ds): (&str, usize),
    xs: &[f64],
    opts: &FitOptions,
    bands: Bands,
) -> Vec<Curve> {
    let p = f.values(d);
    let plain = || Curve {
        name: format!("{name} (fit)"),
        dataset: Some(ds),
        x: xs.to_vec(),
        y: xs.iter().map(|x| eq.eval(&[*x], &p)).collect(),
        ..Default::default()
    };
    let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
    match (bands, nonlinear::bands(eq, f, d, xs, opts)) {
        (Bands::None, _) | (_, Err(_)) => vec![plain()],
        (b, Ok(t)) => {
            let mut c = Curve {
                name: format!("{name} (fit)"),
                dataset: Some(ds),
                x: t.x.clone(),
                y: t.y.clone(),
                ..Default::default()
            };
            if matches!(b, Bands::Confidence | Bands::Both)
                && finite(&t.ci_lower)
                && finite(&t.ci_upper)
            {
                c.lo = t.ci_lower.clone();
                c.hi = t.ci_upper.clone();
            }
            let mut v = vec![c];
            if matches!(b, Bands::Prediction | Bands::Both)
                && finite(&t.pi_lower)
                && finite(&t.pi_upper)
            {
                v.push(Curve {
                    name: String::new(),
                    dataset: Some(ds),
                    x: t.x,
                    y: t.y,
                    lo: t.pi_lower,
                    hi: t.pi_upper,
                    step: false,
                });
            }
            v
        }
    }
}

/// Nonlinear regression.
pub fn nonlinear_regression(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::NonlinearRegression(spec) = ctx.spec else {
        return Err("not a nonlinear regression spec".into());
    };
    check_level(spec.ci_level)?;
    if let Some(q) = spec.rout_q {
        if !(q > 0.0 && q < 100.0) {
            return Err(format!(
                "ROUT's Q is a percentage between 0 and 100 (got {q}); 1 is the usual choice."
            ));
        }
    }
    let style = ctx.p_style;
    let eq = equation(&spec.equation)?;
    let (cons, shared) = constraints(&eq, spec)?;
    let data: Vec<XyData> = extract::xy(ctx.table, &spec.datasets)?
        .into_iter()
        .filter(|d| !d.x.is_empty())
        .collect();
    if data.is_empty() {
        return Err("There are no XY points to fit.".into());
    }
    let opts = FitOptions {
        weighting: stats_weighting(spec.weighting),
        method: if spec.robust && spec.rout_q.is_none() {
            FitMethod::Robust
        } else {
            FitMethod::LeastSquares
        },
        constraints: cons.clone(),
        ci_level: spec.ci_level,
        profile_ci: spec.profile_ci,
        outlier_q: spec.rout_q.map(|q| q / 100.0),
        ..Default::default()
    };
    let global = shared && data.len() > 1;
    let (fits, mut notes) = fit_all(&eq, &data, spec, &opts, global)?;
    if fits.is_empty() {
        return Err(format!("No data set could be fitted. {}", notes.join(" ")));
    }
    let lvl = pct(spec.ci_level);
    let mut s = sheet("nonlinear_regression", "Nonlinear regression");
    s.methods = methods(&format!(
        "Nonlinear regression ({}; Levenberg–Marquardt), {}{}{}{}, {lvl} {} CIs{}",
        eq.name,
        weight_name(spec.weighting),
        if spec.robust && spec.rout_q.is_none() {
            ", robust (adaptive Lorentzian) fit"
        } else {
            ""
        },
        spec.rout_q.map_or(String::new(), |q| format!(
            ", ROUT outlier removal at Q = {}%",
            g(q)
        )),
        if global {
            ", global fit with shared parameters"
        } else {
            ", each data set fitted separately"
        },
        if spec.profile_ci {
            "asymptotic and profile-likelihood"
        } else {
            "asymptotic"
        },
        if cons.is_some() && !spec.constraints.is_empty() {
            ", with constraints"
        } else {
            ""
        }
    ));
    let mut cols = vec![
        "Data set".to_string(),
        "Parameter".into(),
        "Best-fit".into(),
        "SE".into(),
        format!("Lower {lvl} CI"),
        format!("Upper {lvl} CI"),
    ];
    if spec.profile_ci {
        cols.push(format!("Lower {lvl} CI (profile)"));
        cols.push(format!("Upper {lvl} CI (profile)"));
    }
    cols.push("Status".into());
    cols.push("Dependency".into());
    let mut prows = Vec::new();
    let mut drows = Vec::new();
    let mut grows = Vec::new();
    let mut conv = Vec::new();
    let mut runs = Vec::new();
    let mut reps = Vec::new();
    let mut outl = Vec::new();
    let mut resid_sets = Vec::new();
    let mut runs_bad = Vec::new();
    let mut reps_bad = Vec::new();
    let mut statuses = Vec::new();
    let mut view = GraphView {
        family: "scatter".into(),
        ..Default::default()
    };
    let mut log_curves = Vec::new();
    let mut can_log = true;
    for f in &fits {
        statuses.push((
            f.sets
                .iter()
                .map(|x| x.0.clone())
                .collect::<Vec<_>>()
                .join(", "),
            f.fit.convergence.clone(),
        ));
        conv.push(vec![
            txt(f
                .sets
                .iter()
                .map(|x| x.0.clone())
                .collect::<Vec<_>>()
                .join(", ")),
            txt(status_text(f.fit.convergence.status)),
            int(f.fit.convergence.iterations),
            txt(f.fit.convergence.reason.clone()),
        ]);
        for (name, ds, di, xs, ys) in &f.sets {
            let dfit = &f.fit.datasets[*di];
            for p in &dfit.params {
                let mut row = vec![
                    txt(name.clone()),
                    txt(p.name.clone()),
                    num(p.value),
                    num(p.se),
                    num(p.ci_low),
                    num(p.ci_high),
                ];
                if spec.profile_ci {
                    row.push(
                        p.profile_low
                            .map_or(ResultCell::Empty, |v| num_or(v, "not reached")),
                    );
                    row.push(
                        p.profile_high
                            .map_or(ResultCell::Empty, |v| num_or(v, "not reached")),
                    );
                }
                row.push(txt(pstatus(p.status)));
                row.push(num(p.dependency));
                prows.push(row);
            }
            for d in &dfit.derived {
                drows.push(vec![
                    txt(name.clone()),
                    txt(d.name.clone()),
                    num(d.value),
                    num(d.se),
                    num(d.ci_low),
                    num(d.ci_high),
                ]);
            }
            let (aicc, adj) = (f.fit.aicc, f.fit.adj_r_squared);
            grows.push(vec![
                txt(name.clone()),
                int(dfit.n),
                num(dfit.df),
                num(dfit.r_squared),
                if global { ResultCell::Empty } else { num(adj) },
                num(dfit.ss),
                num(dfit.sy_x),
                if global { ResultCell::Empty } else { num(aicc) },
            ]);
            if let Some(r) = &dfit.runs_test {
                runs.push(vec![
                    txt(name.clone()),
                    int(r.runs),
                    num(r.expected_runs),
                    pv(r.p_exact_too_few_runs),
                ]);
                if r.p_exact_too_few_runs < 0.05 {
                    runs_bad.push(format!(
                        "{name} ({})",
                        p_phrase(r.p_exact_too_few_runs, style)
                    ));
                }
            }
            if let Some(l) = &dfit.replicates_test {
                reps.push(vec![
                    txt(name.clone()),
                    num(l.ss_lack_of_fit),
                    num(l.df_lack_of_fit),
                    num(l.ss_pure_error),
                    num(l.df_pure_error),
                    num(l.f),
                    pv(l.p),
                ]);
                if l.p < 0.05 {
                    reps_bad.push(format!("{name} ({})", p_phrase(l.p, style)));
                }
            }
            let flagged: Vec<usize> = if !dfit.outliers.is_empty() {
                dfit.outliers.clone()
            } else {
                f.rout_flags.get(*di).cloned().unwrap_or_default()
            };
            for &i in &flagged {
                outl.push(vec![
                    txt(name.clone()),
                    num(xs.get(i).copied().unwrap_or(f64::NAN)),
                    num(ys.get(i).copied().unwrap_or(f64::NAN)),
                    num(dfit.residuals.get(i).copied().unwrap_or(f64::NAN)),
                    yes_no(!dfit.outliers.is_empty()),
                ]);
            }
            resid_sets.push((
                name.clone(),
                dfit.residuals
                    .iter()
                    .copied()
                    .filter(|v| v.is_finite())
                    .collect::<Vec<_>>(),
            ));
            // Sentence.
            let ps: Vec<String> = dfit
                .params
                .iter()
                .map(|p| match p.status {
                    ParamStatus::Fixed => format!("{} fixed at {}", p.name, g(p.value)),
                    ParamStatus::AtBound => {
                        format!("{} = {} (at its constraint)", p.name, g(p.value))
                    }
                    _ if p.ci_low.is_finite() => format!(
                        "{} = {}",
                        p.name,
                        est_ci(p.value, p.ci_low, p.ci_high, spec.ci_level)
                    ),
                    _ => format!("{} = {}", p.name, g(p.value)),
                })
                .collect();
            let dv: Vec<String> = dfit
                .derived
                .iter()
                .map(|d| {
                    if d.ci_low.is_finite() {
                        format!(
                            "{} = {}",
                            d.name,
                            est_ci(d.value, d.ci_low, d.ci_high, spec.ci_level)
                        )
                    } else {
                        format!("{} = {}", d.name, g(d.value))
                    }
                })
                .collect();
            s.summary.push(format!(
                "{name}: {}{}; R² = {}, Sy.x = {}.",
                ps.join(", "),
                if dv.is_empty() {
                    String::new()
                } else {
                    format!("; {}", dv.join(", "))
                },
                unit(dfit.r_squared, style),
                g(dfit.sy_x)
            ));
            // Curves.
            let (lo, hi) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, x| {
                (a.0.min(*x), a.1.max(*x))
            });
            view.curves.extend(curve_for(
                &eq,
                &f.fit,
                *di,
                (name, *ds),
                &grid(lo, hi, 200, false),
                &opts,
                spec.bands,
            ));
            if lo > 0.0 {
                log_curves.extend(curve_for(
                    &eq,
                    &f.fit,
                    *di,
                    (name, *ds),
                    &grid(lo, hi, 200, true),
                    &opts,
                    spec.bands,
                ));
            } else {
                can_log = false;
            }
        }
        if global {
            grows.push(vec![
                txt("Global (all data sets)"),
                int(f.fit.n),
                num(f.fit.df),
                num(f.fit.r_squared),
                num(f.fit.adj_r_squared),
                num(f.fit.ss),
                num(f.fit.sy_x),
                num(f.fit.aicc),
            ]);
        }
    }
    s.sections
        .push(section_owned("Best-fit values", cols, prows));
    if !drows.is_empty() {
        s.sections.push(section_owned(
            "Derived values",
            vec![
                "Data set".into(),
                "Quantity".into(),
                "Value".into(),
                "SE".into(),
                format!("Lower {lvl} CI"),
                format!("Upper {lvl} CI"),
            ],
            drows,
        ));
    }
    s.sections.push(section(
        "Goodness of fit",
        &[
            "Data set",
            "n",
            "df",
            "R²",
            "Adjusted R²",
            "SS",
            "Sy.x",
            "AICc",
        ],
        grows,
    ));
    s.sections.push(section(
        "Convergence",
        &["Fit", "Status", "Iterations", "Reason"],
        conv,
    ));
    if !runs.is_empty() {
        s.sections.push(section(
            "Runs test",
            &["Data set", "Runs", "Expected runs", "P (too few runs)"],
            runs,
        ));
    }
    let has_reps = !reps.is_empty();
    if has_reps {
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
            reps,
        ));
    }
    let n_out = outl.len();
    if spec.robust || spec.rout_q.is_some() {
        s.sections.push(section(
            format!("Outliers (ROUT, Q = {}%)", g(spec.rout_q.unwrap_or(1.0))),
            &["Data set", "X", "Y", "Residual", "Excluded from the fit?"],
            outl,
        ));
    }
    // Model comparison.
    if let Some(alt_spec) = &spec.compare_with {
        let eq2 = equation(alt_spec)?;
        let mut cspec = spec.clone();
        cspec.constraints.retain(|c| {
            let p = match c {
                Constraint::Fixed { param, .. }
                | Constraint::Min { param, .. }
                | Constraint::Max { param, .. }
                | Constraint::Shared { param } => param,
            };
            eq2.param_index(p).is_some()
                || eq2.param_names().iter().any(|n| n.eq_ignore_ascii_case(p))
        });
        cspec
            .initial
            .retain(|p, _| eq2.param_names().iter().any(|n| n.eq_ignore_ascii_case(p)));
        let (cons2, _) = constraints(&eq2, &cspec)?;
        let opts2 = FitOptions {
            constraints: cons2,
            ..opts.clone()
        };
        let (fits2, n2) = fit_all(&eq2, &data, &cspec, &opts2, global)?;
        notes.extend(n2.into_iter().map(|n| format!("Second model: {n}")));
        let mut rows = Vec::new();
        for (f1, f2) in fits.iter().zip(&fits2) {
            let name = f1
                .sets
                .iter()
                .map(|x| x.0.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let (simple, complex, sname, cname) = if f1.fit.k <= f2.fit.k {
                (&f1.fit, &f2.fit, eq.name.clone(), eq2.name.clone())
            } else {
                (&f2.fit, &f1.fit, eq2.name.clone(), eq.name.clone())
            };
            match compare_summaries(simple.summary(), complex.summary(), 0.05) {
                Ok(mc) => {
                    let pref = |p: Preferred| {
                        if p == Preferred::Simpler {
                            sname.clone()
                        } else {
                            cname.clone()
                        }
                    };
                    rows.push(vec![
                        txt(name.clone()),
                        txt(sname.clone()),
                        txt(cname.clone()),
                        num(mc.f),
                        num(mc.df_num),
                        num(mc.df_den),
                        pv(mc.p),
                        txt(pref(mc.preferred_f)),
                        num(mc.aicc_simple),
                        num(mc.aicc_complex),
                        num(mc.prob_complex * 100.0),
                        txt(pref(mc.preferred_aicc)),
                    ]);
                    let fpart = if mc.f.is_finite() {
                        format!(
                            "the extra-sum-of-squares F test prefers {} (F({}, {}) = {}, {})",
                            pref(mc.preferred_f),
                            df(mc.df_num),
                            df(mc.df_den),
                            st(mc.f),
                            p_phrase(mc.p, style)
                        )
                    } else {
                        "the models are not nested with fewer parameters in one, so only AICc applies".to_string()
                    };
                    s.summary.push(format!(
                        "Model comparison ({name}): {fpart}; AICc prefers {} ({}% probability that it is correct, ΔAICc = {}).",
                        pref(mc.preferred_aicc),
                        format_sig(100.0 * mc.prob_complex.max(mc.prob_simple), Some(1), 4),
                        st(mc.delta_aicc)
                    ));
                }
                Err(e) => notes.push(format!("Model comparison for {name}: {e}.")),
            }
        }
        s.sections.push(section(
            "Model comparison",
            &[
                "Data set",
                "Simpler model",
                "More complex model",
                "F",
                "DFn",
                "DFd",
                "P",
                "Preferred (F test)",
                "AICc (simpler)",
                "AICc (more complex)",
                "Probability more complex is correct (%)",
                "Preferred (AICc)",
            ],
            rows,
        ));
    }
    // Checks.
    s.assumptions.push(checks::values_used(
        &notes,
        notes
            .iter()
            .any(|n| n.contains("not fitted") || n.contains("failed")),
    ));
    let bad: Vec<String> = statuses
        .iter()
        .filter(|(_, c)| c.status != FitStatus::Converged)
        .map(|(n, c)| format!("{n}: {}", status_text(c.status).to_lowercase()))
        .collect();
    let worst = statuses
        .iter()
        .map(|(_, c)| c.status)
        .fold(CheckStatus::Pass, |acc, st| {
            let v = match st {
                FitStatus::Converged => CheckStatus::Pass,
                FitStatus::HitConstraint | FitStatus::Ambiguous => CheckStatus::Caution,
                FitStatus::NotConverged | FitStatus::Failed => CheckStatus::Fail,
            };
            match (acc, v) {
                (CheckStatus::Fail, _) | (_, CheckStatus::Fail) => CheckStatus::Fail,
                (CheckStatus::Caution, _) | (_, CheckStatus::Caution) => CheckStatus::Caution,
                _ => CheckStatus::Pass,
            }
        });
    s.assumptions.push(if bad.is_empty() {
        AssumptionCheck::new("Convergence", CheckStatus::Pass, "Every fit converged with every parameter identifiable and inside its constraints.")
    } else {
        AssumptionCheck::new(
            "Convergence",
            worst,
            format!("{}. Parameters at a constraint have no SE; an ambiguous fit means the data cannot determine some parameter, so its CI is meaningless.", bad.join("; ")),
        )
        .or_else("Constrain or fix the parameter the data cannot determine (for example Bottom = 0), supply better initial values, or collect data that cover the whole curve.")
    });
    s.assumptions.push(if runs_bad.is_empty() {
        AssumptionCheck::new(
            "Runs test",
            CheckStatus::Pass,
            "The points alternate above and below the curve about as often as chance predicts.",
        )
    } else {
        AssumptionCheck::new(
            "Runs test",
            CheckStatus::Caution,
            format!(
                "Too few runs for {}: the curve deviates systematically from the data.",
                runs_bad.join(", ")
            ),
        )
        .or_else("Try a different model (and compare them with \"compare_with\").")
    });
    if has_reps {
        s.assumptions.push(if reps_bad.is_empty() {
            AssumptionCheck::new(
                "Replicates test",
                CheckStatus::Pass,
                "The curve passes as close to the replicate means as their scatter predicts.",
            )
        } else {
            AssumptionCheck::new(
                "Replicates test",
                CheckStatus::Caution,
                format!(
                    "The curve misses the replicate means more than their scatter explains for {}.",
                    reps_bad.join(", ")
                ),
            )
            .or_else("Try a different model, or check whether the replicates are independent.")
        });
    }
    let (c, sec) = checks::normality(
        &resid_sets,
        "the residuals",
        "Use a robust fit (option \"robust\") or weighting if the scatter grows with Y.",
    );
    s.assumptions.push(c);
    s.sections.push(sec);
    if spec.robust || spec.rout_q.is_some() {
        s.assumptions.push(AssumptionCheck::new(
            "Outliers (ROUT)",
            if n_out > 0 {
                CheckStatus::Caution
            } else {
                CheckStatus::Pass
            },
            if n_out > 0 {
                format!(
                    "ROUT flags {n_out} point{}; see the Outliers table.",
                    if n_out == 1 { "" } else { "s" }
                )
            } else {
                "ROUT flags no point.".to_string()
            },
        ));
    } else {
        s.assumptions.push(AssumptionCheck::new(
            "Outliers",
            CheckStatus::NotChecked,
            "Least squares is sensitive to outliers; none were looked for.",
        ).or_else("Set \"rout_q\" to 1 to identify and remove outliers with ROUT, or turn on the robust fit."));
    }
    if eq.x_transform == XTransform::Log10 {
        let xs: Vec<f64> = data.iter().flat_map(|d| d.x.iter().copied()).collect();
        let (lo, hi) = xs
            .iter()
            .fold((f64::INFINITY, 0.0f64), |a, x| (a.0.min(*x), a.1.max(*x)));
        let looks_conc = lo > 0.0 && hi / lo >= 100.0;
        s.assumptions.push(if looks_conc {
            AssumptionCheck::new(
                "X as log(concentration)",
                CheckStatus::Caution,
                format!("This equation expects X = log10(concentration), but X runs from {} to {}, which looks like concentrations.", g(lo), g(hi)),
            )
            .or_else("Transform X with a column formula = log10(X), or choose the matching equation that takes concentrations (e.g. agonist_variable_slope).")
        } else {
            AssumptionCheck::new(
                "X as log(concentration)",
                CheckStatus::Pass,
                "The equation expects X = log10(concentration), and X looks like logarithms.",
            )
        });
    }
    s.assumptions.push(independence("Each point must be independent of the others; replicates of one sample are not independent experiments."));
    s.assumptions.push(AssumptionCheck::new(
        "Scatter of Y",
        CheckStatus::NotChecked,
        format!(
            "{} assumes the scatter of Y about the curve follows a Gaussian distribution with {}.",
            if spec.weighting == Weighting::None {
                "Least squares"
            } else {
                "Weighted least squares"
            },
            match spec.weighting {
                Weighting::None => "the same SD everywhere",
                Weighting::RelativeY2 => "an SD proportional to Y",
                Weighting::PoissonY => "a variance proportional to Y",
                Weighting::InverseSd2 => "the SD of the replicates at each X",
            }
        ),
    ));
    if can_log {
        view.curves_log = Some(log_curves);
    }
    let keys: Vec<Option<usize>> = fits
        .iter()
        .flat_map(|f| f.sets.iter().map(|x| Some(x.1)))
        .collect();
    let mut ag = graph_for("scatter", &keys);
    ag.views.push(view);
    s.graph_family = Some("scatter".into());
    s.graph = Some(ag);
    Ok(s)
}
