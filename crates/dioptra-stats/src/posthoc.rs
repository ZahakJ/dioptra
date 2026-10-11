//! Multiple comparisons: p-value adjustment, Tukey–Kramer, Dunnett, Fisher's LSD and
//! Bonferroni/Šidák/Holm t tests after one-way ANOVA, Dunn's test after Kruskal–Wallis
//! or Friedman, and the compact letter display.
//!
//! Sign convention: a comparison of groups `i` and `j` estimates `value_i - value_j`
//! (first minus second, as the t tests do). All-pairs comparisons list (i, j) with i < j
//! in group order; many-to-one comparisons list (treatment, control). Prism's tables show
//! "A vs B: mean A - mean B" for all pairs, which is the same rule; statsmodels'
//! `pairwise_tukeyhsd` reports mean_j - mean_i instead.

use crate::anova::{clean_groups, complete_rows};
use crate::descriptive::{compensated_sum, mean_of, ss_of};
use crate::dist::{p_two_sided_z, Dunnett, StudentT, StudentizedRange};
use crate::rank::{rank_with_ties, tie_term};
use crate::special;
use crate::ttest::Alternative;
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

// =======================================================================================
// p-value adjustment
// =======================================================================================

/// Multiplicity adjustment.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Adjust {
    /// No adjustment (Fisher's LSD when used for comparisons).
    None,
    /// Bonferroni: min(1, m p).
    Bonferroni,
    /// Šidák: 1 - (1 - p)^m.
    Sidak,
    /// Holm (1979) step-down Bonferroni.
    Holm,
    /// Holm–Šidák step-down (statsmodels `holm-sidak`; Prism's default for selected pairs).
    HolmSidak,
    /// Hochberg (1988) step-up.
    Hochberg,
    /// Hommel (1988), as statsmodels `hommel`.
    Hommel,
    /// Benjamini–Hochberg (1995) FDR.
    BenjaminiHochberg,
    /// Benjamini–Yekutieli (2001) FDR under dependence.
    BenjaminiYekutieli,
    /// Benjamini, Krieger & Yekutieli (2006) two-stage step-up FDR at level `alpha`
    /// (statsmodels `fdr_tsbky`, one iteration). The adjusted values depend on alpha.
    TwoStageBky {
        /// FDR level Q.
        alpha: f64,
    },
}

impl Adjust {
    /// Display name.
    pub fn name(&self) -> &'static str {
        match self {
            Adjust::None => "none",
            Adjust::Bonferroni => "Bonferroni",
            Adjust::Sidak => "Sidak",
            Adjust::Holm => "Holm",
            Adjust::HolmSidak => "Holm-Sidak",
            Adjust::Hochberg => "Hochberg",
            Adjust::Hommel => "Hommel",
            Adjust::BenjaminiHochberg => "Benjamini-Hochberg",
            Adjust::BenjaminiYekutieli => "Benjamini-Yekutieli",
            Adjust::TwoStageBky { .. } => "Benjamini-Krieger-Yekutieli",
        }
    }
}

/// 1 - (1 - p)^m without cancellation for tiny p.
fn sidak(p: f64, m: f64) -> f64 {
    if p >= 1.0 {
        return 1.0;
    }
    -special::expm1(m * special::log1p(-p))
}

/// Benjamini–Hochberg adjusted values of ascending `s` (no cap), times `factor`.
fn bh_sorted(s: &[f64], factor: f64) -> Vec<f64> {
    let m = s.len();
    let mut out = vec![0.0; m];
    let mut run = f64::INFINITY;
    for i in (0..m).rev() {
        let v = s[i] * m as f64 / (i + 1) as f64 * factor;
        run = run.min(v);
        out[i] = run;
    }
    out
}

/// Multiplicity-adjusted p-values. `NaN` entries are left as `NaN` and do not count in m.
///
/// References: Bonferroni; Šidák (1967), JASA 62:626; Holm (1979), Scand. J. Stat. 6:65;
/// Holm–Šidák step-down; Hochberg (1988), Biometrika 75:800; Hommel (1988), Biometrika
/// 75:383 (statsmodels' algorithm); Benjamini & Hochberg (1995),
/// doi:10.1111/j.2517-6161.1995.tb02031.x; Benjamini & Yekutieli (2001), Ann. Stat. 29:1165;
/// Benjamini, Krieger & Yekutieli (2006), doi:10.1093/biomet/93.3.491 (statsmodels
/// `fdrcorrection_twostage(method='bky', maxiter=1)`). Agrees with statsmodels
/// `multipletests`.
pub fn p_adjust(p: &[f64], method: Adjust) -> Vec<f64> {
    let idx: Vec<usize> = (0..p.len()).filter(|&i| !p[i].is_nan()).collect();
    let mut order = idx.clone();
    order.sort_by(|&a, &b| p[a].total_cmp(&p[b]));
    let s: Vec<f64> = order.iter().map(|&i| p[i].clamp(0.0, 1.0)).collect();
    let m = s.len();
    let mf = m as f64;
    let mut out = vec![f64::NAN; p.len()];
    if m == 0 {
        return out;
    }
    let adj_sorted: Vec<f64> = match method {
        Adjust::None => s.clone(),
        Adjust::Bonferroni => s.iter().map(|&x| (x * mf).min(1.0)).collect(),
        Adjust::Sidak => s.iter().map(|&x| sidak(x, mf)).collect(),
        Adjust::Holm | Adjust::HolmSidak => {
            let mut run: f64 = 0.0;
            s.iter()
                .enumerate()
                .map(|(i, &x)| {
                    let k = (m - i) as f64;
                    let v = if method == Adjust::Holm {
                        (x * k).min(1.0)
                    } else {
                        sidak(x, k)
                    };
                    run = run.max(v);
                    run
                })
                .collect()
        }
        Adjust::Hochberg => {
            let mut v = vec![0.0; m];
            let mut run = f64::INFINITY;
            for i in (0..m).rev() {
                run = run.min(s[i] * (m - i) as f64);
                v[i] = run.min(1.0);
            }
            v
        }
        Adjust::Hommel => {
            let mut a = s.clone();
            for mm in (2..=m).rev() {
                let tail = &s[m - mm..];
                let cim = tail
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| mm as f64 * x / (i + 1) as f64)
                    .fold(f64::INFINITY, f64::min);
                for v in a[m - mm..].iter_mut() {
                    *v = v.max(cim);
                }
                for i in 0..(m - mm) {
                    a[i] = a[i].max((mm as f64 * s[i]).min(cim));
                }
            }
            a.into_iter().map(|x| x.min(1.0)).collect()
        }
        Adjust::BenjaminiHochberg => bh_sorted(&s, 1.0).into_iter().map(|x| x.min(1.0)).collect(),
        Adjust::BenjaminiYekutieli => {
            let c: f64 = (1..=m).map(|i| 1.0 / i as f64).sum();
            bh_sorted(&s, c).into_iter().map(|x| x.min(1.0)).collect()
        }
        Adjust::TwoStageBky { alpha } => {
            let fact = 1.0 + alpha;
            let alpha_prime = alpha / fact;
            let bh: Vec<f64> = bh_sorted(&s, 1.0).into_iter().map(|x| x.min(1.0)).collect();
            let r1 = bh.iter().filter(|&&x| x <= alpha_prime).count();
            if r1 == 0 || r1 == m {
                bh.into_iter().map(|x| (x * fact).min(1.0)).collect()
            } else {
                let m0 = (m - r1) as f64;
                bh.into_iter()
                    .map(|x| (x * m0 / mf * fact).min(1.0))
                    .collect()
            }
        }
    };
    for (k, &i) in order.iter().enumerate() {
        out[i] = adj_sorted[k];
    }
    out
}

// =======================================================================================
// Comparisons
// =======================================================================================

/// One pairwise comparison.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Comparison {
    /// First group (the treatment in many-to-one comparisons).
    pub i: usize,
    /// Second group (the control in many-to-one comparisons).
    pub j: usize,
    /// value_i - value_j (means, or mean ranks for Dunn's test).
    pub estimate: f64,
    /// Standard error of `estimate`.
    pub se: f64,
    /// Test statistic: Tukey's q (with the sqrt 2, as Prism reports it), t for Dunnett and
    /// the t-based methods, z for Dunn.
    pub statistic: f64,
    /// Degrees of freedom of the statistic (inf for z).
    pub df: f64,
    /// Unadjusted p-value (two-sided unless a one-sided alternative was requested).
    pub p_raw: f64,
    /// Multiplicity-adjusted p-value.
    pub p_adj: f64,
    /// Simultaneous confidence interval of `estimate`, where the method defines one.
    pub ci: Option<(f64, f64)>,
    /// p_adj < alpha.
    pub significant: bool,
}

/// A family of comparisons.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PostHoc {
    /// Method name.
    pub method: String,
    /// Family-wise significance level used for `significant` and the CIs.
    pub alpha: f64,
    /// Pooled variance (MS within / MS error); NaN for rank methods.
    pub ms_within: f64,
    /// Its df.
    pub df_within: f64,
    /// Critical value of the statistic for the simultaneous CIs (q for Tukey), if any.
    pub critical_value: Option<f64>,
    /// Group means (or mean ranks for Dunn).
    pub centers: Vec<f64>,
    /// Group sizes.
    pub n: Vec<usize>,
    /// The comparisons.
    pub comparisons: Vec<Comparison>,
}

fn check_alpha(alpha: f64) -> Result<()> {
    if !(alpha > 0.0 && alpha < 1.0) {
        return Err(invalid(format!("alpha {alpha} not in (0, 1)")));
    }
    Ok(())
}

/// Means, sizes, pooled MS and df of the groups.
fn pooled(groups: &[&[f64]], what: &str) -> Result<(Vec<f64>, Vec<usize>, f64, f64)> {
    let g = clean_groups(groups, what)?;
    let c = g[0][0];
    let d: Vec<Vec<f64>> = g
        .iter()
        .map(|v| v.iter().map(|x| x - c).collect())
        .collect();
    let means: Vec<f64> = d.iter().map(|v| mean_of(v)).collect();
    let ssw = compensated_sum(d.iter().zip(&means).map(|(v, &m)| ss_of(v, m)));
    let n: Vec<usize> = g.iter().map(|v| v.len()).collect();
    let nt: usize = n.iter().sum();
    if nt <= g.len() {
        return Err(StatsError::NotEnoughData(format!(
            "{what}: needs more values than groups"
        )));
    }
    let df = (nt - g.len()) as f64;
    let msw = ssw / df;
    if !(msw > 0.0) {
        return Err(invalid(format!("{what}: pooled variance is zero")));
    }
    Ok((means.iter().map(|m| m + c).collect(), n, msw, df))
}

fn all_pairs(k: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for i in 0..k {
        for j in (i + 1)..k {
            v.push((i, j));
        }
    }
    v
}

/// Tukey–Kramer all-pairs comparisons after one-way ANOVA (Tukey 1953; Kramer 1956,
/// Biometrics 12:307, doi:10.2307/3001469): q = |m_i - m_j| / sqrt(MS_w/2 (1/n_i + 1/n_j)),
/// adjusted p = P(Q_{k, df_w} >= q) from the studentized range, simultaneous CI
/// (m_i - m_j) +/- q_crit/sqrt 2 sqrt(MS_w (1/n_i + 1/n_j)). `statistic` is Prism's q (with
/// the sqrt 2); statsmodels `pairwise_tukeyhsd` reports the same p and CI with the sign of
/// the difference reversed. `p_raw` is the unadjusted two-sided t-test p (Fisher's LSD).
pub fn tukey_kramer(groups: &[&[f64]], alpha: f64) -> Result<PostHoc> {
    check_alpha(alpha)?;
    let (means, n, msw, df) = pooled(groups, "Tukey")?;
    let k = means.len();
    let sr = StudentizedRange::new(k, df)?;
    let qcrit = sr.quantile_pq(1.0 - alpha, alpha);
    let t = StudentT::new(df)?;
    let comparisons = all_pairs(k)
        .into_iter()
        .map(|(i, j)| {
            let est = means[i] - means[j];
            let v = msw * (1.0 / n[i] as f64 + 1.0 / n[j] as f64);
            let se = v.sqrt();
            let q = est.abs() / (0.5 * v).sqrt();
            let p_adj = sr.cdf_sf(q).1;
            let half = qcrit / std::f64::consts::SQRT_2 * se;
            Comparison {
                i,
                j,
                estimate: est,
                se,
                statistic: q,
                df,
                p_raw: t.two_sided_p(est / se),
                p_adj,
                ci: Some((est - half, est + half)),
                significant: p_adj < alpha,
            }
        })
        .collect();
    Ok(PostHoc {
        method: "Tukey-Kramer".into(),
        alpha,
        ms_within: msw,
        df_within: df,
        critical_value: Some(qcrit),
        centers: means,
        n,
        comparisons,
    })
}

/// Dunnett's many-to-one comparisons (Dunnett 1955, JASA 50:1096,
/// doi:10.1080/01621459.1955.10501294): t_i = (m_i - m_0)/sqrt(MS_w (1/n_i + 1/n_0)) with
/// MS_w pooled over all groups, adjusted p from the exact multivariate t with correlations
/// lambda_i lambda_j ([`Dunnett`], deterministic double integral), and simultaneous CIs
/// from its critical value. `Alternative::Greater` tests treatment > control (CI upper end
/// +inf), `Less` treatment < control. Comparisons are (treatment, control) in group order.
pub fn dunnett(
    groups: &[&[f64]],
    control: usize,
    alternative: Alternative,
    alpha: f64,
) -> Result<PostHoc> {
    check_alpha(alpha)?;
    let (means, n, msw, df) = pooled(groups, "Dunnett")?;
    let k = means.len();
    if control >= k {
        return Err(invalid("Dunnett: control index out of range"));
    }
    let treat: Vec<usize> = (0..k).filter(|&i| i != control).collect();
    let sizes: Vec<usize> = treat.iter().map(|&i| n[i]).collect();
    let dist = Dunnett::from_group_sizes(n[control], &sizes, df)?;
    let two = alternative == Alternative::TwoSided;
    let crit = dist.quantile(1.0 - alpha, two);
    let t = StudentT::new(df)?;
    let comparisons = treat
        .iter()
        .map(|&i| {
            let est = means[i] - means[control];
            let se = (msw * (1.0 / n[i] as f64 + 1.0 / n[control] as f64)).sqrt();
            let ts = est / se;
            let (p_raw, p_adj, ci) = match alternative {
                Alternative::TwoSided => (
                    t.two_sided_p(ts),
                    dist.p_value(ts, true),
                    (est - crit * se, est + crit * se),
                ),
                Alternative::Greater => (
                    t.cdf_sf(ts).1,
                    dist.p_value(ts, false),
                    (est - crit * se, f64::INFINITY),
                ),
                Alternative::Less => (
                    t.cdf_sf(ts).0,
                    dist.p_value(-ts, false),
                    (f64::NEG_INFINITY, est + crit * se),
                ),
            };
            Comparison {
                i,
                j: control,
                estimate: est,
                se,
                statistic: ts,
                df,
                p_raw,
                p_adj,
                ci: Some(ci),
                significant: p_adj < alpha,
            }
        })
        .collect();
    Ok(PostHoc {
        method: "Dunnett".into(),
        alpha,
        ms_within: msw,
        df_within: df,
        critical_value: Some(crit),
        centers: means,
        n,
        comparisons,
    })
}

/// t tests on the pooled ANOVA variance for the given pairs (all pairs if `None`), with
/// the p-values adjusted over the family: `Adjust::None` is Fisher's (unprotected) LSD;
/// Bonferroni and Šidák also give simultaneous CIs (t at alpha/m, resp. 1-(1-alpha)^(1/m));
/// step-down and FDR methods give no CIs (as Prism). References: Fisher (1935) LSD;
/// Šidák (1967); Holm (1979); see [`p_adjust`].
pub fn pairwise_t(
    groups: &[&[f64]],
    pairs: Option<&[(usize, usize)]>,
    adjust: Adjust,
    alpha: f64,
) -> Result<PostHoc> {
    check_alpha(alpha)?;
    let (means, n, msw, df) = pooled(groups, "pairwise t")?;
    let k = means.len();
    let pairs: Vec<(usize, usize)> = match pairs {
        Some(p) => p.to_vec(),
        None => all_pairs(k),
    };
    if pairs.iter().any(|&(i, j)| i >= k || j >= k || i == j) {
        return Err(invalid("pairwise t: invalid pair"));
    }
    let m = pairs.len() as f64;
    let t = StudentT::new(df)?;
    let ci_alpha = match adjust {
        Adjust::None => Some(alpha),
        Adjust::Bonferroni => Some(alpha / m),
        Adjust::Sidak => Some(-special::expm1(special::log1p(-alpha) / m)),
        _ => None,
    };
    let tcrit = ci_alpha.map(|a| t.quantile_pq(1.0 - 0.5 * a, 0.5 * a));
    let mut comps: Vec<Comparison> = pairs
        .iter()
        .map(|&(i, j)| {
            let est = means[i] - means[j];
            let se = (msw * (1.0 / n[i] as f64 + 1.0 / n[j] as f64)).sqrt();
            let ts = est / se;
            Comparison {
                i,
                j,
                estimate: est,
                se,
                statistic: ts,
                df,
                p_raw: t.two_sided_p(ts),
                p_adj: f64::NAN,
                ci: tcrit.map(|c| (est - c * se, est + c * se)),
                significant: false,
            }
        })
        .collect();
    finish_adjust(&mut comps, adjust, alpha);
    let method = if adjust == Adjust::None {
        "Fisher LSD".to_string()
    } else {
        format!("{} t tests", adjust.name())
    };
    Ok(PostHoc {
        method,
        alpha,
        ms_within: msw,
        df_within: df,
        critical_value: tcrit,
        centers: means,
        n,
        comparisons: comps,
    })
}

fn finish_adjust(comps: &mut [Comparison], adjust: Adjust, alpha: f64) {
    let raw: Vec<f64> = comps.iter().map(|c| c.p_raw).collect();
    let adj = p_adjust(&raw, adjust);
    for (c, a) in comps.iter_mut().zip(adj) {
        c.p_adj = a;
        c.significant = a < alpha;
    }
}

fn dunn_pairs(k: usize, control: Option<usize>) -> Result<Vec<(usize, usize)>> {
    match control {
        None => Ok(all_pairs(k)),
        Some(c) if c < k => Ok((0..k).filter(|&i| i != c).map(|i| (i, c)).collect()),
        Some(_) => Err(invalid("Dunn: control index out of range")),
    }
}

/// Dunn's test after Kruskal–Wallis (Dunn 1964, Technometrics 6:241,
/// doi:10.1080/00401706.1964.10490181; as Prism, after Daniel's *Applied Nonparametric
/// Statistics*): ranks over the pooled sample,
/// z = (Rbar_i - Rbar_j) / sqrt((N(N+1)/12 - sum(t^3-t)/(12(N-1))) (1/n_i + 1/n_j)),
/// two-sided normal p, adjusted over all pairs (or the k - 1 comparisons with `control`).
pub fn dunn_kruskal(
    groups: &[&[f64]],
    control: Option<usize>,
    adjust: Adjust,
    alpha: f64,
) -> Result<PostHoc> {
    check_alpha(alpha)?;
    let g = clean_groups(groups, "Dunn")?;
    let k = g.len();
    let all: Vec<f64> = g.iter().flatten().copied().collect();
    let nt = all.len() as f64;
    let (ranks, ties) = rank_with_ties(&all);
    let mut mean_ranks = Vec::with_capacity(k);
    let mut pos = 0;
    for v in &g {
        mean_ranks
            .push(compensated_sum(ranks[pos..pos + v.len()].iter().copied()) / v.len() as f64);
        pos += v.len();
    }
    let var0 = nt * (nt + 1.0) / 12.0 - tie_term(&ties) / (12.0 * (nt - 1.0));
    let n: Vec<usize> = g.iter().map(|v| v.len()).collect();
    let mut comps: Vec<Comparison> = dunn_pairs(k, control)?
        .into_iter()
        .map(|(i, j)| {
            let est = mean_ranks[i] - mean_ranks[j];
            let se = (var0 * (1.0 / n[i] as f64 + 1.0 / n[j] as f64)).sqrt();
            z_comparison(i, j, est, se)
        })
        .collect();
    finish_adjust(&mut comps, adjust, alpha);
    Ok(PostHoc {
        method: format!("Dunn ({})", adjust.name()),
        alpha,
        ms_within: f64::NAN,
        df_within: f64::INFINITY,
        critical_value: None,
        centers: mean_ranks,
        n,
        comparisons: comps,
    })
}

fn z_comparison(i: usize, j: usize, est: f64, se: f64) -> Comparison {
    let z = est / se;
    Comparison {
        i,
        j,
        estimate: est,
        se,
        statistic: z,
        df: f64::INFINITY,
        p_raw: if z.is_nan() {
            f64::NAN
        } else {
            p_two_sided_z(z)
        },
        p_adj: f64::NAN,
        ci: None,
        significant: false,
    }
}

/// Dunn-type comparisons after Friedman, as Prism documents them: ranks within each
/// subject, z = (Rbar_i - Rbar_j)/sqrt(k(k+1)/(6n)) (no tie correction), two-sided normal
/// p, adjusted over the family. Subjects with missing values are dropped.
pub fn dunn_friedman(
    m: &[Vec<f64>],
    control: Option<usize>,
    adjust: Adjust,
    alpha: f64,
) -> Result<PostHoc> {
    check_alpha(alpha)?;
    let (rows, _) = complete_rows(m, "Dunn (Friedman)")?;
    let n = rows.len();
    let k = rows.first().map_or(0, |r| r.len());
    if n < 2 || k < 2 {
        return Err(StatsError::NotEnoughData(
            "Dunn (Friedman): needs 2 conditions and 2 complete subjects".into(),
        ));
    }
    let mut sums = vec![0.0; k];
    for r in &rows {
        let (rk, _) = rank_with_ties(r);
        for (s, x) in sums.iter_mut().zip(rk) {
            *s += x;
        }
    }
    let (nf, kf) = (n as f64, k as f64);
    let mean_ranks: Vec<f64> = sums.iter().map(|s| s / nf).collect();
    let se = (kf * (kf + 1.0) / (6.0 * nf)).sqrt();
    let mut comps: Vec<Comparison> = dunn_pairs(k, control)?
        .into_iter()
        .map(|(i, j)| z_comparison(i, j, mean_ranks[i] - mean_ranks[j], se))
        .collect();
    finish_adjust(&mut comps, adjust, alpha);
    Ok(PostHoc {
        method: format!("Dunn after Friedman ({})", adjust.name()),
        alpha,
        ms_within: f64::NAN,
        df_within: f64::INFINITY,
        critical_value: None,
        centers: mean_ranks,
        n: vec![n; k],
        comparisons: comps,
    })
}

/// Compact letter display (Piepho 2004, J. Comput. Graph. Stat. 13:456, insert-absorb):
/// groups sharing a letter are not significantly different. Letters are assigned in order
/// of the smallest group index in each letter set. `k` is the number of groups; only
/// `significant` comparisons separate groups.
pub fn compact_letters(k: usize, comparisons: &[Comparison]) -> Vec<String> {
    let mut cols: Vec<Vec<bool>> = vec![vec![true; k]];
    for c in comparisons.iter().filter(|c| c.significant) {
        let (i, j) = (c.i, c.j);
        if i >= k || j >= k {
            continue;
        }
        let mut next = Vec::new();
        for col in cols.drain(..) {
            if col[i] && col[j] {
                let mut a = col.clone();
                a[i] = false;
                let mut b = col;
                b[j] = false;
                next.push(a);
                next.push(b);
            } else {
                next.push(col);
            }
        }
        // absorb: drop columns contained in another
        let mut kept: Vec<Vec<bool>> = Vec::new();
        for (x, col) in next.iter().enumerate() {
            let absorbed = next.iter().enumerate().any(|(y, other)| {
                y != x && col.iter().zip(other).all(|(&a, &b)| !a || b) && (col != other || y < x)
            });
            if !absorbed {
                kept.push(col.clone());
            }
        }
        cols = kept;
    }
    cols.retain(|c| c.iter().any(|&x| x));
    cols.sort_by_key(|c| c.iter().position(|&x| x).unwrap_or(k));
    let mut out = vec![String::new(); k];
    for (li, col) in cols.iter().enumerate() {
        let letter = letter_name(li);
        for (g, &member) in col.iter().enumerate() {
            if member {
                out[g].push_str(&letter);
            }
        }
    }
    out
}

fn letter_name(i: usize) -> String {
    let letters = b"abcdefghijklmnopqrstuvwxyz";
    if i < 26 {
        (letters[i] as char).to_string()
    } else {
        format!(
            "{}{}",
            letters[(i / 26 - 1) % 26] as char,
            letters[i % 26] as char
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[f64], b: &[f64]) {
        for (x, y) in a.iter().zip(b) {
            assert!(
                (x - y).abs() <= 1e-14 * y.abs().max(1e-300),
                "{a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn hand_computed_adjustments() {
        let p = [0.01, 0.04, 0.03, 0.2];
        close(&p_adjust(&p, Adjust::Bonferroni), &[0.04, 0.16, 0.12, 0.8]);
        // Holm: sorted .01,.03,.04,.2 -> .04,.09,.08->.09,.2 -> running max
        close(&p_adjust(&p, Adjust::Holm), &[0.04, 0.09, 0.09, 0.2]);
        // Hochberg: from top: .2*1=.2, .04*2=.08, .03*3=.09->min .08, .01*4=.04
        close(&p_adjust(&p, Adjust::Hochberg), &[0.04, 0.08, 0.08, 0.2]);
        // BH: .2*4/4=.2, .04*4/3=.05333, .03*4/2=.06->.05333, .01*4=.04
        close(
            &p_adjust(&p, Adjust::BenjaminiHochberg),
            &[0.04, 0.16 / 3.0, 0.16 / 3.0, 0.2],
        );
        // Sidak: 1 - 0.99^4
        let s = p_adjust(&p, Adjust::Sidak);
        assert!((s[0] - (1.0 - 0.99f64.powi(4))).abs() < 1e-15);
        // NaN passes through and does not count.
        let q = p_adjust(&[0.01, f64::NAN, 0.02], Adjust::Bonferroni);
        assert!(q[1].is_nan());
        close(&[q[0], q[2]], &[0.02, 0.04]);
    }

    #[test]
    fn two_stage_bky_hand() {
        // m = 4, alpha = 0.05: alpha' = 0.05/1.05. BH = [.04, .0533, .0533, .2]:
        // r1 = #{BH <= 0.047619} = 1, m0 = 3, adj = BH * 3/4 * 1.05.
        let p = [0.01, 0.04, 0.03, 0.2];
        let a = p_adjust(&p, Adjust::TwoStageBky { alpha: 0.05 });
        let bh = [0.04, 0.16 / 3.0, 0.16 / 3.0, 0.2];
        let want: Vec<f64> = bh.iter().map(|x| x * 0.75 * 1.05).collect();
        close(&a, &want);
        // all rejected: r1 = m -> BH * (1 + alpha)
        let a = p_adjust(&[0.001, 0.002], Adjust::TwoStageBky { alpha: 0.05 });
        close(&a, &[0.002 * 1.05, 0.002 * 1.05]);
    }

    #[test]
    fn letters() {
        let mk = |i, j, s| Comparison {
            i,
            j,
            estimate: 0.0,
            se: 1.0,
            statistic: 0.0,
            df: 1.0,
            p_raw: 0.0,
            p_adj: 0.0,
            ci: None,
            significant: s,
        };
        // 0 differs from 2 only: 0 -> a, 1 -> ab, 2 -> b
        let l = compact_letters(3, &[mk(0, 1, false), mk(0, 2, true), mk(1, 2, false)]);
        assert_eq!(l, vec!["a", "ab", "b"]);
    }
}
