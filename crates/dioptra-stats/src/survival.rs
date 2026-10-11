//! Survival analysis: Kaplan–Meier with Greenwood variance and three confidence-interval
//! transforms, median survival with the Brookmeyer–Crowley interval, the weighted
//! log-rank family for k groups (Mantel–Cox, Gehan–Breslow–Wilcoxon, Tarone–Ware, test
//! for trend), Prism's two hazard ratios, and Cox proportional hazards (Efron or Breslow
//! ties) by Newton–Raphson.
//!
//! Conventions: `event = true` is an observed event, `false` a censored time. At tied
//! times events are counted before censorings (a subject censored at t is at risk at t).
//! Rows with a non-finite time are dropped.

use crate::dist::{p_two_sided_z, p_upper_chi2};
use crate::special::norm_quantile_pq;
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

fn z_of(level: f64) -> Result<f64> {
    if !(level > 0.0 && level < 1.0) {
        return Err(invalid("ci_level must be in (0, 1)"));
    }
    let a = 1.0 - level;
    Ok(norm_quantile_pq(1.0 - 0.5 * a, 0.5 * a))
}

/// Distinct times with (events, censorings), ascending.
fn tabulate(time: &[f64], event: &[bool]) -> Result<Vec<(f64, usize, usize)>> {
    if time.len() != event.len() {
        return Err(invalid("time and event lengths differ"));
    }
    let mut v: Vec<(f64, bool)> = time
        .iter()
        .zip(event)
        .filter(|(t, _)| t.is_finite())
        .map(|(t, e)| (*t, *e))
        .collect();
    if v.is_empty() {
        return Err(StatsError::NotEnoughData("no finite times".into()));
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, usize, usize)> = Vec::new();
    for (t, e) in v {
        match out.last_mut() {
            Some(last) if last.0 == t => {
                if e {
                    last.1 += 1
                } else {
                    last.2 += 1
                }
            }
            _ => out.push((t, e as usize, (!e) as usize)),
        }
    }
    Ok(out)
}

/// Confidence-interval transform for S(t).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum KmCi {
    /// Log-log ("exponential Greenwood"; Kalbfleisch & Prentice 1980): S^exp(+/- z
    /// sqrt(sum d/(n(n-d)))/|ln S|). Prism's "asymmetrical" default, lifelines' default,
    /// R `conf.type = "log-log"`.
    #[default]
    LogLog,
    /// Linear (plain Greenwood): S +/- z SE, clipped to [0, 1] (Prism "symmetrical").
    Linear,
    /// Log: S exp(+/- z sqrt(sum d/(n(n-d)))), clipped to <= 1 (R's default).
    Log,
}

/// Options for [`kaplan_meier`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KmOptions {
    /// Confidence level (0.95).
    pub ci_level: f64,
    /// Transform for `ci_lower`/`ci_upper` and the median CI.
    pub ci: KmCi,
}

impl Default for KmOptions {
    fn default() -> Self {
        KmOptions {
            ci_level: 0.95,
            ci: KmCi::LogLog,
        }
    }
}

/// One row of the Kaplan–Meier table (one per distinct event time).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KmRow {
    /// Event time.
    pub time: f64,
    /// Number at risk just before `time`.
    pub n_at_risk: usize,
    /// Events at `time`.
    pub events: usize,
    /// Censorings at exactly `time` (counted after the events).
    pub censored: usize,
    /// S(time).
    pub survival: f64,
    /// Greenwood variance S^2 sum d/(n(n-d)) (NaN once S = 0).
    pub greenwood_var: f64,
    /// sqrt of the Greenwood variance.
    pub se: f64,
    /// Log-log CI.
    pub ci_loglog: (f64, f64),
    /// Linear CI clipped to [0, 1].
    pub ci_linear: (f64, f64),
    /// Log CI.
    pub ci_log: (f64, f64),
    /// The CI chosen in the options.
    pub ci_lower: f64,
    /// Upper limit of the chosen CI.
    pub ci_upper: f64,
}

/// Kaplan–Meier estimate.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KaplanMeier {
    /// Rows at the distinct event times.
    pub rows: Vec<KmRow>,
    /// Subjects.
    pub n: usize,
    /// Events.
    pub events: usize,
    /// Median survival: the smallest t with S(t) <= 0.5 (+inf if never reached).
    pub median: f64,
    /// Brookmeyer & Crowley (1982) CI of the median by inverting the chosen pointwise
    /// band: lower = first t with ci_lower(t) <= 0.5, upper = first t with ci_upper(t) <=
    /// 0.5 (+inf if not reached).
    pub median_ci: (f64, f64),
    /// Step curve for plotting: (t, S) vertices from (0 or the first time, 1) to the last
    /// observed time, with a vertical drop at each event time.
    pub curve: Vec<(f64, f64)>,
    /// Censor marks (t, S(t)) at each distinct censored time.
    pub censor_marks: Vec<(f64, f64)>,
    /// Options used.
    pub options: KmOptions,
    /// All distinct times with (events, censorings), for number-at-risk queries.
    table: Vec<(f64, usize, usize)>,
}

impl KaplanMeier {
    /// S(t) (right-continuous step function; 1 before the first event).
    pub fn survival_at(&self, t: f64) -> f64 {
        self.row_at(t).map_or(1.0, |r| r.survival)
    }

    /// The chosen CI of S(t) ((1, 1) before the first event).
    pub fn ci_at(&self, t: f64) -> (f64, f64) {
        self.row_at(t)
            .map_or((1.0, 1.0), |r| (r.ci_lower, r.ci_upper))
    }

    fn row_at(&self, t: f64) -> Option<&KmRow> {
        let k = self.rows.partition_point(|r| r.time <= t);
        if k == 0 {
            None
        } else {
            Some(&self.rows[k - 1])
        }
    }

    /// Number at risk at time t (subjects with time >= t).
    pub fn at_risk_at(&self, t: f64) -> usize {
        self.table
            .iter()
            .filter(|r| r.0 >= t)
            .map(|r| r.1 + r.2)
            .sum()
    }
}

fn clip01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Kaplan–Meier product-limit estimate (Kaplan & Meier 1958) with Greenwood's (1926)
/// variance and log-log, linear and log pointwise CIs.
pub fn kaplan_meier(time: &[f64], event: &[bool], opts: KmOptions) -> Result<KaplanMeier> {
    let z = z_of(opts.ci_level)?;
    let table = tabulate(time, event)?;
    let n: usize = table.iter().map(|r| r.1 + r.2).sum();
    let mut at_risk = n;
    let mut s = 1.0;
    let mut gsum = 0.0;
    let mut rows = Vec::new();
    let mut curve = vec![(table[0].0.min(0.0), 1.0)];
    let mut marks = Vec::new();
    for &(t, d, c) in &table {
        if d > 0 {
            let nn = at_risk as f64;
            let df = d as f64;
            curve.push((t, s));
            s *= 1.0 - df / nn;
            curve.push((t, s));
            gsum += if at_risk > d {
                df / (nn * (nn - df))
            } else {
                f64::INFINITY
            };
            let (var, se, ll, lin, lg) = if s > 0.0 && gsum.is_finite() {
                let var = s * s * gsum;
                let se = var.sqrt();
                let ll = if s < 1.0 {
                    let th = z * gsum.sqrt() / s.ln().abs();
                    (s.powf(th.exp()), s.powf((-th).exp()))
                } else {
                    (f64::NAN, f64::NAN)
                };
                let lin = (clip01(s - z * se), clip01(s + z * se));
                let w = z * gsum.sqrt();
                let lg = (s * (-w).exp(), (s * w.exp()).min(1.0));
                (var, se, ll, lin, lg)
            } else {
                (
                    f64::NAN,
                    f64::NAN,
                    (f64::NAN, f64::NAN),
                    (f64::NAN, f64::NAN),
                    (f64::NAN, f64::NAN),
                )
            };
            let chosen = match opts.ci {
                KmCi::LogLog => ll,
                KmCi::Linear => lin,
                KmCi::Log => lg,
            };
            rows.push(KmRow {
                time: t,
                n_at_risk: at_risk,
                events: d,
                censored: c,
                survival: s,
                greenwood_var: var,
                se,
                ci_loglog: ll,
                ci_linear: lin,
                ci_log: lg,
                ci_lower: chosen.0,
                ci_upper: chosen.1,
            });
        }
        if c > 0 {
            marks.push((t, s));
        }
        at_risk -= d + c;
    }
    let last_t = table.last().map_or(0.0, |r| r.0);
    if curve.last().is_some_and(|p| p.0 < last_t) {
        curve.push((last_t, s));
    }
    let first_le = |f: &dyn Fn(&KmRow) -> f64| -> f64 {
        rows.iter()
            .find(|r| {
                let v = f(r);
                // S = 0 rows have no CI: the band has reached 0 there.
                if v.is_nan() {
                    r.survival <= 0.5
                } else {
                    v <= 0.5
                }
            })
            .map_or(f64::INFINITY, |r| r.time)
    };
    let median = first_le(&|r| r.survival);
    let median_ci = (first_le(&|r| r.ci_lower), first_le(&|r| r.ci_upper));
    let events = rows.iter().map(|r| r.events).sum();
    Ok(KaplanMeier {
        rows,
        n,
        events,
        median,
        median_ci,
        curve,
        censor_marks: marks,
        options: opts,
        table,
    })
}

// =======================================================================================
// Log-rank family
// =======================================================================================

/// Weights of the weighted log-rank family (Fleming & Harrington 1991, ch. 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum LogRankWeight {
    /// w = 1: log-rank (Mantel–Cox, Mantel–Haenszel form).
    #[default]
    LogRank,
    /// w = n at risk: Gehan–Breslow–Wilcoxon.
    Wilcoxon,
    /// w = sqrt(n at risk): Tarone–Ware.
    TaroneWare,
}

/// Weighted log-rank test of k groups.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LogRank {
    /// Weighting.
    pub weight: LogRankWeight,
    /// Chi-square U' V^- U over the first k-1 groups.
    pub chi2: f64,
    /// k - 1.
    pub df: f64,
    /// P(chi2_df >= chi2).
    pub p: f64,
    /// Observed events per group.
    pub observed: Vec<f64>,
    /// Expected events per group (unweighted).
    pub expected: Vec<f64>,
    /// observed/expected per group.
    pub o_over_e: Vec<f64>,
    /// Weighted covariance matrix V of O - E (k x k, row-major).
    pub variance: Vec<f64>,
    /// Test for trend (scores 1..k unless given): chi2 on 1 df, and p; None for k = 2.
    pub trend: Option<(f64, f64)>,
}

struct RiskTable {
    k: usize,
    /// per distinct event time: (n_total, d_total, n_g, d_g)
    rows: Vec<(f64, f64, Vec<f64>, Vec<f64>)>,
}

fn risk_table(groups: &[(&[f64], &[bool])]) -> Result<RiskTable> {
    let k = groups.len();
    let tabs: Vec<Vec<(f64, usize, usize)>> = groups
        .iter()
        .map(|(t, e)| tabulate(t, e))
        .collect::<Result<_>>()?;
    let mut times: Vec<f64> = tabs
        .iter()
        .flat_map(|t| t.iter().filter(|r| r.1 > 0).map(|r| r.0))
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    let totals: Vec<usize> = tabs
        .iter()
        .map(|t| t.iter().map(|r| r.1 + r.2).sum())
        .collect();
    let mut pos = vec![0usize; k];
    let mut at_risk: Vec<usize> = totals.clone();
    let mut rows = Vec::with_capacity(times.len());
    for &t in &times {
        let mut ng = vec![0.0; k];
        let mut dg = vec![0.0; k];
        for g in 0..k {
            // remove subjects with time < t
            while pos[g] < tabs[g].len() && tabs[g][pos[g]].0 < t {
                at_risk[g] -= tabs[g][pos[g]].1 + tabs[g][pos[g]].2;
                pos[g] += 1;
            }
            ng[g] = at_risk[g] as f64;
            if pos[g] < tabs[g].len() && tabs[g][pos[g]].0 == t {
                dg[g] = tabs[g][pos[g]].1 as f64;
            }
        }
        rows.push((ng.iter().sum(), dg.iter().sum(), ng, dg));
    }
    Ok(RiskTable { k, rows })
}

/// Solve the symmetric positive definite system A x = b (Cholesky); None if singular.
fn spd_solve(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                if !(s > 1e-300) {
                    return None;
                }
                l[i * n + i] = s.sqrt();
            } else {
                l[i * n + j] = s / l[j * n + j];
            }
        }
    }
    let mut x = b.to_vec();
    for i in 0..n {
        for k in 0..i {
            x[i] -= l[i * n + k] * x[k];
        }
        x[i] /= l[i * n + i];
    }
    for i in (0..n).rev() {
        for k in (i + 1)..n {
            x[i] -= l[k * n + i] * x[k];
        }
        x[i] /= l[i * n + i];
    }
    Some(x)
}

/// Weighted log-rank test of k >= 2 groups (Mantel 1966; Peto & Peto 1972; Gehan 1965,
/// Breslow 1970; Tarone & Ware 1977) with the hypergeometric variance:
/// E_gj = d_j n_gj/n_j, V_gh = sum_j w_j^2 d_j (n_j - d_j)/(n_j - 1) (n_gj/n_j)(delta_gh -
/// n_hj/n_j), chi2 = U' V^{-1} U over k - 1 groups. The trend test uses `scores` (default
/// 1..k): chi2 = (c'U)^2/(c'Vc) on 1 df.
pub fn logrank(
    groups: &[(&[f64], &[bool])],
    weight: LogRankWeight,
    scores: Option<&[f64]>,
) -> Result<LogRank> {
    if groups.len() < 2 {
        return Err(invalid("need at least two groups"));
    }
    let rt = risk_table(groups)?;
    let k = rt.k;
    let mut u = vec![0.0; k];
    let mut obs = vec![0.0; k];
    let mut exp = vec![0.0; k];
    let mut v = vec![0.0; k * k];
    for (n, d, ng, dg) in &rt.rows {
        let w = match weight {
            LogRankWeight::LogRank => 1.0,
            LogRankWeight::Wilcoxon => *n,
            LogRankWeight::TaroneWare => n.sqrt(),
        };
        let f = if *n > 1.0 {
            d * (n - d) / (n - 1.0)
        } else {
            0.0
        };
        for g in 0..k {
            let e = d * ng[g] / n;
            obs[g] += dg[g];
            exp[g] += e;
            u[g] += w * (dg[g] - e);
            for h in 0..k {
                let delta = if g == h { 1.0 } else { 0.0 };
                v[g * k + h] += w * w * f * (ng[g] / n) * (delta - ng[h] / n);
            }
        }
    }
    let m = k - 1;
    let vs: Vec<f64> = (0..m)
        .flat_map(|g| (0..m).map(move |h| (g, h)))
        .map(|(g, h)| v[g * k + h])
        .collect();
    let chi2 = match spd_solve(&vs, &u[..m], m) {
        Some(x) => x.iter().zip(&u[..m]).map(|(a, b)| a * b).sum(),
        None => f64::NAN,
    };
    let trend = if k > 2 {
        let c: Vec<f64> = match scores {
            Some(s) if s.len() == k => s.to_vec(),
            Some(_) => return Err(invalid("one score per group")),
            None => (1..=k).map(|i| i as f64).collect(),
        };
        let cu: f64 = c.iter().zip(&u).map(|(a, b)| a * b).sum();
        let mut cvc = 0.0;
        for g in 0..k {
            for h in 0..k {
                cvc += c[g] * v[g * k + h] * c[h];
            }
        }
        let t = cu * cu / cvc;
        Some((t, p_upper_chi2(t, 1.0)))
    } else {
        None
    };
    Ok(LogRank {
        weight,
        chi2,
        df: m as f64,
        p: p_upper_chi2(chi2, m as f64),
        o_over_e: obs.iter().zip(&exp).map(|(o, e)| o / e).collect(),
        observed: obs,
        expected: exp,
        variance: v,
        trend,
    })
}

/// Hazard ratio of group 1 relative to group 2, the two ways Prism reports.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HazardRatio {
    /// Observed events, group 1.
    pub o1: f64,
    /// Expected events, group 1.
    pub e1: f64,
    /// Observed events, group 2.
    pub o2: f64,
    /// Expected events, group 2.
    pub e2: f64,
    /// Hypergeometric variance of O1 - E1.
    pub v: f64,
    /// Log-rank method: (O1/E1)/(O2/E2).
    pub hr_logrank: f64,
    /// exp(ln HR +/- z sqrt(1/E1 + 1/E2)).
    pub hr_logrank_ci: (f64, f64),
    /// Mantel–Haenszel method: exp((O1 - E1)/V).
    pub hr_mantel_haenszel: f64,
    /// exp((O1 - E1)/V +/- z/sqrt(V)).
    pub hr_mantel_haenszel_ci: (f64, f64),
    /// Mantel–Haenszel chi-square (O1 - E1)^2/V.
    pub chi2: f64,
}

/// Prism's two hazard ratios from the log-rank components (GraphPad Statistics Guide,
/// "The hazard ratio"; Berry, Kitchin & Mock 1991 for the log-rank method).
pub fn hazard_ratio(
    g1: (&[f64], &[bool]),
    g2: (&[f64], &[bool]),
    ci_level: f64,
) -> Result<HazardRatio> {
    let z = z_of(ci_level)?;
    let lr = logrank(&[g1, g2], LogRankWeight::LogRank, None)?;
    let (o1, e1, o2, e2) = (
        lr.observed[0],
        lr.expected[0],
        lr.observed[1],
        lr.expected[1],
    );
    let v = lr.variance[0];
    let hr = (o1 / e1) / (o2 / e2);
    let sl = (1.0 / e1 + 1.0 / e2).sqrt();
    let l = (o1 - e1) / v;
    Ok(HazardRatio {
        o1,
        e1,
        o2,
        e2,
        v,
        hr_logrank: hr,
        hr_logrank_ci: ((hr.ln() - z * sl).exp(), (hr.ln() + z * sl).exp()),
        hr_mantel_haenszel: l.exp(),
        hr_mantel_haenszel_ci: ((l - z / v.sqrt()).exp(), (l + z / v.sqrt()).exp()),
        chi2: (o1 - e1) * (o1 - e1) / v,
    })
}

// =======================================================================================
// Cox proportional hazards
// =======================================================================================

/// Tie handling in the Cox partial likelihood.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CoxTies {
    /// Efron (1977) (R `coxph` and lifelines default).
    #[default]
    Efron,
    /// Breslow (1974).
    Breslow,
}

/// One Cox coefficient.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CoxCoef {
    /// beta.
    pub coef: f64,
    /// SE from the inverse observed information.
    pub se: f64,
    /// exp(beta).
    pub hr: f64,
    /// exp(beta +/- z SE).
    pub hr_ci: (f64, f64),
    /// Wald z.
    pub z: f64,
    /// Two-sided Wald p.
    pub p: f64,
}

/// Cox proportional-hazards fit.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CoxPh {
    /// One per covariate.
    pub coefficients: Vec<CoxCoef>,
    /// Partial log-likelihood at the estimate.
    pub log_likelihood: f64,
    /// Partial log-likelihood at beta = 0.
    pub log_likelihood_null: f64,
    /// Likelihood-ratio chi-square 2 (ll - ll0).
    pub lr_chi2: f64,
    /// Its p on p df.
    pub lr_p: f64,
    /// Covariance (inverse information), row-major.
    pub covariance: Vec<f64>,
    /// Newton iterations.
    pub iterations: usize,
    /// Subjects.
    pub n: usize,
    /// Events.
    pub events: usize,
}

struct CoxData {
    /// sorted by time descending; covariates centred
    t: Vec<f64>,
    e: Vec<bool>,
    x: Vec<Vec<f64>>,
}

/// Partial log-likelihood, gradient and information (negative Hessian).
fn cox_eval(cd: &CoxData, beta: &[f64], ties: CoxTies) -> (f64, Vec<f64>, Vec<f64>) {
    let p = beta.len();
    let n = cd.t.len();
    let eta: Vec<f64> =
        cd.x.iter()
            .map(|xi| xi.iter().zip(beta).map(|(a, b)| a * b).sum())
            .collect();
    let c = eta.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let r: Vec<f64> = eta.iter().map(|v| (v - c).exp()).collect();
    let mut ll = 0.0;
    let mut grad = vec![0.0; p];
    let mut info = vec![0.0; p * p];
    let mut s0 = 0.0;
    let mut s1 = vec![0.0; p];
    let mut s2 = vec![0.0; p * p];
    let mut i = 0;
    while i < n {
        // block of equal times (descending order): add all to the risk set first
        let t = cd.t[i];
        let mut j = i;
        let mut d = 0usize;
        let mut d0 = 0.0;
        let mut d1 = vec![0.0; p];
        let mut d2 = vec![0.0; p * p];
        while j < n && cd.t[j] == t {
            let ri = r[j];
            s0 += ri;
            for a in 0..p {
                s1[a] += ri * cd.x[j][a];
                for b in 0..p {
                    s2[a * p + b] += ri * cd.x[j][a] * cd.x[j][b];
                }
            }
            if cd.e[j] {
                d += 1;
                d0 += ri;
                ll += eta[j];
                for a in 0..p {
                    d1[a] += ri * cd.x[j][a];
                    grad[a] += cd.x[j][a];
                    for b in 0..p {
                        d2[a * p + b] += ri * cd.x[j][a] * cd.x[j][b];
                    }
                }
            }
            j += 1;
        }
        if d > 0 {
            let df = d as f64;
            for l in 0..d {
                let f = match ties {
                    CoxTies::Efron => l as f64 / df,
                    CoxTies::Breslow => 0.0,
                };
                let den = s0 - f * d0;
                ll -= den.ln() + c;
                let m1: Vec<f64> = (0..p).map(|a| (s1[a] - f * d1[a]) / den).collect();
                for a in 0..p {
                    grad[a] -= m1[a];
                    for b in 0..p {
                        info[a * p + b] +=
                            (s2[a * p + b] - f * d2[a * p + b]) / den - m1[a] * m1[b];
                    }
                }
            }
        }
        i = j;
    }
    (ll, grad, info)
}

/// Cox partial log-likelihood at `beta` (Efron or Breslow ties), e.g. to compare two
/// candidate estimates.
pub fn cox_log_likelihood(
    time: &[f64],
    event: &[bool],
    covariates: &[Vec<f64>],
    beta: &[f64],
    ties: CoxTies,
) -> Result<f64> {
    let p = beta.len();
    if time.len() != event.len() || time.len() != covariates.len() {
        return Err(invalid(
            "time, event and covariate rows must have equal length",
        ));
    }
    let mut idx: Vec<usize> = (0..time.len())
        .filter(|&i| time[i].is_finite() && covariates[i].len() == p)
        .collect();
    idx.sort_by(|&a, &b| time[b].total_cmp(&time[a]));
    let cd = CoxData {
        t: idx.iter().map(|&i| time[i]).collect(),
        e: idx.iter().map(|&i| event[i]).collect(),
        x: idx.iter().map(|&i| covariates[i].clone()).collect(),
    };
    Ok(cox_eval(&cd, beta, ties).0)
}

/// Cox (1972) proportional-hazards regression by Newton–Raphson with step halving on the
/// partial likelihood (Efron or Breslow ties). `covariates` holds one row per subject.
/// Covariates are centred internally (the estimates do not change). SEs from the inverse
/// observed information; Wald and likelihood-ratio tests.
pub fn cox_ph(
    time: &[f64],
    event: &[bool],
    covariates: &[Vec<f64>],
    ties: CoxTies,
    ci_level: f64,
) -> Result<CoxPh> {
    let z = z_of(ci_level)?;
    if time.len() != event.len() || time.len() != covariates.len() {
        return Err(invalid(
            "time, event and covariate rows must have equal length",
        ));
    }
    let p = covariates.first().map_or(0, |r| r.len());
    if p == 0 {
        return Err(invalid("no covariates"));
    }
    let mut idx: Vec<usize> = (0..time.len())
        .filter(|&i| {
            time[i].is_finite()
                && covariates[i].len() == p
                && covariates[i].iter().all(|v| v.is_finite())
        })
        .collect();
    if idx.len() < 2 {
        return Err(StatsError::NotEnoughData(
            "fewer than 2 complete rows".into(),
        ));
    }
    idx.sort_by(|&a, &b| time[b].total_cmp(&time[a]));
    let n = idx.len();
    let means: Vec<f64> = (0..p)
        .map(|a| idx.iter().map(|&i| covariates[i][a]).sum::<f64>() / n as f64)
        .collect();
    let cd = CoxData {
        t: idx.iter().map(|&i| time[i]).collect(),
        e: idx.iter().map(|&i| event[i]).collect(),
        x: idx
            .iter()
            .map(|&i| (0..p).map(|a| covariates[i][a] - means[a]).collect())
            .collect(),
    };
    let events = cd.e.iter().filter(|&&e| e).count();
    if events == 0 {
        return Err(StatsError::NotEnoughData("no events".into()));
    }
    let mut beta = vec![0.0; p];
    let (ll0, mut grad, mut info) = cox_eval(&cd, &beta, ties);
    let mut ll = ll0;
    let mut iterations = 0;
    for it in 0..100 {
        iterations = it + 1;
        let step = spd_solve(&info, &grad, p)
            .ok_or_else(|| StatsError::NoConvergence("singular information matrix".into()))?;
        let mut h = 1.0;
        let mut accepted = None;
        for _ in 0..40 {
            let trial: Vec<f64> = beta.iter().zip(&step).map(|(b, s)| b + h * s).collect();
            let r = cox_eval(&cd, &trial, ties);
            if r.0.is_finite() && r.0 >= ll - 1e-13 * ll.abs() {
                accepted = Some((trial, r));
                break;
            }
            h *= 0.5;
        }
        let Some((trial, (lt, gt, it2))) = accepted else {
            break;
        };
        let size = step
            .iter()
            .zip(&trial)
            .map(|(s, b)| (h * s).abs() / (1.0 + b.abs()))
            .fold(0.0, f64::max);
        beta = trial;
        ll = lt;
        grad = gt;
        info = it2;
        if size <= 1e-13 {
            break;
        }
    }
    if beta.iter().any(|b| !b.is_finite() || b.abs() > 50.0) {
        return Err(StatsError::NoConvergence(
            "Cox coefficients diverge (monotone likelihood)".into(),
        ));
    }
    // covariance = info^{-1}
    let mut cov = vec![0.0; p * p];
    for col in 0..p {
        let mut e = vec![0.0; p];
        e[col] = 1.0;
        let x = spd_solve(&info, &e, p)
            .ok_or_else(|| StatsError::NoConvergence("singular information matrix".into()))?;
        for row in 0..p {
            cov[row * p + col] = x[row];
        }
    }
    let coefficients = (0..p)
        .map(|a| {
            let se = cov[a * p + a].sqrt();
            let zz = beta[a] / se;
            CoxCoef {
                coef: beta[a],
                se,
                hr: beta[a].exp(),
                hr_ci: ((beta[a] - z * se).exp(), (beta[a] + z * se).exp()),
                z: zz,
                p: p_two_sided_z(zz),
            }
        })
        .collect();
    let lr = 2.0 * (ll - ll0);
    Ok(CoxPh {
        coefficients,
        log_likelihood: ll,
        log_likelihood_null: ll0,
        lr_chi2: lr,
        lr_p: p_upper_chi2(lr, p as f64),
        covariance: cov,
        iterations,
        n,
        events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_computed_km() {
        // times 1,2,2,3+,4,5+ ; events at 1, 2 (x2), 4
        let t = [1.0, 2.0, 2.0, 3.0, 4.0, 5.0];
        let e = [true, true, true, false, true, false];
        let km = kaplan_meier(&t, &e, KmOptions::default()).unwrap();
        assert_eq!(km.rows.len(), 3);
        let s: Vec<f64> = km.rows.iter().map(|r| r.survival).collect();
        // 5/6, 5/6 * 3/5 = 1/2, 1/2 * 1/2 = 1/4
        assert!((s[0] - 5.0 / 6.0).abs() < 1e-15);
        assert!((s[1] - 0.5).abs() < 1e-15);
        assert!((s[2] - 0.25).abs() < 1e-15);
        assert_eq!(km.rows[2].n_at_risk, 2);
        assert_eq!(km.median, 2.0);
        // Greenwood at t=2: S^2 (1/(6*5) + 2/(5*3))
        let g = 0.25 * (1.0 / 30.0 + 2.0 / 15.0);
        assert!((km.rows[1].greenwood_var - g).abs() < 1e-15);
        assert_eq!(km.at_risk_at(3.0), 3);
        assert_eq!(km.at_risk_at(3.5), 2);
        assert_eq!(km.survival_at(0.5), 1.0);
        assert_eq!(km.survival_at(3.9), 0.5);
        assert_eq!(km.censor_marks, vec![(3.0, 0.5), (5.0, 0.25)]);
        assert_eq!(*km.curve.last().unwrap(), (5.0, 0.25));
    }

    #[test]
    fn censor_before_event_convention() {
        // a censoring tied with an event stays at risk at that time
        let km =
            kaplan_meier(&[2.0, 2.0, 3.0], &[true, false, true], KmOptions::default()).unwrap();
        assert_eq!(km.rows[0].n_at_risk, 3);
        assert!((km.rows[0].survival - 2.0 / 3.0).abs() < 1e-15);
        assert_eq!(km.rows[1].n_at_risk, 1);
        assert_eq!(km.rows[1].survival, 0.0);
    }
}
