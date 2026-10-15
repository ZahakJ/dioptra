// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Dioptra Lab. Every command here needs a license key (see docs/LAB-TIER.md and
//! LICENSE-LAB); the free core never depends on this crate.
//!
//! The shells call [`dispatch_json`] instead of `Session::dispatch_json`: commands whose
//! `type` starts with `lab_` run here, everything else goes to the core session unchanged.
//! Lab code drives the session through the same JSON commands the UI uses.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod batch;
pub mod pzfx;
pub mod watch;

pub mod check;
pub mod journal;
#[cfg(test)]
mod journal_testkit;
pub mod journal_tiff;
pub mod methods;
#[cfg(not(target_arch = "wasm32"))]
pub mod oq;
pub mod report;
pub mod sign;
pub mod users;

use dioptra_core::api::{Response, Session};
use serde::{Deserialize, Serialize};

/// A Lab request from the UI or the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LabCommand {
    /// Which Lab features this build contains.
    LabFeatures,
    /// The journal presets; needs no key. Replies `LabJournalPresets`.
    LabJournalPresets,
    /// Apply a journal preset to graphs and layouts in one undo step. Replies `LabChanged`.
    LabApplyJournalPreset {
        /// Preset id (`nature`, `cell`, …).
        preset: String,
        /// Column width.
        width: journal::Width,
        /// `"all"` or a list of graph and layout ids.
        #[serde(default)]
        targets: journal::Targets,
    },
    /// Check a graph or layout as exported. Replies `LabFigureCheck`.
    LabCheckFigure {
        /// Graph or layout id.
        id: String,
        /// Journal preset id (general rules when absent).
        #[serde(default)]
        preset: Option<String>,
        /// Raster export to check the resolution of.
        #[serde(default)]
        raster: Option<check::RasterExport>,
    },
    /// Export a graph or layout as an LZW-compressed TIFF. Replies `LabFile`.
    LabExportTiff {
        /// Graph or layout id.
        id: String,
        /// Dots per inch.
        dpi: f64,
        /// Size override for a graph.
        #[serde(default)]
        size: Option<dioptra_core::api::SizeMm>,
    },
    /// The methods paragraph for the project or some analyses. Replies `LabMethods`.
    LabMethods {
        /// `"project"` or a list of analysis (or graph) ids.
        #[serde(default)]
        scope: journal::Targets,
    },
    /// A report of the project. Replies `LabFile`.
    LabReport {
        /// `docx` or `pdf`.
        format: report::ReportFormat,
        /// Rows of each data table (default 20).
        #[serde(default)]
        rows: Option<usize>,
    },
    /// Write data tables as a Prism `.pzfx` file; replies `LabFile`.
    LabExportPzfx {
        /// Table ids; all tables when empty.
        #[serde(default)]
        tables: Vec<String>,
    },
    /// Apply a template project to data files; replies `LabBatch`. See [`batch`].
    LabBatchRun {
        /// The template project (`.dioptra`), base64.
        template_base64: String,
        /// The table that receives each file's data (title or id).
        table: String,
        /// Data files.
        inputs: Vec<batch::BatchInput>,
        /// Figure format, plate map, what to write.
        #[serde(default)]
        options: batch::BatchOptions,
    },
    /// The data tables of a template; replies `LabTemplateInfo`.
    LabTemplateInfo {
        /// The template project (`.dioptra`), base64.
        template_base64: String,
    },
    /// The open project as a batch template (`.dioptra` bytes) without saving it; replies
    /// `LabFile`.
    LabTemplate,
    /// Run the automated OQ (see [`oq`]), all at once or step `step` of it. Replies `LabOq`.
    LabRunOq {
        /// One step (0 starts a run); absent runs every step.
        #[serde(default)]
        step: Option<usize>,
    },
    /// The last OQ report as `pdf` or `csv`. Replies `LabFile`; with `path` (desktop) the file
    /// is written there and `base64` is empty. With `password`, the signed-in user signs the
    /// PDF as executed.
    LabOqReport {
        /// `pdf` or `csv`.
        format: String,
        /// The signed-in user's password, to sign the PDF.
        #[serde(default)]
        password: Option<String>,
        /// Where to write it.
        #[serde(default)]
        path: Option<String>,
    },
    /// Pack files into one zip; replies `LabFile`.
    LabZip {
        /// Zip file name.
        name: String,
        /// Files.
        files: Vec<batch::OutputFile>,
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
    /// Reply to `LabJournalPresets`.
    LabJournalPresets {
        /// Presets in menu order (see [`journal::JournalPreset`]).
        presets: serde_json::Value,
    },
    /// Objects a Lab command changed.
    LabChanged {
        /// Ids.
        ids: Vec<String>,
    },
    /// Reply to `LabCheckFigure`.
    LabFigureCheck(check::FigureCheck),
    /// Reply to `LabMethods`.
    LabMethods(methods::Methods),
    /// Reply to `LabTemplateInfo`.
    LabTemplateInfo(batch::TemplateInfo),
    /// Reply to `LabBatchRun`.
    LabBatch {
        /// One item per input, in order.
        items: Vec<batch::BatchItem>,
        /// One row per input.
        summary: batch::Summary,
    },
    /// Reply to `LabRunOq`.
    LabOq {
        /// Steps done.
        step: usize,
        /// Steps in a run.
        steps: usize,
        /// Title of the next step, while the run goes on.
        #[serde(default)]
        next: Option<String>,
        /// The report, when the run is complete (`oq::OqReport`).
        #[serde(default)]
        report: Option<serde_json::Value>,
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
    // Records for regulated labs: audit, lock and sign (see audit.rs).
    if let Some(out) = audit::intercept(session, json) {
        return out;
    }
    dispatch_plain(session, json)
}

/// `dispatch_json` without the records hook.
pub(crate) fn dispatch_plain(session: &mut Session, json: &str) -> String {
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
            features: vec![
                "prism_export".into(),
                "regulated_records".into(),
                "journal_figures".into(),
                "batch".into(),
            ],
        }),
        LabCommand::LabBatchRun {
            template_base64,
            table,
            inputs,
            options,
        } => {
            session.require_lab("Batch analysis")?;
            let template = dioptra_core::util::base64_decode(&template_base64)?;
            let items: Vec<batch::BatchItem> = inputs
                .iter()
                .map(|i| match dioptra_core::util::base64_decode(&i.base64) {
                    Ok(data) => batch::run_one(&template, &table, &i.name, &data, &options),
                    Err(e) => batch::BatchItem {
                        name: i.name.clone(),
                        error: Some(e),
                        ..Default::default()
                    },
                })
                .collect();
            let summary = batch::summarise(&items);
            Ok(LabResponse::LabBatch { items, summary })
        }
        LabCommand::LabTemplateInfo { template_base64 } => {
            session.require_lab("Batch analysis")?;
            let bytes = dioptra_core::util::base64_decode(&template_base64)?;
            Ok(LabResponse::LabTemplateInfo(batch::template_info(&bytes)?))
        }
        LabCommand::LabTemplate => {
            session.require_lab("Batch analysis")?;
            let p = session.project();
            let bytes = dioptra_core::file::save_bytes(p, &[])?;
            Ok(LabResponse::LabFile {
                name: format!("{}.dioptra", batch::file_stem(&format!("{}.x", p.title))),
                mime: "application/zip".into(),
                base64: base64_encode(&bytes),
                notes: Vec::new(),
            })
        }
        LabCommand::LabZip { name, files } => {
            session.require_lab("Batch analysis")?;
            let files: Vec<(String, Vec<u8>)> =
                files.into_iter().map(|f| (f.name, f.base64)).collect();
            Ok(LabResponse::LabFile {
                name,
                mime: "application/zip".into(),
                base64: base64_encode(&batch::zip(&files)?),
                notes: Vec::new(),
            })
        }
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
        LabCommand::LabJournalPresets => Ok(LabResponse::LabJournalPresets {
            presets: serde_json::to_value(journal::presets()).expect("presets serialise"),
        }),
        LabCommand::LabApplyJournalPreset {
            preset,
            width,
            targets,
        } => journal::apply(session, &preset, width, &targets)
            .map(|ids| LabResponse::LabChanged { ids }),
        LabCommand::LabCheckFigure { id, preset, raster } => {
            journal::check(session, &id, preset.as_deref(), raster).map(LabResponse::LabFigureCheck)
        }
        LabCommand::LabExportTiff { id, dpi, size } => {
            journal::export_tiff(session, &id, dpi, size).map(|(name, bytes)| {
                LabResponse::LabFile {
                    name,
                    mime: "image/tiff".into(),
                    base64: base64_encode(&bytes),
                    notes: Vec::new(),
                }
            })
        }
        LabCommand::LabMethods { scope } => {
            methods::methods(session, &scope).map(LabResponse::LabMethods)
        }
        #[cfg(not(target_arch = "wasm32"))]
        LabCommand::LabRunOq { step } => oq::lab_run_oq(session, step),
        #[cfg(not(target_arch = "wasm32"))]
        LabCommand::LabOqReport {
            format,
            password,
            path,
        } => oq::lab_oq_report(session, &format, password.as_deref(), path),
        #[cfg(target_arch = "wasm32")]
        LabCommand::LabRunOq { .. } | LabCommand::LabOqReport { .. } => {
            Err("The OQ runs in the desktop app and the command-line tool.".into())
        }
        LabCommand::LabReport { format, rows } => {
            report::report(session, format, rows).map(|(name, mime, bytes)| LabResponse::LabFile {
                name,
                mime,
                base64: base64_encode(&bytes),
                notes: Vec::new(),
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
