//! The `.dioptra` file: a deflate zip of pretty-printed JSON (docs/FILE-FORMAT.md).

use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};

use crate::analysis::Analysis;
use crate::api::{Id, PValueStyle};
use crate::graph::{Graph, Layout};
use crate::history::HistoryEntry;
use crate::project::Project;
use crate::results::ResultsSheet;
use crate::table::Table;

/// Format identifier written by this version. 1.1 (Dioptra 0.3) added connecting lines,
/// orientation and bar options to graph specs and retired the `before_after` family.
pub const FORMAT: &str = "dioptra/1.1";

/// Bring a graph spec read from an older file up to this format: format 1.0 graphs of the
/// retired `before_after` family become the `strip` spec with connecting lines that draws
/// the same picture. Returns whether the spec changed.
pub fn migrate_graph(format: &str, spec: &mut crate::graph::GraphSpec) -> bool {
    let _ = format; // Every 1.x reader may meet a hand-written `before_after` spec.
    spec.upgrade_legacy()
}

/// `manifest.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Format version, `dioptra/1` or `dioptra/1.x`.
    pub format: String,
    /// Writing application version.
    pub app: String,
    /// Creation time.
    pub created: String,
    /// Last save.
    pub modified: String,
}

/// `project.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    /// Title.
    pub title: String,
    /// P-value style.
    pub p_value_style: PValueStyle,
    /// Display order.
    pub order: Vec<Id>,
    /// Layouts.
    #[serde(default)]
    pub layouts: Vec<Layout>,
}

fn pretty<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    let mut s = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    s.push(b'\n');
    Ok(s)
}

/// Serialise a project and its history to `.dioptra` bytes.
pub fn save_bytes(p: &Project, history: &[HistoryEntry]) -> Result<Vec<u8>, String> {
    use zip::write::SimpleFileOptions;
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut put = |name: &str, bytes: &[u8]| -> Result<(), String> {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(bytes).map_err(|e| e.to_string())
    };
    let created = if p.created.is_empty() {
        crate::util::now()
    } else {
        p.created.clone()
    };
    put(
        "manifest.json",
        &pretty(&Manifest {
            format: FORMAT.into(),
            app: env!("CARGO_PKG_VERSION").into(),
            created,
            modified: crate::util::now(),
        })?,
    )?;
    put(
        "project.json",
        &pretty(&ProjectFile {
            title: p.title.clone(),
            p_value_style: p.p_value_style,
            order: p.order.clone(),
            layouts: p.layouts.values().cloned().collect(),
        })?,
    )?;
    for t in p.tables.values() {
        put(&format!("tables/{}.json", t.id), &pretty(t)?)?;
    }
    for a in p.analyses.values() {
        put(&format!("analyses/{}.json", a.id), &pretty(a)?)?;
        if let (Some(r), false) = (&a.results, a.stale) {
            put(&format!("results/{}.json", a.id), &pretty(r)?)?;
        }
    }
    for g in p.graphs.values() {
        put(&format!("graphs/{}.json", g.id), &pretty(g)?)?;
    }
    let mut log = Vec::new();
    for h in history {
        log.extend(serde_json::to_vec(h).map_err(|e| e.to_string())?);
        log.push(b'\n');
    }
    put("history.jsonl", &log)?;
    let out = z.finish().map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Read a project and its history from `.dioptra` bytes.
pub fn open_bytes(bytes: &[u8]) -> Result<(Project, Vec<HistoryEntry>), String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("not a Dioptra file (zip): {e}"))?;
    let mut read = |name: &str| -> Result<Option<String>, String> {
        match z.by_name(name) {
            Ok(mut f) => {
                let mut s = String::new();
                f.read_to_string(&mut s)
                    .map_err(|e| format!("{name}: {e}"))?;
                Ok(Some(s))
            }
            Err(zip::result::ZipError::FileNotFound) => Ok(None),
            Err(e) => Err(format!("{name}: {e}")),
        }
    };
    let manifest: Manifest =
        serde_json::from_str(&read("manifest.json")?.ok_or("manifest.json is missing")?)
            .map_err(|e| format!("manifest.json: {e}"))?;
    if manifest.format != "dioptra/1" && !manifest.format.starts_with("dioptra/1.") {
        return Err(format!(
            "this file is format {}, newer than this version of Dioptra reads ({FORMAT})",
            manifest.format
        ));
    }
    let pf: ProjectFile =
        serde_json::from_str(&read("project.json")?.ok_or("project.json is missing")?)
            .map_err(|e| format!("project.json: {e}"))?;
    let history_text = read("history.jsonl")?.unwrap_or_default();
    let names: Vec<String> = z
        .file_names()
        .filter_map(|n| n.ok().map(|n| n.to_string()))
        .collect();
    let mut p = Project {
        title: pf.title,
        p_value_style: pf.p_value_style,
        created: manifest.created,
        ..Default::default()
    };
    let mut read = |name: &str| -> Result<String, String> {
        let mut f = z.by_name(name).map_err(|e| format!("{name}: {e}"))?;
        let mut s = String::new();
        f.read_to_string(&mut s)
            .map_err(|e| format!("{name}: {e}"))?;
        Ok(s)
    };
    let mut results: std::collections::BTreeMap<String, ResultsSheet> = Default::default();
    for n in &names {
        if let Some(rest) = n
            .strip_prefix("tables/")
            .and_then(|r| r.strip_suffix(".json"))
        {
            let mut t: Table = serde_json::from_str(&read(n)?).map_err(|e| format!("{n}: {e}"))?;
            t.id = rest.to_string();
            t.set_rows(t.rows);
            crate::deps::mark_columns(&mut t, None);
            p.tables.insert(t.id.clone(), t);
        } else if n.starts_with("analyses/") {
            let a: Analysis = serde_json::from_str(&read(n)?).map_err(|e| format!("{n}: {e}"))?;
            p.analyses.insert(a.id.clone(), a);
        } else if n.starts_with("graphs/") {
            let mut g: Graph = serde_json::from_str(&read(n)?).map_err(|e| format!("{n}: {e}"))?;
            migrate_graph(&manifest.format, &mut g.spec);
            p.graphs.insert(g.id.clone(), g);
        } else if let Some(rest) = n
            .strip_prefix("results/")
            .and_then(|r| r.strip_suffix(".json"))
        {
            let r: ResultsSheet =
                serde_json::from_str(&read(n)?).map_err(|e| format!("{n}: {e}"))?;
            results.insert(rest.to_string(), r);
        }
    }
    for a in p.analyses.values_mut() {
        a.results = results.remove(&a.id);
        a.stale = a.results.is_none();
    }
    for l in pf.layouts {
        p.layouts.insert(l.id.clone(), l);
    }
    // Display order: the saved order, then anything it forgot.
    let mut order: Vec<Id> = pf
        .order
        .into_iter()
        .filter(|id| p.kind_of(id).is_some())
        .collect();
    let extra: Vec<Id> = p
        .tables
        .keys()
        .chain(p.analyses.keys())
        .chain(p.graphs.keys())
        .chain(p.layouts.keys())
        .filter(|id| !order.contains(id))
        .cloned()
        .collect();
    order.extend(extra);
    p.order = order;
    // Keep map order the same as display order so saves are stable.
    let order_of: std::collections::HashMap<Id, usize> = p
        .order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    p.tables.sort_by(|a, _, b, _| order_of[a].cmp(&order_of[b]));
    p.analyses
        .sort_by(|a, _, b, _| order_of[a].cmp(&order_of[b]));
    p.graphs.sort_by(|a, _, b, _| order_of[a].cmp(&order_of[b]));
    p.layouts
        .sort_by(|a, _, b, _| order_of[a].cmp(&order_of[b]));
    let mut history = Vec::new();
    for (i, line) in history_text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        history.push(
            serde_json::from_str(line).map_err(|e| format!("history.jsonl line {}: {e}", i + 1))?,
        );
    }
    Ok((p, history))
}

/// Save to a path (desktop only).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_path(p: &Project, history: &[HistoryEntry], path: &str) -> Result<(), String> {
    let bytes = save_bytes(p, history)?;
    // Write next to the target, then rename, so a crash never leaves half a file.
    let tmp = format!("{path}.saving");
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {path}: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Read a file's bytes (desktop only).
#[cfg(not(target_arch = "wasm32"))]
pub fn read_path(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// Write bytes to a path (desktop only).
#[cfg(not(target_arch = "wasm32"))]
pub fn write_path(path: &str, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Paths are not available in the browser build.
#[cfg(target_arch = "wasm32")]
pub fn save_path(_: &Project, _: &[HistoryEntry], _: &str) -> Result<(), String> {
    Err(NO_PATHS.into())
}

/// Paths are not available in the browser build.
#[cfg(target_arch = "wasm32")]
pub fn read_path(_: &str) -> Result<Vec<u8>, String> {
    Err(NO_PATHS.into())
}

/// Paths are not available in the browser build.
#[cfg(target_arch = "wasm32")]
pub fn write_path(_: &str, _: &[u8]) -> Result<(), String> {
    Err(NO_PATHS.into())
}

#[cfg(target_arch = "wasm32")]
const NO_PATHS: &str = "file paths are not available in the browser build; use the *Bytes commands";
