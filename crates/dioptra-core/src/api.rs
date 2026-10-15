//! The bridge contract between the engine and the UI.
//!
//! One JSON `Command` in, one JSON `Response` out. The Tauri app and the wasm build both
//! expose exactly `Session::dispatch_json`. Large data never crosses in one piece: the grid
//! asks for viewports (`GetCells`), paste sends raw text once, graphs come back as SVG.
//!
//! Identifiers (`Id`) are opaque strings (UUID v4) owned by the engine. Ranges are
//! half-open on rows and columns, zero-based. "Column index" means the flat index over all
//! datasets and subcolumns of a table, left to right, with the X column (if any) first;
//! `TableMeta.columns` gives the mapping back to dataset/subcolumn.
//!
//! File paths are only meaningful in the desktop build; the wasm build answers
//! `*Path` commands with an error and the UI uses the `*Bytes` variants.

use serde::{Deserialize, Serialize};

/// Opaque object identifier.
pub type Id = String;

/// Table kinds (DESIGN §4.1). Defined in `dioptra-plot` (graphs need it too) and
/// re-exported here unchanged.
pub use dioptra_plot::data::TableKind;

/// How the subcolumns of a dataset are to be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "n")]
pub enum SubcolumnFormat {
    /// `n` replicate values.
    Values(u16),
    /// Mean, SD, N.
    MeanSdN,
    /// Mean, SEM, N.
    MeanSemN,
    /// Mean, 95% CI half-width... stored as lower, upper, N.
    MeanCiN,
    /// Mean, SD.
    MeanSd,
    /// Mean, SEM.
    MeanSem,
    /// Median, lower quartile, upper quartile.
    MedianIqr,
    /// Lower, upper.
    LowerUpper,
    /// A single value.
    Single,
}

/// Declared type of a `variables` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableType {
    /// Continuous number.
    Number,
    /// Categorical (text labels or small integers).
    Category,
    /// Free text.
    Text,
    /// ISO date.
    Date,
}

/// Decimal and separator hints for pasting. Every field is optional in JSON (`{}` = auto).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct PasteOptions {
    /// Decimal mark: auto-detect, point or comma.
    pub decimal: DecimalMark,
    /// Field separator: auto-detect, tab, comma, semicolon or whitespace.
    pub separator: Separator,
    /// Whether the first row is a header (auto if `None`).
    pub header: Option<bool>,
}

/// Decimal mark choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecimalMark {
    /// Decide from the text.
    #[default]
    Auto,
    /// `.`
    Point,
    /// `,`
    Comma,
}

/// Field separator choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Separator {
    /// Decide from the text.
    #[default]
    Auto,
    /// Tab.
    Tab,
    /// Comma.
    Comma,
    /// Semicolon.
    Semicolon,
    /// Runs of spaces.
    Whitespace,
}

/// A rectangular cell range, half-open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// First row.
    pub row0: usize,
    /// One past the last row.
    pub row1: usize,
    /// First flat column index.
    pub col0: usize,
    /// One past the last flat column index.
    pub col1: usize,
}

/// Export formats for tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableFormat {
    /// Comma separated.
    Csv,
    /// Tab separated.
    Tsv,
    /// Excel workbook.
    Xlsx,
}

/// Graph export formats.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "format")]
pub enum GraphFormat {
    /// Scalable vector graphics.
    Svg,
    /// PDF with embedded fonts.
    Pdf,
    /// Raster at the given dots per inch.
    Png {
        /// Dots per inch.
        dpi: f64,
    },
}

/// Physical size in millimetres (defined in `dioptra-plot`, re-exported).
pub use dioptra_plot::spec::SizeMm;

/// P-value display style (DESIGN §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PValueStyle {
    /// APA: p = .005, p < .001.
    #[default]
    Apa,
    /// NEJM: p = 0.005, p < 0.001, three digits.
    Nejm,
    /// GraphPad asterisks: ns, *, **, ***, ****.
    Gp,
    /// Exact: p = 0.0049, four significant digits.
    Exact,
}

/// A request from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Engine identity and version.
    Ping,

    // ---- project ----
    /// Discard everything and start an empty project.
    NewProject,
    /// Open a `.dioptra` file from disk (desktop only).
    OpenProjectPath {
        /// Absolute path.
        path: String,
    },
    /// Open a `.dioptra` file from bytes (base64).
    OpenProjectBytes {
        /// Base64 of the zip.
        base64: String,
        /// Display name.
        name: Option<String>,
    },
    /// Save to disk (desktop only).
    SaveProjectPath {
        /// Absolute path.
        path: String,
    },
    /// Serialise the project; returns `Response::Bytes`.
    SaveProjectBytes,
    /// The project tree: tables, analyses, graphs, layouts, dirty flag, path, p-value style.
    GetProject,
    /// Set project-wide p-value style.
    SetPValueStyle {
        /// Style.
        style: PValueStyle,
    },
    /// Rename the project (its title; the file keeps its path until saved elsewhere).
    SetProjectTitle {
        /// New title.
        title: String,
    },
    /// Rename any object.
    RenameObject {
        /// Object id.
        id: Id,
        /// New title.
        title: String,
    },
    /// Delete any object and everything derived from it.
    DeleteObject {
        /// Object id.
        id: Id,
    },

    // ---- tables ----
    /// Create an empty table.
    CreateTable {
        /// Kind.
        kind: TableKind,
        /// Title.
        title: String,
        /// Number of datasets (Y columns / groups / variables).
        datasets: usize,
        /// Subcolumn format for each dataset.
        format: SubcolumnFormat,
        /// Initial row count.
        rows: usize,
    },
    /// Structure of a table: kind, row count, columns with dataset/subcolumn mapping,
    /// formats, formulas, row titles.
    GetTableMeta {
        /// Table id.
        table: Id,
    },
    /// Cell viewport.
    GetCells {
        /// Table id.
        table: Id,
        /// Range to fetch.
        range: Range,
    },
    /// Type into one cell: a number, text, `=formula`, or empty to clear.
    SetCell {
        /// Table id.
        table: Id,
        /// Row.
        row: usize,
        /// Flat column.
        col: usize,
        /// Raw input as typed.
        input: String,
    },
    /// Set a block of cells from raw strings (multi-cell paste already split by the UI
    /// is NOT the route; use `Paste`). Used for programmatic fills.
    SetCells {
        /// Table id.
        table: Id,
        /// Top-left row.
        row0: usize,
        /// Top-left flat column.
        col0: usize,
        /// Rows of raw inputs.
        values: Vec<Vec<String>>,
    },
    /// Paste delimited text at a position; the engine parses and reports what it decided.
    Paste {
        /// Table id.
        table: Id,
        /// Top-left row.
        row0: usize,
        /// Top-left flat column.
        col0: usize,
        /// Raw clipboard text.
        text: String,
        /// Hints.
        options: PasteOptions,
    },
    /// Clear a range (values and text; formulas on columns are untouched).
    ClearCells {
        /// Table id.
        table: Id,
        /// Range.
        range: Range,
    },
    /// Mark or unmark a range as excluded.
    ExcludeCells {
        /// Table id.
        table: Id,
        /// Range.
        range: Range,
        /// Excluded or not.
        excluded: bool,
    },
    /// Copy the first row of the range down through the range.
    FillDown {
        /// Table id.
        table: Id,
        /// Range.
        range: Range,
    },
    /// Insert `count` rows before `at`.
    InsertRows {
        /// Table id.
        table: Id,
        /// Position.
        at: usize,
        /// Count.
        count: usize,
    },
    /// Delete rows `[at, at+count)`.
    DeleteRows {
        /// Table id.
        table: Id,
        /// Position.
        at: usize,
        /// Count.
        count: usize,
    },
    /// Append a dataset.
    AddDataset {
        /// Table id.
        table: Id,
        /// Name.
        name: Option<String>,
    },
    /// Remove a dataset by index.
    RemoveDataset {
        /// Table id.
        table: Id,
        /// Dataset index.
        dataset: usize,
    },
    /// Rename a dataset.
    SetDatasetName {
        /// Table id.
        table: Id,
        /// Dataset index.
        dataset: usize,
        /// Name.
        name: String,
    },
    /// Change the subcolumn format of every dataset (restructures columns).
    SetSubcolumnFormat {
        /// Table id.
        table: Id,
        /// Format.
        format: SubcolumnFormat,
    },
    /// Set or clear a column formula (`None` clears, keeping computed values as entered).
    SetColumnFormula {
        /// Table id.
        table: Id,
        /// Flat column.
        col: usize,
        /// Expression without the leading `=`.
        expr: Option<String>,
    },
    /// Display decimals for a column (`None` = automatic).
    SetColumnDecimals {
        /// Table id.
        table: Id,
        /// Flat column.
        col: usize,
        /// Decimals.
        decimals: Option<u8>,
    },
    /// Set a row title.
    SetRowTitle {
        /// Table id.
        table: Id,
        /// Row.
        row: usize,
        /// Title.
        title: String,
    },
    /// Set the X column title (XY/survival) or a variable's type.
    SetVariableType {
        /// Table id.
        table: Id,
        /// Dataset index.
        dataset: usize,
        /// Type.
        var_type: VariableType,
    },
    /// Whole-table transform producing a derived table.
    Transform {
        /// Source table id.
        table: Id,
        /// Operation.
        op: crate::transform::TransformOp,
    },
    /// Provenance of one cell.
    GetCellInfo {
        /// Table id.
        table: Id,
        /// Row.
        row: usize,
        /// Flat column.
        col: usize,
    },

    // ---- import / export ----
    /// Import delimited text as a new table.
    ImportText {
        /// Raw text.
        text: String,
        /// Target kind (`Variables` when unsure).
        kind: TableKind,
        /// Title.
        title: Option<String>,
        /// Hints.
        options: PasteOptions,
    },
    /// Import a file by path (desktop): csv, tsv, txt, xlsx, xls, ods, pzfx, prism.
    ImportPath {
        /// Absolute path.
        path: String,
        /// Target kind for delimited files.
        kind: Option<TableKind>,
        /// Plate map for plate-reader exports (see `plate::parse_plate_map`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plate_map: Option<String>,
    },
    /// Import a file from bytes (base64), format from the name's extension.
    ImportBytes {
        /// File name.
        name: String,
        /// Base64 content.
        base64: String,
        /// Target kind for delimited files.
        kind: Option<TableKind>,
        /// Plate map for plate-reader exports (see `plate::parse_plate_map`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plate_map: Option<String>,
    },
    /// What a plate-reader export holds, or `None` when the file is not one; returns
    /// `Response::Plate`. Give `base64` (or `path` on desktop).
    InspectPlate {
        /// File name.
        name: String,
        /// Base64 content.
        #[serde(default)]
        base64: Option<String>,
        /// Absolute path (desktop).
        #[serde(default)]
        path: Option<String>,
    },
    /// Read a file and put its data into an existing table of the same kind, keeping the
    /// table's id, names, column formulas, analyses and graphs (one undo step). Columns are
    /// filled left to right, skipping formula columns; returns `Response::PasteReport`.
    ReplaceTableData {
        /// Table id.
        table: Id,
        /// File name (the format follows its extension and content).
        name: String,
        /// Base64 content.
        base64: String,
        /// Plate map for plate-reader exports.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plate_map: Option<String>,
    },
    /// Export a table; returns `Response::Bytes`.
    ExportTable {
        /// Table id.
        table: Id,
        /// Format.
        format: TableFormat,
    },
    /// Export a table to a path (desktop).
    ExportTablePath {
        /// Table id.
        table: Id,
        /// Format.
        format: TableFormat,
        /// Absolute path.
        path: String,
    },

    // ---- analyses ----
    /// Which analyses fit this table (and optional selection).
    ListAnalyses {
        /// Table id.
        table: Id,
        /// Optional selection.
        range: Option<Range>,
    },
    /// Default options for an analysis kind on a table.
    DefaultAnalysisSpec {
        /// Table id.
        table: Id,
        /// Analysis kind name as in `ListAnalyses`.
        kind: String,
    },
    /// Create an analysis from a spec; returns its id.
    CreateAnalysis {
        /// Table id.
        table: Id,
        /// Specification.
        spec: crate::analysis::AnalysisSpec,
    },
    /// Replace an analysis' spec.
    UpdateAnalysis {
        /// Analysis id.
        id: Id,
        /// Specification.
        spec: crate::analysis::AnalysisSpec,
    },
    /// Spec of an analysis.
    GetAnalysisSpec {
        /// Analysis id.
        id: Id,
    },
    /// Results sheet (recomputed if stale).
    GetResults {
        /// Analysis id.
        id: Id,
    },
    /// Results sheet as TSV or Markdown text for copying.
    ExportResults {
        /// Analysis id.
        id: Id,
        /// `tsv` or `markdown`.
        format: String,
    },

    // ---- graphs ----
    /// Create a graph from a table or analysis with defaults for the family.
    CreateGraph {
        /// Table or analysis id.
        source: Id,
        /// Family name (see `dioptra_plot::GraphFamily`).
        family: String,
        /// Title.
        title: Option<String>,
    },
    /// Spec of a graph.
    GetGraphSpec {
        /// Graph id.
        id: Id,
    },
    /// Replace a graph's spec.
    UpdateGraph {
        /// Graph id.
        id: Id,
        /// Full spec.
        spec: crate::graph::GraphSpec,
    },
    /// Apply a JSON merge patch to a graph's spec (for the inspector).
    PatchGraph {
        /// Graph id.
        id: Id,
        /// RFC 7396 merge patch.
        patch: serde_json::Value,
        /// Merge into the previous undo step when that step was also a coalescing patch
        /// of this graph touching the same fields (typing in a text field sends one patch
        /// per pause; undo should take back the whole edit). Default false.
        #[serde(default)]
        coalesce: bool,
    },
    /// Render a graph to SVG for display; element ids map to spec paths for hit-testing.
    RenderGraph {
        /// Graph id.
        id: Id,
    },
    /// Export a graph; returns `Response::Bytes`.
    ExportGraph {
        /// Graph id.
        id: Id,
        /// Format.
        format: GraphFormat,
        /// Size override.
        size: Option<SizeMm>,
    },
    /// Export a graph to a path (desktop).
    ExportGraphPath {
        /// Graph id.
        id: Id,
        /// Format.
        format: GraphFormat,
        /// Size override.
        size: Option<SizeMm>,
        /// Absolute path.
        path: String,
    },

    // ---- layouts ----
    /// Create a layout.
    CreateLayout {
        /// Title.
        title: Option<String>,
        /// Graph ids to place in reading order.
        graphs: Vec<Id>,
        /// Columns in the grid.
        columns: usize,
    },
    /// Render a layout to SVG.
    RenderLayout {
        /// Layout id.
        id: Id,
    },
    /// Export a layout.
    ExportLayout {
        /// Layout id.
        id: Id,
        /// Format.
        format: GraphFormat,
    },
    /// A layout's graphs and column count; returns `Response::Layout`.
    GetLayout {
        /// Layout id.
        id: Id,
    },
    /// Replace a layout's graphs (reading order) and column count.
    UpdateLayout {
        /// Layout id.
        id: Id,
        /// Graph ids in reading order.
        graphs: Vec<Id>,
        /// Columns in the grid (at least 1).
        columns: usize,
        /// Final-size settings; absent keeps the current ones. Setting anything but the
        /// default needs Dioptra Lab.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<crate::graph::LayoutStyle>,
    },
    /// Export a layout to a path (desktop).
    ExportLayoutPath {
        /// Layout id.
        id: Id,
        /// Format.
        format: GraphFormat,
        /// Absolute path.
        path: String,
    },

    // ---- history ----
    /// Undo the last mutation.
    Undo,
    /// Redo.
    Redo,
    /// Recent history entries.
    GetHistory {
        /// Max entries.
        limit: usize,
    },
    /// Recompute everything that is stale.
    RecomputeAll,

    // ---- Dioptra Lab ----
    /// Set the license key (`None` removes it); returns `Response::License`.
    SetLicense {
        /// The key as pasted.
        key: Option<String>,
        /// Today's date, `YYYY-MM-DD`, from the UI's clock.
        today: String,
    },
    /// The license and whether Lab features are on today.
    GetLicense {
        /// Today's date, `YYYY-MM-DD`.
        today: String,
    },
}

/// What one cell looks like in a viewport.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CellView {
    /// Numeric value, if any (full precision).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<f64>,
    /// Text content for non-numeric entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
    /// Display string with the column's decimals.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub d: String,
    /// Excluded.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub x: bool,
    /// Computed by a formula.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub f: bool,
    /// Formula error message, if the cell's formula failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub e: Option<String>,
}

/// Which object changed, for the UI to refresh.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Changed {
    /// Ids whose content changed (tables, analyses, graphs).
    pub ids: Vec<Id>,
    /// Whether the project tree (object list) changed.
    pub tree: bool,
    /// Id created by this command, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<Id>,
}

/// A reply to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    /// Reply to `Ping`.
    Pong {
        /// Engine version.
        version: String,
    },
    /// Mutation succeeded.
    Ok {
        /// What changed.
        changed: Changed,
    },
    /// The command failed.
    Error {
        /// Human-readable message.
        message: String,
    },
    /// Project tree.
    Project(crate::project::ProjectView),
    /// Table structure.
    TableMeta(crate::table::TableMeta),
    /// Cell viewport, row-major.
    Cells {
        /// Range actually returned (clipped to the table).
        range: Range,
        /// Rows of cells.
        rows: Vec<Vec<CellView>>,
    },
    /// What a paste did.
    PasteReport(crate::import::PasteReport),
    /// Provenance of a cell.
    CellInfo(crate::table::CellInfo),
    /// Analysis options for a table. (A struct variant: serde cannot tag a bare list.)
    Analyses {
        /// Options in menu order.
        analyses: Vec<crate::analysis::AnalysisOption>,
    },
    /// A spec.
    AnalysisSpec(crate::analysis::AnalysisSpec),
    /// A results sheet.
    Results(crate::results::ResultsSheet),
    /// A graph spec.
    GraphSpec(Box<crate::graph::GraphSpec>),
    /// Rendered SVG. `targets` lists the clickable elements of the graph editor: each has
    /// the element `id` in the SVG, the JSON pointer `target` into the graph spec that the
    /// inspector edits, and `bbox` (`x`, `y`, `width`, `height`) in points in the SVG's
    /// `viewBox` coordinates. For layouts, ids inside panel `i` are prefixed `p{i}-`.
    Svg {
        /// SVG document.
        svg: String,
        /// Width in millimetres.
        width_mm: f64,
        /// Height in millimetres.
        height_mm: f64,
        /// Clickable elements for hit-testing.
        #[serde(default)]
        targets: Vec<dioptra_plot::Target>,
    },
    /// Binary payload.
    Bytes {
        /// Suggested file name.
        name: String,
        /// MIME type.
        mime: String,
        /// Base64 content.
        base64: String,
    },
    /// Plain text payload (results export).
    Text {
        /// Content.
        text: String,
    },
    /// A layout.
    Layout {
        /// Layout id.
        id: Id,
        /// Title.
        title: String,
        /// Graph ids in reading order.
        graphs: Vec<Id>,
        /// Columns in the grid.
        columns: usize,
        /// Final-size settings.
        #[serde(default)]
        style: crate::graph::LayoutStyle,
    },
    /// License state.
    License(crate::lab::LicenseView),
    /// Reply to `InspectPlate`.
    Plate {
        /// The plate export, if the file is one.
        plate: Option<crate::plate::PlateSummary>,
    },
    /// History entries, newest first. (A struct variant: serde cannot tag a bare list.)
    History {
        /// Entries.
        entries: Vec<crate::history::HistoryEntry>,
    },
}

/// One open project plus its history, the analysis computations and the graph renderer.
/// `Default`, `Debug` and `dispatch` live in `session.rs`.
pub struct Session {
    pub(crate) project: crate::project::Project,
    pub(crate) history: crate::history::History,
    pub(crate) renderer: Box<dyn crate::graph::Renderer>,
    pub(crate) engine: crate::analysis::AnalysisEngine,
    pub(crate) lab: crate::lab::LabState,
    /// State other components keep for the session (not saved with the project).
    pub(crate) memo: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Session {
    /// Create an empty session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether Dioptra Lab features are on, and the error to show when they are not.
    pub fn require_lab(&self, feature: &str) -> Result<(), String> {
        self.lab.require(feature)
    }

    /// JSON-in, JSON-out convenience for the two bridges.
    pub fn dispatch_json(&mut self, json: &str) -> String {
        let response = match serde_json::from_str::<Command>(json) {
            Ok(cmd) => self.dispatch(cmd),
            Err(e) => Response::Error {
                message: format!("bad command: {e}"),
            },
        };
        serde_json::to_string(&response).expect("response serialises")
    }
}
