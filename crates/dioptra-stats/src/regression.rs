//! Linear regression: simple (with through-origin option, bands, diagnostics, runs and
//! replicates tests), multiple and polynomial (refined Householder QR, aliased-column
//! detection, VIFs), and Deming regression.
//!
//! Numerics: simple regression works on data shifted by the first observation with
//! centred, compensated sums; multiple and polynomial regression use the refined QR of
//! `linalg` (equilibrated, pivoted Householder QR with double-double iterative refinement
//! of the augmented system, Björck 1967), never the normal equations. Polynomial designs
//! carry x^k to double-double precision. Rows with a missing value are dropped.

use crate::descriptive::{compensated_sum, mean_of, ss_of};
use crate::dist::{p_two_sided_t, p_upper_f, StudentT};
use crate::linalg::{dd_mul, rank_select, rss_of, Acc, Design, LeastSquares};
use crate::special;
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

fn check_level(l: f64) -> Result<()> {
    if !(l > 0.0 && l < 1.0) {
        return Err(invalid(format!("ci_level {l} not in (0, 1)")));
    }
    Ok(())
}

fn t_crit(level: f64, df: f64) -> f64 {
    let a = 1.0 - level;
    StudentT { df }.quantile_pq(1.0 - 0.5 * a, 0.5 * a)
}

// =======================================================================================
// Simple linear regression
// =======================================================================================

/// Options for [`simple_linear_regression`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimpleRegressionOptions {
    /// Force the line through the origin (no intercept).
    pub through_origin: bool,
    /// Confidence level of parameter CIs and bands.
    pub ci_level: f64,
    /// Slope value tested by `slope_test` (Prism: "is the slope different from ...").
    pub hypothesized_slope: f64,
}

impl Default for SimpleRegressionOptions {
    fn default() -> Self {
        SimpleRegressionOptions {
            through_origin: false,
            ci_level: 0.95,
            hypothesized_slope: 0.0,
        }
    }
}

/// Wald–Wolfowitz runs test on the signs of the residuals in x order.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunsTest {
    /// Number of runs.
    pub runs: usize,
    /// Positive residuals.
    pub n_positive: usize,
    /// Negative residuals (exact zeros are dropped).
    pub n_negative: usize,
    /// E[R] = 1 + 2 n+ n- / N.
    pub expected_runs: f64,
    /// Normal approximation z = (R - E[R]) / sd(R), no continuity correction.
    pub z: f64,
    /// Exact P(R <= runs) (Swed & Eisenhart 1943): small values mean too few runs, i.e.
    /// systematic deviation from the line (Prism's one-sided p).
    pub p_exact_too_few_runs: f64,
}

/// Replicates (lack-of-fit) test, available when some x values repeat.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LackOfFit {
    /// Residual SS minus pure-error SS.
    pub ss_lack_of_fit: f64,
    /// Number of distinct x minus number of parameters.
    pub df_lack_of_fit: f64,
    /// Sum over x groups of the SS of y about the group mean.
    pub ss_pure_error: f64,
    /// N minus number of distinct x.
    pub df_pure_error: f64,
    /// F = (SS_lof/df_lof)/(SS_pe/df_pe).
    pub f: f64,
    /// P(F >= f): small means the line does not fit the group means.
    pub p: f64,
}

/// Test of the slope against a hypothesised value.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SlopeTest {
    /// Hypothesised slope b0.
    pub value: f64,
    /// t = (b - b0)/SE(b).
    pub t: f64,
    /// Two-sided p.
    pub p: f64,
}

/// Simple linear regression result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimpleRegression {
    /// Complete (x, y) pairs used.
    pub n: usize,
    /// Whether the line was forced through the origin.
    pub through_origin: bool,
    /// Slope.
    pub slope: f64,
    /// Intercept (0 through the origin).
    pub intercept: f64,
    /// SE of the slope.
    pub se_slope: f64,
    /// SE of the intercept (NaN through the origin).
    pub se_intercept: f64,
    /// t of the slope (H0: 0).
    pub t_slope: f64,
    /// Two-sided p of the slope.
    pub p_slope: f64,
    /// t of the intercept.
    pub t_intercept: f64,
    /// Two-sided p of the intercept.
    pub p_intercept: f64,
    /// CI of the slope.
    pub ci_slope: (f64, f64),
    /// CI of the intercept.
    pub ci_intercept: (f64, f64),
    /// Pearson r (signed sqrt of R^2 for the intercept model).
    pub r: f64,
    /// R^2: centred for the intercept model; uncentred (sum y^2 as total) through the origin,
    /// as R `lm`, statsmodels and NIST.
    pub r_squared: f64,
    /// 1 - SS_res / sum (y - ybar)^2 for both models (Prism's R^2 through the origin, which
    /// can be negative).
    pub r_squared_centered: f64,
    /// Adjusted R^2.
    pub adj_r_squared: f64,
    /// Residual SD Sy.x = sqrt(SS_res/df).
    pub sy_x: f64,
    /// Residual sum of squares.
    pub ss_residual: f64,
    /// Regression sum of squares.
    pub ss_regression: f64,
    /// Residual df (n - 2, or n - 1 through the origin).
    pub df_residual: f64,
    /// F for the slope.
    pub f: f64,
    /// p of F.
    pub p_f: f64,
    /// -intercept/slope (NaN if the slope is 0).
    pub x_intercept: f64,
    /// Slope against the hypothesised value.
    pub slope_test: SlopeTest,
    /// Residuals y - fit, in input order of the complete pairs.
    pub residuals: Vec<f64>,
    /// Fitted values.
    pub fitted: Vec<f64>,
    /// Leverages h_i.
    pub leverage: Vec<f64>,
    /// Internally studentised residuals e_i/(s sqrt(1 - h_i)).
    pub standardized_residuals: Vec<f64>,
    /// Cook's distances e_i^2 h_i/(p s^2 (1 - h_i)^2).
    pub cooks_distance: Vec<f64>,
    /// Durbin–Watson statistic on the residuals in input order.
    pub durbin_watson: f64,
    /// Runs test (residuals ordered by x).
    pub runs_test: RunsTest,
    /// Replicates test, when x has repeated values and enough distinct x.
    pub lack_of_fit: Option<LackOfFit>,
    /// Mean of x.
    pub x_mean: f64,
    /// Sum (x - xbar)^2 (or sum x^2 through the origin).
    pub sxx: f64,
    /// Confidence level used.
    pub ci_level: f64,
}

/// Confidence and prediction bands at given x.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bands {
    /// The x values.
    pub x: Vec<f64>,
    /// Fitted values.
    pub fit: Vec<f64>,
    /// Lower confidence band of the line.
    pub ci_lower: Vec<f64>,
    /// Upper confidence band of the line.
    pub ci_upper: Vec<f64>,
    /// Lower prediction band for a new observation.
    pub pi_lower: Vec<f64>,
    /// Upper prediction band.
    pub pi_upper: Vec<f64>,
}

impl SimpleRegression {
    /// Confidence band yhat +/- t s sqrt(1/n + (x0 - xbar)^2/Sxx) and prediction band with
    /// 1 + ... under the root (through the origin: x0^2/sum x^2), at the fit's level.
    pub fn bands(&self, x0: &[f64]) -> Bands {
        let tc = t_crit(self.ci_level, self.df_residual);
        let nf = self.n as f64;
        let mut b = Bands {
            x: x0.to_vec(),
            fit: vec![],
            ci_lower: vec![],
            ci_upper: vec![],
            pi_lower: vec![],
            pi_upper: vec![],
        };
        for &x in x0 {
            let f = self.intercept + self.slope * x;
            let h = if self.through_origin {
                x * x / self.sxx
            } else {
                1.0 / nf + (x - self.x_mean).powi(2) / self.sxx
            };
            let c = tc * self.sy_x * h.sqrt();
            let p = tc * self.sy_x * (1.0 + h).sqrt();
            b.fit.push(f);
            b.ci_lower.push(f - c);
            b.ci_upper.push(f + c);
            b.pi_lower.push(f - p);
            b.pi_upper.push(f + p);
        }
        b
    }
}

fn complete_xy(x: &[f64], y: &[f64]) -> Result<(Vec<f64>, Vec<f64>)> {
    crate::correlation::pairs(x, y, "regression")
}

/// Simple linear regression by least squares (Gauss; any regression text, e.g. Zar 2010
/// ch. 17; Prism's "simple linear regression"), with: parameter SEs, t tests and CIs; R^2
/// (and Prism's centred R^2 through the origin); F; Sy.x; x-intercept; test of the slope
/// against a hypothesised value; residuals, leverage, studentised residuals and Cook's
/// distance (Cook 1977); Durbin–Watson (1950); the runs test with the exact Swed–Eisenhart
/// (1943, doi:10.1214/aoms/1177731178) distribution; and the replicates (lack-of-fit) F test
/// (Draper & Smith 1998 §2.1) when x repeats. Use [`SimpleRegression::bands`] for bands.
pub fn simple_linear_regression(
    x: &[f64],
    y: &[f64],
    opts: SimpleRegressionOptions,
) -> Result<SimpleRegression> {
    check_level(opts.ci_level)?;
    let (x, y) = complete_xy(x, y)?;
    let n = x.len();
    let origin = opts.through_origin;
    let npar = if origin { 1 } else { 2 };
    if n <= npar {
        return Err(StatsError::NotEnoughData(format!(
            "simple regression needs more than {npar} points"
        )));
    }
    let nf = n as f64;
    let ym = mean_of(&y);
    let syy_c = ss_of(&y, ym);
    // Sxx and the mean of x (for bands and leverage) from shifted, centred sums.
    let x_mean = mean_of(&x);
    let sxx = if origin {
        let mut a = Acc::default();
        for v in &x {
            a.add_prod(*v, *v);
        }
        a.value()
    } else {
        let c = x[0];
        let dx: Vec<f64> = x.iter().map(|v| v - c).collect();
        ss_of(&dx, mean_of(&dx))
    };
    if !(sxx > 0.0) {
        return Err(invalid(if origin {
            "regression: all x are zero"
        } else {
            "regression: all x are equal"
        }));
    }
    // Coefficients, residuals and (X'X)^{-1} from the refined QR (the closed form
    // ybar - b xbar loses digits to cancellation when |xbar b| >> |a|, as in NIST Norris).
    let cols: Vec<Vec<f64>> = if origin {
        vec![x.clone()]
    } else {
        vec![vec![1.0; n], x.clone()]
    };
    let design = Design::from_columns(&cols);
    let ls = LeastSquares::new(&design);
    let sol = ls.fit(&y);
    let inv = ls.xtx_inverse();
    let (slope, intercept) = if origin {
        (sol.coef[0], 0.0)
    } else {
        (sol.coef[1], sol.coef[0])
    };
    let resid = sol.resid;
    let rss = sol.rss;
    let df = (n - npar) as f64;
    let s2 = rss / df;
    let s = s2.sqrt();
    let (ss_reg, sst) = if origin {
        let mut a = Acc::default();
        for v in &y {
            a.add_prod(*v, *v);
        }
        let sst = a.value();
        (sst - rss, sst)
    } else {
        (slope * slope * sxx, syy_c)
    };
    let r2 = ss_reg / sst;
    let r2c = 1.0 - rss / syy_c;
    let adj = 1.0 - (1.0 - r2) * (nf - if origin { 0.0 } else { 1.0 }) / df;
    let se_slope = s * inv[if origin { 0 } else { 3 }].sqrt();
    let se_int = if origin { f64::NAN } else { s * inv[0].sqrt() };
    let tc = t_crit(opts.ci_level, df);
    let t_slope = slope / se_slope;
    let t_int = if origin { f64::NAN } else { intercept / se_int };
    let f = ss_reg / s2;
    let fitted: Vec<f64> = y.iter().zip(&resid).map(|(a, e)| a - e).collect();
    let leverage: Vec<f64> = x
        .iter()
        .map(|&v| {
            if origin {
                v * v / sxx
            } else {
                1.0 / nf + (v - x_mean).powi(2) / sxx
            }
        })
        .collect();
    let stdres: Vec<f64> = resid
        .iter()
        .zip(&leverage)
        .map(|(e, h)| e / (s * (1.0 - h).sqrt()))
        .collect();
    let cooks: Vec<f64> = resid
        .iter()
        .zip(&leverage)
        .map(|(e, h)| e * e * h / (npar as f64 * s2 * (1.0 - h).powi(2)))
        .collect();
    let dw = compensated_sum(resid.windows(2).map(|w| (w[1] - w[0]).powi(2))) / rss;
    let slope_t = (slope - opts.hypothesized_slope) / se_slope;
    Ok(SimpleRegression {
        n,
        through_origin: origin,
        slope,
        intercept,
        se_slope,
        se_intercept: se_int,
        t_slope,
        p_slope: p_two_sided_t(t_slope, df),
        t_intercept: t_int,
        p_intercept: if origin {
            f64::NAN
        } else {
            p_two_sided_t(t_int, df)
        },
        ci_slope: (slope - tc * se_slope, slope + tc * se_slope),
        ci_intercept: if origin {
            (0.0, 0.0)
        } else {
            (intercept - tc * se_int, intercept + tc * se_int)
        },
        r: slope.signum() * r2.max(0.0).sqrt(),
        r_squared: r2,
        r_squared_centered: r2c,
        adj_r_squared: adj,
        sy_x: s,
        ss_residual: rss,
        ss_regression: ss_reg,
        df_residual: df,
        f,
        p_f: p_upper_f(f, 1.0, df),
        x_intercept: if slope != 0.0 {
            -intercept / slope
        } else {
            f64::NAN
        },
        slope_test: SlopeTest {
            value: opts.hypothesized_slope,
            t: slope_t,
            p: p_two_sided_t(slope_t, df),
        },
        runs_test: runs_test_by_x(&x, &resid),
        lack_of_fit: lack_of_fit(&x, &y, rss, npar),
        residuals: resid,
        fitted,
        leverage,
        standardized_residuals: stdres,
        cooks_distance: cooks,
        durbin_watson: dw,
        x_mean,
        sxx,
        ci_level: opts.ci_level,
    })
}

/// Runs test of residual signs ordered by x (stable: ties keep input order); exact zeros
/// are dropped.
pub fn runs_test_by_x(x: &[f64], resid: &[f64]) -> RunsTest {
    let mut idx: Vec<usize> = (0..x.len().min(resid.len())).collect();
    idx.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let signs: Vec<bool> = idx
        .iter()
        .filter(|&&i| resid[i] != 0.0)
        .map(|&i| resid[i] > 0.0)
        .collect();
    runs_test(&signs)
}

/// Wald–Wolfowitz runs test on a sequence of signs (Wald & Wolfowitz 1940; exact lower
/// tail from Swed & Eisenhart 1943).
pub fn runs_test(signs: &[bool]) -> RunsTest {
    let n1 = signs.iter().filter(|&&s| s).count();
    let n2 = signs.len() - n1;
    let runs = if signs.is_empty() {
        0
    } else {
        1 + signs.windows(2).filter(|w| w[0] != w[1]).count()
    };
    let (a, b) = (n1 as f64, n2 as f64);
    let nn = a + b;
    let expected = 1.0 + 2.0 * a * b / nn;
    let var = 2.0 * a * b * (2.0 * a * b - nn) / (nn * nn * (nn - 1.0));
    RunsTest {
        runs,
        n_positive: n1,
        n_negative: n2,
        expected_runs: expected,
        z: (runs as f64 - expected) / var.sqrt(),
        p_exact_too_few_runs: runs_cdf(runs, n1, n2),
    }
}

/// P(R <= r) for the number of runs with n1 and n2 symbols (Swed & Eisenhart 1943):
/// P(R = 2k) = 2 C(n1-1, k-1) C(n2-1, k-1) / C(N, n1),
/// P(R = 2k+1) = [C(n1-1, k) C(n2-1, k-1) + C(n1-1, k-1) C(n2-1, k)] / C(N, n1).
pub fn runs_cdf(r: usize, n1: usize, n2: usize) -> f64 {
    if n1 == 0 || n2 == 0 {
        return 1.0;
    }
    let lc = |n: usize, k: isize| -> Option<f64> {
        if k < 0 || k as usize > n {
            None
        } else {
            Some(special::ln_choose(n as f64, k as f64))
        }
    };
    let ltot = special::ln_choose((n1 + n2) as f64, n1 as f64);
    let term = |a: Option<f64>, b: Option<f64>| -> f64 {
        match (a, b) {
            (Some(x), Some(y)) => (x + y - ltot).exp(),
            _ => 0.0,
        }
    };
    let mut terms = Vec::new();
    for rr in 2..=r {
        let k = (rr / 2) as isize;
        if rr % 2 == 0 {
            terms.push(2.0 * term(lc(n1 - 1, k - 1), lc(n2 - 1, k - 1)));
        } else {
            terms.push(
                term(lc(n1 - 1, k), lc(n2 - 1, k - 1)) + term(lc(n1 - 1, k - 1), lc(n2 - 1, k)),
            );
        }
    }
    compensated_sum(terms).min(1.0)
}

pub(crate) fn lack_of_fit(x: &[f64], y: &[f64], rss: f64, npar: usize) -> Option<LackOfFit> {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let mut groups: Vec<Vec<f64>> = Vec::new();
    let mut last = f64::NAN;
    for &i in &idx {
        if x[i] == last {
            groups.last_mut().unwrap().push(y[i]);
        } else {
            groups.push(vec![y[i]]);
            last = x[i];
        }
    }
    let m = groups.len();
    let n = x.len();
    if m == n || m <= npar {
        return None;
    }
    let ss_pe = compensated_sum(groups.iter().map(|g| ss_of(g, mean_of(g))));
    let df_pe = (n - m) as f64;
    let df_lof = (m - npar) as f64;
    let ss_lof = (rss - ss_pe).max(0.0);
    let f = (ss_lof / df_lof) / (ss_pe / df_pe);
    Some(LackOfFit {
        ss_lack_of_fit: ss_lof,
        df_lack_of_fit: df_lof,
        ss_pure_error: ss_pe,
        df_pure_error: df_pe,
        f,
        p: if f.is_finite() {
            p_upper_f(f, df_lof, df_pe)
        } else {
            f64::NAN
        },
    })
}

// =======================================================================================
// Multiple and polynomial regression
// =======================================================================================

/// Options for [`multiple_regression`] and [`polynomial_regression`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MultipleRegressionOptions {
    /// Include an intercept column (first coefficient).
    pub intercept: bool,
    /// Confidence level of the coefficient CIs.
    pub ci_level: f64,
}

impl Default for MultipleRegressionOptions {
    fn default() -> Self {
        MultipleRegressionOptions {
            intercept: true,
            ci_level: 0.95,
        }
    }
}

/// Multiple linear regression result. Coefficient vectors have the intercept first (if
/// fitted), then one entry per predictor; aliased coefficients are NaN.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MultipleRegression {
    /// Rows used.
    pub n: usize,
    /// Whether an intercept was fitted.
    pub intercept: bool,
    /// Coefficients.
    pub coefficients: Vec<f64>,
    /// Standard errors.
    pub se: Vec<f64>,
    /// t statistics.
    pub t: Vec<f64>,
    /// Two-sided p-values.
    pub p: Vec<f64>,
    /// CIs.
    pub ci: Vec<(f64, f64)>,
    /// Indices (into `coefficients`) of columns found linearly dependent on earlier ones.
    pub aliased: Vec<usize>,
    /// Rank of the design.
    pub rank: usize,
    /// R^2 (centred with an intercept, uncentred without, as R and statsmodels).
    pub r_squared: f64,
    /// Adjusted R^2 = 1 - (1 - R^2)(n - i)/df_residual, i = 1 with an intercept.
    pub adj_r_squared: f64,
    /// Regression sum of squares.
    pub ss_regression: f64,
    /// Residual sum of squares.
    pub ss_residual: f64,
    /// Total sum of squares (centred with an intercept).
    pub ss_total: f64,
    /// Regression df (rank - 1 with an intercept).
    pub df_regression: f64,
    /// Residual df n - rank.
    pub df_residual: f64,
    /// SS_regression / df_regression.
    pub ms_regression: f64,
    /// SS_residual / df_residual.
    pub ms_residual: f64,
    /// Overall F.
    pub f: f64,
    /// p of F.
    pub p_f: f64,
    /// Residual SD sqrt(MS_residual).
    pub sy_x: f64,
    /// Variance inflation factors 1/(1 - R_j^2) per coefficient (NaN for the intercept
    /// and aliased columns).
    pub vif: Vec<f64>,
    /// Covariance matrix of the coefficients (row-major, NaN rows/cols for aliased).
    pub covariance: Vec<Vec<f64>>,
    /// Residuals.
    pub residuals: Vec<f64>,
    /// Fitted values.
    pub fitted: Vec<f64>,
    /// Confidence level.
    pub ci_level: f64,
}

/// Multiple linear regression of `y` on predictor rows `x_rows[i] = [x_i1, ..., x_ip]`
/// (an intercept column is added when `opts.intercept`). Least squares by the refined
/// Householder QR of `linalg` (Golub 1965; Björck 1967); columns that are (numerically)
/// linear combinations of earlier columns are reported in `aliased` and dropped, as R `lm`
/// does. SEs from s^2 (X'X)^{-1} computed column-wise by the same refined solver; VIF_j =
/// 1/(1 - R_j^2) of predictor j on the others (Marquardt 1970). Oracle: statsmodels OLS.
pub fn multiple_regression(
    x_rows: &[Vec<f64>],
    y: &[f64],
    opts: MultipleRegressionOptions,
) -> Result<MultipleRegression> {
    if x_rows.len() != y.len() {
        return Err(invalid(
            "multiple regression: x and y have different lengths",
        ));
    }
    let p = x_rows.first().map_or(0, |r| r.len());
    if x_rows.iter().any(|r| r.len() != p) {
        return Err(invalid("multiple regression: ragged predictor rows"));
    }
    let keep: Vec<usize> = (0..y.len())
        .filter(|&i| !y[i].is_nan() && x_rows[i].iter().all(|v| !v.is_nan()))
        .collect();
    if keep
        .iter()
        .any(|&i| y[i].is_infinite() || x_rows[i].iter().any(|v| v.is_infinite()))
    {
        return Err(invalid("multiple regression: infinite value"));
    }
    let n = keep.len();
    let mut cols: Vec<Vec<f64>> = Vec::new();
    if opts.intercept {
        cols.push(vec![1.0; n]);
    }
    cols.extend((0..p).map(|j| keep.iter().map(|&i| x_rows[i][j]).collect::<Vec<f64>>()));
    let yv: Vec<f64> = keep.iter().map(|&i| y[i]).collect();
    fit_design(Design::from_columns(&cols), &yv, opts)
}

/// Polynomial regression y = b0 + b1 x + ... + b_d x^d (b0 omitted without an intercept).
/// The powers x^k are formed in double-double arithmetic so that the refinement sees the
/// design to about 32 digits; otherwise as [`multiple_regression`].
pub fn polynomial_regression(
    x: &[f64],
    y: &[f64],
    degree: usize,
    opts: MultipleRegressionOptions,
) -> Result<MultipleRegression> {
    if degree == 0 {
        return Err(invalid("polynomial regression: degree must be >= 1"));
    }
    let (x, y) = complete_xy(x, y)?;
    let n = x.len();
    let mut hi: Vec<f64> = Vec::new();
    let mut lo: Vec<f64> = Vec::new();
    if opts.intercept {
        hi.extend(std::iter::repeat_n(1.0, n));
        lo.extend(std::iter::repeat_n(0.0, n));
    }
    let mut ph = x.clone();
    let mut pl = vec![0.0; n];
    for k in 1..=degree {
        if k > 1 {
            for i in 0..n {
                let (h, l) = dd_mul(ph[i], pl[i], x[i], 0.0);
                ph[i] = h;
                pl[i] = l;
            }
        }
        hi.extend_from_slice(&ph);
        lo.extend_from_slice(&pl);
    }
    let p = degree + usize::from(opts.intercept);
    let d = Design {
        n,
        p,
        hi,
        lo: Some(lo),
    };
    fit_design(d, &y, opts)
}

fn fit_design(d: Design, y: &[f64], opts: MultipleRegressionOptions) -> Result<MultipleRegression> {
    check_level(opts.ci_level)?;
    let n = d.n;
    let ptot = d.p;
    if ptot == 0 {
        return Err(invalid("regression: no columns"));
    }
    let (kept, aliased) = rank_select(&d);
    let rank = kept.len();
    if n <= rank {
        return Err(StatsError::NotEnoughData(format!(
            "regression needs more rows ({n}) than parameters ({rank})"
        )));
    }
    let sub = d.select(&kept);
    let ls = LeastSquares::new(&sub);
    let sol = ls.fit(y);
    let inv = ls.xtx_inverse();
    let df_res = (n - rank) as f64;
    let rss = sol.rss;
    let ms_res = rss / df_res;
    let s = ms_res.sqrt();
    let sst = if opts.intercept {
        let c = y[0];
        let dy: Vec<f64> = y.iter().map(|v| v - c).collect();
        ss_of(&dy, mean_of(&dy))
    } else {
        let mut a = Acc::default();
        for v in y {
            a.add_prod(*v, *v);
        }
        a.value()
    };
    let ssr = sst - rss;
    let df_reg = (rank - usize::from(opts.intercept)) as f64;
    let ms_reg = ssr / df_reg;
    let f = ms_reg / ms_res;
    let r2 = ssr / sst;
    let nf = n as f64;
    let adj = 1.0 - (1.0 - r2) * (nf - if opts.intercept { 1.0 } else { 0.0 }) / df_res;
    let tc = t_crit(opts.ci_level, df_res);
    let mut coef = vec![f64::NAN; ptot];
    let mut se = vec![f64::NAN; ptot];
    let mut cov = vec![vec![f64::NAN; ptot]; ptot];
    for (a, &ja) in kept.iter().enumerate() {
        coef[ja] = sol.coef[a];
        se[ja] = s * inv[a * rank + a].sqrt();
        for (b, &jb) in kept.iter().enumerate() {
            cov[ja][jb] = ms_res * inv[a * rank + b];
        }
    }
    let t: Vec<f64> = coef.iter().zip(&se).map(|(b, e)| b / e).collect();
    let p: Vec<f64> = t
        .iter()
        .map(|&v| {
            if v.is_nan() {
                f64::NAN
            } else {
                p_two_sided_t(v, df_res)
            }
        })
        .collect();
    let ci = coef
        .iter()
        .zip(&se)
        .map(|(b, e)| (b - tc * e, b + tc * e))
        .collect();
    // VIF of each kept predictor on the other kept columns.
    let mut vif = vec![f64::NAN; ptot];
    let icol = if opts.intercept { Some(0usize) } else { None };
    for &j in &kept {
        if Some(j) == icol {
            continue;
        }
        let others: Vec<usize> = kept.iter().copied().filter(|&k| k != j).collect();
        let xj = d.col(j).to_vec();
        let (rss_j, _) = rss_of(&d, &others, &xj);
        let sst_j = if opts.intercept {
            ss_of(&xj, mean_of(&xj))
        } else {
            xj.iter().map(|v| v * v).sum()
        };
        vif[j] = sst_j / rss_j;
    }
    let fitted: Vec<f64> = y.iter().zip(&sol.resid).map(|(a, e)| a - e).collect();
    Ok(MultipleRegression {
        n,
        intercept: opts.intercept,
        coefficients: coef,
        se,
        t,
        p,
        ci,
        aliased,
        rank,
        r_squared: r2,
        adj_r_squared: adj,
        ss_regression: ssr,
        ss_residual: rss,
        ss_total: sst,
        df_regression: df_reg,
        df_residual: df_res,
        ms_regression: ms_reg,
        ms_residual: ms_res,
        f,
        p_f: if f.is_finite() {
            p_upper_f(f, df_reg, df_res)
        } else {
            f64::NAN
        },
        sy_x: s,
        vif,
        covariance: cov,
        residuals: sol.resid,
        fitted,
        ci_level: opts.ci_level,
    })
}

// =======================================================================================
// Deming regression
// =======================================================================================

/// Deming regression result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Deming {
    /// Error-variance ratio lambda = var(error in y) / var(error in x).
    pub lambda: f64,
    /// Pairs used.
    pub n: usize,
    /// Slope.
    pub slope: f64,
    /// Intercept.
    pub intercept: f64,
    /// Jackknife SE of the slope.
    pub se_slope: f64,
    /// Jackknife SE of the intercept.
    pub se_intercept: f64,
    /// slope +/- t_{n-2} SE.
    pub ci_slope: (f64, f64),
    /// intercept +/- t_{n-2} SE.
    pub ci_intercept: (f64, f64),
    /// Confidence level.
    pub ci_level: f64,
}

fn deming_fit(x: &[f64], y: &[f64], lambda: f64) -> Option<(f64, f64)> {
    let (cx, cy) = (x[0], y[0]);
    let dx: Vec<f64> = x.iter().map(|v| v - cx).collect();
    let dy: Vec<f64> = y.iter().map(|v| v - cy).collect();
    let (mx, my) = (mean_of(&dx), mean_of(&dy));
    let sxx = ss_of(&dx, mx);
    let syy = ss_of(&dy, my);
    let sxy = compensated_sum(dx.iter().zip(&dy).map(|(a, b)| (a - mx) * (b - my)));
    if sxy == 0.0 {
        return None;
    }
    let u = syy - lambda * sxx;
    let b = (u + (u * u + 4.0 * lambda * sxy * sxy).sqrt()) / (2.0 * sxy);
    Some((b, (my + cy) - b * (mx + cx)))
}

/// Deming regression (Deming 1943; Linnet 1993, Clin. Chem. 39:424,
/// doi:10.1093/clinchem/39.3.424) with error-variance ratio `lambda` = sigma_y^2/sigma_x^2
/// (1 = orthogonal regression; Prism's input is the two error SDs, lambda =
/// (SD_y/SD_x)^2): slope = (Syy - l Sxx + sqrt((Syy - l Sxx)^2 + 4 l Sxy^2))/(2 Sxy).
/// SEs by the leave-one-out jackknife sqrt((n-1)/n sum (b_(i) - mean b_(.))^2) as Linnet
/// recommends (NCSS, R `mcr`); CIs use t with n - 2 df. Prism does not document its SE
/// method, so these SEs may differ from Prism's.
pub fn deming_regression(x: &[f64], y: &[f64], lambda: f64, ci_level: f64) -> Result<Deming> {
    check_level(ci_level)?;
    if !(lambda > 0.0) || !lambda.is_finite() {
        return Err(invalid("Deming: lambda must be positive"));
    }
    let (x, y) = complete_xy(x, y)?;
    let n = x.len();
    if n < 3 {
        return Err(StatsError::NotEnoughData(
            "Deming needs at least 3 pairs".into(),
        ));
    }
    let (b, a) = deming_fit(&x, &y, lambda)
        .ok_or_else(|| invalid("Deming: x and y are uncorrelated (Sxy = 0)"))?;
    let mut bs = Vec::with_capacity(n);
    let mut as_ = Vec::with_capacity(n);
    for i in 0..n {
        let xi: Vec<f64> = x
            .iter()
            .enumerate()
            .filter(|&(k, _)| k != i)
            .map(|(_, v)| *v)
            .collect();
        let yi: Vec<f64> = y
            .iter()
            .enumerate()
            .filter(|&(k, _)| k != i)
            .map(|(_, v)| *v)
            .collect();
        let (bb, aa) = deming_fit(&xi, &yi, lambda)
            .ok_or_else(|| invalid("Deming: a jackknife subsample has Sxy = 0"))?;
        bs.push(bb);
        as_.push(aa);
    }
    let nf = n as f64;
    let jse = |v: &[f64]| -> f64 {
        let m = mean_of(v);
        ((nf - 1.0) / nf * compensated_sum(v.iter().map(|x| (x - m) * (x - m)))).sqrt()
    };
    let (se_b, se_a) = (jse(&bs), jse(&as_));
    let tc = t_crit(ci_level, nf - 2.0);
    Ok(Deming {
        lambda,
        n,
        slope: b,
        intercept: a,
        se_slope: se_b,
        se_intercept: se_a,
        ci_slope: (b - tc * se_b, b + tc * se_b),
        ci_intercept: (a - tc * se_a, a + tc * se_a),
        ci_level,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_distribution_sums_to_one() {
        for &(a, b) in &[(5usize, 6usize), (3, 1), (10, 10), (1, 1)] {
            let p = runs_cdf(a + b, a, b);
            assert!((p - 1.0).abs() < 1e-13, "{a},{b}: {p}");
        }
        // n1 = n2 = 2: sequences 6; runs 2: 2 (AABB, BBAA); runs 3: 2; runs 4: 2.
        assert!((runs_cdf(2, 2, 2) - 1.0 / 3.0).abs() < 1e-15);
        assert!((runs_cdf(3, 2, 2) - 2.0 / 3.0).abs() < 1e-15);
    }

    #[test]
    fn aliased_column_reported() {
        let rows: Vec<Vec<f64>> = (0..10)
            .map(|i| {
                let x = i as f64;
                vec![x, 2.0 * x, (x * 0.3).sin()]
            })
            .collect();
        let y: Vec<f64> = (0..10).map(|i| 1.0 + i as f64 + (i as f64).cos()).collect();
        let r = multiple_regression(&rows, &y, MultipleRegressionOptions::default()).unwrap();
        assert_eq!(r.aliased, vec![2]);
        assert!(r.coefficients[2].is_nan());
        assert_eq!(r.rank, 3);
        assert_eq!(r.df_residual, 7.0);
    }

    #[test]
    fn lack_of_fit_with_replicates() {
        // Quadratic truth, straight-line fit: replicates test must flag it.
        let x = [1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0];
        let y: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(i, v)| v * v + if i % 2 == 0 { 0.1 } else { -0.1 })
            .collect();
        let r = simple_linear_regression(&x, &y, SimpleRegressionOptions::default()).unwrap();
        let l = r.lack_of_fit.unwrap();
        assert_eq!(l.df_pure_error, 5.0);
        assert_eq!(l.df_lack_of_fit, 3.0);
        // pure error SS = 5 groups x 2 x 0.1^2 = 0.1
        assert!((l.ss_pure_error - 0.1).abs() < 1e-14);
        assert!(l.p < 1e-4 && l.f > 400.0, "{l:?}");
    }

    #[test]
    fn through_origin_simple_matches_multiple() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [2.1, 3.9, 6.2, 7.8, 9.9];
        let s = simple_linear_regression(
            &x,
            &y,
            SimpleRegressionOptions {
                through_origin: true,
                ..Default::default()
            },
        )
        .unwrap();
        let rows: Vec<Vec<f64>> = x.iter().map(|v| vec![*v]).collect();
        let m = multiple_regression(
            &rows,
            &y,
            MultipleRegressionOptions {
                intercept: false,
                ci_level: 0.95,
            },
        )
        .unwrap();
        assert!((s.slope - m.coefficients[0]).abs() < 1e-15);
        assert!((s.se_slope - m.se[0]).abs() < 1e-15);
        assert!((s.r_squared - m.r_squared).abs() < 1e-15);
    }
}
