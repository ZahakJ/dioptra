// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Dioptra Lab. Every command here needs a license key (see docs/LAB-TIER.md and
//! LICENSE-LAB); the free core never depends on this crate.
//!
//! The shells call [`dispatch_json`] instead of `Session::dispatch_json`: commands whose
//! `type` starts with `lab_` run here, everything else goes to the core session unchanged.
//! Lab code drives the session through the same JSON commands the UI uses.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod pzfx;

use dioptra_core::api::{Response, Session};
use serde::{Deserialize, Serialize};

/// A Lab request from the UI or the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LabCommand {
    /// Which Lab features this build contains.
    LabFeatures,
    /// Write data tables as a Prism `.pzfx` file; replies `LabFile`.
    LabExportPzfx {
        /// Table ids; all tables when empty.
        #[serde(default)]
        tables: Vec<String>,
    },
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
    /// A file to save, with notes on anything approximated.
    LabFile {
        /// Suggested file name.
        name: String,
        /// MIME type.
        mime: String,
        /// Base64 content.
        base64: String,
        /// What could not be written exactly.
        notes: Vec<String>,
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
    match cmd {
        // Listing the features needs no key: the UI uses it to mark Lab items.
        LabCommand::LabFeatures => Ok(LabResponse::LabFeatures {
            features: vec!["prism_export".into()],
        }),
        LabCommand::LabExportPzfx { tables } => {
            session.require_lab("Prism export")?;
            session.dispatch(dioptra_core::api::Command::RecomputeAll);
            let p = session.project();
            let chosen: Vec<&dioptra_core::table::Table> = if tables.is_empty() {
                p.tables.values().collect()
            } else {
                tables
                    .iter()
                    .map(|id| {
                        p.tables
                            .get(id)
                            .ok_or_else(|| format!("no table with id {id}"))
                    })
                    .collect::<Result<_, _>>()?
            };
            if chosen.is_empty() {
                return Err("The project has no tables to export.".into());
            }
            let w = pzfx::write_pzfx(&chosen, env!("CARGO_PKG_VERSION"));
            let stem = p.title.trim();
            Ok(LabResponse::LabFile {
                name: format!("{}.pzfx", if stem.is_empty() { "Untitled" } else { stem }),
                mime: "application/xml".into(),
                base64: base64_encode(w.xml.as_bytes()),
                notes: w.notes,
            })
        }
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn license(s: &mut Session, today: &str) {
        s.dispatch(dioptra_core::api::Command::SetLicense {
            key: Some(dioptra_license::TEST_KEY.into()),
            today: today.into(),
        });
    }

    #[test]
    fn pzfx_export_needs_lab() {
        let mut s = Session::new();
        s.dispatch_json(
            r#"{"type":"create_table","kind":"column","title":"Plasma","datasets":2,"format":{"kind":"values","n":1},"rows":3}"#,
        );
        let cmd = r#"{"type":"lab_export_pzfx"}"#;
        assert!(dispatch_json(&mut s, cmd).contains("part of Dioptra Lab"));
        license(&mut s, "2026-01-15");
        let r: serde_json::Value = serde_json::from_str(&dispatch_json(&mut s, cmd)).unwrap();
        assert_eq!(r["type"], "lab_file", "{r}");
        assert!(r["name"].as_str().unwrap().ends_with(".pzfx"));
        license(&mut s, "2026-10-14");
        assert!(dispatch_json(&mut s, cmd).contains("part of Dioptra Lab"));
    }

    #[test]
    fn other_commands_reach_the_core() {
        let mut s = Session::new();
        assert!(dispatch_json(&mut s, r#"{"type":"ping"}"#).contains("pong"));
    }
}
