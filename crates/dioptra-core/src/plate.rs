//! Plate-reader exports: Molecular Devices SoftMax Pro (`.txt`), BMG MARS (CSV/TXT), Agilent
//! BioTek Gen5 (`.txt`) and Tecan Magellan (`.xlsx`).
//!
//! Files are recognised by their content, not their extension ([`read`] answers `None` for
//! anything else, and the ordinary text or workbook import takes over). Each file becomes a
//! list of [`PlateRead`]s: one per wavelength, filter or result label, holding one frame of
//! well values (endpoint) or one frame per time point (kinetic). [`build_table`] turns them
//! into a table:
//!
//! - endpoint, no plate map: a grouped table, rows A, B, … (plate rows), one data set per read
//!   with one subcolumn per plate column (titled 1, 2, …);
//! - endpoint with a plate map: a column table (one data set per group, wells as replicates),
//!   or an XY table when the map gives doses (X = dose, one data set per group, wells with the
//!   same group and dose as replicates);
//! - kinetic: an XY table, X = time in seconds, one data set per well (or per group of the
//!   plate map, wells as replicates).
//!
//! The parsers follow the layouts documented by Benchling's `allotropy` (MIT) and are tested
//! against its sample files (`fixtures/plate`). Skipped, with a note: spectrum and well-scan
//! reads, SoftMax Pro group (calculation) blocks and reduced data, Gen5 calculated results in
//! kinetic files, BMG table-view (kinetic) exports and Magellan text exports.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::api::{SubcolumnFormat, TableKind};
use crate::import::PasteReport;
use crate::table::{Column, ColumnSource, Dataset, Table};

/// The instrument software that wrote a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    /// Molecular Devices SoftMax Pro.
    SoftMaxPro,
    /// BMG Labtech MARS.
    BmgMars,
    /// Agilent BioTek Gen5.
    Gen5,
    /// Tecan Magellan.
    Magellan,
}

impl Vendor {
    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            Vendor::SoftMaxPro => "Molecular Devices SoftMax Pro",
            Vendor::BmgMars => "BMG Labtech MARS",
            Vendor::Gen5 => "Agilent BioTek Gen5",
            Vendor::Magellan => "Tecan Magellan",
        }
    }
}

/// One read of a plate: a wavelength, filter pair or result label.
#[derive(Debug, Clone, PartialEq)]
pub struct PlateRead {
    /// Name, e.g. `450 nm` or `Raw Data (580/620)`.
    pub name: String,
    /// Plate rows.
    pub rows: usize,
    /// Plate columns.
    pub cols: usize,
    /// Kinetic time points in seconds; empty for an endpoint read.
    pub times: Vec<f64>,
    /// One frame per time point (one for endpoint), each `rows × cols` row-major; NaN where
    /// a well has no number.
    pub frames: Vec<Vec<f64>>,
}

impl PlateRead {
    fn new(name: String, rows: usize, cols: usize) -> Self {
        PlateRead {
            name,
            rows,
            cols,
            times: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// Whether this read has time points.
    pub fn kinetic(&self) -> bool {
        !self.times.is_empty()
    }

    fn blank_frame(&self) -> Vec<f64> {
        vec![f64::NAN; self.rows * self.cols]
    }
}

/// What a plate-reader file holds.
#[derive(Debug, Clone, PartialEq)]
pub struct PlateExport {
    /// Who wrote it.
    pub vendor: Vendor,
    /// Reads in file order.
    pub reads: Vec<PlateRead>,
    /// Plain-English notes about what was skipped or unusual.
    pub notes: Vec<String>,
}

/// A short description for the import dialog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlateSummary {
    /// Who wrote the file.
    pub vendor: Vendor,
    /// Vendor display name.
    pub vendor_label: String,
    /// Read names (only the kind that will be imported).
    pub reads: Vec<String>,
    /// Plate rows.
    pub rows: usize,
    /// Plate columns.
    pub cols: usize,
    /// Kinetic (time course) rather than endpoint.
    pub kinetic: bool,
    /// Time points of a kinetic read.
    pub time_points: usize,
    /// Notes.
    pub notes: Vec<String>,
}

impl PlateExport {
    /// The reads that will be imported: the kinetic ones when the first read is kinetic,
    /// the endpoint ones otherwise.
    pub fn used_reads(&self) -> Vec<&PlateRead> {
        let kinetic = self.reads.first().is_some_and(PlateRead::kinetic);
        self.reads
            .iter()
            .filter(|r| r.kinetic() == kinetic)
            .collect()
    }

    /// Summary for the UI.
    pub fn summary(&self) -> PlateSummary {
        let used = self.used_reads();
        let first = used.first();
        PlateSummary {
            vendor: self.vendor,
            vendor_label: self.vendor.label().into(),
            reads: used.iter().map(|r| r.name.clone()).collect(),
            rows: first.map_or(0, |r| r.rows),
            cols: first.map_or(0, |r| r.cols),
            kinetic: first.is_some_and(|r| r.kinetic()),
            time_points: first.map_or(0, |r| r.times.len()),
            notes: self.notes.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Decode a text export: UTF-16 (with or without a byte-order mark), UTF-8, or Latin-1.
pub fn decode_text(bytes: &[u8]) -> String {
    fn utf16(b: &[u8], le: bool) -> String {
        let u: Vec<u16> = b
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| {
                if le {
                    u16::from_le_bytes(c)
                } else {
                    u16::from_be_bytes(c)
                }
            })
            .collect();
        String::from_utf16_lossy(&u)
    }
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [a, 0, b, 0, ..] if *a != 0 && *b != 0 => utf16(bytes, true),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => bytes.iter().map(|&b| b as char).collect(),
        },
    }
}

/// Standard plate formats as (rows, columns).
const PLATES: [(usize, usize); 7] = [(2, 3), (3, 4), (4, 6), (6, 8), (8, 12), (16, 24), (32, 48)];

/// The plate dimensions for a well count (6 … 1536).
fn plate_of_wells(n: usize) -> Option<(usize, usize)> {
    PLATES.iter().copied().find(|(r, c)| r * c == n)
}

/// The smallest standard plate holding `rows × cols`, or exactly that.
fn snap_plate(rows: usize, cols: usize) -> (usize, usize) {
    PLATES
        .iter()
        .copied()
        .find(|&(r, c)| r >= rows && c >= cols)
        .unwrap_or((rows, cols))
}

/// Row letter(s) to index: A = 0 … Z = 25, AA = 26 … AF = 31.
fn row_index(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    match b {
        [c] if c.is_ascii_uppercase() => Some((c - b'A') as usize),
        [b'A', c] if c.is_ascii_uppercase() => Some(26 + (c - b'A') as usize),
        _ => None,
    }
}

/// Row index to letter(s).
pub fn row_name(i: usize) -> String {
    if i < 26 {
        ((b'A' + i as u8) as char).to_string()
    } else {
        format!("A{}", (b'A' + (i - 26) as u8) as char)
    }
}

/// A well name such as `B7` or `AF48` to (row, column), zero-based. Leading zeros (`A01`)
/// are accepted.
pub fn parse_well(s: &str) -> Option<(usize, usize)> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = s.split_at(split);
    let r = row_index(&letters.to_ascii_uppercase())?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let c: usize = digits.parse().ok()?;
    (1..=48).contains(&c).then(|| (r, c - 1))
}

/// A well's name.
pub fn well_name(r: usize, c: usize) -> String {
    format!("{}{}", row_name(r), c + 1)
}

/// A measured value; text such as `OVRFLW`, `>40.000` or `?????` is not a number.
fn value(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `H:MM:SS`, `M:SS` or plain seconds, to seconds.
fn seconds(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split(':').collect();
    let nums: Option<Vec<f64>> = parts.iter().map(|p| p.trim().parse::<f64>().ok()).collect();
    let nums = nums?;
    match nums.as_slice() {
        [h, m, sec] => Some(h * 3600.0 + m * 60.0 + sec),
        [m, sec] => Some(m * 60.0 + sec),
        _ => None,
    }
}

/// Counts text found where a number was expected, for one note per file.
#[derive(Default)]
struct TextCount {
    n: usize,
    first: Option<String>,
}

impl TextCount {
    fn cell(&mut self, s: &str) -> f64 {
        match value(s) {
            Some(v) => v,
            None => {
                let t = s.trim();
                if !t.is_empty() {
                    self.n += 1;
                    if self.first.is_none() {
                        self.first = Some(t.to_string());
                    }
                }
                f64::NAN
            }
        }
    }

    fn note(&self, notes: &mut Vec<String>) {
        if let Some(f) = &self.first {
            notes.push(format!(
                "{} well value{} not a number (the first is \"{f}\") and {} left empty.",
                self.n,
                if self.n == 1 { " was" } else { "s were" },
                if self.n == 1 { "was" } else { "were" },
            ));
        }
    }
}

fn split_lines(text: &str, sep: char) -> Vec<Vec<String>> {
    text.lines()
        .map(|l| {
            l.trim_end_matches('\r')
                .split(sep)
                .map(str::to_string)
                .collect()
        })
        .collect()
}

fn blank(f: &[String]) -> bool {
    f.iter().all(|c| c.trim().is_empty())
}

fn joined(f: &[String]) -> String {
    f.iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Recognise the file from its content.
pub fn sniff(name: &str, bytes: &[u8]) -> Option<Vendor> {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "xlsx" | "xls" => magellan_header(bytes).map(|_| Vendor::Magellan),
        "txt" | "csv" | "tsv" | "tab" => sniff_text(&decode_text(bytes)),
        _ => None,
    }
}

fn sniff_text(text: &str) -> Option<Vendor> {
    let head: Vec<&str> = text.lines().take(40).collect();
    let first = head.iter().find(|l| !l.trim().is_empty())?;
    if first
        .trim_start_matches('\u{feff}')
        .starts_with("##BLOCKS=")
    {
        return Some(Vendor::SoftMaxPro);
    }
    if head.iter().take(6).any(|l| l.contains("Test name:"))
        || first.trim_start_matches('\u{feff}').starts_with("User:")
    {
        return Some(Vendor::BmgMars);
    }
    let has = |s: &str| {
        text.lines()
            .take(200)
            .any(|l| l.trim_start().starts_with(s))
    };
    if has("Software Version") && (has("Procedure Details") || has("Plate Number")) {
        return Some(Vendor::Gen5);
    }
    None
}

/// Read a plate-reader export, or `None` when the file is not one.
pub fn read(name: &str, bytes: &[u8]) -> Result<Option<PlateExport>, String> {
    let Some(vendor) = sniff(name, bytes) else {
        return Ok(None);
    };
    let mut export = match vendor {
        Vendor::SoftMaxPro => read_softmax(&decode_text(bytes))?,
        Vendor::BmgMars => read_bmg(&decode_text(bytes))?,
        Vendor::Gen5 => read_gen5(&decode_text(bytes))?,
        Vendor::Magellan => read_magellan(bytes)?,
    };
    if export.reads.is_empty() {
        return Err(format!(
            "this {} export holds no plate data Dioptra can read",
            vendor.label()
        ));
    }
    let kinetic = export.reads[0].kinetic();
    let other = export
        .reads
        .iter()
        .filter(|r| r.kinetic() != kinetic)
        .count();
    if other > 0 {
        export.notes.push(format!(
            "{other} {} read{} not imported.",
            if kinetic { "endpoint" } else { "kinetic" },
            if other == 1 { " was" } else { "s were" }
        ));
    }
    Ok(Some(export))
}

// ---------------------------------------------------------------------------
// SoftMax Pro
// ---------------------------------------------------------------------------

fn read_softmax(text: &str) -> Result<PlateExport, String> {
    let lines = split_lines(text, '\t');
    let mut notes = Vec::new();
    let mut reads = Vec::new();
    let mut texts = TextCount::default();
    let mut groups = 0;
    let mut i = lines
        .iter()
        .position(|l| l[0].trim_start_matches('\u{feff}').starts_with("##BLOCKS="))
        .ok_or("not a SoftMax Pro export: no ##BLOCKS= line")?
        + 1;
    let n = lines.len();
    let mut plates = Vec::new();
    while i < n {
        let start = i;
        while i < n && lines[i][0].trim() != "~End" {
            i += 1;
        }
        let block = &lines[start..i];
        i += 1;
        let Some(head) = block.iter().position(|l| !blank(l)) else {
            continue;
        };
        let block = &block[head..];
        let tag = block[0][0].trim();
        if tag.starts_with("Plate:") {
            plates.push(block);
        } else if tag.starts_with("Group:") {
            groups += 1;
        }
    }
    let many = plates.len() > 1;
    for block in plates {
        softmax_plate(block, many, &mut reads, &mut notes, &mut texts)?;
    }
    if groups > 0 {
        notes.push(format!(
            "{groups} group (calculation) block{} not imported; the well values were.",
            if groups == 1 { " was" } else { "s were" }
        ));
    }
    texts.note(&mut notes);
    Ok(PlateExport {
        vendor: Vendor::SoftMaxPro,
        reads,
        notes,
    })
}

fn softmax_plate(
    block: &[Vec<String>],
    many: bool,
    reads: &mut Vec<PlateRead>,
    notes: &mut Vec<String>,
    texts: &mut TextCount,
) -> Result<(), String> {
    let h = &block[0];
    let get = |k: usize| h.get(k).map(|s| s.trim()).unwrap_or("");
    let plate = get(1).to_string();
    let format = get(3);
    let read_type = get(4);
    let mode = get(5);
    let off = usize::from(mode == "Fluorescence");
    let data_type = get(6 + off);
    let points: usize = get(8 + off).parse().unwrap_or(1).max(1);
    let n_wl: usize = get(14 + off).parse().unwrap_or(1).max(1);
    let wavelengths: Vec<String> = get(15 + off)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let wells: usize = get(18 + off).parse().unwrap_or(0);
    if !matches!(read_type, "Endpoint" | "Kinetic") {
        notes.push(format!(
            "Plate \"{plate}\" is a {} read and was skipped.",
            read_type.to_lowercase()
        ));
        return Ok(());
    }
    let (rows, cols) = plate_of_wells(wells)
        .ok_or_else(|| format!("plate \"{plate}\": unknown plate size {wells}"))?;
    let kinetic = read_type == "Kinetic";
    let name_of = |w: usize| -> String {
        let wl = wavelengths.get(w).map(String::as_str).unwrap_or("");
        let mut s = if wl.is_empty() || wl == "0" {
            mode.to_string()
        } else {
            format!("{wl} nm")
        };
        if n_wl > 1 && wavelengths.iter().filter(|x| *x == wl).count() > 1 {
            s = format!("{s} ({})", w + 1);
        }
        if data_type == "Reduced" {
            s.push_str(" (reduced)");
        }
        if many {
            format!("{plate} · {s}")
        } else {
            s
        }
    };
    let mut out: Vec<PlateRead> = (0..n_wl)
        .map(|w| PlateRead::new(name_of(w), rows, cols))
        .collect();
    let body = &block[1..];
    if format == "PlateFormat" {
        // Column header line, then `rows` lines per time point; each line holds the time,
        // the temperature, then the wavelengths side by side, separated by one empty field.
        let mut k = 1; // skip the column header
        for p in 0..points {
            while k < body.len() && body[k].iter().all(|c| c.is_empty()) && body[k].len() <= 1 {
                k += 1;
            }
            if k + rows > body.len() {
                if p == 0 {
                    return Err(format!("plate \"{plate}\": the plate grid is cut short"));
                }
                notes.push(format!(
                    "Plate \"{plate}\": {p} of {points} time points were found."
                ));
                break;
            }
            let grid = &body[k..k + rows];
            let t = if kinetic {
                Some(seconds(&grid[0][0]).ok_or_else(|| {
                    format!("plate \"{plate}\": \"{}\" is not a time", grid[0][0])
                })?)
            } else {
                None
            };
            for (w, read) in out.iter_mut().enumerate() {
                let mut frame = read.blank_frame();
                for (r, line) in grid.iter().enumerate() {
                    for c in 0..cols {
                        let at = 2 + w * (cols + 1) + c;
                        frame[r * cols + c] = texts.cell(line.get(at).map_or("", String::as_str));
                    }
                }
                if let Some(t) = t {
                    read.times.push(t);
                }
                read.frames.push(frame);
            }
            k += rows;
        }
    } else if format == "TimeFormat" {
        // Column header (Time, Temperature, A1, A2, …), then one block of rows per
        // wavelength, each ended by an empty line.
        let header = &body[0];
        let well_at: Vec<Option<(usize, usize)>> = header
            .iter()
            .map(|c| parse_well(c).filter(|&(r, c)| r < rows && c < cols))
            .collect();
        let mut k = 1;
        for read in &mut out {
            while k < body.len() && blank(&body[k]) {
                k += 1;
            }
            while k < body.len() && !blank(&body[k]) {
                let line = &body[k];
                let mut frame = read.blank_frame();
                for (j, w) in well_at.iter().enumerate() {
                    if let Some((r, c)) = w {
                        frame[r * cols + c] = texts.cell(line.get(j).map_or("", String::as_str));
                    }
                }
                if kinetic {
                    match seconds(&line[0]) {
                        Some(t) => read.times.push(t),
                        None => break,
                    }
                }
                read.frames.push(frame);
                k += 1;
                if !kinetic {
                    break;
                }
            }
        }
    } else {
        return Err(format!(
            "plate \"{plate}\": unknown export format \"{format}\""
        ));
    }
    reads.extend(out.into_iter().filter(|r| !r.frames.is_empty()));
    Ok(())
}

// ---------------------------------------------------------------------------
// Grids (BMG MARS and Gen5)
// ---------------------------------------------------------------------------

/// One row line of a plate grid: plate row, cells, label (Gen5's last column).
struct GridLine {
    row: usize,
    cells: Vec<String>,
    label: String,
}

/// A plate grid: a header line `"", 1, 2, …` followed by lines that start with a row letter
/// (or, in Gen5, continue the previous row with another label).
struct Grid {
    title: String,
    cols: usize,
    lines: Vec<GridLine>,
}

fn grid_header(f: &[String]) -> Option<usize> {
    if f.len() < 3 || !f[0].trim().is_empty() {
        return None;
    }
    let mut n = 0;
    for (k, c) in f[1..].iter().enumerate() {
        let c = c.trim();
        if c == (k + 1).to_string() {
            n = k + 1;
        } else if c.is_empty() || n > 0 {
            break;
        } else {
            return None;
        }
    }
    (n >= 2).then_some(n)
}

fn find_grids(lines: &[Vec<String>]) -> Vec<Grid> {
    let mut grids = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(cols) = grid_header(&lines[i]) else {
            i += 1;
            continue;
        };
        let title = lines[..i]
            .iter()
            .rev()
            .find(|l| !blank(l))
            .map(|l| joined(l))
            .unwrap_or_default();
        let mut g = Grid {
            title,
            cols,
            lines: Vec::new(),
        };
        let mut row = None;
        let mut j = i + 1;
        while j < lines.len() {
            let f = &lines[j];
            let f0 = f[0].trim();
            if let Some(r) = row_index(f0) {
                row = Some(r);
            } else if !(f0.is_empty() && row.is_some() && !blank(f)) {
                break;
            }
            let cells = (1..=cols)
                .map(|k| f.get(k).cloned().unwrap_or_default())
                .collect();
            let label = f
                .get(cols + 1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            g.lines.push(GridLine {
                row: row.expect("row set"),
                cells,
                label,
            });
            j += 1;
        }
        if !g.lines.is_empty() {
            grids.push(g);
        }
        i = j.max(i + 1);
    }
    grids
}

/// The reads of one grid, one per label (lines without a label count by position in the row).
fn grid_reads(g: &Grid, texts: &mut TextCount) -> Vec<PlateRead> {
    let max_row = g.lines.iter().map(|l| l.row).max().unwrap_or(0);
    let (rows, cols) = snap_plate(max_row + 1, g.cols);
    let mut order: Vec<String> = Vec::new();
    let mut by: BTreeMap<String, PlateRead> = BTreeMap::new();
    let mut k = 0;
    let mut last_row = usize::MAX;
    for l in &g.lines {
        if l.row != last_row {
            k = 0;
            last_row = l.row;
        } else {
            k += 1;
        }
        let name = if !l.label.is_empty() {
            l.label.clone()
        } else if k == 0 {
            g.title.clone()
        } else {
            format!("{} ({})", g.title, k + 1)
        };
        let read = by.entry(name.clone()).or_insert_with(|| {
            order.push(name.clone());
            let mut r = PlateRead::new(name.clone(), rows, cols);
            r.frames.push(r.blank_frame());
            r
        });
        for (c, s) in l.cells.iter().enumerate() {
            read.frames[0][l.row * cols + c] = texts.cell(s);
        }
    }
    order.into_iter().filter_map(|n| by.remove(&n)).collect()
}

fn grid_separator(text: &str) -> char {
    for l in text.lines() {
        for sep in [',', '\t', ';'] {
            if grid_header(&l.split(sep).map(str::to_string).collect::<Vec<_>>()).is_some() {
                return sep;
            }
        }
    }
    ','
}

fn read_bmg(text: &str) -> Result<PlateExport, String> {
    let lines = split_lines(text, grid_separator(text));
    let mut texts = TextCount::default();
    let mut notes = Vec::new();
    let mut reads = Vec::new();
    for g in find_grids(&lines) {
        reads.extend(grid_reads(&g, &mut texts));
    }
    if reads.is_empty() {
        notes.push(
            "No plate grid was found; table-view (kinetic) exports are not read as plates.".into(),
        );
    }
    texts.note(&mut notes);
    Ok(PlateExport {
        vendor: Vendor::BmgMars,
        reads,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Gen5
// ---------------------------------------------------------------------------

fn read_gen5(text: &str) -> Result<PlateExport, String> {
    let lines = split_lines(text, '\t');
    let mut texts = TextCount::default();
    let mut notes = Vec::new();
    let mut reads = Vec::new();
    // Kinetic blocks: a title line (the read), then `Time, T° …, A1, A2, …` and one line
    // per time point.
    let mut i = 0;
    while i < lines.len() {
        let f = &lines[i];
        let is_kinetic =
            f[0].trim() == "Time" && f.len() > 2 && f[2..].iter().any(|c| parse_well(c).is_some());
        if !is_kinetic {
            i += 1;
            continue;
        }
        let title = lines[..i]
            .iter()
            .rev()
            .find(|l| !blank(l))
            .map(|l| joined(l))
            .unwrap_or_else(|| "Read".into());
        let wells: Vec<Option<(usize, usize)>> = f.iter().map(|c| parse_well(c)).collect();
        let max_r = wells.iter().flatten().map(|w| w.0).max().unwrap_or(0);
        let max_c = wells.iter().flatten().map(|w| w.1).max().unwrap_or(0);
        let (rows, cols) = snap_plate(max_r + 1, max_c + 1);
        let mut read = PlateRead::new(title, rows, cols);
        let mut j = i + 1;
        while j < lines.len() {
            let l = &lines[j];
            let Some(t) = seconds(&l[0]) else { break };
            j += 1;
            if l.iter().skip(2).all(|c| c.trim().is_empty()) {
                continue; // Gen5 pads the planned reads that never ran.
            }
            let mut frame = read.blank_frame();
            for (k, w) in wells.iter().enumerate() {
                if let Some((r, c)) = w {
                    frame[r * cols + c] = texts.cell(l.get(k).map_or("", String::as_str));
                }
            }
            read.times.push(t);
            read.frames.push(frame);
        }
        if !read.frames.is_empty() {
            reads.push(read);
        }
        i = j;
    }
    let kinetic = !reads.is_empty();
    let mut skipped = 0;
    for g in find_grids(&lines) {
        if g.title == "Layout" || g.lines.iter().any(|l| l.label == "Well ID") {
            continue;
        }
        if kinetic {
            skipped += 1;
            continue;
        }
        reads.extend(grid_reads(&g, &mut texts));
    }
    if skipped > 0 {
        notes.push("The calculated results grid of this kinetic export was not imported; the time course was.".into());
    }
    texts.note(&mut notes);
    Ok(PlateExport {
        vendor: Vendor::Gen5,
        reads,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Tecan Magellan (.xlsx, list layout)
// ---------------------------------------------------------------------------

fn sheet_cells(bytes: &[u8]) -> Option<Vec<Vec<String>>> {
    use calamine::{open_workbook_auto_from_rs, Data, Reader};
    let mut wb = open_workbook_auto_from_rs(std::io::Cursor::new(bytes.to_vec())).ok()?;
    let name = wb.sheet_names().first().cloned()?;
    let range = wb.worksheet_range(&name).ok()?;
    Some(
        range
            .rows()
            .map(|r| {
                r.iter()
                    .map(|c| match c {
                        Data::Empty => String::new(),
                        Data::Int(i) => i.to_string(),
                        Data::Float(f) => format!("{f}"),
                        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(d) => d.as_f64().to_string(),
                        Data::Error(e) => format!("#{e:?}"),
                    })
                    .collect()
            })
            .collect(),
    )
}

/// The header row and well column of a Magellan list export.
fn magellan_header(bytes: &[u8]) -> Option<(Vec<Vec<String>>, usize, usize)> {
    // Cheap check before parsing: an xlsx is a zip.
    if !bytes.starts_with(b"PK") {
        return None;
    }
    let cells = sheet_cells(bytes)?;
    let (r, c) = cells.iter().take(10).enumerate().find_map(|(r, row)| {
        row.iter()
            .position(|c| c.trim().eq_ignore_ascii_case("well positions"))
            .map(|c| (r, c))
    })?;
    Some((cells, r, c))
}

fn read_magellan(bytes: &[u8]) -> Result<PlateExport, String> {
    let (cells, hr, wc) = magellan_header(bytes).ok_or("not a Magellan list export")?;
    let header = &cells[hr];
    let mut texts = TextCount::default();
    let mut notes = Vec::new();
    // Value columns: every titled column but the wells.
    let value_cols: Vec<usize> = (0..header.len())
        .filter(|&c| c != wc && !header[c].trim().is_empty())
        .collect();
    let mut wells: Vec<(usize, (usize, usize))> = Vec::new();
    let mut times: BTreeMap<usize, f64> = BTreeMap::new();
    for (r, row) in cells.iter().enumerate().skip(hr + 1) {
        let w = row.get(wc).map(String::as_str).unwrap_or("");
        if let Some(p) = parse_well(w) {
            wells.push((r, p));
        } else if w.trim().is_empty() {
            for &c in &value_cols {
                let s = row.get(c).map(|s| s.trim()).unwrap_or("");
                if let Some(t) = s
                    .strip_suffix('s')
                    .and_then(|t| t.trim().parse::<f64>().ok())
                {
                    times.insert(c, t);
                }
            }
        }
    }
    if wells.is_empty() {
        return Err("the Magellan export lists no wells".into());
    }
    let max_r = wells.iter().map(|w| w.1 .0).max().unwrap_or(0);
    let max_c = wells.iter().map(|w| w.1 .1).max().unwrap_or(0);
    let (rows, cols) = snap_plate(max_r + 1, max_c + 1);
    // Drop columns without numbers (the plate name, for instance).
    let numeric: Vec<usize> = value_cols
        .into_iter()
        .filter(|&c| {
            wells
                .iter()
                .any(|(r, _)| value(cells[*r].get(c).map_or("", String::as_str)).is_some())
        })
        .collect();
    let frame_of = |c: usize, texts: &mut TextCount| -> Vec<f64> {
        let mut f = vec![f64::NAN; rows * cols];
        for (r, (wr, wcol)) in &wells {
            f[wr * cols + wcol] = texts.cell(cells[*r].get(c).map_or("", String::as_str));
        }
        f
    };
    let col_times: Vec<f64> = numeric
        .iter()
        .filter_map(|c| times.get(c).copied())
        .collect();
    let kinetic = numeric.len() > 1
        && col_times.len() == numeric.len()
        && col_times.windows(2).all(|w| w[1] > w[0]);
    let mut reads = Vec::new();
    if kinetic {
        let names: BTreeSet<&str> = numeric.iter().map(|&c| header[c].trim()).collect();
        let name = if names.len() == 1 {
            names.into_iter().next().unwrap_or("Kinetic").to_string()
        } else {
            "Kinetic".to_string()
        };
        let mut read = PlateRead::new(name, rows, cols);
        for &c in &numeric {
            read.times.push(times[&c]);
            read.frames.push(frame_of(c, &mut texts));
        }
        reads.push(read);
    } else {
        for &c in &numeric {
            let mut read = PlateRead::new(header[c].trim().to_string(), rows, cols);
            read.frames.push(frame_of(c, &mut texts));
            reads.push(read);
        }
    }
    texts.note(&mut notes);
    Ok(PlateExport {
        vendor: Vendor::Magellan,
        reads,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Plate maps
// ---------------------------------------------------------------------------

/// What a plate map says about one well.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WellLabel {
    /// Sample or group name.
    pub group: Option<String>,
    /// Dose or concentration.
    pub dose: Option<f64>,
}

/// Wells to samples, groups and doses.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlateMap {
    /// Labelled wells by (row, column), zero-based.
    pub wells: BTreeMap<(usize, usize), WellLabel>,
}

impl PlateMap {
    /// Whether any well has a dose.
    pub fn has_doses(&self) -> bool {
        self.wells.values().any(|w| w.dose.is_some())
    }
}

fn parse_label(s: &str) -> Result<Option<WellLabel>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    if let Some((g, d)) = s.rsplit_once('@') {
        let dose = value(d).ok_or_else(|| format!("\"{s}\": the dose after @ is not a number"))?;
        let g = g.trim();
        return Ok(Some(WellLabel {
            group: (!g.is_empty()).then(|| g.to_string()),
            dose: Some(dose),
        }));
    }
    Ok(Some(match value(s) {
        Some(d) => WellLabel {
            group: None,
            dose: Some(d),
        },
        None => WellLabel {
            group: Some(s.to_string()),
            dose: None,
        },
    }))
}

/// Parse a plate map, pasted either as a grid (rows A, B, …; columns 1, 2, …; the header
/// row and the row letters are optional) or as a list of `well, label[, dose]` lines.
///
/// A label is a group name (`Control`), a dose (`10`), or both (`Drug A @ 10`). In the list
/// form the dose may also be a third column. Empty cells leave a well out. Fields are
/// separated by tabs (as copied from a spreadsheet), semicolons or commas.
pub fn parse_plate_map(text: &str) -> Result<PlateMap, String> {
    let sep = if text.contains('\t') {
        '\t'
    } else if text.contains(';') {
        ';'
    } else {
        ','
    };
    let mut rows: Vec<Vec<String>> = split_lines(text, sep);
    rows.retain(|r| !blank(r));
    while rows
        .iter()
        .all(|r| r.last().is_some_and(|c| c.trim().is_empty()) && r.len() > 1)
    {
        for r in &mut rows {
            r.pop();
        }
    }
    let mut map = PlateMap::default();
    if rows.is_empty() {
        return Ok(map);
    }
    let is_well = |r: &Vec<String>| parse_well(&r[0]).is_some();
    let list_from = if rows.iter().all(is_well) {
        Some(0)
    } else if rows.len() > 1 && rows[1..].iter().all(is_well) {
        Some(1)
    } else {
        None
    };
    if let Some(from) = list_from {
        for r in &rows[from..] {
            let w = parse_well(&r[0]).expect("checked");
            let label = r.get(1).map(String::as_str).unwrap_or("");
            let Some(mut l) = parse_label(label)? else {
                continue;
            };
            if let Some(d) = r.get(2).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let d = value(d)
                    .ok_or_else(|| format!("well {}: dose \"{d}\" is not a number", r[0].trim()))?;
                if l.dose.is_none() && l.group.is_none() {
                    l.group = Some(label.trim().to_string());
                }
                l.dose = Some(d);
            }
            map.wells.insert(w, l);
        }
        return Ok(map);
    }
    // Grid.
    let mut body = &rows[..];
    if grid_header(&body[0]).is_some()
        || body[0]
            .iter()
            .enumerate()
            .all(|(k, c)| c.trim() == (k + 1).to_string())
    {
        body = &body[1..];
    }
    let letters = body.iter().all(|r| row_index(r[0].trim()).is_some());
    for (i, r) in body.iter().enumerate() {
        let (row, cells) = if letters {
            (row_index(r[0].trim()).expect("checked"), &r[1..])
        } else {
            (i, &r[..])
        };
        for (c, s) in cells.iter().enumerate() {
            if let Some(l) = parse_label(s)? {
                map.wells.insert((row, c), l);
            }
        }
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

fn source(file: &str, column: String) -> ColumnSource {
    ColumnSource::Imported {
        file: file.to_string(),
        sheet: None,
        column,
    }
}

fn column(values: Vec<f64>, title: String, file: &str, from: String) -> Column {
    Column {
        title,
        values,
        source: source(file, from),
        ..Default::default()
    }
}

/// A group key of the plate map: the group name, the dose, or both.
fn group_name(l: &WellLabel, with_dose: bool) -> String {
    match (&l.group, l.dose) {
        (Some(g), Some(d)) if with_dose => format!("{g} @ {}", crate::util::format_number(d, None)),
        (Some(g), _) => g.clone(),
        (None, Some(d)) => crate::util::format_number(d, None),
        (None, None) => String::new(),
    }
}

/// Build the table for a plate-reader export (see the module docs for the shapes).
pub fn build_table(
    id: String,
    title: String,
    file: &str,
    export: &PlateExport,
    map: Option<&PlateMap>,
    report: &mut PasteReport,
) -> Result<Table, String> {
    let reads = export.used_reads();
    let first = reads.first().ok_or("the file holds no plate reads")?;
    let (prows, pcols) = (first.rows, first.cols);
    let many = reads.len() > 1;
    let ds_name = |read: &PlateRead, what: &str| {
        if many {
            format!("{what} · {}", read.name)
        } else {
            what.to_string()
        }
    };
    let map = map.filter(|m| !m.wells.is_empty());
    if let Some(m) = map {
        let outside = m
            .wells
            .keys()
            .filter(|(r, c)| *r >= prows || *c >= pcols)
            .count();
        if outside > 0 {
            report.notes.push(format!(
                "{outside} well{} of the plate map lie outside the {prows} × {pcols} plate and were ignored.",
                if outside == 1 { "" } else { "s" }
            ));
        }
    }
    // Wells of the map inside the plate, in plate order, grouped.
    let groups: Vec<(String, Wells)> = map
        .map(|m| {
            let mut order: Vec<String> = Vec::new();
            let mut by: BTreeMap<String, Wells> = BTreeMap::new();
            for (&(r, c), l) in &m.wells {
                if r >= prows || c >= pcols {
                    continue;
                }
                let key = if m.has_doses() {
                    l.group.clone().unwrap_or_default()
                } else {
                    group_name(l, false)
                };
                if !by.contains_key(&key) {
                    order.push(key.clone());
                }
                by.entry(key).or_default().push((r, c, l.dose));
            }
            order
                .into_iter()
                .map(|k| {
                    let v = by.remove(&k).unwrap_or_default();
                    (k, v)
                })
                .collect()
        })
        .unwrap_or_default();
    let mut t;
    if first.kinetic() {
        // X = time (union of the reads' time points), datasets per well or per group.
        let mut all: Vec<f64> = reads.iter().flat_map(|r| r.times.iter().copied()).collect();
        all.sort_by(f64::total_cmp);
        all.dedup();
        let row_of = |t: f64| all.binary_search_by(|x| x.total_cmp(&t)).expect("time");
        t = Table::new(
            id,
            TableKind::Xy,
            title,
            0,
            SubcolumnFormat::Values(1),
            all.len(),
        );
        t.x = Some(column(all.clone(), "Time (s)".into(), file, "Time".into()));
        let series = |read: &PlateRead, r: usize, c: usize| -> Vec<f64> {
            let mut v = vec![f64::NAN; all.len()];
            for (k, f) in read.frames.iter().enumerate() {
                v[row_of(read.times[k])] = f[r * read.cols + c];
            }
            v
        };
        for read in &reads {
            if map.is_some() {
                for (g, wells) in &groups {
                    let name = if g.is_empty() {
                        "Unlabelled".to_string()
                    } else {
                        g.clone()
                    };
                    let subs: Vec<Column> = wells
                        .iter()
                        .map(|&(r, c, _)| {
                            let w = well_name(r, c);
                            column(
                                series(read, r, c),
                                w.clone(),
                                file,
                                format!("{} {w}", read.name),
                            )
                        })
                        .collect();
                    push(&mut t, ds_name(read, &name), subs);
                }
            } else {
                for r in 0..read.rows {
                    for c in 0..read.cols {
                        let v = series(read, r, c);
                        if v.iter().all(|x| x.is_nan()) {
                            continue;
                        }
                        let w = well_name(r, c);
                        push(
                            &mut t,
                            ds_name(read, &w),
                            vec![column(v, String::new(), file, format!("{} {w}", read.name))],
                        );
                    }
                }
            }
        }
    } else if let Some(m) = map.filter(|m| m.has_doses()) {
        // XY: X = dose, a data set per group, replicate wells as subcolumns.
        let mut doses: Vec<f64> = m.wells.values().filter_map(|l| l.dose).collect();
        doses.sort_by(f64::total_cmp);
        doses.dedup();
        let skipped = m.wells.values().filter(|l| l.dose.is_none()).count();
        if skipped > 0 {
            report.notes.push(format!(
                "{skipped} well{} of the plate map had no dose and {} left out of the XY table.",
                if skipped == 1 { "" } else { "s" },
                if skipped == 1 { "was" } else { "were" }
            ));
        }
        t = Table::new(
            id,
            TableKind::Xy,
            title,
            0,
            SubcolumnFormat::Values(1),
            doses.len(),
        );
        t.x = Some(column(
            doses.clone(),
            "Dose".into(),
            file,
            "plate map".into(),
        ));
        for read in &reads {
            for (g, wells) in &groups {
                let mut per: Vec<Vec<f64>> = vec![Vec::new(); doses.len()];
                for &(r, c, d) in wells {
                    if let Some(d) = d {
                        let k = doses.binary_search_by(|x| x.total_cmp(&d)).expect("dose");
                        per[k].push(read.frames[0][r * read.cols + c]);
                    }
                }
                let n = per.iter().map(Vec::len).max().unwrap_or(0);
                if n == 0 {
                    continue;
                }
                let subs = (0..n)
                    .map(|j| {
                        let v = per
                            .iter()
                            .map(|p| p.get(j).copied().unwrap_or(f64::NAN))
                            .collect();
                        column(v, String::new(), file, read.name.clone())
                    })
                    .collect();
                let name = if g.is_empty() {
                    "Response".to_string()
                } else {
                    g.clone()
                };
                push(&mut t, ds_name(read, &name), subs);
            }
        }
    } else if map.is_some() {
        // Column: a data set per group, wells down the rows.
        let n = groups.iter().map(|g| g.1.len()).max().unwrap_or(0);
        t = Table::new(
            id,
            TableKind::Column,
            title,
            0,
            SubcolumnFormat::Values(1),
            n,
        );
        for read in &reads {
            for (g, wells) in &groups {
                let mut v: Vec<f64> = wells
                    .iter()
                    .map(|&(r, c, _)| read.frames[0][r * read.cols + c])
                    .collect();
                v.resize(n, f64::NAN);
                push(
                    &mut t,
                    ds_name(read, g),
                    vec![column(v, String::new(), file, read.name.clone())],
                );
            }
        }
    } else {
        // Grouped: plate rows down, a data set per read, plate columns as subcolumns.
        t = Table::new(
            id,
            TableKind::Grouped,
            title,
            0,
            SubcolumnFormat::Values(1),
            prows,
        );
        t.row_titles = (0..prows).map(row_name).collect();
        for read in &reads {
            let subs = (0..read.cols)
                .map(|c| {
                    let v = (0..prows)
                        .map(|r| {
                            if r < read.rows {
                                read.frames[0][r * read.cols + c]
                            } else {
                                f64::NAN
                            }
                        })
                        .collect();
                    column(
                        v,
                        (c + 1).to_string(),
                        file,
                        format!("{} column {}", read.name, c + 1),
                    )
                })
                .collect();
            push(&mut t, read.name.clone(), subs);
        }
    }
    let rows = t.rows;
    for c in t.columns_mut() {
        c.values.resize(rows, f64::NAN);
    }
    let numeric = t
        .datasets
        .iter()
        .flat_map(|d| &d.subcolumns)
        .map(|c| c.values.iter().filter(|v| !v.is_nan()).count())
        .sum();
    report.rows = rows;
    report.cols = t.col_count();
    report.numeric_cells = numeric;
    let shape = match t.kind {
        TableKind::Xy if first.kinetic() => "kinetic, as an XY table (time in seconds)",
        TableKind::Xy => "endpoint, as an XY table (dose from the plate map)",
        TableKind::Column => "endpoint, as a column table (groups from the plate map)",
        _ => "endpoint, as a grouped table (plate rows by columns)",
    };
    report.notes.insert(
        0,
        format!(
            "Read a {} export: {} read{} ({}) of a {}-well plate, {shape}.",
            export.vendor.label(),
            reads.len(),
            if many { "s" } else { "" },
            reads
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            prows * pcols
        ),
    );
    report.notes.extend(export.notes.iter().cloned());
    Ok(t)
}

/// Wells of one group: (row, column, dose).
type Wells = Vec<(usize, usize, Option<f64>)>;

fn push(t: &mut Table, name: String, subcolumns: Vec<Column>) {
    let n = subcolumns.len().max(1);
    t.datasets.push(Dataset {
        name,
        format: SubcolumnFormat::Values(n as u16),
        subcolumns,
        var_type: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wells_and_times() {
        assert_eq!(parse_well("A1"), Some((0, 0)));
        assert_eq!(parse_well("h12"), Some((7, 11)));
        assert_eq!(parse_well("P24"), Some((15, 23)));
        assert_eq!(parse_well("AF48"), Some((31, 47)));
        assert_eq!(parse_well("A01"), Some((0, 0)));
        assert_eq!(parse_well("A"), None);
        assert_eq!(parse_well("A0"), None);
        assert_eq!(well_name(31, 47), "AF48");
        assert_eq!(seconds("00:01:30"), Some(90.0));
        assert_eq!(seconds("1:04:22"), Some(3862.0));
        assert_eq!(seconds("0:30"), Some(30.0));
        assert_eq!(seconds("abc"), None);
    }

    #[test]
    fn plate_map_grid_and_list() {
        let grid = "\t1\t2\t3\nA\tCtrl\tDrug @ 1\tDrug@10\nB\t\t0.5\t\n";
        let m = parse_plate_map(grid).unwrap();
        assert_eq!(m.wells.len(), 4);
        assert_eq!(m.wells[&(0, 0)].group.as_deref(), Some("Ctrl"));
        assert_eq!(m.wells[&(0, 2)].dose, Some(10.0));
        assert_eq!(
            m.wells[&(1, 1)],
            WellLabel {
                group: None,
                dose: Some(0.5)
            }
        );
        let list = "Well,Sample,Dose\nA1,Drug,1\nA2,Drug,10\nB1,Blank\n";
        let m = parse_plate_map(list).unwrap();
        assert_eq!(
            m.wells[&(0, 1)],
            WellLabel {
                group: Some("Drug".into()),
                dose: Some(10.0)
            }
        );
        assert_eq!(m.wells[&(1, 0)].group.as_deref(), Some("Blank"));
        // A bare grid without headers.
        let m = parse_plate_map("a\tb\nc\t\n").unwrap();
        assert_eq!(m.wells.len(), 3);
        assert!(parse_plate_map("A1,x @ y").is_err());
    }
}
