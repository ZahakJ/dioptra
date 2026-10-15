//! Dioptra's document model. See DESIGN.md §4 and docs/FILE-FORMAT.md.
//!
//! The UI talks to [`api::Session`] through one JSON command at a time
//! (`Session::dispatch_json`). Inside: [`table`] holds the data, [`formula`] computes
//! columns, [`deps`] keeps them up to date, [`transform`] derives tables, [`import`] and
//! [`export`] move data in and out, [`analysis`] and [`results`] produce results sheets,
//! [`graph`] describes figures, [`project`], [`file`] and [`history`] hold the document,
//! its file and its undo stack.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod analysis;
pub mod api;
pub mod compute;
pub mod deps;
pub mod export;
pub mod file;
pub mod formula;
pub mod graph;
pub mod history;
pub mod import;
pub mod lab;
pub mod numerics;
pub mod project;
pub mod results;
mod session;
pub mod table;
pub mod transform;
pub mod util;

pub use session::merge_patch;
