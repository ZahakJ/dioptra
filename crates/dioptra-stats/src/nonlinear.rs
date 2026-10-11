//! Nonlinear regression (curve fitting) the way GraphPad Prism does it: least squares by
//! Levenberg–Marquardt (Moré 1978 / MINPACK `lmder`, crate-private `lm`), weighting,
//! box constraints, fixed and shared parameters (global fits over several data sets),
//! robust fitting with Prism's adaptive Lorentzian merit and ROUT outlier removal
//! (Motulsky & Brown 2006), asymptotic and profile-likelihood confidence intervals,
//! confidence and prediction bands, goodness of fit and model comparison.
//!
//! Conventions:
//!
//! * Residuals are reported as `Y - f(X)`. Internally the solver minimises
//!   `sum r_i^2` with `r_i = (f_i - y_i)/sigma_i` (weighted residuals).
//! * Standard errors are asymptotic: `SE_j = sqrt(s^2 [(J'J)^{-1}]_jj)`, `s^2 = SS/df`,
//!   with J the weighted-residual Jacobian at the solution and `(J'J)^{-1}` from the
//!   refined pivoted QR of `linalg` (the normal equations are never formed). CIs are
//!   `b +/- t(1-alpha/2, df) SE` (Prism's "symmetrical"); profile-likelihood CIs
//!   (Prism's "asymmetrical") on request.
//! * AICc = N ln(SS/N) + 2K + 2K(K+1)/(N-K-1) with K = fitted parameters + 1 (Prism
//!   counts the residual variance as a parameter).
//! * Box constraints are enforced by an active-set projection inside LM (see `lm`); a
//!   parameter that ends on a bound is reported with status `AtBound`, no SE, and the fit
//!   is flagged "hit constraint" as in Prism.

use crate::dist::{p_upper_f, FisherF, StudentT};
use crate::equations::{DerivedKind, Equation};
use crate::linalg::{rank_select, Acc, Design, LeastSquares};
use crate::lm::{self, LmOptions, LmStop, Residuals};
use crate::outliers::{rout_fdr_flags, rsdr};
use crate::regression::{lack_of_fit, runs_test_by_x, LackOfFit, RunsTest};
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

// =======================================================================================
// Inputs
// =======================================================================================

/// One data set: X (and further independent variables for user equations with several),
/// Y, and optional per-point SD (for 1/SD^2 weighting, e.g. from replicates).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Dataset {
    /// First independent variable.
    pub x: Vec<f64>,
    /// Response.
    pub y: Vec<f64>,
    /// Per-point SD (only read by [`Weighting::InverseSD2`]).
    pub sd: Option<Vec<f64>>,
    /// Further independent variables (columns), for equations with more than one.
    pub extra_x: Vec<Vec<f64>>,
}

impl Dataset {
    /// X and Y.
    pub fn new(x: &[f64], y: &[f64]) -> Self {
        Dataset {
            x: x.to_vec(),
            y: y.to_vec(),
            sd: None,
            extra_x: vec![],
        }
    }
    /// With per-point SDs.
    pub fn with_sd(mut self, sd: &[f64]) -> Self {
        self.sd = Some(sd.to_vec());
        self
    }
}

/// Weighting (Prism's Method tab). Weights are 1/sigma^2 with sigma as below.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Weighting {
    /// Ordinary least squares.
    #[default]
    None,
    /// 1/Y (sigma^2 = |Y|; "Poisson-like").
    InverseY,
    /// 1/Y^2 (relative weighting: minimises sum ((y-f)/Y)^2).
    InverseY2,
    /// 1/Y^K (general weighting).
    InverseYK(f64),
    /// 1/X.
    InverseX,
    /// 1/X^2.
    InverseX2,
    /// 1/SD^2 with the SDs of the data set.
    InverseSD2,
    /// Poisson regression: minimises the Poisson deviance 2 sum [y ln(y/f) - (y - f)]
    /// (deviance residuals); Y must be counts, f > 0.
    Poisson,
}

/// Which Y the 1/Y^K weights use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum WeightBase {
    /// The fitted curve (Prism's choice: the weights move with the fit; the objective is
    /// sum ((y - f)/|f|^{K/2})^2, differentiated in full).
    #[default]
    Curve,
    /// The data (fixed weights 1/|y|^K; SciPy `curve_fit(sigma=|y|^{K/2})`).
    Data,
}

/// Robust loss with a fixed scale c (SciPy `least_squares(loss=..., f_scale=c)`): the
/// objective is sum c^2 rho((r/c)^2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RobustLoss {
    /// rho(z) = ln(1 + z) (Cauchy / Lorentzian).
    Cauchy,
    /// rho(z) = 2 (sqrt(1 + z) - 1).
    SoftL1,
}

/// Fitting method.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum FitMethod {
    /// Least squares.
    #[default]
    LeastSquares,
    /// Prism's robust fit: minimise the Lorentzian merit sum ln(1 + (r/RSDR)^2) with RSDR
    /// recomputed from the residuals (Motulsky & Brown 2006). No SEs or CIs (as Prism).
    Robust,
    /// A robust loss with a fixed scale.
    RobustFixedScale {
        /// The loss.
        loss: RobustLoss,
        /// The scale c.
        scale: f64,
    },
}

/// Jacobian source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum JacobianMode {
    /// The equation's own gradient: analytic for built-in equations, automatic
    /// differentiation for parsed equations, central differences for closures.
    #[default]
    Exact,
    /// Central finite differences (optionally Richardson-extrapolated) for every equation.
    FiniteDifference {
        /// Richardson extrapolation from h and h/2.
        richardson: bool,
    },
}

/// Constraint on one equation parameter (Prism's Constrain tab).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParamConstraint {
    /// Hold at this value in every data set.
    pub fixed: Option<f64>,
    /// Hold at a per-data-set value (data-set constants such as an inhibitor
    /// concentration).
    pub fixed_per_dataset: Option<Vec<f64>>,
    /// Lower bound.
    pub lower: f64,
    /// Upper bound.
    pub upper: f64,
    /// One value shared by all data sets (global fit).
    pub shared: bool,
    /// Initial value for every data set (overrides the equation's rule).
    pub initial: Option<f64>,
}

impl Default for ParamConstraint {
    fn default() -> Self {
        ParamConstraint {
            fixed: None,
            fixed_per_dataset: None,
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
            shared: false,
            initial: None,
        }
    }
}

impl ParamConstraint {
    /// Fit freely.
    pub fn free() -> Self {
        Self::default()
    }
    /// Constant value.
    pub fn fixed(v: f64) -> Self {
        ParamConstraint {
            fixed: Some(v),
            ..Self::default()
        }
    }
    /// Shared across data sets.
    pub fn shared() -> Self {
        ParamConstraint {
            shared: true,
            ..Self::default()
        }
    }
    /// Between bounds.
    pub fn between(lower: f64, upper: f64) -> Self {
        ParamConstraint {
            lower,
            upper,
            ..Self::default()
        }
    }
}

/// Fit options.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FitOptions {
    /// Weighting.
    pub weighting: Weighting,
    /// Data or curve Y for 1/Y^K weights.
    pub weight_base: WeightBase,
    /// Least squares or robust.
    pub method: FitMethod,
    /// One constraint per equation parameter; `None` uses the equation's defaults.
    pub constraints: Option<Vec<ParamConstraint>>,
    /// Initial values per data set (full parameter vectors); `None` uses the rules.
    pub initial: Option<Vec<Vec<f64>>>,
    /// Confidence level (0.95).
    pub ci_level: f64,
    /// Compute profile-likelihood (asymmetrical) CIs.
    pub profile_ci: bool,
    /// Maximum LM iterations (1000, Prism's limit).
    pub max_iter: usize,
    /// Relative sum-of-squares reduction tolerance (1e-12).
    pub ftol: f64,
    /// Relative step tolerance (1e-12).
    pub xtol: f64,
    /// Gradient (cosine) tolerance (1e-15).
    pub gtol: f64,
    /// Use geodesic acceleration in LM.
    pub geodesic: bool,
    /// Jacobian source.
    pub jacobian: JacobianMode,
    /// Points in the curve table with bands (0: none).
    pub curve_points: usize,
    /// X range of the curve table (default: the data range).
    pub curve_range: Option<(f64, f64)>,
    /// ROUT outlier removal with this Q (fraction, e.g. 0.01): robust fit, flag, refit.
    pub outlier_q: Option<f64>,
}

impl Default for FitOptions {
    fn default() -> Self {
        FitOptions {
            weighting: Weighting::None,
            weight_base: WeightBase::Curve,
            method: FitMethod::LeastSquares,
            constraints: None,
            initial: None,
            ci_level: 0.95,
            profile_ci: false,
            max_iter: 1000,
            ftol: 1e-12,
            xtol: 1e-12,
            gtol: 1e-15,
            geodesic: false,
            jacobian: JacobianMode::Exact,
            curve_points: 0,
            curve_range: None,
            outlier_q: None,
        }
    }
}

// =======================================================================================
// Outputs
// =======================================================================================

/// How a parameter was treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParamStatus {
    /// Fitted for this data set.
    Fitted,
    /// Fitted, one value shared by all data sets.
    Shared,
    /// Held constant.
    Fixed,
    /// Fitted but ended on a bound (no SE).
    AtBound,
}

/// A parameter estimate.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParamEstimate {
    /// Name.
    pub name: String,
    /// Value.
    pub value: f64,
    /// Asymptotic standard error (NaN for fixed, at-bound, robust or df = 0).
    pub se: f64,
    /// Lower asymptotic CI limit.
    pub ci_low: f64,
    /// Upper asymptotic CI limit.
    pub ci_high: f64,
    /// Profile-likelihood CI (lower), when requested; +/-inf when the profile never
    /// reaches the threshold (Prism: "???"), the bound when it reaches a constraint first.
    pub profile_low: Option<f64>,
    /// Profile-likelihood CI (upper).
    pub profile_high: Option<f64>,
    /// Treatment.
    pub status: ParamStatus,
    /// Dependency 1 - 1/([(J'J)^{-1}]_jj (J'J)_jj) (Prism: near 1 means redundant).
    pub dependency: f64,
}

/// A derived quantity (EC50, half-life, ...).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DerivedEstimate {
    /// Name.
    pub name: String,
    /// Value.
    pub value: f64,
    /// Delta-method SE.
    pub se: f64,
    /// CI: the transformed parameter CI for transforms, t interval otherwise.
    pub ci_low: f64,
    /// Upper CI limit.
    pub ci_high: f64,
}

/// Fitted curve with bands on a grid.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurveTable {
    /// X.
    pub x: Vec<f64>,
    /// f(X).
    pub y: Vec<f64>,
    /// Confidence band (delta method: f +/- t sqrt(g' Cov g)).
    pub ci_lower: Vec<f64>,
    /// Upper confidence band.
    pub ci_upper: Vec<f64>,
    /// Prediction band (f +/- t sqrt(g' Cov g + s^2/w(X))).
    pub pi_lower: Vec<f64>,
    /// Upper prediction band.
    pub pi_upper: Vec<f64>,
}

/// Per-data-set part of a fit.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DatasetFit {
    /// Parameters, in equation order.
    pub params: Vec<ParamEstimate>,
    /// Derived quantities.
    pub derived: Vec<DerivedEstimate>,
    /// Points used.
    pub n: usize,
    /// (Weighted) residual sum of squares of this data set.
    pub ss: f64,
    /// n minus the parameters that affect this data set.
    pub df: f64,
    /// sqrt(SS/df).
    pub sy_x: f64,
    /// 1 - SS/SS_total (weighted with the final weights when weighted).
    pub r_squared: f64,
    /// Y - f(X) at every input position (NaN where the point was not used).
    pub residuals: Vec<f64>,
    /// f(X) at every input position.
    pub fitted: Vec<f64>,
    /// Weighted residuals (y - f)/sigma at every input position.
    pub weighted_residuals: Vec<f64>,
    /// Runs test of the residual signs in X order.
    pub runs_test: Option<RunsTest>,
    /// Replicates (lack-of-fit) test when X repeats (unweighted least squares only).
    pub replicates_test: Option<LackOfFit>,
    /// Curve table with bands.
    pub curve: Option<CurveTable>,
    /// Indices flagged as outliers by ROUT (excluded from the fit).
    pub outliers: Vec<usize>,
}

/// Outcome of the iterations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FitStatus {
    /// Converged.
    Converged,
    /// Converged with at least one parameter on a constraint (Prism: "Hit constraint").
    HitConstraint,
    /// Converged, but a parameter is not identifiable (aliased Jacobian column or
    /// dependency > 0.9999; Prism: "Ambiguous").
    Ambiguous,
    /// Stopped at the iteration limit (Prism: "Not converged").
    NotConverged,
    /// The model could not be evaluated (non-finite values).
    Failed,
}

/// Convergence diagnostics.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Convergence {
    /// Status.
    pub status: FitStatus,
    /// Human-readable reason.
    pub reason: String,
    /// LM iterations.
    pub iterations: usize,
    /// Residual-function evaluations.
    pub function_evaluations: usize,
}

/// A curve fit (one data set, or a global fit over several).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurveFit {
    /// Equation name.
    pub equation: String,
    /// Parameter names.
    pub param_names: Vec<String>,
    /// Per data set.
    pub datasets: Vec<DatasetFit>,
    /// Labels of the fitted parameters ("Top", or "Top [2]" for data set 2 when local).
    pub free_labels: Vec<String>,
    /// Fitted values, in `free_labels` order.
    pub free_values: Vec<f64>,
    /// Covariance s^2 (J'J)^{-1} of the fitted parameters, row-major (NaN rows/columns
    /// for parameters at a bound).
    pub covariance: Vec<f64>,
    /// Correlation matrix of the fitted parameters, row-major.
    pub correlation: Vec<f64>,
    /// Total points.
    pub n: usize,
    /// Fitted parameters (K - 1 in the AICc).
    pub k: usize,
    /// N - k.
    pub df: f64,
    /// (Weighted) residual sum of squares (robust fits: of the plain residuals).
    pub ss: f64,
    /// sqrt(SS/df).
    pub sy_x: f64,
    /// 1 - SS/SS_total over all data sets.
    pub r_squared: f64,
    /// 1 - (1 - R^2)(N - 1)/(N - k).
    pub adj_r_squared: f64,
    /// AICc with K = k + 1.
    pub aicc: f64,
    /// RSDR for robust fits and ROUT.
    pub rsdr: Option<f64>,
    /// Robust objective sum c^2 rho((r/c)^2) for fixed-scale robust fits.
    pub robust_objective: Option<f64>,
    /// Poisson deviance for Poisson fits.
    pub deviance: Option<f64>,
    /// Confidence level.
    pub ci_level: f64,
    /// t critical value for the CIs.
    pub t_critical: f64,
    /// Convergence.
    pub convergence: Convergence,
    /// Weighting used.
    pub weighting: Weighting,
    /// Method used.
    pub method: FitMethod,
}

impl CurveFit {
    /// Parameter values of data set `d`.
    pub fn values(&self, d: usize) -> Vec<f64> {
        self.datasets[d].params.iter().map(|p| p.value).collect()
    }
    /// Parameter values of the first data set.
    pub fn params(&self) -> Vec<f64> {
        self.values(0)
    }
    /// Standard errors of the first data set.
    pub fn se(&self) -> Vec<f64> {
        self.datasets[0].params.iter().map(|p| p.se).collect()
    }
    /// The quantities model comparison needs.
    pub fn summary(&self) -> FitSummary {
        FitSummary {
            n: self.n,
            k: self.k,
            ss: self.ss,
            df: self.df,
        }
    }
}

// =======================================================================================
// The residual problem
// =======================================================================================

#[derive(Debug, Clone)]
struct CleanSet {
    /// Row-major points, nv values each.
    x: Vec<f64>,
    y: Vec<f64>,
    /// Constant weights (1 when none or curve-based).
    w: Vec<f64>,
    orig: Vec<usize>,
    n_in: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Src {
    Free(usize),
    Fixed(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ResKind {
    Plain,
    CurveY(f64),
    Poisson,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Loss {
    None,
    Cauchy(f64),
    SoftL1(f64),
}

struct Problem<'a> {
    eq: &'a Equation,
    sets: Vec<CleanSet>,
    map: Vec<Vec<Src>>,
    nv: usize,
    kind: ResKind,
    loss: Loss,
    fd: Option<bool>,
    m: usize,
}

impl Problem<'_> {
    fn params_of(&self, d: usize, theta: &[f64], out: &mut [f64]) {
        for (j, s) in self.map[d].iter().enumerate() {
            out[j] = match *s {
                Src::Free(k) => theta[k],
                Src::Fixed(v) => v,
            };
        }
    }

    /// Base residual r0 = (f - y)/sigma and dr0/df.
    #[inline]
    fn base(&self, f: f64, y: f64, w: f64) -> (f64, f64) {
        match self.kind {
            ResKind::Plain => {
                let s = w.sqrt();
                (s * (f - y), s)
            }
            ResKind::CurveY(k) => {
                let sig = f.abs().powf(0.5 * k);
                let e = f - y;
                (e / sig, (1.0 - 0.5 * k * e / f) / sig)
            }
            ResKind::Poisson => {
                if !(f > 0.0) {
                    return (f64::NAN, f64::NAN);
                }
                let dev = if y > 0.0 {
                    2.0 * (y * (y / f).ln() - (y - f))
                } else {
                    2.0 * f
                };
                let r = (f - y).signum() * dev.max(0.0).sqrt();
                let d = if r.abs() > 1e-150 && (f - y).abs() > 1e-8 * f {
                    (f - y) / (f * r)
                } else {
                    1.0 / f.sqrt()
                };
                (r, d)
            }
        }
    }

    /// Loss transform z(r0) and dz/dr0.
    #[inline]
    fn transform(&self, r: f64) -> (f64, f64) {
        match self.loss {
            Loss::None => (r, 1.0),
            Loss::Cauchy(c) => {
                let u = r / c;
                let u2 = u * u;
                if u.abs() < 1e-8 {
                    return (r, 1.0);
                }
                let l = u2.ln_1p().sqrt();
                (c * r.signum() * l, u.abs() / ((1.0 + u2) * l))
            }
            Loss::SoftL1(c) => {
                let u = r / c;
                let u2 = u * u;
                if u.abs() < 1e-8 {
                    return (r, 1.0);
                }
                let q = (1.0 + u2).sqrt();
                let rho = 2.0 * u2 / (q + 1.0);
                let l = rho.sqrt();
                (c * r.signum() * l, u.abs() / (q * l))
            }
        }
    }

    fn eval_point(&self, x: &[f64], p: &[f64], g: &mut [f64], want_grad: bool) -> f64 {
        if !want_grad {
            return self.eq.eval(x, p);
        }
        match self.fd {
            None => self.eq.eval_grad(x, p, g),
            Some(r) => self.eq.eval_grad_fd(x, p, g, r),
        }
    }

    /// Base residuals (no loss transform) and fitted values per data set.
    fn base_residuals(&self, theta: &[f64]) -> Vec<(Vec<f64>, Vec<f64>)> {
        let np = self.eq.n_params();
        let mut p = vec![0.0; np];
        let mut g = vec![0.0; np];
        self.sets
            .iter()
            .enumerate()
            .map(|(d, s)| {
                self.params_of(d, theta, &mut p);
                let mut r = Vec::with_capacity(s.y.len());
                let mut fv = Vec::with_capacity(s.y.len());
                for i in 0..s.y.len() {
                    let f =
                        self.eval_point(&s.x[i * self.nv..(i + 1) * self.nv], &p, &mut g, false);
                    r.push(self.base(f, s.y[i], s.w[i]).0);
                    fv.push(f);
                }
                (r, fv)
            })
            .collect()
    }
}

impl Residuals for Problem<'_> {
    fn m(&self) -> usize {
        self.m
    }
    fn eval(&self, theta: &[f64], r: &mut [f64], mut jac: Option<&mut [f64]>) -> bool {
        let np = self.eq.n_params();
        let m = self.m;
        let mut p = vec![0.0; np];
        let mut g = vec![0.0; np];
        if let Some(j) = jac.as_deref_mut() {
            j.fill(0.0);
        }
        let want = jac.is_some();
        let mut idx = 0;
        for (d, s) in self.sets.iter().enumerate() {
            self.params_of(d, theta, &mut p);
            for i in 0..s.y.len() {
                let f = self.eval_point(&s.x[i * self.nv..(i + 1) * self.nv], &p, &mut g, want);
                let (r0, d0) = self.base(f, s.y[i], s.w[i]);
                let (z, dz) = self.transform(r0);
                if !z.is_finite() {
                    return false;
                }
                r[idx] = z;
                if let Some(j) = jac.as_deref_mut() {
                    let fac = d0 * dz;
                    for (pj, src) in self.map[d].iter().enumerate() {
                        if let Src::Free(k) = *src {
                            j[k * m + idx] += fac * g[pj];
                        }
                    }
                }
                idx += 1;
            }
        }
        true
    }
}

/// The problem with free parameter k pinned at v (profile likelihood).
struct Pinned<'a> {
    inner: &'a dyn Residuals,
    k: usize,
    v: f64,
    n: usize,
}

impl Pinned<'_> {
    fn full(&self, q: &[f64]) -> Vec<f64> {
        let mut t = Vec::with_capacity(self.n);
        t.extend_from_slice(&q[..self.k]);
        t.push(self.v);
        t.extend_from_slice(&q[self.k..]);
        t
    }
}

impl Residuals for Pinned<'_> {
    fn m(&self) -> usize {
        self.inner.m()
    }
    fn eval(&self, q: &[f64], r: &mut [f64], jac: Option<&mut [f64]>) -> bool {
        let t = self.full(q);
        match jac {
            None => self.inner.eval(&t, r, None),
            Some(j) => {
                let m = self.m();
                let mut full = vec![0.0; m * self.n];
                if !self.inner.eval(&t, r, Some(&mut full)) {
                    return false;
                }
                let mut c = 0;
                for col in 0..self.n {
                    if col == self.k {
                        continue;
                    }
                    j[c * m..(c + 1) * m].copy_from_slice(&full[col * m..(col + 1) * m]);
                    c += 1;
                }
                true
            }
        }
    }
}

// =======================================================================================
// Setup
// =======================================================================================

struct Setup {
    sets: Vec<CleanSet>,
    map: Vec<Vec<Src>>,
    theta0: Vec<f64>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    labels: Vec<String>,
    shared: Vec<bool>,
}

fn clean_sets(
    eq: &Equation,
    data: &[Dataset],
    weighting: Weighting,
    base: WeightBase,
    exclude: Option<&[Vec<usize>]>,
) -> Result<Vec<CleanSet>> {
    let nv = eq.variables.len();
    let mut out = Vec::new();
    for (d, ds) in data.iter().enumerate() {
        if ds.x.len() != ds.y.len() {
            return Err(invalid(format!(
                "data set {}: X and Y lengths differ",
                d + 1
            )));
        }
        if ds.extra_x.len() + 1 < nv {
            return Err(invalid(format!(
                "data set {}: the equation needs {nv} independent variables",
                d + 1
            )));
        }
        let mut s = CleanSet {
            x: Vec::new(),
            y: Vec::new(),
            w: Vec::new(),
            orig: Vec::new(),
            n_in: ds.y.len(),
        };
        for i in 0..ds.y.len() {
            if exclude.is_some_and(|e| e[d].contains(&i)) {
                continue;
            }
            let mut xs = vec![ds.x[i]];
            for c in ds.extra_x.iter().take(nv - 1) {
                xs.push(c.get(i).copied().unwrap_or(f64::NAN));
            }
            let y = ds.y[i];
            if xs.iter().any(|v| !v.is_finite()) || !y.is_finite() {
                continue;
            }
            let w = match (weighting, base) {
                (Weighting::None, _) | (Weighting::Poisson, _) => 1.0,
                (Weighting::InverseY, WeightBase::Data) => 1.0 / y.abs(),
                (Weighting::InverseY2, WeightBase::Data) => 1.0 / (y * y),
                (Weighting::InverseYK(k), WeightBase::Data) => 1.0 / y.abs().powf(k),
                (Weighting::InverseY, _)
                | (Weighting::InverseY2, _)
                | (Weighting::InverseYK(_), _) => 1.0,
                (Weighting::InverseX, _) => 1.0 / xs[0].abs(),
                (Weighting::InverseX2, _) => 1.0 / (xs[0] * xs[0]),
                (Weighting::InverseSD2, _) => {
                    let sd = ds
                        .sd
                        .as_ref()
                        .and_then(|v| v.get(i).copied())
                        .unwrap_or(f64::NAN);
                    if sd.is_nan() {
                        continue;
                    }
                    1.0 / (sd * sd)
                }
            };
            if !(w.is_finite() && w > 0.0) {
                return Err(invalid(format!(
                    "impossible weights: data set {}, point {} (weight {w})",
                    d + 1,
                    i + 1
                )));
            }
            if weighting == Weighting::Poisson && y < 0.0 {
                return Err(invalid("Poisson regression needs Y >= 0"));
            }
            s.x.extend_from_slice(&xs);
            s.y.push(y);
            s.w.push(w);
            s.orig.push(i);
        }
        out.push(s);
    }
    Ok(out)
}

fn constraints_of(eq: &Equation, opts: &FitOptions) -> Result<Vec<ParamConstraint>> {
    let np = eq.n_params();
    let cs: Vec<ParamConstraint> = match &opts.constraints {
        Some(c) => {
            if c.len() != np {
                return Err(invalid(format!(
                    "{} constraints for {np} parameters",
                    c.len()
                )));
            }
            c.clone()
        }
        None => eq
            .params
            .iter()
            .map(|p| ParamConstraint {
                lower: p.lower,
                upper: p.upper,
                ..ParamConstraint::default()
            })
            .collect(),
    };
    for (j, c) in cs.iter().enumerate() {
        if eq.params[j].constant && c.fixed.is_none() && c.fixed_per_dataset.is_none() {
            return Err(invalid(format!(
                "{} is a data-set constant: give its value",
                eq.params[j].name
            )));
        }
        if c.lower.is_nan() || c.upper.is_nan() || c.lower > c.upper {
            return Err(invalid(format!("bad bounds for {}", eq.params[j].name)));
        }
    }
    Ok(cs)
}

fn setup(eq: &Equation, data: &[Dataset], opts: &FitOptions, sets: Vec<CleanSet>) -> Result<Setup> {
    let np = eq.n_params();
    let cs = constraints_of(eq, opts)?;
    let nd = data.len();
    let mut init: Vec<Vec<f64>> = Vec::with_capacity(nd);
    for (d, ds) in data.iter().enumerate() {
        let v = match opts.initial.as_ref().and_then(|v| v.get(d)) {
            Some(v) => {
                if v.len() != np {
                    return Err(invalid(format!(
                        "data set {}: {} initial values for {np} parameters",
                        d + 1,
                        v.len()
                    )));
                }
                v.clone()
            }
            None => {
                let mut v = eq.initial_values(&ds.x, &ds.y);
                for (j, c) in cs.iter().enumerate() {
                    if let Some(i0) = c.initial {
                        v[j] = i0;
                    }
                }
                v
            }
        };
        init.push(
            v.into_iter()
                .map(|a| if a.is_finite() { a } else { 1.0 })
                .collect(),
        );
    }
    let mut map = vec![vec![Src::Fixed(0.0); np]; nd];
    let mut theta0 = Vec::new();
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    let mut labels = Vec::new();
    let mut shared = Vec::new();
    for (j, c) in cs.iter().enumerate() {
        let name = &eq.params[j].name;
        let clamp = |v: f64| v.max(c.lower).min(c.upper);
        if let Some(v) = c.fixed {
            for m in map.iter_mut() {
                m[j] = Src::Fixed(v);
            }
        } else if let Some(vs) = &c.fixed_per_dataset {
            if vs.len() != nd {
                return Err(invalid(format!(
                    "{name}: {} values for {nd} data sets",
                    vs.len()
                )));
            }
            for (d, m) in map.iter_mut().enumerate() {
                m[j] = Src::Fixed(vs[d]);
            }
        } else if c.shared || nd == 1 {
            let k = theta0.len();
            let v0 = if nd > 1 && opts.initial.is_none() && c.initial.is_none() {
                init.iter().map(|v| v[j]).sum::<f64>() / nd as f64
            } else {
                init[0][j]
            };
            theta0.push(clamp(v0));
            lower.push(c.lower);
            upper.push(c.upper);
            labels.push(name.clone());
            shared.push(nd > 1);
            for m in map.iter_mut() {
                m[j] = Src::Free(k);
            }
        } else {
            for (d, m) in map.iter_mut().enumerate() {
                let k = theta0.len();
                theta0.push(clamp(init[d][j]));
                lower.push(c.lower);
                upper.push(c.upper);
                labels.push(format!("{name} [{}]", d + 1));
                shared.push(false);
                m[j] = Src::Free(k);
            }
        }
    }
    Ok(Setup {
        sets,
        map,
        theta0,
        lower,
        upper,
        labels,
        shared,
    })
}

fn res_kind(opts: &FitOptions) -> ResKind {
    match (opts.weighting, opts.weight_base) {
        (Weighting::Poisson, _) => ResKind::Poisson,
        (Weighting::InverseY, WeightBase::Curve) => ResKind::CurveY(1.0),
        (Weighting::InverseY2, WeightBase::Curve) => ResKind::CurveY(2.0),
        (Weighting::InverseYK(k), WeightBase::Curve) => ResKind::CurveY(k),
        _ => ResKind::Plain,
    }
}

fn lm_options(opts: &FitOptions, s: &Setup, polish: bool) -> LmOptions {
    let mut o = LmOptions::new(s.theta0.len());
    o.ftol = opts.ftol;
    o.xtol = opts.xtol;
    o.gtol = opts.gtol;
    o.max_iter = opts.max_iter;
    o.geodesic = opts.geodesic;
    o.lower = s.lower.clone();
    o.upper = s.upper.clone();
    o.polish = polish;
    o
}

// =======================================================================================
// Fitting
// =======================================================================================

/// Fit one data set. See [`fit_global`].
pub fn fit(eq: &Equation, data: &Dataset, opts: &FitOptions) -> Result<CurveFit> {
    fit_global(eq, std::slice::from_ref(data), opts)
}

/// Fit an equation to one or more data sets at once, with parameters free per data set,
/// shared (global fitting), fixed, or bounded as `opts.constraints` says.
///
/// Optimiser: Levenberg–Marquardt after Moré (1978) / MINPACK `lmder` with a scaled trust
/// region, then Newton polishing (see `lm`). Linear equations (lines, polynomials) start
/// from the exact linear least-squares solution. Robust fits (Prism's adaptive
/// Lorentzian): start from the least-squares fit, then alternate RSDR = P68 N/(N-K) of the
/// current residuals with an LM minimisation of sum ln(1 + (r/RSDR)^2) (residuals
/// transformed to RSDR sqrt(ln(1 + (r/RSDR)^2)) so LM minimises the merit exactly) until
/// the parameters and RSDR change by less than 1e-10 relative (at most 200 rounds).
/// ROUT (`outlier_q`): robust fit, the FDR test of Motulsky & Brown (2006) on its
/// residuals with N-K df, then a least-squares refit without the outliers.
pub fn fit_global(eq: &Equation, data: &[Dataset], opts: &FitOptions) -> Result<CurveFit> {
    if data.is_empty() {
        return Err(invalid("no data sets"));
    }
    if !(opts.ci_level > 0.0 && opts.ci_level < 1.0) {
        return Err(invalid("ci_level must be in (0, 1)"));
    }
    if let Some(q) = opts.outlier_q {
        return rout_fit_global(eq, data, q, opts).map(|r| r.fit);
    }
    let sets = clean_sets(eq, data, opts.weighting, opts.weight_base, None)?;
    fit_sets(eq, data, opts, sets, vec![vec![]; data.len()])
}

fn fit_sets(
    eq: &Equation,
    data: &[Dataset],
    opts: &FitOptions,
    sets: Vec<CleanSet>,
    outliers: Vec<Vec<usize>>,
) -> Result<CurveFit> {
    let s = setup(eq, data, opts, sets)?;
    let m: usize = s.sets.iter().map(|c| c.y.len()).sum();
    let nfree = s.theta0.len();
    if nfree == 0 {
        return Err(invalid("no parameter is fitted"));
    }
    if m < nfree {
        return Err(StatsError::NotEnoughData(format!(
            "{m} points for {nfree} fitted parameters"
        )));
    }
    let fd = match opts.jacobian {
        JacobianMode::Exact => None,
        JacobianMode::FiniteDifference { richardson } => Some(richardson),
    };
    let exact = fd.is_none() && eq.has_exact_gradient();
    let mut prob = Problem {
        eq,
        sets: s.sets.clone(),
        map: s.map.clone(),
        nv: eq.variables.len(),
        kind: res_kind(opts),
        loss: Loss::None,
        fd,
        m,
    };
    let lmo = lm_options(opts, &s, exact);
    let mut theta0 = s.theta0.clone();
    if eq.linear && prob.kind == ResKind::Plain {
        if let Some(t) = linear_start(&prob, &theta0) {
            theta0 = t
                .iter()
                .enumerate()
                .map(|(k, v)| v.max(s.lower[k]).min(s.upper[k]))
                .collect();
        }
    }
    let mut out = lm::minimize(&prob, &theta0, &lmo);
    let mut iterations = out.iterations;
    let mut nfev = out.nfev;
    let mut rsdr_v = None;
    let mut robust_obj = None;
    match opts.method {
        FitMethod::LeastSquares => {}
        FitMethod::RobustFixedScale { loss, scale } => {
            if !(scale > 0.0) {
                return Err(invalid("robust scale must be > 0"));
            }
            prob.loss = match loss {
                RobustLoss::Cauchy => Loss::Cauchy(scale),
                RobustLoss::SoftL1 => Loss::SoftL1(scale),
            };
            let start = if out.stop.converged() {
                out.p.clone()
            } else {
                theta0.clone()
            };
            out = lm::minimize(&prob, &start, &lmo);
            iterations += out.iterations;
            nfev += out.nfev;
            robust_obj = Some(out.ss);
        }
        FitMethod::Robust => {
            let k = nfree as f64;
            let mut theta = if out.stop.converged() {
                out.p.clone()
            } else {
                theta0.clone()
            };
            let mut sc = f64::NAN;
            for _ in 0..200 {
                let r: Vec<f64> = prob
                    .base_residuals(&theta)
                    .into_iter()
                    .flat_map(|(r, _)| r)
                    .collect();
                let new_sc = rsdr(&r, k);
                if !(new_sc > 0.0) || !new_sc.is_finite() {
                    break;
                }
                prob.loss = Loss::Cauchy(new_sc);
                let o = lm::minimize(&prob, &theta, &lmo);
                iterations += o.iterations;
                nfev += o.nfev;
                let change =
                    o.p.iter()
                        .zip(&theta)
                        .map(|(a, b)| ((a - b) / b.abs().max(1e-300)).abs())
                        .fold(0.0, f64::max)
                        .max(((new_sc - sc) / new_sc).abs());
                theta = o.p.clone();
                out = o;
                sc = new_sc;
                if change <= 1e-10 {
                    break;
                }
            }
            let r: Vec<f64> = prob
                .base_residuals(&theta)
                .into_iter()
                .flat_map(|(r, _)| r)
                .collect();
            rsdr_v = Some(rsdr(&r, k));
        }
    }
    out.iterations = iterations;
    out.nfev = nfev;
    let robust = opts.method != FitMethod::LeastSquares;
    finish(
        eq, data, opts, &s, &mut prob, out, robust, rsdr_v, robust_obj, outliers,
    )
}

/// Exact linear least-squares solution for a linear equation (Jacobian independent of the
/// parameters).
fn linear_start(prob: &Problem, theta0: &[f64]) -> Option<Vec<f64>> {
    let m = prob.m;
    let n = theta0.len();
    let zero = vec![0.0; n];
    let mut r = vec![0.0; m];
    let mut j = vec![0.0; m * n];
    if !prob.eval(&zero, &mut r, Some(&mut j)) {
        return None;
    }
    let cols: Vec<Vec<f64>> = (0..n).map(|k| j[k * m..(k + 1) * m].to_vec()).collect();
    let d = Design::from_columns(&cols);
    let (_, aliased) = rank_select(&d);
    if !aliased.is_empty() {
        return None;
    }
    let ls = LeastSquares::new(&d);
    // r(0) = sqrt(w)(f(0) - y); minimise |r(0) + J t|.
    let neg: Vec<f64> = r.iter().map(|v| -v).collect();
    Some(ls.fit(&neg).coef)
}

#[allow(clippy::too_many_arguments)]
fn finish(
    eq: &Equation,
    data: &[Dataset],
    opts: &FitOptions,
    s: &Setup,
    prob: &mut Problem,
    out: lm::LmOutcome,
    robust: bool,
    rsdr_v: Option<f64>,
    robust_obj: Option<f64>,
    outliers: Vec<Vec<usize>>,
) -> Result<CurveFit> {
    let theta = out.p.clone();
    let nfree = theta.len();
    let m = prob.m;
    let loss = prob.loss;
    prob.loss = Loss::None;
    let mut r = vec![0.0; m];
    let mut jac = vec![0.0; m * nfree];
    let ok = prob.eval(&theta, &mut r, Some(&mut jac));
    let ss = lm::sum_sq(&r);
    let df = m as f64 - nfree as f64;
    let mut status;
    let mut reason;
    match out.stop {
        LmStop::NotFinite => {
            status = FitStatus::Failed;
            reason = "the equation could not be evaluated (non-finite value)".to_string();
        }
        LmStop::MaxIter => {
            status = FitStatus::NotConverged;
            reason = format!("not converged after {} iterations", out.iterations);
        }
        st => {
            status = FitStatus::Converged;
            reason = match st {
                LmStop::Ftol => "relative change in the sum of squares below tolerance",
                LmStop::Xtol => "relative change in the parameters below tolerance",
                _ => "gradient orthogonal to the residuals",
            }
            .to_string();
        }
    }
    if !ok && status != FitStatus::Failed {
        status = FitStatus::Failed;
        reason = "the equation could not be evaluated at the solution".into();
    }
    // Covariance over the parameters not on a bound.
    let active: Vec<usize> = (0..nfree).filter(|&k| !out.at_bound[k]).collect();
    if active.len() < nfree && status == FitStatus::Converged {
        status = FitStatus::HitConstraint;
        reason = "a parameter ended on a constraint".into();
    }
    let mut cunscaled = vec![f64::NAN; nfree * nfree];
    let mut jtj_diag = vec![f64::NAN; nfree];
    for k in 0..nfree {
        let mut a = Acc::default();
        for i in 0..m {
            a.add_prod(jac[k * m + i], jac[k * m + i]);
        }
        jtj_diag[k] = a.value();
    }
    let mut aliased_any = false;
    if ok && !active.is_empty() {
        let cols: Vec<Vec<f64>> = active
            .iter()
            .map(|&k| jac[k * m..(k + 1) * m].to_vec())
            .collect();
        let d = Design::from_columns(&cols);
        let (_, aliased) = rank_select(&d);
        if aliased.is_empty() {
            let inv = LeastSquares::new(&d).xtx_inverse();
            let na = active.len();
            for (a, &ka) in active.iter().enumerate() {
                for (b, &kb) in active.iter().enumerate() {
                    cunscaled[ka * nfree + kb] = inv[a * na + b];
                }
            }
        } else {
            aliased_any = true;
        }
    }
    let s2 = if prob.kind == ResKind::Poisson {
        1.0
    } else if df > 0.0 {
        ss / df
    } else {
        f64::NAN
    };
    let mut cov: Vec<f64> = cunscaled.iter().map(|c| c * s2).collect();
    if robust {
        cov.iter_mut().for_each(|c| *c = f64::NAN);
    }
    let dependency: Vec<f64> = (0..nfree)
        .map(|k| 1.0 - 1.0 / (cunscaled[k * nfree + k] * jtj_diag[k]))
        .collect();
    if status == FitStatus::Converged && (aliased_any || dependency.iter().any(|&d| d > 0.9999)) {
        status = FitStatus::Ambiguous;
        reason = "a parameter is not identifiable (dependency > 0.9999)".into();
    }
    let mut corr = vec![f64::NAN; nfree * nfree];
    for a in 0..nfree {
        for b in 0..nfree {
            corr[a * nfree + b] =
                cov[a * nfree + b] / (cov[a * nfree + a] * cov[b * nfree + b]).sqrt();
        }
    }
    let tcrit = if df > 0.0 {
        let a = 1.0 - opts.ci_level;
        StudentT { df }.quantile_pq(1.0 - 0.5 * a, 0.5 * a)
    } else {
        f64::NAN
    };
    let se_free: Vec<f64> = (0..nfree).map(|k| cov[k * nfree + k].sqrt()).collect();
    // Profile-likelihood CIs.
    let mut profile: Vec<Option<(f64, f64)>> = vec![None; nfree];
    if opts.profile_ci && !robust && df > 0.0 && ok && status != FitStatus::Failed {
        let lmo = lm_options(opts, s, false);
        for k in 0..nfree {
            if out.at_bound[k] {
                continue;
            }
            profile[k] = Some(profile_ci(
                prob,
                &theta,
                k,
                ss,
                df,
                opts.ci_level,
                se_free[k],
                &lmo,
            ));
        }
    }
    // Per data set.
    let base = prob.base_residuals(&theta);
    let np = eq.n_params();
    let mut datasets = Vec::with_capacity(data.len());
    let mut ss_tot_all = Acc::default();
    let mut pvals = vec![0.0; np];
    for (d, cs) in prob.sets.iter().enumerate() {
        prob.params_of(d, &theta, &mut pvals);
        let (wr, fv) = &base[d];
        let mut ssd = Acc::default();
        for v in wr {
            ssd.add_prod(*v, *v);
        }
        let ssd = ssd.value();
        let nd = cs.y.len();
        let mut uses: Vec<usize> = prob.map[d]
            .iter()
            .filter_map(|s| if let Src::Free(k) = s { Some(*k) } else { None })
            .collect();
        uses.sort_unstable();
        uses.dedup();
        let dfd = nd as f64 - uses.len() as f64;
        // R^2 with the final weights
        let wts: Vec<f64> = (0..nd)
            .map(|i| {
                let e = fv[i] - cs.y[i];
                if e != 0.0 && wr[i].is_finite() {
                    (wr[i] / e) * (wr[i] / e)
                } else {
                    match prob.kind {
                        ResKind::Plain => cs.w[i],
                        ResKind::CurveY(k) => 1.0 / fv[i].abs().powf(k),
                        ResKind::Poisson => 1.0 / fv[i],
                    }
                }
            })
            .collect();
        let sw: f64 = wts.iter().sum();
        let ybar = wts.iter().zip(&cs.y).map(|(w, y)| w * y).sum::<f64>() / sw;
        let mut sst = Acc::default();
        for (w, y) in wts.iter().zip(&cs.y) {
            sst.add_prod(w * (y - ybar), y - ybar);
        }
        let sst = sst.value();
        ss_tot_all.add(sst);
        let r2 = if prob.kind == ResKind::Poisson {
            f64::NAN
        } else {
            1.0 - ssd / sst
        };
        // residuals at the input positions
        let mut residuals = vec![f64::NAN; cs.n_in];
        let mut fitted = vec![f64::NAN; cs.n_in];
        let mut wres = vec![f64::NAN; cs.n_in];
        for i in 0..nd {
            residuals[cs.orig[i]] = cs.y[i] - fv[i];
            fitted[cs.orig[i]] = fv[i];
            wres[cs.orig[i]] = -wr[i];
        }
        let xs: Vec<f64> = (0..nd).map(|i| cs.x[i * prob.nv]).collect();
        let used_res: Vec<f64> = (0..nd).map(|i| cs.y[i] - fv[i]).collect();
        // outliers keep their residuals from this fit
        for &o in &outliers[d] {
            let ds = &data[d];
            if o < ds.x.len() && prob.nv == 1 {
                let f = eq.eval(&[ds.x[o]], &pvals);
                residuals[o] = ds.y[o] - f;
                fitted[o] = f;
            }
        }
        let runs = if nd >= 2 {
            Some(runs_test_by_x(&xs, &used_res))
        } else {
            None
        };
        let lof = if prob.kind == ResKind::Plain
            && opts.weighting == Weighting::None
            && !robust
            && prob.nv == 1
        {
            lack_of_fit(&xs, &cs.y, ssd, uses.len())
        } else {
            None
        };
        // parameters
        let mut params = Vec::with_capacity(np);
        for (j, src) in prob.map[d].iter().enumerate() {
            let name = eq.params[j].name.clone();
            let est = match *src {
                Src::Fixed(v) => ParamEstimate {
                    name,
                    value: v,
                    se: f64::NAN,
                    ci_low: f64::NAN,
                    ci_high: f64::NAN,
                    profile_low: None,
                    profile_high: None,
                    status: ParamStatus::Fixed,
                    dependency: f64::NAN,
                },
                Src::Free(k) => {
                    let se = se_free[k];
                    let st = if out.at_bound[k] {
                        ParamStatus::AtBound
                    } else if s.shared[k] {
                        ParamStatus::Shared
                    } else {
                        ParamStatus::Fitted
                    };
                    ParamEstimate {
                        name,
                        value: theta[k],
                        se,
                        ci_low: theta[k] - tcrit * se,
                        ci_high: theta[k] + tcrit * se,
                        profile_low: profile[k].map(|p| p.0),
                        profile_high: profile[k].map(|p| p.1),
                        status: st,
                        dependency: dependency[k],
                    }
                }
            };
            params.push(est);
        }
        let derived = derived_estimates(eq, &pvals, &prob.map[d], &cov, nfree, tcrit, &params);
        let curve = if opts.curve_points >= 2 && prob.nv == 1 {
            let (a, b) = opts.curve_range.unwrap_or_else(|| {
                let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                (lo, hi)
            });
            let grid: Vec<f64> = (0..opts.curve_points)
                .map(|i| a + (b - a) * i as f64 / (opts.curve_points - 1) as f64)
                .collect();
            Some(bands_impl(
                eq,
                &pvals,
                &prob.map[d],
                &cov,
                nfree,
                s2,
                tcrit,
                opts,
                &grid,
            ))
        } else {
            None
        };
        datasets.push(DatasetFit {
            params,
            derived,
            n: nd,
            ss: ssd,
            df: dfd,
            sy_x: (ssd / dfd).sqrt(),
            r_squared: r2,
            residuals,
            fitted,
            weighted_residuals: wres,
            runs_test: runs,
            replicates_test: lof,
            curve,
            outliers: outliers[d].clone(),
        });
    }
    let n = m;
    let kk = nfree as f64 + 1.0;
    let nf = n as f64;
    let aicc = if nf - kk - 1.0 > 0.0 && ss > 0.0 {
        nf * (ss / nf).ln() + 2.0 * kk + 2.0 * kk * (kk + 1.0) / (nf - kk - 1.0)
    } else {
        f64::NAN
    };
    let r2 = if prob.kind == ResKind::Poisson {
        f64::NAN
    } else {
        1.0 - ss / ss_tot_all.value()
    };
    let deviance = if prob.kind == ResKind::Poisson {
        Some(ss)
    } else {
        None
    };
    prob.loss = loss;
    Ok(CurveFit {
        equation: eq.name.clone(),
        param_names: eq.param_names(),
        datasets,
        free_labels: s.labels.clone(),
        free_values: theta,
        covariance: cov,
        correlation: corr,
        n,
        k: nfree,
        df,
        ss,
        sy_x: (ss / df).sqrt(),
        r_squared: r2,
        adj_r_squared: 1.0 - (1.0 - r2) * (nf - 1.0) / (nf - nfree as f64),
        aicc,
        rsdr: rsdr_v,
        robust_objective: robust_obj,
        deviance,
        ci_level: opts.ci_level,
        t_critical: tcrit,
        convergence: Convergence {
            status,
            reason,
            iterations: out.iterations,
            function_evaluations: out.nfev,
        },
        weighting: opts.weighting,
        method: opts.method,
    })
}

fn derived_estimates(
    eq: &Equation,
    p: &[f64],
    map: &[Src],
    cov: &[f64],
    nfree: usize,
    tcrit: f64,
    params: &[ParamEstimate],
) -> Vec<DerivedEstimate> {
    eq.derived
        .iter()
        .map(|dspec| match dspec.kind {
            DerivedKind::Transform { param, f } => {
                let v = f(p[param]);
                let pe = &params[param];
                let h = 1e-6 * p[param].abs().max(1e-6);
                let der = (f(p[param] + h) - f(p[param] - h)) / (2.0 * h);
                let (a, b) = (f(pe.ci_low), f(pe.ci_high));
                DerivedEstimate {
                    name: dspec.name.clone(),
                    value: v,
                    se: (der * pe.se).abs(),
                    ci_low: a.min(b),
                    ci_high: a.max(b),
                }
            }
            DerivedKind::Function(f) => {
                let v = f(p);
                // gradient over the free parameters
                let mut g = vec![0.0; nfree];
                let mut q = p.to_vec();
                for (j, src) in map.iter().enumerate() {
                    if let Src::Free(k) = *src {
                        let h = 1e-6 * p[j].abs().max(1e-6);
                        q[j] = p[j] + h;
                        let fp = f(&q);
                        q[j] = p[j] - h;
                        let fm = f(&q);
                        q[j] = p[j];
                        g[k] += (fp - fm) / (2.0 * h);
                    }
                }
                let var = quad_form(&g, cov, nfree);
                let se = var.sqrt();
                DerivedEstimate {
                    name: dspec.name.clone(),
                    value: v,
                    se,
                    ci_low: v - tcrit * se,
                    ci_high: v + tcrit * se,
                }
            }
        })
        .collect()
}

fn quad_form(g: &[f64], cov: &[f64], n: usize) -> f64 {
    let mut s = 0.0;
    for a in 0..n {
        if g[a] == 0.0 {
            continue;
        }
        for b in 0..n {
            if g[b] == 0.0 {
                continue;
            }
            s += g[a] * cov[a * n + b] * g[b];
        }
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn bands_impl(
    eq: &Equation,
    p: &[f64],
    map: &[Src],
    cov: &[f64],
    nfree: usize,
    s2: f64,
    tcrit: f64,
    opts: &FitOptions,
    xs: &[f64],
) -> CurveTable {
    let np = eq.n_params();
    let mut g = vec![0.0; np];
    let mut gf = vec![0.0; nfree];
    let mut t = CurveTable {
        x: xs.to_vec(),
        y: Vec::with_capacity(xs.len()),
        ci_lower: Vec::with_capacity(xs.len()),
        ci_upper: Vec::with_capacity(xs.len()),
        pi_lower: Vec::with_capacity(xs.len()),
        pi_upper: Vec::with_capacity(xs.len()),
    };
    for &x in xs {
        let f = eq.eval_grad(&[x], p, &mut g);
        gf.fill(0.0);
        for (j, src) in map.iter().enumerate() {
            if let Src::Free(k) = *src {
                gf[k] += g[j];
            }
        }
        let var = quad_form(&gf, cov, nfree).max(0.0);
        let inv_w = match opts.weighting {
            Weighting::None | Weighting::InverseSD2 => 1.0,
            Weighting::InverseY => f.abs(),
            Weighting::InverseY2 => f * f,
            Weighting::InverseYK(k) => f.abs().powf(k),
            Weighting::InverseX => x.abs(),
            Weighting::InverseX2 => x * x,
            Weighting::Poisson => f,
        };
        let c = tcrit * var.sqrt();
        let pr = tcrit * (var + s2 * inv_w).sqrt();
        t.y.push(f);
        t.ci_lower.push(f - c);
        t.ci_upper.push(f + c);
        t.pi_lower.push(f - pr);
        t.pi_upper.push(f + pr);
    }
    t
}

/// Confidence and prediction bands of data set `d` of a fit at the X values `xs` (the
/// equation must be the one fitted). Delta method: var f(x) = g(x)' Cov g(x), with g the
/// gradient over the fitted parameters; prediction adds s^2/w(x).
pub fn bands(
    eq: &Equation,
    fit: &CurveFit,
    d: usize,
    xs: &[f64],
    opts: &FitOptions,
) -> Result<CurveTable> {
    let ds = fit
        .datasets
        .get(d)
        .ok_or_else(|| invalid("no such data set"))?;
    let nfree = fit.k;
    let p: Vec<f64> = ds.params.iter().map(|q| q.value).collect();
    // rebuild the map from the labels
    let mut map = Vec::with_capacity(p.len());
    for q in &ds.params {
        let src = match q.status {
            ParamStatus::Fixed => Src::Fixed(q.value),
            _ => {
                let local = format!("{} [{}]", q.name, d + 1);
                let k = fit
                    .free_labels
                    .iter()
                    .position(|l| *l == local || *l == q.name)
                    .ok_or_else(|| invalid("fit and equation do not match"))?;
                Src::Free(k)
            }
        };
        map.push(src);
    }
    let s2 = if fit.weighting == Weighting::Poisson {
        1.0
    } else {
        fit.ss / fit.df
    };
    Ok(bands_impl(
        eq,
        &p,
        &map,
        &fit.covariance,
        nfree,
        s2,
        fit.t_critical,
        opts,
        xs,
    ))
}

/// Profile-likelihood CI of free parameter k (Venzon & Moolgavkar 1988; Bates & Watts
/// 1988 ch. 6; Prism's "asymmetrical" CI): the two values of theta_k where the
/// re-minimised SS(theta_k) = SS_min (1 + F(level; 1, df)/df). Bracketed by stepping
/// out in multiples of the SE (x1.6 per step, at most 40 steps or the bound), then Brent.
#[allow(clippy::too_many_arguments)]
fn profile_ci(
    prob: &Problem,
    theta: &[f64],
    k: usize,
    ss_min: f64,
    df: f64,
    level: f64,
    se: f64,
    lmo: &LmOptions,
) -> (f64, f64) {
    let n = theta.len();
    let f = FisherF { d1: 1.0, d2: df }.quantile_pq(level, 1.0 - level);
    let target = ss_min * (1.0 + f / df);
    let mut sub = LmOptions::new(n - 1);
    sub.ftol = lmo.ftol;
    sub.xtol = lmo.xtol;
    sub.max_iter = lmo.max_iter;
    sub.polish = true;
    sub.lower = (0..n).filter(|&j| j != k).map(|j| lmo.lower[j]).collect();
    sub.upper = (0..n).filter(|&j| j != k).map(|j| lmo.upper[j]).collect();
    let reduced = |t: &[f64]| -> Vec<f64> { (0..n).filter(|&j| j != k).map(|j| t[j]).collect() };
    let base_step = if se.is_finite() && se > 0.0 {
        se
    } else {
        (0.1 * theta[k].abs()).max(1e-3)
    };
    let mut res = [0.0; 2];
    for (side_i, side) in [-1.0f64, 1.0].iter().enumerate() {
        let mut warm = reduced(theta);
        let mut prev_v = theta[k];
        let mut found = None;
        let mut mult = 1.0;
        let bound = if *side < 0.0 {
            lmo.lower[k]
        } else {
            lmo.upper[k]
        };
        let eval = |v: f64, warm: &mut Vec<f64>| -> f64 {
            let pin = Pinned {
                inner: prob,
                k,
                v,
                n,
            };
            if n == 1 {
                let mut r = vec![0.0; prob.m];
                return if prob.eval(&[v], &mut r, None) {
                    lm::sum_sq(&r) - target
                } else {
                    f64::INFINITY
                };
            }
            let o = lm::minimize(&pin, warm, &sub);
            if o.stop == LmStop::NotFinite || !o.ss.is_finite() {
                return f64::INFINITY;
            }
            *warm = o.p;
            o.ss - target
        };
        let mut hit_bound = false;
        for _ in 0..40 {
            let mut v = theta[k] + side * base_step * mult;
            if (*side < 0.0 && v <= bound) || (*side > 0.0 && v >= bound) {
                v = bound;
                hit_bound = true;
            }
            let g = eval(v, &mut warm);
            if g >= 0.0 {
                found = Some((prev_v, v));
                break;
            }
            if hit_bound {
                break;
            }
            prev_v = v;
            mult *= 1.6;
        }
        res[side_i] = match found {
            Some((a, b)) => {
                let mut w = warm.clone();
                crate::roots::brent(|v| eval(v, &mut w), a, b, 1e-300, 1e-10, 200)
                    .unwrap_or(f64::NAN)
            }
            None if hit_bound => bound,
            None => side * f64::INFINITY,
        };
    }
    (res[0], res[1])
}

// =======================================================================================
// ROUT for curves
// =======================================================================================

/// Result of [`rout_fit`] / [`rout_fit_global`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoutFit {
    /// The robust (adaptive Lorentzian) fit on all points.
    pub robust: CurveFit,
    /// RSDR of the robust fit (P68 of |residuals| x N/(N-K)).
    pub rsdr: f64,
    /// Outlier indices per data set (input positions).
    pub outliers: Vec<Vec<usize>>,
    /// Least-squares refit without the outliers.
    pub fit: CurveFit,
    /// Q.
    pub q: f64,
}

/// ROUT for a curve (Motulsky & Brown 2006, BMC Bioinformatics 7:123): robust fit with
/// the adaptive Lorentzian merit; RSDR; FDR test on the (weighted) residuals with N - K
/// df ([`crate::outliers::rout_fdr_flags`]); least-squares refit without the outliers.
pub fn rout_fit(eq: &Equation, data: &Dataset, q: f64, opts: &FitOptions) -> Result<RoutFit> {
    rout_fit_global(eq, std::slice::from_ref(data), q, opts)
}

/// ROUT over several data sets fitted together (residuals pooled for the FDR test).
pub fn rout_fit_global(
    eq: &Equation,
    data: &[Dataset],
    q: f64,
    opts: &FitOptions,
) -> Result<RoutFit> {
    if !(q > 0.0 && q < 1.0) {
        return Err(invalid("Q must be in (0, 1)"));
    }
    let mut ro = opts.clone();
    ro.outlier_q = None;
    ro.method = FitMethod::Robust;
    ro.profile_ci = false;
    ro.curve_points = 0;
    let robust = fit_global(eq, data, &ro)?;
    let rs = robust.rsdr.unwrap_or(f64::NAN);
    // pooled weighted residuals, positions
    let mut pos = Vec::new();
    let mut res = Vec::new();
    for (d, dsf) in robust.datasets.iter().enumerate() {
        for (i, r) in dsf.weighted_residuals.iter().enumerate() {
            if r.is_finite() {
                pos.push((d, i));
                res.push(*r);
            }
        }
    }
    let flags = rout_fdr_flags(&res, rs, robust.k as f64, q)?;
    let mut outliers = vec![Vec::new(); data.len()];
    for f in flags {
        let (d, i) = pos[f];
        outliers[d].push(i);
    }
    for o in outliers.iter_mut() {
        o.sort_unstable();
    }
    let mut lo = opts.clone();
    lo.outlier_q = None;
    lo.method = FitMethod::LeastSquares;
    if lo.initial.is_none() {
        lo.initial = Some((0..data.len()).map(|d| robust.values(d)).collect());
    }
    let sets = clean_sets(eq, data, lo.weighting, lo.weight_base, Some(&outliers))?;
    let mut fitv = fit_sets(eq, data, &lo, sets, outliers.clone())?;
    fitv.rsdr = Some(rs);
    Ok(RoutFit {
        robust,
        rsdr: rs,
        outliers,
        fit: fitv,
        q,
    })
}

// =======================================================================================
// Model comparison
// =======================================================================================

/// What model comparison needs from a fit.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FitSummary {
    /// Points.
    pub n: usize,
    /// Fitted parameters.
    pub k: usize,
    /// Residual sum of squares.
    pub ss: f64,
    /// Degrees of freedom.
    pub df: f64,
}

impl FitSummary {
    /// AICc = N ln(SS/N) + 2K + 2K(K+1)/(N-K-1), K = k + 1.
    pub fn aicc(&self) -> f64 {
        let n = self.n as f64;
        let k = self.k as f64 + 1.0;
        if n - k - 1.0 <= 0.0 || !(self.ss > 0.0) {
            return f64::NAN;
        }
        n * (self.ss / n).ln() + 2.0 * k + 2.0 * k * (k + 1.0) / (n - k - 1.0)
    }
}

/// Which model a comparison prefers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Preferred {
    /// The simpler model.
    Simpler,
    /// The more complex model.
    MoreComplex,
}

/// Extra-sum-of-squares F test and AICc comparison of two fits to the same data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelComparison {
    /// F = ((SS1 - SS2)/(df1 - df2))/(SS2/df2) (NaN when df1 <= df2).
    pub f: f64,
    /// df1 - df2.
    pub df_num: f64,
    /// df2.
    pub df_den: f64,
    /// P(F >= f).
    pub p: f64,
    /// Preferred by the F test at `alpha`.
    pub preferred_f: Preferred,
    /// AICc of the simpler model.
    pub aicc_simple: f64,
    /// AICc of the more complex model.
    pub aicc_complex: f64,
    /// AICc(simpler) - AICc(more complex) (Prism's sign).
    pub delta_aicc: f64,
    /// Akaike weight of the simpler model, exp(-D/2)/(1 + exp(-D/2)) with D its AICc
    /// minus the other's.
    pub prob_simple: f64,
    /// Akaike weight of the more complex model.
    pub prob_complex: f64,
    /// prob_complex / prob_simple.
    pub evidence_ratio: f64,
    /// Preferred by AICc.
    pub preferred_aicc: Preferred,
    /// One-line summary, e.g. "the more complex model is preferred (F test p = 0.0013;
    /// AICc: 99.19% probability)".
    pub summary: String,
}

/// Compare a simpler and a more complex fit of the same data: extra-sum-of-squares F test
/// (nested models; Motulsky & Christopoulos 2004 ch. 22) and AICc (Akaike weights;
/// Burnham & Anderson 2002). `alpha` is the F test's threshold (0.05).
pub fn compare_summaries(
    simple: FitSummary,
    complex: FitSummary,
    alpha: f64,
) -> Result<ModelComparison> {
    if simple.n != complex.n {
        return Err(invalid("models must be fitted to the same points"));
    }
    let df_num = simple.df - complex.df;
    let (f, p) = if df_num > 0.0 && complex.df > 0.0 {
        let f = ((simple.ss - complex.ss) / df_num) / (complex.ss / complex.df);
        let p = if f > 0.0 {
            p_upper_f(f, df_num, complex.df)
        } else {
            1.0
        };
        (f, p)
    } else {
        (f64::NAN, f64::NAN)
    };
    let a_s = simple.aicc();
    let a_c = complex.aicc();
    let d = a_c - a_s;
    // prob_complex = exp(-d/2)/(1 + exp(-d/2)) = 1/(1 + exp(d/2)), stably
    let prob_complex = if d > 0.0 {
        let e = (-0.5 * d).exp();
        e / (1.0 + e)
    } else {
        1.0 / (1.0 + (0.5 * d).exp())
    };
    let prob_simple = 1.0 - prob_complex;
    let preferred_f = if p < alpha {
        Preferred::MoreComplex
    } else {
        Preferred::Simpler
    };
    let preferred_aicc = if prob_complex > 0.5 {
        Preferred::MoreComplex
    } else {
        Preferred::Simpler
    };
    let name = |pr: Preferred| match pr {
        Preferred::Simpler => "simpler",
        Preferred::MoreComplex => "more complex",
    };
    let summary = format!(
        "F test: the {} model is preferred (p = {:.4}); AICc: the {} model is preferred ({:.2}% probability)",
        name(preferred_f),
        p,
        name(preferred_aicc),
        100.0 * prob_complex.max(prob_simple)
    );
    Ok(ModelComparison {
        f,
        df_num,
        df_den: complex.df,
        p,
        preferred_f,
        aicc_simple: a_s,
        aicc_complex: a_c,
        delta_aicc: a_s - a_c,
        prob_simple,
        prob_complex,
        evidence_ratio: prob_complex / prob_simple,
        preferred_aicc,
        summary,
    })
}

/// [`compare_summaries`] of two fits at alpha = 0.05.
pub fn compare_models(simple: &CurveFit, complex: &CurveFit) -> Result<ModelComparison> {
    compare_summaries(simple.summary(), complex.summary(), 0.05)
}

/// Shared versus separate parameters for a family of data sets.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FamilyFit {
    /// Each data set fitted alone (sharing removed; other constraints kept).
    pub separate: Vec<CurveFit>,
    /// The global fit with the shared parameters.
    pub global: CurveFit,
    /// Global (simpler: shared) versus separate (more complex: SS, df and parameters
    /// summed over the individual fits).
    pub comparison: ModelComparison,
}

/// Fit every data set separately and all together with the shared parameters of
/// `opts.constraints`, and compare (Prism: "Does one curve fit all data sets?" /
/// "Do the best-fit values of selected parameters differ between data sets?").
pub fn fit_family(eq: &Equation, data: &[Dataset], opts: &FitOptions) -> Result<FamilyFit> {
    if data.len() < 2 {
        return Err(invalid("need at least two data sets"));
    }
    let cs = constraints_of(eq, opts)?;
    if !cs.iter().any(|c| c.shared) {
        return Err(invalid("no parameter is shared"));
    }
    let global = fit_global(eq, data, opts)?;
    let mut separate = Vec::with_capacity(data.len());
    for (d, ds) in data.iter().enumerate() {
        let mut o = opts.clone();
        o.constraints = Some(
            cs.iter()
                .map(|c| {
                    let mut c = c.clone();
                    c.shared = false;
                    if let Some(v) = &c.fixed_per_dataset {
                        c.fixed = Some(v[d]);
                        c.fixed_per_dataset = None;
                    }
                    c
                })
                .collect(),
        );
        o.initial = opts
            .initial
            .as_ref()
            .and_then(|v| v.get(d))
            .map(|v| vec![v.clone()]);
        separate.push(fit(eq, ds, &o)?);
    }
    let complex = FitSummary {
        n: separate.iter().map(|f| f.n).sum(),
        k: separate.iter().map(|f| f.k).sum(),
        ss: separate.iter().map(|f| f.ss).sum(),
        df: separate.iter().map(|f| f.df).sum(),
    };
    let comparison = compare_summaries(global.summary(), complex, 0.05)?;
    Ok(FamilyFit {
        separate,
        global,
        comparison,
    })
}
