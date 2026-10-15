// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Writing GraphPad Prism `.pzfx` files: the data tables of a project, with replicates,
//! summary formats, row titles, subcolumn titles, text and excluded values, for Prism 5
//! and later. Analyses and graphs are not written: Prism keeps its own encrypted, and the
//! receiving lab redoes them in Prism.
//!
//! The element layout follows what Prism writes (see the files in fixtures/prism) and what
//! `dioptra_core::import::read_pzfx` reads, so a written file imports back unchanged
//! wherever Prism has a matching format.

use dioptra_core::api::{SubcolumnFormat, TableKind};
use dioptra_core::table::{Column, Dataset, Table};
use std::borrow::Cow;
use std::fmt::Write;

/// A `.pzfx` document and what could not be written exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct Written {
    /// The XML.
    pub xml: String,
    /// One line per approximation.
    pub notes: Vec<String>,
}

/// Write `tables` as one Prism project.
pub fn write_pzfx(tables: &[&Table], version: &str) -> Written {
    let mut notes = Vec::new();
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<GraphPadPrismFile PrismXMLVersion=\"5.00\">\n");
    // The header and project-info block follow the R pzfx package's writer, whose files Prism
    // users open; Dioptra's version goes in the project notes.
    x.push_str("<Created>\n<OriginalVersion CreatedByProgram=\"GraphPad Prism\" CreatedByVersion=\"6.0f.254\" Login=\"\" DateTime=\"\"></OriginalVersion>\n</Created>\n");
    x.push_str("<InfoSequence>\n<Ref ID=\"Info0\" Selected=\"1\"></Ref>\n</InfoSequence>\n");
    let _ = writeln!(
        x,
        "<Info ID=\"Info0\">\n<Title>Project info 1</Title>\n<Notes><Font Color=\"#000000\" Face=\"Helvetica\">Written by Dioptra {}<BR/></Font></Notes>\n</Info>",
        esc(version)
    );
    x.push_str("<TableSequence>\n");
    for i in 0..tables.len() {
        let _ = writeln!(
            x,
            "<Ref ID=\"Table{i}\"{}></Ref>",
            if i == 0 { " Selected=\"1\"" } else { "" }
        );
    }
    x.push_str("</TableSequence>\n");
    for (i, t) in tables.iter().enumerate() {
        table(&mut x, i, t, &mut notes);
    }
    x.push_str("</GraphPadPrismFile>\n");
    Written { xml: x, notes }
}

/// Prism's `YFormat` for a Dioptra format: (name, subcolumns Prism expects, how to write).
fn y_format(f: SubcolumnFormat) -> (&'static str, usize, Shape) {
    match f {
        SubcolumnFormat::Values(n) => ("replicates", n.max(1) as usize, Shape::AsIs),
        SubcolumnFormat::Single => ("replicates", 1, Shape::AsIs),
        SubcolumnFormat::MeanSdN => ("SDN", 3, Shape::AsIs),
        SubcolumnFormat::MeanSemN => ("SEN", 3, Shape::AsIs),
        SubcolumnFormat::MeanSd => ("SD", 2, Shape::AsIs),
        SubcolumnFormat::MeanSem => ("SE", 2, Shape::AsIs),
        // Prism has no two-column interval; it stores a centre with upper and lower limits.
        SubcolumnFormat::LowerUpper | SubcolumnFormat::MeanCiN => {
            ("upper-lower-limits", 3, Shape::Limits)
        }
        SubcolumnFormat::MedianIqr => ("upper-lower-limits", 3, Shape::MedianQuartiles),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    AsIs,
    /// Lower, upper (, N) → centre, upper, lower.
    Limits,
    /// Median, Q1, Q3 → median, Q3, Q1.
    MedianQuartiles,
}

/// One Prism column to write: its title and its subcolumns.
struct OutColumn<'a> {
    title: String,
    subs: Vec<Cow<'a, Column>>,
}

fn table(x: &mut String, i: usize, t: &Table, notes: &mut Vec<String>) {
    let (tt, ext) = match t.kind {
        TableKind::Xy => ("XY", None),
        TableKind::Column => ("OneWay", None),
        TableKind::Grouped => ("TwoWay", None),
        TableKind::Contingency => ("Contingency", None),
        TableKind::Survival => ("Survival", None),
        TableKind::Parts => ("PartsOfWhole", None),
        TableKind::Nested => ("TwoWay", Some("Nested")),
        TableKind::Variables => ("OneWay", Some("MultipleVariables")),
    };
    let summary_kind = !matches!(
        t.kind,
        TableKind::Contingency | TableKind::Survival | TableKind::Parts | TableKind::Variables
    );

    // Prism holds one Y format per table; Dioptra one per data set.
    // A "<name> (mean)" data set just before a lower/upper one is that interval's centre,
    // as the importer makes from Prism's limits; it does not count as a format of its own.
    let is_centre = |k: usize| {
        let d = &t.datasets[k];
        d.subcolumns.len() == 1
            && t.datasets.get(k + 1).is_some_and(|n| {
                matches!(
                    n.format,
                    SubcolumnFormat::LowerUpper | SubcolumnFormat::MeanCiN
                ) && d.name == format!("{} (mean)", n.name)
            })
    };
    let mut formats: Vec<SubcolumnFormat> = (0..t.datasets.len())
        .filter(|&k| !is_centre(k))
        .map(|k| t.datasets[k].format)
        .collect();
    // Replicate counts may differ between data sets: Prism keeps the table's largest and
    // writes each column's own count.
    let max_reps = formats
        .iter()
        .filter_map(|f| match f {
            SubcolumnFormat::Values(n) => Some(*n),
            _ => None,
        })
        .max();
    if let Some(n) = max_reps.filter(|_| {
        formats
            .iter()
            .all(|f| matches!(f, SubcolumnFormat::Values(_)))
    }) {
        formats = vec![SubcolumnFormat::Values(n)];
    }
    formats.dedup();
    let mixed = formats.len() > 1;
    let fmt = if mixed {
        let n = t
            .datasets
            .iter()
            .map(|d| d.subcolumns.len())
            .max()
            .unwrap_or(1);
        notes.push(format!(
            "Table \"{}\": its data sets use different formats; Prism needs one per table, so every subcolumn was written as a replicate ({n} per data set).",
            t.title
        ));
        SubcolumnFormat::Values(n as u16)
    } else {
        formats
            .first()
            .copied()
            .unwrap_or(SubcolumnFormat::Values(1))
    };
    let (yname, width, shape) = y_format(fmt);

    let mut cols: Vec<OutColumn> = Vec::new();
    let mut k = 0;
    while k < t.datasets.len() {
        let d = &t.datasets[k];
        if mixed || shape == Shape::AsIs || is_centre(k) {
            cols.push(OutColumn {
                title: d.name.clone(),
                subs: d.subcolumns.iter().map(Cow::Borrowed).collect(),
            });
            k += 1;
            continue;
        }
        if shape == Shape::MedianQuartiles {
            let s = &d.subcolumns;
            cols.push(OutColumn {
                title: d.name.clone(),
                subs: vec![
                    Cow::Borrowed(&s[0]),
                    Cow::Borrowed(&s[2]),
                    Cow::Borrowed(&s[1]),
                ],
            });
            k += 1;
            continue;
        }
        // Lower and upper: a "<name> (mean)" data set right before, as the importer makes
        // from Prism's limits, supplies the centre; otherwise the midpoint does.
        let (centre, name) = match cols.last() {
            Some(prev) if prev.title == format!("{} (mean)", d.name) && prev.subs.len() == 1 => {
                let c = cols.pop().expect("checked").subs.remove(0).into_owned();
                (c, d.name.clone())
            }
            _ => {
                notes.push(format!(
                    "Table \"{}\", data set \"{}\": Prism stores limits around a centre; the midpoint of lower and upper was written as the centre.",
                    t.title, d.name
                ));
                (midpoint(&d.subcolumns[0], &d.subcolumns[1]), d.name.clone())
            }
        };
        if fmt == SubcolumnFormat::MeanCiN {
            notes.push(format!(
                "Table \"{}\", data set \"{}\": Prism's limits format has no N; N was not written.",
                t.title, d.name
            ));
        }
        cols.push(OutColumn {
            title: name,
            subs: vec![
                Cow::Owned(centre),
                Cow::Borrowed(&d.subcolumns[1]),
                Cow::Borrowed(&d.subcolumns[0]),
            ],
        });
        k += 1;
    }
    if shape == Shape::MedianQuartiles && !mixed {
        notes.push(format!(
            "Table \"{}\": Prism has no median-and-quartiles format; they were written as a centre with upper and lower limits.",
            t.title
        ));
    }

    let _ = write!(x, "<Table ID=\"Table{i}\"");
    let _ = write!(
        x,
        " XFormat=\"{}\"",
        if t.x.is_some() { "numbers" } else { "none" }
    );
    if summary_kind {
        let _ = write!(x, " YFormat=\"{yname}\"");
        if yname == "replicates" {
            let _ = write!(x, " Replicates=\"{width}\"");
        }
    }
    let _ = write!(x, " TableType=\"{tt}\"");
    if let Some(e) = ext {
        let _ = write!(x, " ExtTableType=\"{e}\"");
    }
    x.push_str(" EVFormat=\"AsteriskAfterNumber\">\n");
    let _ = writeln!(x, "<Title>{}</Title>", esc(&t.title));

    if t.row_titles.iter().any(|r| !r.is_empty()) {
        x.push_str("<RowTitlesColumn Width=\"89\">\n<Subcolumn>\n");
        for r in 0..t.rows {
            let v = t.row_titles.get(r).map(String::as_str).unwrap_or("");
            let _ = writeln!(x, "<d>{}</d>", esc(v));
        }
        x.push_str("</Subcolumn>\n</RowTitlesColumn>\n");
    }

    // Replicate titles (nested subgroups): shared when every data set has the same ones.
    if matches!(fmt, SubcolumnFormat::Values(_)) && !mixed {
        let titles: Vec<Vec<&str>> = cols
            .iter()
            .map(|c| c.subs.iter().map(|s| s.title.as_str()).collect())
            .collect();
        if titles.iter().flatten().any(|s| !s.is_empty()) {
            let shared = titles.windows(2).all(|w| w[0] == w[1]);
            let _ = writeln!(
                x,
                "<SubColumnTitles OwnSet=\"{}\">",
                if shared { 0 } else { 1 }
            );
            for s in 0..width {
                x.push_str("<Subcolumn>\n");
                let sets = if shared { &titles[..1] } else { &titles[..] };
                for set in sets {
                    let _ = writeln!(x, "<d>{}</d>", esc(set.get(s).copied().unwrap_or("")));
                }
                x.push_str("</Subcolumn>\n");
            }
            x.push_str("</SubColumnTitles>\n");
        }
    }

    if let Some(xc) = &t.x {
        let _ = writeln!(
            x,
            "<XColumn Width=\"89\" Decimals=\"{}\" Subcolumns=\"1\">\n<Title>{}</Title>",
            xc.decimals.unwrap_or(2),
            esc(&xc.title)
        );
        subcolumn(x, xc, t.rows);
        x.push_str("</XColumn>\n");
    }

    for (c, d) in cols.iter().zip(dataset_attrs(t, &cols)) {
        let n = if summary_kind && yname != "replicates" {
            width
        } else {
            c.subs.len().max(1)
        };
        let _ = write!(
            x,
            "<YColumn Width=\"{}\" Decimals=\"{}\" Subcolumns=\"{n}\"",
            81 * n.min(3),
            d.decimals
        );
        if let Some(mv) = d.mv_type {
            let _ = write!(x, " MVType=\"{mv}\"");
        }
        let _ = writeln!(x, ">\n<Title>{}</Title>", esc(&c.title));
        for s in 0..n {
            match c.subs.get(s) {
                Some(col) => subcolumn(x, col, t.rows),
                None => x.push_str("<Subcolumn></Subcolumn>\n"),
            }
        }
        x.push_str("</YColumn>\n");
    }
    x.push_str("</Table>\n");
}

struct ColumnAttrs {
    decimals: u8,
    mv_type: Option<&'static str>,
}

fn dataset_attrs(t: &Table, cols: &[OutColumn]) -> Vec<ColumnAttrs> {
    cols.iter()
        .map(|c| {
            let d: Option<&Dataset> = t.datasets.iter().find(|d| d.name == c.title);
            ColumnAttrs {
                decimals: c.subs.first().and_then(|s| s.decimals).unwrap_or(2),
                mv_type: (t.kind == TableKind::Variables).then(|| {
                    match d.and_then(|d| d.var_type) {
                        Some(dioptra_core::api::VariableType::Category)
                        | Some(dioptra_core::api::VariableType::Text) => "Manual|Categorical",
                        _ => "Manual|Continues",
                    }
                }),
            }
        })
        .collect()
}

fn subcolumn(x: &mut String, c: &Column, rows: usize) {
    x.push_str("<Subcolumn>\n");
    // Trailing empty cells are left out, as Prism does.
    let last = (0..rows.min(c.values.len().max(rows)))
        .rev()
        .find(|&r| !cell(c, r).is_empty() || c.excluded.contains(&r))
        .map_or(0, |r| r + 1);
    for r in 0..last {
        let v = cell(c, r);
        if c.excluded.contains(&r) {
            let _ = writeln!(x, "<d Excluded=\"1\">{v}</d>");
        } else {
            let _ = writeln!(x, "<d>{v}</d>");
        }
    }
    x.push_str("</Subcolumn>\n");
}

/// A cell as Prism text: the number in shortest round-trip form, text escaped, or empty.
fn cell(c: &Column, r: usize) -> String {
    if let Some(t) = c.text.get(&r) {
        return esc(t);
    }
    match c.values.get(r) {
        Some(v) if v.is_finite() => format!("{v}"),
        _ => String::new(),
    }
}

fn midpoint(a: &Column, b: &Column) -> Column {
    let mut m = a.clone();
    m.text.clear();
    for (v, w) in m.values.iter_mut().zip(&b.values) {
        *v = (*v + *w) / 2.0;
    }
    m
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioptra_core::import::{read_prism_file, read_pzfx};

    fn read(bytes: &[u8]) -> Vec<Table> {
        let mut n = 0;
        read_prism_file(bytes, || {
            n += 1;
            format!("t{n}")
        })
        .unwrap()
        .0
    }

    fn reread(xml: &str) -> Vec<Table> {
        let mut n = 0;
        read_pzfx(xml, || {
            n += 1;
            format!("t{n}")
        })
        .unwrap_or_else(|e| panic!("{e}\n{xml}"))
        .0
    }

    fn same_cells(a: &Column, b: &Column, rows: usize, what: &str) {
        for r in 0..rows {
            let (x, y) = (
                a.values.get(r).copied().unwrap_or(f64::NAN),
                b.values.get(r).copied().unwrap_or(f64::NAN),
            );
            assert!(
                x == y || (x.is_nan() && y.is_nan()),
                "{what} row {r}: {x} vs {y}"
            );
            assert_eq!(a.text.get(&r), b.text.get(&r), "{what} row {r} text");
            assert_eq!(
                a.excluded.contains(&r),
                b.excluded.contains(&r),
                "{what} row {r} exclusion"
            );
        }
    }

    /// Every committed Prism fixture: import, write, import again, and compare.
    #[test]
    fn fixtures_round_trip() {
        let base = format!("{}/../../fixtures/prism", env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        for dir in [base.clone(), format!("{base}/corpus")] {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext != "pzfx" && ext != "prism" {
                    continue;
                }
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                let first = read(&std::fs::read(&path).unwrap());
                let refs: Vec<&Table> = first.iter().collect();
                let w = write_pzfx(&refs, "test");
                let second = reread(&w.xml);
                assert_eq!(first.len(), second.len(), "{name}: table count");
                for (a, b) in first.iter().zip(&second) {
                    let what = format!("{name} / {}", a.title);
                    assert_eq!(a.kind, b.kind, "{what}: kind");
                    assert_eq!(a.title, b.title, "{what}: title");
                    assert_eq!(a.datasets.len(), b.datasets.len(), "{what}: data sets");
                    if let (Some(x), Some(y)) = (&a.x, &b.x) {
                        same_cells(x, y, a.rows, &format!("{what} X"));
                    }
                    for (da, db) in a.datasets.iter().zip(&b.datasets) {
                        assert_eq!(da.name, db.name, "{what}: data set name");
                        assert_eq!(da.format, db.format, "{what} / {}: format", da.name);
                        for (k, (ca, cb)) in da.subcolumns.iter().zip(&db.subcolumns).enumerate() {
                            same_cells(ca, cb, a.rows, &format!("{what} / {} / {k}", da.name));
                            assert_eq!(ca.title, cb.title, "{what}: subcolumn title");
                        }
                    }
                    let titled = |t: &Table| t.row_titles.iter().any(|r| !r.is_empty());
                    if titled(a) {
                        assert_eq!(
                            a.row_titles.iter().map(|s| s.trim()).collect::<Vec<_>>(),
                            b.row_titles
                                .iter()
                                .map(|s| s.trim())
                                .take(a.row_titles.len())
                                .collect::<Vec<_>>(),
                            "{what}: row titles"
                        );
                    }
                }
                checked += 1;
            }
        }
        assert!(checked >= 30, "{checked}");
    }

    #[test]
    fn text_is_escaped() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Column,
            "A & <B>".into(),
            1,
            SubcolumnFormat::Values(1),
            2,
        );
        t.datasets[0].name = "\"q\"".into();
        t.datasets[0].subcolumns[0].values = vec![1.5, f64::NAN];
        t.datasets[0].subcolumns[0].text.insert(1, "n/a <1".into());
        let w = write_pzfx(&[&t], "x");
        let back = reread(&w.xml);
        assert_eq!(back[0].title, "A & <B>");
        assert_eq!(back[0].datasets[0].name, "\"q\"");
        assert_eq!(
            back[0].datasets[0].subcolumns[0]
                .text
                .get(&1)
                .map(String::as_str),
            Some("n/a <1")
        );
    }
}
