//! Putting a file's data into an existing table (`ReplaceTableData`): re-running a project
//! on new data by hand.
//!
//! The file is read as a table of the existing table's kind (plate-reader exports bring their
//! own kind, which must match). The existing table keeps its id, title, data set names,
//! subcolumn titles and column formulas, so its analyses, graphs and formula columns stay
//! linked. Data columns are filled left to right, skipping formula columns; extra columns of
//! the file become new data sets, and columns the file does not reach are cleared.

use crate::api::TableKind;
use crate::import::{self, Origin, PasteReport};
use crate::plate::{self, PlateMap};
use crate::table::{Column, Table};

/// Read a data file as one table of `kind`.
pub fn file_table(
    name: &str,
    bytes: Vec<u8>,
    kind: TableKind,
    map: Option<&PlateMap>,
) -> Result<(Table, PasteReport), String> {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let mut report = PasteReport::default();
    if let Some(export) = plate::read(name, &bytes)? {
        let t = plate::build_table("new".into(), file.into(), file, &export, map, &mut report)?;
        return Ok((t, report));
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => {
            let (block, mut report, sheet) = import::read_workbook(bytes, None)?;
            let origin = Origin::File {
                file,
                sheet: Some(&sheet),
            };
            let t = import::table_from_block(
                "new".into(),
                kind,
                file.into(),
                &block,
                &origin,
                &mut report,
            );
            Ok((t, report))
        }
        "pzfx" | "prism" => {
            let mut n = 0;
            let (tables, notes) = import::read_prism_file(&bytes, || {
                n += 1;
                format!("new{n}")
            })?;
            let t = tables
                .into_iter()
                .find(|t| t.kind == kind)
                .ok_or_else(|| format!("{file} holds no table of the same kind"))?;
            report.notes = notes;
            Ok((t, report))
        }
        _ => {
            let text = plate::decode_text(&bytes);
            let mut opts = crate::api::PasteOptions::default();
            if ext == "tsv" || ext == "tab" {
                opts.separator = crate::api::Separator::Tab;
            }
            let (block, mut report) = import::parse_delimited(&text, opts);
            if block.cols == 0 {
                return Err(format!("{file} holds no data"));
            }
            let origin = Origin::File { file, sheet: None };
            let t = import::table_from_block(
                "new".into(),
                kind,
                file.into(),
                &block,
                &origin,
                &mut report,
            );
            Ok((t, report))
        }
    }
}

fn kind_name(k: TableKind) -> &'static str {
    match k {
        TableKind::Xy => "an XY",
        TableKind::Column => "a column",
        TableKind::Grouped => "a grouped",
        TableKind::Contingency => "a contingency",
        TableKind::Survival => "a survival",
        TableKind::Parts => "a parts-of-whole",
        TableKind::Variables => "a multiple-variables",
        TableKind::Nested => "a nested",
    }
}

fn take_data(slot: &mut Column, from: &Column) {
    slot.values = from.values.clone();
    slot.text = from.text.clone();
    slot.excluded = from.excluded.clone();
    slot.source = from.source.clone();
    slot.cell_formulas.clear();
    slot.edits.clear();
    slot.errors.clear();
    if slot.title.is_empty() {
        slot.title = from.title.clone();
    }
}

fn clear_data(slot: &mut Column, rows: usize) {
    *slot = Column {
        title: std::mem::take(&mut slot.title),
        decimals: slot.decimals,
        ..Column::empty(rows)
    };
}

/// `old` with the data of `new`; the notes say what did not line up.
pub fn merge(old: &Table, new: &Table) -> Result<(Table, Vec<String>), String> {
    if old.kind != new.kind {
        return Err(format!(
            "the file gives {} table and \"{}\" is {} table",
            kind_name(new.kind),
            old.title,
            kind_name(old.kind)
        ));
    }
    let mut notes = Vec::new();
    let rows = new.rows;
    let mut t = old.clone();
    t.rows = rows;
    if let (Some(x), Some(nx)) = (t.x.as_mut(), new.x.as_ref()) {
        if !x.has_formula() {
            take_data(x, nx);
        }
    }
    if new.has_row_titles() {
        t.row_titles = new.row_titles.clone();
    } else {
        t.row_titles.truncate(rows);
    }
    let sources: Vec<&Column> = new.datasets.iter().flat_map(|d| &d.subcolumns).collect();
    let mut k = 0;
    let mut cleared = 0;
    for d in &mut t.datasets {
        for c in &mut d.subcolumns {
            if c.has_formula() {
                continue;
            }
            match sources.get(k) {
                Some(s) => take_data(c, s),
                None => {
                    if c.values.iter().any(|v| !v.is_nan()) || !c.text.is_empty() {
                        cleared += 1;
                    }
                    clear_data(c, rows);
                }
            }
            k += 1;
        }
    }
    if cleared > 0 {
        notes.push(format!(
            "The file has fewer columns than \"{}\"; {cleared} column{} cleared.",
            old.title,
            if cleared == 1 { " was" } else { "s were" }
        ));
    }
    if k < sources.len() {
        // Extra columns: new data sets in the table's own format.
        let fmt = t.default_format();
        let per = crate::table::subcolumn_count(fmt).max(1);
        let extra = &sources[k..];
        let first_new = t.datasets.len();
        for chunk in extra.chunks(per) {
            let idx = t.push_dataset(None, fmt);
            for (c, s) in t.datasets[idx].subcolumns.iter_mut().zip(chunk) {
                take_data(c, s);
            }
        }
        // Name them after the file's data set of their first column.
        let names: Vec<&str> = new
            .datasets
            .iter()
            .flat_map(|nd| nd.subcolumns.iter().map(move |_| nd.name.as_str()))
            .collect();
        for (i, d) in t.datasets[first_new..].iter_mut().enumerate() {
            if let Some(n) = names.get(k + i * per) {
                d.name = n.to_string();
            }
        }
        notes.push(format!(
            "The file has more columns than \"{}\"; {} data set{} added.",
            old.title,
            t.datasets.len() - first_new,
            if t.datasets.len() - first_new == 1 {
                " was"
            } else {
                "s were"
            }
        ));
    }
    for c in t.columns_mut() {
        c.resize(rows);
    }
    Ok((t, notes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SubcolumnFormat;

    #[test]
    fn kinds_must_match() {
        let a = Table::new(
            "a".into(),
            TableKind::Xy,
            "A".into(),
            1,
            SubcolumnFormat::Values(1),
            3,
        );
        let b = Table::new(
            "b".into(),
            TableKind::Column,
            "B".into(),
            1,
            SubcolumnFormat::Values(1),
            3,
        );
        assert!(merge(&a, &b).unwrap_err().contains("column table"));
    }
}
