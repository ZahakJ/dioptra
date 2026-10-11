//! Analyses: a specification (which table, which test, which options) plus a cached
//! results sheet (DESIGN §4.3, §5).
//!
//! Computations plug in through [`AnalysisEngine`], a table from analysis kind to a
//! function. This crate implements descriptive statistics itself; every other kind is
//! declared (so the UI can be built against its options) and answers with a sheet that
//! says it is not available yet until `dioptra-stats` provides the numerics.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::api::{Id, PValueStyle, Range, SubcolumnFormat, TableKind, VariableType};
use crate::formula::{mean, variance};
use crate::numerics::t_quantile;
use crate::results::{
    AssumptionCheck, CheckStatus, ResultCell, ResultTable, ResultsSheet, Section,
};
use crate::table::{ColLoc, Table};
use crate::util::format_sig;

/// Direction of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alternative {
    /// Two-sided (the default).
    #[default]
    TwoSided,
    /// The first group is larger (or the mean is above `mu`).
    Greater,
    /// The first group is smaller.
    Less,
}

fn ci95() -> f64 {
    0.95
}

/// Which t test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TTestVariant {
    /// One sample against `mu`.
    OneSample,
    /// Student's unpaired (pooled variance).
    Unpaired,
    /// Welch's unpaired (the default).
    #[default]
    Welch,
    /// Paired.
    Paired,
    /// Ratio paired (paired t on logs).
    RatioPaired,
}

/// Multiple-comparison method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum McMethod {
    /// Tukey–Kramer, all pairs (default).
    #[default]
    Tukey,
    /// Dunnett against a control dataset.
    Dunnett {
        /// Dataset index of the control.
        control: usize,
    },
    /// Šidák.
    Sidak,
    /// Holm step-down.
    Holm,
    /// Bonferroni.
    Bonferroni,
    /// Dunn (after rank tests).
    Dunn,
    /// Benjamini–Hochberg false discovery rate.
    BenjaminiHochberg,
}

/// Which comparisons to make.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Comparisons {
    /// Every pair.
    #[default]
    AllPairs,
    /// Each dataset against a control.
    VsControl {
        /// Control dataset index.
        control: usize,
    },
    /// Chosen pairs of dataset indices.
    Selected {
        /// Pairs.
        pairs: Vec<(usize, usize)>,
    },
}

/// Post-hoc multiple comparisons.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MultipleComparisons {
    /// Method.
    pub method: McMethod,
    /// Which comparisons.
    pub comparisons: Comparisons,
}

/// Sums of squares type for two-way ANOVA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SsType {
    /// Type I (sequential).
    I,
    /// Type II.
    Ii,
    /// Type III (default, as Prism).
    #[default]
    Iii,
}

/// Which factor of a two-way design is repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatedFactor {
    /// Rows are matched (each row one subject).
    Rows,
    /// Columns (subcolumns) are matched.
    Columns,
}

/// Correlation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationMethod {
    /// Pearson.
    #[default]
    Pearson,
    /// Spearman rank.
    Spearman,
    /// Kendall's tau-b.
    Kendall,
}

/// Bands drawn around a regression line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bands {
    /// None.
    #[default]
    None,
    /// Confidence band of the line.
    Confidence,
    /// Prediction band for new points.
    Prediction,
    /// Both.
    Both,
}

/// A nonlinear model.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EquationSpec {
    /// Built-in name, e.g. `four_parameter_logistic`, `one_phase_decay`, `michaelis_menten`.
    pub name: String,
    /// A user-defined model in the formula language (`Y = …` of `X` and parameters).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

/// Weighting for nonlinear regression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weighting {
    /// Equal weights.
    #[default]
    None,
    /// 1/Y².
    RelativeY2,
    /// 1/Y.
    PoissonY,
    /// 1/SD² from replicates.
    InverseSd2,
}

/// A parameter constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Constraint {
    /// Fixed value.
    Fixed {
        /// Parameter.
        param: String,
        /// Value.
        value: f64,
    },
    /// Lower bound.
    Min {
        /// Parameter.
        param: String,
        /// Bound.
        value: f64,
    },
    /// Upper bound.
    Max {
        /// Parameter.
        param: String,
        /// Bound.
        value: f64,
    },
    /// Shared across datasets.
    Shared {
        /// Parameter.
        param: String,
    },
}

/// Normality tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalityTest {
    /// Shapiro–Wilk.
    ShapiroWilk,
    /// D'Agostino–Pearson omnibus K².
    DagostinoPearson,
    /// Anderson–Darling.
    AndersonDarling,
    /// Kolmogorov–Smirnov (Lilliefors).
    KolmogorovSmirnov,
}

/// Outlier method.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum OutlierMethod {
    /// Grubbs (one outlier at a time).
    Grubbs {
        /// Significance level.
        alpha: f64,
    },
    /// ROUT with false discovery rate Q (percent).
    Rout {
        /// Q in percent (1 is the recommended default).
        q: f64,
    },
}

impl Default for OutlierMethod {
    fn default() -> Self {
        OutlierMethod::Rout { q: 1.0 }
    }
}

/// Survival curve comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurvivalCompare {
    /// Log-rank (Mantel–Cox).
    #[default]
    LogRank,
    /// Gehan–Breslow–Wilcoxon.
    GehanBreslowWilcoxon,
}

/// Bland–Altman difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlandAltmanMethod {
    /// A − B.
    #[default]
    Difference,
    /// A / B.
    Ratio,
    /// 100 (A − B) / mean.
    PercentDifference,
}

macro_rules! spec_struct {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty = $d:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
        impl Default for $name {
            fn default() -> Self { Self { $($f: $d),* } }
        }
    };
}

spec_struct!(
    /// Descriptive statistics options.
    DescriptiveSpec {
        /// Flat columns to describe, grouped by dataset (empty: every dataset).
        columns: Vec<usize> = Vec::new(),
        /// Confidence level of the mean's interval.
        ci_level: f64 = ci95(),
        /// Extra percentiles (0–100).
        percentiles: Vec<f64> = Vec::new(),
    }
);
spec_struct!(
    /// t test options.
    TTestSpec {
        /// Flat columns (one for one-sample, two otherwise; empty: first datasets).
        columns: Vec<usize> = Vec::new(),
        /// Which test.
        variant: TTestVariant = TTestVariant::Welch,
        /// Hypothetical mean (one-sample) or difference.
        mu: f64 = 0.0,
        /// Direction.
        alternative: Alternative = Alternative::TwoSided,
        /// Confidence level.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// Mann–Whitney options.
    MannWhitneySpec {
        /// Two flat columns.
        columns: Vec<usize> = Vec::new(),
        /// Direction.
        alternative: Alternative = Alternative::TwoSided,
        /// Exact p when affordable (None = automatic).
        exact: Option<bool> = None,
        /// Confidence level of the Hodges–Lehmann interval.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// Wilcoxon signed-rank options.
    WilcoxonSpec {
        /// One column (vs `mu`) or two (paired).
        columns: Vec<usize> = Vec::new(),
        /// Hypothetical median.
        mu: f64 = 0.0,
        /// Direction.
        alternative: Alternative = Alternative::TwoSided,
        /// Confidence level.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// One-way ANOVA options.
    OneWayAnovaSpec {
        /// Datasets as flat columns (empty: all).
        columns: Vec<usize> = Vec::new(),
        /// Post-hoc comparisons.
        post_hoc: Option<MultipleComparisons> = Some(MultipleComparisons::default()),
        /// Welch's ANOVA (unequal variances).
        welch: bool = false,
        /// Brown–Forsythe ANOVA.
        brown_forsythe: bool = false,
    }
);
spec_struct!(
    /// Repeated-measures one-way ANOVA options.
    RmAnovaSpec {
        /// Datasets as flat columns (empty: all).
        columns: Vec<usize> = Vec::new(),
        /// Greenhouse–Geisser correction.
        gg_correction: bool = true,
        /// Post-hoc comparisons.
        post_hoc: Option<MultipleComparisons> = Some(MultipleComparisons::default()),
    }
);
spec_struct!(
    /// Two-way ANOVA options.
    TwoWayAnovaSpec {
        /// Sums of squares.
        ss_type: SsType = SsType::Iii,
        /// Post-hoc comparisons.
        post_hoc: Option<MultipleComparisons> = None,
        /// Repeated factor, if any.
        repeated: Option<RepeatedFactor> = None,
    }
);
spec_struct!(
    /// Kruskal–Wallis options.
    KruskalWallisSpec {
        /// Datasets as flat columns (empty: all).
        columns: Vec<usize> = Vec::new(),
        /// Dunn's post-hoc test.
        dunn: bool = true,
    }
);
spec_struct!(
    /// Friedman options.
    FriedmanSpec {
        /// Datasets as flat columns (empty: all).
        columns: Vec<usize> = Vec::new(),
        /// Dunn's post-hoc test.
        dunn: bool = true,
    }
);
spec_struct!(
    /// Chi-square options.
    ChiSquareSpec {
        /// Yates continuity correction (2×2 only).
        yates: bool = false,
    }
);
spec_struct!(
    /// Fisher's exact test options.
    FisherExactSpec {
        /// Direction.
        alternative: Alternative = Alternative::TwoSided,
    }
);
spec_struct!(
    /// Correlation options.
    CorrelationSpec {
        /// Flat columns (empty: all numeric).
        columns: Vec<usize> = Vec::new(),
        /// Method.
        method: CorrelationMethod = CorrelationMethod::Pearson,
        /// Confidence level.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// Simple linear regression options.
    LinearRegressionSpec {
        /// Datasets (empty: all).
        datasets: Vec<usize> = Vec::new(),
        /// Force the line through the origin.
        through_origin: bool = false,
        /// Bands to compute.
        bands: Bands = Bands::None,
        /// Confidence level.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// Nonlinear regression options.
    NonlinearRegressionSpec {
        /// Datasets (empty: all).
        datasets: Vec<usize> = Vec::new(),
        /// Model.
        equation: EquationSpec = EquationSpec { name: "four_parameter_logistic".into(), formula: None },
        /// Weighting.
        weighting: Weighting = Weighting::None,
        /// Robust fit.
        robust: bool = false,
        /// Constraints.
        constraints: Vec<Constraint> = Vec::new(),
        /// Initial values by parameter name.
        initial: BTreeMap<String, f64> = BTreeMap::new(),
        /// Compare with a second model (extra sum of squares F test / AICc).
        compare_with: Option<EquationSpec> = None,
    }
);
spec_struct!(
    /// Normality test options.
    NormalitySpec {
        /// Flat columns (empty: all datasets).
        columns: Vec<usize> = Vec::new(),
        /// Tests to run.
        tests: Vec<NormalityTest> = vec![NormalityTest::ShapiroWilk, NormalityTest::DagostinoPearson],
    }
);
spec_struct!(
    /// Outlier identification options.
    OutliersSpec {
        /// Flat columns (empty: all datasets).
        columns: Vec<usize> = Vec::new(),
        /// Method.
        method: OutlierMethod = OutlierMethod::Rout { q: 1.0 },
    }
);
spec_struct!(
    /// Survival analysis options.
    SurvivalSpec {
        /// Curve comparison.
        compare: SurvivalCompare = SurvivalCompare::LogRank,
        /// Confidence level of the curves.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// ROC curve options.
    RocSpec {
        /// Controls and patients as two flat columns (column tables).
        columns: Vec<usize> = Vec::new(),
        /// Label of the positive class (variables tables).
        positive_label: Option<String> = None,
        /// Confidence level of the AUC.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// Bland–Altman options.
    BlandAltmanSpec {
        /// Two flat columns (methods A and B).
        columns: Vec<usize> = Vec::new(),
        /// Difference measure.
        method: BlandAltmanMethod = BlandAltmanMethod::Difference,
        /// Confidence level for the limits of agreement.
        ci_level: f64 = ci95(),
    }
);
spec_struct!(
    /// Principal component analysis options.
    PcaSpec {
        /// Flat columns (empty: all numeric variables).
        columns: Vec<usize> = Vec::new(),
        /// Standardise variables (correlation matrix).
        standardize: bool = true,
        /// Number of components to keep (None: by parallel analysis).
        components: Option<usize> = None,
    }
);

/// An analysis specification. Serialised with a `kind` tag (`{"kind": "t_test", …}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalysisSpec {
    /// Descriptive statistics.
    Descriptive(DescriptiveSpec),
    /// t tests.
    TTest(TTestSpec),
    /// Mann–Whitney U.
    MannWhitney(MannWhitneySpec),
    /// Wilcoxon signed-rank.
    Wilcoxon(WilcoxonSpec),
    /// One-way ANOVA.
    OneWayAnova(OneWayAnovaSpec),
    /// Repeated-measures one-way ANOVA.
    RepeatedMeasuresAnova(RmAnovaSpec),
    /// Two-way ANOVA.
    TwoWayAnova(TwoWayAnovaSpec),
    /// Kruskal–Wallis.
    KruskalWallis(KruskalWallisSpec),
    /// Friedman.
    Friedman(FriedmanSpec),
    /// Chi-square test of a contingency table.
    ChiSquare(ChiSquareSpec),
    /// Fisher's exact test.
    FisherExact(FisherExactSpec),
    /// Correlation.
    Correlation(CorrelationSpec),
    /// Simple linear regression.
    LinearRegression(LinearRegressionSpec),
    /// Nonlinear regression.
    NonlinearRegression(NonlinearRegressionSpec),
    /// Normality tests.
    Normality(NormalitySpec),
    /// Outlier identification.
    Outliers(OutliersSpec),
    /// Kaplan–Meier survival.
    Survival(SurvivalSpec),
    /// ROC curve.
    Roc(RocSpec),
    /// Bland–Altman agreement.
    BlandAltman(BlandAltmanSpec),
    /// Principal component analysis.
    Pca(PcaSpec),
}

/// Every analysis kind with its display name, in menu order.
pub const KINDS: [(&str, &str); 20] = [
    ("descriptive", "Descriptive statistics"),
    ("t_test", "t test"),
    ("mann_whitney", "Mann–Whitney test"),
    ("wilcoxon", "Wilcoxon signed-rank test"),
    ("one_way_anova", "One-way ANOVA"),
    ("repeated_measures_anova", "Repeated-measures one-way ANOVA"),
    ("two_way_anova", "Two-way ANOVA"),
    ("kruskal_wallis", "Kruskal–Wallis test"),
    ("friedman", "Friedman test"),
    ("chi_square", "Chi-square test"),
    ("fisher_exact", "Fisher's exact test"),
    ("correlation", "Correlation"),
    ("linear_regression", "Simple linear regression"),
    ("nonlinear_regression", "Nonlinear regression (curve fit)"),
    ("normality", "Normality tests"),
    ("outliers", "Identify outliers"),
    ("survival", "Survival curves (Kaplan–Meier)"),
    ("roc", "ROC curve"),
    ("bland_altman", "Bland–Altman"),
    ("pca", "Principal component analysis"),
];

impl AnalysisSpec {
    /// The kind name, as in [`KINDS`].
    pub fn kind(&self) -> &'static str {
        match self {
            AnalysisSpec::Descriptive(_) => "descriptive",
            AnalysisSpec::TTest(_) => "t_test",
            AnalysisSpec::MannWhitney(_) => "mann_whitney",
            AnalysisSpec::Wilcoxon(_) => "wilcoxon",
            AnalysisSpec::OneWayAnova(_) => "one_way_anova",
            AnalysisSpec::RepeatedMeasuresAnova(_) => "repeated_measures_anova",
            AnalysisSpec::TwoWayAnova(_) => "two_way_anova",
            AnalysisSpec::KruskalWallis(_) => "kruskal_wallis",
            AnalysisSpec::Friedman(_) => "friedman",
            AnalysisSpec::ChiSquare(_) => "chi_square",
            AnalysisSpec::FisherExact(_) => "fisher_exact",
            AnalysisSpec::Correlation(_) => "correlation",
            AnalysisSpec::LinearRegression(_) => "linear_regression",
            AnalysisSpec::NonlinearRegression(_) => "nonlinear_regression",
            AnalysisSpec::Normality(_) => "normality",
            AnalysisSpec::Outliers(_) => "outliers",
            AnalysisSpec::Survival(_) => "survival",
            AnalysisSpec::Roc(_) => "roc",
            AnalysisSpec::BlandAltman(_) => "bland_altman",
            AnalysisSpec::Pca(_) => "pca",
        }
    }

    /// Display name.
    pub fn name(&self) -> &'static str {
        display_name(self.kind())
    }

    /// Default options for a kind on a table.
    pub fn default_for(kind: &str, table: &Table) -> Option<Self> {
        let two = || -> Vec<usize> {
            (0..table.datasets.len().min(2))
                .filter_map(|d| table.flat_index(d, 0))
                .collect()
        };
        Some(match kind {
            "descriptive" => AnalysisSpec::Descriptive(Default::default()),
            "t_test" => {
                let cols = two();
                let variant = if cols.len() == 1 {
                    TTestVariant::OneSample
                } else {
                    TTestVariant::Welch
                };
                AnalysisSpec::TTest(TTestSpec {
                    columns: cols,
                    variant,
                    ..Default::default()
                })
            }
            "mann_whitney" => AnalysisSpec::MannWhitney(MannWhitneySpec {
                columns: two(),
                ..Default::default()
            }),
            "wilcoxon" => AnalysisSpec::Wilcoxon(WilcoxonSpec {
                columns: two(),
                ..Default::default()
            }),
            "one_way_anova" => AnalysisSpec::OneWayAnova(Default::default()),
            "repeated_measures_anova" => AnalysisSpec::RepeatedMeasuresAnova(Default::default()),
            "two_way_anova" => AnalysisSpec::TwoWayAnova(Default::default()),
            "kruskal_wallis" => AnalysisSpec::KruskalWallis(Default::default()),
            "friedman" => AnalysisSpec::Friedman(Default::default()),
            "chi_square" => AnalysisSpec::ChiSquare(Default::default()),
            "fisher_exact" => AnalysisSpec::FisherExact(Default::default()),
            "correlation" => AnalysisSpec::Correlation(Default::default()),
            "linear_regression" => AnalysisSpec::LinearRegression(Default::default()),
            "nonlinear_regression" => AnalysisSpec::NonlinearRegression(Default::default()),
            "normality" => AnalysisSpec::Normality(Default::default()),
            "outliers" => AnalysisSpec::Outliers(Default::default()),
            "survival" => AnalysisSpec::Survival(Default::default()),
            "roc" => AnalysisSpec::Roc(RocSpec {
                columns: two(),
                ..Default::default()
            }),
            "bland_altman" => AnalysisSpec::BlandAltman(BlandAltmanSpec {
                columns: two(),
                ..Default::default()
            }),
            "pca" => AnalysisSpec::Pca(Default::default()),
            _ => return None,
        })
    }
}

/// Display name of a kind.
pub fn display_name(kind: &str) -> &'static str {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, n)| *n)
        .unwrap_or("Analysis")
}

/// An analysis object in a project. Equality compares the document (id, title, table,
/// spec), not the cached results or the stale flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    /// Id.
    pub id: Id,
    /// Title.
    pub title: String,
    /// Table analysed.
    pub table: Id,
    /// Specification.
    pub spec: AnalysisSpec,
    /// Cached results (saved separately in `results/<id>.json`).
    #[serde(skip)]
    pub results: Option<ResultsSheet>,
    /// Whether the cached results are out of date (runtime).
    #[serde(skip)]
    pub stale: bool,
}

impl PartialEq for Analysis {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id && self.title == o.title && self.table == o.table && self.spec == o.spec
    }
}

/// One entry of the Analyze list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisOption {
    /// Kind name.
    pub kind: String,
    /// Display name.
    pub name: String,
    /// Whether it fits this table (and selection).
    pub fits: bool,
    /// Why it does not fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether the engine can compute it yet.
    pub implemented: bool,
}

fn kind_name(k: TableKind) -> &'static str {
    match k {
        TableKind::Xy => "an XY",
        TableKind::Column => "a column",
        TableKind::Grouped => "a grouped",
        TableKind::Contingency => "a contingency",
        TableKind::Survival => "a survival",
        TableKind::Parts => "a parts-of-whole",
        TableKind::Variables => "a multiple-variables",
        TableKind::Nested => "a nested",
    }
}

/// Which table kinds an analysis accepts, and how many datasets it needs at least.
fn accepts(kind: &str) -> (&'static [TableKind], usize) {
    use TableKind::*;
    match kind {
        "descriptive" | "normality" | "outliers" => (&[Column, Xy, Grouped, Variables, Nested], 1),
        "t_test" | "wilcoxon" => (&[Column, Variables, Nested], 1),
        "mann_whitney" | "roc" | "bland_altman" => (&[Column, Variables], 2),
        "one_way_anova" | "kruskal_wallis" => (&[Column, Nested], 2),
        "repeated_measures_anova" | "friedman" => (&[Column], 3),
        "two_way_anova" => (&[Grouped], 2),
        "chi_square" => (&[Contingency, Parts], 1),
        "fisher_exact" => (&[Contingency], 2),
        "correlation" => (&[Xy, Variables, Column], 1),
        "linear_regression" | "nonlinear_regression" => (&[Xy], 1),
        "survival" => (&[Survival], 1),
        "pca" => (&[Variables], 2),
        _ => (&[], 0),
    }
}

/// Analyses for a table, with those that do not fit marked and explained.
pub fn list_analyses(
    table: &Table,
    range: Option<Range>,
    engine: &AnalysisEngine,
) -> Vec<AnalysisOption> {
    let selected_datasets = range.map(|r| {
        let mut ds: Vec<usize> = (r.col0..r.col1)
            .filter_map(|c| match table.locate(c) {
                Some(ColLoc::Y { dataset, .. }) => Some(dataset),
                _ => None,
            })
            .collect();
        ds.dedup();
        ds.len()
    });
    KINDS
        .iter()
        .map(|(kind, name)| {
            let (kinds, min_ds) = accepts(kind);
            let n = selected_datasets
                .filter(|&n| n > 0)
                .unwrap_or(table.datasets.len());
            let reason = if !kinds.contains(&table.kind) {
                let names: Vec<&str> = kinds.iter().map(|k| kind_name(*k)).collect();
                Some(format!(
                    "Needs {} table; this is {} table.",
                    join_or(&names),
                    kind_name(table.kind)
                ))
            } else if n < min_ds {
                Some(format!(
                    "Needs at least {min_ds} data sets; {n} {} selected.",
                    if n == 1 { "is" } else { "are" }
                ))
            } else if *kind == "fisher_exact" && (table.datasets.len() != 2 || table.rows != 2) {
                Some("Fisher's exact test here needs a 2 × 2 table.".into())
            } else {
                None
            };
            AnalysisOption {
                kind: kind.to_string(),
                name: name.to_string(),
                fits: reason.is_none(),
                reason,
                implemented: engine.has(kind),
            }
        })
        .collect()
}

fn join_or(items: &[&str]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].to_string(),
        n => format!("{} or {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

/// What a computation receives.
pub struct AnalysisContext<'a> {
    /// The table (already recomputed).
    pub table: &'a Table,
    /// The specification.
    pub spec: &'a AnalysisSpec,
    /// Project p-value style, for summary sentences.
    pub p_style: PValueStyle,
}

/// A computation: table and spec in, results sheet out.
pub type ComputeFn = fn(&AnalysisContext<'_>) -> Result<ResultsSheet, String>;

/// The table of computations by kind. `dioptra-stats` wiring registers more here.
#[derive(Clone)]
pub struct AnalysisEngine {
    compute: BTreeMap<&'static str, ComputeFn>,
}

impl std::fmt::Debug for AnalysisEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalysisEngine")
            .field("kinds", &self.compute.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for AnalysisEngine {
    fn default() -> Self {
        let mut e = AnalysisEngine {
            compute: BTreeMap::new(),
        };
        e.register("descriptive", descriptive);
        e
    }
}

impl AnalysisEngine {
    /// Register (or replace) the computation for a kind.
    pub fn register(&mut self, kind: &'static str, f: ComputeFn) {
        self.compute.insert(kind, f);
    }

    /// Whether a kind can be computed.
    pub fn has(&self, kind: &str) -> bool {
        self.compute.contains_key(kind)
    }

    /// Run an analysis. Failures become a sheet with a message, never a panic.
    pub fn run(&self, table: &Table, spec: &AnalysisSpec, p_style: PValueStyle) -> ResultsSheet {
        let kind = spec.kind();
        let (kinds, _) = accepts(kind);
        let mut sheet = if !kinds.contains(&table.kind) {
            message_sheet(
                spec,
                format!(
                    "{} cannot be run on {} table.",
                    spec.name(),
                    kind_name(table.kind)
                ),
            )
        } else {
            match self.compute.get(kind) {
                Some(f) => {
                    let ctx = AnalysisContext { table, spec, p_style };
                    f(&ctx).unwrap_or_else(|e| message_sheet(spec, e))
                }
                None => message_sheet(
                    spec,
                    format!("{} is not available in this build yet: its numerics are still being written and tested (dioptra-stats).", spec.name()),
                ),
            }
        };
        if sheet.checklist.is_empty() {
            sheet.checklist = checklist(kind);
        }
        sheet
    }
}

fn message_sheet(spec: &AnalysisSpec, message: String) -> ResultsSheet {
    ResultsSheet {
        title: spec.name().into(),
        analysis_kind: spec.kind().into(),
        message: Some(message),
        ..Default::default()
    }
}

fn checklist(kind: &str) -> Vec<String> {
    let v: &[&str] = match kind {
        "descriptive" => &[
            "Are the values independent? Repeated measurements of one subject are not independent observations.",
            "The SD describes scatter in the data; the SEM and CI describe the precision of the mean. Report the one that answers your question.",
            "If the distribution is skewed, the median and quartiles describe it better than the mean and SD.",
        ],
        "t_test" => &[
            "Are the two groups sampled from populations that are roughly Gaussian, or are the samples large?",
            "Welch's test does not assume equal SDs; Student's test does.",
            "Are the data unpaired? If each value in one group pairs with one in the other, use the paired test.",
            "Did you choose a one-sided test before seeing the data? If not, use two-sided.",
        ],
        "one_way_anova" | "repeated_measures_anova" => &[
            "Are the residuals roughly Gaussian and the SDs similar (or is Welch's ANOVA chosen)?",
            "Are the errors independent?",
            "Plan the comparisons you care about before looking at the data.",
        ],
        "chi_square" | "fisher_exact" => &[
            "Are the counts of independent subjects (not of repeated observations of the same subjects)?",
            "With small expected counts, use Fisher's exact test.",
        ],
        _ => &["Check that the design of the experiment matches the assumptions of this analysis."],
    };
    v.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// Descriptive statistics
// ---------------------------------------------------------------------------

/// Percentile by the (n + 1)p rule (Hyndman–Fan type 6), as Prism does. `sorted` ascending.
pub fn percentile6(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    let h = (n as f64 + 1.0) * p;
    if h <= 1.0 {
        return sorted[0];
    }
    if h >= n as f64 {
        return sorted[n - 1];
    }
    let lo = h.floor() as usize;
    sorted[lo - 1] + (h - lo as f64) * (sorted[lo] - sorted[lo - 1])
}

struct Group {
    name: String,
    values: Vec<f64>,
    text: usize,
    excluded: usize,
    summary_format: Option<SubcolumnFormat>,
}

fn groups(table: &Table, columns: &[usize]) -> Vec<Group> {
    let mut picks: Vec<(Option<usize>, Vec<usize>)> = Vec::new(); // (dataset or None for X, flat cols)
    if columns.is_empty() {
        for (d, ds) in table.datasets.iter().enumerate() {
            if table.kind == TableKind::Variables && ds.var_type != Some(VariableType::Number) {
                continue;
            }
            picks.push((
                Some(d),
                (0..ds.subcolumns.len())
                    .filter_map(|s| table.flat_index(d, s))
                    .collect(),
            ));
        }
    } else {
        for &c in columns {
            let key = match table.locate(c) {
                Some(ColLoc::X) => None,
                Some(ColLoc::Y { dataset, .. }) => Some(dataset),
                None => continue,
            };
            match picks.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(c),
                None => picks.push((key, vec![c])),
            }
        }
    }
    picks
        .into_iter()
        .map(|(d, cols)| {
            let name = match d {
                Some(d) => table.datasets[d].name.clone(),
                None => table.col_header(0),
            };
            let fmt = d.map(|d| table.datasets[d].format);
            let summary_format =
                fmt.filter(|f| !matches!(f, SubcolumnFormat::Values(_) | SubcolumnFormat::Single));
            let mut g = Group {
                name,
                values: Vec::new(),
                text: 0,
                excluded: 0,
                summary_format,
            };
            for c in cols {
                let col = table.col(c).expect("column");
                g.values.extend(col.usable());
                g.text += col
                    .text
                    .keys()
                    .filter(|r| !col.excluded.contains(r))
                    .count();
                g.excluded += col.excluded.len();
            }
            g
        })
        .collect()
}

fn descriptive(ctx: &AnalysisContext<'_>) -> Result<ResultsSheet, String> {
    let AnalysisSpec::Descriptive(spec) = ctx.spec else {
        return Err("not a descriptive spec".into());
    };
    if !(0.0 < spec.ci_level && spec.ci_level < 1.0) {
        return Err(format!(
            "The confidence level must be between 0 and 1 (got {}).",
            spec.ci_level
        ));
    }
    for p in &spec.percentiles {
        if !(0.0..=100.0).contains(p) {
            return Err(format!("Percentiles must be between 0 and 100 (got {p})."));
        }
    }
    let gs = groups(ctx.table, &spec.columns);
    if gs.is_empty() {
        return Err("There are no numeric data sets to describe.".into());
    }
    let lvl = format_sig(spec.ci_level * 100.0, None, 4);
    let mut columns: Vec<String> = ["Data set", "N", "Mean", "SD", "SEM"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    columns.push(format!("Lower {lvl}% CI of mean"));
    columns.push(format!("Upper {lvl}% CI of mean"));
    for s in [
        "Minimum",
        "25% percentile",
        "Median",
        "75% percentile",
        "Maximum",
        "Range",
        "CV (%)",
        "Geometric mean",
    ] {
        columns.push(s.into());
    }
    for p in &spec.percentiles {
        columns.push(format!("{}% percentile", format_sig(*p, None, 4)));
    }
    let mut rows = Vec::new();
    let mut summary = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut caution = false;
    for g in &gs {
        if let Some(f) = g.summary_format {
            notes.push(format!(
                "{} was entered as summary values ({f:?}); descriptive statistics need raw values.",
                g.name
            ));
            let mut row = vec![ResultCell::text(g.name.clone())];
            row.resize(columns.len(), ResultCell::Empty);
            rows.push(row);
            continue;
        }
        if g.text > 0 {
            caution = true;
            notes.push(format!(
                "{}: {} text cell{} not used.",
                g.name,
                g.text,
                if g.text == 1 { " was" } else { "s were" }
            ));
        }
        if g.excluded > 0 {
            notes.push(format!(
                "{}: {} excluded value{} not used.",
                g.name,
                g.excluded,
                if g.excluded == 1 { " was" } else { "s were" }
            ));
        }
        let mut x = g.values.clone();
        x.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        let n = x.len();
        let m = if n > 0 { mean(&x) } else { f64::NAN };
        let sd = if n > 1 { variance(&x).sqrt() } else { f64::NAN };
        let sem = sd / (n as f64).sqrt();
        let tq = if n > 1 {
            t_quantile(0.5 + spec.ci_level / 2.0, n as f64 - 1.0)
        } else {
            f64::NAN
        };
        let (lo, hi) = (m - tq * sem, m + tq * sem);
        let min = x.first().copied().unwrap_or(f64::NAN);
        let max = x.last().copied().unwrap_or(f64::NAN);
        let geo = if n > 0 && x.iter().all(|v| *v > 0.0) {
            (x.iter().map(|v| v.ln()).sum::<f64>() / n as f64).exp()
        } else {
            f64::NAN
        };
        let cv = if m != 0.0 { 100.0 * sd / m } else { f64::NAN };
        let mut row = vec![
            ResultCell::text(g.name.clone()),
            ResultCell::count(n),
            ResultCell::num(m),
            ResultCell::num(sd),
            ResultCell::num(sem),
            ResultCell::num(lo),
            ResultCell::num(hi),
            ResultCell::num(min),
            ResultCell::num(percentile6(&x, 0.25)),
            ResultCell::num(percentile6(&x, 0.5)),
            ResultCell::num(percentile6(&x, 0.75)),
            ResultCell::num(max),
            ResultCell::num(max - min),
            ResultCell::num(cv),
            ResultCell::num(geo),
        ];
        for p in &spec.percentiles {
            row.push(ResultCell::num(percentile6(&x, p / 100.0)));
        }
        rows.push(row);
        let f = |v: f64| format_sig(v, None, 4);
        summary.push(if n > 1 {
            format!(
                "{}: M = {}, SD = {}, {lvl}% CI [{}, {}], n = {n}.",
                g.name,
                f(m),
                f(sd),
                f(lo),
                f(hi)
            )
        } else if n == 1 {
            format!("{}: one value, {}.", g.name, f(m))
        } else {
            format!("{}: no values.", g.name)
        });
    }
    let mut assumptions = vec![AssumptionCheck {
        name: "Values used".into(),
        status: if caution {
            CheckStatus::Caution
        } else {
            CheckStatus::Pass
        },
        detail: if notes.is_empty() {
            "Every non-empty numeric cell was used.".into()
        } else {
            notes.join(" ")
        },
    }];
    assumptions.push(AssumptionCheck {
        name: "Independent observations".into(),
        status: CheckStatus::NotChecked,
        detail: "The CI of the mean assumes each value is an independent sample; only the design of the experiment can tell.".into(),
    });
    assumptions.push(AssumptionCheck {
        name: "Gaussian distribution".into(),
        status: CheckStatus::NotChecked,
        detail: "The CI of the mean is exact for Gaussian data and approximate otherwise; run a normality test, or report the median and quartiles for skewed data.".into(),
    });
    let mut sheet = ResultsSheet {
        title: "Descriptive statistics".into(),
        analysis_kind: "descriptive".into(),
        sections: vec![Section {
            title: "Descriptive statistics".into(),
            table: ResultTable { columns, rows },
        }],
        assumptions,
        summary,
        checklist: checklist("descriptive"),
        ..Default::default()
    };
    sheet.checklist.push("Quartiles and percentiles use the (n + 1)p rule, as Prism does; spreadsheet QUARTILE.INC can differ slightly for small samples.".into());
    Ok(sheet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        let mut t = Table::new(
            "t".into(),
            TableKind::Column,
            "T".into(),
            2,
            SubcolumnFormat::Values(1),
            0,
        );
        t.datasets[0].name = "Control".into();
        t.datasets[1].name = "Drug".into();
        for (r, v) in ["2", "4", "4", "4", "5", "5", "7", "9"].iter().enumerate() {
            t.set_cell_raw(r, 0, v).unwrap();
        }
        for (r, v) in ["1", "2", "oops", "100"].iter().enumerate() {
            t.set_cell_raw(r, 1, v).unwrap();
        }
        t.datasets[1].subcolumns[0].excluded.insert(3);
        t
    }

    fn num(c: &ResultCell) -> f64 {
        match c {
            ResultCell::Number { v, .. } => *v,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn descriptive_values() {
        let t = table();
        let e = AnalysisEngine::default();
        let s = e.run(
            &t,
            &AnalysisSpec::Descriptive(Default::default()),
            PValueStyle::Apa,
        );
        assert!(s.message.is_none(), "{:?}", s.message);
        let r = &s.sections[0].table.rows[0];
        assert_eq!(num(&r[1]), 8.0);
        assert_eq!(num(&r[2]), 5.0);
        let sd = (32.0f64 / 7.0).sqrt();
        assert!((num(&r[3]) - sd).abs() < 1e-12);
        assert!((num(&r[4]) - sd / 8f64.sqrt()).abs() < 1e-12);
        let half = 2.3646242510102993 * sd / 8f64.sqrt();
        assert!((num(&r[5]) - (5.0 - half)).abs() < 1e-9);
        assert!((num(&r[6]) - (5.0 + half)).abs() < 1e-9);
        assert_eq!(num(&r[7]), 2.0);
        assert_eq!(num(&r[8]), 4.0);
        assert_eq!(num(&r[9]), 4.5);
        assert_eq!(num(&r[10]), 6.5);
        assert_eq!(num(&r[11]), 9.0);
        let d = &s.sections[0].table.rows[1];
        assert_eq!(num(&d[1]), 2.0);
        assert_eq!(num(&d[2]), 1.5);
        assert_eq!(s.assumptions[0].status, CheckStatus::Caution);
        assert!(s.assumptions[0].detail.contains("1 text cell was not used"));
        assert!(s.assumptions[0]
            .detail
            .contains("1 excluded value was not used"));
        assert!(
            s.summary[0].starts_with("Control: M = 5, SD = 2.138, 95% CI [3.213, 6.787], n = 8."),
            "{}",
            s.summary[0]
        );
    }

    #[test]
    fn unimplemented_kinds_say_so() {
        let t = table();
        let e = AnalysisEngine::default();
        let spec = AnalysisSpec::default_for("t_test", &t).unwrap();
        let s = e.run(&t, &spec, PValueStyle::Apa);
        assert!(s.message.unwrap().contains("not available"));
        assert!(!s.checklist.is_empty());
    }

    #[test]
    fn listing_marks_fit() {
        let t = table();
        let l = list_analyses(&t, None, &AnalysisEngine::default());
        let get = |k: &str| l.iter().find(|o| o.kind == k).unwrap();
        assert!(get("descriptive").fits && get("descriptive").implemented);
        assert!(get("t_test").fits && !get("t_test").implemented);
        assert!(!get("chi_square").fits);
        assert!(get("chi_square")
            .reason
            .as_ref()
            .unwrap()
            .contains("contingency"));
        assert!(!get("friedman").fits);
    }

    #[test]
    fn spec_json_shape() {
        let s = AnalysisSpec::TTest(TTestSpec::default());
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j["kind"], "t_test");
        assert_eq!(j["variant"], "welch");
        let back: AnalysisSpec =
            serde_json::from_str(r#"{"kind":"one_way_anova","welch":true}"#).unwrap();
        match back {
            AnalysisSpec::OneWayAnova(o) => {
                assert!(o.welch);
                assert_eq!(o.post_hoc.unwrap().method, McMethod::Tukey);
            }
            _ => panic!(),
        }
        let o: AnalysisSpec = serde_json::from_str(
            r#"{"kind":"outliers","method":{"method":"grubbs","alpha":0.05}}"#,
        )
        .unwrap();
        assert_eq!(
            o,
            AnalysisSpec::Outliers(OutliersSpec {
                columns: vec![],
                method: OutlierMethod::Grubbs { alpha: 0.05 }
            })
        );
        for (k, _) in KINDS {
            let spec = AnalysisSpec::default_for(k, &table()).unwrap();
            assert_eq!(spec.kind(), k);
            let j = serde_json::to_string(&spec).unwrap();
            assert_eq!(serde_json::from_str::<AnalysisSpec>(&j).unwrap(), spec);
        }
    }
}
