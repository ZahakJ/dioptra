// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Automated operational qualification (OQ): the comparisons of the dioptra-stats test suite,
//! run inside the installed build on fixtures embedded in it.
//!
//! * NIST StRD: all 58 data sets (univariate, linear regression, one-way ANOVA, nonlinear
//!   regression from both starting points), scored by the log relative error (LRE) against
//!   the certified values and held to the minimum LREs of `crates/dioptra-stats/tests/nist_*.rs`.
//! * Oracle fixtures: every comparison of the 28 families in `docs/CORRECTNESS.md` (7,841)
//!   against SciPy,
//!   statsmodels, lifelines, scikit-learn and 32-digit mpmath, each within the tolerance
//!   stored with its case.
//!
//! [`run`] is the whole OQ; the app runs it a step at a time ([`steps`], [`run_step`]) to
//! show progress. [`csv`] and [`pdf`] render a report.

#[path = "oq/families.rs"]
mod families;
#[path = "oq/fixtures.rs"]
mod fixtures;
#[path = "oq/harness.rs"]
mod harness;

use std::time::Instant;

use dioptra_core::api::Session;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::LabResponse;

use harness::Checker;

/// Comparisons with the oracle fixtures: the sum of the family table of `docs/CORRECTNESS.md`.
pub const ORACLE_COMPARISONS: usize = 7841;
/// NIST StRD data sets.
pub const NIST_SETS: usize = 58;
/// Checks on the NIST sets: 54 univariate, 25 linear, 22 ANOVA, 270 nonlinear.
pub const NIST_CHECKS: usize = 371;

/// One comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OqCheck {
    /// Stable ID: section code and number (`ORC-TTEST-0012`) or NIST set and quantity
    /// (`NIST-LIN-Filip-CERT`).
    pub id: String,
    /// Section code.
    pub section: String,
    /// Case and context the value comes from.
    pub reference: String,
    /// The quantity compared.
    pub quantity: String,
    /// Reference value (NIST checks: the minimum LRE required).
    #[serde(with = "number")]
    pub expected: f64,
    /// Value computed by this build (NIST checks: the LRE achieved).
    #[serde(with = "number")]
    pub got: f64,
    /// What counts as agreement.
    pub tolerance: String,
    /// Whether it agrees.
    pub pass: bool,
    /// Why it failed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A section of the OQ: one fixture family or one class of NIST sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OqSection {
    /// Code, the prefix of its check IDs.
    pub code: String,
    /// What it covers.
    pub title: String,
    /// Where the reference values come from.
    pub source: String,
    /// Comparisons.
    pub checks: usize,
    /// Comparisons that failed.
    pub failed: usize,
    /// Run time in seconds.
    pub seconds: f64,
}

/// Counts over the whole run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OqTotals {
    /// Every comparison.
    pub checks: usize,
    /// Comparisons that agree.
    pub passed: usize,
    /// Comparisons that do not.
    pub failed: usize,
    /// NIST StRD data sets run.
    pub nist_sets: usize,
    /// Checks on the NIST sets.
    pub nist_checks: usize,
    /// Comparisons with the oracle fixtures.
    pub oracle_checks: usize,
}

/// The execution signature on a report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    /// User ID.
    pub user: String,
    /// Full name.
    pub name: String,
    /// When the signer confirmed the password (UTC).
    pub at: String,
    /// Always `executed`.
    pub meaning: String,
}

/// The result of an OQ run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OqReport {
    /// `Dioptra`.
    pub product: String,
    /// Build version.
    pub version: String,
    /// Target triple of the build.
    pub target: String,
    /// Operating system and architecture it ran on.
    pub os: String,
    /// When it ran (UTC).
    pub date: String,
    /// SHA-256 of the embedded fixture set (see [`fixture_set_sha256`]).
    pub fixtures_sha256: String,
    /// Embedded fixture files.
    pub fixture_files: usize,
    /// Whether every check passed.
    pub pass: bool,
    /// Counts.
    pub totals: OqTotals,
    /// Run time of the comparisons in seconds.
    pub seconds: f64,
    /// Sections in run order.
    pub sections: Vec<OqSection>,
    /// Every comparison.
    pub checks: Vec<OqCheck>,
    /// SHA-256 of this report's content (everything above), tying a PDF to its CSV.
    pub report_sha256: String,
}

/// Non-finite numbers as the fixtures write them: "Infinity", "-Infinity", "NaN".
mod number {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f64(*v)
        } else if v.is_nan() {
            s.serialize_str("NaN")
        } else if *v > 0.0 {
            s.serialize_str("Infinity")
        } else {
            s.serialize_str("-Infinity")
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(super::harness::num(&v).unwrap_or(f64::NAN))
    }
}

// ---- steps -------------------------------------------------------------------------------

/// One step of the run.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    /// Section code.
    pub code: &'static str,
    /// What it covers.
    pub title: &'static str,
    run: Runner,
}

#[derive(Debug, Clone, Copy)]
enum Runner {
    Oracle(fn() -> Checker, &'static str),
    NistUnivariate,
    NistLinear,
    NistAnova,
    NistNonlinear(&'static [&'static str]),
}

const NL_LOWER: &[&str] = &[
    "Misra1a", "Chwirut2", "Chwirut1", "Lanczos3", "Gauss1", "Gauss2", "DanWood", "Misra1b",
];
const NL_AVERAGE: &[&str] = &[
    "Kirby2", "Hahn1", "Nelson", "MGH17", "Lanczos1", "Lanczos2", "Gauss3", "Misra1c", "Misra1d",
    "Roszman1", "ENSO",
];
const NL_HIGHER: &[&str] = &[
    "MGH09", "Thurber", "BoxBOD", "Rat42", "MGH10", "Eckerle4", "Rat43", "Bennett5",
];

const STEPS: &[Step] = {
    use families as f;
    use Runner::*;
    &[
        Step {
            code: "NIST-UNI",
            title: "NIST StRD univariate (9 sets)",
            run: NistUnivariate,
        },
        Step {
            code: "NIST-LIN",
            title: "NIST StRD linear regression (11 sets)",
            run: NistLinear,
        },
        Step {
            code: "NIST-ANOVA",
            title: "NIST StRD one-way ANOVA (11 sets)",
            run: NistAnova,
        },
        Step {
            code: "NIST-NL",
            title: "NIST StRD nonlinear regression, lower difficulty (8 sets)",
            run: NistNonlinear(NL_LOWER),
        },
        Step {
            code: "NIST-NL",
            title: "NIST StRD nonlinear regression, average difficulty (11 sets)",
            run: NistNonlinear(NL_AVERAGE),
        },
        Step {
            code: "NIST-NL",
            title: "NIST StRD nonlinear regression, higher difficulty (8 sets)",
            run: NistNonlinear(NL_HIGHER),
        },
        Step {
            code: "ORC-BETA",
            title: "Incomplete beta and inverse",
            run: Oracle(f::incomplete_beta, "distributions"),
        },
        Step {
            code: "ORC-GAMMA",
            title: "Incomplete gamma and inverse, lgamma, digamma",
            run: Oracle(f::incomplete_gamma, "distributions"),
        },
        Step {
            code: "ORC-NORM",
            title: "Normal distribution",
            run: Oracle(f::normal, "distributions"),
        },
        Step {
            code: "ORC-TFCHI",
            title: "t, F and chi-square distributions",
            run: Oracle(f::t_f_chi2, "distributions"),
        },
        Step {
            code: "ORC-BINHYP",
            title: "Binomial and hypergeometric distributions",
            run: Oracle(f::binomial_hypergeometric, "distributions"),
        },
        Step {
            code: "ORC-NCT",
            title: "Noncentral t distribution",
            run: Oracle(f::noncentral_t, "distributions"),
        },
        Step {
            code: "ORC-QRANGE",
            title: "Studentized range distribution",
            run: Oracle(f::studentized_range, "studentized_range"),
        },
        Step {
            code: "ORC-DUNNETT",
            title: "Dunnett distribution",
            run: Oracle(f::dunnett, "multiple_comparisons"),
        },
        Step {
            code: "ORC-DESC",
            title: "Descriptive statistics",
            run: Oracle(f::descriptive, "descriptive"),
        },
        Step {
            code: "ORC-TTEST",
            title: "t tests",
            run: Oracle(f::t_tests, "t_tests"),
        },
        Step {
            code: "ORC-MW",
            title: "Mann-Whitney test",
            run: Oracle(f::mann_whitney_family, "mann_whitney"),
        },
        Step {
            code: "ORC-WSR",
            title: "Wilcoxon signed-rank test",
            run: Oracle(f::wilcoxon_family, "wilcoxon_signed_rank"),
        },
        Step {
            code: "ORC-NORMTEST",
            title: "Normality tests",
            run: Oracle(f::normality_family, "normality"),
        },
        Step {
            code: "ORC-OUTLIER",
            title: "Outliers (Grubbs, ROUT)",
            run: Oracle(f::outliers_family, "outliers"),
        },
        Step {
            code: "ORC-ANOVA1",
            title: "One-way ANOVA, Welch, Brown-Forsythe, Bartlett, Levene",
            run: Oracle(f::anova_oneway_family, "anova_oneway"),
        },
        Step {
            code: "ORC-ANOVA2",
            title: "Two-way ANOVA (types I, II, III)",
            run: Oracle(f::anova_twoway_family, "anova_twoway"),
        },
        Step {
            code: "ORC-ANOVARM",
            title: "Repeated-measures ANOVA, sphericity, Friedman",
            run: Oracle(f::anova_rm_family, "anova_rm"),
        },
        Step {
            code: "ORC-KWF",
            title: "Kruskal-Wallis, Friedman, Dunn",
            run: Oracle(f::kruskal_friedman_dunn_family, "kruskal_friedman_dunn"),
        },
        Step {
            code: "ORC-POSTHOC",
            title: "Tukey-Kramer, Dunnett, p-value adjustment",
            run: Oracle(f::multiple_comparisons_family, "multiple_comparisons"),
        },
        Step {
            code: "ORC-SANITY",
            title: "Cross-checks: statsmodels qsturng, SciPy QMC Dunnett",
            run: Oracle(f::multiple_comparisons_sanity, "multiple_comparisons"),
        },
        Step {
            code: "ORC-CONT",
            title: "Contingency tables (chi-square, Fisher, McNemar, trend)",
            run: Oracle(f::contingency_family, "contingency"),
        },
        Step {
            code: "ORC-CORR",
            title: "Correlation (Pearson, Spearman, Kendall)",
            run: Oracle(f::correlation_family, "correlation"),
        },
        Step {
            code: "ORC-LINREG",
            title: "Linear regression (simple, bands, runs, Deming, multiple)",
            run: Oracle(f::linear_regression_family, "linear_regression"),
        },
        Step {
            code: "ORC-NLREG",
            title: "Nonlinear regression (fits, comparison, weighting, robust)",
            run: Oracle(f::nonlinear_family, "nonlinear_regression"),
        },
        Step {
            code: "ORC-SURV",
            title: "Survival (Kaplan-Meier, log-rank family, hazard ratios, Cox)",
            run: Oracle(f::survival_family, "survival"),
        },
        Step {
            code: "ORC-ROC",
            title: "ROC curves (AUC, DeLong, Hanley-McNeil, Youden)",
            run: Oracle(f::roc_family, "roc"),
        },
        Step {
            code: "ORC-BA",
            title: "Bland-Altman",
            run: Oracle(f::bland_altman_family, "bland_altman"),
        },
        Step {
            code: "ORC-PCA",
            title: "Principal component analysis",
            run: Oracle(f::pca_family, "pca"),
        },
    ]
};

/// The steps of a run, in order.
pub fn steps() -> &'static [Step] {
    STEPS
}

/// The result of one step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// The section.
    pub section: OqSection,
    /// Its comparisons.
    pub checks: Vec<OqCheck>,
}

const NIST_SOURCE: &str =
    "NIST Statistical Reference Datasets, certified values (www.itl.nist.gov/div898/strd); LRE as in McCullough (1998)";

fn oracle_source(file: &str) -> String {
    let refs = harness::load(&format!("oracle/{file}.json"));
    match refs["reference"].as_str() {
        Some(r) => format!("fixtures/oracle/{file}.json: {r}"),
        None => format!("fixtures/oracle/{file}.json"),
    }
}

/// Run step `i` of [`steps`]. A step that panics is recorded as one failed check.
pub fn run_step(i: usize) -> StepResult {
    let step = STEPS[i];
    let t0 = Instant::now();
    let (source, checks) = match std::panic::catch_unwind(|| run_inner(step)) {
        Ok(r) => r,
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown error".into());
            let check = OqCheck {
                id: format!("{}-STEP{}", step.code, i + 1),
                section: step.code.into(),
                reference: step.title.into(),
                quantity: "step completed".into(),
                expected: 1.0,
                got: 0.0,
                tolerance: "completes without error".into(),
                pass: false,
                note: format!("The step stopped: {msg}"),
            };
            (String::new(), vec![check])
        }
    };
    let failed = checks.iter().filter(|c| !c.pass).count();
    StepResult {
        section: OqSection {
            code: step.code.into(),
            title: step.title.into(),
            source,
            checks: checks.len(),
            failed,
            seconds: t0.elapsed().as_secs_f64(),
        },
        checks,
    }
}

fn run_inner(step: Step) -> (String, Vec<OqCheck>) {
    match step.run {
        Runner::Oracle(f, file) => (oracle_source(file), oracle_checks(step.code, f())),
        Runner::NistUnivariate => (NIST_SOURCE.into(), nist_univariate()),
        Runner::NistLinear => (NIST_SOURCE.into(), nist_linear()),
        Runner::NistAnova => (NIST_SOURCE.into(), nist_anova()),
        Runner::NistNonlinear(sets) => (NIST_SOURCE.into(), nist_nonlinear(sets)),
    }
}

fn tol_text(t: &Option<harness::Tol>) -> String {
    match t {
        Some(t) => format!("rel {:e} or abs {:e}", t.rel, t.abs),
        None => "condition holds".into(),
    }
}

fn oracle_checks(code: &str, ck: Checker) -> Vec<OqCheck> {
    ck.compared
        .into_iter()
        .enumerate()
        .map(|(i, c)| OqCheck {
            id: format!("{code}-{:04}", i + 1),
            section: code.into(),
            reference: c.ctx,
            quantity: c.field,
            expected: c.expected,
            got: c.got,
            tolerance: tol_text(&c.tol),
            pass: c.pass,
            note: c.note,
        })
        .collect()
}

/// A NIST check: the LRE achieved against the minimum required.
fn lre_check(id: String, reference: String, quantity: &str, need: f64, got: f64) -> OqCheck {
    let pass = got >= need;
    OqCheck {
        section: id.split('-').take(2).collect::<Vec<_>>().join("-"),
        id,
        reference,
        quantity: quantity.into(),
        expected: need,
        got,
        tolerance: format!("LRE >= {need}"),
        pass,
        note: if pass {
            String::new()
        } else {
            format!("LRE {got:.2} < {need}")
        },
    }
}

// The minimum LREs below are those of crates/dioptra-stats/tests/nist_*.rs.

fn nist_univariate() -> Vec<OqCheck> {
    let mut out = Vec::new();
    for row in families::nist_univariate() {
        let [lm, ls, lr] = row.certified;
        let [xm, xs, xr] = row.exact_on_doubles;
        let (sd_min, r1_min) = match row.difficulty.as_str() {
            "Lower" => (13.0, 12.0),
            "Average" => (10.0, 9.0),
            _ => (8.0, 7.0),
        };
        let sd_min = if row.name == "NumAcc3" { 9.0 } else { sd_min };
        let cert = format!(
            "NIST StRD {} ({}), certified values",
            row.name, row.difficulty
        );
        let exact = format!(
            "NIST StRD {} ({}), exact statistics of the inputs as parsed into doubles",
            row.name, row.difficulty
        );
        for (key, what, reference, l, need) in [
            ("MEAN", "mean", &cert, lm, 14.0),
            ("SD", "SD (n-1)", &cert, ls, sd_min),
            ("R1", "lag-1 autocorrelation", &cert, lr, r1_min),
            ("MEAN-X", "mean", &exact, xm, 13.0),
            ("SD-X", "SD (n-1)", &exact, xs, 13.0),
            ("R1-X", "lag-1 autocorrelation", &exact, xr, 13.0),
        ] {
            out.push(lre_check(
                format!("NIST-UNI-{}-{key}", row.name),
                reference.clone(),
                &format!("LRE of the {what}"),
                need,
                l,
            ));
        }
    }
    out
}

fn nist_linear() -> Vec<OqCheck> {
    let mut out = Vec::new();
    for r in families::nist_linear() {
        let need = match r.name.as_str() {
            "Filip" | "Longley" | "Wampler3" | "Wampler4" | "Wampler5" => 12.0,
            _ => 13.0,
        };
        let q = "minimum LRE over parameters, their SDs, R^2, residual SD and the ANOVA table";
        out.push(lre_check(
            format!("NIST-LIN-{}-CERT", r.name),
            format!("NIST StRD {} ({}), certified values", r.name, r.difficulty),
            q,
            need,
            r.min,
        ));
        out.push(lre_check(
            format!("NIST-LIN-{}-EXACT", r.name),
            format!(
                "NIST StRD {} ({}), exact least-squares solution for the inputs as parsed into doubles",
                r.name, r.difficulty
            ),
            q,
            12.0,
            nan_low(r.min_exact_on_doubles),
        ));
    }
    // The closed-form simple-regression path on the straight-line sets.
    use dioptra_stats::regression::{simple_linear_regression, SimpleRegressionOptions};
    use families::lre;
    for name in ["Norris", "NoInt1", "NoInt2"] {
        let j = harness::load(&format!("nist/{name}.json"));
        let x = harness::nums(&j["data"]["x"]);
        let y = harness::nums(&j["data"]["y"]);
        let origin = !j["model"]["intercept"].as_bool().unwrap_or(true);
        let m = match simple_linear_regression(
            &x,
            &y,
            SimpleRegressionOptions {
                through_origin: origin,
                ..Default::default()
            },
        ) {
            Ok(r) => {
                let ps = j["parameters"].as_array().cloned().unwrap_or_default();
                let got = if origin {
                    vec![(r.slope, r.se_slope)]
                } else {
                    vec![(r.intercept, r.se_intercept), (r.slope, r.se_slope)]
                };
                let mut m = f64::INFINITY;
                for (p, (b, s)) in ps.iter().zip(got) {
                    let c = p["certified"].as_f64().unwrap_or(f64::NAN);
                    let sd = p["std_dev"].as_f64().unwrap_or(f64::NAN);
                    m = m.min(lre(b, c)).min(lre(s, sd));
                }
                m = m.min(lre(
                    r.sy_x,
                    j["residual_std_dev"]["value"].as_f64().unwrap_or(f64::NAN),
                ));
                m.min(lre(
                    r.r_squared,
                    j["r_squared"]["value"].as_f64().unwrap_or(f64::NAN),
                ))
            }
            Err(_) => f64::NAN,
        };
        out.push(lre_check(
            format!("NIST-LIN-{name}-SIMPLE"),
            format!("NIST StRD {name}, certified values, simple-regression routine"),
            "minimum LRE over parameters, their SDs, residual SD and R^2",
            13.0,
            nan_low(m),
        ));
    }
    out
}

/// NaN scores as 0 digits.
fn nan_low(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v
    }
}

fn nist_anova() -> Vec<OqCheck> {
    let mut out = Vec::new();
    for r in families::nist_anova() {
        let need = match r.name.as_str() {
            "SmLs05" | "SmLs06" => 9.9,
            "SmLs07" | "SmLs08" | "SmLs09" => 3.9,
            _ => match r.difficulty.as_str() {
                "Lower" => 13.0,
                _ => 10.0,
            },
        };
        let q = "minimum LRE over SS, MS, F, R^2 and residual SD";
        out.push(lre_check(
            format!("NIST-ANOVA-{}-CERT", r.name),
            format!("NIST StRD {} ({}), certified values", r.name, r.difficulty),
            q,
            need,
            r.min,
        ));
        out.push(lre_check(
            format!("NIST-ANOVA-{}-EXACT", r.name),
            format!(
                "NIST StRD {} ({}), exact statistics of the inputs as parsed into doubles",
                r.name, r.difficulty
            ),
            q,
            13.0,
            nan_low(r.min_exact_on_doubles),
        ));
    }
    out
}

fn nist_nonlinear(sets: &[&str]) -> Vec<OqCheck> {
    let mut out = Vec::new();
    for name in sets {
        let r = families::nist_nonlinear_set(name);
        for (k, s) in r.starts.iter().enumerate() {
            let start = k + 1;
            let reference = format!(
                "NIST StRD {} ({}), certified values, start {start}",
                r.name, r.difficulty
            );
            out.push(OqCheck {
                id: format!("NIST-NL-{}-S{start}-CONV", r.name),
                section: "NIST-NL".into(),
                reference: reference.clone(),
                quantity: "fit converges".into(),
                expected: 1.0,
                got: if s.converged { 1.0 } else { 0.0 },
                tolerance: "converged".into(),
                pass: s.converged,
                note: if s.converged {
                    String::new()
                } else {
                    format!("status {} after {} iterations", s.status, s.iterations)
                },
            });
            let need = if r.name == "Lanczos1" { 2.5 } else { 9.0 };
            for (key, what, l, need) in [
                ("PAR", "parameters", s.params, 9.0),
                ("SD", "parameter SDs", s.sds, need),
                ("SS", "residual SS", s.ss, need),
                ("RSD", "residual SD", s.residual_sd, need),
            ] {
                out.push(lre_check(
                    format!("NIST-NL-{}-S{start}-{key}", r.name),
                    reference.clone(),
                    &format!("minimum LRE of the {what}"),
                    need,
                    nan_low(l),
                ));
            }
        }
    }
    out
}

// ---- the report -----------------------------------------------------------------------------

/// SHA-256 of the embedded fixture set: the SHA-256 of the text made of one line
/// `<sha256 of file>  <path>\n` per file, sorted by path (byte order), with each file's
/// line endings normalised to LF. The same as
/// `cd fixtures && sha256sum nist/[!i]*.json oracle/*.json | LC_ALL=C sort -k2 | sha256sum`
/// on a checkout with LF line endings.
pub fn fixture_set_sha256() -> String {
    let mut files: Vec<&(&str, &str)> = fixtures::FILES.iter().collect();
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut manifest = String::new();
    for (path, text) in files {
        let lf = text.replace("\r\n", "\n");
        manifest.push_str(&crate::audit::sha256_hex(lf.as_bytes()));
        manifest.push_str("  ");
        manifest.push_str(path);
        manifest.push('\n');
    }
    crate::audit::sha256_hex(manifest.as_bytes())
}

fn os_text() -> String {
    let base = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    #[cfg(target_os = "linux")]
    if let Ok(t) = std::fs::read_to_string("/etc/os-release") {
        if let Some(name) = t
            .lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"').to_string())
        {
            return format!("{base} ({name})");
        }
    }
    base
}

/// Put step results together into a report.
pub fn assemble(results: Vec<StepResult>) -> OqReport {
    let mut sections = Vec::new();
    let mut checks = Vec::new();
    for r in results {
        sections.push(r.section);
        checks.extend(r.checks);
    }
    let failed = checks.iter().filter(|c| !c.pass).count();
    let nist_checks = checks.iter().filter(|c| c.id.starts_with("NIST-")).count();
    let totals = OqTotals {
        checks: checks.len(),
        passed: checks.len() - failed,
        failed,
        nist_sets: NIST_SETS,
        nist_checks,
        oracle_checks: checks.len() - nist_checks,
    };
    let seconds = sections.iter().map(|s| s.seconds).sum();
    let mut report = OqReport {
        product: "Dioptra".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        target: env!("DIOPTRA_TARGET").into(),
        os: os_text(),
        date: dioptra_core::util::now(),
        fixtures_sha256: fixture_set_sha256(),
        fixture_files: fixtures::FILES.len(),
        pass: failed == 0 && !checks.is_empty(),
        totals,
        seconds,
        sections,
        checks,
        report_sha256: String::new(),
    };
    report.report_sha256 = report_hash(&report);
    report
}

/// SHA-256 of the canonical JSON of the report without `report_sha256`.
pub fn report_hash(report: &OqReport) -> String {
    let mut v = serde_json::to_value(report).expect("report serialises");
    if let Some(o) = v.as_object_mut() {
        o.remove("report_sha256");
    }
    crate::audit::sha256_hex(crate::audit::canonical(&v).as_bytes())
}

/// Run the whole OQ.
pub fn run() -> OqReport {
    run_with(|_, _, _| {})
}

/// Run the whole OQ, calling `progress(step, steps, title)` before each step.
pub fn run_with(mut progress: impl FnMut(usize, usize, &str)) -> OqReport {
    let n = STEPS.len();
    let results = (0..n)
        .map(|i| {
            progress(i, n, STEPS[i].title);
            run_step(i)
        })
        .collect();
    assemble(results)
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn num_text(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        format!("{v:e}")
    }
}

/// The report as CSV: one row per check; the run's identification in `#` lines first.
pub fn csv(report: &OqReport) -> String {
    let mut out = String::new();
    for (k, v) in [
        ("product", report.product.clone()),
        ("version", report.version.clone()),
        ("target", report.target.clone()),
        ("os", report.os.clone()),
        ("date_utc", report.date.clone()),
        ("fixtures_sha256", report.fixtures_sha256.clone()),
        ("report_sha256", report.report_sha256.clone()),
        ("checks", report.totals.checks.to_string()),
        ("failed", report.totals.failed.to_string()),
        ("result", if report.pass { "PASS" } else { "FAIL" }.into()),
    ] {
        out.push_str(&format!("# {k},{}\n", csv_field(&v)));
    }
    out.push_str("id,section,reference,quantity,expected,got,tolerance,pass,note\n");
    for c in &report.checks {
        let row = [
            c.id.clone(),
            c.section.clone(),
            c.reference.clone(),
            c.quantity.clone(),
            num_text(c.expected),
            num_text(c.got),
            c.tolerance.clone(),
            if c.pass { "pass" } else { "FAIL" }.into(),
            c.note.clone(),
        ];
        out.push_str(
            &row.iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

// ---- PDF ------------------------------------------------------------------------------------

const PAGE_W: f64 = 595.0;
const PAGE_H: f64 = 842.0;
const MARGIN: f64 = 42.0;
const FONT: f64 = 7.5;
const LINE: f64 = 10.0;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

struct Pages {
    done: Vec<String>,
    body: String,
    y: f64,
}

impl Pages {
    fn new() -> Self {
        Pages {
            done: Vec::new(),
            body: String::new(),
            y: MARGIN + 12.0,
        }
    }

    fn text(&mut self, x: f64, size: f64, weight: u16, s: &str) {
        self.text_at(x, self.y, size, weight, "#1a1a1a", s);
    }

    fn text_at(&mut self, x: f64, y: f64, size: f64, weight: u16, fill: &str, s: &str) {
        self.body.push_str(&format!(
            "<text x=\"{x:.1}\" y=\"{y:.1}\" font-family=\"Inter\" font-size=\"{size}\" font-weight=\"{weight}\" fill=\"{fill}\">{}</text>",
            esc(s)
        ));
    }

    fn rule(&mut self, y: f64) {
        self.body.push_str(&format!(
            "<line x1=\"{MARGIN}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" stroke=\"#c8c8c8\" stroke-width=\"0.5\"/>",
            PAGE_W - MARGIN
        ));
    }

    /// Room for `h` more points, or a new page.
    fn need(&mut self, h: f64) -> bool {
        if self.y + h > PAGE_H - MARGIN - 14.0 {
            self.done.push(std::mem::take(&mut self.body));
            self.y = MARGIN + 12.0;
            true
        } else {
            false
        }
    }

    fn finish(mut self, footer: &str) -> Vec<String> {
        self.done.push(std::mem::take(&mut self.body));
        let n = self.done.len();
        self.done
            .iter()
            .enumerate()
            .map(|(i, body)| {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"210mm\" height=\"297mm\" viewBox=\"0 0 {PAGE_W} {PAGE_H}\"><rect width=\"{PAGE_W}\" height=\"{PAGE_H}\" fill=\"#ffffff\"/>{body}<text x=\"{MARGIN}\" y=\"{:.1}\" font-family=\"Inter\" font-size=\"7\" fill=\"#666666\">{}</text></svg>",
                    PAGE_H - 22.0,
                    esc(&format!("{footer}, page {} of {n}", i + 1))
                )
            })
            .collect()
    }
}

/// The report as PDF: identification, result, sections, the NIST checks, any failed check
/// and a signature block. Every check is in the CSV of the same report (same report SHA-256).
pub fn pdf(report: &OqReport, execution: Option<&Execution>) -> Result<Vec<u8>, String> {
    dioptra_plot::output::svgs_to_pdf(&pdf_pages(report, execution)).map_err(|e| e.to_string())
}

fn pdf_pages(r: &OqReport, execution: Option<&Execution>) -> Vec<String> {
    let mut p = Pages::new();
    p.text(MARGIN, 15.0, 600, "Operational qualification report");
    p.y += 16.0;
    p.text(
        MARGIN,
        9.0,
        400,
        &format!(
            "{} {}: automated OQ of the statistical engine",
            r.product, r.version
        ),
    );
    p.y += LINE * 2.0;
    let rows = [
        (
            "Product and version",
            format!("{} {}", r.product, r.version),
        ),
        ("Build target", r.target.clone()),
        ("Operating system", r.os.clone()),
        ("Executed (UTC)", r.date.replace('T', " ").replace('Z', "")),
        (
            "Reference data",
            format!("{} embedded fixture files", r.fixture_files),
        ),
        ("Fixture set SHA-256", r.fixtures_sha256.clone()),
        ("Report SHA-256", r.report_sha256.clone()),
        (
            "Checks",
            format!(
                "{} ({} on the {} NIST StRD data sets, {} against oracle fixtures)",
                r.totals.checks, r.totals.nist_checks, r.totals.nist_sets, r.totals.oracle_checks
            ),
        ),
        (
            "Passed / failed",
            format!("{} / {}", r.totals.passed, r.totals.failed),
        ),
        ("Run time", format!("{:.1} s", r.seconds)),
    ];
    for (k, v) in rows {
        p.text(MARGIN, FONT + 0.5, 600, k);
        p.text(MARGIN + 120.0, FONT + 0.5, 400, &v);
        p.y += LINE + 1.0;
    }
    p.y += 6.0;
    let (fill, verdict) = if r.pass {
        (
            "#1d6b3a",
            "RESULT: PASS. Every check agrees with its reference within its acceptance criterion.",
        )
    } else {
        (
            "#a32020",
            "RESULT: FAIL. One or more checks disagree with their reference; see Failed checks.",
        )
    };
    let y = p.y;
    p.text_at(MARGIN, y, 10.0, 700, fill, verdict);
    p.y += LINE * 2.0;

    // Sections.
    p.text(MARGIN, 10.0, 600, "Sections");
    p.y += LINE + 2.0;
    let cols = [
        MARGIN,
        MARGIN + 72.0,
        MARGIN + 380.0,
        MARGIN + 425.0,
        MARGIN + 470.0,
    ];
    let head = |p: &mut Pages| {
        for (t, x) in ["Code", "Covers", "Checks", "Failed", "Time"]
            .iter()
            .zip(cols)
        {
            p.text(x, FONT, 600, t);
        }
        p.y += 3.0;
        let y = p.y;
        p.rule(y);
        p.y += LINE;
    };
    head(&mut p);
    for s in &r.sections {
        if p.need(LINE) {
            head(&mut p);
        }
        let cells = [
            s.code.clone(),
            clip(&s.title, 66),
            s.checks.to_string(),
            s.failed.to_string(),
            format!("{:.2} s", s.seconds),
        ];
        for (c, x) in cells.iter().zip(cols) {
            p.text(x, FONT, 400, c);
        }
        p.y += LINE;
    }
    p.y += LINE;

    // Signature block.
    sign_block(&mut p, execution);

    // Failed checks.
    let failed: Vec<&OqCheck> = r.checks.iter().filter(|c| !c.pass).collect();
    if !failed.is_empty() {
        p.need(LINE * 4.0);
        p.text(
            MARGIN,
            10.0,
            600,
            &format!("Failed checks ({})", failed.len()),
        );
        p.y += LINE + 2.0;
        for c in failed.iter().take(500) {
            p.need(LINE * 2.0);
            p.text(MARGIN, FONT, 600, &c.id);
            p.text(
                MARGIN + 120.0,
                FONT,
                400,
                &clip(&format!("{} :: {}", c.reference, c.quantity), 90),
            );
            p.y += LINE;
            p.text(MARGIN + 120.0, FONT, 400, &clip(&c.note, 90));
            p.y += LINE;
        }
        if failed.len() > 500 {
            p.text(
                MARGIN,
                FONT,
                400,
                &format!("… and {} more in the CSV.", failed.len() - 500),
            );
            p.y += LINE;
        }
        p.y += LINE;
    }

    // NIST checks in full, from a new page.
    p.need(PAGE_H);
    p.text(MARGIN, 10.0, 600, "NIST StRD checks");
    p.y += LINE + 2.0;
    p.text(
        MARGIN,
        FONT,
        400,
        "LRE: log relative error, the number of correct significant digits (McCullough 1998), capped at 15 (11 for nonlinear sets).",
    );
    p.y += LINE * 1.5;
    let ncols = [MARGIN, MARGIN + 150.0, MARGIN + 430.0, MARGIN + 470.0];
    let nhead = |p: &mut Pages| {
        for (t, x) in ["Check", "Quantity", "LRE", "Required"].iter().zip(ncols) {
            p.text(x, FONT, 600, t);
        }
        p.y += 3.0;
        let y = p.y;
        p.rule(y);
        p.y += LINE;
    };
    nhead(&mut p);
    for c in r.checks.iter().filter(|c| c.id.starts_with("NIST-")) {
        if p.need(LINE) {
            nhead(&mut p);
        }
        let got = if c.tolerance == "converged" {
            if c.pass { "yes" } else { "NO" }.to_string()
        } else {
            format!("{:.2}", c.got)
        };
        let need = if c.tolerance == "converged" {
            "converged".to_string()
        } else {
            format!("{}", c.expected)
        };
        let cells = [c.id.clone(), clip(&c.quantity, 60), got, need];
        for (t, x) in cells.iter().zip(ncols) {
            p.text(x, FONT, if c.pass { 400 } else { 700 }, t);
        }
        p.y += LINE;
    }
    p.y += LINE;
    p.need(LINE * 3.0);
    p.text(
        MARGIN,
        FONT,
        400,
        &format!(
            "The {} oracle comparisons are listed one per row in the CSV export of this report (same report SHA-256).",
            r.totals.oracle_checks
        ),
    );
    p.finish(&format!(
        "{} {} OQ report {}",
        r.product,
        r.version,
        &r.report_sha256[..16.min(r.report_sha256.len())]
    ))
}

fn sign_block(p: &mut Pages, execution: Option<&Execution>) {
    p.need(LINE * 12.0);
    p.text(MARGIN, 10.0, 600, "Signatures");
    p.y += LINE + 4.0;
    match execution {
        Some(e) => {
            p.text(
                MARGIN,
                FONT + 0.5,
                600,
                &format!("Executed by {} ({})", e.name, e.user),
            );
            p.y += LINE;
            p.text(
                MARGIN,
                FONT + 0.5,
                400,
                &format!(
                    "{} UTC. Meaning: executed. Electronic signature; identity confirmed by password at signing.",
                    e.at.replace('T', " ").replace('Z', "")
                ),
            );
            p.y += LINE * 2.0;
        }
        None => {
            manual_line(p, "Executed by");
        }
    }
    manual_line(p, "Reviewed by (QA)");
}

fn manual_line(p: &mut Pages, role: &str) {
    p.text(MARGIN, FONT + 0.5, 600, role);
    p.y += LINE * 2.2;
    let y = p.y;
    for (x, w, label) in [
        (MARGIN, 170.0, "Name"),
        (MARGIN + 190.0, 170.0, "Signature"),
        (MARGIN + 380.0, 131.0, "Date"),
    ] {
        p.body.push_str(&format!(
            "<line x1=\"{x:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" stroke=\"#1a1a1a\" stroke-width=\"0.6\"/>",
            x + w
        ));
        p.text_at(x, y + 9.0, 6.5, 400, "#666666", label);
    }
    p.y += LINE * 2.0;
}

// ---- commands -------------------------------------------------------------------------------

const FEATURE: &str = "Validation pack";
const MEMO_REPORT: &str = "lab.oq_report";
const MEMO_PARTS: &str = "lab.oq_parts";

fn store(session: &mut Session, report: &OqReport) -> Json {
    let v = serde_json::to_value(report).expect("report serialises");
    session.set_memo(MEMO_REPORT, Some(v.clone()));
    v
}

/// `lab_run_oq`: the whole run, or one step of it (step 0 starts a run).
pub(crate) fn lab_run_oq(
    session: &mut Session,
    step: Option<usize>,
) -> Result<LabResponse, String> {
    session.require_lab(FEATURE)?;
    let n = STEPS.len();
    let Some(i) = step else {
        let report = run();
        return Ok(LabResponse::LabOq {
            step: n,
            steps: n,
            next: None,
            report: Some(store(session, &report)),
        });
    };
    if i >= n {
        return Err(format!("The OQ has {n} steps."));
    }
    let mut parts: Vec<Json> = match (i, session.memo(MEMO_PARTS)) {
        (0, _) => Vec::new(),
        (_, Some(Json::Array(a))) if a.len() == i => a.clone(),
        _ => return Err("Run the OQ steps in order, starting with step 0.".into()),
    };
    parts.push(serde_json::to_value(run_step(i)).expect("step serialises"));
    if i + 1 < n {
        session.set_memo(MEMO_PARTS, Some(Json::Array(parts)));
        return Ok(LabResponse::LabOq {
            step: i + 1,
            steps: n,
            next: Some(STEPS[i + 1].title.into()),
            report: None,
        });
    }
    session.set_memo(MEMO_PARTS, None);
    let results: Vec<StepResult> = parts
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let report = assemble(results);
    Ok(LabResponse::LabOq {
        step: n,
        steps: n,
        next: None,
        report: Some(store(session, &report)),
    })
}

/// `lab_oq_report`: the last report of this session as PDF or CSV.
pub(crate) fn lab_oq_report(
    session: &mut Session,
    format: &str,
    password: Option<&str>,
    path: Option<String>,
) -> Result<LabResponse, String> {
    session.require_lab(FEATURE)?;
    let report: OqReport = session
        .memo(MEMO_REPORT)
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .ok_or("Run the OQ first (Records → Run OQ…).")?;
    let stem = format!("Dioptra {} OQ report", report.version);
    let (bytes, mime) = match format {
        "csv" => (csv(&report).into_bytes(), "text/csv"),
        "pdf" => {
            let execution = match password {
                Some(pw) => {
                    let u = crate::sign::confirm_signer(session, pw)?;
                    Some(Execution {
                        user: u.id,
                        name: u.name,
                        at: dioptra_core::util::now(),
                        meaning: "executed".into(),
                    })
                }
                None => None,
            };
            (pdf(&report, execution.as_ref())?, "application/pdf")
        }
        f => return Err(format!("Unknown report format {f}; use pdf or csv.")),
    };
    if let Some(path) = path {
        dioptra_core::file::write_path(&path, &bytes)?;
        return Ok(LabResponse::LabFile {
            name: path,
            mime: mime.into(),
            base64: String::new(),
            notes: Vec::new(),
        });
    }
    Ok(LabResponse::LabFile {
        name: format!("{stem}.{format}"),
        mime: mime.into(),
        base64: dioptra_core::util::base64_encode(&bytes),
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_set_is_complete() {
        assert_eq!(fixtures::FILES.len(), 79);
        assert_eq!(
            fixtures::FILES
                .iter()
                .filter(|(p, _)| p.starts_with("nist/"))
                .count(),
            NIST_SETS
        );
        assert_eq!(fixture_set_sha256().len(), 64);
    }

    #[test]
    fn numbers_round_trip() {
        let c = OqCheck {
            id: "X".into(),
            section: "X".into(),
            reference: String::new(),
            quantity: String::new(),
            expected: f64::INFINITY,
            got: f64::NAN,
            tolerance: String::new(),
            pass: true,
            note: String::new(),
        };
        let back: OqCheck = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.expected, f64::INFINITY);
        assert!(back.got.is_nan());
    }
}
