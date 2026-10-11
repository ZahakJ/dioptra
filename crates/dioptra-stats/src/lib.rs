//! Dioptra's statistical engine. Pure functions over `f64` slices; no I/O.
//!
//! Every public function names its reference in `docs/METHODS.md` and is tested against
//! `fixtures/oracle/*.json` and `fixtures/nist/*.json`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

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
