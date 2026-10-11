//! Graphs and layouts in the project (DESIGN §4.4), data extraction for the plot engine,
//! and the renderer seam.
//!
//! The specification types live in `dioptra_plot::spec` and are re-exported here, so the
//! JSON the UI sends and receives is the plot engine's own. [`extract`] turns a table into
//! [`GraphData`]; a [`Renderer`] turns spec + data into SVG, PDF or PNG. The session uses
//! [`DioptraRenderer`] (the real engine) unless another renderer is installed.

use serde::{Deserialize, Serialize};

pub use dioptra_plot::data::{AtRiskTable, Curve, GraphData, Series};
pub use dioptra_plot::spec::*;
pub use dioptra_plot::Target;

use crate::api::{GraphFormat, SubcolumnFormat, TableKind};
use crate::table::{ColLoc, Column, Table};

/// A graph object in a project. Equality ignores the stale flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    /// Id.
    pub id: Id,
    /// Title.
    pub title: String,
    /// Specification.
    pub spec: GraphSpec,
    /// Whether the source changed since the last render (runtime).
    #[serde(skip)]
    pub stale: bool,
}

impl PartialEq for Graph {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id && self.title == o.title && self.spec == o.spec
    }
}

/// A page of graphs in a grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Id.
    pub id: Id,
    /// Title.
    pub title: String,
    /// Graphs in reading order.
    pub graphs: Vec<Id>,
    /// Columns in the grid.
    pub columns: usize,
}

/// Error bounds for a summary row (mean/SD/N and friends), converted to what the spec
/// asks for when the stored numbers allow it. Returns (centre, low, high).
fn summary_row(
    format: SubcolumnFormat,
    v: &[f64],
    eb: ErrorBars,
) -> Option<(f64, Option<(f64, f64)>)> {
    let g = |i: usize| v.get(i).copied().unwrap_or(f64::NAN);
    let t = |n: f64| dioptra_plot::stats::t_quantile(0.975, n - 1.0);
    let (c, half): (f64, Option<f64>) = match format {
        SubcolumnFormat::MeanSdN | SubcolumnFormat::MeanSd => {
            let (m, sd, n) = (g(0), g(1), g(2));
            let h = match eb {
                ErrorBars::Sem if n > 1.0 => sd / n.sqrt(),
                ErrorBars::Ci95 if n > 1.0 => t(n) * sd / n.sqrt(),
                _ => sd,
            };
            (m, Some(h))
        }
        SubcolumnFormat::MeanSemN | SubcolumnFormat::MeanSem => {
            let (m, sem, n) = (g(0), g(1), g(2));
            let h = match eb {
                ErrorBars::Sd if n > 0.0 => sem * n.sqrt(),
                ErrorBars::Ci95 if n > 1.0 => t(n) * sem,
                _ => sem,
            };
            (m, Some(h))
        }
        SubcolumnFormat::MeanCiN => {
            let (m, ci, n) = (g(0), g(1), g(2));
            let h = match eb {
                ErrorBars::Sem if n > 1.0 => ci / t(n),
                ErrorBars::Sd if n > 1.0 => ci / t(n) * n.sqrt(),
                _ => ci,
            };
            (m, Some(h))
        }
        SubcolumnFormat::MedianIqr => {
            let (m, q1, q3) = (g(0), g(1), g(2));
            if !m.is_finite() {
                return None;
            }
            let e = (eb != ErrorBars::None && q1.is_finite() && q3.is_finite()).then_some((q1, q3));
            return Some((m, e));
        }
        SubcolumnFormat::LowerUpper => {
            let (lo, hi) = (g(0), g(1));
            if !(lo.is_finite() && hi.is_finite()) {
                return None;
            }
            let e = (eb != ErrorBars::None).then_some((lo, hi));
            return Some(((lo + hi) / 2.0, e));
        }
        SubcolumnFormat::Values(_) | SubcolumnFormat::Single => (g(0), None),
    };
    if !c.is_finite() {
        return None;
    }
    let e = match (eb, half) {
        (ErrorBars::None, _) => None,
        (_, Some(h)) if h.is_finite() => Some((c - h, c + h)),
        _ => None,
    };
    Some((c, e))
}

fn is_summary(f: SubcolumnFormat) -> bool {
    !matches!(f, SubcolumnFormat::Values(_) | SubcolumnFormat::Single)
}

fn row_label(table: &Table, r: usize) -> String {
    table
        .row_titles
        .get(r)
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("{}", r + 1))
}

fn text_or_value(c: &Column, r: usize) -> String {
    if let Some(t) = c.text.get(&r) {
        return t.clone();
    }
    let v = c.values.get(r).copied().unwrap_or(f64::NAN);
    if v.is_finite() {
        crate::util::format_sig(v, None, 6)
    } else {
        String::new()
    }
}

/// Pull the plotted values out of a table. Excluded and empty cells become NaN (kept in
/// place inside `replicates` so rows stay aligned) or are left out.
pub fn extract(spec: &GraphSpec, table: &Table) -> GraphData {
    let mut d = GraphData {
        kind: table.kind,
        ..Default::default()
    };
    let wanted = |i: usize| spec.mapping.datasets.is_empty() || spec.mapping.datasets.contains(&i);
    // Forest plots show confidence intervals whatever the error-bar setting says.
    let eb = if spec.family == "forest" {
        ErrorBars::Ci95
    } else {
        spec.mapping.error_bars
    };
    let rows = table.rows;
    match table.kind {
        TableKind::Xy | TableKind::Survival => {
            let x = table.x.as_ref();
            d.x_label = x.map(|c| c.title.clone()).unwrap_or_default();
            for (i, ds) in table
                .datasets
                .iter()
                .enumerate()
                .filter(|(i, _)| wanted(*i))
            {
                let mut s = Series {
                    name: ds.name.clone(),
                    dataset: i,
                    ..Default::default()
                };
                let reps = matches!(ds.format, SubcolumnFormat::Values(n) if n > 1)
                    && table.kind == TableKind::Xy;
                for r in 0..rows {
                    let xv = x.map_or((r + 1) as f64, |c| c.usable_at(r));
                    if !xv.is_finite() {
                        continue;
                    }
                    let vals: Vec<f64> = ds.subcolumns.iter().map(|c| c.usable_at(r)).collect();
                    if is_summary(ds.format) {
                        if let Some((c, e)) = summary_row(ds.format, &vals, eb) {
                            s.x.push(xv);
                            s.y.push(c);
                            s.error.push(e.unwrap_or((f64::NAN, f64::NAN)));
                        }
                    } else if reps {
                        if vals.iter().any(|v| v.is_finite()) {
                            s.x.push(xv);
                            s.y.push(dioptra_plot::stats::mean(&vals));
                            s.replicates.push(vals);
                        }
                    } else if vals[0].is_finite() {
                        s.x.push(xv);
                        s.y.push(vals[0]);
                        if !table.row_titles.is_empty() {
                            s.labels.push(row_label(table, r));
                        }
                    }
                }
                if s.error.iter().all(|e| !e.0.is_finite()) {
                    s.error.clear();
                }
                d.series.push(s);
            }
            if table.datasets.len() == 1 {
                d.y_label = table.datasets[0].name.clone();
            }
        }
        TableKind::Variables if spec.mapping.x.is_some() => {
            let xi = spec.mapping.x.expect("x");
            let xc = table.col(xi);
            d.x_label = table.col_header(xi);
            let ys: Vec<usize> = if spec.mapping.y.is_empty() {
                (0..table.col_count())
                    .filter(|c| *c != xi && Some(*c) != spec.mapping.label)
                    .collect()
            } else {
                spec.mapping.y.clone()
            };
            if ys.len() == 1 {
                d.y_label = table.col_header(ys[0]);
            }
            let lc = spec.mapping.label.and_then(|l| table.col(l));
            for yi in ys {
                let (Some(xc), Some(yc)) = (xc, table.col(yi)) else {
                    continue;
                };
                let ds = match table.locate(yi) {
                    Some(ColLoc::Y { dataset, .. }) => dataset,
                    _ => 0,
                };
                let mut s = Series {
                    name: table.col_header(yi),
                    dataset: ds,
                    ..Default::default()
                };
                for r in 0..rows {
                    let (a, b) = (xc.usable_at(r), yc.usable_at(r));
                    if a.is_finite() && b.is_finite() {
                        s.x.push(a);
                        s.y.push(b);
                        if let Some(lc) = lc {
                            s.labels.push(text_or_value(lc, r));
                        } else if !table.row_titles.is_empty() {
                            s.labels.push(row_label(table, r));
                        }
                    }
                }
                d.series.push(s);
            }
        }
        TableKind::Variables
            if matches!(spec.family.as_str(), "correlation_matrix" | "heatmap") =>
        {
            // One series per numeric variable, aligned by row.
            for (i, ds) in table
                .datasets
                .iter()
                .enumerate()
                .filter(|(i, _)| wanted(*i))
            {
                if !matches!(ds.var_type, None | Some(crate::api::VariableType::Number)) {
                    continue;
                }
                let c = &ds.subcolumns[0];
                let mut s = Series {
                    name: ds.name.clone(),
                    dataset: i,
                    ..Default::default()
                };
                for r in 0..rows {
                    let v = c.usable_at(r);
                    if v.is_finite() {
                        s.x.push((r + 1) as f64);
                        s.y.push(v);
                    }
                }
                d.series.push(s);
            }
            d.categories = (0..rows).map(|r| row_label(table, r)).collect();
        }
        TableKind::Grouped | TableKind::Contingency | TableKind::Parts => {
            d.categories = (0..rows).map(|r| row_label(table, r)).collect();
            for (i, ds) in table
                .datasets
                .iter()
                .enumerate()
                .filter(|(i, _)| wanted(*i))
            {
                let mut s = Series {
                    name: ds.name.clone(),
                    dataset: i,
                    ..Default::default()
                };
                let reps = matches!(ds.format, SubcolumnFormat::Values(n) if n > 1);
                for r in 0..rows {
                    let vals: Vec<f64> = ds.subcolumns.iter().map(|c| c.usable_at(r)).collect();
                    let pos = (r + 1) as f64;
                    if is_summary(ds.format) {
                        if let Some((c, e)) = summary_row(ds.format, &vals, eb) {
                            s.x.push(pos);
                            s.y.push(c);
                            s.error.push(e.unwrap_or((f64::NAN, f64::NAN)));
                        }
                    } else if reps {
                        if vals.iter().any(|v| v.is_finite()) {
                            s.x.push(pos);
                            s.y.push(dioptra_plot::stats::mean(&vals));
                            s.replicates.push(vals);
                        }
                    } else if vals[0].is_finite() {
                        s.x.push(pos);
                        s.y.push(vals[0]);
                    }
                }
                if s.error.iter().all(|e| !e.0.is_finite()) {
                    s.error.clear();
                }
                d.series.push(s);
            }
            if table.datasets.len() == 1 {
                d.y_label = table.datasets[0].name.clone();
            }
        }
        _ => {
            // Column, Nested and Variables without an X: one category per dataset.
            let nested = table.kind == TableKind::Nested;
            let mut max_sub = 0;
            for (i, ds) in table
                .datasets
                .iter()
                .enumerate()
                .filter(|(i, _)| wanted(*i))
            {
                let pos = (d.categories.len() + 1) as f64;
                d.categories.push(ds.name.clone());
                if is_summary(ds.format) {
                    let vals: Vec<f64> = ds.subcolumns.iter().map(|c| c.usable_at(0)).collect();
                    let mut s = Series {
                        name: ds.name.clone(),
                        dataset: i,
                        ..Default::default()
                    };
                    if let Some((c, e)) = summary_row(ds.format, &vals, eb) {
                        s.x.push(pos);
                        s.y.push(c);
                        if let Some(e) = e {
                            s.error.push(e);
                        }
                    }
                    d.series.push(s);
                    continue;
                }
                // Row-aligned values (NaN kept) so paired families can match rows.
                let mut vals = Vec::new();
                let mut groups = Vec::new();
                for (k, c) in ds.subcolumns.iter().enumerate() {
                    for r in 0..rows {
                        vals.push(c.usable_at(r));
                        groups.push(k);
                    }
                }
                if !nested {
                    // Trim trailing empty rows.
                    while vals.last().is_some_and(|v| v.is_nan()) {
                        vals.pop();
                        groups.pop();
                    }
                }
                max_sub = max_sub.max(ds.subcolumns.len());
                let mut s = Series::category(&ds.name, i, pos, vals);
                if nested {
                    s.groups = groups;
                }
                d.series.push(s);
            }
            if nested {
                d.group_names = (0..max_sub)
                    .map(|k| {
                        table
                            .datasets
                            .iter()
                            .find_map(|ds| {
                                ds.subcolumns
                                    .get(k)
                                    .map(|c| c.title.clone())
                                    .filter(|t| !t.is_empty())
                            })
                            .unwrap_or_else(|| format!("Subgroup {}", k + 1))
                    })
                    .collect();
            }
        }
    }
    d
}

/// A rendered graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// SVG document.
    pub svg: String,
    /// Width in millimetres.
    pub width_mm: f64,
    /// Height in millimetres.
    pub height_mm: f64,
    /// Clickable elements for the graph editor.
    pub targets: Vec<Target>,
}

/// An exported file: bytes, MIME type and extension.
pub type Exported = (Vec<u8>, &'static str, &'static str);

/// Turns specs and data into drawings.
pub trait Renderer: Send + Sync {
    /// Render to SVG for display.
    fn render(&self, spec: &GraphSpec, data: &GraphData) -> Result<Rendered, String>;

    /// Export to a format. The default supports SVG only.
    fn export(
        &self,
        spec: &GraphSpec,
        data: &GraphData,
        format: GraphFormat,
    ) -> Result<Exported, String> {
        match format {
            GraphFormat::Svg => Ok((
                self.render(spec, data)?.svg.into_bytes(),
                "image/svg+xml",
                "svg",
            )),
            _ => Err("this renderer exports SVG only".into()),
        }
    }

    /// Render a layout (a grid of graphs with panel letters).
    fn render_layout(
        &self,
        panels: &[(GraphSpec, GraphData)],
        columns: usize,
    ) -> Result<Rendered, String> {
        let _ = (panels, columns);
        Err("this renderer cannot draw layouts".into())
    }

    /// Export a layout. The default supports SVG only.
    fn export_layout(
        &self,
        panels: &[(GraphSpec, GraphData)],
        columns: usize,
        format: GraphFormat,
    ) -> Result<Exported, String> {
        match format {
            GraphFormat::Svg => Ok((
                self.render_layout(panels, columns)?.svg.into_bytes(),
                "image/svg+xml",
                "svg",
            )),
            _ => Err("this renderer exports SVG only".into()),
        }
    }
}

/// The plot engine (`dioptra-plot`): SVG, PDF with embedded fonts, PNG at any DPI.
#[derive(Debug, Default, Clone, Copy)]
pub struct DioptraRenderer;

fn from_scene(s: &dioptra_plot::Scene) -> Rendered {
    let r = dioptra_plot::rendered(s);
    Rendered {
        svg: r.svg,
        width_mm: r.width_mm,
        height_mm: r.height_mm,
        targets: r.targets,
    }
}

fn export_scene(s: &dioptra_plot::Scene, format: GraphFormat) -> Result<Exported, String> {
    match format {
        GraphFormat::Svg => Ok((dioptra_plot::to_svg(s).into_bytes(), "image/svg+xml", "svg")),
        GraphFormat::Pdf => Ok((
            dioptra_plot::to_pdf(s).map_err(|e| e.to_string())?,
            "application/pdf",
            "pdf",
        )),
        GraphFormat::Png { dpi } => Ok((
            dioptra_plot::to_png(s, dpi).map_err(|e| e.to_string())?,
            "image/png",
            "png",
        )),
    }
}

fn layout_scene(
    panels: &[(GraphSpec, GraphData)],
    columns: usize,
) -> Result<dioptra_plot::Scene, String> {
    let scenes = panels
        .iter()
        .map(|(s, d)| dioptra_plot::scene(s, d).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(dioptra_plot::compose_layout(&scenes, columns, true))
}

impl Renderer for DioptraRenderer {
    fn render(&self, spec: &GraphSpec, data: &GraphData) -> Result<Rendered, String> {
        Ok(from_scene(
            &dioptra_plot::scene(spec, data).map_err(|e| e.to_string())?,
        ))
    }

    fn export(
        &self,
        spec: &GraphSpec,
        data: &GraphData,
        format: GraphFormat,
    ) -> Result<Exported, String> {
        export_scene(
            &dioptra_plot::scene(spec, data).map_err(|e| e.to_string())?,
            format,
        )
    }

    fn render_layout(
        &self,
        panels: &[(GraphSpec, GraphData)],
        columns: usize,
    ) -> Result<Rendered, String> {
        Ok(from_scene(&layout_scene(panels, columns)?))
    }

    fn export_layout(
        &self,
        panels: &[(GraphSpec, GraphData)],
        columns: usize,
        format: GraphFormat,
    ) -> Result<Exported, String> {
        export_scene(&layout_scene(panels, columns)?, format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xy() -> Table {
        let mut t = Table::new(
            "t".into(),
            TableKind::Xy,
            "T".into(),
            1,
            SubcolumnFormat::Values(1),
            0,
        );
        for r in 0..3 {
            t.set_cell_raw(r, 0, &format!("{}", r + 1)).unwrap();
            t.set_cell_raw(r, 1, &format!("{}", (r + 1) * 2)).unwrap();
        }
        t
    }

    #[test]
    fn renders_points_with_targets() {
        let t = xy();
        let spec = GraphSpec::new("t".into(), "scatter");
        let data = extract(&spec, &t);
        assert_eq!(data.series[0].y, vec![2.0, 4.0, 6.0]);
        let r = DioptraRenderer.render(&spec, &data).unwrap();
        assert_eq!(r.svg.matches("<circle").count(), 3);
        assert!(r.svg.contains("id=\"series-0\""));
        assert!(r
            .targets
            .iter()
            .any(|t| t.id == "axis-x" && t.target == "/axes/x"));
        let page = DioptraRenderer
            .render_layout(&[(spec.clone(), data.clone()), (spec, data)], 2)
            .unwrap();
        assert_eq!(page.svg.matches("<circle").count(), 6);
        assert!((page.width_mm - 170.0).abs() < 1e-6);
    }

    #[test]
    fn exports_pdf_and_png() {
        let t = xy();
        let spec = GraphSpec::new("t".into(), "scatter");
        let data = extract(&spec, &t);
        let (pdf, mime, ext) = DioptraRenderer
            .export(&spec, &data, GraphFormat::Pdf)
            .unwrap();
        assert!(pdf.starts_with(b"%PDF-1."));
        assert_eq!((mime, ext), ("application/pdf", "pdf"));
        let (png, _, _) = DioptraRenderer
            .export(&spec, &data, GraphFormat::Png { dpi: 300.0 })
            .unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1004);
    }

    #[test]
    fn summary_formats_become_error_bars() {
        let mut t = Table::new(
            "t".into(),
            TableKind::Grouped,
            "T".into(),
            1,
            SubcolumnFormat::MeanSdN,
            1,
        );
        t.set_cell_raw(0, 0, "10").unwrap();
        t.set_cell_raw(0, 1, "2").unwrap();
        t.set_cell_raw(0, 2, "4").unwrap();
        let mut spec = GraphSpec::new("t".into(), "column");
        spec.mapping.error_bars = ErrorBars::Sem;
        let d = extract(&spec, &t);
        assert_eq!(d.series[0].y, vec![10.0]);
        assert_eq!(d.series[0].error, vec![(9.0, 11.0)]);
        spec.mapping.error_bars = ErrorBars::Sd;
        assert_eq!(extract(&spec, &t).series[0].error, vec![(8.0, 12.0)]);
    }

    #[test]
    fn spec_round_trips_with_defaults() {
        let s: GraphSpec = serde_json::from_str(r#"{"source":"a","family":"column"}"#).unwrap();
        assert_eq!(s.style.theme, "instrument");
        let mut s2 = s.clone();
        s2.annotations.push(Annotation::SignificanceBracket {
            from: 0,
            to: 1,
            text: "**".into(),
            y: None,
        });
        let j = serde_json::to_string(&s2).unwrap();
        assert_eq!(serde_json::from_str::<GraphSpec>(&j).unwrap(), s2);
    }
}
