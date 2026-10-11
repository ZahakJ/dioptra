//! The data a graph draws, extracted from a table by `dioptra-core`.
//!
//! Conventions:
//! - `Series::x` holds one entry per X value or category position (categories sit at
//!   1, 2, 3, …). `y` is aligned with `x`: the value, or the mean of the replicates.
//! - `replicates`, when present, is aligned with `x` and holds the raw values at that X.
//!   Missing cells are kept as NaN so rows line up across series (paired data); every
//!   family skips non-finite values.
//! - `error` holds precomputed absolute (low, high) bounds aligned with `x`, for tables
//!   that store mean/SD/N and similar. When it is empty, the engine computes error bars
//!   from `replicates` as the spec asks.
//! - `groups` labels each replicate value with a subgroup (nested tables, SuperPlots); it
//!   is aligned with the replicates flattened in order (`replicates[0]`, then
//!   `replicates[1]`, …), and `GraphData::group_names` names the subgroups.

use serde::{Deserialize, Serialize};

/// Table kinds (DESIGN §4.1). `dioptra-core` re-exports this as `api::TableKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    /// X column plus Y datasets with subcolumns.
    #[default]
    Xy,
    /// Datasets of values, rows unaligned.
    Column,
    /// Rows = factor A levels, datasets = factor B levels, subcolumns = replicates.
    Grouped,
    /// Rows × datasets of counts.
    Contingency,
    /// X = time, datasets with 1 = event, 0 = censored.
    Survival,
    /// One dataset, rows = categories.
    Parts,
    /// Flat data frame; each dataset is a variable with a declared type.
    Variables,
    /// Datasets = groups, subcolumns = subgroups.
    Nested,
}

/// One plotted series (usually one dataset).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Series {
    /// Name (dataset name).
    pub name: String,
    /// Dataset index in the source table.
    pub dataset: usize,
    /// X values or category positions (1-based).
    pub x: Vec<f64>,
    /// Y values (or means of the replicates), aligned with `x`.
    pub y: Vec<f64>,
    /// Raw replicate values per X, aligned with `x` (may contain NaN).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub replicates: Vec<Vec<f64>>,
    /// Precomputed (low, high) error bounds per X.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub error: Vec<(f64, f64)>,
    /// Subgroup id per replicate value (flattened order).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<usize>,
    /// Precomputed (low, high) X error bounds per X.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub x_error: Vec<(f64, f64)>,
    /// Point labels aligned with `x` (volcano, forest).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

impl Series {
    /// A series of plain (x, y) points.
    pub fn xy(name: &str, dataset: usize, x: Vec<f64>, y: Vec<f64>) -> Self {
        Series {
            name: name.into(),
            dataset,
            x,
            y,
            ..Default::default()
        }
    }

    /// A categorical series: one category at `position` holding `values`.
    pub fn category(name: &str, dataset: usize, position: f64, values: Vec<f64>) -> Self {
        let m = crate::stats::mean(&values);
        Series {
            name: name.into(),
            dataset,
            x: vec![position],
            y: vec![m],
            replicates: vec![values],
            ..Default::default()
        }
    }

    /// Finite values at X index `i`: the replicates, or the single Y value.
    pub fn values_at(&self, i: usize) -> Vec<f64> {
        match self.replicates.get(i) {
            Some(r) => r.iter().copied().filter(|v| v.is_finite()).collect(),
            None => self
                .y
                .get(i)
                .copied()
                .filter(|v| v.is_finite())
                .into_iter()
                .collect(),
        }
    }

    /// Every finite value of the series (all replicates, or all Y values).
    pub fn all_values(&self) -> Vec<f64> {
        if self.replicates.is_empty() {
            self.y.iter().copied().filter(|v| v.is_finite()).collect()
        } else {
            self.replicates
                .iter()
                .flatten()
                .copied()
                .filter(|v| v.is_finite())
                .collect()
        }
    }
}

/// A curve: a fit line, a survival step curve, an ROC line, with an optional band.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Curve {
    /// Legend label.
    pub name: String,
    /// Dataset whose colour the curve takes (`None`: the foreground colour).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset: Option<usize>,
    /// X.
    pub x: Vec<f64>,
    /// Y.
    pub y: Vec<f64>,
    /// Lower band (empty: no band).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lo: Vec<f64>,
    /// Upper band.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hi: Vec<f64>,
    /// Draw as steps (survival).
    pub step: bool,
    /// Censoring times, drawn as ticks on a step curve at the curve's value there
    /// (survival curves supplied by an analysis).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub censor: Vec<f64>,
}

impl Curve {
    /// The value of a step curve at `t`: the Y of the last point at or before `t` (the
    /// first Y before the curve starts).
    pub fn step_value(&self, t: f64) -> f64 {
        let i = self.x.partition_point(|x| *x <= t);
        if i == 0 {
            self.y.first().copied().unwrap_or(f64::NAN)
        } else {
            self.y.get(i - 1).copied().unwrap_or(f64::NAN)
        }
    }
}

/// Numbers at risk under a survival graph.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AtRiskTable {
    /// Row label.
    pub name: String,
    /// Dataset (colour).
    pub dataset: usize,
    /// Times.
    pub times: Vec<f64>,
    /// Numbers at risk at those times.
    pub counts: Vec<usize>,
}

/// Data extracted from the source for the renderer.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphData {
    /// Series.
    pub series: Vec<Series>,
    /// Fit curves, survival curves, ROC lines, CI bands.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub curves: Vec<Curve>,
    /// At-risk tables (survival).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<AtRiskTable>,
    /// Category names when X is categorical (position i + 1 is `categories[i]`).
    pub categories: Vec<String>,
    /// Subgroup names for `Series::groups`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub group_names: Vec<String>,
    /// Default X title.
    pub x_label: String,
    /// Default Y title.
    pub y_label: String,
    /// Kind of the source table.
    pub kind: TableKind,
}
