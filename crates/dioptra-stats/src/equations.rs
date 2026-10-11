//! The curve-fit equation library, with GraphPad Prism's equation and parameter names
//! (GraphPad Curve Fitting Guide, "Models built in to Prism"), analytic partial
//! derivatives, Prism-style rules for initial values, default constraints and derived
//! quantities (EC50 from LogEC50, half-life from K, ...), plus user-defined equations
//! parsed from text.
//!
//! Each equation is data: an [`Equation`] carries its parameters ([`ParamSpec`]: name,
//! [`InitialRule`], bounds, whether it is a data-set constant), derived quantities
//! ([`DerivedSpec`]), the scale its X is expected on ([`XTransform`]) and an evaluator.
//! Simple models have hand-written derivatives; the longer ones (5PL, bell-shaped,
//! two-site competition, ...) are written in the expression language and differentiated
//! exactly by forward-mode automatic differentiation, as are user equations
//! ([`Equation::custom`]). Equations given only as a Rust closure
//! ([`Equation::from_closure`]) use central finite differences.

use crate::expr::Compiled;
use crate::{Result, StatsError};
use std::f64::consts::{LN_10, LN_2};
use std::sync::Arc;

/// Statistics of a data set that initial-value rules read (Prism: YMIN, YMAX, YMID,
/// XMIN, XMAX, XMID, X at YMID, Y at XMIN, Y at XMAX, slope).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DataSummary {
    /// Number of points.
    pub n: usize,
    /// Smallest X.
    pub xmin: f64,
    /// Largest X.
    pub xmax: f64,
    /// (XMIN + XMAX)/2.
    pub xmid: f64,
    /// Smallest Y.
    pub ymin: f64,
    /// Largest Y.
    pub ymax: f64,
    /// (YMIN + YMAX)/2.
    pub ymid: f64,
    /// X where the data (sorted by X, replicates averaged) first cross YMID, by linear
    /// interpolation; the X of the point closest to YMID if they never cross.
    pub x_at_ymid: f64,
    /// X of the largest Y.
    pub x_at_ymax: f64,
    /// X of the smallest Y.
    pub x_at_ymin: f64,
    /// Mean Y at the smallest X.
    pub y_at_xmin: f64,
    /// Mean Y at the largest X.
    pub y_at_xmax: f64,
    /// Least-squares slope of Y on X.
    pub slope: f64,
    /// Least-squares intercept.
    pub intercept: f64,
}

impl DataSummary {
    /// Summarise (x, y); NaN pairs are skipped. Returns `None` without finite points.
    pub fn new(x: &[f64], y: &[f64]) -> Option<DataSummary> {
        let mut pts: Vec<(f64, f64)> = x
            .iter()
            .zip(y)
            .filter(|(a, b)| a.is_finite() && b.is_finite())
            .map(|(a, b)| (*a, *b))
            .collect();
        if pts.is_empty() {
            return None;
        }
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        // average replicates
        let mut means: Vec<(f64, f64)> = Vec::new();
        let mut i = 0;
        while i < pts.len() {
            let mut j = i;
            let mut s = 0.0;
            while j < pts.len() && pts[j].0 == pts[i].0 {
                s += pts[j].1;
                j += 1;
            }
            means.push((pts[i].0, s / (j - i) as f64));
            i = j;
        }
        let n = pts.len();
        let xmin = pts[0].0;
        let xmax = pts[n - 1].0;
        let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut x_at_ymin, mut x_at_ymax) = (xmin, xmin);
        for &(a, b) in &pts {
            if b < ymin {
                ymin = b;
                x_at_ymin = a;
            }
            if b > ymax {
                ymax = b;
                x_at_ymax = a;
            }
        }
        let ymid = 0.5 * (ymin + ymax);
        let mut x_at_ymid = f64::NAN;
        for w in means.windows(2) {
            let (a, b) = (w[0], w[1]);
            if (a.1 - ymid) * (b.1 - ymid) <= 0.0 && a.1 != b.1 {
                x_at_ymid = a.0 + (ymid - a.1) * (b.0 - a.0) / (b.1 - a.1);
                break;
            }
        }
        if !x_at_ymid.is_finite() {
            x_at_ymid = means
                .iter()
                .min_by(|a, b| (a.1 - ymid).abs().total_cmp(&(b.1 - ymid).abs()))
                .map_or(xmin, |p| p.0);
        }
        let nf = n as f64;
        let mx = pts.iter().map(|p| p.0).sum::<f64>() / nf;
        let my = pts.iter().map(|p| p.1).sum::<f64>() / nf;
        let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
        let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
        let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
        Some(DataSummary {
            n,
            xmin,
            xmax,
            xmid: 0.5 * (xmin + xmax),
            ymin,
            ymax,
            ymid,
            x_at_ymid,
            x_at_ymax,
            x_at_ymin,
            y_at_xmin: means[0].1,
            y_at_xmax: means[means.len() - 1].1,
            slope,
            intercept: my - slope * mx,
        })
    }

    fn half_life(&self) -> f64 {
        let range = (self.xmax - self.xmin).abs();
        let h = (self.x_at_ymid - self.xmin).abs();
        if h > 1e-6 * range && h.is_finite() {
            h
        } else if range > 0.0 {
            0.25 * range
        } else {
            1.0
        }
    }

    fn trend(&self) -> f64 {
        if self.y_at_xmax >= self.y_at_xmin {
            1.0
        } else {
            -1.0
        }
    }
}

/// Rule for a parameter's initial value (Prism's "rules for initial values": a multiplier
/// times a data statistic, or a constant).
#[derive(Debug, Clone, Copy)]
pub enum InitialRule {
    /// A fixed value.
    Value(f64),
    /// c * YMIN.
    YMin(f64),
    /// c * YMAX.
    YMax(f64),
    /// c * YMID.
    YMid(f64),
    /// c * XMIN.
    XMin(f64),
    /// c * XMAX.
    XMax(f64),
    /// c * XMID.
    XMid(f64),
    /// c * (X at YMID).
    XAtYMid(f64),
    /// c * (X at YMAX).
    XAtYMax(f64),
    /// c * (Y at XMIN).
    YAtXMin(f64),
    /// c * (Y at XMAX).
    YAtXMax(f64),
    /// c * SIGN(Y at XMAX - Y at XMIN) (Prism's rule for HillSlope).
    SignTrend(f64),
    /// c * ln 2 / (half-time estimate |X at YMID - XMIN|): a rate constant.
    Rate(f64),
    /// c * least-squares slope.
    Slope(f64),
    /// c * least-squares intercept.
    Intercept(f64),
    /// c * (XMAX - XMIN).
    XRange(f64),
    /// c * (YMAX - YMIN).
    YRange(f64),
    /// Any function of the summary.
    Custom(fn(&DataSummary) -> f64),
}

impl InitialRule {
    /// Evaluate the rule.
    pub fn eval(&self, s: &DataSummary) -> f64 {
        match *self {
            InitialRule::Value(v) => v,
            InitialRule::YMin(c) => c * s.ymin,
            InitialRule::YMax(c) => c * s.ymax,
            InitialRule::YMid(c) => c * s.ymid,
            InitialRule::XMin(c) => c * s.xmin,
            InitialRule::XMax(c) => c * s.xmax,
            InitialRule::XMid(c) => c * s.xmid,
            InitialRule::XAtYMid(c) => c * s.x_at_ymid,
            InitialRule::XAtYMax(c) => c * s.x_at_ymax,
            InitialRule::YAtXMin(c) => c * s.y_at_xmin,
            InitialRule::YAtXMax(c) => c * s.y_at_xmax,
            InitialRule::SignTrend(c) => c * s.trend(),
            InitialRule::Rate(c) => c * LN_2 / s.half_life(),
            InitialRule::Slope(c) => c * s.slope,
            InitialRule::Intercept(c) => c * s.intercept,
            InitialRule::XRange(c) => c * (s.xmax - s.xmin),
            InitialRule::YRange(c) => c * (s.ymax - s.ymin),
            InitialRule::Custom(f) => f(s),
        }
    }
}

/// A parameter of an equation.
#[derive(Debug, Clone)]
pub struct ParamSpec {
    /// Prism's name.
    pub name: String,
    /// Initial-value rule.
    pub initial: InitialRule,
    /// Default lower bound (-inf for none).
    pub lower: f64,
    /// Default upper bound (+inf for none).
    pub upper: f64,
    /// A data-set constant (Prism: inhibitor or radioligand concentration): never
    /// fitted; its value must be given per data set in the fit's constraints.
    pub constant: bool,
}

impl ParamSpec {
    fn new(name: &str, initial: InitialRule) -> Self {
        ParamSpec {
            name: name.to_string(),
            initial,
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
            constant: false,
        }
    }
    fn constant(name: &str) -> Self {
        ParamSpec {
            constant: true,
            ..ParamSpec::new(name, InitialRule::Value(f64::NAN))
        }
    }
}

/// How a derived quantity is computed from the parameters.
#[derive(Debug, Clone, Copy)]
pub enum DerivedKind {
    /// A monotone function of one parameter: its confidence interval is the transformed
    /// interval of the parameter (Prism: EC50 from LogEC50, half-life from K).
    Transform {
        /// Index of the parameter.
        param: usize,
        /// The function.
        f: fn(f64) -> f64,
    },
    /// A function of several parameters: SE by the delta method (central-difference
    /// gradient), symmetric t interval.
    Function(fn(&[f64]) -> f64),
}

/// A derived quantity reported with the fit.
#[derive(Debug, Clone)]
pub struct DerivedSpec {
    /// Name, e.g. "EC50" or "HalfLife".
    pub name: String,
    /// How it is computed.
    pub kind: DerivedKind,
}

/// The scale an equation expects X on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum XTransform {
    /// X as entered.
    None,
    /// X is log10 of a concentration (Prism's "log(agonist)" families); data entered as
    /// concentrations must be transformed with `log10` before fitting.
    Log10,
}

type ClosureFn = Arc<dyn Fn(&[f64], &[f64]) -> f64 + Send + Sync>;

#[derive(Clone)]
enum Kind {
    /// Hand-written value and gradient (one X).
    Builtin(fn(f64, &[f64], &mut [f64]) -> f64),
    /// Expression with automatic differentiation.
    Expr(Arc<Compiled>),
    /// Closure; gradient by central differences.
    Closure(ClosureFn),
}

/// A curve-fit equation.
#[derive(Clone)]
pub struct Equation {
    /// Prism's name, e.g. "log(agonist) vs. response -- Variable slope (four parameters)".
    pub name: String,
    /// Family ("Dose-response - Stimulation", "Exponential", ...).
    pub family: String,
    /// The model as text.
    pub formula: String,
    /// Parameters.
    pub params: Vec<ParamSpec>,
    /// Derived quantities.
    pub derived: Vec<DerivedSpec>,
    /// Scale of X.
    pub x_transform: XTransform,
    /// Names of the independent variables (["X"] for every built-in equation).
    pub variables: Vec<String>,
    /// Linear in the parameters (straight lines and polynomials): fitted by linear least
    /// squares before any iteration.
    pub linear: bool,
    kind: Kind,
}

impl std::fmt::Debug for Equation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Equation")
            .field("name", &self.name)
            .field("formula", &self.formula)
            .field("params", &self.param_names())
            .finish()
    }
}

impl Equation {
    /// Number of parameters (data-set constants included).
    pub fn n_params(&self) -> usize {
        self.params.len()
    }

    /// Parameter names.
    pub fn param_names(&self) -> Vec<String> {
        self.params.iter().map(|p| p.name.clone()).collect()
    }

    /// Index of a parameter by name (case-insensitive).
    pub fn param_index(&self, name: &str) -> Option<usize> {
        self.params
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Whether the gradient is exact (hand-written or automatic differentiation) rather
    /// than finite differences.
    pub fn has_exact_gradient(&self) -> bool {
        !matches!(self.kind, Kind::Closure(_))
    }

    /// f(x, p); `x` holds one value per independent variable.
    pub fn eval(&self, x: &[f64], p: &[f64]) -> f64 {
        match &self.kind {
            Kind::Builtin(f) => {
                let mut g = [0.0; 16];
                f(x[0], p, &mut g[..p.len()])
            }
            Kind::Expr(c) => c.eval(x, p),
            Kind::Closure(f) => f(x, p),
        }
    }

    /// f(x, p) and the gradient with respect to p (length n_params).
    pub fn eval_grad(&self, x: &[f64], p: &[f64], grad: &mut [f64]) -> f64 {
        match &self.kind {
            Kind::Builtin(f) => f(x[0], p, grad),
            Kind::Expr(c) => c.eval_grad(x, p, grad),
            Kind::Closure(f) => {
                let v = f(x, p);
                fd_gradient(|q| f(x, q), p, grad, false);
                v
            }
        }
    }

    /// The gradient by central differences (with Richardson extrapolation when
    /// `richardson`), whatever the equation kind: the finite-difference Jacobian mode.
    pub fn eval_grad_fd(&self, x: &[f64], p: &[f64], grad: &mut [f64], richardson: bool) -> f64 {
        let v = self.eval(x, p);
        fd_gradient(|q| self.eval(x, q), p, grad, richardson);
        v
    }

    /// Initial values from the rules, for data (x, y).
    pub fn initial_values(&self, x: &[f64], y: &[f64]) -> Vec<f64> {
        match DataSummary::new(x, y) {
            Some(s) => self.params.iter().map(|p| p.initial.eval(&s)).collect(),
            None => vec![f64::NAN; self.params.len()],
        }
    }

    /// A user-defined equation in one variable `X`, parameters in order of first
    /// appearance. See the module `expr` grammar: `+ - * / ^`, functions
    /// `exp ln log log10 log2 sqrt abs sin cos tan asin acos atan sinh cosh tanh pow`,
    /// constants `pi`, `e` (`log` is base 10, as in Prism). Derivatives are exact (forward
    /// automatic differentiation). Initial values default to 1 (supply them in the fit
    /// options).
    pub fn custom(expr: &str) -> Result<Equation> {
        Self::custom_with(expr, &["X"], None)
    }

    /// A user-defined equation with named independent variables and, optionally, an
    /// explicit parameter order (identifiers outside `params` are then an error).
    pub fn custom_with(
        expr: &str,
        variables: &[&str],
        params: Option<&[&str]>,
    ) -> Result<Equation> {
        if variables.is_empty() {
            return Err(StatsError::InvalidArgument(
                "an equation needs at least one independent variable".into(),
            ));
        }
        let vars: Vec<String> = variables.iter().map(|s| s.to_string()).collect();
        let pnames: Option<Vec<String>> = params.map(|p| p.iter().map(|s| s.to_string()).collect());
        let c = Compiled::parse(expr, &vars, pnames.as_deref())?;
        if c.n_params == 0 {
            return Err(StatsError::InvalidArgument(
                "the equation has no parameters".into(),
            ));
        }
        if c.n_params > 64 {
            return Err(StatsError::InvalidArgument(
                "more than 64 parameters".into(),
            ));
        }
        Ok(Equation {
            name: "User-defined".into(),
            family: "User-defined".into(),
            formula: expr.to_string(),
            params: c
                .params
                .iter()
                .map(|n| ParamSpec::new(n, InitialRule::Value(1.0)))
                .collect(),
            derived: vec![],
            x_transform: XTransform::None,
            variables: vars,
            linear: false,
            kind: Kind::Expr(Arc::new(c)),
        })
    }

    /// An equation given as a closure f(x, p) with named parameters; the gradient is by
    /// central finite differences.
    pub fn from_closure<F>(name: &str, params: &[&str], f: F) -> Equation
    where
        F: Fn(&[f64], &[f64]) -> f64 + Send + Sync + 'static,
    {
        Equation {
            name: name.to_string(),
            family: "User-defined".into(),
            formula: String::new(),
            params: params
                .iter()
                .map(|n| ParamSpec::new(n, InitialRule::Value(1.0)))
                .collect(),
            derived: vec![],
            x_transform: XTransform::None,
            variables: vec!["X".into()],
            linear: false,
            kind: Kind::Closure(Arc::new(f)),
        }
    }

    /// Set the initial-value rule of a parameter (builder style).
    pub fn with_initial(mut self, name: &str, rule: InitialRule) -> Self {
        if let Some(i) = self.param_index(name) {
            self.params[i].initial = rule;
        }
        self
    }

    /// Set default bounds of a parameter (builder style).
    pub fn with_bounds(mut self, name: &str, lower: f64, upper: f64) -> Self {
        if let Some(i) = self.param_index(name) {
            self.params[i].lower = lower;
            self.params[i].upper = upper;
        }
        self
    }

    /// Value of each derived quantity at p.
    pub fn derived_values(&self, p: &[f64]) -> Vec<(String, f64)> {
        self.derived
            .iter()
            .map(|d| {
                let v = match d.kind {
                    DerivedKind::Transform { param, f } => f(p[param]),
                    DerivedKind::Function(f) => f(p),
                };
                (d.name.clone(), v)
            })
            .collect()
    }
}

/// Central-difference gradient; h = eps^(1/3) max(|p|, 1) (the optimum for central
/// differences), or with one Richardson extrapolation from h and h/2 at
/// h = eps^(1/5) max(|p|, 1) (truncation O(h^4)).
fn fd_gradient<F: Fn(&[f64]) -> f64>(f: F, p: &[f64], grad: &mut [f64], richardson: bool) {
    let mut q = p.to_vec();
    for j in 0..p.len() {
        let scale = p[j].abs().max(1.0);
        let central = |q: &mut Vec<f64>, h: f64| -> f64 {
            q[j] = p[j] + h;
            let hp = q[j] - p[j];
            let fp = f(q);
            q[j] = p[j] - h;
            let hm = p[j] - q[j];
            let fm = f(q);
            q[j] = p[j];
            (fp - fm) / (hp + hm)
        };
        grad[j] = if richardson {
            let h = f64::EPSILON.powf(0.2) * scale;
            let d1 = central(&mut q, h);
            let d2 = central(&mut q, 0.5 * h);
            (4.0 * d2 - d1) / 3.0
        } else {
            central(&mut q, f64::EPSILON.cbrt() * scale)
        };
    }
}

// =======================================================================================
// Numerically safe pieces
// =======================================================================================

/// s = 1/(1 + e^z) and s(1 - s), without overflow.
#[inline]
fn logistic(z: f64) -> (f64, f64) {
    if z > 0.0 {
        let e = (-z).exp();
        let s = e / (1.0 + e);
        (s, s / (1.0 + e))
    } else {
        let e = z.exp();
        let s = 1.0 / (1.0 + e);
        (s, s * e / (1.0 + e))
    }
}

// =======================================================================================
// Hand-written models: value and gradient
// =======================================================================================

fn f_line(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    g[0] = 1.0;
    g[1] = x;
    p[0] + p[1] * x
}

fn f_origin(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    g[0] = x;
    p[0] * x
}

fn f_poly(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let mut v = 0.0;
    let mut xp = 1.0;
    for (j, pj) in p.iter().enumerate() {
        g[j] = xp;
        v += pj * xp;
        xp *= x;
    }
    v
}

fn f_one_phase_decay(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (y0, pl, k) = (p[0], p[1], p[2]);
    let e = (-k * x).exp();
    g[0] = e;
    g[1] = 1.0 - e;
    g[2] = -(y0 - pl) * x * e;
    (y0 - pl) * e + pl
}

fn f_one_phase_assoc(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (y0, pl, k) = (p[0], p[1], p[2]);
    let e = (-k * x).exp();
    g[0] = e;
    g[1] = 1.0 - e;
    g[2] = (pl - y0) * x * e;
    y0 + (pl - y0) * (1.0 - e)
}

fn f_two_phase_decay(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (y0, pl, pf, kf, ks) = (p[0], p[1], p[2], p[3], p[4]);
    let s = y0 - pl;
    let f = 0.01 * pf;
    let ef = (-kf * x).exp();
    let es = (-ks * x).exp();
    g[0] = f * ef + (1.0 - f) * es;
    g[1] = 1.0 - f * ef - (1.0 - f) * es;
    g[2] = 0.01 * s * (ef - es);
    g[3] = -s * f * x * ef;
    g[4] = -s * (1.0 - f) * x * es;
    pl + s * f * ef + s * (1.0 - f) * es
}

fn f_two_phase_assoc(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (y0, pl, pf, kf, ks) = (p[0], p[1], p[2], p[3], p[4]);
    let s = pl - y0;
    let f = 0.01 * pf;
    let ef = (-kf * x).exp();
    let es = (-ks * x).exp();
    let (af, as_) = (1.0 - ef, 1.0 - es);
    g[0] = 1.0 - f * af - (1.0 - f) * as_;
    g[1] = f * af + (1.0 - f) * as_;
    g[2] = 0.01 * s * (es - ef);
    g[3] = s * f * x * ef;
    g[4] = s * (1.0 - f) * x * es;
    y0 + s * f * af + s * (1.0 - f) * as_
}

fn f_plateau_decay(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (x0, y0, pl, k) = (p[0], p[1], p[2], p[3]);
    if x < x0 {
        g[0] = 0.0;
        g[1] = 1.0;
        g[2] = 0.0;
        g[3] = 0.0;
        return y0;
    }
    let e = (-k * (x - x0)).exp();
    g[0] = (y0 - pl) * k * e;
    g[1] = e;
    g[2] = 1.0 - e;
    g[3] = -(y0 - pl) * (x - x0) * e;
    pl + (y0 - pl) * e
}

fn f_plateau_assoc(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (x0, y0, pl, k) = (p[0], p[1], p[2], p[3]);
    if x < x0 {
        g[0] = 0.0;
        g[1] = 1.0;
        g[2] = 0.0;
        g[3] = 0.0;
        return y0;
    }
    let e = (-k * (x - x0)).exp();
    g[0] = -(pl - y0) * k * e;
    g[1] = e;
    g[2] = 1.0 - e;
    g[3] = (pl - y0) * (x - x0) * e;
    y0 + (pl - y0) * (1.0 - e)
}

fn f_exp_growth(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let e = (p[1] * x).exp();
    g[0] = e;
    g[1] = p[0] * x * e;
    p[0] * e
}

/// Bottom + (Top-Bottom)/(1 + 10^((L - x) h)) with parameters (Bottom, Top, L[, h]).
fn sigmoid_log(x: f64, b: f64, t: f64, l: f64, h: f64, g: &mut [f64], with_h: bool) -> f64 {
    let z = (l - x) * h * LN_10;
    let (s, sp) = logistic(z);
    g[0] = 1.0 - s;
    g[1] = s;
    g[2] = -(t - b) * sp * h * LN_10;
    if with_h {
        g[3] = -(t - b) * sp * (l - x) * LN_10;
    }
    b + (t - b) * s
}

fn f_log_agonist(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    sigmoid_log(x, p[0], p[1], p[2], 1.0, g, false)
}

fn f_4pl(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    sigmoid_log(x, p[0], p[1], p[2], p[3], g, true)
}

fn f_log_inhibitor(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    sigmoid_log(x, p[0], p[1], p[2], -1.0, g, false)
}

/// One site -- Fit logIC50, parameters (Top, Bottom, LogIC50).
fn f_one_site_logic50(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let mut h = [0.0; 3];
    let v = sigmoid_log(x, p[1], p[0], p[2], -1.0, &mut h, false);
    g[0] = h[1];
    g[1] = h[0];
    g[2] = h[2];
    v
}

fn f_agonist_linear(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (b, t, e) = (p[0], p[1], p[2]);
    let d = e + x;
    g[0] = 1.0 - x / d;
    g[1] = x / d;
    g[2] = -x * (t - b) / (d * d);
    b + x * (t - b) / d
}

fn f_mm(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (v, k) = (p[0], p[1]);
    let d = k + x;
    g[0] = x / d;
    g[1] = -v * x / (d * d);
    v * x / d
}

fn f_substrate_inhibition(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (v, km, ki) = (p[0], p[1], p[2]);
    let d = km + x * (1.0 + x / ki);
    g[0] = x / d;
    g[1] = -v * x / (d * d);
    g[2] = v * x * x * x / (ki * ki * d * d);
    v * x / d
}

/// V x^h/(K^h + x^h), parameters (V, K, h).
fn f_hill(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (v, k, h) = (p[0], p[1], p[2]);
    if x <= 0.0 {
        let val = if x == 0.0 && h > 0.0 { 0.0 } else { f64::NAN };
        g[0] = val;
        g[1] = 0.0;
        g[2] = 0.0;
        return val;
    }
    // x^h/(K^h + x^h) = 1/(1 + (K/x)^h) = logistic(h ln(K/x))
    let z = h * (k / x).ln();
    let (s, sp) = logistic(z);
    g[0] = s;
    g[1] = -v * sp * h / k;
    g[2] = -v * sp * (k / x).ln();
    v * s
}

fn f_competitive(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (v, km, ki, i) = (p[0], p[1], p[2], p[3]);
    let kobs = km * (1.0 + i / ki);
    let d = kobs + x;
    let q = v * x / (d * d);
    g[0] = x / d;
    g[1] = -q * (1.0 + i / ki);
    g[2] = q * km * i / (ki * ki);
    g[3] = -q * km / ki;
    v * x / d
}

fn f_one_site_total(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (bmax, kd, ns, bg) = (p[0], p[1], p[2], p[3]);
    let d = kd + x;
    g[0] = x / d;
    g[1] = -bmax * x / (d * d);
    g[2] = x;
    g[3] = 1.0;
    bmax * x / d + ns * x + bg
}

fn f_gaussian(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (a, m, s) = (p[0], p[1], p[2]);
    let z = (x - m) / s;
    let e = (-0.5 * z * z).exp();
    g[0] = e;
    g[1] = a * e * z / s;
    g[2] = a * e * z * z / s;
    a * e
}

fn f_lorentzian(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (a, c, w) = (p[0], p[1], p[2]);
    let q = (x - c) / w;
    let d = 1.0 + q * q;
    g[0] = 1.0 / d;
    g[1] = a * 2.0 * q / (w * d * d);
    g[2] = a * 2.0 * q * q / (w * d * d);
    a / d
}

fn f_sine(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (a, lam, phi) = (p[0], p[1], p[2]);
    let w = 2.0 * std::f64::consts::PI;
    let th = w * x / lam + phi;
    let (s, c) = th.sin_cos();
    g[0] = s;
    g[1] = -a * c * w * x / (lam * lam);
    g[2] = a * c;
    a * s
}

fn f_sine_baseline(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let v = f_sine(x, &p[1..], &mut g[1..]);
    g[0] = 1.0;
    p[0] + v
}

fn f_power(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (a, b) = (p[0], p[1]);
    let xb = x.powf(b);
    g[0] = xb;
    g[1] = if xb == 0.0 { 0.0 } else { a * xb * x.ln() };
    a * xb
}

fn f_boltzmann(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (b, t, v50, sl) = (p[0], p[1], p[2], p[3]);
    let z = (v50 - x) / sl;
    let (s, sp) = logistic(z);
    g[0] = 1.0 - s;
    g[1] = s;
    g[2] = -(t - b) * sp / sl;
    g[3] = (t - b) * sp * z / sl;
    b + (t - b) * s
}

fn f_segmental(x: f64, p: &[f64], g: &mut [f64]) -> f64 {
    let (x0, s1, s2, i1) = (p[0], p[1], p[2], p[3]);
    g[3] = 1.0;
    if x < x0 {
        g[0] = 0.0;
        g[1] = x;
        g[2] = 0.0;
        i1 + s1 * x
    } else {
        g[0] = s1 - s2;
        g[1] = x0;
        g[2] = x - x0;
        i1 + s1 * x0 + s2 * (x - x0)
    }
}

// =======================================================================================
// The library
// =======================================================================================

#[allow(clippy::too_many_arguments)]
fn builtin(
    name: &str,
    family: &str,
    formula: &str,
    params: Vec<ParamSpec>,
    f: fn(f64, &[f64], &mut [f64]) -> f64,
    derived: Vec<DerivedSpec>,
    xt: XTransform,
) -> Equation {
    Equation {
        name: name.into(),
        family: family.into(),
        formula: formula.into(),
        params,
        derived,
        x_transform: xt,
        variables: vec!["X".into()],
        linear: false,
        kind: Kind::Builtin(f),
    }
}

fn expr_builtin(
    name: &str,
    family: &str,
    formula: &str,
    params: Vec<ParamSpec>,
    derived: Vec<DerivedSpec>,
    xt: XTransform,
) -> Equation {
    let names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
    let c = Compiled::parse(formula, &["X".to_string()], Some(&names))
        .expect("built-in formula parses");
    Equation {
        name: name.into(),
        family: family.into(),
        formula: formula.into(),
        params,
        derived,
        x_transform: xt,
        variables: vec!["X".into()],
        linear: false,
        kind: Kind::Expr(Arc::new(c)),
    }
}

fn p(name: &str, r: InitialRule) -> ParamSpec {
    ParamSpec::new(name, r)
}

fn tr(name: &str, param: usize, f: fn(f64) -> f64) -> DerivedSpec {
    DerivedSpec {
        name: name.into(),
        kind: DerivedKind::Transform { param, f },
    }
}

fn fun(name: &str, f: fn(&[f64]) -> f64) -> DerivedSpec {
    DerivedSpec {
        name: name.into(),
        kind: DerivedKind::Function(f),
    }
}

fn pow10(v: f64) -> f64 {
    10f64.powf(v)
}
fn half_life(k: f64) -> f64 {
    LN_2 / k
}
fn recip(k: f64) -> f64 {
    1.0 / k
}

use InitialRule as R;

const DR_STIM: &str = "Dose-response - Stimulation";
const DR_INHIB: &str = "Dose-response - Inhibition";
const DR_SPECIAL: &str = "Dose-response - Special";
const EXPO: &str = "Exponential";
const BINDING_SAT: &str = "Binding - Saturation";
const BINDING_COMP: &str = "Binding - Competitive";
const ENZYME: &str = "Enzyme kinetics";
const LINES: &str = "Lines";
const POLY: &str = "Polynomial";
const GAUSS: &str = "Gaussian";
const SINE: &str = "Sine waves";
const GROWTH: &str = "Growth";
const CLASSIC: &str = "Classic equations";

/// Straight line `Y = YIntercept + Slope*X`.
pub fn straight_line() -> Equation {
    let mut e = builtin(
        "Straight line",
        LINES,
        "Y = YIntercept + Slope*X",
        vec![
            p("YIntercept", R::Intercept(1.0)),
            p("Slope", R::Slope(1.0)),
        ],
        f_line,
        vec![fun("XIntercept", |p| -p[0] / p[1])],
        XTransform::None,
    );
    e.linear = true;
    e
}

/// Line through the origin `Y = Slope*X`.
pub fn line_through_origin() -> Equation {
    let mut e = builtin(
        "Line through origin",
        LINES,
        "Y = Slope*X",
        vec![p("Slope", R::Slope(1.0))],
        f_origin,
        vec![],
        XTransform::None,
    );
    e.linear = true;
    e
}

/// Polynomial of order 1..=6, `Y = B0 + B1*X + ... + Bk*X^k`, fitted by linear least
/// squares.
pub fn polynomial(order: usize) -> Result<Equation> {
    if !(1..=6).contains(&order) {
        return Err(StatsError::InvalidArgument(
            "polynomial order must be 1 to 6".into(),
        ));
    }
    let names = ["First", "Second", "Third", "Fourth", "Fifth", "Sixth"];
    let params = (0..=order)
        .map(|j| {
            p(
                &format!("B{j}"),
                match j {
                    0 => R::Intercept(1.0),
                    1 => R::Slope(1.0),
                    _ => R::Value(0.0),
                },
            )
        })
        .collect();
    let formula = (0..=order)
        .map(|j| match j {
            0 => "B0".to_string(),
            1 => "B1*X".to_string(),
            _ => format!("B{j}*X^{j}"),
        })
        .collect::<Vec<_>>()
        .join(" + ");
    let mut e = builtin(
        &format!("{} order polynomial", names[order - 1]),
        POLY,
        &format!("Y = {formula}"),
        params,
        f_poly,
        vec![],
        XTransform::None,
    );
    e.linear = true;
    Ok(e)
}

fn rate_rule() -> InitialRule {
    R::Rate(1.0)
}

/// One phase decay `Y = (Y0 - Plateau)*exp(-K*X) + Plateau`; derived Span, HalfLife
/// (ln 2/K), Tau (1/K).
pub fn one_phase_decay() -> Equation {
    builtin(
        "One phase decay",
        EXPO,
        "Y = (Y0 - Plateau)*exp(-K*X) + Plateau",
        vec![
            p("Y0", R::YAtXMin(1.0)),
            p("Plateau", R::YAtXMax(1.0)),
            p("K", rate_rule()),
        ],
        f_one_phase_decay,
        vec![
            fun("Span", |p| p[0] - p[1]),
            tr("HalfLife", 2, half_life),
            tr("Tau", 2, recip),
        ],
        XTransform::None,
    )
}

/// One phase association `Y = Y0 + (Plateau - Y0)*(1 - exp(-K*X))`.
pub fn one_phase_association() -> Equation {
    builtin(
        "One phase association",
        EXPO,
        "Y = Y0 + (Plateau - Y0)*(1 - exp(-K*X))",
        vec![
            p("Y0", R::YAtXMin(1.0)),
            p("Plateau", R::YAtXMax(1.0)),
            p("K", rate_rule()),
        ],
        f_one_phase_assoc,
        vec![
            fun("Span", |p| p[1] - p[0]),
            tr("HalfTime", 2, half_life),
            tr("Tau", 2, recip),
        ],
        XTransform::None,
    )
}

/// Two phase decay: SpanFast = (Y0-Plateau)*PercentFast*.01, SpanSlow the rest,
/// `Y = Plateau + SpanFast*exp(-KFast*X) + SpanSlow*exp(-KSlow*X)`.
pub fn two_phase_decay() -> Equation {
    builtin(
        "Two phase decay",
        EXPO,
        "Y = Plateau + SpanFast*exp(-KFast*X) + SpanSlow*exp(-KSlow*X)",
        vec![
            p("Y0", R::YAtXMin(1.0)),
            p("Plateau", R::YAtXMax(1.0)),
            p("PercentFast", R::Value(50.0)),
            p("KFast", R::Rate(4.0)),
            p("KSlow", R::Rate(0.5)),
        ],
        f_two_phase_decay,
        vec![
            fun("SpanFast", |p| (p[0] - p[1]) * p[2] * 0.01),
            fun("SpanSlow", |p| (p[0] - p[1]) * (100.0 - p[2]) * 0.01),
            tr("HalfLifeFast", 3, half_life),
            tr("HalfLifeSlow", 4, half_life),
        ],
        XTransform::None,
    )
}

/// Two phase association.
pub fn two_phase_association() -> Equation {
    builtin(
        "Two phase association",
        EXPO,
        "Y = Y0 + SpanFast*(1 - exp(-KFast*X)) + SpanSlow*(1 - exp(-KSlow*X))",
        vec![
            p("Y0", R::YAtXMin(1.0)),
            p("Plateau", R::YAtXMax(1.0)),
            p("PercentFast", R::Value(50.0)),
            p("KFast", R::Rate(4.0)),
            p("KSlow", R::Rate(0.5)),
        ],
        f_two_phase_assoc,
        vec![
            fun("SpanFast", |p| (p[1] - p[0]) * p[2] * 0.01),
            fun("SpanSlow", |p| (p[1] - p[0]) * (100.0 - p[2]) * 0.01),
            tr("HalfTimeFast", 3, half_life),
            tr("HalfTimeSlow", 4, half_life),
        ],
        XTransform::None,
    )
}

/// Plateau followed by one phase decay `Y = IF(X<X0, Y0, Plateau+(Y0-Plateau)*exp(-K*(X-X0)))`.
pub fn plateau_then_one_phase_decay() -> Equation {
    builtin(
        "Plateau followed by one phase decay",
        EXPO,
        "Y = IF(X<X0, Y0, Plateau + (Y0-Plateau)*exp(-K*(X-X0)))",
        vec![
            p("X0", R::XMin(1.0)),
            p("Y0", R::YAtXMin(1.0)),
            p("Plateau", R::YAtXMax(1.0)),
            p("K", rate_rule()),
        ],
        f_plateau_decay,
        vec![tr("HalfLife", 3, half_life), tr("Tau", 3, recip)],
        XTransform::None,
    )
}

/// Plateau followed by one phase association.
pub fn plateau_then_one_phase_association() -> Equation {
    builtin(
        "Plateau followed by one phase association",
        EXPO,
        "Y = IF(X<X0, Y0, Y0 + (Plateau-Y0)*(1 - exp(-K*(X-X0))))",
        vec![
            p("X0", R::XMin(1.0)),
            p("Y0", R::YAtXMin(1.0)),
            p("Plateau", R::YAtXMax(1.0)),
            p("K", rate_rule()),
        ],
        f_plateau_assoc,
        vec![tr("HalfTime", 3, half_life), tr("Tau", 3, recip)],
        XTransform::None,
    )
}

/// Exponential growth `Y = Y0*exp(K*X)`; derived DoublingTime (ln 2/K).
pub fn exponential_growth() -> Equation {
    builtin(
        "Exponential growth",
        EXPO,
        "Y = Y0*exp(K*X)",
        vec![
            p("Y0", R::YAtXMin(1.0)),
            p(
                "K",
                R::Custom(|s| {
                    let r = (s.y_at_xmax / s.y_at_xmin).abs().ln() / (s.xmax - s.xmin);
                    if r.is_finite() {
                        r
                    } else {
                        1.0
                    }
                }),
            ),
        ],
        f_exp_growth,
        vec![tr("DoublingTime", 1, half_life)],
        XTransform::None,
    )
}

/// log(agonist) vs. response `Y = Bottom + (Top-Bottom)/(1+10^(LogEC50-X))` (Hill slope 1).
pub fn log_agonist_vs_response() -> Equation {
    builtin(
        "log(agonist) vs. response",
        DR_STIM,
        "Y = Bottom + (Top-Bottom)/(1+10^(LogEC50-X))",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("LogEC50", R::XAtYMid(1.0)),
        ],
        f_log_agonist,
        vec![tr("EC50", 2, pow10), fun("Span", |p| p[1] - p[0])],
        XTransform::Log10,
    )
}

/// log(agonist) vs. response -- Variable slope (four parameters), the 4PL:
/// `Y = Bottom + (Top-Bottom)/(1+10^((LogEC50-X)*HillSlope))`.
pub fn log_agonist_variable_slope() -> Equation {
    builtin(
        "log(agonist) vs. response -- Variable slope (four parameters)",
        DR_STIM,
        "Y = Bottom + (Top-Bottom)/(1+10^((LogEC50-X)*HillSlope))",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("LogEC50", R::XAtYMid(1.0)),
            p("HillSlope", R::SignTrend(1.0)),
        ],
        f_4pl,
        vec![tr("EC50", 2, pow10), fun("Span", |p| p[1] - p[0])],
        XTransform::Log10,
    )
}

/// log(agonist) vs. normalized response -- Variable slope `Y = 100/(1+10^((LogEC50-X)*HillSlope))`.
pub fn log_agonist_normalized_variable_slope() -> Equation {
    expr_builtin(
        "log(agonist) vs. normalized response -- Variable slope",
        DR_STIM,
        "100/(1+10^((LogEC50-X)*HillSlope))",
        vec![
            p("LogEC50", R::XAtYMid(1.0)),
            p("HillSlope", R::SignTrend(1.0)),
        ],
        vec![tr("EC50", 0, pow10)],
        XTransform::Log10,
    )
}

/// log(inhibitor) vs. response `Y = Bottom + (Top-Bottom)/(1+10^(X-LogIC50))`.
pub fn log_inhibitor_vs_response() -> Equation {
    builtin(
        "log(inhibitor) vs. response",
        DR_INHIB,
        "Y = Bottom + (Top-Bottom)/(1+10^(X-LogIC50))",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("LogIC50", R::XAtYMid(1.0)),
        ],
        f_log_inhibitor,
        vec![tr("IC50", 2, pow10), fun("Span", |p| p[1] - p[0])],
        XTransform::Log10,
    )
}

/// log(inhibitor) vs. response -- Variable slope (four parameters):
/// `Y = Bottom + (Top-Bottom)/(1+10^((LogIC50-X)*HillSlope))` (HillSlope < 0 for a
/// falling curve).
pub fn log_inhibitor_variable_slope() -> Equation {
    builtin(
        "log(inhibitor) vs. response -- Variable slope (four parameters)",
        DR_INHIB,
        "Y = Bottom + (Top-Bottom)/(1+10^((LogIC50-X)*HillSlope))",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("LogIC50", R::XAtYMid(1.0)),
            p("HillSlope", R::SignTrend(1.0)),
        ],
        f_4pl,
        vec![tr("IC50", 2, pow10), fun("Span", |p| p[1] - p[0])],
        XTransform::Log10,
    )
}

/// Asymmetric sigmoidal, five parameters (5PL), X is log(concentration):
/// `LogXb = LogEC50 + (1/HillSlope)*log(2^(1/S)-1)`;
/// `Y = Bottom + (Top-Bottom)/(1+10^((LogXb-X)*HillSlope))^S` (S = 1 is the 4PL).
pub fn asymmetric_5pl() -> Equation {
    expr_builtin(
        "Asymmetric Sigmoidal, 5PL, X is log(concentration)",
        DR_SPECIAL,
        "Bottom + (Top-Bottom)/(1+10^((LogEC50 + (1/HillSlope)*log(2^(1/S)-1) - X)*HillSlope))^S",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("LogEC50", R::XAtYMid(1.0)),
            p("HillSlope", R::SignTrend(1.0)),
            p("S", R::Value(1.0)),
        ],
        vec![tr("EC50", 2, pow10)],
        XTransform::Log10,
    )
}

/// [Agonist] vs. response `Y = Bottom + X*(Top-Bottom)/(EC50 + X)` (X a concentration).
pub fn agonist_vs_response() -> Equation {
    builtin(
        "[Agonist] vs. response",
        DR_STIM,
        "Y = Bottom + X*(Top-Bottom)/(EC50 + X)",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("EC50", R::XAtYMid(1.0)),
        ],
        f_agonist_linear,
        vec![fun("Span", |p| p[1] - p[0])],
        XTransform::None,
    )
}

/// [Agonist] vs. response -- Variable slope
/// `Y = Bottom + (X^HillSlope)*(Top-Bottom)/(X^HillSlope + EC50^HillSlope)`.
pub fn agonist_variable_slope() -> Equation {
    expr_builtin(
        "[Agonist] vs. response -- Variable slope",
        DR_STIM,
        "Bottom + (X^HillSlope)*(Top-Bottom)/(X^HillSlope + EC50^HillSlope)",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("EC50", R::XAtYMid(1.0)),
            p("HillSlope", R::Value(1.0)),
        ],
        vec![fun("Span", |p| p[1] - p[0])],
        XTransform::None,
    )
}

/// Bell-shaped dose-response:
/// `Y = Dip + (Plateau1-Dip)/(1+10^((LogEC50_1-X)*nH1)) + (Plateau2-Dip)/(1+10^((X-LogEC50_2)*nH2))`.
pub fn bell_shaped() -> Equation {
    expr_builtin(
        "Bell-shaped dose-response",
        DR_SPECIAL,
        "Dip + (Plateau1-Dip)/(1+10^((LogEC50_1-X)*nH1)) + (Plateau2-Dip)/(1+10^((X-LogEC50_2)*nH2))",
        vec![
            p("Plateau1", R::YAtXMin(1.0)),
            p("Dip", R::YMax(1.0)),
            p("Plateau2", R::YAtXMax(1.0)),
            p(
                "LogEC50_1",
                R::Custom(|s| s.xmin + 0.5 * (s.x_at_ymax - s.xmin)),
            ),
            p(
                "LogEC50_2",
                R::Custom(|s| s.x_at_ymax + 0.5 * (s.xmax - s.x_at_ymax)),
            ),
            p("nH1", R::Value(1.0)),
            p("nH2", R::Value(1.0)),
        ],
        vec![tr("EC50_1", 3, pow10), tr("EC50_2", 4, pow10)],
        XTransform::Log10,
    )
}

/// Michaelis-Menten `Y = Vmax*X/(Km + X)`.
pub fn michaelis_menten() -> Equation {
    builtin(
        "Michaelis-Menten",
        ENZYME,
        "Y = Vmax*X/(Km + X)",
        vec![p("Vmax", R::YMax(1.0)), p("Km", R::XAtYMid(1.0))],
        f_mm,
        vec![],
        XTransform::None,
    )
}

/// Substrate inhibition `Y = Vmax*X/(Km + X*(1+X/Ki))`.
pub fn substrate_inhibition() -> Equation {
    builtin(
        "Substrate inhibition",
        ENZYME,
        "Y = Vmax*X/(Km + X*(1+X/Ki))",
        vec![
            p("Vmax", R::YMax(2.0)),
            p("Km", R::XAtYMax(0.5)),
            p("Ki", R::XAtYMax(2.0)),
        ],
        f_substrate_inhibition,
        vec![],
        XTransform::None,
    )
}

/// Allosteric sigmoidal `Y = Vmax*X^h/(Khalf^h + X^h)`.
pub fn allosteric_sigmoidal() -> Equation {
    builtin(
        "Allosteric sigmoidal",
        ENZYME,
        "Y = Vmax*X^h/(Khalf^h + X^h)",
        vec![
            p("Vmax", R::YMax(1.0)),
            p("Khalf", R::XAtYMid(1.0)),
            p("h", R::Value(1.0)),
        ],
        f_hill,
        vec![],
        XTransform::None,
    )
}

/// Competitive inhibition `KmObs = Km*(1+I/Ki)`, `Y = Vmax*X/(KmObs + X)`; I (inhibitor
/// concentration) is a data-set constant.
pub fn competitive_inhibition() -> Equation {
    builtin(
        "Competitive inhibition",
        ENZYME,
        "KmObs = Km*(1+I/Ki); Y = Vmax*X/(KmObs + X)",
        vec![
            p("Vmax", R::YMax(1.0)),
            p("Km", R::XAtYMid(1.0)),
            p("Ki", R::XAtYMid(1.0)),
            ParamSpec::constant("I"),
        ],
        f_competitive,
        vec![],
        XTransform::None,
    )
}

/// One site -- Specific binding `Y = Bmax*X/(Kd + X)`.
pub fn one_site_specific() -> Equation {
    builtin(
        "One site -- Specific binding",
        BINDING_SAT,
        "Y = Bmax*X/(Kd + X)",
        vec![p("Bmax", R::YMax(1.0)), p("Kd", R::XAtYMid(1.0))],
        f_mm,
        vec![],
        XTransform::None,
    )
}

/// Specific binding with Hill slope `Y = Bmax*X^h/(Kd^h + X^h)`.
pub fn specific_binding_hill() -> Equation {
    builtin(
        "Specific binding with Hill slope",
        BINDING_SAT,
        "Y = Bmax*X^h/(Kd^h + X^h)",
        vec![
            p("Bmax", R::YMax(1.0)),
            p("Kd", R::XAtYMid(1.0)),
            p("h", R::Value(1.0)),
        ],
        f_hill,
        vec![],
        XTransform::None,
    )
}

/// One site -- Total `Y = Bmax*X/(Kd + X) + NS*X + Background`.
pub fn one_site_total() -> Equation {
    builtin(
        "One site -- Total",
        BINDING_SAT,
        "Y = Bmax*X/(Kd + X) + NS*X + Background",
        vec![
            p("Bmax", R::YMax(1.0)),
            p("Kd", R::XAtYMid(1.0)),
            p(
                "NS",
                R::Custom(|s| 0.1 * (s.ymax - s.ymin) / (s.xmax - s.xmin).max(1e-300)),
            ),
            p("Background", R::YMin(1.0)),
        ],
        f_one_site_total,
        vec![],
        XTransform::None,
    )
}

/// One site -- Fit logIC50 `Y = Bottom + (Top-Bottom)/(1+10^(X-LogIC50))`, parameters
/// in Prism's order Top, Bottom, LogIC50; see [`cheng_prusoff_ki`].
pub fn one_site_fit_logic50() -> Equation {
    builtin(
        "One site -- Fit logIC50",
        BINDING_COMP,
        "Y = Bottom + (Top-Bottom)/(1+10^(X-LogIC50))",
        vec![
            p("Top", R::YMax(1.0)),
            p("Bottom", R::YMin(1.0)),
            p("LogIC50", R::XAtYMid(1.0)),
        ],
        f_one_site_logic50,
        vec![tr("IC50", 2, pow10)],
        XTransform::Log10,
    )
}

/// One site -- Fit Ki: `logEC50 = log(10^logKi*(1+HotNM/HotKdNM))`,
/// `Y = Bottom + (Top-Bottom)/(1+10^(X-LogEC50))`; HotNM (radioligand concentration)
/// and HotKdNM (its Kd) are data-set constants (Cheng & Prusoff 1973 inside the model).
pub fn one_site_fit_ki() -> Equation {
    expr_builtin(
        "One site -- Fit Ki",
        BINDING_COMP,
        "Bottom + (Top-Bottom)/(1+10^(X-log(10^logKi*(1+HotNM/HotKdNM))))",
        vec![
            p("Top", R::YMax(1.0)),
            p("Bottom", R::YMin(1.0)),
            p("logKi", R::XAtYMid(1.0)),
            ParamSpec::constant("HotNM"),
            ParamSpec::constant("HotKdNM"),
        ],
        vec![tr("Ki", 2, pow10)],
        XTransform::Log10,
    )
}

/// Two sites -- Fit logIC50:
/// `Y = Bottom + (Top-Bottom)*(FractionOne/(1+10^(X-LogIC50_1)) + (1-FractionOne)/(1+10^(X-LogIC50_2)))`.
pub fn two_sites_fit_logic50() -> Equation {
    expr_builtin(
        "Two sites -- Fit logIC50",
        BINDING_COMP,
        "Bottom + (Top-Bottom)*(FractionOne/(1+10^(X-LogIC50_1)) + (1-FractionOne)/(1+10^(X-LogIC50_2)))",
        vec![
            p("Top", R::YMax(1.0)),
            p("Bottom", R::YMin(1.0)),
            p("FractionOne", R::Value(0.5)),
            p("LogIC50_1", R::Custom(|s| s.x_at_ymid - 1.0)),
            p("LogIC50_2", R::Custom(|s| s.x_at_ymid + 1.0)),
        ],
        vec![tr("IC50_1", 3, pow10), tr("IC50_2", 4, pow10)],
        XTransform::Log10,
    )
}

/// Gaussian `Y = Amplitude*exp(-0.5*((X-Mean)/SD)^2)`.
pub fn gaussian() -> Equation {
    builtin(
        "Gaussian",
        GAUSS,
        "Y = Amplitude*exp(-0.5*((X-Mean)/SD)^2)",
        vec![
            p("Amplitude", R::YMax(1.0)),
            p("Mean", R::XAtYMax(1.0)),
            p("SD", R::XRange(0.2)),
        ],
        f_gaussian,
        vec![fun("Area", |p| {
            p[0] * p[2].abs() * (2.0 * std::f64::consts::PI).sqrt()
        })],
        XTransform::None,
    )
}

/// Lorentzian `Y = Amplitude/(1+((X-Center)/Width)^2)`.
pub fn lorentzian() -> Equation {
    builtin(
        "Lorentzian",
        GAUSS,
        "Y = Amplitude/(1+((X-Center)/Width)^2)",
        vec![
            p("Amplitude", R::YMax(1.0)),
            p("Center", R::XAtYMax(1.0)),
            p("Width", R::XRange(0.1)),
        ],
        f_lorentzian,
        vec![],
        XTransform::None,
    )
}

/// Standard sine wave `Y = Amplitude*sin((2*pi*X/Wavelength)+PhaseShift)`.
pub fn sine_wave() -> Equation {
    builtin(
        "Standard sine wave",
        SINE,
        "Y = Amplitude*sin((2*pi*X/Wavelength)+PhaseShift)",
        vec![
            p("Amplitude", R::YRange(0.5)),
            p("Wavelength", R::XRange(0.5)),
            p("PhaseShift", R::Value(0.0)),
        ],
        f_sine,
        vec![tr("Frequency", 1, recip)],
        XTransform::None,
    )
}

/// Sine wave with nonzero baseline `Y = Baseline + Amplitude*sin((2*pi*X/Wavelength)+PhaseShift)`.
pub fn sine_wave_baseline() -> Equation {
    builtin(
        "Sine wave with nonzero baseline",
        SINE,
        "Y = Baseline + Amplitude*sin((2*pi*X/Wavelength)+PhaseShift)",
        vec![
            p("Baseline", R::YMid(1.0)),
            p("Amplitude", R::YRange(0.5)),
            p("Wavelength", R::XRange(0.5)),
            p("PhaseShift", R::Value(0.0)),
        ],
        f_sine_baseline,
        vec![tr("Frequency", 2, recip)],
        XTransform::None,
    )
}

/// Power `Y = A*X^B`.
pub fn power() -> Equation {
    builtin(
        "Power",
        CLASSIC,
        "Y = A*X^B",
        vec![p("A", R::YMax(1.0)), p("B", R::Value(1.0))],
        f_power,
        vec![],
        XTransform::None,
    )
}

/// Power series `Y = A*X^B + C*X^D`.
pub fn power_series() -> Equation {
    expr_builtin(
        "Power series",
        CLASSIC,
        "A*X^B + C*X^D",
        vec![
            p("A", R::YMax(1.0)),
            p("B", R::Value(1.0)),
            p("C", R::Value(0.0)),
            p("D", R::Value(2.0)),
        ],
        vec![],
        XTransform::None,
    )
}

/// Boltzmann sigmoid `Y = Bottom + (Top-Bottom)/(1+exp((V50-X)/Slope))`.
pub fn boltzmann_sigmoid() -> Equation {
    builtin(
        "Boltzmann sigmoid",
        CLASSIC,
        "Y = Bottom + (Top-Bottom)/(1+exp((V50-X)/Slope))",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("V50", R::XAtYMid(1.0)),
            p("Slope", R::Custom(|s| 0.1 * (s.xmax - s.xmin) * s.trend())),
        ],
        f_boltzmann,
        vec![],
        XTransform::None,
    )
}

/// Segmental linear regression: `Y1 = Intercept1 + Slope1*X`,
/// `Y2 = Intercept1 + Slope1*X0 + Slope2*(X - X0)`, `Y = IF(X<X0, Y1, Y2)`; derived
/// Intercept2.
pub fn segmental_linear() -> Equation {
    builtin(
        "Segmental linear regression",
        LINES,
        "Y = IF(X<X0, Intercept1 + Slope1*X, Intercept1 + Slope1*X0 + Slope2*(X-X0))",
        vec![
            p("X0", R::XMid(1.0)),
            p("Slope1", R::Slope(1.0)),
            p("Slope2", R::Slope(1.0)),
            p("Intercept1", R::Intercept(1.0)),
        ],
        f_segmental,
        vec![fun("Intercept2", |p| p[3] + (p[1] - p[2]) * p[0])],
        XTransform::None,
    )
}

/// Log-logistic, four parameters (the `drc` LL.4 model with X a concentration):
/// `Y = Bottom + (Top-Bottom)/(1+exp(Slope*(ln(X)-ln(EC50))))`.
pub fn log_logistic() -> Equation {
    expr_builtin(
        "Log-logistic (four parameters)",
        DR_SPECIAL,
        "Bottom + (Top-Bottom)/(1+exp(Slope*(ln(X)-ln(EC50))))",
        vec![
            p("Bottom", R::YMin(1.0)),
            p("Top", R::YMax(1.0)),
            p("EC50", R::XAtYMid(1.0)),
            p("Slope", R::SignTrend(-1.0)),
        ],
        vec![],
        XTransform::None,
    )
}

/// Logistic growth `Y = YM*Y0/((YM-Y0)*exp(-K*X) + Y0)`.
pub fn logistic_growth() -> Equation {
    expr_builtin(
        "Logistic growth",
        GROWTH,
        "YM*Y0/((YM-Y0)*exp(-K*X) + Y0)",
        vec![
            p("YM", R::YMax(1.0)),
            p("Y0", R::YAtXMin(1.0)),
            p("K", rate_rule()),
        ],
        vec![tr("DoublingTime", 2, half_life)],
        XTransform::None,
    )
}

/// Gompertz growth `Y = YM*(Y0/YM)^(exp(-K*X))`.
pub fn gompertz_growth() -> Equation {
    expr_builtin(
        "Gompertz growth",
        GROWTH,
        "YM*(Y0/YM)^(exp(-K*X))",
        vec![
            p("YM", R::YMax(1.0)),
            p("Y0", R::YAtXMin(1.0)),
            p("K", rate_rule()),
        ],
        vec![],
        XTransform::None,
    )
}

/// Every built-in equation (polynomials of order 1 to 6 included).
pub fn library() -> Vec<Equation> {
    let mut v = vec![straight_line(), line_through_origin()];
    for k in 1..=6 {
        v.push(polynomial(k).expect("order 1..6"));
    }
    v.extend([
        segmental_linear(),
        one_phase_decay(),
        two_phase_decay(),
        plateau_then_one_phase_decay(),
        one_phase_association(),
        two_phase_association(),
        plateau_then_one_phase_association(),
        exponential_growth(),
        log_agonist_vs_response(),
        log_agonist_variable_slope(),
        log_agonist_normalized_variable_slope(),
        log_inhibitor_vs_response(),
        log_inhibitor_variable_slope(),
        asymmetric_5pl(),
        agonist_vs_response(),
        agonist_variable_slope(),
        bell_shaped(),
        log_logistic(),
        michaelis_menten(),
        substrate_inhibition(),
        allosteric_sigmoidal(),
        competitive_inhibition(),
        one_site_specific(),
        specific_binding_hill(),
        one_site_total(),
        one_site_fit_logic50(),
        one_site_fit_ki(),
        two_sites_fit_logic50(),
        gaussian(),
        lorentzian(),
        sine_wave(),
        sine_wave_baseline(),
        power(),
        power_series(),
        boltzmann_sigmoid(),
        logistic_growth(),
        gompertz_growth(),
    ]);
    v
}

/// A built-in equation by its Prism name (case-insensitive), or a short alias: "4PL",
/// "3PL", "5PL", "line", "MM".
pub fn by_name(name: &str) -> Option<Equation> {
    let alias = match name.to_ascii_lowercase().as_str() {
        "4pl" => Some(log_agonist_variable_slope()),
        "3pl" => Some(log_agonist_vs_response()),
        "5pl" => Some(asymmetric_5pl()),
        "line" => Some(straight_line()),
        "mm" => Some(michaelis_menten()),
        _ => None,
    };
    alias.or_else(|| {
        library()
            .into_iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
    })
}

/// Cheng & Prusoff (1973) Ki from an IC50: Ki = IC50/(1 + [L]/Kd), with [L] the
/// radioligand concentration and Kd its dissociation constant (same units).
pub fn cheng_prusoff_ki(ic50: f64, ligand: f64, kd: f64) -> f64 {
    ic50 / (1.0 + ligand / kd)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Analytic gradients of every built-in equation against Richardson differences.
    #[test]
    fn gradients_match_finite_differences() {
        for e in library() {
            let np = e.n_params();
            for trial in 0..4 {
                let pv: Vec<f64> = (0..np)
                    .map(|j| {
                        let n = &e.params[j].name;
                        let base = 0.7 + 0.37 * ((j * 7 + trial * 3) % 5) as f64;
                        if n.starts_with("Log") || n.starts_with("log") {
                            -1.0 - 0.3 * base
                        } else if n == "PercentFast" || n == "FractionOne" {
                            30.0 * base / 100.0 * if n == "PercentFast" { 100.0 } else { 1.0 }
                        } else if n == "HillSlope" && trial % 2 == 1 {
                            -base
                        } else {
                            base
                        }
                    })
                    .collect();
                let x = if e.x_transform == XTransform::Log10 {
                    -1.3 + 0.2 * trial as f64
                } else {
                    0.4 + 0.55 * trial as f64
                };
                let mut g = vec![0.0; np];
                let mut gf = vec![0.0; np];
                let v = e.eval_grad(&[x], &pv, &mut g);
                let v2 = e.eval_grad_fd(&[x], &pv, &mut gf, true);
                assert!((v - v2).abs() <= 1e-14 * (1.0 + v.abs()), "{}", e.name);
                assert!((v - e.eval(&[x], &pv)).abs() <= 1e-14 * (1.0 + v.abs()));
                for j in 0..np {
                    assert!(
                        (g[j] - gf[j]).abs() <= 1e-7 * (1.0 + g[j].abs()),
                        "{} d/d{}: analytic {} vs fd {} at x={x}, p={pv:?}",
                        e.name,
                        e.params[j].name,
                        g[j],
                        gf[j]
                    );
                }
            }
        }
    }

    #[test]
    fn five_pl_reduces_to_four_pl() {
        let a = asymmetric_5pl();
        let b = log_agonist_variable_slope();
        for x in [-8.0, -6.5, -5.0] {
            let p5 = [2.0, 98.0, -6.2, 1.3, 1.0];
            let v5 = a.eval(&[x], &p5);
            let v4 = b.eval(&[x], &p5[..4]);
            assert!((v5 - v4).abs() < 1e-12, "{v5} {v4}");
        }
    }

    #[test]
    fn initial_rules() {
        let x = [-9.0, -8.0, -7.0, -6.0, -5.0, -4.0];
        let y = [1.0, 5.0, 20.0, 70.0, 95.0, 99.0];
        let e = log_agonist_variable_slope();
        let p0 = e.initial_values(&x, &y);
        assert_eq!(p0[0], 1.0);
        assert_eq!(p0[1], 99.0);
        assert!(p0[2] > -7.0 && p0[2] < -6.0);
        assert_eq!(p0[3], 1.0);
        assert!(by_name("4pl").is_some());
        assert!(by_name("One phase decay").is_some());
        assert!((cheng_prusoff_ki(10.0, 1.0, 1.0) - 5.0).abs() < 1e-15);
    }
}
