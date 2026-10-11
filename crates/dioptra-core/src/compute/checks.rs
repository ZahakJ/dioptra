//! Assumption checks shared by the analyses (DESIGN §2.1: state the assumptions, check
//! what can be checked, show the check, offer the alternative).
//!
//! Policy (α = 0.05 for every check):
//! - Normality: Shapiro–Wilk for 3 <= n <= 5000, D'Agostino–Pearson as a second opinion
//!   for n >= 8. Pass when every p >= .05; Caution when the smallest p is in [.01, .05)
//!   or only D'Agostino–Pearson objects; Fail below .01. Not checked below n = 3.
//! - Equal variances: Brown–Forsythe (Levene on medians) decides; Bartlett is shown.
//! - Outliers: Grubbs at α = 0.05, as a note only (Caution, never Fail).
//! - Sample size: Fail below 2 values, Caution below the analysis's minimum.

use dioptra_stats::anova::{bartlett, levene, LeveneCenter};
use dioptra_stats::normality::{dagostino_pearson, shapiro_wilk};
use dioptra_stats::outliers::grubbs;

use super::{df, g, int, num, pv, section, st, txt};
use crate::results::{AssumptionCheck, CheckStatus, ResultCell, Section};
use crate::util::format_sig;

/// Significance level of every assumption check.
pub const ALPHA: f64 = 0.05;

fn p3(p: f64) -> String {
    crate::results::p_phrase(p, crate::api::PValueStyle::Apa)
}

/// Normality of each set. `subject` completes "Gaussian distribution of …" ("each group",
/// "the residuals", "the paired differences"). Returns the check and a "Normality"
/// section.
pub fn normality(
    sets: &[(String, Vec<f64>)],
    subject: &str,
    alternative: &str,
) -> (AssumptionCheck, Section) {
    let mut rows = Vec::new();
    let mut tested = 0;
    let mut min_p = f64::INFINITY;
    let mut objections: Vec<String> = Vec::new();
    let mut dp_only: Vec<String> = Vec::new();
    let mut small = false;
    for (name, v) in sets {
        let n = v.len();
        let sw = if (3..=5000).contains(&n) {
            shapiro_wilk(v).ok()
        } else {
            None
        };
        let dp = if n >= 8 {
            dagostino_pearson(v).ok()
        } else {
            None
        };
        if n < 8 {
            small = true;
        }
        let primary = sw.as_ref().map(|r| r.p).or(dp.as_ref().map(|r| r.p));
        if let Some(p) = primary {
            tested += 1;
            min_p = min_p.min(p);
            if p < ALPHA {
                objections.push(match &sw {
                    Some(r) => format!("{name} (W = {}, {})", format_sig(r.w, Some(3), 3), p3(r.p)),
                    None => format!("{name} ({})", p3(p)),
                });
            } else if let Some(d) = &dp {
                if d.p < ALPHA {
                    dp_only.push(format!("{name} (K² = {}, {})", st(d.k2), p3(d.p)));
                }
            }
        }
        let passed = match primary {
            Some(p) => txt(if p >= ALPHA { "Yes" } else { "No" }),
            None => txt(if n < 3 { "n too small" } else { "not tested" }),
        };
        rows.push(vec![
            txt(name.clone()),
            int(n),
            sw.as_ref().map_or(ResultCell::Empty, |r| num(r.w)),
            sw.as_ref().map_or(ResultCell::Empty, |r| pv(r.p)),
            dp.as_ref().map_or(ResultCell::Empty, |r| num(r.k2)),
            dp.as_ref().map_or(ResultCell::Empty, |r| pv(r.p)),
            passed,
        ]);
    }
    let sec = section(
        "Normality",
        &[
            "Data set",
            "n",
            "Shapiro–Wilk W",
            "P (Shapiro–Wilk)",
            "D'Agostino–Pearson K²",
            "P (D'Agostino–Pearson)",
            "Passed (α = .05)?",
        ],
        rows,
    );
    let name = format!("Gaussian distribution of {subject}");
    if tested == 0 {
        return (
            AssumptionCheck::new(
                name,
                CheckStatus::NotChecked,
                "Too few values to test normality (Shapiro–Wilk needs at least 3). The result rests on the assumption without any check.",
            )
            .or_else(alternative),
            sec,
        );
    }
    let power = if small {
        " With fewer than 8 values a normality test has little power, so passing it is weak reassurance."
    } else {
        ""
    };
    let (status, detail) = if objections.is_empty() && dp_only.is_empty() {
        (
            CheckStatus::Pass,
            format!(
                "Shapiro–Wilk finds no evidence against a Gaussian distribution of {subject} (every p ≥ .05).{power}"
            ),
        )
    } else if objections.is_empty() {
        (
            CheckStatus::Caution,
            format!(
                "Shapiro–Wilk passes, but D'Agostino–Pearson (skewness and kurtosis) objects for {}. The p value and CI may be somewhat inaccurate.",
                dp_only.join(", ")
            ),
        )
    } else {
        (
            if min_p < 0.01 {
                CheckStatus::Fail
            } else {
                CheckStatus::Caution
            },
            format!(
                "Shapiro–Wilk rejects a Gaussian distribution for {}. The p value and CI rest on this assumption and may be inaccurate, more so with small or unequal samples.",
                objections.join(", ")
            ),
        )
    };
    let mut c = AssumptionCheck::new(name, status, detail);
    if status != CheckStatus::Pass {
        c = c.or_else(alternative);
    }
    (c, sec)
}

/// Equal variances across groups: Brown–Forsythe decides, Bartlett is shown. With
/// `informational`, the test in use does not assume equal variances and the check only
/// reports (Pass).
pub fn equal_variances(
    groups: &[(String, Vec<f64>)],
    informational: bool,
    alternative: &str,
) -> Option<(AssumptionCheck, Section)> {
    if groups.len() < 2 || groups.iter().any(|(_, v)| v.len() < 2) {
        return Some((
            AssumptionCheck::new(
                "Equal variances",
                CheckStatus::NotChecked,
                "Every group needs at least two values to compare variances.",
            ),
            section(
                "Equal variances",
                &["Test", "Statistic", "DFn", "DFd", "P"],
                vec![],
            ),
        ));
    }
    let refs: Vec<&[f64]> = groups.iter().map(|(_, v)| v.as_slice()).collect();
    let bf = levene(&refs, LeveneCenter::Median).ok();
    let ba = bartlett(&refs).ok();
    let mut rows = Vec::new();
    if let Some(t) = &bf {
        rows.push(vec![
            txt("Brown–Forsythe (Levene, median)"),
            num(t.statistic),
            num(t.df1),
            num(t.df2),
            pv(t.p),
        ]);
    }
    if let Some(t) = &ba {
        rows.push(vec![
            txt("Bartlett"),
            num(t.statistic),
            num(t.df1),
            ResultCell::Empty,
            pv(t.p),
        ]);
    }
    let sds: Vec<f64> = groups
        .iter()
        .map(|(_, v)| {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() as f64 - 1.0)).sqrt()
        })
        .collect();
    let (lo, hi) = sds
        .iter()
        .fold((f64::INFINITY, 0.0f64), |a, s| (a.0.min(*s), a.1.max(*s)));
    let ratio = if lo > 0.0 { hi / lo } else { f64::INFINITY };
    let sec = section(
        "Equal variances",
        &["Test", "Statistic", "DFn", "DFd", "P"],
        rows,
    );
    let Some(t) = bf else {
        return Some((
            AssumptionCheck::new(
                "Equal variances",
                CheckStatus::NotChecked,
                "The variances could not be compared (a group has no spread).",
            ),
            sec,
        ));
    };
    let base = format!(
        "Brown–Forsythe F({}, {}) = {}, {}; the largest SD is {} times the smallest.",
        df(t.df1),
        df(t.df2),
        st(t.statistic),
        p3(t.p),
        g(ratio)
    );
    if informational {
        return Some((
            AssumptionCheck::new(
                "Equal variances",
                CheckStatus::Pass,
                format!("{base} The test used does not assume equal variances, so this is shown for information."),
            ),
            sec,
        ));
    }
    let status = if t.p < 0.01 {
        CheckStatus::Fail
    } else if t.p < ALPHA {
        CheckStatus::Caution
    } else {
        CheckStatus::Pass
    };
    let meaning = if status == CheckStatus::Pass {
        " No evidence that the variances differ."
    } else {
        " The variances appear to differ; a test that pools them can give a misleading p value, especially with unequal group sizes."
    };
    let mut c = AssumptionCheck::new("Equal variances", status, format!("{base}{meaning}"));
    if status != CheckStatus::Pass {
        c = c.or_else(alternative);
    }
    Some((c, sec))
}

/// Sample-size check: Fail below 2 values in a group, Caution below `min`.
pub fn sample_size(groups: &[(String, usize)], min: usize) -> AssumptionCheck {
    let tiny: Vec<String> = groups
        .iter()
        .filter(|(_, n)| *n < 2)
        .map(|(g, n)| format!("{g} (n = {n})"))
        .collect();
    if !tiny.is_empty() {
        return AssumptionCheck::new(
            "Sample size",
            CheckStatus::Fail,
            format!(
                "{} {} fewer than two values; nothing about its spread can be estimated.",
                tiny.join(", "),
                if tiny.len() == 1 { "has" } else { "have" }
            ),
        )
        .or_else("Collect more data; no test can rescue a group of one.");
    }
    let small: Vec<String> = groups
        .iter()
        .filter(|(_, n)| *n < min)
        .map(|(g, n)| format!("{g} (n = {n})"))
        .collect();
    if small.is_empty() {
        AssumptionCheck::new(
            "Sample size",
            CheckStatus::Pass,
            format!("Every group has at least {min} values."),
        )
    } else {
        AssumptionCheck::new(
            "Sample size",
            CheckStatus::Caution,
            format!(
                "{} {} fewer than {min} values: the assumption checks have almost no power, so the result rests on assumptions the data cannot confirm.",
                small.join(", "),
                if small.len() == 1 { "has" } else { "have" }
            ),
        )
    }
}

/// Grubbs' test at α = 0.05 on each set, as a hint only.
pub fn outlier_hint(sets: &[(String, Vec<f64>)]) -> AssumptionCheck {
    let mut flagged = Vec::new();
    let mut tested = 0;
    for (name, v) in sets {
        if v.len() < 3 {
            continue;
        }
        if let Ok(r) = grubbs(v, ALPHA) {
            tested += 1;
            for &i in &r.outlier_indices {
                flagged.push(format!("{} in {name}", g(v[i])));
            }
        }
    }
    if tested == 0 {
        return AssumptionCheck::new(
            "Outliers",
            CheckStatus::NotChecked,
            "Too few values to look for outliers (Grubbs' test needs at least 3).",
        );
    }
    if flagged.is_empty() {
        AssumptionCheck::new(
            "Outliers",
            CheckStatus::Pass,
            "Grubbs' test (α = 0.05) flags no value as an outlier.",
        )
    } else {
        AssumptionCheck::new(
            "Outliers",
            CheckStatus::Caution,
            format!(
                "Grubbs' test (α = 0.05) flags {} as a possible outlier. Check for an error in recording; do not remove a value on this basis alone.",
                flagged.join(", ")
            ),
        )
        .or_else("Analyze → Identify outliers shows the details; a rank-based test is less sensitive to extreme values.")
    }
}

/// What was used and what was left out.
pub fn values_used(notes: &[String], caution: bool) -> AssumptionCheck {
    if notes.is_empty() {
        AssumptionCheck::new(
            "Values used",
            CheckStatus::Pass,
            "Every numeric value in the chosen data sets was used.",
        )
    } else {
        AssumptionCheck::new(
            "Values used",
            if caution {
                CheckStatus::Caution
            } else {
                CheckStatus::Pass
            },
            notes.join(" "),
        )
    }
}
