//! Getting data out: CSV/TSV and XLSX for tables, TSV and Markdown for results sheets.
//!
//! Tables are written with a header row of column headers (and a leading "Row title"
//! column when the table has row titles). Numbers are written at full precision in their
//! shortest round-trip form, text as text. **Excluded cells are written empty**, because a
//! spreadsheet that reads the file would otherwise use them; the exclusion itself lives
//! only in the `.dioptra` file.

use crate::api::PValueStyle;
use crate::results::ResultsSheet;
use crate::table::Table;

/// Text of one cell for export (empty for empty or excluded cells).
fn cell_text(t: &Table, flat: usize, row: usize) -> String {
    let c = t.col(flat).expect("column");
    if c.excluded.contains(&row) {
        return String::new();
    }
    let v = c.values[row];
    if !v.is_nan() {
        format!("{v}")
    } else {
        c.text.get(&row).cloned().unwrap_or_default()
    }
}

/// A table as CSV (`sep` = b',') or TSV (`sep` = b'\t'), RFC 4180 quoting.
pub fn table_to_delimited(t: &Table, sep: u8) -> Result<String, String> {
    let mut w = csv::WriterBuilder::new()
        .delimiter(sep)
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    let rt = t.has_row_titles();
    let mut header: Vec<String> = Vec::new();
    if rt {
        header.push("Row title".into());
    }
    header.extend((0..t.col_count()).map(|c| t.col_header(c)));
    w.write_record(&header).map_err(|e| e.to_string())?;
    let mut rec: Vec<String> = Vec::with_capacity(header.len());
    for r in 0..t.rows {
        rec.clear();
        if rt {
            rec.push(t.row_title(r).to_string());
        }
        rec.extend((0..t.col_count()).map(|c| cell_text(t, c, r)));
        w.write_record(&rec).map_err(|e| e.to_string())?;
    }
    let bytes = w.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn sheet_name(title: &str, used: &mut Vec<String>) -> String {
    let mut s: String = title.chars().filter(|c| !"[]:*?/\\".contains(*c)).collect();
    s = s.trim().trim_matches('\'').to_string();
    if s.is_empty() {
        s = "Table".into();
    }
    let base: String = s.chars().take(28).collect();
    let mut name: String = s.chars().take(31).collect();
    let mut k = 2;
    while used.iter().any(|u| u.eq_ignore_ascii_case(&name)) {
        name = format!("{base} {k}");
        k += 1;
    }
    used.push(name.clone());
    name
}

/// Tables as an Excel workbook, one sheet per table, bold header row.
pub fn tables_to_xlsx(tables: &[&Table]) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Format, Workbook};
    let mut wb = Workbook::new();
    let bold = Format::new().set_bold();
    let mut used = Vec::new();
    for t in tables {
        if t.rows + 1 > 1_048_576 || t.col_count() + 1 > 16_384 {
            return Err(format!(
                "\"{}\" is larger than an Excel sheet allows",
                t.title
            ));
        }
        let ws = wb.add_worksheet();
        ws.set_name(sheet_name(&t.title, &mut used))
            .map_err(|e| e.to_string())?;
        let rt = t.has_row_titles();
        let off = u16::from(rt);
        if rt {
            ws.write_string_with_format(0, 0, "Row title", &bold)
                .map_err(|e| e.to_string())?;
        }
        for c in 0..t.col_count() {
            ws.write_string_with_format(0, c as u16 + off, t.col_header(c), &bold)
                .map_err(|e| e.to_string())?;
        }
        for r in 0..t.rows {
            let xr = r as u32 + 1;
            if rt && !t.row_title(r).is_empty() {
                ws.write_string(xr, 0, t.row_title(r))
                    .map_err(|e| e.to_string())?;
            }
            for (c, col) in t.columns().enumerate() {
                if col.excluded.contains(&r) {
                    continue;
                }
                let v = col.values[r];
                let xc = c as u16 + off;
                if !v.is_nan() {
                    ws.write_number(xr, xc, v).map_err(|e| e.to_string())?;
                } else if let Some(s) = col.text.get(&r) {
                    ws.write_string(xr, xc, s).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    wb.save_to_buffer().map_err(|e| e.to_string())
}

/// A results sheet as TSV: each section is a title line, a header row and its rows,
/// followed by a blank line; summary sentences come last.
pub fn results_to_tsv(sheet: &ResultsSheet, style: PValueStyle) -> String {
    let mut out = format!("{}\n\n", sheet.title);
    for s in &sheet.sections {
        out.push_str(&s.title);
        out.push('\n');
        out.push_str(&s.table.columns.join("\t"));
        out.push('\n');
        for r in &s.table.rows {
            let cells: Vec<String> = r
                .iter()
                .map(|c| c.display(style).replace(['\t', '\n'], " "))
                .collect();
            out.push_str(&cells.join("\t"));
            out.push('\n');
        }
        out.push('\n');
    }
    for a in &sheet.assumptions {
        out.push_str(&format!("{}\t{:?}\t{}\n", a.name, a.status, a.detail));
    }
    for s in &sheet.summary {
        out.push_str(s);
        out.push('\n');
    }
    if let Some(m) = &sheet.methods {
        out.push_str(m);
        out.push('\n');
    }
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// A results sheet as Markdown.
pub fn results_to_markdown(sheet: &ResultsSheet, style: PValueStyle) -> String {
    let mut out = format!("# {}\n\n", sheet.title);
    if let Some(m) = &sheet.message {
        out.push_str(&format!("{m}\n\n"));
    }
    if !sheet.assumptions.is_empty() {
        out.push_str("## Assumptions\n\n");
        for a in &sheet.assumptions {
            out.push_str(&format!(
                "- **{}** ({}): {}{}\n",
                a.name,
                a.status.label(),
                a.detail,
                a.alternative
                    .as_ref()
                    .map_or(String::new(), |x| format!(" *Alternative:* {x}"))
            ));
        }
        out.push('\n');
    }
    if !sheet.summary.is_empty() {
        out.push_str("## Summary\n\n");
        for s in &sheet.summary {
            out.push_str(&format!("{s}\n\n"));
        }
    }
    if let Some(m) = &sheet.methods {
        out.push_str(&format!("## Methods\n\n{m}\n\n"));
    }
    for s in &sheet.sections {
        out.push_str(&format!("## {}\n\n", s.title));
        let cols: Vec<String> = s.table.columns.iter().map(|c| md_escape(c)).collect();
        out.push_str(&format!("| {} |\n", cols.join(" | ")));
        out.push_str(&format!(
            "|{}\n",
            cols.iter()
                .enumerate()
                .map(|(i, _)| if i == 0 { "---|" } else { "---:|" })
                .collect::<String>()
        ));
        for r in &s.table.rows {
            let cells: Vec<String> = r.iter().map(|c| md_escape(&c.display(style))).collect();
            out.push_str(&format!("| {} |\n", cells.join(" | ")));
        }
        out.push('\n');
    }
    if !sheet.checklist.is_empty() {
        out.push_str("## Checklist\n\n");
        for c in &sheet.checklist {
            out.push_str(&format!("- {c}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{SubcolumnFormat, TableKind};

    fn table() -> Table {
        let mut t = Table::new(
            "t".into(),
            TableKind::Grouped,
            "My, table".into(),
            2,
            SubcolumnFormat::Values(1),
            0,
        );
        t.datasets[0].name = "Ctrl".into();
        t.datasets[1].name = "Drug \"X\"".into();
        t.set_cell_raw(0, 0, "1.5").unwrap();
        t.set_cell_raw(0, 1, "a, b").unwrap();
        t.set_cell_raw(1, 0, "0.1").unwrap();
        t.set_cell_raw(1, 1, "3").unwrap();
        t.datasets[1].subcolumns[0].excluded.insert(1);
        t.set_row_title(0, "Day 1".into());
        t
    }

    #[test]
    fn csv_and_tsv() {
        let t = table();
        let csv = table_to_delimited(&t, b',').unwrap();
        assert_eq!(
            csv,
            "Row title,Ctrl,\"Drug \"\"X\"\"\"\r\nDay 1,1.5,\"a, b\"\r\n,0.1,\r\n"
        );
        let tsv = table_to_delimited(&t, b'\t').unwrap();
        assert!(tsv.starts_with("Row title\tCtrl\t"));
    }

    #[test]
    fn xlsx_round_trip_through_calamine() {
        let t = table();
        let bytes = tables_to_xlsx(&[&t]).unwrap();
        let (block, report, sheet) = crate::import::read_workbook(bytes, None).unwrap();
        assert_eq!(sheet, "My, table");
        assert!(report.header_detected);
        let h = block.header.unwrap();
        assert_eq!(h, vec!["Row title", "Ctrl", "Drug \"X\""]);
        assert_eq!(block.text[0][&0], "Day 1");
        assert_eq!(block.columns[1], vec![1.5, 0.1]);
        assert_eq!(block.text[2][&0], "a, b");
        assert!(
            block.columns[2][1].is_nan(),
            "excluded cell is written empty"
        );
    }
}
