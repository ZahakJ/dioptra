//! Dioptra's statistical engine. Pure functions over `f64` slices; no I/O.
//!
//! Every public function names its reference in `docs/METHODS.md` and is tested against
//! `fixtures/oracle/*.json` and `fixtures/nist/*.json`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// `!(x > 0.0)` is used deliberately throughout: unlike `x <= 0.0` it is also true for NaN,
// so invalid parameters are rejected in one comparison.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod agreement;
pub mod anova;
pub mod contingency;
pub mod correlation;
pub mod descriptive;
pub mod dist;
pub mod equations;
mod expr;
mod lilliefors_table;
mod linalg;
mod lm;
pub mod mannwhitney;
pub mod multivariate;
pub mod nonlinear;
pub mod normality;
pub mod outliers;
pub mod posthoc;
pub mod quad;
pub mod rank;
pub mod regression;
pub mod resample;
pub mod roc;
pub mod roots;
pub mod special;
pub mod survival;
pub mod ttest;
pub mod wilcoxon;

/// Error type for the engine.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum StatsError {
    /// Too few observations for the requested procedure.
    #[error("not enough data: {0}")]
    NotEnoughData(String),
    /// A parameter or option was invalid.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// An iterative procedure did not converge.
    #[error("did not converge: {0}")]
    NoConvergence(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, StatsError>;
