//! From a table and a spec to clean numbers for `dioptra-stats`.
//!
//! Every helper honours excluded cells (never used), empty and text cells (missing), and
//! the subcolumn format of each data set. Helpers report what they left out in `notes`
//! so the results sheet can say so (DESIGN P13: missing values are never dropped
//! silently).

use crate::api::{SubcolumnFormat, TableKind, VariableType};
use crate::table::{ColLoc, Table};
use crate::util::letters;

/// Summary values entered instead of raw data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summ {
    /// Mean.
    pub mean: f64,
    /// SD.
    pub sd: f64,
    /// N.
    pub n: usize,
}

/// One group of values (a data set of a column table, a variable, the X column).
#[derive(Debug, Clone, Default)]
pub struct Group {
    /// Display name.
    pub name: String,
    /// Data set index (`None`: the X column).
    pub dataset: Option<usize>,
    /// Usable values.
    pub values: Vec<f64>,
    /// Row of each value (for nested tables: the subcolumn).
    pub rows: Vec<usize>,
    /// Mean, SD and N when the data set holds summary values.
    pub summary: Option<Summ>,
    /// What was left out and why.
    pub notes: Vec<String>,
}

impl Group {
    /// Number of observations (summary N when entered as summary data).
    pub fn n(&self) -> usize {
        match self.summary {
            Some(s) => s.n,
            None => self.values.len(),
        }
    }

    /// Values for computations that depend only on the mean, SD and N: the raw values,
    /// or a synthetic sample with exactly the entered mean, SD and N.
    pub fn effective(&self) -> Vec<f64> {
        match self.summary {
            Some(s) => synthetic(s),
            None => self.values.clone(),
        }
    }
}

/// A sample with exactly the given mean, SD (n - 1) and size: n - 2 values at the mean and
/// two at mean +/- SD sqrt((n - 1)/2). Every statistic that depends only on group means,
/// SDs and sizes (t tests, one- and two-way ANOVA, Tukey, Dunnett, Bartlett) is then
/// exact. Needs n >= 2.
pub fn synthetic(s: Summ) -> Vec<f64> {
    if s.n < 2 {
        return vec![s.mean; s.n];
    }
    let h = s.sd * ((s.n as f64 - 1.0) / 2.0).sqrt();
    let mut v = vec![s.mean; s.n - 2];
    v.push(s.mean - h);
    v.push(s.mean + h);
    v
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Mean, SD and N from a summary row, when the format allows it.
pub fn summ_from(format: SubcolumnFormat, v: &[f64]) -> Result<Summ, String> {
    let g = |i: usize| v.get(i).copied().unwrap_or(f64::NAN);
    let n_of = |x: f64| -> Result<usize, String> {
        if x.is_finite() && x >= 1.0 && x.fract() == 0.0 {
            Ok(x as usize)
        } else {
            Err(format!(
                "N must be a whole number of at least 1 (found {x})."
            ))
        }
    };
    let (m, spread, n) = (g(0), g(1), g(2));
    if !m.is_finite() {
        return Err("the mean is missing".into());
    }
    match format {
        SubcolumnFormat::MeanSdN => Ok(Summ {
            mean: m,
            sd: spread,
            n: n_of(n)?,
        }),
        SubcolumnFormat::MeanSemN => {
            let n = n_of(n)?;
            Ok(Summ {
                mean: m,
                sd: spread * (n as f64).sqrt(),
                n,
            })
        }
        SubcolumnFormat::MeanCiN => {
            let n = n_of(n)?;
            let t = if n > 1 {
                dioptra_stats::dist::t_critical_two_sided(0.05, n as f64 - 1.0)
            } else {
                f64::NAN
            };
            Ok(Summ {
                mean: m,
                sd: spread / t * (n as f64).sqrt(),
                n,
            })
        }
        SubcolumnFormat::MeanSd | SubcolumnFormat::MeanSem => {
            Err("the format has no N, so the sample size is unknown".into())
        }
        SubcolumnFormat::MedianIqr | SubcolumnFormat::LowerUpper => Err(
            "medians and quartiles (or lower and upper limits) do not give a mean and SD".into(),
        ),
        SubcolumnFormat::Values(_) | SubcolumnFormat::Single => Err("not a summary format".into()),
    }
}

/// Whether a format holds summary values rather than raw values.
pub fn is_summary(f: SubcolumnFormat) -> bool {
    !matches!(f, SubcolumnFormat::Values(_) | SubcolumnFormat::Single)
}

/// The data set a flat column belongs to (`None` for X or out of range).
pub fn dataset_of(table: &Table, flat: usize) -> Option<usize> {
    match table.locate(flat) {
        Some(ColLoc::Y { dataset, .. }) => Some(dataset),
        _ => None,
    }
}

fn numeric(table: &Table, d: usize) -> bool {
    table.kind != TableKind::Variables
        || matches!(
            table.datasets[d].var_type,
            None | Some(VariableType::Number)
        )
}

/// Which data sets an analysis uses: those of `columns` (flat indices, in order, each
/// data set once), or every numeric data set (at most `limit`) when `columns` is empty.
/// `Ok(None)` entries stand for the X column.
pub fn pick(
    table: &Table,
    columns: &[usize],
    limit: Option<usize>,
) -> Result<Vec<Option<usize>>, String> {
    let mut out: Vec<Option<usize>> = Vec::new();
    if columns.is_empty() {
        for d in 0..table.datasets.len() {
            if numeric(table, d) {
                out.push(Some(d));
            }
        }
        if let Some(l) = limit {
            out.truncate(l);
        }
        return Ok(out);
    }
    for &c in columns {
        let key = match table.locate(c) {
            Some(ColLoc::X) => None,
            Some(ColLoc::Y { dataset, .. }) => {
                if !numeric(table, dataset) {
                    return Err(format!(
                        "{} is not a number variable; this analysis needs numbers.",
                        table.datasets[dataset].name
                    ));
                }
                Some(dataset)
            }
            None => {
                return Err(format!(
                    "Column {} does not exist in \"{}\" (it has {} columns).",
                    c + 1,
                    table.title,
                    table.col_count()
                ))
            }
        };
        if !out.contains(&key) {
            out.push(key);
        }
    }
    Ok(out)
}

/// The values of one data set (or the X column) as a group.
pub fn group(table: &Table, key: Option<usize>) -> Result<Group, String> {
    let Some(d) = key else {
        let x = table.x.as_ref().ok_or("this table has no X column")?;
        let mut g = Group {
            name: table.col_header(0),
            dataset: None,
            ..Default::default()
        };
        for r in 0..table.rows {
            let v = x.usable_at(r);
            if v.is_finite() {
                g.values.push(v);
                g.rows.push(r);
            }
        }
        return Ok(g);
    };
    let ds = &table.datasets[d];
    let mut g = Group {
        name: ds.name.clone(),
        dataset: Some(d),
        ..Default::default()
    };
    if is_summary(ds.format) {
        let row: Vec<f64> = ds.subcolumns.iter().map(|c| c.usable_at(0)).collect();
        g.summary =
            Some(summ_from(ds.format, &row).map_err(|e| {
                format!("{} holds summary values that cannot be used: {e}", ds.name)
            })?);
        g.notes.push(format!(
            "{} was entered as summary values (mean, SD and N); results that need raw values are not available for it.",
            ds.name
        ));
        return Ok(g);
    }
    let (mut text, mut excluded) = (0, 0);
    if table.kind == TableKind::Nested {
        // One value per subgroup: its mean (DESIGN P19).
        for (k, c) in ds.subcolumns.iter().enumerate() {
            let v: Vec<f64> = c.usable().collect();
            excluded += c.excluded.len();
            if !v.is_empty() {
                g.values.push(v.iter().sum::<f64>() / v.len() as f64);
                g.rows.push(k);
            }
        }
        g.notes.push(format!(
            "{}: each of the {} subgroups was averaged first, so n counts subgroups (the experimental units), not observations.",
            ds.name,
            g.values.len()
        ));
    } else {
        for r in 0..table.rows {
            for c in &ds.subcolumns {
                let v = c.usable_at(r);
                if v.is_finite() {
                    g.values.push(v);
                    g.rows.push(r);
                } else if c.excluded.contains(&r) && !c.values[r].is_nan() {
                    excluded += 1;
                } else if c.text.contains_key(&r) {
                    text += 1;
                }
            }
        }
    }
    if excluded > 0 {
        g.notes.push(format!(
            "{}: {} not used.",
            ds.name,
            plural(excluded, "excluded value was", "excluded values were")
        ));
    }
    if text > 0 {
        g.notes.push(format!(
            "{}: {} not used.",
            ds.name,
            plural(text, "text cell was", "text cells were")
        ));
    }
    Ok(g)
}

/// Groups for `columns` (see [`pick`]).
pub fn groups(
    table: &Table,
    columns: &[usize],
    limit: Option<usize>,
) -> Result<Vec<Group>, String> {
    pick(table, columns, limit)?
        .into_iter()
        .map(|k| group(table, k))
        .collect()
}

/// Fail when a group holds summary values.
pub fn require_raw(gs: &[Group], what: &str) -> Result<(), String> {
    if let Some(g) = gs.iter().find(|g| g.summary.is_some()) {
        return Err(format!(
            "{what} needs the individual values, and {} was entered as summary data (mean, SD and N). This is not possible from summary data; enter the raw values.",
            g.name
        ));
    }
    Ok(())
}

/// Value of a data set at a row as a paired analysis sees it: the single value, or the
/// mean of the usable replicates.
pub fn row_value(table: &Table, d: usize, r: usize) -> f64 {
    let ds = &table.datasets[d];
    let v: Vec<f64> = ds
        .subcolumns
        .iter()
        .map(|c| c.usable_at(r))
        .filter(|v| v.is_finite())
        .collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Values of a data set (or X) by row, NaN where missing.
pub fn column_by_row(table: &Table, key: Option<usize>) -> Vec<f64> {
    (0..table.rows)
        .map(|r| match key {
            Some(d) => row_value(table, d, r),
            None => table.x.as_ref().map_or(f64::NAN, |x| x.usable_at(r)),
        })
        .collect()
}

/// Name of a data set or of X.
pub fn key_name(table: &Table, key: Option<usize>) -> String {
    match key {
        Some(d) => table.datasets[d].name.clone(),
        None => table.col_header(0),
    }
}

/// Paired values aligned by row.
#[derive(Debug, Clone, Default)]
pub struct Paired {
    /// First member of each pair.
    pub x: Vec<f64>,
    /// Second member.
    pub y: Vec<f64>,
    /// Rows used.
    pub rows: Vec<usize>,
    /// Rows with exactly one side present, dropped.
    pub dropped: usize,
}

/// Pairs of two data sets (or X and a data set) by row; rows with a missing side are
/// dropped and counted.
pub fn paired(table: &Table, a: Option<usize>, b: Option<usize>) -> Result<Paired, String> {
    for k in [a, b].into_iter().flatten() {
        if is_summary(table.datasets[k].format) {
            return Err(format!(
                "Paired analyses need the individual values, and {} was entered as summary data. This is not possible from summary data.",
                table.datasets[k].name
            ));
        }
    }
    let (ca, cb) = (column_by_row(table, a), column_by_row(table, b));
    let mut p = Paired::default();
    for r in 0..table.rows {
        let (u, v) = (ca[r], cb[r]);
        match (u.is_finite(), v.is_finite()) {
            (true, true) => {
                p.x.push(u);
                p.y.push(v);
                p.rows.push(r);
            }
            (false, false) => {}
            _ => p.dropped += 1,
        }
    }
    Ok(p)
}

/// A subjects x conditions matrix for repeated measures: one row per table row with any
/// value, rows with a missing condition dropped and counted.
#[derive(Debug, Clone, Default)]
pub struct Matrix {
    /// Complete rows.
    pub m: Vec<Vec<f64>>,
    /// Table row of each.
    pub rows: Vec<usize>,
    /// Incomplete rows dropped.
    pub dropped: usize,
}

/// See [`Matrix`].
pub fn matrix(table: &Table, keys: &[Option<usize>]) -> Result<Matrix, String> {
    for k in keys.iter().flatten() {
        if is_summary(table.datasets[*k].format) {
            return Err(format!(
                "Repeated measures need the individual values, and {} was entered as summary data. This is not possible from summary data.",
                table.datasets[*k].name
            ));
        }
    }
    let cols: Vec<Vec<f64>> = keys.iter().map(|k| column_by_row(table, *k)).collect();
    let mut out = Matrix::default();
    for r in 0..table.rows {
        let row: Vec<f64> = cols.iter().map(|c| c[r]).collect();
        if row.iter().all(|v| v.is_finite()) {
            out.m.push(row);
            out.rows.push(r);
        } else if row.iter().any(|v| v.is_finite()) {
            out.dropped += 1;
        }
    }
    Ok(out)
}

/// Row name of a table row (its title, or its number).
pub fn row_name(table: &Table, r: usize) -> String {
    table
        .row_titles
        .get(r)
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("Row {}", r + 1))
}

/// Cells of a grouped table: rows (factor A) x data sets (factor B) -> replicate values.
#[derive(Debug, Clone, Default)]
pub struct Cells {
    /// `cells[row][dataset]`.
    pub cells: Vec<Vec<Vec<f64>>>,
    /// Names of the rows used.
    pub row_names: Vec<String>,
    /// Names of the data sets.
    pub col_names: Vec<String>,
    /// Table rows used.
    pub rows: Vec<usize>,
    /// Whether any cell came from summary values.
    pub from_summary: bool,
    /// Notes.
    pub notes: Vec<String>,
}

/// See [`Cells`]. Rows without any value are left out.
pub fn cells(table: &Table) -> Result<Cells, String> {
    let mut out = Cells {
        col_names: table.datasets.iter().map(|d| d.name.clone()).collect(),
        ..Default::default()
    };
    let mut excluded = 0;
    for r in 0..table.rows {
        let mut row = Vec::new();
        for ds in &table.datasets {
            if is_summary(ds.format) {
                let v: Vec<f64> = ds.subcolumns.iter().map(|c| c.usable_at(r)).collect();
                if v.iter().all(|x| !x.is_finite()) {
                    row.push(vec![]);
                    continue;
                }
                let s = summ_from(ds.format, &v).map_err(|e| {
                    format!("{} {}: {e}", ds.name, row_name(table, r).to_lowercase())
                })?;
                out.from_summary = true;
                row.push(synthetic(s));
            } else {
                let mut v = Vec::new();
                for c in &ds.subcolumns {
                    let x = c.usable_at(r);
                    if x.is_finite() {
                        v.push(x);
                    } else if c.excluded.contains(&r) && !c.values[r].is_nan() {
                        excluded += 1;
                    }
                }
                row.push(v);
            }
        }
        if row.iter().any(|c| !c.is_empty()) {
            out.cells.push(row);
            out.row_names.push(row_name(table, r));
            out.rows.push(r);
        }
    }
    if excluded > 0 {
        out.notes.push(format!(
            "{} not used.",
            plural(excluded, "excluded value was", "excluded values were")
        ));
    }
    if out.from_summary {
        out.notes.push(
            "Some cells were entered as mean, SD and N; the ANOVA is exact from these, but residual checks are not possible for them."
                .into(),
        );
    }
    Ok(out)
}

/// One survival group.
#[derive(Debug, Clone, Default)]
pub struct SurvGroup {
    /// Name.
    pub name: String,
    /// Data set.
    pub dataset: usize,
    /// Times.
    pub time: Vec<f64>,
    /// `true` = event.
    pub event: Vec<bool>,
}

/// Survival data: X = time, each data set codes 1 = event, 0 = censored. Validates every
/// used cell and names the first bad one (DESIGN P7).
pub fn survival(table: &Table, keys: &[usize]) -> Result<(Vec<SurvGroup>, Vec<String>), String> {
    let x = table
        .x
        .as_ref()
        .ok_or("a survival table needs a time (X) column")?;
    let mut out = Vec::new();
    let mut notes = Vec::new();
    for &d in keys {
        let ds = &table.datasets[d];
        let c = &ds.subcolumns[0];
        let mut g = SurvGroup {
            name: ds.name.clone(),
            dataset: d,
            ..Default::default()
        };
        let mut missing_time = 0;
        for r in 0..table.rows {
            if c.excluded.contains(&r) {
                continue;
            }
            if let Some(t) = c.text.get(&r) {
                return Err(format!(
                    "Cell {} of {} holds \"{t}\"; survival codes must be 1 (event) or 0 (censored).",
                    table.address(r, table.flat_index(d, 0).unwrap_or(0)),
                    ds.name
                ));
            }
            let v = c.values.get(r).copied().unwrap_or(f64::NAN);
            if v.is_nan() {
                continue;
            }
            if v != 0.0 && v != 1.0 {
                return Err(format!(
                    "Cell {} of {} holds {v}; survival codes must be 1 (event) or 0 (censored).",
                    table.address(r, table.flat_index(d, 0).unwrap_or(0)),
                    ds.name
                ));
            }
            let t = x.usable_at(r);
            if !t.is_finite() {
                if let Some(s) = x.text.get(&r) {
                    return Err(format!(
                        "Time in row {} is \"{s}\"; times must be numbers.",
                        r + 1
                    ));
                }
                missing_time += 1;
                continue;
            }
            if t < 0.0 {
                return Err(format!(
                    "Time in row {} is {t}; survival times cannot be negative.",
                    r + 1
                ));
            }
            g.time.push(t);
            g.event.push(v == 1.0);
        }
        if missing_time > 0 {
            notes.push(format!(
                "{}: {} without a time (or with an excluded time) not used.",
                ds.name,
                plural(missing_time, "subject", "subjects")
            ));
        }
        out.push(g);
    }
    Ok((out, notes))
}

/// A table of counts.
#[derive(Debug, Clone, Default)]
pub struct Counts {
    /// `counts[row][column]`.
    pub counts: Vec<Vec<u64>>,
    /// Row names.
    pub row_names: Vec<String>,
    /// Column (data set) names.
    pub col_names: Vec<String>,
}

/// Counts of a contingency (or parts-of-whole) table. Every used cell must be a
/// non-negative whole number; an empty row or column at the end is ignored, any other
/// empty cell is an error naming it (DESIGN P7: the table is never rewritten).
pub fn counts(table: &Table) -> Result<Counts, String> {
    let ncols = table.datasets.len();
    let used_rows: Vec<usize> = (0..table.rows)
        .filter(|&r| {
            table
                .row_titles
                .get(r)
                .is_some_and(|t| !t.trim().is_empty())
                || table.datasets.iter().any(|d| {
                    let c = &d.subcolumns[0];
                    !c.values[r].is_nan() || c.text.contains_key(&r)
                })
        })
        .collect();
    let used_cols: Vec<usize> = (0..ncols)
        .filter(|&d| {
            let c = &table.datasets[d].subcolumns[0];
            used_rows
                .iter()
                .any(|&r| !c.values[r].is_nan() || c.text.contains_key(&r))
        })
        .collect();
    if used_rows.is_empty() || used_cols.is_empty() {
        return Err("The table has no counts.".into());
    }
    let mut out = Counts {
        row_names: used_rows.iter().map(|&r| row_name(table, r)).collect(),
        col_names: used_cols
            .iter()
            .map(|&d| table.datasets[d].name.clone())
            .collect(),
        ..Default::default()
    };
    for &r in &used_rows {
        let mut row = Vec::new();
        for &d in &used_cols {
            let c = &table.datasets[d].subcolumns[0];
            let addr = table.address(r, table.flat_index(d, 0).unwrap_or(0));
            let v = c.values[r];
            if c.excluded.contains(&r) {
                return Err(format!(
                    "Cell {addr} ({}, {}) is excluded; a count cannot be left out of a contingency table. Include it again or remove the row.",
                    row_name(table, r),
                    table.datasets[d].name
                ));
            }
            if let Some(t) = c.text.get(&r) {
                return Err(format!(
                    "Cell {addr} ({}, {}) holds \"{t}\"; counts must be whole numbers of subjects.",
                    row_name(table, r),
                    table.datasets[d].name
                ));
            }
            if v.is_nan() {
                return Err(format!(
                    "Cell {addr} ({}, {}) is empty; enter 0 if no subjects fell in it.",
                    row_name(table, r),
                    table.datasets[d].name
                ));
            }
            if v < 0.0 || v.fract() != 0.0 || v > 9.0e15 {
                return Err(format!(
                    "Cell {addr} ({}, {}) holds {}; counts must be non-negative whole numbers of subjects, not percentages or means.",
                    row_name(table, r),
                    table.datasets[d].name,
                    crate::util::format_sig(v, None, 6)
                ));
            }
            row.push(v as u64);
        }
        out.counts.push(row);
    }
    Ok(out)
}

/// One XY data set.
#[derive(Debug, Clone, Default)]
pub struct XyData {
    /// Name.
    pub name: String,
    /// Data set index.
    pub dataset: usize,
    /// X of every point (replicates repeat their X).
    pub x: Vec<f64>,
    /// Y of every point.
    pub y: Vec<f64>,
    /// Table row of every point.
    pub rows: Vec<usize>,
    /// Per-X means: (x, mean, SD, n) for rows with at least one value.
    pub means: Vec<(f64, f64, f64, usize)>,
    /// Whether Y came from summary values (points are the means).
    pub from_summary: bool,
    /// Notes.
    pub notes: Vec<String>,
}

/// XY data sets: every replicate is a point; summary formats give one point per row (the
/// mean) with its SD.
pub fn xy(table: &Table, datasets: &[usize]) -> Result<Vec<XyData>, String> {
    let x = table.x.as_ref().ok_or("this table has no X column")?;
    let picks: Vec<usize> = if datasets.is_empty() {
        (0..table.datasets.len()).collect()
    } else {
        datasets.to_vec()
    };
    let mut out = Vec::new();
    for d in picks {
        let ds = table
            .datasets
            .get(d)
            .ok_or_else(|| format!("Data set {} does not exist.", letters(d)))?;
        let mut o = XyData {
            name: ds.name.clone(),
            dataset: d,
            from_summary: is_summary(ds.format),
            ..Default::default()
        };
        let mut no_x = 0;
        for r in 0..table.rows {
            let row: Vec<f64> = ds.subcolumns.iter().map(|c| c.usable_at(r)).collect();
            let xv = x.usable_at(r);
            if o.from_summary {
                if row.iter().all(|v| !v.is_finite()) {
                    continue;
                }
                let s = summ_from(ds.format, &row)
                    .map_err(|e| format!("{} row {}: {e}", ds.name, r + 1))?;
                if !xv.is_finite() {
                    no_x += 1;
                    continue;
                }
                o.x.push(xv);
                o.y.push(s.mean);
                o.rows.push(r);
                o.means.push((xv, s.mean, s.sd, s.n));
                continue;
            }
            let vals: Vec<f64> = row.into_iter().filter(|v| v.is_finite()).collect();
            if vals.is_empty() {
                continue;
            }
            if !xv.is_finite() {
                no_x += vals.len();
                continue;
            }
            for v in &vals {
                o.x.push(xv);
                o.y.push(*v);
                o.rows.push(r);
            }
            let m = vals.iter().sum::<f64>() / vals.len() as f64;
            let sd = if vals.len() > 1 {
                (vals.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (vals.len() as f64 - 1.0))
                    .sqrt()
            } else {
                f64::NAN
            };
            o.means.push((xv, m, sd, vals.len()));
        }
        if no_x > 0 {
            o.notes.push(format!(
                "{}: {} without an X value not used.",
                ds.name,
                plural(no_x, "value", "values")
            ));
        }
        let excluded: usize = ds.subcolumns.iter().map(|c| c.excluded.len()).sum();
        if excluded > 0 {
            o.notes.push(format!(
                "{}: {} not used.",
                ds.name,
                plural(excluded, "excluded value was", "excluded values were")
            ));
        }
        if o.from_summary {
            o.notes.push(format!(
                "{}: entered as summary values; each X contributes its mean.",
                ds.name
            ));
        }
        out.push(o);
    }
    Ok(out)
}

/// Columns aligned by row for multivariable analyses (correlation matrix, PCA): the
/// chosen data sets (or X), NaN where missing.
pub fn aligned(table: &Table, keys: &[Option<usize>]) -> Result<Vec<(String, Vec<f64>)>, String> {
    keys.iter()
        .map(|&k| {
            if let Some(d) = k {
                if is_summary(table.datasets[d].format) {
                    return Err(format!(
                        "{} was entered as summary data; this analysis needs the individual values (not possible from summary data).",
                        table.datasets[d].name
                    ));
                }
            }
            Ok((key_name(table, k), column_by_row(table, k)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_keeps_mean_sd_n() {
        let v = synthetic(Summ {
            mean: 10.0,
            sd: 2.5,
            n: 7,
        });
        assert_eq!(v.len(), 7);
        let m = v.iter().sum::<f64>() / 7.0;
        let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / 6.0).sqrt();
        assert!((m - 10.0).abs() < 1e-12 && (sd - 2.5).abs() < 1e-12);
    }

    #[test]
    fn counts_name_the_bad_cell() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Contingency,
            "T".into(),
            2,
            SubcolumnFormat::Single,
            0,
        );
        for (r, row) in [["3", "1"], ["1.23", "3"]].iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                t.set_cell_raw(r, c, v).unwrap();
            }
        }
        let e = counts(&t).unwrap_err();
        assert!(e.contains("A2") && e.contains("1.23"), "{e}");
        t.set_cell_raw(1, 0, "1").unwrap();
        assert_eq!(counts(&t).unwrap().counts, vec![vec![3, 1], vec![1, 3]]);
    }
}
