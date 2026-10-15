// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Dioptra Lab. Every command here needs a license key (see docs/LAB-TIER.md and
//! LICENSE-LAB); the free core never depends on this crate.
//!
//! The shells call [`dispatch_json`] instead of `Session::dispatch_json`: commands whose
//! `type` starts with `lab_` run here, everything else goes to the core session unchanged.
//! Lab code drives the session through the same JSON commands the UI uses.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use dioptra_core::api::{Response, Session};
use serde::{Deserialize, Serialize};

/// A Lab request from the UI or the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LabCommand {
    /// Which Lab features this build contains.
    LabFeatures,
}

/// A Lab reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LabResponse {
    /// Reply to `LabFeatures`.
    LabFeatures {
        /// Feature names.
        features: Vec<String>,
    },
}

/// JSON in, JSON out: Lab commands here, the rest to the session.
pub fn dispatch_json(session: &mut Session, json: &str) -> String {
    let is_lab = serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("type")?.as_str().map(|t| t.starts_with("lab_")))
        .unwrap_or(false);
    if !is_lab {
        return session.dispatch_json(json);
    }
    match serde_json::from_str::<LabCommand>(json) {
        Ok(cmd) => match dispatch(session, cmd) {
            Ok(r) => serde_json::to_string(&r).expect("lab response serialises"),
            Err(message) => error(message),
        },
        Err(e) => error(format!("bad command: {e}")),
    }
}

fn error(message: String) -> String {
    serde_json::to_string(&Response::Error { message }).expect("error serialises")
}

/// Run one Lab command.
pub fn dispatch(session: &mut Session, cmd: LabCommand) -> Result<LabResponse, String> {
    let _ = session;
    match cmd {
        // Listing the features needs no key: the UI uses it to mark Lab items.
        LabCommand::LabFeatures => Ok(LabResponse::LabFeatures {
            features: Vec::new(),
        }),
    }
}
