//! `Session::dispatch`: every [`Command`] of the bridge contract.

use std::collections::BTreeSet;

use serde_json::Value as Json;

use crate::analysis::{list_analyses, Analysis, AnalysisEngine, AnalysisSpec};
use crate::api::{
    CellView, Changed, Command, GraphFormat, Id, Range, Response, Session, TableFormat, TableKind,
};
use crate::deps;
use crate::export;
use crate::file;
use crate::formula;
use crate::graph::{self, DioptraRenderer, Graph, GraphSpec, Layout, Renderer};
use crate::history::{capture_cells, Entry, History, HistoryEntry, Snapshot, Touched};
use crate::import::{self, Origin, PasteReport};
use crate::plate;
use crate::project::Project;
use crate::table::{CellEdit, CellValue, Table};
use crate::util::{base64_decode, base64_encode, now};

/// Above this many cells, undo keeps a copy of the whole table instead of cell by cell.
const CELL_SNAPSHOT_LIMIT: usize = 50_000;

type R<T> = Result<T, String>;

impl Default for Session {
    fn default() -> Self {
        Session {
            project: Project::new(),
            history: History::default(),
            renderer: Box::new(DioptraRenderer),
            engine: {
                let mut e = AnalysisEngine::default();
                crate::compute::register_all(&mut e);
                e
            },
            lab: Default::default(),
            memo: Default::default(),
        }
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("project", &self.project.title)
            .field("tables", &self.project.tables.len())
            .field("undo", &self.history.undo.len())
            .field("engine", &self.engine)
            .finish()
    }
}

fn changed(ids: Vec<Id>, tree: bool, created: Option<Id>) -> Response {
    Response::Ok {
        changed: Changed { ids, tree, created },
    }
}

fn file_stem(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "untitled".into()
    } else {
        s
    }
}

/// An untitled project takes the name of the file it is opened from or saved to.
fn name_untitled(p: &mut Project, path: &str) {
    if !(p.title.trim().is_empty() || p.title == "Untitled") {
        return;
    }
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = name.strip_suffix(".dioptra").unwrap_or(name).trim();
    if !stem.is_empty() {
        p.title = stem.to_string();
    }
}

/// The command as JSON for the audit log, with bulky payloads summarised.
fn summarise(cmd: &Command) -> Json {
    let mut j = serde_json::to_value(cmd).unwrap_or(Json::Null);
    if let Some(o) = j.as_object_mut() {
        for key in ["text", "base64"] {
            if let Some(Json::String(s)) = o.get(key) {
                if s.len() > 4096 {
                    let n = s.len();
                    o.insert(key.into(), serde_json::json!({ "omitted_bytes": n }));
                }
            }
        }
        if let Some(Json::Array(v)) = o.get("values") {
            if v.len() > 200 {
                let n = v.len();
                o.insert("values".into(), serde_json::json!({ "omitted_rows": n }));
            }
        }
    }
    j
}

fn clip(t: &Table, r: Range) -> Range {
    let row1 = r.row1.min(t.rows);
    let col1 = r.col1.min(t.col_count());
    Range {
        row0: r.row0.min(row1),
        row1,
        col0: r.col0.min(col1),
        col1,
    }
}

impl Session {
    /// Install a clock (ISO 8601 strings) for timestamps; the wasm facade injects
    /// `Date.now()` here. `None` restores the default.
    pub fn set_clock(&mut self, clock: Option<Box<dyn Fn() -> String>>) {
        crate::util::set_clock(clock);
    }

    /// Replace the graph renderer (the default is [`DioptraRenderer`], the plot engine).
    pub fn set_renderer(&mut self, renderer: Box<dyn Renderer>) {
        self.renderer = renderer;
    }

    /// The analysis computations, to register more kinds.
    pub fn engine_mut(&mut self) -> &mut AnalysisEngine {
        &mut self.engine
    }

    /// The open project.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// A file another component keeps in the project (see `Project::extensions`).
    pub fn extension(&self, name: &str) -> Option<&[u8]> {
        self.project.extensions.get(name).map(Vec::as_slice)
    }

    /// Store such a file; the project then has unsaved changes. Not an undoable edit.
    pub fn set_extension(&mut self, name: &str, bytes: Vec<u8>) {
        self.project.extensions.insert(name.to_string(), bytes);
        self.project.dirty = true;
    }

    /// Session state another component keeps (Dioptra Lab's signed-in user); not saved.
    pub fn memo(&self, name: &str) -> Option<&Json> {
        self.memo.get(name)
    }

    /// Set (`Some`) or clear (`None`) such state.
    pub fn set_memo(&mut self, name: &str, value: Option<Json>) {
        match value {
            Some(v) => self.memo.insert(name.to_string(), v),
            None => self.memo.remove(name),
        };
    }

    /// The command history of the open project, oldest first.
    pub fn history_log(&self) -> &[HistoryEntry] {
        &self.history.log
    }

    /// Bring every table's formula and derived values up to date (as saving does).
    pub fn refresh_tables(&mut self) {
        deps::recompute_all_tables(&mut self.project);
    }

    /// Recompute every table, then every analysis. Returns error messages, if any.
    pub fn recompute_all(&mut self) -> Vec<String> {
        let mut errors = deps::recompute_all(&mut self.project);
        let ids: Vec<Id> = self.project.analyses.keys().cloned().collect();
        for id in ids {
            if let Err(e) = self.results(&id) {
                errors.push(e);
            }
        }
        errors
    }

    /// Run one command.
    pub fn dispatch(&mut self, cmd: Command) -> Response {
        let log = summarise(&cmd);
        match self.run(cmd, log) {
            Ok(r) => r,
            Err(message) => Response::Error { message },
        }
    }

    // ---- plumbing ----------------------------------------------------------

    fn table(&self, id: &str) -> R<&Table> {
        self.project
            .tables
            .get(id)
            .ok_or_else(|| format!("no table with id {id}"))
    }

    fn fresh_table(&mut self, id: &str) -> R<&Table> {
        deps::refresh_table(&mut self.project, id)?;
        self.table(id)
    }

    /// A table that may be edited (derived tables are read-only).
    fn editable(&self, id: &str) -> R<&Table> {
        let t = self.table(id)?;
        if let Some(d) = &t.derived_from {
            let src = self
                .project
                .tables
                .get(&d.table)
                .map(|s| s.title.as_str())
                .unwrap_or("another table");
            return Err(format!(
                "\"{}\" is derived from \"{src}\" by {} and is read-only; edit the source table instead.",
                t.title,
                d.op.name()
            ));
        }
        Ok(t)
    }

    fn tmut(&mut self, id: &str) -> &mut Table {
        self.project.tables.get_mut(id).expect("checked table")
    }

    /// Record a finished mutation.
    fn commit(&mut self, label: String, command: Json, snapshots: Vec<Snapshot>) {
        self.history.coalesce = None;
        self.history.log.push(HistoryEntry {
            at: now(),
            label: label.clone(),
            command,
        });
        self.history.push(Entry { label, snapshots });
        self.project.dirty = true;
    }

    /// Undo a half-done mutation without recording it.
    fn rollback(&mut self, snapshots: Vec<Snapshot>) {
        let mut t = Touched::default();
        History::apply(
            &mut self.project,
            Entry {
                label: String::new(),
                snapshots,
            },
            &mut t,
        );
        for id in t.tables {
            deps::table_changed(&mut self.project, &id, None);
        }
    }

    /// Snapshots for writing into `area` of a table (cell by cell when small, the whole
    /// table when large or when the shape will change).
    fn area_snapshots(
        &self,
        id: &str,
        row0: usize,
        rows: usize,
        col0: usize,
        cols: usize,
    ) -> Vec<Snapshot> {
        let t = &self.project.tables[id];
        let structural = col0 + cols > t.col_count();
        if structural || rows.saturating_mul(cols) > CELL_SNAPSHOT_LIMIT {
            return vec![Snapshot::Table {
                id: id.into(),
                table: Some(Box::new(t.clone())),
            }];
        }
        let r1 = (row0 + rows).min(t.rows);
        let cells = capture_cells(
            t,
            (row0..r1).flat_map(|r| (col0..col0 + cols).map(move |c| (r, c))),
        );
        let mut v = Vec::new();
        if row0 + rows > t.rows {
            v.push(Snapshot::RowCount {
                table: id.into(),
                rows: t.rows,
            });
        }
        v.push(Snapshot::Cells {
            table: id.into(),
            cells,
        });
        v
    }

    fn table_snapshot(&self, id: &str) -> Snapshot {
        Snapshot::Table {
            id: id.into(),
            table: self.project.tables.get(id).cloned().map(Box::new),
        }
    }

    fn new_object_snapshots(&self, id: &str) -> Vec<Snapshot> {
        vec![
            Snapshot::Order(self.project.order.clone()),
            Snapshot::Table {
                id: id.into(),
                table: None,
            },
        ]
    }

    /// After a data change: dirty columns, stale dependents, response.
    fn data_changed(&mut self, id: &str, cols: Option<&BTreeSet<usize>>) -> Vec<Id> {
        deps::table_changed(&mut self.project, id, cols)
    }

    fn results(&mut self, id: &str) -> R<crate::results::ResultsSheet> {
        let a = self
            .project
            .analyses
            .get(id)
            .ok_or_else(|| format!("no analysis with id {id}"))?;
        let table_id = a.table.clone();
        let needs = a.stale || a.results.is_none();
        if !self.project.tables.contains_key(&table_id) {
            return Err("the analysed table no longer exists".into());
        }
        let was_dirty = {
            let t = &self.project.tables[&table_id];
            t.stale || !t.dirty.is_empty()
        };
        deps::refresh_table(&mut self.project, &table_id)?;
        if needs || was_dirty {
            let t = &self.project.tables[&table_id];
            let a = &self.project.analyses[id];
            let mut sheet = self.engine.run(t, &a.spec, self.project.p_value_style);
            sheet.title = a.title.clone();
            let a = self.project.analyses.get_mut(id).expect("analysis");
            a.results = Some(sheet);
            a.stale = false;
        }
        Ok(self.project.analyses[id].results.clone().expect("results"))
    }

    /// The graph material of an analysis, recomputing the analysis when it is stale or
    /// was loaded from a file (graph material is not saved).
    fn analysis_graph(&mut self, id: &str) -> R<Option<graph::AnalysisGraph>> {
        let sheet = self.results(id)?;
        if sheet.graph.is_some() || sheet.message.is_some() {
            return Ok(sheet.graph);
        }
        if let Some(a) = self.project.analyses.get_mut(id) {
            a.stale = true;
        }
        Ok(self.results(id)?.graph)
    }

    fn graph_data(&mut self, spec: &GraphSpec) -> R<graph::GraphData> {
        if self.project.tables.contains_key(&spec.source) {
            let t = self.fresh_table(&spec.source.clone())?;
            return Ok(graph::extract(spec, t));
        }
        let Some(a) = self.project.analyses.get(&spec.source) else {
            return Err("the graph's data source no longer exists".into());
        };
        let table_id = a.table.clone();
        let ag = self.analysis_graph(&spec.source.clone())?;
        let t = self.fresh_table(&table_id)?;
        Ok(graph::extract_analysis(spec, t, ag.as_ref()))
    }

    fn render(&mut self, id: &str) -> R<graph::Rendered> {
        let spec = self
            .project
            .graphs
            .get(id)
            .ok_or_else(|| format!("no graph with id {id}"))?
            .spec
            .clone();
        let data = self.graph_data(&spec)?;
        let r = self.renderer.render(&spec, &data)?;
        if let Some(g) = self.project.graphs.get_mut(id) {
            g.stale = false;
        }
        Ok(r)
    }

    fn import_block(
        &mut self,
        kind: TableKind,
        title: String,
        block: &import::ParsedBlock,
        origin: &Origin<'_>,
        report: &mut PasteReport,
        log: Json,
    ) -> R<Response> {
        if block.cols == 0 {
            return Err("there is nothing to import".into());
        }
        let id = Project::new_id();
        let t = import::table_from_block(id.clone(), kind, title, block, origin, report);
        let snaps = self.new_object_snapshots(&id);
        let label = format!("Import {} × {} as \"{}\"", block.rows, block.cols, t.title);
        self.project.tables.insert(id.clone(), t);
        self.project.order.push(id.clone());
        self.data_changed(&id, None);
        self.commit(label, log, snaps);
        report.changed = Changed {
            ids: vec![id.clone()],
            tree: true,
            created: Some(id),
        };
        Ok(Response::PasteReport(report.clone()))
    }

    fn import_file(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        kind: Option<TableKind>,
        plate_map: Option<&str>,
        log: Json,
    ) -> R<Response> {
        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        let stem = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let stem = stem
            .strip_suffix(&format!(".{ext}"))
            .unwrap_or(stem)
            .to_string();
        let title = self.project.unique_title(&stem);
        if let Some(export) = plate::read(name, &bytes)? {
            let map = plate_map_of(plate_map)?;
            let mut report = PasteReport::default();
            let id = Project::new_id();
            let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
            let t =
                plate::build_table(id.clone(), title, file, &export, map.as_ref(), &mut report)?;
            let snaps = self.new_object_snapshots(&id);
            let label = format!("Import {file} as \"{}\"", t.title);
            self.project.tables.insert(id.clone(), t);
            self.project.order.push(id.clone());
            self.data_changed(&id, None);
            self.commit(label, log, snaps);
            report.changed = Changed {
                ids: vec![id.clone()],
                tree: true,
                created: Some(id),
            };
            return Ok(Response::PasteReport(report));
        }
        match ext.as_str() {
            "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => {
                let (block, mut report, sheet) = import::read_workbook(bytes, None)?;
                let origin = Origin::File {
                    file: name,
                    sheet: Some(&sheet),
                };
                self.import_block(
                    kind.unwrap_or(TableKind::Variables),
                    title,
                    &block,
                    &origin,
                    &mut report,
                    log,
                )
            }
            "pzfx" | "prism" => {
                let (mut tables, notes) = import::read_prism_file(&bytes, Project::new_id)?;
                let mut snaps = vec![Snapshot::Order(self.project.order.clone())];
                let mut ids = Vec::new();
                for t in &mut tables {
                    t.title = self.project.unique_title(&t.title);
                    for c in t.columns_mut() {
                        if let crate::table::ColumnSource::Imported { file, .. } = &mut c.source {
                            *file = name.to_string();
                        }
                    }
                    snaps.push(Snapshot::Table {
                        id: t.id.clone(),
                        table: None,
                    });
                    ids.push(t.id.clone());
                }
                let mut report = PasteReport {
                    notes,
                    ..Default::default()
                };
                for t in tables {
                    report.rows = report.rows.max(t.rows);
                    report.cols += t.col_count();
                    for c in t.columns() {
                        report.numeric_cells += c.values.iter().filter(|v| !v.is_nan()).count();
                        report.text_cells += c.text.len();
                    }
                    let id = t.id.clone();
                    self.project.tables.insert(id.clone(), t);
                    self.project.order.push(id.clone());
                    self.data_changed(&id, None);
                }
                report.notes.insert(
                    0,
                    format!(
                        "Imported {} table{} from {name}.",
                        ids.len(),
                        if ids.len() == 1 { "" } else { "s" }
                    ),
                );
                self.commit(format!("Import {name}"), log, snaps);
                report.changed = Changed {
                    created: ids.first().cloned(),
                    ids,
                    tree: true,
                };
                Ok(Response::PasteReport(report))
            }
            _ => {
                let text = match bytes.as_slice() {
                    [0xFF, 0xFE, rest @ ..] => {
                        let u: Vec<u16> = (0..rest.len() / 2)
                            .map(|i| u16::from_le_bytes([rest[2 * i], rest[2 * i + 1]]))
                            .collect();
                        String::from_utf16_lossy(&u)
                    }
                    _ => String::from_utf8_lossy(&bytes).into_owned(),
                };
                let mut opts = crate::api::PasteOptions::default();
                if ext == "tsv" || ext == "tab" {
                    opts.separator = crate::api::Separator::Tab;
                }
                let (block, mut report) = import::parse_delimited(&text, opts);
                let origin = Origin::File {
                    file: name,
                    sheet: None,
                };
                self.import_block(
                    kind.unwrap_or(TableKind::Variables),
                    title,
                    &block,
                    &origin,
                    &mut report,
                    log,
                )
            }
        }
    }

    fn replace_table_data(
        &mut self,
        id: &str,
        name: &str,
        bytes: Vec<u8>,
        plate_map: Option<&str>,
        log: Json,
    ) -> R<Response> {
        let old = self.editable(id)?;
        let map = plate_map_of(plate_map)?;
        let (new, mut report) = crate::replace::file_table(name, bytes, old.kind, map.as_ref())?;
        let (t, notes) = crate::replace::merge(old, &new)?;
        report.notes.extend(notes);
        let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let label = format!("Replace the data of \"{}\" with {file}", old.title);
        let snaps = vec![self.table_snapshot(id)];
        self.project.tables.insert(id.to_string(), t);
        let ids = self.data_changed(id, None);
        self.commit(label, log, snaps);
        report.changed = Changed {
            ids,
            tree: false,
            created: None,
        };
        Ok(Response::PasteReport(report))
    }

    fn export_table_bytes(
        &mut self,
        id: &str,
        format: TableFormat,
    ) -> R<(Vec<u8>, String, String)> {
        let t = self.fresh_table(id)?;
        let stem = file_stem(&t.title);
        Ok(match format {
            TableFormat::Csv => (
                export::table_to_delimited(t, b',')?.into_bytes(),
                format!("{stem}.csv"),
                "text/csv".into(),
            ),
            TableFormat::Tsv => (
                export::table_to_delimited(t, b'\t')?.into_bytes(),
                format!("{stem}.tsv"),
                "text/tab-separated-values".into(),
            ),
            TableFormat::Xlsx => (
                export::tables_to_xlsx(&[t])?,
                format!("{stem}.xlsx"),
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
            ),
        })
    }

    fn export_graph_bytes(
        &mut self,
        id: &str,
        format: GraphFormat,
        size: Option<crate::api::SizeMm>,
    ) -> R<(Vec<u8>, String, String)> {
        let g = self
            .project
            .graphs
            .get(id)
            .ok_or_else(|| format!("no graph with id {id}"))?;
        let mut spec = g.spec.clone();
        let stem = file_stem(&g.title);
        if let Some(s) = size {
            spec.style.size_mm = s;
        }
        let data = self.graph_data(&spec)?;
        let (bytes, mime, ext) = self.renderer.export(&spec, &data, format)?;
        Ok((bytes, format!("{stem}.{ext}"), mime.into()))
    }

    fn remove_object_snapshots(&self, ids: &[Id]) -> Vec<Snapshot> {
        let p = &self.project;
        let mut snaps = vec![Snapshot::Order(p.order.clone())];
        for id in ids {
            if let Some(t) = p.tables.get(id) {
                snaps.push(Snapshot::Table {
                    id: id.clone(),
                    table: Some(Box::new(t.clone())),
                });
            } else if let Some(a) = p.analyses.get(id) {
                snaps.push(Snapshot::Analysis {
                    id: id.clone(),
                    analysis: Some(Box::new(a.clone())),
                });
            } else if let Some(g) = p.graphs.get(id) {
                snaps.push(Snapshot::Graph {
                    id: id.clone(),
                    graph: Some(Box::new(g.clone())),
                });
            } else if let Some(l) = p.layouts.get(id) {
                snaps.push(Snapshot::Layout {
                    id: id.clone(),
                    layout: Some(l.clone()),
                });
            }
        }
        for l in p.layouts.values() {
            if !ids.contains(&l.id) && l.graphs.iter().any(|g| ids.contains(g)) {
                snaps.push(Snapshot::Layout {
                    id: l.id.clone(),
                    layout: Some(l.clone()),
                });
            }
        }
        snaps
    }

    fn undo_redo(&mut self, redo: bool, log: Json) -> R<Response> {
        let entry = if redo {
            self.history.redo.pop()
        } else {
            self.history.undo.pop()
        };
        let Some(entry) = entry else {
            return Err(if redo {
                "nothing to redo".into()
            } else {
                "nothing to undo".into()
            });
        };
        self.history.coalesce = None;
        let label = entry.label.clone();
        let mut touched = Touched::default();
        let back = History::apply(&mut self.project, entry, &mut touched);
        if redo {
            self.history.undo.push(back);
        } else {
            self.history.redo.push(back);
        }
        let mut ids: Vec<Id> = Vec::new();
        let tables: BTreeSet<Id> = touched.tables.into_iter().collect();
        for id in tables {
            for x in deps::table_changed(&mut self.project, &id, None) {
                if !ids.contains(&x) {
                    ids.push(x);
                }
            }
        }
        for id in touched.objects {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.retain(|i| self.project.kind_of(i).is_some());
        let word = if redo { "Redo" } else { "Undo" };
        self.history.log.push(HistoryEntry {
            at: now(),
            label: format!("{word}: {label}"),
            command: log,
        });
        self.project.dirty = true;
        Ok(changed(ids, touched.tree, None))
    }

    // ---- the commands --------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn run(&mut self, cmd: Command, log: Json) -> R<Response> {
        match cmd {
            Command::Ping => Ok(Response::Pong {
                version: env!("CARGO_PKG_VERSION").into(),
            }),

            // ---- Dioptra Lab ----
            Command::SetLicense { key, today } => {
                Ok(Response::License(self.lab.set_key(key.as_deref(), &today)))
            }
            Command::GetLicense { today } => Ok(Response::License(self.lab.view(&today))),

            // ---- project ----
            Command::NewProject => {
                self.project = Project::new();
                self.history = History::default();
                Ok(changed(vec![], true, None))
            }
            Command::OpenProjectPath { path } => {
                let bytes = file::read_path(&path)?;
                let (mut p, log) = file::open_bytes(&bytes)?;
                name_untitled(&mut p, &path);
                p.path = Some(path);
                self.project = p;
                self.history = History {
                    log,
                    ..Default::default()
                };
                Ok(changed(self.project.order.clone(), true, None))
            }
            Command::OpenProjectBytes { base64, name } => {
                let bytes = base64_decode(&base64)?;
                let (mut p, log) = file::open_bytes(&bytes)?;
                if let Some(n) = name {
                    name_untitled(&mut p, &n);
                }
                self.project = p;
                self.history = History {
                    log,
                    ..Default::default()
                };
                Ok(changed(self.project.order.clone(), true, None))
            }
            Command::SaveProjectPath { path } => {
                deps::recompute_all_tables(&mut self.project);
                name_untitled(&mut self.project, &path);
                file::save_path(&self.project, &self.history.log, &path)?;
                self.project.path = Some(path);
                self.project.dirty = false;
                Ok(changed(vec![], true, None))
            }
            Command::SaveProjectBytes => {
                deps::recompute_all_tables(&mut self.project);
                let bytes = file::save_bytes(&self.project, &self.history.log)?;
                self.project.dirty = false;
                Ok(Response::Bytes {
                    name: format!("{}.dioptra", file_stem(&self.project.title)),
                    mime: "application/zip".into(),
                    base64: base64_encode(&bytes),
                })
            }
            Command::GetProject => Ok(Response::Project(
                self.project
                    .view(!self.history.undo.is_empty(), !self.history.redo.is_empty()),
            )),
            Command::SetPValueStyle { style } => {
                let snaps = vec![Snapshot::Settings {
                    title: self.project.title.clone(),
                    p_value_style: self.project.p_value_style,
                }];
                self.project.p_value_style = style;
                let mut ids = Vec::new();
                for a in self.project.analyses.values_mut() {
                    a.stale = true;
                    ids.push(a.id.clone());
                }
                for g in self.project.graphs.values_mut() {
                    g.stale = true;
                    ids.push(g.id.clone());
                }
                self.commit(format!("Set p-value style to {style:?}"), log, snaps);
                Ok(changed(ids, true, None))
            }
            Command::SetProjectTitle { title } => {
                let title = title.trim().to_string();
                if title.is_empty() {
                    return Err("a project title cannot be empty".to_string());
                }
                let snaps = vec![Snapshot::Settings {
                    title: self.project.title.clone(),
                    p_value_style: self.project.p_value_style,
                }];
                let old = std::mem::replace(&mut self.project.title, title.clone());
                self.commit(
                    format!("Rename the project \"{old}\" to \"{title}\""),
                    log,
                    snaps,
                );
                Ok(changed(Vec::new(), true, None))
            }
            Command::RenameObject { id, title } => {
                let old = self
                    .project
                    .title_of(&id)
                    .ok_or_else(|| format!("no object with id {id}"))?
                    .to_string();
                let snaps = vec![Snapshot::Title {
                    id: id.clone(),
                    title: old.clone(),
                }];
                self.project.set_title(&id, title.clone());
                self.commit(format!("Rename \"{old}\" to \"{title}\""), log, snaps);
                Ok(changed(vec![id], true, None))
            }
            Command::DeleteObject { id } => {
                let title = self
                    .project
                    .title_of(&id)
                    .ok_or_else(|| format!("no object with id {id}"))?
                    .to_string();
                let mut ids = vec![id.clone()];
                ids.extend(deps::all_dependents(&self.project, &id));
                let snaps = self.remove_object_snapshots(&ids);
                for i in &ids {
                    self.project.tables.shift_remove(i);
                    self.project.analyses.shift_remove(i);
                    self.project.graphs.shift_remove(i);
                    self.project.layouts.shift_remove(i);
                }
                for l in self.project.layouts.values_mut() {
                    l.graphs.retain(|g| !ids.contains(g));
                }
                self.project.order.retain(|o| !ids.contains(o));
                let n = ids.len();
                self.commit(
                    if n > 1 {
                        format!(
                            "Delete \"{title}\" and {} derived object{}",
                            n - 1,
                            if n == 2 { "" } else { "s" }
                        )
                    } else {
                        format!("Delete \"{title}\"")
                    },
                    log,
                    snaps,
                );
                Ok(changed(ids, true, None))
            }

            // ---- tables ----
            Command::CreateTable {
                kind,
                title,
                datasets,
                format,
                rows,
            } => {
                let id = Project::new_id();
                let title = if title.trim().is_empty() {
                    let n = self.project.tables.len() + 1;
                    self.project.unique_title(&format!("Table {n}"))
                } else {
                    title
                };
                let snaps = self.new_object_snapshots(&id);
                let t = Table::new(id.clone(), kind, title.clone(), datasets, format, rows);
                self.project.tables.insert(id.clone(), t);
                self.project.order.push(id.clone());
                self.commit(format!("Create table \"{title}\""), log, snaps);
                Ok(changed(vec![id.clone()], true, Some(id)))
            }
            Command::GetTableMeta { table } => {
                Ok(Response::TableMeta(self.fresh_table(&table)?.meta()))
            }
            Command::GetCells { table, range } => {
                let t = self.fresh_table(&table)?;
                let r = clip(t, range);
                let cols: Vec<&crate::table::Column> = (r.col0..r.col1)
                    .map(|c| t.col(c).expect("clipped"))
                    .collect();
                let rows: Vec<Vec<CellView>> = (r.row0..r.row1)
                    .map(|row| cols.iter().map(|c| t.cell_view(row, c)).collect())
                    .collect();
                Ok(Response::Cells { range: r, rows })
            }
            Command::SetCell {
                table,
                row,
                col,
                input,
            } => {
                let t = self.editable(&table)?;
                if t.col(col).is_none() {
                    return Err(format!("column {col} does not exist"));
                }
                let addr = t.address(row, col);
                let title = t.title.clone();
                let mut snaps = Vec::new();
                if row >= t.rows {
                    snaps.push(Snapshot::RowCount {
                        table: table.clone(),
                        rows: t.rows,
                    });
                }
                snaps.push(Snapshot::Cells {
                    table: table.clone(),
                    cells: capture_cells(t, std::iter::once((row, col))),
                });
                snaps.push(Snapshot::EditLog {
                    table: table.clone(),
                    flat: col,
                    edits: t.col(col).expect("col").edits.clone(),
                });
                let tm = self.tmut(&table);
                let before = match tm.set_cell_raw(row, col, &input) {
                    Ok(b) => b,
                    Err(e) => {
                        self.rollback(snaps);
                        return Err(e);
                    }
                };
                let after = tm.col(col).expect("col").cell_value(row);
                if input.trim_start().starts_with('=') {
                    let expr = input.trim_start()[1..].trim();
                    let check = formula::parse(expr)
                        .map_err(|e| e.to_string())
                        .and_then(|_| deps::check(tm));
                    if let Err(e) = check {
                        self.rollback(snaps);
                        return Err(e);
                    }
                }
                if before != after {
                    tm.col_mut(col).expect("col").edits.push(CellEdit {
                        row,
                        at: now(),
                        from: before,
                        to: after,
                    });
                }
                let ids = self.data_changed(&table, Some(&[col].into()));
                self.commit(format!("Type in {addr} of \"{title}\""), log, snaps);
                Ok(changed(ids, false, None))
            }
            Command::SetCells {
                table,
                row0,
                col0,
                values,
            } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let nrows = values.len();
                let ncols = values.iter().map(Vec::len).max().unwrap_or(0);
                let snaps = self.area_snapshots(&table, row0, nrows, col0, ncols);
                let tm = self.tmut(&table);
                let fmt = tm.default_format();
                while tm.col_count() < col0 + ncols {
                    tm.push_dataset(None, fmt);
                }
                let mut cols = BTreeSet::new();
                let mut err = None;
                'outer: for (i, row) in values.iter().enumerate() {
                    for (j, v) in row.iter().enumerate() {
                        if let Err(e) = tm.set_cell_raw(row0 + i, col0 + j, v) {
                            err = Some(e);
                            break 'outer;
                        }
                        cols.insert(col0 + j);
                    }
                }
                if err.is_none()
                    && values
                        .iter()
                        .flatten()
                        .any(|v| v.trim_start().starts_with('='))
                {
                    err = deps::check(tm).err();
                }
                if let Some(e) = err {
                    self.rollback(snaps);
                    return Err(e);
                }
                let ids = self.data_changed(&table, Some(&cols));
                self.commit(
                    format!("Fill {nrows} × {ncols} cells in \"{title}\""),
                    log,
                    snaps,
                );
                Ok(changed(ids, ncols > 0, None))
            }
            Command::Paste {
                table,
                row0,
                col0,
                text,
                options,
            } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let (block, mut report) = import::parse_delimited(&text, options);
                if block.rows == 0 && block.header.is_none() {
                    report.notes.push("There was nothing to paste.".into());
                    return Ok(Response::PasteReport(report));
                }
                let snaps = self.area_snapshots(&table, row0, block.rows, col0, block.cols);
                let at = now();
                let cols_before = self.table(&table)?.col_count();
                let written =
                    import::paste_into(self.tmut(&table), row0, col0, &block, &at, &mut report);
                let tree = self.table(&table)?.col_count() != cols_before;
                let ids = self.data_changed(&table, Some(&written));
                self.commit(
                    format!("Paste {} × {} into \"{title}\"", block.rows, block.cols),
                    log,
                    snaps,
                );
                report.changed = Changed {
                    ids,
                    tree,
                    created: None,
                };
                Ok(Response::PasteReport(report))
            }
            Command::ClearCells { table, range } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let r = clip(t, range);
                let snaps =
                    self.area_snapshots(&table, r.row0, r.row1 - r.row0, r.col0, r.col1 - r.col0);
                let tm = self.tmut(&table);
                let mut cols = BTreeSet::new();
                for c in r.col0..r.col1 {
                    let col = tm.col_mut(c).expect("clipped");
                    if col.formula.is_some() {
                        continue;
                    }
                    for row in r.row0..r.row1 {
                        col.clear_cell(row);
                    }
                    cols.insert(c);
                }
                let ids = self.data_changed(&table, Some(&cols));
                self.commit(
                    format!(
                        "Clear {} × {} cells in \"{title}\"",
                        r.row1 - r.row0,
                        r.col1 - r.col0
                    ),
                    log,
                    snaps,
                );
                Ok(changed(ids, false, None))
            }
            Command::ExcludeCells {
                table,
                range,
                excluded,
            } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let r = clip(t, range);
                let snaps =
                    self.area_snapshots(&table, r.row0, r.row1 - r.row0, r.col0, r.col1 - r.col0);
                let tm = self.tmut(&table);
                let mut cols = BTreeSet::new();
                for c in r.col0..r.col1 {
                    let col = tm.col_mut(c).expect("clipped");
                    for row in r.row0..r.row1 {
                        if excluded {
                            col.excluded.insert(row);
                        } else {
                            col.excluded.remove(&row);
                        }
                    }
                    cols.insert(c);
                }
                let ids = self.data_changed(&table, Some(&cols));
                let word = if excluded { "Exclude" } else { "Include" };
                self.commit(
                    format!(
                        "{word} {} × {} cells in \"{title}\"",
                        r.row1 - r.row0,
                        r.col1 - r.col0
                    ),
                    log,
                    snaps,
                );
                Ok(changed(ids, false, None))
            }
            Command::FillDown { table, range } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let r = clip(t, range);
                if r.row1 <= r.row0 + 1 {
                    return Ok(changed(vec![], false, None));
                }
                let snaps =
                    self.area_snapshots(&table, r.row0, r.row1 - r.row0, r.col0, r.col1 - r.col0);
                let tm = self.tmut(&table);
                let mut cols = BTreeSet::new();
                for c in r.col0..r.col1 {
                    let col = tm.col(c).expect("clipped");
                    if col.formula.is_some() {
                        continue;
                    }
                    let first = col.cell_value(r.row0);
                    let parsed = match &first {
                        CellValue::Text(s) if s.starts_with('=') => formula::parse(s).ok(),
                        _ => None,
                    };
                    for row in r.row0 + 1..r.row1 {
                        let v = match &parsed {
                            Some(e) => CellValue::Text(format!(
                                "={}",
                                e.shift_rows((row - r.row0) as isize)
                            )),
                            None => first.clone(),
                        };
                        tm.put_cell_value(row, c, &v);
                    }
                    cols.insert(c);
                }
                if let Err(e) = deps::check(self.table(&table)?) {
                    self.rollback(snaps);
                    return Err(e);
                }
                let ids = self.data_changed(&table, Some(&cols));
                self.commit(
                    format!("Fill down {} rows in \"{title}\"", r.row1 - r.row0),
                    log,
                    snaps,
                );
                Ok(changed(ids, false, None))
            }
            Command::InsertRows { table, at, count } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let snaps = vec![self.table_snapshot(&table)];
                self.tmut(&table).insert_rows(at, count);
                let ids = self.data_changed(&table, None);
                self.commit(
                    format!(
                        "Insert {count} row{} in \"{title}\"",
                        if count == 1 { "" } else { "s" }
                    ),
                    log,
                    snaps,
                );
                Ok(changed(ids, false, None))
            }
            Command::DeleteRows { table, at, count } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let snaps = vec![self.table_snapshot(&table)];
                self.tmut(&table).delete_rows(at, count);
                let ids = self.data_changed(&table, None);
                self.commit(
                    format!(
                        "Delete {count} row{} in \"{title}\"",
                        if count == 1 { "" } else { "s" }
                    ),
                    log,
                    snaps,
                );
                Ok(changed(ids, false, None))
            }
            Command::AddDataset { table, name } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let fmt = t.default_format();
                let snaps = vec![self.table_snapshot(&table)];
                let i = self.tmut(&table).push_dataset(name, fmt);
                let ids = self.data_changed(&table, None);
                self.commit(
                    format!("Add data set {} to \"{title}\"", crate::util::letters(i)),
                    log,
                    snaps,
                );
                Ok(changed(ids, true, None))
            }
            Command::RemoveDataset { table, dataset } => {
                let t = self.editable(&table)?;
                if dataset >= t.datasets.len() {
                    return Err(format!("there is no data set {dataset}"));
                }
                let title = t.title.clone();
                let name = t.datasets[dataset].name.clone();
                let snaps = vec![self.table_snapshot(&table)];
                self.tmut(&table).datasets.remove(dataset);
                let ids = self.data_changed(&table, None);
                self.commit(
                    format!("Remove data set \"{name}\" from \"{title}\""),
                    log,
                    snaps,
                );
                Ok(changed(ids, true, None))
            }
            Command::SetDatasetName {
                table,
                dataset,
                name,
            } => {
                let t = self.editable(&table)?;
                if dataset >= t.datasets.len() {
                    return Err(format!("there is no data set {dataset}"));
                }
                let snaps = vec![self.table_snapshot(&table)];
                let old =
                    std::mem::replace(&mut self.tmut(&table).datasets[dataset].name, name.clone());
                // Formulas that name the dataset in backticks follow it only if rewritten;
                // recompute so a broken reference shows an error rather than stale values.
                let ids = self.data_changed(&table, None);
                self.commit(
                    format!("Rename data set \"{old}\" to \"{name}\""),
                    log,
                    snaps,
                );
                Ok(changed(ids, true, None))
            }
            Command::SetSubcolumnFormat { table, format } => {
                let t = self.editable(&table)?;
                let title = t.title.clone();
                let snaps = vec![self.table_snapshot(&table)];
                self.tmut(&table).set_format(format);
                let ids = self.data_changed(&table, None);
                self.commit(
                    format!("Change subcolumn format of \"{title}\""),
                    log,
                    snaps,
                );
                Ok(changed(ids, true, None))
            }
            Command::SetColumnFormula { table, col, expr } => {
                let t = self.editable(&table)?;
                let c = t
                    .col(col)
                    .ok_or_else(|| format!("column {col} does not exist"))?;
                let label = t.col_label(col);
                let snaps = vec![Snapshot::Column {
                    table: table.clone(),
                    flat: col,
                    column: Box::new(c.clone()),
                }];
                let tm = self.tmut(&table);
                let c = tm.col_mut(col).expect("col");
                match &expr {
                    Some(e) => {
                        let e = e.trim().trim_start_matches('=').trim().to_string();
                        if let Err(pe) = formula::parse(&e) {
                            self.rollback(snaps);
                            return Err(pe.to_string());
                        }
                        c.formula = Some(e.clone());
                        c.cell_formulas.clear();
                        c.source = crate::table::ColumnSource::Formula { expr: e };
                    }
                    None => {
                        c.formula = None;
                        c.formula_error = None;
                        c.errors.clear();
                    }
                }
                if let Err(e) = deps::check(tm) {
                    self.rollback(snaps);
                    return Err(e);
                }
                let ids = self.data_changed(&table, Some(&[col].into()));
                let what = match &expr {
                    Some(e) => format!("Set formula of column {label} to ={e}"),
                    None => format!("Clear the formula of column {label}"),
                };
                self.commit(what, log, snaps);
                Ok(changed(ids, true, None))
            }
            Command::SetColumnDecimals {
                table,
                col,
                decimals,
            } => {
                let t = self.table(&table)?;
                let c = t
                    .col(col)
                    .ok_or_else(|| format!("column {col} does not exist"))?;
                let snaps = vec![Snapshot::Decimals {
                    table: table.clone(),
                    flat: col,
                    decimals: c.decimals,
                }];
                let label = t.col_label(col);
                self.tmut(&table).col_mut(col).expect("col").decimals = decimals;
                self.commit(format!("Set decimals of column {label}"), log, snaps);
                Ok(changed(vec![table], false, None))
            }
            Command::SetRowTitle { table, row, title } => {
                let t = self.editable(&table)?;
                let mut snaps = Vec::new();
                if row >= t.rows {
                    snaps.push(Snapshot::RowCount {
                        table: table.clone(),
                        rows: t.rows,
                    });
                }
                snaps.push(Snapshot::RowTitles {
                    table: table.clone(),
                    titles: t.row_titles.clone(),
                });
                self.tmut(&table).set_row_title(row, title);
                let ids = self.data_changed(&table, Some(&BTreeSet::new()));
                self.commit(format!("Set row title {}", row + 1), log, snaps);
                Ok(changed(ids, false, None))
            }
            Command::SetVariableType {
                table,
                dataset,
                var_type,
            } => {
                let t = self.editable(&table)?;
                if t.kind != TableKind::Variables {
                    return Err("variable types belong to multiple-variables tables".into());
                }
                if dataset >= t.datasets.len() {
                    return Err(format!("there is no variable {dataset}"));
                }
                let snaps = vec![self.table_snapshot(&table)];
                let name = t.datasets[dataset].name.clone();
                self.tmut(&table).datasets[dataset].var_type = Some(var_type);
                let ids = self.data_changed(&table, None);
                self.commit(
                    format!("Set type of \"{name}\" to {var_type:?}"),
                    log,
                    snaps,
                );
                Ok(changed(ids, true, None))
            }
            Command::Transform { table, op } => {
                let src = self.fresh_table(&table)?;
                let mut t = crate::transform::apply(src, &op)?;
                let id = Project::new_id();
                t.id = id.clone();
                t.title = self.project.unique_title(&t.title);
                t.derived_from = Some(crate::table::DerivedFrom {
                    table: table.clone(),
                    op: op.clone(),
                });
                let snaps = self.new_object_snapshots(&id);
                let label = format!("{} \"{}\"", op.name(), self.table(&table)?.title);
                self.project.tables.insert(id.clone(), t);
                self.project.order.push(id.clone());
                self.commit(label, log, snaps);
                Ok(changed(vec![id.clone()], true, Some(id)))
            }
            Command::GetCellInfo { table, row, col } => {
                let t = self.fresh_table(&table)?;
                t.cell_info(row, col)
                    .map(Response::CellInfo)
                    .ok_or_else(|| format!("column {col} does not exist"))
            }

            // ---- import / export ----
            Command::ImportText {
                text,
                kind,
                title,
                options,
            } => {
                let (block, mut report) = import::parse_delimited(&text, options);
                let n = self.project.tables.len() + 1;
                let title = title
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| self.project.unique_title(&format!("Table {n}")));
                let at = now();
                self.import_block(kind, title, &block, &Origin::Pasted(&at), &mut report, log)
            }
            Command::ImportPath {
                path,
                kind,
                plate_map,
            } => {
                let bytes = file::read_path(&path)?;
                self.import_file(&path, bytes, kind, plate_map.as_deref(), log)
            }
            Command::ImportBytes {
                name,
                base64,
                kind,
                plate_map,
            } => {
                let bytes = base64_decode(&base64)?;
                self.import_file(&name, bytes, kind, plate_map.as_deref(), log)
            }
            Command::InspectPlate { name, base64, path } => {
                let bytes = match (base64, path) {
                    (Some(b), _) => base64_decode(&b)?,
                    (None, Some(p)) => file::read_path(&p)?,
                    (None, None) => return Err("give the file's bytes or path".into()),
                };
                Ok(Response::Plate {
                    plate: plate::read(&name, &bytes)?.map(|e| e.summary()),
                })
            }
            Command::ReplaceTableData {
                table,
                name,
                base64,
                plate_map,
            } => {
                let bytes = base64_decode(&base64)?;
                self.replace_table_data(&table, &name, bytes, plate_map.as_deref(), log)
            }
            Command::ExportTable { table, format } => {
                let (bytes, name, mime) = self.export_table_bytes(&table, format)?;
                Ok(Response::Bytes {
                    name,
                    mime,
                    base64: base64_encode(&bytes),
                })
            }
            Command::ExportTablePath {
                table,
                format,
                path,
            } => {
                let (bytes, _, _) = self.export_table_bytes(&table, format)?;
                file::write_path(&path, &bytes)?;
                Ok(changed(vec![], false, None))
            }

            // ---- analyses ----
            Command::ListAnalyses { table, range } => {
                deps::refresh_table(&mut self.project, &table)?;
                Ok(Response::Analyses {
                    analyses: list_analyses(self.table(&table)?, range, &self.engine),
                })
            }
            Command::DefaultAnalysisSpec { table, kind } => {
                let t = self.table(&table)?;
                AnalysisSpec::default_for(&kind, t)
                    .map(Response::AnalysisSpec)
                    .ok_or_else(|| format!("unknown analysis kind \"{kind}\""))
            }
            Command::CreateAnalysis { table, spec } => {
                let t = self.table(&table)?;
                let id = Project::new_id();
                let title = self
                    .project
                    .unique_title(&format!("{} of {}", spec.name(), t.title));
                let snaps = vec![
                    Snapshot::Order(self.project.order.clone()),
                    Snapshot::Analysis {
                        id: id.clone(),
                        analysis: None,
                    },
                ];
                self.project.analyses.insert(
                    id.clone(),
                    Analysis {
                        id: id.clone(),
                        title: title.clone(),
                        table,
                        spec,
                        results: None,
                        stale: true,
                    },
                );
                self.project.order.push(id.clone());
                self.commit(format!("Create analysis \"{title}\""), log, snaps);
                Ok(changed(vec![id.clone()], true, Some(id)))
            }
            Command::UpdateAnalysis { id, spec } => {
                let a = self
                    .project
                    .analyses
                    .get(&id)
                    .ok_or_else(|| format!("no analysis with id {id}"))?;
                let snaps = vec![Snapshot::Analysis {
                    id: id.clone(),
                    analysis: Some(Box::new(a.clone())),
                }];
                let title = a.title.clone();
                let a = self.project.analyses.get_mut(&id).expect("analysis");
                a.spec = spec;
                a.stale = true;
                let mut ids = vec![id.clone()];
                ids.extend(deps::all_dependents(&self.project, &id));
                for g in &ids {
                    if let Some(g) = self.project.graphs.get_mut(g) {
                        g.stale = true;
                    }
                }
                self.commit(format!("Change options of \"{title}\""), log, snaps);
                Ok(changed(ids, false, None))
            }
            Command::GetAnalysisSpec { id } => {
                let a = self
                    .project
                    .analyses
                    .get(&id)
                    .ok_or_else(|| format!("no analysis with id {id}"))?;
                Ok(Response::AnalysisSpec(a.spec.clone()))
            }
            Command::GetResults { id } => Ok(Response::Results(self.results(&id)?)),
            Command::ExportResults { id, format } => {
                let sheet = self.results(&id)?;
                let style = self.project.p_value_style;
                let text = match format.to_ascii_lowercase().as_str() {
                    "tsv" => export::results_to_tsv(&sheet, style),
                    "markdown" | "md" => export::results_to_markdown(&sheet, style),
                    other => {
                        return Err(format!(
                            "unknown results format \"{other}\": use tsv or markdown"
                        ))
                    }
                };
                Ok(Response::Text { text })
            }

            // ---- graphs ----
            Command::CreateGraph {
                source,
                family,
                title,
            } => {
                let src_title = self
                    .project
                    .title_of(&source)
                    .ok_or_else(|| format!("no table or analysis with id {source}"))?
                    .to_string();
                if !(self.project.tables.contains_key(&source)
                    || self.project.analyses.contains_key(&source))
                {
                    return Err("a graph's source must be a table or an analysis".into());
                }
                let id = Project::new_id();
                let title = title
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| self.project.unique_title(&format!("Graph of {src_title}")));
                let snaps = vec![
                    Snapshot::Order(self.project.order.clone()),
                    Snapshot::Graph {
                        id: id.clone(),
                        graph: None,
                    },
                ];
                let spec = if let Some(a) = self.project.analyses.get(&source) {
                    // Computed afresh, not cached: creating a graph does not change the
                    // analysis (its stale flag and cached sheet stay as they were).
                    let (table_id, aspec) = (a.table.clone(), a.spec.clone());
                    let style = self.project.p_value_style;
                    deps::refresh_table(&mut self.project, &table_id)?;
                    let t = self.table(&table_id)?;
                    let ag = self.engine.run(t, &aspec, style).graph;
                    crate::compute::graph_spec_for_analysis(
                        source,
                        &family,
                        ag.as_ref(),
                        self.project.p_value_style,
                    )
                } else {
                    let family = if family.trim().is_empty() || family == "auto" {
                        crate::compute::default_table_family(self.table(&source)?.kind).to_string()
                    } else {
                        family
                    };
                    GraphSpec::new(source, &family)
                };
                self.project.graphs.insert(
                    id.clone(),
                    Graph {
                        id: id.clone(),
                        title: title.clone(),
                        spec,
                        stale: true,
                    },
                );
                self.project.order.push(id.clone());
                self.commit(format!("Create graph \"{title}\""), log, snaps);
                Ok(changed(vec![id.clone()], true, Some(id)))
            }
            Command::GetGraphSpec { id } => {
                let g = self
                    .project
                    .graphs
                    .get(&id)
                    .ok_or_else(|| format!("no graph with id {id}"))?;
                Ok(Response::GraphSpec(Box::new(g.spec.clone())))
            }
            Command::UpdateGraph { id, spec } => self.set_graph_spec(id, spec, log),
            Command::PatchGraph {
                id,
                patch,
                coalesce,
            } => {
                let g = self
                    .project
                    .graphs
                    .get(&id)
                    .ok_or_else(|| format!("no graph with id {id}"))?;
                let mut v = serde_json::to_value(&g.spec).map_err(|e| e.to_string())?;
                merge_patch(&mut v, &patch);
                let spec: GraphSpec = serde_json::from_value(v)
                    .map_err(|e| format!("the patched graph is not valid: {e}"))?;
                if !coalesce {
                    return self.set_graph_spec(id, spec, log);
                }
                let mut paths = Vec::new();
                patch_paths(&patch, String::new(), &mut paths);
                paths.sort();
                let key = format!("{id} {}", paths.join(" "));
                if self.history.coalesce.as_deref() == Some(key.as_str())
                    && !self.history.undo.is_empty()
                {
                    // The open undo step already holds the graph as it was before this
                    // run of edits: apply without a new step.
                    self.check_graph_source(&spec)?;
                    let g = self.project.graphs.get_mut(&id).expect("graph");
                    g.spec = spec;
                    g.stale = true;
                    self.history.redo.clear();
                    if let Some(last) = self.history.log.last_mut() {
                        last.at = now();
                        last.command = log;
                    }
                    self.project.dirty = true;
                    return Ok(changed(vec![id], false, None));
                }
                let r = self.set_graph_spec(id, spec, log)?;
                self.history.coalesce = Some(key);
                Ok(r)
            }
            Command::RenderGraph { id } => {
                let r = self.render(&id)?;
                Ok(Response::Svg {
                    svg: r.svg,
                    width_mm: r.width_mm,
                    height_mm: r.height_mm,
                    targets: r.targets,
                })
            }
            Command::ExportGraph { id, format, size } => {
                let (bytes, name, mime) = self.export_graph_bytes(&id, format, size)?;
                Ok(Response::Bytes {
                    name,
                    mime,
                    base64: base64_encode(&bytes),
                })
            }
            Command::ExportGraphPath {
                id,
                format,
                size,
                path,
            } => {
                let (bytes, _, _) = self.export_graph_bytes(&id, format, size)?;
                file::write_path(&path, &bytes)?;
                Ok(changed(vec![], false, None))
            }

            // ---- layouts ----
            Command::CreateLayout {
                title,
                graphs,
                columns,
            } => {
                for g in &graphs {
                    if !self.project.graphs.contains_key(g) {
                        return Err(format!("no graph with id {g}"));
                    }
                }
                let id = Project::new_id();
                let n = self.project.layouts.len() + 1;
                let title = title
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| self.project.unique_title(&format!("Layout {n}")));
                let snaps = vec![
                    Snapshot::Order(self.project.order.clone()),
                    Snapshot::Layout {
                        id: id.clone(),
                        layout: None,
                    },
                ];
                self.project.layouts.insert(
                    id.clone(),
                    Layout {
                        id: id.clone(),
                        title: title.clone(),
                        graphs,
                        columns: columns.max(1),
                        style: Default::default(),
                    },
                );
                self.project.order.push(id.clone());
                self.commit(format!("Create layout \"{title}\""), log, snaps);
                Ok(changed(vec![id.clone()], true, Some(id)))
            }
            Command::GetLayout { id } => {
                let l = self
                    .project
                    .layouts
                    .get(&id)
                    .ok_or_else(|| format!("no layout with id {id}"))?;
                Ok(Response::Layout {
                    id: l.id.clone(),
                    title: l.title.clone(),
                    graphs: l.graphs.clone(),
                    columns: l.columns,
                    style: l.style.clone(),
                })
            }
            Command::UpdateLayout {
                id,
                graphs,
                columns,
                style,
            } => {
                let l = self
                    .project
                    .layouts
                    .get(&id)
                    .ok_or_else(|| format!("no layout with id {id}"))?;
                for g in &graphs {
                    if !self.project.graphs.contains_key(g) {
                        return Err(format!("no graph with id {g}"));
                    }
                }
                if let Some(st) = &style {
                    st.validate()?;
                    // Final-size layouts are a Dioptra Lab feature; going back to the
                    // default needs no key, and files keep drawing as saved either way.
                    if *st != l.style && !st.is_default() {
                        self.lab.require("Journal figures")?;
                    }
                }
                let snaps = vec![Snapshot::Layout {
                    id: id.clone(),
                    layout: Some(l.clone()),
                }];
                let title = l.title.clone();
                let l = self.project.layouts.get_mut(&id).expect("layout");
                l.graphs = graphs;
                l.columns = columns.max(1);
                if let Some(st) = style {
                    l.style = st;
                }
                self.commit(format!("Edit layout \"{title}\""), log, snaps);
                Ok(changed(vec![id], false, None))
            }
            Command::ExportLayoutPath { id, format, path } => {
                let (panels, columns, letters) = self.layout_panels(&id)?;
                let (bytes, _, _) = self
                    .renderer
                    .export_layout(&panels, columns, &letters, format)?;
                file::write_path(&path, &bytes)?;
                Ok(changed(vec![], false, None))
            }
            Command::RenderLayout { id } => {
                let (panels, columns, letters) = self.layout_panels(&id)?;
                let r = self.renderer.render_layout(&panels, columns, &letters)?;
                Ok(Response::Svg {
                    svg: r.svg,
                    width_mm: r.width_mm,
                    height_mm: r.height_mm,
                    targets: r.targets,
                })
            }
            Command::ExportLayout { id, format } => {
                let (panels, columns, letters) = self.layout_panels(&id)?;
                let (bytes, mime, ext) = self
                    .renderer
                    .export_layout(&panels, columns, &letters, format)?;
                Ok(Response::Bytes {
                    name: format!("{}.{ext}", file_stem(&self.project.layouts[&id].title)),
                    mime: mime.into(),
                    base64: base64_encode(&bytes),
                })
            }

            // ---- history ----
            Command::Undo => self.undo_redo(false, log),
            Command::Redo => self.undo_redo(true, log),
            Command::GetHistory { limit } => Ok(Response::History {
                entries: self.history.log.iter().rev().take(limit).cloned().collect(),
            }),
            Command::RecomputeAll => {
                let errors = self.recompute_all();
                let ids: Vec<Id> = self.project.order.clone();
                if errors.is_empty() {
                    Ok(changed(ids, false, None))
                } else {
                    Err(format!("Recomputed, with problems: {}", errors.join("; ")))
                }
            }
        }
    }

    fn check_graph_source(&self, spec: &GraphSpec) -> R<()> {
        if self.project.tables.contains_key(&spec.source)
            || self.project.analyses.contains_key(&spec.source)
        {
            Ok(())
        } else {
            Err("a graph's source must be a table or an analysis".into())
        }
    }

    fn set_graph_spec(&mut self, id: Id, spec: GraphSpec, log: Json) -> R<Response> {
        self.check_graph_source(&spec)?;
        let g = self
            .project
            .graphs
            .get(&id)
            .ok_or_else(|| format!("no graph with id {id}"))?;
        let snaps = vec![Snapshot::Graph {
            id: id.clone(),
            graph: Some(Box::new(g.clone())),
        }];
        let title = g.title.clone();
        let g = self.project.graphs.get_mut(&id).expect("graph");
        g.spec = spec;
        g.stale = true;
        self.commit(format!("Edit graph \"{title}\""), log, snaps);
        Ok(changed(vec![id], false, None))
    }

    /// Specs (with the layout's style applied) and data of a layout's graphs, in reading
    /// order, its column count and its panel letters.
    #[allow(clippy::type_complexity)]
    fn layout_panels(
        &mut self,
        id: &str,
    ) -> R<(
        Vec<(GraphSpec, graph::GraphData)>,
        usize,
        graph::PanelLetters,
    )> {
        let l = self
            .project
            .layouts
            .get(id)
            .ok_or_else(|| format!("no layout with id {id}"))?
            .clone();
        let mut panels = Vec::new();
        for g in &l.graphs {
            let spec = self
                .project
                .graphs
                .get(g)
                .ok_or_else(|| format!("no graph with id {g}"))?
                .spec
                .clone();
            let data = self.graph_data(&spec)?;
            if let Some(gr) = self.project.graphs.get_mut(g) {
                gr.stale = false;
            }
            panels.push((spec, data));
        }
        let mut specs: Vec<GraphSpec> = panels.iter().map(|(s, _)| s.clone()).collect();
        l.style.apply(&mut specs, l.columns);
        for ((s, _), styled) in panels.iter_mut().zip(specs) {
            *s = styled;
        }
        Ok((panels, l.columns, l.style.letters()))
    }

    /// The scene a graph exports to, at its own size or `size`: for checks that must see
    /// exactly what is written (Dioptra Lab's figure check).
    pub fn graph_scene(
        &mut self,
        id: &str,
        size: Option<crate::api::SizeMm>,
    ) -> R<dioptra_plot::Scene> {
        let mut spec = self
            .project
            .graphs
            .get(id)
            .ok_or_else(|| format!("no graph with id {id}"))?
            .spec
            .clone();
        if let Some(s) = size {
            spec.style.size_mm = s;
        }
        let data = self.graph_data(&spec)?;
        dioptra_plot::scene(&spec, &data).map_err(|e| e.to_string())
    }

    /// The scene a layout exports to (its style applied).
    pub fn layout_scene(&mut self, id: &str) -> R<dioptra_plot::Scene> {
        let (panels, columns, letters) = self.layout_panels(id)?;
        graph::layout_scene(&panels, columns, &letters)
    }

    /// Run several commands as one undo step labelled `label`. When one fails, the ones
    /// before it are taken back and the error is returned; nothing enters the undo stack.
    /// The audit log keeps every command.
    pub fn batch(&mut self, label: &str, cmds: Vec<Command>) -> R<Vec<Response>> {
        let saved = std::mem::take(&mut self.history.undo);
        let redo = self.history.redo.clone();
        let mut out = Vec::with_capacity(cmds.len());
        let mut failed = None;
        for cmd in cmds {
            match self.dispatch(cmd) {
                Response::Error { message } => {
                    failed = Some(message);
                    break;
                }
                r => out.push(r),
            }
        }
        let steps = std::mem::replace(&mut self.history.undo, saved);
        if let Some(message) = failed {
            for e in steps.into_iter().rev() {
                self.rollback(e.snapshots);
            }
            self.history.redo = redo;
            return Err(message);
        }
        if !steps.is_empty() {
            let snapshots = steps.into_iter().flat_map(|e| e.snapshots).collect();
            self.history.push(Entry {
                label: label.to_string(),
                snapshots,
            });
            self.history.coalesce = None;
        }
        Ok(out)
    }
}

/// The JSON pointers of the leaves a merge patch sets or removes.
fn patch_paths(patch: &Json, prefix: String, out: &mut Vec<String>) {
    match patch {
        Json::Object(o) if !o.is_empty() => {
            for (k, v) in o {
                patch_paths(v, format!("{prefix}/{k}"), out);
            }
        }
        _ => out.push(prefix),
    }
}

/// RFC 7396 JSON merge patch.
pub fn merge_patch(target: &mut Json, patch: &Json) {
    match patch {
        Json::Object(p) => {
            if !target.is_object() {
                *target = Json::Object(Default::default());
            }
            let t = target.as_object_mut().expect("object");
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge_patch(t.entry(k.clone()).or_insert(Json::Null), v);
                }
            }
        }
        other => *target = other.clone(),
    }
}

fn plate_map_of(text: Option<&str>) -> R<Option<plate::PlateMap>> {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => Ok(Some(
            plate::parse_plate_map(t).map_err(|e| format!("plate map: {e}"))?,
        )),
        None => Ok(None),
    }
}
