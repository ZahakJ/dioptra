// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Batch analysis: a template project applied to many data files.
//!
//! A template is any `.dioptra` project plus the name (or id) of the table that receives the
//! data. For each input file a fresh session opens the template, puts the file's data into that
//! table with `ReplaceTableData`, recomputes everything, and writes the project, every graph and
//! layout (at their own size) and the headline numbers of every analysis.
//!
//! **Headline numbers.** In each section of an analysis' results sheet, the value columns are
//! those headed `Value`, `Best-fit` or `Estimate`, and every column holding p values. A row's
//! label is the text of its cells left of the first value column (`Data set A · LogEC50`,
//! `t`, `Cohen's d`). Each number in a value column becomes one summary column named
//! `<analysis> · <label>`; the column heading is appended for p-value columns other than the
//! main value column, and the section title is added when a name would repeat. So a nonlinear
//! fit gives its parameters (`Bottom`, `Top`, `LogEC50`, `HillSlope`) and derived values
//! (`EC50`), a t test its t, df, P value, difference and effect sizes.

use dioptra_core::api::{Command, GraphFormat, Response, Session};
use dioptra_core::project::ItemKind;
use dioptra_core::results::{ResultCell, ResultsSheet};
use serde::{Deserialize, Serialize};

/// Figure format of the batch outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureFormat {
    /// PDF with embedded fonts.
    #[default]
    Pdf,
    /// PNG at `dpi`.
    Png,
    /// SVG.
    Svg,
}

/// Batch options. Every field is optional in JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BatchOptions {
    /// Figure format.
    pub format: FigureFormat,
    /// Resolution for PNG.
    pub dpi: f64,
    /// Plate map applied to plate-reader inputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plate_map: Option<String>,
    /// Write the recomputed project of each input.
    pub project: bool,
    /// Render graphs and layouts.
    pub figures: bool,
}

impl Default for BatchOptions {
    fn default() -> Self {
        BatchOptions {
            format: FigureFormat::Pdf,
            dpi: 300.0,
            plate_map: None,
            project: true,
            figures: true,
        }
    }
}

/// One input file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInput {
    /// File name; its extension and content decide the format.
    pub name: String,
    /// Base64 content.
    pub base64: String,
}

/// A file produced for one input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputFile {
    /// File name.
    pub name: String,
    /// MIME type.
    pub mime: String,
    /// Content (base64 in JSON).
    #[serde(with = "b64")]
    pub base64: Vec<u8>,
}

/// One headline number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Headline {
    /// Summary column name.
    pub name: String,
    /// Value (`null` in JSON when not finite).
    pub value: Option<f64>,
    /// Whether it is a p value.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub p: bool,
}

/// What happened to one input.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BatchItem {
    /// Input file name.
    pub name: String,
    /// Why the input could not be analysed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Notes from the import and the analyses.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Headline numbers of every analysis.
    #[serde(default)]
    pub headlines: Vec<Headline>,
    /// Output files.
    #[serde(default)]
    pub files: Vec<OutputFile>,
    /// Paths written (desktop and CLI, which write the files themselves).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub written: Vec<String>,
}

/// The summary: one row per input, one column per headline number.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Summary {
    /// `File`, the headline names, `Error`.
    pub columns: Vec<String>,
    /// Rows: the file name, numbers (or `null`), the error (or `null`).
    pub rows: Vec<Vec<serde_json::Value>>,
}

mod b64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&dioptra_core::util::base64_encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        dioptra_core::util::base64_decode(&s).map_err(serde::de::Error::custom)
    }
}

/// What a template holds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TemplateInfo {
    /// Project title.
    pub title: String,
    /// Tables that can receive data (not derived), in project order.
    pub tables: Vec<String>,
    /// Number of analyses.
    pub analyses: usize,
    /// Number of graphs and layouts.
    pub figures: usize,
}

/// Read a template's tables.
pub fn template_info(template: &[u8]) -> Result<TemplateInfo, String> {
    let (p, _) = dioptra_core::file::open_bytes(template)
        .map_err(|e| format!("the template does not open: {e}"))?;
    Ok(TemplateInfo {
        title: p.title.clone(),
        tables: p
            .tables
            .values()
            .filter(|t| t.derived_from.is_none())
            .map(|t| t.title.clone())
            .collect(),
        analyses: p.analyses.len(),
        figures: p.graphs.len() + p.layouts.len(),
    })
}

/// A name safe for every file system.
pub fn file_stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = match base.rfind('.') {
        Some(i) if i > 0 => &base[..i],
        _ => base,
    };
    let s: String = stem
        .chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "untitled".into()
    } else {
        s
    }
}

fn call(s: &mut Session, cmd: Command) -> Result<Response, String> {
    match s.dispatch(cmd) {
        Response::Error { message } => Err(message),
        r => Ok(r),
    }
}

fn is_value_heading(h: &str) -> bool {
    matches!(
        h.trim().to_ascii_lowercase().as_str(),
        "value" | "best-fit" | "estimate"
    )
}

/// The headline numbers of one results sheet (see the module docs).
pub fn headlines(analysis: &str, sheet: &ResultsSheet) -> Vec<Headline> {
    let mut out: Vec<Headline> = Vec::new();
    for sec in &sheet.sections {
        let t = &sec.table;
        let ncols = t.columns.len();
        let mut value_cols: Vec<usize> = (0..ncols)
            .filter(|&c| is_value_heading(&t.columns[c]))
            .collect();
        let main = value_cols.first().copied();
        for c in 0..ncols {
            let has_p = t
                .rows
                .iter()
                .any(|r| matches!(r.get(c), Some(ResultCell::PValue { .. })));
            if has_p && !value_cols.contains(&c) {
                value_cols.push(c);
            }
        }
        value_cols.sort_unstable();
        let Some(&first) = value_cols.first() else {
            continue;
        };
        for row in &t.rows {
            let label = row[..first.min(row.len())]
                .iter()
                .filter_map(|c| match c {
                    ResultCell::Text(s) if !s.trim().is_empty() => Some(s.trim()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" · ");
            for &c in &value_cols {
                let (v, p) = match row.get(c) {
                    Some(ResultCell::Number { v, .. }) => (*v, false),
                    Some(ResultCell::PValue { p }) => (*p, true),
                    _ => continue,
                };
                let mut parts = vec![analysis.to_string()];
                if !label.is_empty() {
                    parts.push(label.clone());
                }
                if Some(c) != main || label.is_empty() {
                    parts.push(t.columns[c].trim().to_string());
                }
                let mut name = parts.join(" · ");
                if out.iter().any(|h| h.name == name) {
                    name = format!("{name} ({})", sec.title);
                }
                out.push(Headline {
                    name,
                    value: v.is_finite().then_some(v),
                    p,
                });
            }
        }
    }
    out
}

fn figure_format(opts: &BatchOptions) -> GraphFormat {
    match opts.format {
        FigureFormat::Pdf => GraphFormat::Pdf,
        FigureFormat::Svg => GraphFormat::Svg,
        FigureFormat::Png => GraphFormat::Png { dpi: opts.dpi },
    }
}

/// Apply the template to one input. Never fails: errors are reported in the item.
pub fn run_one(
    template: &[u8],
    table: &str,
    name: &str,
    data: &[u8],
    opts: &BatchOptions,
) -> BatchItem {
    let mut item = BatchItem {
        name: name.to_string(),
        ..Default::default()
    };
    if let Err(e) = run_into(&mut item, template, table, name, data, opts) {
        item.error = Some(e);
    }
    item
}

fn run_into(
    item: &mut BatchItem,
    template: &[u8],
    table: &str,
    name: &str,
    data: &[u8],
    opts: &BatchOptions,
) -> Result<(), String> {
    let mut s = Session::new();
    call(
        &mut s,
        Command::OpenProjectBytes {
            base64: dioptra_core::util::base64_encode(template),
            name: None,
        },
    )
    .map_err(|e| format!("the template does not open: {e}"))?;
    let Response::Project(view) = call(&mut s, Command::GetProject)? else {
        return Err("unexpected reply to get_project".into());
    };
    let tables: Vec<_> = view
        .items
        .iter()
        .filter(|i| i.kind == ItemKind::Table)
        .collect();
    let target = tables
        .iter()
        .find(|i| i.id == table || i.title == table)
        .or_else(|| {
            let loose: Vec<_> = tables
                .iter()
                .filter(|i| i.title.eq_ignore_ascii_case(table.trim()))
                .collect();
            (loose.len() == 1).then(|| loose[0])
        })
        .ok_or_else(|| format!("the template has no table called \"{table}\""))?
        .id
        .clone();
    let r = call(
        &mut s,
        Command::ReplaceTableData {
            table: target,
            name: name.to_string(),
            base64: dioptra_core::util::base64_encode(data),
            plate_map: opts.plate_map.clone(),
        },
    )?;
    if let Response::PasteReport(rep) = r {
        item.notes.extend(rep.notes);
    }
    call(&mut s, Command::RecomputeAll)?;
    let stem = file_stem(name);
    for it in &view.items {
        match it.kind {
            ItemKind::Analysis => match call(&mut s, Command::GetResults { id: it.id.clone() }) {
                Ok(Response::Results(sheet)) => {
                    if let Some(m) = &sheet.message {
                        item.notes.push(format!("{}: {m}", it.title));
                    }
                    item.headlines.extend(headlines(&it.title, &sheet));
                }
                Ok(_) => {}
                Err(e) => item.notes.push(format!("{}: {e}", it.title)),
            },
            ItemKind::Graph | ItemKind::Layout if opts.figures => {
                let format = figure_format(opts);
                let cmd = if it.kind == ItemKind::Graph {
                    Command::ExportGraph {
                        id: it.id.clone(),
                        format,
                        size: None,
                    }
                } else {
                    Command::ExportLayout {
                        id: it.id.clone(),
                        format,
                    }
                };
                match call(&mut s, cmd) {
                    Ok(Response::Bytes {
                        name: n,
                        mime,
                        base64,
                    }) => {
                        let ext = n.rsplit('.').next().unwrap_or("pdf");
                        item.files.push(OutputFile {
                            name: format!(
                                "{stem} - {}.{ext}",
                                file_stem(&format!("{}.x", it.title))
                            ),
                            mime,
                            base64: dioptra_core::util::base64_decode(&base64)?,
                        });
                    }
                    Ok(_) => {}
                    Err(e) => item.notes.push(format!("{}: {e}", it.title)),
                }
            }
            _ => {}
        }
    }
    if opts.project {
        if let Response::Bytes { mime, base64, .. } = call(&mut s, Command::SaveProjectBytes)? {
            item.files.insert(
                0,
                OutputFile {
                    name: format!("{stem}.dioptra"),
                    mime,
                    base64: dioptra_core::util::base64_decode(&base64)?,
                },
            );
        }
    }
    Ok(())
}

/// The summary table of a batch.
pub fn summarise(items: &[BatchItem]) -> Summary {
    let mut names: Vec<String> = Vec::new();
    for it in items {
        for h in &it.headlines {
            if !names.contains(&h.name) {
                names.push(h.name.clone());
            }
        }
    }
    let mut columns = vec!["File".to_string()];
    columns.extend(names.iter().cloned());
    columns.push("Error".into());
    let rows = items
        .iter()
        .map(|it| {
            let mut row = vec![serde_json::Value::from(it.name.clone())];
            for n in &names {
                row.push(
                    it.headlines
                        .iter()
                        .find(|h| &h.name == n)
                        .and_then(|h| h.value)
                        .map_or(serde_json::Value::Null, serde_json::Value::from),
                );
            }
            row.push(
                it.error
                    .clone()
                    .map_or(serde_json::Value::Null, serde_json::Value::from),
            );
            row
        })
        .collect();
    Summary { columns, rows }
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The summary as CSV (numbers at full precision).
pub fn summary_csv(s: &Summary) -> String {
    let mut out = String::new();
    let line = |cells: Vec<String>| cells.join(",") + "\r\n";
    out.push_str(&line(s.columns.iter().map(|c| csv_field(c)).collect()));
    for r in &s.rows {
        out.push_str(&line(
            r.iter()
                .map(|v| match v {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(t) => csv_field(t),
                    other => other.to_string(),
                })
                .collect(),
        ));
    }
    out
}

/// Pack files into one zip.
pub fn zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut seen = std::collections::BTreeSet::new();
    for (name, bytes) in files {
        if !seen.insert(name.clone()) {
            continue;
        }
        w.start_file(name.as_str(), opts)
            .map_err(|e| e.to_string())?;
        w.write_all(bytes).map_err(|e| e.to_string())?;
    }
    Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
}
