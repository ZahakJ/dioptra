//! Tables: the eight kinds over one storage model (DESIGN §4.1, docs/FILE-FORMAT.md).
//!
//! A table has an optional X column, datasets made of subcolumns, and optional row titles.
//! Every column is a dense `Vec<f64>` the length of the table's row count, with NaN for an
//! empty cell, plus sparse maps for text entries, excluded rows and cell formulas.
//! Columns are addressed by a *flat index*: the X column first (if any), then each
//! dataset's subcolumns left to right.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::api::{CellView, Id, SubcolumnFormat, TableKind, VariableType};
use crate::util::{format_number, letters, nan_eq, nan_vec};

/// Where a column's values came from.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColumnSource {
    /// Typed by hand (the default for new columns).
    #[default]
    Entered,
    /// Pasted from the clipboard.
    Pasted {
        /// When.
        at: String,
    },
    /// Imported from a file.
    Imported {
        /// File name.
        file: String,
        /// Sheet name, for workbooks.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sheet: Option<String>,
        /// Column heading or letter in the file.
        column: String,
    },
    /// Computed by a column formula.
    Formula {
        /// Expression without the leading `=`.
        expr: String,
    },
    /// Produced by a transform of another table.
    Derived {
        /// Source table id.
        table: Id,
        /// Operation name.
        op: String,
    },
}

/// A cell's content, as recorded in the edit log.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CellValue {
    /// Empty.
    #[default]
    Empty,
    /// A number.
    Number(f64),
    /// Text (or a cell formula, written with its leading `=`).
    Text(String),
}

/// One hand edit of a cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellEdit {
    /// Row.
    pub row: usize,
    /// When.
    pub at: String,
    /// Previous content.
    pub from: CellValue,
    /// New content.
    pub to: CellValue,
}

/// One column of numbers with its provenance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Column {
    /// Optional subcolumn title (the X column's title lives here).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Values; NaN is an empty cell.
    #[serde(with = "nan_vec")]
    pub values: Vec<f64>,
    /// Excluded rows (kept, shown struck through, ignored by analyses and formulas).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub excluded: BTreeSet<usize>,
    /// Non-numeric entries by row.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub text: BTreeMap<usize, String>,
    /// Where the values came from.
    #[serde(default)]
    pub source: ColumnSource,
    /// Column formula, without the leading `=`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// Excel-style cell formulas by row (only when there is no column formula).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cell_formulas: BTreeMap<usize, String>,
    /// Display decimals; `None` is automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    /// Hand edits on top of the source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<CellEdit>,
    /// Formula errors by row (computed, not saved).
    #[serde(skip)]
    pub errors: BTreeMap<usize, String>,
    /// An error that applies to the whole column formula (computed, not saved).
    #[serde(skip)]
    pub formula_error: Option<String>,
}

impl PartialEq for Column {
    fn eq(&self, o: &Self) -> bool {
        self.title == o.title
            && self.values.len() == o.values.len()
            && self
                .values
                .iter()
                .zip(&o.values)
                .all(|(a, b)| nan_eq(*a, *b))
            && self.excluded == o.excluded
            && self.text == o.text
            && self.source == o.source
            && self.formula == o.formula
            && self.cell_formulas == o.cell_formulas
            && self.decimals == o.decimals
            && self.edits == o.edits
    }
}

impl Column {
    /// An empty column with `rows` empty cells.
    pub fn empty(rows: usize) -> Self {
        Column {
            values: vec![f64::NAN; rows],
            ..Default::default()
        }
    }

    /// A column from values.
    pub fn from_values(values: Vec<f64>) -> Self {
        Column {
            values,
            ..Default::default()
        }
    }

    /// Whether the column (or any of its cells) is computed by a formula.
    pub fn has_formula(&self) -> bool {
        self.formula.is_some() || !self.cell_formulas.is_empty()
    }

    /// Whether a cell is computed by a formula.
    pub fn is_formula_cell(&self, row: usize) -> bool {
        self.formula.is_some() || self.cell_formulas.contains_key(&row)
    }

    /// The content of a cell as a [`CellValue`] (cell formulas as `=expr` text).
    pub fn cell_value(&self, row: usize) -> CellValue {
        if let Some(f) = self.cell_formulas.get(&row) {
            return CellValue::Text(format!("={f}"));
        }
        let v = self.values.get(row).copied().unwrap_or(f64::NAN);
        if !v.is_nan() {
            CellValue::Number(v)
        } else if let Some(t) = self.text.get(&row) {
            CellValue::Text(t.clone())
        } else {
            CellValue::Empty
        }
    }

    /// Clear a cell's value and text (not its exclusion).
    pub fn clear_cell(&mut self, row: usize) {
        if let Some(v) = self.values.get_mut(row) {
            *v = f64::NAN;
        }
        self.text.remove(&row);
        self.cell_formulas.remove(&row);
        self.errors.remove(&row);
    }

    /// Numeric values that analyses may use: not empty, not text, not excluded.
    pub fn usable(&self) -> impl Iterator<Item = f64> + '_ {
        self.values
            .iter()
            .enumerate()
            .filter(|(i, v)| !v.is_nan() && !self.excluded.contains(i))
            .map(|(_, v)| *v)
    }

    /// Value at a row as analyses see it: NaN for empty, text or excluded.
    pub fn usable_at(&self, row: usize) -> f64 {
        match self.values.get(row) {
            Some(v) if !self.excluded.contains(&row) => *v,
            _ => f64::NAN,
        }
    }

    pub(crate) fn resize(&mut self, rows: usize) {
        self.values.resize(rows, f64::NAN);
        let cut = |r: &usize| *r < rows;
        self.excluded.retain(cut);
        self.text.retain(|r, _| *r < rows);
        self.cell_formulas.retain(|r, _| *r < rows);
        self.errors.retain(|r, _| *r < rows);
    }

    pub(crate) fn insert_rows(&mut self, at: usize, count: usize) {
        let at = at.min(self.values.len());
        self.values
            .splice(at..at, std::iter::repeat_n(f64::NAN, count));
        self.excluded = self
            .excluded
            .iter()
            .map(|&r| if r >= at { r + count } else { r })
            .collect();
        shift_map(&mut self.text, at, count as isize);
        shift_map(&mut self.cell_formulas, at, count as isize);
        shift_map(&mut self.errors, at, count as isize);
        for e in &mut self.edits {
            if e.row >= at {
                e.row += count;
            }
        }
    }

    pub(crate) fn delete_rows(&mut self, at: usize, count: usize) {
        let end = (at + count).min(self.values.len());
        if at >= end {
            return;
        }
        let n = end - at;
        self.values.drain(at..end);
        self.excluded = self
            .excluded
            .iter()
            .filter(|&&r| r < at || r >= end)
            .map(|&r| if r >= end { r - n } else { r })
            .collect();
        for m in [&mut self.text, &mut self.cell_formulas, &mut self.errors] {
            m.retain(|r, _| *r < at || *r >= end);
            shift_map(m, end, -(n as isize));
        }
        self.edits.retain(|e| e.row < at || e.row >= end);
        for e in &mut self.edits {
            if e.row >= end {
                e.row -= n;
            }
        }
    }
}

fn shift_map<V>(m: &mut BTreeMap<usize, V>, at: usize, delta: isize) {
    if m.range(at..).next().is_none() {
        return;
    }
    let moved: Vec<(usize, V)> = {
        let keys: Vec<usize> = m.range(at..).map(|(k, _)| *k).collect();
        keys.into_iter()
            .map(|k| (k, m.remove(&k).expect("key")))
            .collect()
    };
    for (k, v) in moved {
        m.insert((k as isize + delta) as usize, v);
    }
}

/// A dataset: a named group of subcolumns (replicates or summary values).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    /// Name shown in the header.
    pub name: String,
    /// How the subcolumns are read.
    pub format: SubcolumnFormat,
    /// The subcolumns.
    pub subcolumns: Vec<Column>,
    /// Declared type, for `variables` tables.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub var_type: Option<VariableType>,
}

/// Where a derived table came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedFrom {
    /// Source table.
    pub table: Id,
    /// The transform.
    pub op: crate::transform::TransformOp,
}

/// A data table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    /// Stable id.
    pub id: Id,
    /// User title.
    pub title: String,
    /// Kind.
    pub kind: TableKind,
    /// Row count; every column has exactly this many values.
    pub rows: usize,
    /// X column for `xy` and `survival` tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Column>,
    /// Row titles (may be shorter than `rows`; missing entries are empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_titles: Vec<String>,
    /// Datasets.
    pub datasets: Vec<Dataset>,
    /// Recipe, for derived tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<DerivedFrom>,
    /// Flat indices of formula columns that need recomputing (runtime only).
    #[serde(skip)]
    pub dirty: BTreeSet<usize>,
    /// Derived table whose recipe must be replayed (runtime only).
    #[serde(skip)]
    pub stale: bool,
}

impl PartialEq for Table {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
            && self.title == o.title
            && self.kind == o.kind
            && self.rows == o.rows
            && self.x == o.x
            && self.row_titles_trimmed() == o.row_titles_trimmed()
            && self.datasets == o.datasets
            && self.derived_from == o.derived_from
    }
}

/// Role of a flat column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnRole {
    /// The X column.
    X,
    /// A Y (dataset) subcolumn.
    Y,
}

/// Location of a flat column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColLoc {
    /// The X column.
    X,
    /// Dataset and subcolumn.
    Y {
        /// Dataset index.
        dataset: usize,
        /// Subcolumn index.
        sub: usize,
    },
}

/// One flat column in [`TableMeta`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnMeta {
    /// Flat index.
    pub index: usize,
    /// X or Y.
    pub role: ColumnRole,
    /// Dataset index (`None` for X).
    pub dataset: Option<usize>,
    /// Subcolumn index within the dataset.
    pub subcolumn: usize,
    /// Short label such as `X`, `A`, `A:Y1`, `B:SD`.
    pub label: String,
    /// Header text such as `Control (1)`.
    pub header: String,
    /// Dataset name (X title for X).
    pub dataset_name: String,
    /// Subcolumn format of the dataset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<SubcolumnFormat>,
    /// Column formula.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// Whether some cells carry their own formulas.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cell_formulas: bool,
    /// Display decimals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    /// Variable type (variables tables).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub var_type: Option<VariableType>,
    /// Column source.
    pub source: ColumnSource,
}

/// One dataset in [`TableMeta`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetMeta {
    /// Name.
    pub name: String,
    /// Letter used in formulas (`A`, `B`, …).
    pub letter: String,
    /// Format.
    pub format: SubcolumnFormat,
    /// Flat index of the first subcolumn.
    pub first_col: usize,
    /// Number of subcolumns.
    pub subcolumns: usize,
    /// Variable type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub var_type: Option<VariableType>,
}

/// Structure of a table for the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableMeta {
    /// Id.
    pub id: Id,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: TableKind,
    /// Row count.
    pub rows: usize,
    /// Whether the table has an X column (flat column 0).
    pub has_x: bool,
    /// Whether row titles are shown.
    pub has_row_titles: bool,
    /// Row titles, padded to `rows` only when any is set (else empty).
    pub row_titles: Vec<String>,
    /// Flat columns.
    pub columns: Vec<ColumnMeta>,
    /// Datasets.
    pub datasets: Vec<DatasetMeta>,
    /// Source table, for derived tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<Id>,
    /// Whether the table is read-only (derived).
    pub read_only: bool,
}

/// Provenance of one cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellInfo {
    /// Table.
    pub table: Id,
    /// Row.
    pub row: usize,
    /// Flat column.
    pub col: usize,
    /// Address such as `B3` (dataset letter, 1-based row) or `X3`.
    pub address: String,
    /// The cell as the grid shows it.
    pub cell: CellView,
    /// Column source.
    pub source: ColumnSource,
    /// Column formula.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_formula: Option<String>,
    /// This cell's own formula.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_formula: Option<String>,
    /// Hand edits of this cell, oldest first.
    pub edits: Vec<CellEdit>,
    /// Plain-English provenance.
    pub provenance: String,
}

/// Number of subcolumns a format implies.
pub fn subcolumn_count(format: SubcolumnFormat) -> usize {
    match format {
        SubcolumnFormat::Values(n) => (n as usize).max(1),
        SubcolumnFormat::MeanSdN | SubcolumnFormat::MeanSemN | SubcolumnFormat::MeanCiN => 3,
        SubcolumnFormat::MedianIqr => 3,
        SubcolumnFormat::MeanSd | SubcolumnFormat::MeanSem | SubcolumnFormat::LowerUpper => 2,
        SubcolumnFormat::Single => 1,
    }
}

/// Names of the subcolumns of a format (`None` for replicate values).
pub fn subcolumn_names(format: SubcolumnFormat) -> Option<&'static [&'static str]> {
    Some(match format {
        SubcolumnFormat::Values(_) => return None,
        SubcolumnFormat::MeanSdN => &["Mean", "SD", "N"],
        SubcolumnFormat::MeanSemN => &["Mean", "SEM", "N"],
        SubcolumnFormat::MeanCiN => &["Mean", "CI", "N"],
        SubcolumnFormat::MeanSd => &["Mean", "SD"],
        SubcolumnFormat::MeanSem => &["Mean", "SEM"],
        SubcolumnFormat::MedianIqr => &["Median", "Q1", "Q3"],
        SubcolumnFormat::LowerUpper => &["Lower", "Upper"],
        SubcolumnFormat::Single => return None,
    })
}

/// Whether a kind has an X column.
pub fn kind_has_x(kind: TableKind) -> bool {
    matches!(kind, TableKind::Xy | TableKind::Survival)
}

/// Whether a kind shows row titles.
pub fn kind_has_row_titles(kind: TableKind) -> bool {
    matches!(
        kind,
        TableKind::Grouped | TableKind::Contingency | TableKind::Parts | TableKind::Xy
    )
}

/// The format a kind forces, if any.
pub fn forced_format(kind: TableKind) -> Option<SubcolumnFormat> {
    match kind {
        TableKind::Contingency | TableKind::Survival | TableKind::Parts | TableKind::Variables => {
            Some(SubcolumnFormat::Single)
        }
        _ => None,
    }
}

/// Parse a typed number the way the grid does: point decimal, optional exponent, no
/// thousands separators. Rejects `inf`/`nan` spellings.
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let first = *s.as_bytes().first()?;
    if !(first.is_ascii_digit() || first == b'-' || first == b'+' || first == b'.') {
        return None;
    }
    let last = *s.as_bytes().last()?;
    if !(last.is_ascii_digit() || last == b'.') {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

impl Table {
    /// A new empty table.
    pub fn new(
        id: Id,
        kind: TableKind,
        title: String,
        datasets: usize,
        format: SubcolumnFormat,
        rows: usize,
    ) -> Self {
        let format = forced_format(kind).unwrap_or(format);
        let mut t = Table {
            id,
            title,
            kind,
            rows,
            x: kind_has_x(kind).then(|| Column {
                title: if kind == TableKind::Survival {
                    "Time".into()
                } else {
                    "X".into()
                },
                ..Column::empty(rows)
            }),
            row_titles: Vec::new(),
            datasets: Vec::new(),
            derived_from: None,
            dirty: BTreeSet::new(),
            stale: false,
        };
        for _ in 0..datasets {
            t.push_dataset(None, format);
        }
        t
    }

    fn row_titles_trimmed(&self) -> &[String] {
        let n = self
            .row_titles
            .iter()
            .rposition(|s| !s.is_empty())
            .map_or(0, |i| i + 1);
        &self.row_titles[..n]
    }

    /// Default name for the dataset at `index`.
    pub fn default_dataset_name(&self, index: usize) -> String {
        match self.kind {
            TableKind::Variables => format!("Variable {}", letters(index)),
            _ => format!("Data set {}", letters(index)),
        }
    }

    /// Append a dataset; returns its index.
    pub fn push_dataset(&mut self, name: Option<String>, format: SubcolumnFormat) -> usize {
        let format = forced_format(self.kind).unwrap_or(format);
        let index = self.datasets.len();
        let name = name.unwrap_or_else(|| self.default_dataset_name(index));
        let var_type = (self.kind == TableKind::Variables).then_some(VariableType::Number);
        self.datasets.push(Dataset {
            name,
            format,
            subcolumns: (0..subcolumn_count(format))
                .map(|_| Column::empty(self.rows))
                .collect(),
            var_type,
        });
        index
    }

    /// The format new datasets get (that of the last dataset, or one value).
    pub fn default_format(&self) -> SubcolumnFormat {
        self.datasets
            .last()
            .map(|d| d.format)
            .unwrap_or(match self.kind {
                TableKind::Column | TableKind::Grouped | TableKind::Xy | TableKind::Nested => {
                    SubcolumnFormat::Values(1)
                }
                _ => SubcolumnFormat::Single,
            })
    }

    /// Number of flat columns.
    pub fn col_count(&self) -> usize {
        usize::from(self.x.is_some())
            + self
                .datasets
                .iter()
                .map(|d| d.subcolumns.len())
                .sum::<usize>()
    }

    /// Where a flat column lives.
    pub fn locate(&self, flat: usize) -> Option<ColLoc> {
        let mut i = flat;
        if self.x.is_some() {
            if i == 0 {
                return Some(ColLoc::X);
            }
            i -= 1;
        }
        for (d, ds) in self.datasets.iter().enumerate() {
            if i < ds.subcolumns.len() {
                return Some(ColLoc::Y { dataset: d, sub: i });
            }
            i -= ds.subcolumns.len();
        }
        None
    }

    /// Flat index of a dataset's subcolumn.
    pub fn flat_index(&self, dataset: usize, sub: usize) -> Option<usize> {
        let ds = self.datasets.get(dataset)?;
        if sub >= ds.subcolumns.len() {
            return None;
        }
        let before: usize = self.datasets[..dataset]
            .iter()
            .map(|d| d.subcolumns.len())
            .sum();
        Some(usize::from(self.x.is_some()) + before + sub)
    }

    /// A flat column.
    pub fn col(&self, flat: usize) -> Option<&Column> {
        match self.locate(flat)? {
            ColLoc::X => self.x.as_ref(),
            ColLoc::Y { dataset, sub } => self.datasets[dataset].subcolumns.get(sub),
        }
    }

    /// A flat column, mutably.
    pub fn col_mut(&mut self, flat: usize) -> Option<&mut Column> {
        match self.locate(flat)? {
            ColLoc::X => self.x.as_mut(),
            ColLoc::Y { dataset, sub } => self.datasets[dataset].subcolumns.get_mut(sub),
        }
    }

    /// All columns in flat order.
    pub fn columns(&self) -> impl Iterator<Item = &Column> {
        self.x
            .iter()
            .chain(self.datasets.iter().flat_map(|d| d.subcolumns.iter()))
    }

    /// All columns in flat order, mutably.
    pub fn columns_mut(&mut self) -> impl Iterator<Item = &mut Column> {
        self.x.iter_mut().chain(
            self.datasets
                .iter_mut()
                .flat_map(|d| d.subcolumns.iter_mut()),
        )
    }

    /// Grow (never shrink) the row count.
    pub fn ensure_rows(&mut self, rows: usize) {
        if rows > self.rows {
            self.set_rows(rows);
        }
    }

    /// Set the row count exactly.
    pub fn set_rows(&mut self, rows: usize) {
        self.rows = rows;
        for c in self.columns_mut() {
            c.resize(rows);
        }
        self.row_titles.truncate(rows);
    }

    /// Row title (empty when unset).
    pub fn row_title(&self, row: usize) -> &str {
        self.row_titles.get(row).map(String::as_str).unwrap_or("")
    }

    /// Set a row title, growing rows if needed.
    pub fn set_row_title(&mut self, row: usize, title: String) {
        self.ensure_rows(row + 1);
        if self.row_titles.len() <= row {
            self.row_titles.resize(row + 1, String::new());
        }
        self.row_titles[row] = title;
    }

    /// Whether any row title is set.
    pub fn has_row_titles(&self) -> bool {
        self.row_titles.iter().any(|s| !s.is_empty())
    }

    /// Short label of a flat column (`X`, `A`, `A:Y2`, `B:SD`).
    pub fn col_label(&self, flat: usize) -> String {
        match self.locate(flat) {
            Some(ColLoc::X) => "X".into(),
            Some(ColLoc::Y { dataset, sub }) => {
                let ds = &self.datasets[dataset];
                let l = letters(dataset);
                if ds.subcolumns.len() == 1 {
                    l
                } else if let Some(names) = subcolumn_names(ds.format) {
                    format!("{l}:{}", names.get(sub).copied().unwrap_or("?"))
                } else {
                    format!("{l}:Y{}", sub + 1)
                }
            }
            None => String::new(),
        }
    }

    /// Header text of a flat column (`Control`, `Control (2)`, `Control (SD)`), or the
    /// subcolumn's own title when it has one (`Control (Mouse 3)`).
    pub fn col_header(&self, flat: usize) -> String {
        match self.locate(flat) {
            Some(ColLoc::X) => {
                let t = &self.x.as_ref().expect("x").title;
                if t.is_empty() {
                    "X".into()
                } else {
                    t.clone()
                }
            }
            Some(ColLoc::Y { dataset, sub }) => {
                let ds = &self.datasets[dataset];
                let own = ds.subcolumns.get(sub).map_or("", |c| c.title.as_str());
                if ds.subcolumns.len() == 1 {
                    ds.name.clone()
                } else if !own.is_empty() {
                    // A named replicate or nested subgroup.
                    format!("{} ({own})", ds.name)
                } else if let Some(names) = subcolumn_names(ds.format) {
                    format!("{} ({})", ds.name, names.get(sub).copied().unwrap_or("?"))
                } else {
                    format!("{} ({})", ds.name, sub + 1)
                }
            }
            None => String::new(),
        }
    }

    /// Cell address like `B3` (dataset letter, 1-based row; `B.2[3]` for subcolumns).
    pub fn address(&self, row: usize, flat: usize) -> String {
        match self.locate(flat) {
            Some(ColLoc::X) => format!("X{}", row + 1),
            Some(ColLoc::Y { dataset, sub }) => {
                if self.datasets[dataset].subcolumns.len() == 1 || sub == 0 {
                    format!("{}{}", letters(dataset), row + 1)
                } else {
                    format!("{}.{}[{}]", letters(dataset), sub + 1, row + 1)
                }
            }
            None => format!("?{}", row + 1),
        }
    }

    /// Structure for the UI.
    pub fn meta(&self) -> TableMeta {
        let mut columns = Vec::with_capacity(self.col_count());
        let mut datasets = Vec::with_capacity(self.datasets.len());
        let mut flat = 0;
        if let Some(x) = &self.x {
            columns.push(ColumnMeta {
                index: 0,
                role: ColumnRole::X,
                dataset: None,
                subcolumn: 0,
                label: "X".into(),
                header: self.col_header(0),
                dataset_name: x.title.clone(),
                format: None,
                formula: x.formula.clone(),
                cell_formulas: !x.cell_formulas.is_empty(),
                decimals: x.decimals,
                var_type: None,
                source: x.source.clone(),
            });
            flat = 1;
        }
        for (d, ds) in self.datasets.iter().enumerate() {
            datasets.push(DatasetMeta {
                name: ds.name.clone(),
                letter: letters(d),
                format: ds.format,
                first_col: flat,
                subcolumns: ds.subcolumns.len(),
                var_type: ds.var_type,
            });
            for (s, c) in ds.subcolumns.iter().enumerate() {
                columns.push(ColumnMeta {
                    index: flat,
                    role: ColumnRole::Y,
                    dataset: Some(d),
                    subcolumn: s,
                    label: self.col_label(flat),
                    header: self.col_header(flat),
                    dataset_name: ds.name.clone(),
                    format: Some(ds.format),
                    formula: c.formula.clone(),
                    cell_formulas: !c.cell_formulas.is_empty(),
                    decimals: c.decimals,
                    var_type: ds.var_type,
                    source: c.source.clone(),
                });
                flat += 1;
            }
        }
        let has_rt = self.has_row_titles();
        let mut row_titles = Vec::new();
        if has_rt {
            row_titles = (0..self.rows)
                .map(|r| self.row_title(r).to_string())
                .collect();
        }
        TableMeta {
            id: self.id.clone(),
            title: self.title.clone(),
            kind: self.kind,
            rows: self.rows,
            has_x: self.x.is_some(),
            has_row_titles: has_rt || kind_has_row_titles(self.kind),
            row_titles,
            columns,
            datasets,
            derived_from: self.derived_from.as_ref().map(|d| d.table.clone()),
            read_only: self.derived_from.is_some(),
        }
    }

    /// One cell as the grid shows it.
    pub fn cell_view(&self, row: usize, col: &Column) -> CellView {
        let mut cv = CellView::default();
        let v = col.values.get(row).copied().unwrap_or(f64::NAN);
        if !v.is_nan() {
            cv.v = Some(v);
            cv.d = format_number(v, col.decimals);
        } else if let Some(t) = col.text.get(&row) {
            cv.t = Some(t.clone());
            cv.d = t.clone();
        }
        cv.x = !col.excluded.is_empty() && col.excluded.contains(&row);
        cv.f = col.is_formula_cell(row);
        let err = col.formula_error.as_ref().or_else(|| {
            if col.errors.is_empty() {
                None
            } else {
                col.errors.get(&row)
            }
        });
        if let Some(e) = err {
            cv.e = Some(e.clone());
            if cv.d.is_empty() {
                cv.d = "#ERROR".into();
            }
        }
        cv
    }

    /// Store raw typed input in a cell, growing the table if needed. Returns the previous
    /// content. Numbers are stored as numbers, anything else as text (never dropped);
    /// input starting with `=` becomes a cell formula. Errors if the column has a column
    /// formula or the column does not exist.
    pub fn set_cell_raw(
        &mut self,
        row: usize,
        flat: usize,
        input: &str,
    ) -> Result<CellValue, String> {
        let label = self.col_label(flat);
        let col = self.col(flat).ok_or_else(|| format!("no column {flat}"))?;
        if let Some(f) = &col.formula {
            return Err(format!(
                "Column {label} is computed by the formula ={f}; clear the formula to type values."
            ));
        }
        self.ensure_rows(row + 1);
        let col = self.col_mut(flat).expect("column exists");
        let before = col.cell_value(row);
        col.clear_cell(row);
        let trimmed = input.trim();
        if trimmed.is_empty() {
        } else if let Some(expr) = trimmed.strip_prefix('=') {
            col.cell_formulas.insert(row, expr.trim().to_string());
        } else if let Some(v) = parse_number(trimmed) {
            col.values[row] = v;
        } else {
            col.text.insert(row, input.to_string());
        }
        Ok(before)
    }

    /// Restore a cell to a recorded content (used by undo).
    pub fn put_cell_value(&mut self, row: usize, flat: usize, value: &CellValue) {
        self.ensure_rows(row + 1);
        if let Some(col) = self.col_mut(flat) {
            col.clear_cell(row);
            match value {
                CellValue::Empty => {}
                CellValue::Number(v) => col.values[row] = *v,
                CellValue::Text(t) => {
                    if let Some(e) = t.strip_prefix('=') {
                        col.cell_formulas.insert(row, e.to_string());
                    } else {
                        col.text.insert(row, t.clone());
                    }
                }
            }
        }
    }

    /// Insert empty rows.
    pub fn insert_rows(&mut self, at: usize, count: usize) {
        let at = at.min(self.rows);
        for c in self.columns_mut() {
            c.insert_rows(at, count);
        }
        if self.row_titles.len() > at {
            self.row_titles
                .splice(at..at, std::iter::repeat_n(String::new(), count));
        }
        self.rows += count;
    }

    /// Delete rows `[at, at+count)`.
    pub fn delete_rows(&mut self, at: usize, count: usize) {
        if at >= self.rows {
            return;
        }
        let end = (at + count).min(self.rows);
        for c in self.columns_mut() {
            c.delete_rows(at, end - at);
        }
        if self.row_titles.len() > at {
            let e = end.min(self.row_titles.len());
            self.row_titles.drain(at..e);
        }
        self.rows -= end - at;
    }

    /// Change every dataset's format, keeping existing subcolumns where they fit.
    pub fn set_format(&mut self, format: SubcolumnFormat) {
        let format = forced_format(self.kind).unwrap_or(format);
        let rows = self.rows;
        for ds in &mut self.datasets {
            ds.format = format;
            let n = subcolumn_count(format);
            ds.subcolumns.truncate(n);
            while ds.subcolumns.len() < n {
                ds.subcolumns.push(Column::empty(rows));
            }
        }
    }

    /// Provenance of a cell.
    pub fn cell_info(&self, row: usize, flat: usize) -> Option<CellInfo> {
        let col = self.col(flat)?;
        let cell = if row < self.rows {
            self.cell_view(row, col)
        } else {
            CellView::default()
        };
        let edits: Vec<CellEdit> = col.edits.iter().filter(|e| e.row == row).cloned().collect();
        let cell_formula = col.cell_formulas.get(&row).cloned();
        let mut p = String::new();
        if let Some(f) = &col.formula {
            p.push_str(&format!("Computed by the column formula ={f}."));
        } else if let Some(f) = &cell_formula {
            p.push_str(&format!("Computed by the cell formula ={f}."));
        } else {
            match &col.source {
                ColumnSource::Entered => p.push_str("Typed by hand."),
                ColumnSource::Pasted { at } => {
                    p.push_str(&format!("Pasted from the clipboard at {at}."))
                }
                ColumnSource::Imported {
                    file,
                    sheet,
                    column,
                } => {
                    p.push_str(&format!("Imported from {file}"));
                    if let Some(s) = sheet {
                        p.push_str(&format!(", sheet {s}"));
                    }
                    p.push_str(&format!(", column {column}."));
                }
                ColumnSource::Formula { expr } => {
                    p.push_str(&format!("Computed by ={expr} (formula since cleared)."))
                }
                ColumnSource::Derived { table, op } => {
                    p.push_str(&format!("Derived from table {table} by {op}."))
                }
            }
        }
        for e in &edits {
            p.push_str(&format!(
                " Edited by hand at {} from {} to {}.",
                e.at,
                describe(&e.from),
                describe(&e.to)
            ));
        }
        if cell.x {
            p.push_str(" Excluded from analyses.");
        }
        if let Some(e) = &cell.e {
            p.push_str(&format!(" Error: {e}"));
        }
        Some(CellInfo {
            table: self.id.clone(),
            row,
            col: flat,
            address: self.address(row, flat),
            cell,
            source: col.source.clone(),
            column_formula: col.formula.clone(),
            cell_formula,
            edits,
            provenance: p,
        })
    }

    /// Usable values of a whole dataset (all subcolumns pooled), for replicate formats.
    pub fn dataset_values(&self, dataset: usize) -> Vec<f64> {
        self.datasets
            .get(dataset)
            .map(|d| d.subcolumns.iter().flat_map(|c| c.usable()).collect())
            .unwrap_or_default()
    }
}

fn describe(v: &CellValue) -> String {
    match v {
        CellValue::Empty => "empty".into(),
        CellValue::Number(x) => format_number(*x, None),
        CellValue::Text(t) => format!("\"{t}\""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds() -> [TableKind; 8] {
        [
            TableKind::Xy,
            TableKind::Column,
            TableKind::Grouped,
            TableKind::Contingency,
            TableKind::Survival,
            TableKind::Parts,
            TableKind::Variables,
            TableKind::Nested,
        ]
    }

    #[test]
    fn every_kind_creates_and_grows() {
        for kind in kinds() {
            let mut t = Table::new(
                "t".into(),
                kind,
                "T".into(),
                2,
                SubcolumnFormat::Values(2),
                3,
            );
            let n = t.col_count();
            assert!(n >= 2, "{kind:?}");
            t.set_cell_raw(9, n - 1, "4.5").unwrap();
            assert_eq!(t.rows, 10);
            for c in t.columns() {
                assert_eq!(c.values.len(), 10);
            }
            assert_eq!(t.col(n - 1).unwrap().values[9], 4.5);
            let meta = t.meta();
            assert_eq!(meta.columns.len(), n);
            assert_eq!(meta.has_x, kind_has_x(kind));
        }
    }

    #[test]
    fn flat_indexing_and_labels() {
        let t = Table::new(
            "t".into(),
            TableKind::Xy,
            "T".into(),
            2,
            SubcolumnFormat::Values(3),
            1,
        );
        assert_eq!(t.col_count(), 7);
        assert_eq!(t.locate(0), Some(ColLoc::X));
        assert_eq!(t.locate(4), Some(ColLoc::Y { dataset: 1, sub: 0 }));
        assert_eq!(t.col_label(1), "A:Y1");
        assert_eq!(t.col_label(6), "B:Y3");
        assert_eq!(t.col_header(2), "Data set A (2)");
        assert_eq!(t.flat_index(1, 2), Some(6));
        let g = Table::new(
            "g".into(),
            TableKind::Grouped,
            "G".into(),
            1,
            SubcolumnFormat::MeanSdN,
            1,
        );
        assert_eq!(g.col_label(1), "A:SD");
    }

    #[test]
    fn text_is_kept_and_contingency_keeps_decimals() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Contingency,
            "T".into(),
            2,
            SubcolumnFormat::Single,
            2,
        );
        t.set_cell_raw(0, 0, "-3.5").unwrap();
        t.set_cell_raw(1, 0, "lots").unwrap();
        assert_eq!(t.datasets[0].subcolumns[0].values[0], -3.5);
        assert_eq!(t.datasets[0].subcolumns[0].text[&1], "lots");
        let cv = t.cell_view(1, &t.datasets[0].subcolumns[0]);
        assert_eq!(cv.t.as_deref(), Some("lots"));
    }

    #[test]
    fn insert_and_delete_rows_shift_sparse_maps() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Column,
            "T".into(),
            1,
            SubcolumnFormat::Values(1),
            5,
        );
        t.set_cell_raw(3, 0, "abc").unwrap();
        t.datasets[0].subcolumns[0].excluded.insert(4);
        t.insert_rows(1, 2);
        assert_eq!(t.rows, 7);
        assert_eq!(t.datasets[0].subcolumns[0].text[&5], "abc");
        assert!(t.datasets[0].subcolumns[0].excluded.contains(&6));
        t.delete_rows(0, 3);
        assert_eq!(t.rows, 4);
        assert_eq!(t.datasets[0].subcolumns[0].text[&2], "abc");
        assert!(t.datasets[0].subcolumns[0].excluded.contains(&3));
    }

    #[test]
    fn set_format_restructures() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Grouped,
            "T".into(),
            2,
            SubcolumnFormat::Values(1),
            2,
        );
        t.set_cell_raw(0, 0, "1").unwrap();
        t.set_format(SubcolumnFormat::MeanSdN);
        assert_eq!(t.col_count(), 6);
        assert_eq!(t.datasets[0].subcolumns[0].values[0], 1.0);
    }

    #[test]
    fn cell_formula_input() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Column,
            "T".into(),
            2,
            SubcolumnFormat::Values(1),
            2,
        );
        t.set_cell_raw(0, 1, "=A1*2").unwrap();
        assert_eq!(t.datasets[1].subcolumns[0].cell_formulas[&0], "A1*2");
        t.datasets[1].subcolumns[0].formula = Some("A".into());
        assert!(t.set_cell_raw(1, 1, "3").is_err());
    }
}
