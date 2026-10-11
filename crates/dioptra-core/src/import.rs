//! Getting data in: clipboard paste, delimited text, spreadsheets and Prism `.pzfx`.
//!
//! Paste parsing decides the separator, the decimal mark and whether the first row is a
//! header, and says what it decided in a [`PasteReport`] (DESIGN §7). Text that is not a
//! number is kept as text, never dropped (complaint P6).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::api::{
    Changed, DecimalMark, PasteOptions, Separator, SubcolumnFormat, TableKind, VariableType,
};
use crate::table::{forced_format, parse_number, Column, ColumnSource, Dataset, Table};

/// What a paste or import decided and did.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PasteReport {
    /// Separator used.
    pub separator: Separator,
    /// Decimal mark used.
    pub decimal: DecimalMark,
    /// Whether the first row was read as a header.
    pub header_detected: bool,
    /// Data rows (without the header).
    pub rows: usize,
    /// Columns.
    pub cols: usize,
    /// Cells stored as numbers.
    pub numeric_cells: usize,
    /// Cells kept as text.
    pub text_cells: usize,
    /// Empty or missing cells (`NA`, `-`, …).
    pub missing_cells: usize,
    /// Plain-English notes about anything unusual.
    pub notes: Vec<String>,
    /// What changed in the project (filled by the session).
    #[serde(default)]
    pub changed: Changed,
}

/// One parsed cell.
#[derive(Debug, Clone, PartialEq)]
pub enum RawCell {
    /// Empty or a missing-value code.
    Missing,
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
}

/// A parsed rectangular block, stored by column.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedBlock {
    /// Header names, when the first row was a header.
    pub header: Option<Vec<String>>,
    /// Data rows.
    pub rows: usize,
    /// Columns.
    pub cols: usize,
    /// Values by column (NaN = empty or text).
    pub columns: Vec<Vec<f64>>,
    /// Text entries by column, then row.
    pub text: Vec<BTreeMap<usize, String>>,
}

fn is_missing_token(s: &str) -> bool {
    matches!(
        s,
        "" | "-"
            | "."
            | "NA"
            | "N/A"
            | "na"
            | "n/a"
            | "NaN"
            | "nan"
            | "NAN"
            | "#N/A"
            | "null"
            | "NULL"
    )
}

// ---------------------------------------------------------------------------
// Splitting
// ---------------------------------------------------------------------------

fn split_records(text: &str, sep: Separator) -> Vec<Vec<std::borrow::Cow<'_, str>>> {
    use std::borrow::Cow;
    let mut rows: Vec<Vec<Cow<'_, str>>> = Vec::new();
    if sep == Separator::Whitespace {
        for line in text.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            rows.push(line.split_whitespace().map(Cow::Borrowed).collect());
        }
    } else {
        let sepb = match sep {
            Separator::Comma => b',',
            Separator::Semicolon => b';',
            _ => b'\t',
        };
        let b = text.as_bytes();
        let mut i = 0;
        let mut row: Vec<Cow<'_, str>> = Vec::new();
        let n = b.len();
        loop {
            // One field.
            if i < n && b[i] == b'"' {
                let mut s = String::new();
                i += 1;
                let mut start = i;
                loop {
                    if i >= n {
                        s.push_str(&text[start..n]);
                        break;
                    }
                    if b[i] == b'"' {
                        s.push_str(&text[start..i]);
                        if i + 1 < n && b[i + 1] == b'"' {
                            s.push('"');
                            i += 2;
                            start = i;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                // Lenient: anything after the closing quote up to the separator is kept.
                let tail_start = i;
                while i < n && b[i] != sepb && b[i] != b'\n' && b[i] != b'\r' {
                    i += 1;
                }
                s.push_str(&text[tail_start..i]);
                row.push(Cow::Owned(s));
            } else {
                let start = i;
                while i < n && b[i] != sepb && b[i] != b'\n' && b[i] != b'\r' {
                    i += 1;
                }
                row.push(Cow::Borrowed(&text[start..i]));
            }
            if i >= n {
                rows.push(row);
                break;
            }
            match b[i] {
                c if c == sepb => i += 1,
                b'\r' => {
                    i += if i + 1 < n && b[i + 1] == b'\n' { 2 } else { 1 };
                    rows.push(std::mem::take(&mut row));
                    if i >= n {
                        break;
                    }
                }
                _ => {
                    i += 1;
                    rows.push(std::mem::take(&mut row));
                    if i >= n {
                        break;
                    }
                }
            }
        }
    }
    // A line with one empty field is an empty row.
    for r in &mut rows {
        if r.len() == 1 && r[0].is_empty() {
            r.clear();
        }
    }
    while rows.last().is_some_and(|r| r.is_empty()) {
        rows.pop();
    }
    rows
}

fn count_outside_quotes(line: &str, c: u8) -> usize {
    let mut q = false;
    let mut n = 0;
    for &b in line.as_bytes() {
        if b == b'"' {
            q = !q;
        } else if b == c && !q {
            n += 1;
        }
    }
    n
}

fn detect_separator(text: &str) -> Separator {
    if text.contains('\t') {
        return Separator::Tab;
    }
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(100)
        .collect();
    if lines.is_empty() {
        return Separator::Tab;
    }
    let consistent = |c: u8| -> Option<usize> {
        let counts: Vec<usize> = lines.iter().map(|l| count_outside_quotes(l, c)).collect();
        let first = counts[0];
        // Allow a header with a different count only when every data line agrees.
        let data = if counts.len() > 1 {
            &counts[1..]
        } else {
            &counts[..]
        };
        let d0 = data[0];
        if d0 > 0 && data.iter().all(|&k| k == d0) && (first == d0 || first > 0) {
            Some(d0)
        } else if first > 0
            && counts.iter().filter(|&&k| k == first).count() * 10 >= counts.len() * 8
        {
            Some(first)
        } else {
            None
        }
    };
    if consistent(b';').is_some() {
        return Separator::Semicolon;
    }
    if consistent(b',').is_some() {
        return Separator::Comma;
    }
    let ws: Vec<usize> = lines.iter().map(|l| l.split_whitespace().count()).collect();
    if ws[0] >= 2 && ws.iter().all(|&k| k == ws[0]) {
        return Separator::Whitespace;
    }
    if text.contains(',') {
        return Separator::Comma;
    }
    Separator::Tab
}

// ---------------------------------------------------------------------------
// Cell parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Point,
    CommaDecimal,
    Ambiguous,
    Other,
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn strip_sign(s: &str) -> &str {
    s.strip_prefix('-')
        .or_else(|| s.strip_prefix('+'))
        .unwrap_or(s)
}

/// `1,234` (one comma, three digits after) could be a thousands separator or a decimal.
fn comma_shape(s: &str) -> Shape {
    let t = strip_sign(s);
    match t.split_once(',') {
        Some((a, b)) if digits(a) && digits(b) && !b.contains(',') => {
            if b.len() == 3 && a.len() <= 3 && a != "0" {
                Shape::Ambiguous
            } else {
                Shape::CommaDecimal
            }
        }
        _ => {
            if parse_number(s).is_some() {
                Shape::Point
            } else {
                Shape::Other
            }
        }
    }
}

/// Thousands-grouped digits: `1,234,567` with `sep` = ','.
fn grouped(int: &str, sep: char) -> Option<String> {
    let parts: Vec<&str> = int.split(sep).collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[0].len() > 3 || !digits(parts[0]) {
        return None;
    }
    if parts[1..].iter().all(|p| p.len() == 3 && digits(p)) {
        Some(parts.concat())
    } else {
        None
    }
}

/// Parse one field. `ambiguous` counts cells kept as text because their meaning was unclear.
fn parse_cell(raw: &str, decimal: DecimalMark, ambiguous: &mut usize) -> RawCell {
    let s = raw.trim_matches(|c: char| c == ' ' || c == '\u{a0}');
    if is_missing_token(s) {
        return RawCell::Missing;
    }
    let b0 = s.as_bytes()[0];
    if b0 == b'\'' || b0 == b'=' {
        return RawCell::Text(raw.to_string());
    }
    if decimal != DecimalMark::Comma {
        if let Some(v) = parse_number(s) {
            return RawCell::Number(v);
        }
        if s.contains(',') {
            let (sign, t) = if let Some(t) = s.strip_prefix('-') {
                ("-", t)
            } else {
                ("", s.strip_prefix('+').unwrap_or(s))
            };
            let (int, frac) = match t.split_once('.') {
                Some((i, f)) => (i, Some(f)),
                None => (t, None),
            };
            if let Some(g) = grouped(int, ',') {
                // Unambiguous when there is a decimal point or more than one group.
                if frac.is_some() || int.matches(',').count() > 1 {
                    let num = match frac {
                        Some(f) if digits(f) => format!("{sign}{g}.{f}"),
                        Some(_) => return RawCell::Text(raw.to_string()),
                        None => format!("{sign}{g}"),
                    };
                    if let Ok(v) = num.parse::<f64>() {
                        return RawCell::Number(v);
                    }
                } else {
                    *ambiguous += 1;
                }
            }
        }
        return RawCell::Text(raw.to_string());
    }
    // Comma decimal mark.
    if !s.contains('.') {
        if let Some(v) = parse_number(&s.replacen(',', ".", 1)) {
            if s.matches(',').count() <= 1 {
                return RawCell::Number(v);
            }
        }
        return RawCell::Text(raw.to_string());
    }
    let (sign, t) = if let Some(t) = s.strip_prefix('-') {
        ("-", t)
    } else {
        ("", s.strip_prefix('+').unwrap_or(s))
    };
    let (int, frac) = match t.split_once(',') {
        Some((i, f)) => (i, Some(f)),
        None => (t, None),
    };
    if let Some(g) = grouped(int, '.') {
        if frac.is_some() || int.matches('.').count() > 1 {
            let num = match frac {
                Some(f) if digits(f) => format!("{sign}{g}.{f}"),
                Some(_) => return RawCell::Text(raw.to_string()),
                None => format!("{sign}{g}"),
            };
            if let Ok(v) = num.parse::<f64>() {
                return RawCell::Number(v);
            }
        } else {
            *ambiguous += 1;
            return RawCell::Text(raw.to_string());
        }
    }
    // A point decimal among comma decimals: read it, but say so.
    if frac.is_none() {
        if let Some(v) = parse_number(s) {
            return RawCell::Number(v);
        }
    }
    RawCell::Text(raw.to_string())
}

// ---------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------

/// Header heuristic: the first row has text and no numbers, and a later row has numbers.
fn detect_header(rows: &[Vec<RawCell>]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let first = &rows[0];
    let has_text = first.iter().any(|c| matches!(c, RawCell::Text(_)));
    let has_num = first.iter().any(|c| matches!(c, RawCell::Number(_)));
    has_text
        && !has_num
        && rows[1..rows.len().min(21)]
            .iter()
            .any(|r| r.iter().any(|c| matches!(c, RawCell::Number(_))))
}

/// Build a block from parsed rows, filling report counts.
pub fn block_from_rows(
    mut rows: Vec<Vec<RawCell>>,
    header: Option<bool>,
    report: &mut PasteReport,
) -> ParsedBlock {
    let use_header = header.unwrap_or_else(|| detect_header(&rows));
    report.header_detected = use_header;
    let header_row = if use_header && !rows.is_empty() {
        Some(rows.remove(0))
    } else {
        None
    };
    let nrows = rows.len();
    let widths: Vec<usize> = rows.iter().map(Vec::len).collect();
    let mut ncols = widths.iter().copied().max().unwrap_or(0);
    if let Some(h) = &header_row {
        ncols = ncols.max(h.len());
    }
    let (minw, maxw) = (
        widths.iter().copied().filter(|&w| w > 0).min().unwrap_or(0),
        widths.iter().copied().max().unwrap_or(0),
    );
    if minw != maxw && nrows > 1 {
        report.notes.push(format!("Rows have different lengths ({minw} to {maxw} fields); short rows were padded with empty cells."));
    }
    let mut columns = vec![vec![f64::NAN; nrows]; ncols];
    let mut text: Vec<BTreeMap<usize, String>> = vec![BTreeMap::new(); ncols];
    let mut first_text: Option<(usize, usize, String)> = None;
    for (r, row) in rows.into_iter().enumerate() {
        for (c, cell) in row.into_iter().enumerate() {
            match cell {
                RawCell::Number(v) => {
                    columns[c][r] = v;
                    report.numeric_cells += 1;
                }
                RawCell::Text(t) => {
                    report.text_cells += 1;
                    if first_text.is_none() {
                        first_text = Some((r, c, t.clone()));
                    }
                    text[c].insert(r, t);
                }
                RawCell::Missing => {}
            }
        }
    }
    report.rows = nrows;
    report.cols = ncols;
    report.missing_cells = nrows * ncols - report.numeric_cells - report.text_cells;
    if let Some((r, c, t)) = first_text {
        report.notes.push(format!(
            "{} cell{} kept as text (the first is \"{}\" in row {}, column {}).",
            report.text_cells,
            if report.text_cells == 1 {
                " was"
            } else {
                "s were"
            },
            t,
            r + 1,
            c + 1
        ));
    }
    let header = header_row.map(|h| {
        let mut names: Vec<String> = h
            .into_iter()
            .map(|c| match c {
                RawCell::Text(t) => t.trim().to_string(),
                RawCell::Number(v) => crate::util::format_number(v, None),
                RawCell::Missing => String::new(),
            })
            .collect();
        names.resize(ncols, String::new());
        names
    });
    ParsedBlock {
        header,
        rows: nrows,
        cols: ncols,
        columns,
        text,
    }
}

/// Parse clipboard or file text.
pub fn parse_delimited(text: &str, options: PasteOptions) -> (ParsedBlock, PasteReport) {
    let mut report = PasteReport::default();
    let text = match text.strip_prefix('\u{feff}') {
        Some(t) => {
            report
                .notes
                .push("Removed a byte-order mark at the start.".into());
            t
        }
        None => text,
    };
    let sep = if options.separator == Separator::Auto {
        detect_separator(text)
    } else {
        options.separator
    };
    report.separator = sep;
    let records = split_records(text, sep);
    // Decimal mark.
    let decimal = match options.decimal {
        DecimalMark::Auto if sep == Separator::Comma => DecimalMark::Point,
        DecimalMark::Auto => {
            let (mut comma, mut point, mut amb) = (0usize, 0usize, 0usize);
            for f in records.iter().take(2000).flatten() {
                match comma_shape(f.trim()) {
                    Shape::CommaDecimal => comma += 1,
                    Shape::Point if f.contains('.') => point += 1,
                    Shape::Ambiguous => amb += 1,
                    _ => {}
                }
            }
            if comma > 0 && comma * 2 > comma + point + amb {
                report.notes.push(format!(
                    "Commas were read as decimal marks ({comma} values looked like 1,5)."
                ));
                DecimalMark::Comma
            } else {
                DecimalMark::Point
            }
        }
        d => d,
    };
    report.decimal = decimal;
    let mut ambiguous = 0;
    let rows: Vec<Vec<RawCell>> = records
        .iter()
        .map(|r| {
            r.iter()
                .map(|f| parse_cell(f, decimal, &mut ambiguous))
                .collect()
        })
        .collect();
    if ambiguous > 0 {
        report.notes.push(format!(
            "{ambiguous} value{} such as 1,234 could use the comma either as a thousands separator or as a decimal mark; {} kept as text. Choose the decimal mark explicitly to read {}.",
            if ambiguous == 1 { "" } else { "s" },
            if ambiguous == 1 { "it was" } else { "they were" },
            if ambiguous == 1 { "it" } else { "them" }
        ));
    }
    let block = block_from_rows(rows, options.header, &mut report);
    (block, report)
}

// ---------------------------------------------------------------------------
// Into tables
// ---------------------------------------------------------------------------

/// Write a block into a table at (row0, col0), growing rows and adding datasets as needed.
/// Columns with a column formula are skipped (noted). Returns the flat columns written.
pub fn paste_into(
    table: &mut Table,
    row0: usize,
    col0: usize,
    block: &ParsedBlock,
    at: &str,
    report: &mut PasteReport,
) -> BTreeSet<usize> {
    let need = col0 + block.cols;
    let fmt = table.default_format();
    while table.col_count() < need {
        table.push_dataset(None, fmt);
    }
    table.ensure_rows(row0 + block.rows);
    let mut written = BTreeSet::new();
    for j in 0..block.cols {
        let flat = col0 + j;
        let label = table.col_label(flat);
        let first_sub = match table.locate(flat) {
            Some(crate::table::ColLoc::Y { sub, .. }) => sub == 0,
            _ => true,
        };
        let col = table.col_mut(flat).expect("column exists");
        if col.formula.is_some() {
            report.notes.push(format!(
                "Column {label} is computed by a formula; its pasted values were skipped."
            ));
            continue;
        }
        let end = row0 + block.rows;
        col.values[row0..end].copy_from_slice(&block.columns[j]);
        for m in [&mut col.cell_formulas, &mut col.text, &mut col.errors] {
            if m.range(row0..end).next().is_some() {
                let tail = m.split_off(&row0);
                m.extend(tail.into_iter().filter(|(r, _)| *r >= end));
            }
        }
        if col.excluded.range(row0..end).next().is_some() {
            col.excluded.retain(|r| *r < row0 || *r >= end);
        }
        for (r, t) in &block.text[j] {
            col.text.insert(row0 + r, t.clone());
        }
        col.source = ColumnSource::Pasted { at: at.to_string() };
        written.insert(flat);
        if let (Some(h), true) = (&block.header, first_sub) {
            let name = h[j].clone();
            if !name.is_empty() {
                match table.locate(flat) {
                    Some(crate::table::ColLoc::X) => table.x.as_mut().expect("x").title = name,
                    Some(crate::table::ColLoc::Y { dataset, .. }) => {
                        table.datasets[dataset].name = name
                    }
                    None => {}
                }
            }
        }
    }
    written
}

/// Infer a variable type from a column: all numbers → number; ISO dates → date; at most
/// twelve distinct text values → category; otherwise text.
pub fn infer_type(values: &[f64], text: &BTreeMap<usize, String>) -> VariableType {
    if text.is_empty() {
        return VariableType::Number;
    }
    let numbers = values.iter().filter(|v| !v.is_nan()).count();
    if numbers == 0 && text.values().all(|t| is_iso_date(t.trim())) {
        return VariableType::Date;
    }
    let mut distinct: BTreeSet<String> = text.values().map(|t| t.trim().to_lowercase()).collect();
    for v in values.iter().filter(|v| !v.is_nan()) {
        distinct.insert(crate::util::format_number(*v, None));
        if distinct.len() > 12 {
            break;
        }
    }
    if distinct.len() <= 12 {
        VariableType::Category
    } else {
        VariableType::Text
    }
}

/// `YYYY-MM-DD`, optionally followed by `THH:MM[:SS]` or a space and a time.
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 10 {
        return false;
    }
    let date_ok = b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit);
    date_ok
        && (b.len() == 10 || ((b[10] == b'T' || b[10] == b' ') && b.len() >= 16 && b[13] == b':'))
}

/// How a new table's columns are to be labelled with their source.
pub enum Origin<'a> {
    /// From the clipboard / text at a time.
    Pasted(&'a str),
    /// From a file (and sheet).
    File {
        /// File name.
        file: &'a str,
        /// Sheet.
        sheet: Option<&'a str>,
    },
}

/// Build a new table of `kind` from a block.
pub fn table_from_block(
    id: String,
    kind: TableKind,
    title: String,
    block: &ParsedBlock,
    origin: &Origin<'_>,
    report: &mut PasteReport,
) -> Table {
    let mut t = Table::new(id, kind, title, 0, SubcolumnFormat::Values(1), block.rows);
    let name = |j: usize| -> Option<String> {
        block
            .header
            .as_ref()
            .map(|h| h[j].clone())
            .filter(|s| !s.is_empty())
    };
    let col_of = |j: usize| -> Column {
        let source = match origin {
            Origin::Pasted(at) => ColumnSource::Pasted { at: at.to_string() },
            Origin::File { file, sheet } => ColumnSource::Imported {
                file: file.to_string(),
                sheet: sheet.map(str::to_string),
                column: name(j).unwrap_or_else(|| crate::util::letters(j)),
            },
        };
        Column {
            values: block.columns[j].clone(),
            text: block.text[j].clone(),
            source,
            ..Default::default()
        }
    };
    let mut j0 = 0;
    if crate::table::kind_has_x(kind) && block.cols > 0 {
        let mut x = col_of(0);
        x.title = name(0).unwrap_or_else(|| {
            if kind == TableKind::Survival {
                "Time".into()
            } else {
                "X".into()
            }
        });
        t.x = Some(x);
        j0 = 1;
    } else if matches!(
        kind,
        TableKind::Grouped | TableKind::Contingency | TableKind::Parts
    ) && block.cols > 1
    {
        let texts = block.text[0].len();
        let nums = block.columns[0].iter().filter(|v| !v.is_nan()).count();
        if texts > nums {
            t.row_titles = (0..block.rows)
                .map(|r| {
                    block.text[0]
                        .get(&r)
                        .cloned()
                        .unwrap_or_else(|| crate::util::format_number(block.columns[0][r], None))
                })
                .collect();
            report
                .notes
                .push("The first column was used as row titles.".into());
            j0 = 1;
        }
    }
    let fmt = forced_format(kind).unwrap_or(SubcolumnFormat::Values(1));
    for j in j0..block.cols {
        let c = col_of(j);
        let var_type = (kind == TableKind::Variables).then(|| infer_type(&c.values, &c.text));
        let idx = t.datasets.len();
        t.datasets.push(Dataset {
            name: name(j).unwrap_or_else(|| t.default_dataset_name(idx)),
            format: fmt,
            subcolumns: vec![c],
            var_type,
        });
    }
    t
}

// ---------------------------------------------------------------------------
// Spreadsheets
// ---------------------------------------------------------------------------

/// Read the first (or named) sheet of an xlsx/xls/xlsb/ods workbook.
pub fn read_workbook(
    bytes: Vec<u8>,
    sheet: Option<&str>,
) -> Result<(ParsedBlock, PasteReport, String), String> {
    use calamine::{open_workbook_auto_from_rs, Data, Reader};
    let mut wb = open_workbook_auto_from_rs(std::io::Cursor::new(bytes))
        .map_err(|e| format!("cannot read the workbook: {e}"))?;
    let names = wb.sheet_names();
    let name = match sheet {
        Some(s) => names
            .iter()
            .find(|n| n.as_str() == s)
            .cloned()
            .ok_or_else(|| format!("no sheet named \"{s}\""))?,
        None => names.first().cloned().ok_or("the workbook has no sheets")?,
    };
    let range = wb
        .worksheet_range(&name)
        .map_err(|e| format!("cannot read sheet \"{name}\": {e}"))?;
    let mut report = PasteReport::default();
    if names.len() > 1 {
        report.notes.push(format!(
            "Read sheet \"{name}\" (the workbook has {} sheets).",
            names.len()
        ));
    }
    let mut ambiguous = 0;
    let mut text_numbers = 0;
    let mut dates = 0;
    let rows: Vec<Vec<RawCell>> = range
        .rows()
        .map(|r| {
            r.iter()
                .map(|c| match c {
                    Data::Empty => RawCell::Missing,
                    Data::Int(i) => RawCell::Number(*i as f64),
                    Data::Float(f) => RawCell::Number(*f),
                    Data::Bool(b) => RawCell::Text(if *b { "TRUE" } else { "FALSE" }.into()),
                    Data::String(s) => match parse_cell(s, DecimalMark::Point, &mut ambiguous) {
                        RawCell::Number(v) => {
                            text_numbers += 1;
                            RawCell::Number(v)
                        }
                        other => other,
                    },
                    Data::DateTime(d) => {
                        dates += 1;
                        let (y, m, dd, h, mi, s, _) = d.to_ymd_hms_milli();
                        if d.is_duration() {
                            RawCell::Number(d.as_f64())
                        } else if h == 0 && mi == 0 && s == 0 {
                            RawCell::Text(format!("{y:04}-{m:02}-{dd:02}"))
                        } else {
                            RawCell::Text(format!("{y:04}-{m:02}-{dd:02}T{h:02}:{mi:02}:{s:02}"))
                        }
                    }
                    Data::DateTimeIso(s) | Data::DurationIso(s) => RawCell::Text(s.clone()),
                    Data::Error(e) => RawCell::Text(format!("#{e:?}")),
                })
                .collect()
        })
        .collect();
    let mut rows = rows;
    while rows
        .last()
        .is_some_and(|r| r.iter().all(|c| *c == RawCell::Missing))
    {
        rows.pop();
    }
    if text_numbers > 0 {
        report.notes.push(format!("{text_numbers} cells stored as text in the workbook held numbers and were read as numbers."));
    }
    if dates > 0 {
        report.notes.push(format!(
            "{dates} date cells were read as ISO dates (YYYY-MM-DD)."
        ));
    }
    let block = block_from_rows(rows, None, &mut report);
    Ok((block, report, name))
}

// ---------------------------------------------------------------------------
// Prism .pzfx
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PzSub {
    values: Vec<(String, bool)>,
}

#[derive(Default)]
struct PzCol {
    title: String,
    subs: Vec<PzSub>,
}

#[derive(Default)]
struct PzTable {
    attrs: BTreeMap<String, String>,
    title: String,
    row_titles: Option<PzCol>,
    x: Option<PzCol>,
    ys: Vec<PzCol>,
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn resolve_ref(name: &str) -> String {
    match name {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        n => {
            let code = if let Some(h) = n.strip_prefix("#x").or_else(|| n.strip_prefix("#X")) {
                u32::from_str_radix(h, 16).ok()
            } else {
                n.strip_prefix('#').and_then(|d| d.parse().ok())
            };
            code.and_then(char::from_u32)
                .map(String::from)
                .unwrap_or_default()
        }
    }
}

/// Parse a Prism `.pzfx` file into tables (data only; analyses, graphs and layouts live in
/// an undocumented binary blob and are reported in the notes).
pub fn read_pzfx(
    xml: &str,
    mut new_id: impl FnMut() -> String,
) -> Result<(Vec<Table>, Vec<String>), String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    let mut notes: Vec<String> = Vec::new();
    let mut ignored: BTreeSet<String> = BTreeSet::new();
    let mut tables: Vec<PzTable> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut cur: Option<PzTable> = None;
    let mut col: Option<PzCol> = None;
    let mut sub: Option<PzSub> = None;
    let mut d: Option<(String, bool)> = None;
    let mut title_buf: Option<String> = None;
    let attrs_of = |e: &quick_xml::events::BytesStart<'_>| -> BTreeMap<String, String> {
        e.attributes()
            .flatten()
            .map(|a| {
                let k = AsRef::<str>::as_ref(&a.key).to_string();
                (k, unescape(&a.value))
            })
            .collect()
    };
    loop {
        let ev = reader
            .read_event()
            .map_err(|e| format!("not a readable .pzfx file: {e}"))?;
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty_tag = matches!(ev, Event::Empty(_));
                let name = AsRef::<str>::as_ref(&e.local_name()).to_string();
                let attrs = attrs_of(e);
                let parent = stack.last().map(String::as_str).unwrap_or("");
                match name.as_str() {
                    "Table" | "HugeTable" if cur.is_none() => {
                        cur = Some(PzTable {
                            attrs,
                            ..Default::default()
                        })
                    }
                    "Title" if cur.is_some() => title_buf = Some(String::new()),
                    "XColumn" | "XAdvancedColumn" | "YColumn" | "RowTitlesColumn"
                        if cur.is_some() =>
                    {
                        col = Some(PzCol::default())
                    }
                    "Subcolumn" if col.is_some() => sub = Some(PzSub::default()),
                    "d" if sub.is_some() => {
                        let excl = attrs.get("Excluded").is_some_and(|v| v == "1");
                        d = Some((String::new(), excl));
                    }
                    "BR" if title_buf.is_some() => {}
                    "B" | "I" | "U" | "Font" | "Sup" | "Sub" | "BR" => {}
                    _ => {
                        let in_table = cur.is_some() && parent != "d";
                        let known_outside = cur.is_none()
                            && matches!(name.as_str(), "Template" | "Info" | "FloatingNote");
                        if in_table || known_outside {
                            ignored.insert(name.clone());
                        }
                    }
                }
                if is_empty_tag {
                    // Self-closing: run the end handling now.
                    end_tag(
                        &name,
                        &mut cur,
                        &mut col,
                        &mut sub,
                        &mut d,
                        &mut title_buf,
                        &mut tables,
                        parent,
                    );
                } else {
                    stack.push(name);
                }
            }
            Event::Text(t) => {
                let s = t.xml10_content();
                if let Some((buf, _)) = d.as_mut() {
                    buf.push_str(&s);
                } else if let Some(b) = title_buf.as_mut() {
                    b.push_str(&s);
                }
            }
            Event::CData(t) => {
                let s = AsRef::<str>::as_ref(&t).to_string();
                if let Some((buf, _)) = d.as_mut() {
                    buf.push_str(&s);
                } else if let Some(b) = title_buf.as_mut() {
                    b.push_str(&s);
                }
            }
            Event::GeneralRef(r) => {
                let s = resolve_ref(&r.xml10_content());
                if let Some((buf, _)) = d.as_mut() {
                    buf.push_str(&s);
                } else if let Some(b) = title_buf.as_mut() {
                    b.push_str(&s);
                }
            }
            Event::End(e) => {
                let name = AsRef::<str>::as_ref(&e.local_name()).to_string();
                stack.pop();
                let parent = stack.last().map(String::as_str).unwrap_or("").to_string();
                end_tag(
                    &name,
                    &mut cur,
                    &mut col,
                    &mut sub,
                    &mut d,
                    &mut title_buf,
                    &mut tables,
                    &parent,
                );
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if tables.is_empty() {
        return Err("no data tables found in the .pzfx file".into());
    }
    if ignored.contains("Template") {
        notes.push("Prism's analyses, graphs and layouts (the binary Template block) cannot be read; only data tables were imported.".into());
        ignored.remove("Template");
    }
    if !ignored.is_empty() {
        notes.push(format!(
            "Ignored: {}.",
            ignored.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let mut out = Vec::new();
    for pt in tables {
        out.push(pz_to_table(pt, new_id(), &mut notes));
    }
    Ok((out, notes))
}

#[allow(clippy::too_many_arguments)]
fn end_tag(
    name: &str,
    cur: &mut Option<PzTable>,
    col: &mut Option<PzCol>,
    sub: &mut Option<PzSub>,
    d: &mut Option<(String, bool)>,
    title_buf: &mut Option<String>,
    tables: &mut Vec<PzTable>,
    parent: &str,
) {
    match name {
        "d" => {
            if let (Some(v), Some(s)) = (d.take(), sub.as_mut()) {
                s.values.push(v);
            }
        }
        "Subcolumn" => {
            if let (Some(s), Some(c)) = (sub.take(), col.as_mut()) {
                c.subs.push(s);
            }
        }
        "Title" => {
            if let Some(t) = title_buf.take() {
                let t = t.trim().to_string();
                if let Some(c) = col.as_mut() {
                    c.title = t;
                } else if let Some(tb) = cur.as_mut() {
                    if matches!(parent, "Table" | "HugeTable") {
                        tb.title = t;
                    }
                }
            }
        }
        "XColumn" => {
            if let (Some(c), Some(t)) = (col.take(), cur.as_mut()) {
                t.x = Some(c);
            }
        }
        "XAdvancedColumn" => {
            if let (Some(c), Some(t)) = (col.take(), cur.as_mut()) {
                // Date X columns carry the date text here; keep it when there is no XColumn.
                if t.x.is_none() {
                    t.x = Some(c);
                }
            }
        }
        "YColumn" => {
            if let (Some(c), Some(t)) = (col.take(), cur.as_mut()) {
                t.ys.push(c);
            }
        }
        "RowTitlesColumn" => {
            if let (Some(c), Some(t)) = (col.take(), cur.as_mut()) {
                t.row_titles = Some(c);
            }
        }
        "Table" | "HugeTable" => {
            if let Some(t) = cur.take() {
                tables.push(t);
            }
        }
        _ => {}
    }
}

fn pz_value(s: &str) -> RawCell {
    let s = s.trim();
    if s.is_empty() {
        return RawCell::Missing;
    }
    if let Some(v) = parse_number(s) {
        return RawCell::Number(v);
    }
    // Prism writes numbers in the author's locale.
    if let Some(v) = parse_number(&s.replacen(',', ".", 1))
        .filter(|_| s.matches(',').count() == 1 && !s.contains('.'))
    {
        return RawCell::Number(v);
    }
    RawCell::Text(s.to_string())
}

fn pz_column(sub: &PzSub, rows: usize) -> Column {
    let mut c = Column::empty(rows);
    for (i, (v, excl)) in sub.values.iter().enumerate() {
        match pz_value(v) {
            RawCell::Number(x) => c.values[i] = x,
            RawCell::Text(t) => {
                c.text.insert(i, t);
            }
            RawCell::Missing => {}
        }
        if *excl {
            c.excluded.insert(i);
        }
    }
    c
}

fn pz_to_table(pt: PzTable, id: String, notes: &mut Vec<String>) -> Table {
    let tt = pt.attrs.get("TableType").map(String::as_str).unwrap_or("");
    let kind = match tt {
        "XY" => TableKind::Xy,
        "OneWay" => TableKind::Column,
        "TwoWay" => TableKind::Grouped,
        "Contingency" => TableKind::Contingency,
        "Survival" => TableKind::Survival,
        "PartsOfWhole" => TableKind::Parts,
        "MultipleVariables" => TableKind::Variables,
        "Nested" => TableKind::Nested,
        "" => {
            if pt.x.is_some() {
                TableKind::Xy
            } else {
                TableKind::Column
            }
        }
        other => {
            notes.push(format!(
                "Table \"{}\": unknown table type {other}; imported as a column table.",
                pt.title
            ));
            TableKind::Column
        }
    };
    let yformat = pt
        .attrs
        .get("YFormat")
        .map(String::as_str)
        .unwrap_or("replicates");
    let reps: u16 = pt
        .attrs
        .get("Replicates")
        .and_then(|r| r.parse().ok())
        .unwrap_or(1);
    let declared = match yformat {
        "replicates" => Some(SubcolumnFormat::Values(reps.max(1))),
        "SDN" => Some(SubcolumnFormat::MeanSdN),
        "SEN" => Some(SubcolumnFormat::MeanSemN),
        "SD" => Some(SubcolumnFormat::MeanSd),
        "SE" => Some(SubcolumnFormat::MeanSem),
        other => {
            notes.push(format!(
                "Table \"{}\": the {other} error format has no Dioptra equivalent yet; its subcolumns were kept as plain values.",
                pt.title
            ));
            None
        }
    };
    if pt.attrs.get("XFormat").is_some_and(|f| f == "error") {
        notes.push(format!(
            "Table \"{}\": the X error subcolumn was not imported.",
            pt.title
        ));
    }
    let mut rows = 0;
    for c in pt.x.iter().chain(pt.ys.iter()).chain(pt.row_titles.iter()) {
        for s in &c.subs {
            rows = rows.max(s.values.len());
        }
    }
    let title = if pt.title.is_empty() {
        pt.attrs
            .get("ID")
            .cloned()
            .unwrap_or_else(|| "Prism table".into())
    } else {
        pt.title.clone()
    };
    let mut t = Table::new(id, kind, title, 0, SubcolumnFormat::Values(1), rows);
    if let Some(x) = &pt.x {
        if let Some(s) = x.subs.first() {
            let mut c = pz_column(s, rows);
            c.title = if x.title.is_empty() {
                "X".into()
            } else {
                x.title.clone()
            };
            if t.x.is_some() {
                t.x = Some(c);
            } else {
                notes.push(format!(
                    "Table \"{}\": an X column was present but this kind has none; it was dropped.",
                    t.title
                ));
            }
        }
    }
    if let Some(rt) = &pt.row_titles {
        if let Some(s) = rt.subs.first() {
            t.row_titles = s.values.iter().map(|(v, _)| v.trim().to_string()).collect();
        }
    }
    for (i, y) in pt.ys.iter().enumerate() {
        let n = y.subs.len().max(1);
        let format = match (forced_format(kind), declared) {
            (Some(f), _) => f,
            (None, Some(f)) if crate::table::subcolumn_count(f) == n => f,
            _ => SubcolumnFormat::Values(n as u16),
        };
        let mut subs: Vec<Column> = y.subs.iter().map(|s| pz_column(s, rows)).collect();
        let want = crate::table::subcolumn_count(format);
        if subs.len() > want {
            notes.push(format!(
                "Table \"{}\", data set \"{}\": only the first {want} subcolumns were kept.",
                t.title, y.title
            ));
            subs.truncate(want);
        }
        while subs.len() < want {
            subs.push(Column::empty(rows));
        }
        let name = if y.title.is_empty() {
            t.default_dataset_name(i)
        } else {
            y.title.clone()
        };
        let var_type =
            (kind == TableKind::Variables).then(|| infer_type(&subs[0].values, &subs[0].text));
        for c in &mut subs {
            c.source = ColumnSource::Imported {
                file: "Prism file".into(),
                sheet: Some(t.title.clone()),
                column: name.clone(),
            };
        }
        t.datasets.push(Dataset {
            name,
            format,
            subcolumns: subs,
            var_type,
        });
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> (ParsedBlock, PasteReport) {
        parse_delimited(text, PasteOptions::default())
    }

    #[test]
    fn tab_with_comma_decimals() {
        let (b, r) = p("Dose\tResp\n1,5\t2,25\n3,0\t-4,5\n");
        assert_eq!(r.separator, Separator::Tab);
        assert_eq!(r.decimal, DecimalMark::Comma);
        assert!(r.header_detected);
        assert_eq!(
            b.header.as_ref().unwrap(),
            &vec!["Dose".to_string(), "Resp".to_string()]
        );
        assert_eq!(b.columns[0], vec![1.5, 3.0]);
        assert_eq!(b.columns[1], vec![2.25, -4.5]);
        assert_eq!(
            (r.rows, r.cols, r.numeric_cells, r.text_cells),
            (2, 2, 4, 0)
        );
    }

    #[test]
    fn csv_with_quotes_and_crlf() {
        let (b, r) = p("\u{feff}name,value\r\n\"Smith, J\",1.5\r\n\"say \"\"hi\"\"\",2\r\n\"multi\nline\",NA\r\n");
        assert_eq!(r.separator, Separator::Comma);
        assert_eq!(r.decimal, DecimalMark::Point);
        assert!(r.header_detected);
        assert_eq!(b.text[0][&0], "Smith, J");
        assert_eq!(b.text[0][&1], "say \"hi\"");
        assert_eq!(b.text[0][&2], "multi\nline");
        assert_eq!(b.columns[1][..2], [1.5, 2.0]);
        assert!(b.columns[1][2].is_nan());
        assert_eq!(r.missing_cells, 1);
        assert!(r.notes.iter().any(|n| n.contains("byte-order")));
    }

    #[test]
    fn semicolon_european() {
        let (b, r) = p("a;b;c\n1,5;2,5;3\n4;5,25;6,125\n");
        assert_eq!(r.separator, Separator::Semicolon);
        assert_eq!(r.decimal, DecimalMark::Comma);
        assert_eq!(b.columns[1], vec![2.5, 5.25]);
        assert_eq!(b.columns[2], vec![3.0, 6.125]);
    }

    #[test]
    fn whitespace_columns() {
        let (b, r) = p("1.0   2.0  3\n4     5    6\n");
        assert_eq!(r.separator, Separator::Whitespace);
        assert!(!r.header_detected);
        assert_eq!(b.columns[2], vec![3.0, 6.0]);
    }

    #[test]
    fn header_detection_and_override() {
        let (_, r) = p("1\t2\n3\t4");
        assert!(!r.header_detected);
        let (_, r) = p("x\ty\n1\t2");
        assert!(r.header_detected);
        let (b, r) = parse_delimited(
            "x\ty\n1\t2",
            PasteOptions {
                header: Some(false),
                ..Default::default()
            },
        );
        assert!(!r.header_detected);
        assert_eq!(b.text[0][&0], "x");
        let (_, r) = p("a\tb\nc\td");
        assert!(!r.header_detected, "all text: no header");
    }

    #[test]
    fn thousands_separators() {
        // Unambiguous with a decimal point or several groups.
        let (b, r) = p("1,234.5\t1,234,567\n2.5\t3\n");
        assert_eq!(r.decimal, DecimalMark::Point);
        assert_eq!(b.columns[0], vec![1234.5, 2.5]);
        assert_eq!(b.columns[1], vec![1234567.0, 3.0]);
        // Ambiguous on its own: kept as text with a note.
        let (b, r) = p("1,234\t2.5\n7.5\t8.25\n");
        assert_eq!(r.decimal, DecimalMark::Point);
        assert_eq!(b.text[0][&0], "1,234");
        assert!(r.notes.iter().any(|n| n.contains("thousands")));
        // European grouping with comma decimals.
        let (b, r) = p("1.234,5\t2,5\n3,25\t4,75\n");
        assert_eq!(r.decimal, DecimalMark::Comma);
        assert_eq!(b.columns[0], vec![1234.5, 3.25]);
    }

    #[test]
    fn missing_codes_and_text_kept() {
        let (b, r) = p("1\tNA\n-\tn/a\n.\t'007\n=A1\tabc\n");
        assert_eq!(r.missing_cells, 4);
        assert_eq!(b.text[1][&2], "'007");
        assert_eq!(b.text[0][&3], "=A1");
        assert_eq!(b.text[1][&3], "abc");
        assert_eq!(r.text_cells, 3);
    }

    #[test]
    fn ragged_rows_note() {
        let (b, r) = p("1\t2\t3\n4\n5\t6");
        assert_eq!(b.cols, 3);
        assert!(b.columns[2][1].is_nan());
        assert!(r.notes.iter().any(|n| n.contains("different lengths")));
    }

    #[test]
    fn variables_infer_types() {
        let (b, mut r) =
            p("id,group,date,note,score\n1,ctrl,2026-01-02,a,1.5\n2,drug,2026-01-03,b,2.5\n");
        let t = table_from_block(
            "t".into(),
            TableKind::Variables,
            "T".into(),
            &b,
            &Origin::Pasted("now"),
            &mut r,
        );
        let types: Vec<_> = t.datasets.iter().map(|d| d.var_type.unwrap()).collect();
        assert_eq!(
            types,
            vec![
                VariableType::Number,
                VariableType::Category,
                VariableType::Date,
                VariableType::Category,
                VariableType::Number
            ]
        );
        assert_eq!(t.datasets[1].name, "group");
        let many: String = (0..20).map(|i| format!("w{i}\n")).collect();
        let (b, mut r) = p(&format!("word\n{many}"));
        let t = table_from_block(
            "t".into(),
            TableKind::Variables,
            "T".into(),
            &b,
            &Origin::Pasted("now"),
            &mut r,
        );
        assert_eq!(t.datasets[0].var_type, Some(VariableType::Text));
    }

    #[test]
    fn paste_into_grows_and_names() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Column,
            "T".into(),
            1,
            SubcolumnFormat::Values(1),
            2,
        );
        let (b, mut r) = p("Ctrl\tDrug\tHigh\n1\t2\t3\n4\t5\t6\n7\t8\t9");
        let w = paste_into(&mut t, 1, 0, &b, "now", &mut r);
        assert_eq!(w.len(), 3);
        assert_eq!(t.rows, 4);
        assert_eq!(t.datasets.len(), 3);
        assert_eq!(t.datasets[2].name, "High");
        assert_eq!(t.datasets[0].subcolumns[0].values[1..], [1.0, 4.0, 7.0]);
    }

    const PZFX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<GraphPadPrismFile PrismXMLVersion="5.00">
  <Created><OriginalVersion CreatedByProgram="GraphPad Prism" CreatedByVersion="6.0f.254"/></Created>
  <InfoSequence><Ref ID="Info0" Selected="1"/></InfoSequence>
  <Info ID="Info0"><Title>Project info 1</Title><Notes>n</Notes></Info>
  <TableSequence Selected="1"><Ref ID="Table0" Selected="1"/><Ref ID="Table1"/></TableSequence>
  <Table ID="Table0" XFormat="numbers" YFormat="replicates" Replicates="2" TableType="XY" EVFormat="AsteriskAfterNumber">
    <Title>Exponential <B>decay</B> &amp; more</Title>
    <FloatingNote ID="Sticky0" Color="Yellow"><B><Font Size="9">note</Font></B><BR/></FloatingNote>
    <RowTitlesColumn Width="39"><Subcolumn><d>r1</d><d>r2</d><d>r3</d></Subcolumn></RowTitlesColumn>
    <XColumn Width="103" Decimals="1" Subcolumns="1">
      <Title>Minutes</Title>
      <Subcolumn><d>1.0</d><d>2.0</d><d>3,5</d></Subcolumn>
    </XColumn>
    <YColumn Width="269" Decimals="0" Subcolumns="2">
      <Title>Control</Title>
      <Subcolumn><d>8887.</d><d Excluded="1">8329</d><d>7000</d></Subcolumn>
      <Subcolumn><d>7366.</d><d></d><d/></Subcolumn>
    </YColumn>
  </Table>
  <Table ID="Table1" TableType="OneWay" YFormat="SDN" Replicates="1">
    <Title>Summary</Title>
    <YColumn Subcolumns="3"><Title>A</Title>
      <Subcolumn><d>5</d></Subcolumn><Subcolumn><d>1.2</d></Subcolumn><Subcolumn><d>10</d></Subcolumn>
    </YColumn>
    <YColumn Subcolumns="3"><Title>B</Title>
      <Subcolumn><d>oops</d></Subcolumn><Subcolumn><d>1.1</d></Subcolumn><Subcolumn><d>9</d></Subcolumn>
    </YColumn>
  </Table>
  <Template dt:dt="bin.base64" xmlns:dt="urn:schemas-microsoft-com:datatypes">eNrs</Template>
</GraphPadPrismFile>"#;

    #[test]
    fn pzfx_tables() {
        let mut n = 0;
        let (tables, notes) = read_pzfx(PZFX, || {
            n += 1;
            format!("id{n}")
        })
        .unwrap();
        assert_eq!(tables.len(), 2);
        let t = &tables[0];
        assert_eq!(t.kind, TableKind::Xy);
        assert_eq!(t.title, "Exponential decay & more");
        assert_eq!(t.rows, 3);
        assert_eq!(t.x.as_ref().unwrap().title, "Minutes");
        assert_eq!(t.x.as_ref().unwrap().values, vec![1.0, 2.0, 3.5]);
        assert_eq!(t.row_titles, vec!["r1", "r2", "r3"]);
        assert_eq!(t.datasets[0].name, "Control");
        assert_eq!(t.datasets[0].format, SubcolumnFormat::Values(2));
        assert_eq!(
            t.datasets[0].subcolumns[0].values,
            vec![8887.0, 8329.0, 7000.0]
        );
        assert!(t.datasets[0].subcolumns[0].excluded.contains(&1));
        assert!(t.datasets[0].subcolumns[1].values[1].is_nan());
        let s = &tables[1];
        assert_eq!(s.kind, TableKind::Column);
        assert_eq!(s.datasets[0].format, SubcolumnFormat::MeanSdN);
        assert_eq!(s.datasets[1].subcolumns[0].text[&0], "oops");
        assert!(notes.iter().any(|n| n.contains("Template")), "{notes:?}");
        assert!(
            notes.iter().any(|n| n.contains("FloatingNote")),
            "{notes:?}"
        );
    }

    #[test]
    fn pzfx_rejects_garbage() {
        assert!(read_pzfx("<GraphPadPrismFile></GraphPadPrismFile>", || "x".into()).is_err());
    }
}
