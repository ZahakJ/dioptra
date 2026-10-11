//! Analysis of variance: one-way (with Welch and Brown–Forsythe tests of means and the
//! Bartlett, Brown–Forsythe and Levene tests of variances), two-way with interaction
//! (type I, II and III sums of squares from a sum-to-zero coded linear model),
//! two-way without replication, repeated-measures one-way with sphericity diagnostics,
//! and the rank-based Kruskal–Wallis and Friedman tests.
//!
//! Accuracy: every sum of squares is computed on data shifted by one of the observations
//! (exact by Sterbenz's lemma when the data share their leading digits, as in NIST
//! SmLs07-09), with two-pass corrected means and compensated sums, so constant leading
//! digits cost nothing.
//!
//! Missing values: `NaN`s are skipped in grouped data; repeated-measures and Friedman
//! drop every subject (row) with a missing value (listwise deletion) and report how many.

use crate::descriptive::{compensated_sum, mean_of, ss_of};
use crate::dist::{p_upper_chi2, p_upper_f};
use crate::linalg::{rss_of, Design};
use crate::rank::{rank_with_ties, tie_term};
use crate::{Result, StatsError};

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

fn not_enough(msg: impl Into<String>) -> StatsError {
    StatsError::NotEnoughData(msg.into())
}

/// Non-missing values of each group; error on infinities or an empty group.
pub(crate) fn clean_groups(groups: &[&[f64]], what: &str) -> Result<Vec<Vec<f64>>> {
    let mut out = Vec::with_capacity(groups.len());
    for (i, g) in groups.iter().enumerate() {
        let mut v = Vec::with_capacity(g.len());
        for &x in g.iter() {
            if x.is_nan() {
                continue;
            }
            if x.is_infinite() {
                return Err(invalid(format!("{what}: infinite value in group {i}")));
            }
            v.push(x);
        }
        if v.is_empty() {
            return Err(not_enough(format!("{what}: group {i} has no values")));
        }
        out.push(v);
    }
    if out.len() < 2 {
        return Err(not_enough(format!("{what} needs at least two groups")));
    }
    Ok(out)
}

/// The first value of the data, used as the shift for every sum of squares.
fn shift_of(groups: &[Vec<f64>]) -> f64 {
    groups
        .iter()
        .find_map(|g| g.first().copied())
        .unwrap_or(0.0)
}

/// Per-group (n, mean of shifted data, SS about the mean) on data shifted by `c`.
struct Shifted {
    c: f64,
    d: Vec<Vec<f64>>,
    n: Vec<f64>,
    mean: Vec<f64>,
    ss: Vec<f64>,
}

impl Shifted {
    fn new(groups: &[Vec<f64>]) -> Self {
        let c = shift_of(groups);
        let d: Vec<Vec<f64>> = groups
            .iter()
            .map(|g| g.iter().map(|x| x - c).collect())
            .collect();
        let n = d.iter().map(|g| g.len() as f64).collect();
        let mean: Vec<f64> = d.iter().map(|g| mean_of(g)).collect();
        let ss = d.iter().zip(&mean).map(|(g, &m)| ss_of(g, m)).collect();
        Shifted { c, d, n, mean, ss }
    }
    fn var(&self, i: usize) -> f64 {
        self.ss[i] / (self.n[i] - 1.0)
    }
}

// =======================================================================================
// One-way ANOVA
// =======================================================================================

/// n, mean, SD and SEM of one group.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GroupSummary {
    /// Number of non-missing values.
    pub n: usize,
    /// Mean.
    pub mean: f64,
    /// Sample SD (n - 1); NaN when n = 1.
    pub sd: f64,
    /// SD / sqrt(n).
    pub sem: f64,
}

/// A heteroscedasticity-robust F test of equal means.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RobustAnova {
    /// Test name.
    pub method: String,
    /// F statistic.
    pub f: f64,
    /// Numerator df.
    pub df1: f64,
    /// Denominator df (Satterthwaite-type, fractional).
    pub df2: f64,
    /// P(F_{df1, df2} >= f).
    pub p: f64,
}

/// Brown–Forsythe F* test of equal means, with both numerator-df conventions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrownForsytheAnova {
    /// F* = SS_between / sum (1 - n_i/N) s_i^2.
    pub f: f64,
    /// Numerator df k - 1 (Brown & Forsythe 1974).
    pub df1: f64,
    /// Satterthwaite denominator df.
    pub df2: f64,
    /// p on (k - 1, df2): the 1974 test (Prism's "Brown-Forsythe ANOVA").
    pub p: f64,
    /// Mehrotra (1997) modified numerator df (statsmodels `anova_oneway(use_var='bf')`).
    pub mehrotra_df1: f64,
    /// p on (mehrotra_df1, df2).
    pub mehrotra_p: f64,
}

/// A test of equal variances.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VarianceTest {
    /// Test name.
    pub method: String,
    /// Statistic (F for Levene-type tests, chi-square for Bartlett).
    pub statistic: f64,
    /// df (chi-square) or numerator df (F).
    pub df1: f64,
    /// Denominator df (F tests); NaN for chi-square tests.
    pub df2: f64,
    /// p-value.
    pub p: f64,
}

/// One-way ANOVA table, effect sizes, group summaries and the companion tests.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OneWayAnova {
    /// Number of groups.
    pub k: usize,
    /// Total number of values.
    pub n_total: usize,
    /// Between-groups sum of squares sum n_i (mean_i - grand mean)^2.
    pub ss_between: f64,
    /// Within-groups (residual) sum of squares.
    pub ss_within: f64,
    /// Total sum of squares about the grand mean (computed directly).
    pub ss_total: f64,
    /// k - 1.
    pub df_between: f64,
    /// N - k.
    pub df_within: f64,
    /// N - 1.
    pub df_total: f64,
    /// SS_between / df_between.
    pub ms_between: f64,
    /// SS_within / df_within.
    pub ms_within: f64,
    /// F = MS_between / MS_within.
    pub f: f64,
    /// P(F_{k-1, N-k} >= F).
    pub p: f64,
    /// R^2 = eta^2 = SS_between / SS_total.
    pub r_squared: f64,
    /// omega^2 = (SS_between - (k-1) MS_within) / (SS_total + MS_within) (Hays 1963).
    pub omega_squared: f64,
    /// sqrt(MS_within).
    pub residual_sd: f64,
    /// Grand mean.
    pub grand_mean: f64,
    /// Per-group n, mean, SD, SEM.
    pub groups: Vec<GroupSummary>,
    /// Welch's test (None if a group has n < 2 or zero variance).
    pub welch: Option<RobustAnova>,
    /// Brown–Forsythe F* test of means (None if a group has n < 2).
    pub brown_forsythe: Option<BrownForsytheAnova>,
    /// Bartlett's test of equal variances (None if a group has n < 2 or zero variance).
    pub bartlett: Option<VarianceTest>,
    /// Brown–Forsythe test of equal variances (Levene on |x - median|).
    pub brown_forsythe_variance: Option<VarianceTest>,
    /// Levene's original test (on |x - mean|).
    pub levene_mean: Option<VarianceTest>,
}

/// One-way ANOVA (Fisher 1925). Groups are slices; `NaN`s are skipped. Needs at least two
/// non-empty groups and N > k.
///
/// References: Fisher (1925), *Statistical Methods for Research Workers*; effect sizes
/// Hays (1963); companion tests as in [`welch_anova`], [`brown_forsythe_anova`],
/// [`bartlett`], [`levene`].
pub fn one_way_anova(groups: &[&[f64]]) -> Result<OneWayAnova> {
    let g = clean_groups(groups, "one-way ANOVA")?;
    let k = g.len();
    let n_total: usize = g.iter().map(|v| v.len()).sum();
    if n_total <= k {
        return Err(not_enough(
            "one-way ANOVA needs more values than groups (N > k)",
        ));
    }
    let s = Shifted::new(&g);
    let all: Vec<f64> = s.d.iter().flatten().copied().collect();
    let gm = mean_of(&all);
    let ss_total = ss_of(&all, gm);
    let ss_between = compensated_sum((0..k).map(|i| {
        let dm = s.mean[i] - gm;
        s.n[i] * dm * dm
    }));
    let ss_within = compensated_sum(s.ss.iter().copied());
    let df_b = (k - 1) as f64;
    let df_w = (n_total - k) as f64;
    let ms_b = ss_between / df_b;
    let ms_w = ss_within / df_w;
    let f = ms_b / ms_w;
    let p = if f.is_nan() {
        f64::NAN
    } else {
        p_upper_f(f, df_b, df_w)
    };
    let groups_summary = (0..k)
        .map(|i| {
            let sd = if s.n[i] > 1.0 {
                s.var(i).sqrt()
            } else {
                f64::NAN
            };
            GroupSummary {
                n: g[i].len(),
                mean: s.mean[i] + s.c,
                sd,
                sem: sd / s.n[i].sqrt(),
            }
        })
        .collect();
    let refs: Vec<&[f64]> = g.iter().map(|v| v.as_slice()).collect();
    Ok(OneWayAnova {
        k,
        n_total,
        ss_between,
        ss_within,
        ss_total,
        df_between: df_b,
        df_within: df_w,
        df_total: (n_total - 1) as f64,
        ms_between: ms_b,
        ms_within: ms_w,
        f,
        p,
        r_squared: ss_between / ss_total,
        omega_squared: (ss_between - df_b * ms_w) / (ss_total + ms_w),
        residual_sd: ms_w.sqrt(),
        grand_mean: gm + s.c,
        groups: groups_summary,
        welch: welch_anova(&refs).ok(),
        brown_forsythe: brown_forsythe_anova(&refs).ok(),
        bartlett: bartlett(&refs).ok(),
        brown_forsythe_variance: levene(&refs, LeveneCenter::Median).ok(),
        levene_mean: levene(&refs, LeveneCenter::Mean).ok(),
    })
}

fn need_n2(s: &Shifted, what: &str) -> Result<()> {
    if s.n.iter().any(|&n| n < 2.0) {
        return Err(not_enough(format!("{what}: every group needs n >= 2")));
    }
    Ok(())
}

/// Welch's heteroscedastic one-way ANOVA (Welch 1951, Biometrika 38:330,
/// doi:10.2307/2332579): w_i = n_i/s_i^2, F = [sum w_i (m_i - m_w)^2/(k-1)] /
/// [1 + 2(k-2)/(k^2-1) Lambda], Lambda = sum (1 - w_i/W)^2/(n_i - 1), df = (k-1,
/// (k^2-1)/(3 Lambda)). Same as statsmodels `anova_oneway(use_var='unequal')` and R
/// `oneway.test`.
pub fn welch_anova(groups: &[&[f64]]) -> Result<RobustAnova> {
    let g = clean_groups(groups, "Welch ANOVA")?;
    let s = Shifted::new(&g);
    need_n2(&s, "Welch ANOVA")?;
    let k = g.len();
    let kf = k as f64;
    let w: Vec<f64> = (0..k).map(|i| s.n[i] / s.var(i)).collect();
    if w.iter().any(|x| !x.is_finite()) {
        return Err(invalid("Welch ANOVA: a group has zero variance"));
    }
    let wsum = compensated_sum(w.iter().copied());
    let mw = compensated_sum((0..k).map(|i| w[i] * s.mean[i])) / wsum;
    let a = compensated_sum((0..k).map(|i| w[i] * (s.mean[i] - mw).powi(2))) / (kf - 1.0);
    let lambda = compensated_sum((0..k).map(|i| (1.0 - w[i] / wsum).powi(2) / (s.n[i] - 1.0)));
    let f = a / (1.0 + 2.0 * (kf - 2.0) / (kf * kf - 1.0) * lambda);
    let df1 = kf - 1.0;
    let df2 = (kf * kf - 1.0) / (3.0 * lambda);
    Ok(RobustAnova {
        method: "Welch ANOVA".into(),
        f,
        df1,
        df2,
        p: p_upper_f(f, df1, df2),
    })
}

/// Brown–Forsythe F* test of equal means under unequal variances (Brown & Forsythe 1974,
/// Technometrics 16:129, doi:10.2307/1267501): F* = SS_between / sum (1 - n_i/N) s_i^2,
/// Satterthwaite denominator df 1/df2 = sum c_i^2/(n_i - 1),
/// c_i = (1 - n_i/N) s_i^2 / sum_j (1 - n_j/N) s_j^2. Also reports Mehrotra's (1997,
/// doi:10.1080/03610919708813431) numerator df
/// (sum (1-n_i/N) s_i^2)^2 / (sum s_i^4 + (sum n_i s_i^2/N)^2 - 2 sum n_i s_i^4/N), the
/// statsmodels default. This is Prism's "Brown-Forsythe ANOVA" (a test of means; compare
/// [`levene`] with the median, the Brown–Forsythe test of variances).
pub fn brown_forsythe_anova(groups: &[&[f64]]) -> Result<BrownForsytheAnova> {
    let g = clean_groups(groups, "Brown-Forsythe ANOVA")?;
    let s = Shifted::new(&g);
    need_n2(&s, "Brown-Forsythe ANOVA")?;
    let k = g.len();
    let nt: f64 = s.n.iter().sum();
    let all: Vec<f64> = s.d.iter().flatten().copied().collect();
    let gm = mean_of(&all);
    let ssb = compensated_sum((0..k).map(|i| s.n[i] * (s.mean[i] - gm).powi(2)));
    let v: Vec<f64> = (0..k).map(|i| s.var(i)).collect();
    let terms: Vec<f64> = (0..k).map(|i| (1.0 - s.n[i] / nt) * v[i]).collect();
    let denom = compensated_sum(terms.iter().copied());
    if !(denom > 0.0) {
        return Err(invalid(
            "Brown-Forsythe ANOVA: all groups have zero variance",
        ));
    }
    let f = ssb / denom;
    let df2 = 1.0 / compensated_sum((0..k).map(|i| (terms[i] / denom).powi(2) / (s.n[i] - 1.0)));
    let df1 = (k - 1) as f64;
    let m_den = compensated_sum(v.iter().map(|x| x * x))
        + compensated_sum((0..k).map(|i| s.n[i] * v[i] / nt)).powi(2)
        - 2.0 * compensated_sum((0..k).map(|i| s.n[i] * v[i] * v[i] / nt));
    let mdf1 = denom * denom / m_den;
    Ok(BrownForsytheAnova {
        f,
        df1,
        df2,
        p: p_upper_f(f, df1, df2),
        mehrotra_df1: mdf1,
        mehrotra_p: p_upper_f(f, mdf1, df2),
    })
}

/// Bartlett's test of equal variances (Bartlett 1937, Proc. R. Soc. A 160:268;
/// Snedecor & Cochran 1989 §13.9): T = [(N-k) ln s_p^2 - sum (n_i-1) ln s_i^2] /
/// [1 + (sum 1/(n_i-1) - 1/(N-k))/(3(k-1))], chi-square with k-1 df. SciPy `bartlett`.
pub fn bartlett(groups: &[&[f64]]) -> Result<VarianceTest> {
    let g = clean_groups(groups, "Bartlett's test")?;
    let s = Shifted::new(&g);
    need_n2(&s, "Bartlett's test")?;
    let k = g.len();
    let kf = k as f64;
    let nt: f64 = s.n.iter().sum();
    let sp2 = compensated_sum(s.ss.iter().copied()) / (nt - kf);
    if (0..k).any(|i| !(s.var(i) > 0.0)) {
        return Err(invalid("Bartlett's test: a group has zero variance"));
    }
    let num =
        (nt - kf) * sp2.ln() - compensated_sum((0..k).map(|i| (s.n[i] - 1.0) * s.var(i).ln()));
    let c = 1.0
        + (compensated_sum((0..k).map(|i| 1.0 / (s.n[i] - 1.0))) - 1.0 / (nt - kf))
            / (3.0 * (kf - 1.0));
    let t = num / c;
    Ok(VarianceTest {
        method: "Bartlett".into(),
        statistic: t,
        df1: kf - 1.0,
        df2: f64::NAN,
        p: p_upper_chi2(t, kf - 1.0),
    })
}

/// Centre used by [`levene`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LeveneCenter {
    /// Group mean: Levene's (1960) original test.
    Mean,
    /// Group median: the Brown–Forsythe (1974, JASA 69:364) test of equal variances.
    Median,
}

/// Levene-type test of equal variances: one-way ANOVA F on z_ij = |x_ij - centre_i|
/// (Levene 1960; Brown & Forsythe 1974, doi:10.1080/01621459.1974.10482955, with the
/// median). SciPy `levene(center=...)`.
pub fn levene(groups: &[&[f64]], center: LeveneCenter) -> Result<VarianceTest> {
    let g = clean_groups(groups, "Levene's test")?;
    let z: Vec<Vec<f64>> = g
        .iter()
        .map(|v| {
            let c = match center {
                LeveneCenter::Mean => mean_of(v),
                LeveneCenter::Median => median_of(v),
            };
            v.iter().map(|x| (x - c).abs()).collect()
        })
        .collect();
    let refs: Vec<&[f64]> = z.iter().map(|v| v.as_slice()).collect();
    let a = one_way_core(&refs)?;
    Ok(VarianceTest {
        method: match center {
            LeveneCenter::Mean => "Levene (mean)".into(),
            LeveneCenter::Median => "Brown-Forsythe (median)".into(),
        },
        statistic: a.0,
        df1: a.1,
        df2: a.2,
        p: a.3,
    })
}

fn median_of(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// (F, df1, df2, p) of a plain one-way ANOVA without the companion tests.
fn one_way_core(groups: &[&[f64]]) -> Result<(f64, f64, f64, f64)> {
    let g = clean_groups(groups, "one-way ANOVA")?;
    let k = g.len();
    let nt: usize = g.iter().map(|v| v.len()).sum();
    if nt <= k {
        return Err(not_enough("one-way ANOVA needs N > k"));
    }
    let s = Shifted::new(&g);
    let all: Vec<f64> = s.d.iter().flatten().copied().collect();
    let gm = mean_of(&all);
    let ssb = compensated_sum((0..k).map(|i| s.n[i] * (s.mean[i] - gm).powi(2)));
    let ssw = compensated_sum(s.ss.iter().copied());
    let (d1, d2) = ((k - 1) as f64, (nt - k) as f64);
    let f = (ssb / d1) / (ssw / d2);
    Ok((
        f,
        d1,
        d2,
        if f.is_nan() {
            f64::NAN
        } else {
            p_upper_f(f, d1, d2)
        },
    ))
}

// =======================================================================================
// Two-way ANOVA
// =======================================================================================

/// Which sums of squares a two-way ANOVA reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SsType {
    /// Sequential: A, then B given A, then A:B given both.
    I,
    /// Each main effect adjusted for the other main effect (not for the interaction).
    II,
    /// Each term adjusted for every other term, with sum-to-zero coding (Prism's choice).
    #[default]
    III,
}

/// Options for [`two_way_anova`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TwoWayOptions {
    /// Type of sums of squares.
    pub ss_type: SsType,
    /// Fit the A x B interaction. Ignored (treated as false) when no cell has more than
    /// one value: the interaction is then the residual (two-way ANOVA without
    /// replication, Tukey's additive model).
    pub interaction: bool,
}

impl Default for TwoWayOptions {
    fn default() -> Self {
        TwoWayOptions {
            ss_type: SsType::III,
            interaction: true,
        }
    }
}

/// One row of an ANOVA table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnovaTerm {
    /// "Intercept", "A" (rows), "B" (columns), "A:B" or "Residual".
    pub name: String,
    /// Sum of squares.
    pub ss: f64,
    /// Degrees of freedom.
    pub df: f64,
    /// Mean square SS/df.
    pub ms: f64,
    /// F = MS / MS_residual (NaN for the residual row).
    pub f: f64,
    /// P(F >= f) (NaN for the residual row).
    pub p: f64,
    /// Partial eta^2 = SS / (SS + SS_residual) (NaN for the residual row).
    pub partial_eta_squared: f64,
}

/// Two-way ANOVA result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TwoWayAnova {
    /// Type of sums of squares in `terms`.
    pub ss_type: SsType,
    /// Whether the interaction was fitted.
    pub interaction: bool,
    /// Intercept row (type III only, as statsmodels `anova_lm(typ=3)` reports it).
    pub intercept: Option<AnovaTerm>,
    /// A (row factor), B (column factor) and, if fitted, A:B.
    pub terms: Vec<AnovaTerm>,
    /// Residual row.
    pub residual: AnovaTerm,
    /// Number of values used.
    pub n: usize,
    /// Number of (row, column) cells with no data.
    pub empty_cells: usize,
    /// Rank of the full model's design.
    pub rank: usize,
    /// Cell means [row][column] (NaN for empty cells).
    pub cell_means: Vec<Vec<f64>>,
    /// Cell sizes [row][column].
    pub cell_n: Vec<Vec<usize>>,
}

/// Group long-format data (row-factor level, column-factor level, value) into the cell
/// array `[row][column] -> values` that [`two_way_anova`] takes.
pub fn cells_from_long(a: &[usize], b: &[usize], y: &[f64]) -> Result<Vec<Vec<Vec<f64>>>> {
    if a.len() != y.len() || b.len() != y.len() {
        return Err(invalid("cells_from_long: lengths differ"));
    }
    let ra = a.iter().max().map_or(0, |m| m + 1);
    let rb = b.iter().max().map_or(0, |m| m + 1);
    let mut cells = vec![vec![Vec::new(); rb]; ra];
    for ((&i, &j), &v) in a.iter().zip(b).zip(y) {
        cells[i][j].push(v);
    }
    Ok(cells)
}

/// Two-way ANOVA from a cell array `cells[row][column] = replicates` (NaN skipped).
/// Unequal cell sizes and empty cells are allowed where the effects remain estimable.
///
/// Fits y = mu + alpha_i + beta_j (+ gamma_ij) by least squares with sum-to-zero
/// (deviation) coding — patsy's `C(., Sum)` — through the refined Householder QR of
/// `linalg`, and obtains every sum of squares as a difference of residual sums of squares
/// of nested models: type I (sequential A, B, A:B), type II (each main effect given the
/// other), type III (each term given all others, Yates' weighted squares of means), as
/// statsmodels `anova_lm(typ=1|2|3)` on `y ~ C(A, Sum) * C(B, Sum)`. The df of a term is
/// the rank it adds. References: Searle (1987), *Linear Models for Unbalanced Data*;
/// Langsrud (2003), Statistics and Computing 13:163, doi:10.1023/A:1023260610025.
pub fn two_way_anova(cells: &[Vec<Vec<f64>>], opts: TwoWayOptions) -> Result<TwoWayAnova> {
    let ra = cells.len();
    let rb = cells.first().map_or(0, |r| r.len());
    if ra < 2 || rb < 2 {
        return Err(not_enough("two-way ANOVA needs at least 2 x 2 cells"));
    }
    if cells.iter().any(|r| r.len() != rb) {
        return Err(invalid("two-way ANOVA: ragged cell array"));
    }
    let mut ai = Vec::new();
    let mut bi = Vec::new();
    let mut y = Vec::new();
    let mut cell_n = vec![vec![0usize; rb]; ra];
    let mut cell_means = vec![vec![f64::NAN; rb]; ra];
    for (i, row) in cells.iter().enumerate() {
        for (j, cell) in row.iter().enumerate() {
            let v: Vec<f64> = cell.iter().copied().filter(|x| !x.is_nan()).collect();
            if v.iter().any(|x| x.is_infinite()) {
                return Err(invalid("two-way ANOVA: infinite value"));
            }
            cell_n[i][j] = v.len();
            if !v.is_empty() {
                cell_means[i][j] = mean_of(&v);
            }
            for x in v {
                ai.push(i);
                bi.push(j);
                y.push(x);
            }
        }
    }
    for (i, row) in cell_n.iter().enumerate() {
        if row.iter().all(|&c| c == 0) {
            return Err(not_enough(format!(
                "two-way ANOVA: row level {i} has no data"
            )));
        }
    }
    for j in 0..rb {
        if cell_n.iter().all(|r| r[j] == 0) {
            return Err(not_enough(format!(
                "two-way ANOVA: column level {j} has no data"
            )));
        }
    }
    let n = y.len();
    let empty_cells = cell_n.iter().flatten().filter(|&&c| c == 0).count();
    let replicated = cell_n.iter().flatten().any(|&c| c > 1);
    let interaction = opts.interaction && replicated;
    let c = y[0];
    let yd: Vec<f64> = y.iter().map(|v| v - c).collect();

    // Columns: 0 intercept | A: 1..ra | B: ra..ra+rb-1 | AB: products.
    let sum_code = |level: usize, levels: usize, col: usize| -> f64 {
        if level == levels - 1 {
            -1.0
        } else if level == col {
            1.0
        } else {
            0.0
        }
    };
    let mut cols: Vec<Vec<f64>> = vec![vec![1.0; n]];
    for l in 0..ra - 1 {
        cols.push(ai.iter().map(|&a| sum_code(a, ra, l)).collect());
    }
    for l in 0..rb - 1 {
        cols.push(bi.iter().map(|&b| sum_code(b, rb, l)).collect());
    }
    let a_cols: Vec<usize> = (1..ra).collect();
    let b_cols: Vec<usize> = (ra..ra + rb - 1).collect();
    let mut ab_cols = Vec::new();
    if interaction {
        for &ca in &a_cols {
            for &cb in &b_cols {
                let col: Vec<f64> = (0..n).map(|r| cols[ca][r] * cols[cb][r]).collect();
                ab_cols.push(cols.len());
                cols.push(col);
            }
        }
    }
    let design = Design::from_columns(&cols);
    let join = |parts: &[&[usize]]| -> Vec<usize> {
        let mut v: Vec<usize> = parts.iter().flat_map(|p| p.iter().copied()).collect();
        v.sort_unstable();
        v
    };
    let icpt: &[usize] = &[0];
    let full = join(&[icpt, &a_cols, &b_cols, &ab_cols]);
    let (rss_full, rank_full) = rss_of(&design, &full, &yd);
    let df_res = (n - rank_full) as f64;
    if !(df_res > 0.0) {
        return Err(not_enough(
            "two-way ANOVA: no residual degrees of freedom (add replicates or drop the interaction)",
        ));
    }
    let ms_res = rss_full / df_res;
    let term = |name: &str, (ss, df): (f64, f64)| -> AnovaTerm {
        let ss = ss.max(0.0);
        let ms = ss / df;
        let f = ms / ms_res;
        AnovaTerm {
            name: name.into(),
            ss,
            df,
            ms,
            f,
            p: if f.is_nan() {
                f64::NAN
            } else {
                p_upper_f(f, df, df_res)
            },
            partial_eta_squared: ss / (ss + rss_full),
        }
    };
    let diff = |reduced: &[usize], bigger: &[usize]| -> (f64, f64) {
        let (r0, k0) = rss_of(&design, reduced, &yd);
        let (r1, k1) = rss_of(&design, bigger, &yd);
        (r0 - r1, (k1 - k0) as f64)
    };
    let without = |drop: &[usize]| -> Vec<usize> {
        full.iter().copied().filter(|c| !drop.contains(c)).collect()
    };
    let mut terms = Vec::new();
    let mut intercept = None;
    let main = join(&[icpt, &a_cols, &b_cols]);
    match opts.ss_type {
        SsType::I => {
            let ia = join(&[icpt, &a_cols]);
            terms.push(term("A", diff(icpt, &ia)));
            terms.push(term("B", diff(&ia, &main)));
            if interaction {
                terms.push(term("A:B", diff(&main, &full)));
            }
        }
        SsType::II => {
            let ia = join(&[icpt, &a_cols]);
            let ib = join(&[icpt, &b_cols]);
            terms.push(term("A", diff(&ib, &main)));
            terms.push(term("B", diff(&ia, &main)));
            if interaction {
                terms.push(term("A:B", diff(&main, &full)));
            }
        }
        SsType::III => {
            // Intercept SS on the original (unshifted) response.
            let (r0, k0) = rss_of(&design, &without(icpt), &y);
            let (r1, k1) = rss_of(&design, &full, &y);
            intercept = Some(term("Intercept", (r0 - r1, (k1 - k0) as f64)));
            terms.push(term("A", diff(&without(&a_cols), &full)));
            terms.push(term("B", diff(&without(&b_cols), &full)));
            if interaction {
                terms.push(term("A:B", diff(&without(&ab_cols), &full)));
            }
        }
    }
    let residual = AnovaTerm {
        name: "Residual".into(),
        ss: rss_full,
        df: df_res,
        ms: ms_res,
        f: f64::NAN,
        p: f64::NAN,
        partial_eta_squared: f64::NAN,
    };
    Ok(TwoWayAnova {
        ss_type: opts.ss_type,
        interaction,
        intercept,
        terms,
        residual,
        n,
        empty_cells,
        rank: rank_full,
        cell_means,
        cell_n,
    })
}

// =======================================================================================
// Repeated-measures one-way ANOVA
// =======================================================================================

/// Rows (subjects) with no missing value, and how many were dropped.
pub(crate) fn complete_rows(m: &[Vec<f64>], what: &str) -> Result<(Vec<Vec<f64>>, usize)> {
    let k = m.first().map_or(0, |r| r.len());
    if m.iter().any(|r| r.len() != k) {
        return Err(invalid(format!("{what}: rows have different lengths")));
    }
    let mut out = Vec::new();
    for r in m {
        if r.iter().any(|x| x.is_infinite()) {
            return Err(invalid(format!("{what}: infinite value")));
        }
        if r.iter().all(|x| !x.is_nan()) {
            out.push(r.clone());
        }
    }
    let dropped = m.len() - out.len();
    Ok((out, dropped))
}

/// Repeated-measures one-way ANOVA with sphericity diagnostics.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RmAnova {
    /// Subjects used (complete rows).
    pub n_subjects: usize,
    /// Subjects dropped for missing values (listwise deletion).
    pub n_excluded: usize,
    /// Number of conditions (treatments).
    pub k: usize,
    /// Treatment (conditions) sum of squares.
    pub ss_treatment: f64,
    /// Between-subjects sum of squares.
    pub ss_subjects: f64,
    /// Residual (subjects x treatment) sum of squares.
    pub ss_error: f64,
    /// Total sum of squares.
    pub ss_total: f64,
    /// k - 1.
    pub df_treatment: f64,
    /// n - 1.
    pub df_subjects: f64,
    /// (n - 1)(k - 1).
    pub df_error: f64,
    /// SS_treatment / df_treatment.
    pub ms_treatment: f64,
    /// SS_subjects / df_subjects.
    pub ms_subjects: f64,
    /// SS_error / df_error.
    pub ms_error: f64,
    /// F = MS_treatment / MS_error.
    pub f: f64,
    /// Sphericity-assumed p.
    pub p: f64,
    /// F for the subjects ("was the matching effective?") = MS_subjects / MS_error.
    pub f_subjects: f64,
    /// p of `f_subjects`.
    pub p_subjects: f64,
    /// Partial eta^2 = SS_treatment / (SS_treatment + SS_error).
    pub partial_eta_squared: f64,
    /// Greenhouse–Geisser (Box) epsilon from orthonormal contrasts.
    pub gg_epsilon: f64,
    /// The same epsilon from Box's double-centred covariance formula (a cross-check).
    pub gg_epsilon_box_formula: f64,
    /// Huynh–Feldt epsilon with the Lecoutre (1991) correction, before capping at 1.
    pub hf_epsilon_uncapped: f64,
    /// Huynh–Feldt epsilon capped at 1.
    pub hf_epsilon: f64,
    /// Lower-bound epsilon 1/(k-1).
    pub lower_bound_epsilon: f64,
    /// GG-corrected numerator df.
    pub df1_gg: f64,
    /// GG-corrected denominator df.
    pub df2_gg: f64,
    /// GG-corrected p (Prism's "Geisser-Greenhouse correction").
    pub p_gg: f64,
    /// HF-corrected numerator df.
    pub df1_hf: f64,
    /// HF-corrected denominator df.
    pub df2_hf: f64,
    /// HF-corrected p.
    pub p_hf: f64,
    /// Lower-bound-corrected p.
    pub p_lower_bound: f64,
    /// Mauchly's W (1 when k = 2).
    pub mauchly_w: f64,
    /// Mauchly's chi-square approximation.
    pub mauchly_chi2: f64,
    /// Its df k(k-1)/2 - 1.
    pub mauchly_df: f64,
    /// Its p (NaN when df = 0, i.e. k = 2).
    pub mauchly_p: f64,
    /// Condition means.
    pub condition_means: Vec<f64>,
}

/// Repeated-measures one-way ANOVA from a subjects x conditions matrix (`m[subject][condition]`).
/// Subjects with any missing value are dropped (count in `n_excluded`).
///
/// SS partition as statsmodels `AnovaRM`. Sphericity: S = sample covariance of the
/// conditions, C = orthonormal Helmert contrasts, S_c = C S C';
/// Greenhouse–Geisser epsilon tr(S_c)^2/((k-1) tr(S_c^2)) (Box 1954; Greenhouse & Geisser
/// 1959, doi:10.1007/BF02289823); Huynh–Feldt with Lecoutre's (1991,
/// doi:10.3102/10769986016004371) correction (n(k-1)e - 2)/((k-1)(n-1-(k-1)e)); Mauchly's
/// W = det(S_c)/(tr(S_c)/(k-1))^(k-1) with the chi-square approximation
/// -(n-1)(1 - (2p^2+p+2)/(6p(n-1))) ln W, p = k-1 (Mauchly 1940,
/// doi:10.1214/aoms/1177731915). Corrected tests multiply both df by epsilon.
pub fn rm_anova(m: &[Vec<f64>]) -> Result<RmAnova> {
    let (rows, n_excluded) = complete_rows(m, "repeated-measures ANOVA")?;
    let n = rows.len();
    let k = rows.first().map_or(0, |r| r.len());
    if k < 2 {
        return Err(not_enough(
            "repeated-measures ANOVA needs at least 2 conditions",
        ));
    }
    if n < 2 {
        return Err(not_enough(
            "repeated-measures ANOVA needs at least 2 complete subjects",
        ));
    }
    let c = rows[0][0];
    let d: Vec<Vec<f64>> = rows
        .iter()
        .map(|r| r.iter().map(|x| x - c).collect())
        .collect();
    let all: Vec<f64> = d.iter().flatten().copied().collect();
    let gm = mean_of(&all);
    let col_means: Vec<f64> = (0..k)
        .map(|j| mean_of(&d.iter().map(|r| r[j]).collect::<Vec<_>>()))
        .collect();
    let row_means: Vec<f64> = d.iter().map(|r| mean_of(r)).collect();
    let (nf, kf) = (n as f64, k as f64);
    let ss_total = ss_of(&all, gm);
    let ss_treat = nf * compensated_sum(col_means.iter().map(|m| (m - gm).powi(2)));
    let ss_subj = kf * compensated_sum(row_means.iter().map(|m| (m - gm).powi(2)));
    let ss_err = compensated_sum((0..n).flat_map(|i| {
        let d = &d;
        let row_means = &row_means;
        let col_means = &col_means;
        (0..k).map(move |j| {
            let e = d[i][j] - row_means[i] - col_means[j] + gm;
            e * e
        })
    }));
    let df_t = kf - 1.0;
    let df_s = nf - 1.0;
    let df_e = df_t * df_s;
    let ms_t = ss_treat / df_t;
    let ms_s = ss_subj / df_s;
    let ms_e = ss_err / df_e;
    let f = ms_t / ms_e;
    let p = p_upper_f(f, df_t, df_e);
    let f_s = ms_s / ms_e;

    // Sample covariance S (n - 1).
    let mut s = vec![vec![0.0; k]; k];
    for a in 0..k {
        for b in a..k {
            let v = compensated_sum(
                (0..n).map(|i| (d[i][a] - col_means[a]) * (d[i][b] - col_means[b])),
            ) / df_s;
            s[a][b] = v;
            s[b][a] = v;
        }
    }
    let p_dim = k - 1;
    // Orthonormal Helmert contrasts.
    let mut cm = nalgebra::DMatrix::<f64>::zeros(p_dim, k);
    for r in 1..k {
        let rf = r as f64;
        let norm = (rf * (rf + 1.0)).sqrt();
        for j in 0..r {
            cm[(r - 1, j)] = 1.0 / norm;
        }
        cm[(r - 1, r)] = -rf / norm;
    }
    let sm = nalgebra::DMatrix::from_fn(k, k, |i, j| s[i][j]);
    let sc = &cm * sm * cm.transpose();
    let tr = sc.trace();
    let tr2 = (&sc * &sc).trace();
    let pf = p_dim as f64;
    let gg = (tr * tr / (pf * tr2)).clamp(1.0 / pf, 1.0);
    // Box's double-centred form.
    let smean = s.iter().flatten().sum::<f64>() / (kf * kf);
    let sdiag = (0..k).map(|i| s[i][i]).sum::<f64>() / kf;
    let rowm: Vec<f64> = s.iter().map(|r| r.iter().sum::<f64>() / kf).collect();
    let sum_sq: f64 = s.iter().flatten().map(|x| x * x).sum();
    let box_eps = kf * kf * (sdiag - smean).powi(2)
        / (pf
            * (sum_sq - 2.0 * kf * rowm.iter().map(|x| x * x).sum::<f64>()
                + kf * kf * smean * smean));
    let hf_un = (nf * pf * gg - 2.0) / (pf * (nf - 1.0 - pf * gg));
    let hf = hf_un.min(1.0);
    let lb = 1.0 / pf;
    let corrected = |e: f64| -> (f64, f64, f64) {
        let (d1, d2) = (e * df_t, e * df_e);
        (d1, d2, p_upper_f(f, d1, d2))
    };
    let (d1g, d2g, pgg) = corrected(gg);
    let (d1h, d2h, phf) = corrected(hf);
    let (_, _, plb) = corrected(lb);
    // Mauchly.
    let det = sc.clone().determinant();
    let w = if p_dim == 1 {
        1.0
    } else {
        det / (tr / pf).powf(pf)
    };
    let mdf = pf * (pf + 1.0) / 2.0 - 1.0;
    let chi2 = -(nf - 1.0) * (1.0 - (2.0 * pf * pf + pf + 2.0) / (6.0 * pf * (nf - 1.0))) * w.ln();
    let mp = if mdf > 0.0 && chi2.is_finite() {
        p_upper_chi2(chi2, mdf)
    } else {
        f64::NAN
    };
    Ok(RmAnova {
        n_subjects: n,
        n_excluded,
        k,
        ss_treatment: ss_treat,
        ss_subjects: ss_subj,
        ss_error: ss_err,
        ss_total,
        df_treatment: df_t,
        df_subjects: df_s,
        df_error: df_e,
        ms_treatment: ms_t,
        ms_subjects: ms_s,
        ms_error: ms_e,
        f,
        p,
        f_subjects: f_s,
        p_subjects: p_upper_f(f_s, df_s, df_e),
        partial_eta_squared: ss_treat / (ss_treat + ss_err),
        gg_epsilon: gg,
        gg_epsilon_box_formula: box_eps,
        hf_epsilon_uncapped: hf_un,
        hf_epsilon: hf,
        lower_bound_epsilon: lb,
        df1_gg: d1g,
        df2_gg: d2g,
        p_gg: pgg,
        df1_hf: d1h,
        df2_hf: d2h,
        p_hf: phf,
        p_lower_bound: plb,
        mauchly_w: w,
        mauchly_chi2: chi2,
        mauchly_df: mdf,
        mauchly_p: mp,
        condition_means: col_means.iter().map(|m| m + c).collect(),
    })
}

// =======================================================================================
// Kruskal–Wallis and Friedman
// =======================================================================================

/// Kruskal–Wallis test result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KruskalWallis {
    /// H corrected for ties.
    pub h: f64,
    /// H without the tie correction.
    pub h_uncorrected: f64,
    /// Tie correction factor 1 - sum(t^3 - t)/(N^3 - N).
    pub tie_correction: f64,
    /// k - 1.
    pub df: f64,
    /// Chi-square approximation P(chi^2_{k-1} >= H).
    pub p: f64,
    /// Group sizes.
    pub n: Vec<usize>,
    /// Rank sums (midranks over the pooled sample).
    pub rank_sums: Vec<f64>,
    /// Mean ranks.
    pub mean_ranks: Vec<f64>,
    /// Epsilon^2 effect size H / ((N^2 - 1)/(N + 1)) = H/(N - 1) (Tomczak & Tomczak 2014).
    pub epsilon_squared: f64,
}

/// Kruskal–Wallis rank test (Kruskal & Wallis 1952, JASA 47:583,
/// doi:10.1080/01621459.1952.10483441): H = 12/(N(N+1)) sum R_i^2/n_i - 3(N+1), divided by
/// 1 - sum(t^3-t)/(N^3-N); chi-square p on k - 1 df. SciPy `kruskal`.
pub fn kruskal_wallis(groups: &[&[f64]]) -> Result<KruskalWallis> {
    let g = clean_groups(groups, "Kruskal-Wallis")?;
    let k = g.len();
    let all: Vec<f64> = g.iter().flatten().copied().collect();
    let nt = all.len() as f64;
    let (ranks, ties) = rank_with_ties(&all);
    let mut rank_sums = Vec::with_capacity(k);
    let mut pos = 0;
    for v in &g {
        rank_sums.push(compensated_sum(ranks[pos..pos + v.len()].iter().copied()));
        pos += v.len();
    }
    let h0 = 12.0 / (nt * (nt + 1.0))
        * compensated_sum((0..k).map(|i| rank_sums[i] * rank_sums[i] / g[i].len() as f64))
        - 3.0 * (nt + 1.0);
    let corr = 1.0 - tie_term(&ties) / (nt * nt * nt - nt);
    if !(corr > 0.0) {
        return Err(invalid("Kruskal-Wallis: all values are identical"));
    }
    let h = h0 / corr;
    let df = (k - 1) as f64;
    Ok(KruskalWallis {
        h,
        h_uncorrected: h0,
        tie_correction: corr,
        df,
        p: p_upper_chi2(h, df),
        n: g.iter().map(|v| v.len()).collect(),
        mean_ranks: (0..k).map(|i| rank_sums[i] / g[i].len() as f64).collect(),
        rank_sums,
        epsilon_squared: h / (nt - 1.0),
    })
}

/// Friedman test result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Friedman {
    /// Chi-square statistic corrected for ties within subjects.
    pub chi2: f64,
    /// Statistic without the tie correction.
    pub chi2_uncorrected: f64,
    /// Tie correction factor 1 - sum(t^3 - t)/(n(k^3 - k)).
    pub tie_correction: f64,
    /// k - 1.
    pub df: f64,
    /// Chi-square approximation p.
    pub p: f64,
    /// Kendall's coefficient of concordance W = chi2/(n(k-1)).
    pub kendall_w: f64,
    /// Rank sums of the conditions (ranks within each subject).
    pub rank_sums: Vec<f64>,
    /// Mean ranks.
    pub mean_ranks: Vec<f64>,
    /// Subjects used.
    pub n_subjects: usize,
    /// Subjects dropped for missing values.
    pub n_excluded: usize,
    /// Number of conditions.
    pub k: usize,
}

/// Friedman's rank test for a subjects x conditions matrix (Friedman 1937, JASA 32:675,
/// doi:10.1080/01621459.1937.10503522): ranks within each subject (midranks),
/// chi2 = [12/(n k (k+1)) sum R_j^2 - 3n(k+1)] / [1 - sum(t^3-t)/(n(k^3-k))], k - 1 df.
/// SciPy `friedmanchisquare`. Kendall's W (Kendall & Babington Smith 1939) as effect size.
pub fn friedman(m: &[Vec<f64>]) -> Result<Friedman> {
    let (rows, n_excluded) = complete_rows(m, "Friedman")?;
    let n = rows.len();
    let k = rows.first().map_or(0, |r| r.len());
    if k < 2 || n < 2 {
        return Err(not_enough(
            "Friedman needs at least 2 conditions and 2 complete subjects",
        ));
    }
    let mut rank_sums = vec![0.0; k];
    let mut ties_total = 0.0;
    for r in &rows {
        let (rk, ties) = rank_with_ties(r);
        for (s, x) in rank_sums.iter_mut().zip(rk) {
            *s += x;
        }
        ties_total += tie_term(&ties);
    }
    let (nf, kf) = (n as f64, k as f64);
    let q0 = 12.0 / (nf * kf * (kf + 1.0)) * compensated_sum(rank_sums.iter().map(|r| r * r))
        - 3.0 * nf * (kf + 1.0);
    let corr = 1.0 - ties_total / (nf * (kf * kf * kf - kf));
    if !(corr > 0.0) {
        return Err(invalid("Friedman: every subject has all values tied"));
    }
    let q = q0 / corr;
    let df = kf - 1.0;
    Ok(Friedman {
        chi2: q,
        chi2_uncorrected: q0,
        tie_correction: corr,
        df,
        p: p_upper_chi2(q, df),
        kendall_w: q / (nf * (kf - 1.0)),
        mean_ranks: rank_sums.iter().map(|r| r / nf).collect(),
        rank_sums,
        n_subjects: n,
        n_excluded,
        k,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_way_shift_invariance() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 7.0];
        let r = one_way_anova(&[&a, &b]).unwrap();
        assert!((r.f - 10.0).abs() < 1e-13);
        let a2: Vec<f64> = a.iter().map(|x| x + 1e12).collect();
        let b2: Vec<f64> = b.iter().map(|x| x + 1e12).collect();
        let r2 = one_way_anova(&[&a2, &b2]).unwrap();
        assert!((r2.f - 10.0).abs() < 1e-12, "{}", r2.f);
    }

    #[test]
    fn errors_not_panics() {
        assert!(one_way_anova(&[&[1.0]]).is_err());
        assert!(one_way_anova(&[&[1.0], &[2.0]]).is_err());
        assert!(one_way_anova(&[&[1.0, f64::INFINITY], &[2.0, 3.0]]).is_err());
        assert!(rm_anova(&[vec![1.0, 2.0]]).is_err());
        assert!(kruskal_wallis(&[&[1.0, 1.0], &[1.0]]).is_err());
        assert!(two_way_anova(&[vec![vec![1.0]]], TwoWayOptions::default()).is_err());
    }

    #[test]
    fn rm_listwise_deletion() {
        let m = vec![
            vec![45.0, 50.0, 55.0],
            vec![42.0, 42.0, 45.0],
            vec![36.0, f64::NAN, 43.0],
            vec![39.0, 35.0, 40.0],
            vec![51.0, 55.0, 59.0],
        ];
        let r = rm_anova(&m).unwrap();
        assert_eq!(r.n_subjects, 4);
        assert_eq!(r.n_excluded, 1);
    }

    #[test]
    fn two_way_without_replication() {
        // Additive table: residual = interaction, no interaction term.
        let cells = vec![
            vec![vec![1.0], vec![3.0], vec![5.5]],
            vec![vec![2.0], vec![4.5], vec![6.0]],
        ];
        let r = two_way_anova(&cells, TwoWayOptions::default()).unwrap();
        assert!(!r.interaction);
        assert_eq!(r.terms.len(), 2);
        assert_eq!(r.residual.df, 2.0);
    }
}
