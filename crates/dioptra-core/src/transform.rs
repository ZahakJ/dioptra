//! Whole-table transforms that produce derived tables (DESIGN §4.2).
//!
//! A derived table stores its recipe (`derived_from: {table, op}`) and is rebuilt from the
//! source whenever the source changes. Derived tables are read-only; the source is never
//! modified. Row and column numbers in operations are zero-based flat indices, like the
//! rest of the API; formulas inside `FilterRows` use the formula language (1-based rows).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::api::{SubcolumnFormat, TableKind, VariableType};
use crate::formula::{self, Value};
use crate::table::{ColLoc, Column, ColumnSource, Dataset, Table};
use crate::util::format_number;

/// Which value a baseline is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BaselineSpec {
    /// The first row of each column.
    FirstRow,
    /// The mean of the given (zero-based) rows of each column.
    MeanOfRows {
        /// Rows.
        rows: Vec<usize>,
    },
    /// Row by row, the value in another (flat) column.
    Column {
        /// Flat column index of the baseline column.
        col: usize,
    },
    /// The row with this row title.
    RowTitle {
        /// Title.
        title: String,
    },
}

/// How the baseline is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineMode {
    /// `x - b`
    Subtract,
    /// `x / b`
    Divide,
    /// `100 * x / b`
    Percent,
}

/// What a fraction is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FractionBy {
    /// Each row's total.
    Row,
    /// Each column's total.
    Column,
    /// The grand total.
    Table,
}

/// A reference value for normalisation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NormRef {
    /// Smallest value in the column.
    Min,
    /// Largest value in the column.
    Max,
    /// First row's value.
    FirstRow,
    /// Last non-empty row's value.
    LastRow,
    /// A fixed number.
    Value {
        /// The number.
        value: f64,
    },
}

/// A whole-table operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TransformOp {
    /// Rows become datasets and datasets become rows.
    Transpose,
    /// Long to wide: one row per id, one dataset per key.
    PivotLongToWide {
        /// Flat column holding the row identifier.
        id_col: usize,
        /// Flat column holding the key that becomes the dataset.
        key_col: usize,
        /// Flat column holding the value.
        value_col: usize,
    },
    /// Wide to long: a `variables` table of (id, variable, value).
    PivotWideToLong {
        /// Flat column used as the identifier (`None`: row title, else row number).
        #[serde(default)]
        id_col: Option<usize>,
    },
    /// Keep rows where the predicate is true.
    FilterRows {
        /// Formula, e.g. `A > 0 && !isna(B)`.
        predicate: String,
    },
    /// Sort rows by a column.
    Sort {
        /// Flat column.
        col: usize,
        /// Largest first.
        #[serde(default)]
        descending: bool,
    },
    /// Subtract or divide by a baseline.
    RemoveBaseline {
        /// The baseline.
        baseline: BaselineSpec,
        /// How to remove it.
        mode: BaselineMode,
    },
    /// Each value as a fraction of a total.
    FractionOfTotal {
        /// Row, column or table totals.
        by: FractionBy,
    },
    /// Keep every k-th row starting at `offset`.
    Prune {
        /// Keep one row in this many.
        keep_every: usize,
        /// First row kept.
        #[serde(default)]
        offset: usize,
    },
    /// Replicate subcolumns become mean, SD and N.
    AverageReplicates,
    /// Rows `[start, end)`.
    SelectRows {
        /// First row.
        start: usize,
        /// One past the last row.
        end: usize,
    },
    /// Only these flat columns (each Y column becomes its own dataset).
    SelectColumns {
        /// Flat columns.
        columns: Vec<usize>,
    },
    /// Columns to a long `variables` table of (group, value); empty cells are dropped.
    Stack {
        /// Flat columns (empty = every Y column).
        #[serde(default)]
        columns: Vec<usize>,
    },
    /// Scale each column so that `lo` maps to 0 and `hi` to 1 (or 100%).
    Normalize {
        /// Value mapped to 0.
        lo: NormRef,
        /// Value mapped to 1.
        hi: NormRef,
        /// Multiply by 100.
        #[serde(default)]
        as_percent: bool,
    },
}

impl TransformOp {
    /// Short name, used in titles and column provenance.
    pub fn name(&self) -> &'static str {
        match self {
            TransformOp::Transpose => "transpose",
            TransformOp::PivotLongToWide { .. } => "pivot long to wide",
            TransformOp::PivotWideToLong { .. } => "pivot wide to long",
            TransformOp::FilterRows { .. } => "filter rows",
            TransformOp::Sort { .. } => "sort",
            TransformOp::RemoveBaseline { .. } => "remove baseline",
            TransformOp::FractionOfTotal { .. } => "fraction of total",
            TransformOp::Prune { .. } => "prune rows",
            TransformOp::AverageReplicates => "average replicates",
            TransformOp::SelectRows { .. } => "select rows",
            TransformOp::SelectColumns { .. } => "select columns",
            TransformOp::Stack { .. } => "stack",
            TransformOp::Normalize { .. } => "normalize",
        }
    }
}

/// A copy of the table's data with formulas, edits and provenance replaced by
/// `derived` provenance.
fn snapshot(src: &Table, op: &TransformOp) -> Table {
    let mut t = src.clone();
    t.derived_from = None;
    t.dirty.clear();
    t.stale = false;
    for c in t.columns_mut() {
        freeze(c, &src.id, op);
    }
    t
}

fn freeze(c: &mut Column, src: &str, op: &TransformOp) {
    c.formula = None;
    c.cell_formulas.clear();
    c.edits.clear();
    c.errors.clear();
    c.formula_error = None;
    c.source = ColumnSource::Derived {
        table: src.to_string(),
        op: op.name().into(),
    };
}

fn empty_like(src: &Table, kind: TableKind) -> Table {
    Table::new(
        String::new(),
        kind,
        String::new(),
        0,
        SubcolumnFormat::Single,
        0,
    )
    .with_source_title(&src.title)
}

impl Table {
    fn with_source_title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }
}

/// Text key of a cell for grouping: the text, or the number formatted shortest.
fn key_of(c: &Column, row: usize) -> Option<String> {
    let v = c.values.get(row).copied().unwrap_or(f64::NAN);
    if !v.is_nan() {
        Some(format_number(v, None))
    } else {
        c.text.get(&row).cloned()
    }
}

/// Keep the given rows (in this order) of every column.
fn select_rows(src: &Table, rows: &[usize], op: &TransformOp) -> Table {
    let mut t = snapshot(src, op);
    let pick = |c: &Column| -> Column {
        let mut n = Column {
            title: c.title.clone(),
            decimals: c.decimals,
            source: c.source.clone(),
            ..Default::default()
        };
        n.values = rows.iter().map(|&r| c.values[r]).collect();
        for (i, &r) in rows.iter().enumerate() {
            if let Some(s) = c.text.get(&r) {
                n.text.insert(i, s.clone());
            }
            if c.excluded.contains(&r) {
                n.excluded.insert(i);
            }
        }
        n
    };
    for c in t.columns_mut() {
        *c = pick(c);
    }
    t.row_titles = if src.has_row_titles() {
        rows.iter().map(|&r| src.row_title(r).to_string()).collect()
    } else {
        Vec::new()
    };
    t.rows = rows.len();
    t
}

/// Y columns (flat indices) of a table.
fn y_cols(t: &Table) -> Vec<usize> {
    (usize::from(t.x.is_some())..t.col_count()).collect()
}

fn map_y(
    src: &Table,
    op: &TransformOp,
    f: impl Fn(usize, &Column) -> Result<Vec<f64>, String>,
) -> Result<Table, String> {
    let mut t = snapshot(src, op);
    for flat in y_cols(src) {
        let c = src.col(flat).expect("column");
        let v = f(flat, c)?;
        t.col_mut(flat).expect("column").values = v;
    }
    Ok(t)
}

fn norm_ref(c: &Column, r: NormRef) -> f64 {
    let usable: Vec<f64> = c.usable().collect();
    match r {
        NormRef::Min => usable.iter().copied().fold(f64::NAN, f64::min),
        NormRef::Max => usable.iter().copied().fold(f64::NAN, f64::max),
        NormRef::FirstRow => c.usable_at(0),
        NormRef::LastRow => (0..c.values.len())
            .rev()
            .map(|i| c.usable_at(i))
            .find(|v| !v.is_nan())
            .unwrap_or(f64::NAN),
        NormRef::Value { value } => value,
    }
}

/// Apply a transform to a source table. The result has an empty id; the caller assigns
/// the id, title and `derived_from`.
pub fn apply(src: &Table, op: &TransformOp) -> Result<Table, String> {
    let mut out = match op {
        TransformOp::Transpose => transpose(src, op)?,
        TransformOp::PivotLongToWide {
            id_col,
            key_col,
            value_col,
        } => pivot_wide(src, *id_col, *key_col, *value_col, op)?,
        TransformOp::PivotWideToLong { id_col } => {
            let cols: Vec<usize> = (0..src.col_count())
                .filter(|c| Some(*c) != *id_col)
                .collect();
            long(src, &cols, *id_col, true, op)?
        }
        TransformOp::Stack { columns } => {
            let cols = if columns.is_empty() {
                y_cols(src)
            } else {
                columns.clone()
            };
            long(src, &cols, None, false, op)?
        }
        TransformOp::FilterRows { predicate } => {
            let e = formula::parse(predicate).map_err(|e| e.to_string())?;
            let c = formula::compile(&e, src)?;
            let mut keep = Vec::new();
            for r in 0..src.rows {
                match c.at(r) {
                    Value::Bool(true) => keep.push(r),
                    Value::Number(x) if x != 0.0 => keep.push(r),
                    Value::Vector(_) => {
                        return Err("the filter must give one true/false per row".into())
                    }
                    _ => {}
                }
            }
            select_rows(src, &keep, op)
        }
        TransformOp::Sort { col, descending } => {
            let c = src.col(*col).ok_or("no such column")?;
            let mut rows: Vec<usize> = (0..src.rows).collect();
            // Numbers, then text, then empty; empties stay last either way.
            let rank = |r: usize| -> (u8, f64, String) {
                let v = c.values[r];
                if !v.is_nan() {
                    (0, v, String::new())
                } else if let Some(t) = c.text.get(&r) {
                    (1, 0.0, t.clone())
                } else {
                    (2, 0.0, String::new())
                }
            };
            rows.sort_by(|&a, &b| {
                let (ka, kb) = (rank(a), rank(b));
                let o =
                    ka.0.cmp(&kb.0)
                        .then(ka.1.partial_cmp(&kb.1).expect("finite"))
                        .then(ka.2.cmp(&kb.2));
                if *descending && ka.0 < 2 && kb.0 < 2 && ka.0 == kb.0 {
                    o.reverse()
                } else {
                    o
                }
            });
            select_rows(src, &rows, op)
        }
        TransformOp::Prune { keep_every, offset } => {
            if *keep_every == 0 {
                return Err("keep every must be at least 1".into());
            }
            let rows: Vec<usize> = (*offset..src.rows).step_by(*keep_every).collect();
            select_rows(src, &rows, op)
        }
        TransformOp::SelectRows { start, end } => {
            let rows: Vec<usize> = (*start..(*end).min(src.rows)).collect();
            select_rows(src, &rows, op)
        }
        TransformOp::SelectColumns { columns } => select_columns(src, columns, op)?,
        TransformOp::RemoveBaseline { baseline, mode } => {
            let base_col = match baseline {
                BaselineSpec::Column { col } => {
                    Some(src.col(*col).ok_or("no such baseline column")?)
                }
                _ => None,
            };
            let title_row = match baseline {
                BaselineSpec::RowTitle { title } => Some(
                    (0..src.rows)
                        .find(|&r| src.row_title(r) == title)
                        .ok_or_else(|| format!("no row titled \"{title}\""))?,
                ),
                _ => None,
            };
            map_y(src, op, |_, c| {
                let fixed = match baseline {
                    BaselineSpec::FirstRow => Some(c.usable_at(0)),
                    BaselineSpec::MeanOfRows { rows } => {
                        let v: Vec<f64> = rows
                            .iter()
                            .map(|&r| c.usable_at(r))
                            .filter(|v| !v.is_nan())
                            .collect();
                        Some(if v.is_empty() {
                            f64::NAN
                        } else {
                            formula::mean(&v)
                        })
                    }
                    BaselineSpec::RowTitle { .. } => Some(c.usable_at(title_row.expect("row"))),
                    BaselineSpec::Column { .. } => None,
                };
                Ok((0..c.values.len())
                    .map(|r| {
                        let b = fixed.unwrap_or_else(|| base_col.expect("col").usable_at(r));
                        let x = c.values[r];
                        match mode {
                            BaselineMode::Subtract => x - b,
                            BaselineMode::Divide => {
                                if b == 0.0 {
                                    f64::NAN
                                } else {
                                    x / b
                                }
                            }
                            BaselineMode::Percent => {
                                if b == 0.0 {
                                    f64::NAN
                                } else {
                                    100.0 * x / b
                                }
                            }
                        }
                    })
                    .collect())
            })?
        }
        TransformOp::FractionOfTotal { by } => {
            let ys = y_cols(src);
            let total: f64 = ys
                .iter()
                .map(|&f| src.col(f).expect("col").usable().sum::<f64>())
                .sum();
            let row_tot: Vec<f64> = (0..src.rows)
                .map(|r| {
                    ys.iter()
                        .map(|&f| src.col(f).expect("col").usable_at(r))
                        .filter(|v| !v.is_nan())
                        .sum()
                })
                .collect();
            map_y(src, op, |_, c| {
                let col_tot: f64 = c.usable().sum();
                Ok((0..c.values.len())
                    .map(|r| {
                        let d = match by {
                            FractionBy::Column => col_tot,
                            FractionBy::Row => row_tot[r],
                            FractionBy::Table => total,
                        };
                        if d == 0.0 {
                            f64::NAN
                        } else {
                            c.values[r] / d
                        }
                    })
                    .collect())
            })?
        }
        TransformOp::Normalize { lo, hi, as_percent } => map_y(src, op, |_, c| {
            let (l, h) = (norm_ref(c, *lo), norm_ref(c, *hi));
            let k = if *as_percent { 100.0 } else { 1.0 };
            Ok(c.values
                .iter()
                .map(|x| {
                    if h == l {
                        f64::NAN
                    } else {
                        k * (x - l) / (h - l)
                    }
                })
                .collect())
        })?,
        TransformOp::AverageReplicates => average(src, op),
    };
    out.id = String::new();
    out.title = format!("{} ({})", src.title, op.name());
    Ok(out)
}

fn transpose(src: &Table, op: &TransformOp) -> Result<Table, String> {
    let kind = match src.kind {
        TableKind::Variables => TableKind::Variables,
        TableKind::Contingency => TableKind::Contingency,
        _ => TableKind::Grouped,
    };
    let ys = y_cols(src);
    let mut t = empty_like(src, kind);
    t.rows = ys.len();
    for r in 0..src.rows {
        let name = if !src.row_title(r).is_empty() {
            src.row_title(r).to_string()
        } else if let Some(x) = src.x.as_ref().and_then(|x| key_of(x, r)) {
            x
        } else {
            format!("Row {}", r + 1)
        };
        let mut c = Column::empty(ys.len());
        for (i, &f) in ys.iter().enumerate() {
            let s = src.col(f).expect("col");
            c.values[i] = s.values[r];
            if let Some(tx) = s.text.get(&r) {
                c.text.insert(i, tx.clone());
            }
            if s.excluded.contains(&r) {
                c.excluded.insert(i);
            }
        }
        freeze(&mut c, &src.id, op);
        t.datasets.push(Dataset {
            name,
            format: if kind == TableKind::Grouped {
                SubcolumnFormat::Values(1)
            } else {
                SubcolumnFormat::Single
            },
            subcolumns: vec![c],
            var_type: (kind == TableKind::Variables).then_some(VariableType::Number),
        });
    }
    t.row_titles = ys.iter().map(|&f| src.col_header(f)).collect();
    Ok(t)
}

fn pivot_wide(
    src: &Table,
    id_col: usize,
    key_col: usize,
    value_col: usize,
    op: &TransformOp,
) -> Result<Table, String> {
    let (ic, kc, vc) = (
        src.col(id_col).ok_or("no such id column")?,
        src.col(key_col).ok_or("no such key column")?,
        src.col(value_col).ok_or("no such value column")?,
    );
    let mut ids: Vec<String> = Vec::new();
    let mut id_ix: BTreeMap<String, usize> = BTreeMap::new();
    let mut keys: Vec<String> = Vec::new();
    let mut key_ix: BTreeMap<String, usize> = BTreeMap::new();
    let mut cells: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for r in 0..src.rows {
        let (Some(id), Some(k)) = (key_of(ic, r), key_of(kc, r)) else {
            continue;
        };
        let i = *id_ix.entry(id.clone()).or_insert_with(|| {
            ids.push(id.clone());
            ids.len() - 1
        });
        let j = *key_ix.entry(k.clone()).or_insert_with(|| {
            keys.push(k.clone());
            keys.len() - 1
        });
        if cells.insert((i, j), r).is_some() {
            return Err(format!(
                "id \"{id}\" has more than one value for \"{k}\"; a pivot needs one value per pair"
            ));
        }
    }
    let mut t = empty_like(src, TableKind::Grouped);
    t.rows = ids.len();
    for k in &keys {
        let mut c = Column::empty(ids.len());
        freeze(&mut c, &src.id, op);
        t.datasets.push(Dataset {
            name: k.clone(),
            format: SubcolumnFormat::Values(1),
            subcolumns: vec![c],
            var_type: None,
        });
    }
    for ((i, j), r) in cells {
        let c = &mut t.datasets[j].subcolumns[0];
        c.values[i] = vc.values[r];
        if let Some(tx) = vc.text.get(&r) {
            c.text.insert(i, tx.clone());
        }
        if vc.excluded.contains(&r) {
            c.excluded.insert(i);
        }
    }
    t.row_titles = ids;
    Ok(t)
}

fn long(
    src: &Table,
    cols: &[usize],
    id_col: Option<usize>,
    keep_empty: bool,
    op: &TransformOp,
) -> Result<Table, String> {
    let mut t = empty_like(src, TableKind::Variables);
    let mut id = Column::default();
    let mut var = Column::default();
    let mut val = Column::default();
    let mut n = 0usize;
    for &f in cols {
        let c = src.col(f).ok_or_else(|| format!("no column {f}"))?;
        let header = src.col_header(f);
        for r in 0..src.rows {
            let empty = c.values[r].is_nan() && !c.text.contains_key(&r);
            if empty && !keep_empty {
                continue;
            }
            match id_col {
                Some(ic) => {
                    let s = src.col(ic).ok_or("no such id column")?;
                    id.values.push(s.values[r]);
                    if let Some(tx) = s.text.get(&r) {
                        id.text.insert(n, tx.clone());
                    }
                }
                None if keep_empty => {
                    if src.row_title(r).is_empty() {
                        id.values.push((r + 1) as f64);
                    } else {
                        id.values.push(f64::NAN);
                        id.text.insert(n, src.row_title(r).to_string());
                    }
                }
                None => {}
            }
            var.values.push(f64::NAN);
            var.text.insert(n, header.clone());
            val.values.push(c.values[r]);
            if let Some(tx) = c.text.get(&r) {
                val.text.insert(n, tx.clone());
            }
            if c.excluded.contains(&r) {
                val.excluded.insert(n);
            }
            n += 1;
        }
    }
    t.rows = n;
    let mut push = |name: &str, mut c: Column, ty: VariableType| {
        freeze(&mut c, &src.id, op);
        t.datasets.push(Dataset {
            name: name.into(),
            format: SubcolumnFormat::Single,
            subcolumns: vec![c],
            var_type: Some(ty),
        });
    };
    if keep_empty {
        let ty = if id.text.is_empty() {
            VariableType::Number
        } else {
            VariableType::Category
        };
        push("id", id, ty);
        push("variable", var, VariableType::Category);
    } else {
        push("group", var, VariableType::Category);
    }
    push("value", val, VariableType::Number);
    Ok(t)
}

fn select_columns(src: &Table, columns: &[usize], op: &TransformOp) -> Result<Table, String> {
    let mut t = snapshot(src, op);
    let keep_x = src.x.is_some() && columns.contains(&0);
    if !keep_x {
        t.x = None;
    }
    t.datasets.clear();
    let fmt = if crate::table::forced_format(src.kind).is_some() {
        SubcolumnFormat::Single
    } else {
        SubcolumnFormat::Values(1)
    };
    for &f in columns {
        match src.locate(f) {
            Some(ColLoc::X) => {}
            Some(ColLoc::Y { dataset, .. }) => {
                let mut c = src.col(f).expect("col").clone();
                freeze(&mut c, &src.id, op);
                t.datasets.push(Dataset {
                    name: src.col_header(f),
                    format: fmt,
                    subcolumns: vec![c],
                    var_type: src.datasets[dataset].var_type,
                });
            }
            None => return Err(format!("no column {f}")),
        }
    }
    if crate::table::kind_has_x(t.kind) && t.x.is_none() {
        // Without its X column an XY table is a column table.
        t.kind = TableKind::Column;
    }
    Ok(t)
}

fn average(src: &Table, op: &TransformOp) -> Table {
    let mut t = snapshot(src, op);
    for (d, ds) in src.datasets.iter().enumerate() {
        if !matches!(ds.format, SubcolumnFormat::Values(_)) {
            continue;
        }
        let mut m = Column::empty(src.rows);
        let mut s = Column::empty(src.rows);
        let mut n = Column::empty(src.rows);
        for r in 0..src.rows {
            let v: Vec<f64> = ds
                .subcolumns
                .iter()
                .map(|c| c.usable_at(r))
                .filter(|v| !v.is_nan())
                .collect();
            if v.is_empty() {
                continue;
            }
            m.values[r] = formula::mean(&v);
            if v.len() > 1 {
                s.values[r] = formula::variance(&v).sqrt();
            }
            n.values[r] = v.len() as f64;
        }
        let mut cols = vec![m, s, n];
        for c in &mut cols {
            freeze(c, &src.id, op);
        }
        t.datasets[d] = Dataset {
            name: ds.name.clone(),
            format: SubcolumnFormat::MeanSdN,
            subcolumns: cols,
            var_type: ds.var_type,
        };
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src() -> Table {
        let mut t = Table::new(
            "s".into(),
            TableKind::Column,
            "S".into(),
            2,
            SubcolumnFormat::Values(1),
            0,
        );
        for (r, (a, b)) in [("1", "10"), ("2", "20"), ("3", "x"), ("4", "40")]
            .iter()
            .enumerate()
        {
            t.set_cell_raw(r, 0, a).unwrap();
            t.set_cell_raw(r, 1, b).unwrap();
        }
        t
    }

    fn vals(t: &Table, flat: usize) -> Vec<f64> {
        t.col(flat).unwrap().values.clone()
    }

    #[test]
    fn transpose_swaps() {
        let t = apply(&src(), &TransformOp::Transpose).unwrap();
        assert_eq!(t.rows, 2);
        assert_eq!(t.datasets.len(), 4);
        assert_eq!(vals(&t, 0), vec![1.0, 10.0]);
        assert_eq!(t.datasets[2].subcolumns[0].text[&1], "x");
        assert_eq!(t.row_titles, vec!["Data set A", "Data set B"]);
    }

    #[test]
    fn filter_sort_prune_select() {
        let s = src();
        let f = apply(
            &s,
            &TransformOp::FilterRows {
                predicate: "A > 1 && B > 15".into(),
            },
        )
        .unwrap();
        assert_eq!(vals(&f, 0), vec![2.0, 4.0]);
        let o = apply(
            &s,
            &TransformOp::Sort {
                col: 0,
                descending: true,
            },
        )
        .unwrap();
        assert_eq!(vals(&o, 0), vec![4.0, 3.0, 2.0, 1.0]);
        assert_eq!(o.datasets[1].subcolumns[0].text[&1], "x");
        let p = apply(
            &s,
            &TransformOp::Prune {
                keep_every: 2,
                offset: 1,
            },
        )
        .unwrap();
        assert_eq!(vals(&p, 0), vec![2.0, 4.0]);
        let r = apply(&s, &TransformOp::SelectRows { start: 1, end: 3 }).unwrap();
        assert_eq!(vals(&r, 0), vec![2.0, 3.0]);
        let c = apply(&s, &TransformOp::SelectColumns { columns: vec![1] }).unwrap();
        assert_eq!(c.datasets.len(), 1);
        assert_eq!(c.datasets[0].name, "Data set B");
    }

    #[test]
    fn baseline_fraction_normalize() {
        let s = src();
        let b = apply(
            &s,
            &TransformOp::RemoveBaseline {
                baseline: BaselineSpec::FirstRow,
                mode: BaselineMode::Subtract,
            },
        )
        .unwrap();
        assert_eq!(vals(&b, 0), vec![0.0, 1.0, 2.0, 3.0]);
        let b = apply(
            &s,
            &TransformOp::RemoveBaseline {
                baseline: BaselineSpec::MeanOfRows { rows: vec![0, 1] },
                mode: BaselineMode::Percent,
            },
        )
        .unwrap();
        assert_eq!(
            vals(&b, 0),
            vec![100.0 / 1.5, 200.0 / 1.5, 300.0 / 1.5, 400.0 / 1.5]
        );
        let b = apply(
            &s,
            &TransformOp::RemoveBaseline {
                baseline: BaselineSpec::Column { col: 0 },
                mode: BaselineMode::Divide,
            },
        )
        .unwrap();
        assert_eq!(vals(&b, 1)[1], 10.0);
        let f = apply(
            &s,
            &TransformOp::FractionOfTotal {
                by: FractionBy::Column,
            },
        )
        .unwrap();
        assert_eq!(vals(&f, 0), vec![0.1, 0.2, 0.3, 0.4]);
        let f = apply(
            &s,
            &TransformOp::FractionOfTotal {
                by: FractionBy::Row,
            },
        )
        .unwrap();
        assert_eq!(vals(&f, 0)[2], 1.0);
        let n = apply(
            &s,
            &TransformOp::Normalize {
                lo: NormRef::Min,
                hi: NormRef::Max,
                as_percent: true,
            },
        )
        .unwrap();
        let nb = vals(&n, 1);
        assert_eq!((nb[0], nb[1], nb[3]), (0.0, 100.0 / 3.0, 100.0));
        assert!(nb[2].is_nan());
        assert_eq!(vals(&n, 0), vec![0.0, 100.0 / 3.0, 200.0 / 3.0, 100.0]);
    }

    #[test]
    fn pivots_and_stack() {
        let s = src();
        let st = apply(&s, &TransformOp::Stack { columns: vec![] }).unwrap();
        assert_eq!(st.kind, TableKind::Variables);
        assert_eq!(st.rows, 8);
        assert_eq!(st.datasets[0].subcolumns[0].text[&7], "Data set B");
        let long = apply(&s, &TransformOp::PivotWideToLong { id_col: None }).unwrap();
        assert_eq!(long.rows, 8);
        assert_eq!(long.datasets.len(), 3);
        // Back to wide: id = flat 0, key = 1, value = 2.
        let wide = apply(
            &long,
            &TransformOp::PivotLongToWide {
                id_col: 0,
                key_col: 1,
                value_col: 2,
            },
        )
        .unwrap();
        assert_eq!(wide.rows, 4);
        assert_eq!(wide.datasets.len(), 2);
        assert_eq!(vals(&wide, 1)[3], 40.0);
        assert_eq!(wide.datasets[1].subcolumns[0].text[&2], "x");
        let dup = apply(
            &st,
            &TransformOp::PivotLongToWide {
                id_col: 0,
                key_col: 0,
                value_col: 1,
            },
        );
        assert!(dup.is_err());
    }

    #[test]
    fn average_replicates() {
        let mut t = Table::new(
            "s".into(),
            TableKind::Xy,
            "S".into(),
            1,
            SubcolumnFormat::Values(3),
            0,
        );
        for (c, v) in ["1", "2", "4", "6"].iter().enumerate() {
            t.set_cell_raw(0, c, v).unwrap();
        }
        let a = apply(&t, &TransformOp::AverageReplicates).unwrap();
        assert_eq!(a.datasets[0].format, SubcolumnFormat::MeanSdN);
        assert_eq!(vals(&a, 1), vec![4.0]);
        assert_eq!(vals(&a, 2), vec![2.0]);
        assert_eq!(vals(&a, 3), vec![3.0]);
    }
}
