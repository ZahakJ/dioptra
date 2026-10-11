//! Graph and layout specifications (DESIGN §4.4, §6) and the renderer seam.
//!
//! The plot engine (`dioptra-plot`) turns a [`GraphSpec`] plus extracted [`GraphData`]
//! into a scene and renders SVG, PDF or PNG. Until it is wired in, the session holds a
//! [`FallbackRenderer`] that draws plain axes and points as SVG so the whole pipeline can
//! be exercised.

use serde::{Deserialize, Serialize};

use crate::api::{GraphFormat, Id, SizeMm, TableKind};
use crate::table::{ColLoc, Table};

/// Axis scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    /// Linear.
    #[default]
    Linear,
    /// Base-10 logarithmic.
    Log10,
    /// Base-2 logarithmic.
    Log2,
}

/// Tick direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickDirection {
    /// Outside the plot area (default).
    #[default]
    Out,
    /// Inside.
    In,
    /// Crossing the axis.
    Cross,
}

/// Tick settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ticks {
    /// Major tick interval (`None`: automatic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major: Option<f64>,
    /// Minor ticks between majors.
    pub minor: u32,
    /// Direction.
    pub direction: TickDirection,
    /// Tick length in points.
    pub length_pt: f64,
}

impl Default for Ticks {
    fn default() -> Self {
        Ticks {
            major: None,
            minor: 0,
            direction: TickDirection::Out,
            length_pt: 3.0,
        }
    }
}

/// A gap in an axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxisBreak {
    /// Start of the gap.
    pub from: f64,
    /// End of the gap.
    pub to: f64,
}

/// One axis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Axis {
    /// Title (`^`/`_` for super/subscripts, `\alpha` for Greek).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Scale.
    pub scale: Scale,
    /// Minimum (`None`: automatic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Maximum (`None`: automatic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Breaks.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub breaks: Vec<AxisBreak>,
    /// Ticks.
    pub ticks: Ticks,
}

/// Both axes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Axes {
    /// Horizontal.
    pub x: Axis,
    /// Vertical.
    pub y: Axis,
}

/// Legend position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegendPosition {
    /// No legend.
    Hidden,
    /// Right of the plot (default).
    #[default]
    Right,
    /// Above.
    Top,
    /// Below.
    Bottom,
    /// Inside, top left.
    InsideTopLeft,
    /// Inside, top right.
    InsideTopRight,
    /// Inside, bottom left.
    InsideBottomLeft,
    /// Inside, bottom right.
    InsideBottomRight,
}

/// Legend.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Legend {
    /// Position.
    pub position: LegendPosition,
}

/// Line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineStyle {
    /// Solid.
    Solid,
    /// Dashed.
    Dashed,
    /// Dotted.
    Dotted,
    /// No line.
    None,
}

/// Per-dataset overrides.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DatasetStyle {
    /// Dataset index.
    pub dataset: usize,
    /// Colour (`#rrggbb`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// Marker (`circle`, `square`, `triangle`, `diamond`, `none`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    /// Line style.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_style: Option<LineStyle>,
    /// Legend label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Visual style of a graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphStyle {
    /// Theme: `instrument`, `night` or `journal`.
    pub theme: String,
    /// Palette: `okabe_ito`, `tol_bright`, `tol_muted`, `viridis`, `cividis`, `tol_sunset`, `grey`.
    pub palette: String,
    /// Size in millimetres.
    pub size_mm: SizeMm,
    /// Base font size in points.
    pub font_pt: f64,
    /// Line width in points.
    pub line_pt: f64,
    /// Per-dataset overrides.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<DatasetStyle>,
}

impl Default for GraphStyle {
    fn default() -> Self {
        GraphStyle {
            theme: "instrument".into(),
            palette: "okabe_ito".into(),
            size_mm: SizeMm {
                width: 85.0,
                height: 65.0,
            },
            font_pt: 8.0,
            line_pt: 0.75,
            datasets: Vec::new(),
        }
    }
}

/// Error bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBars {
    /// None.
    None,
    /// Standard deviation.
    Sd,
    /// Standard error of the mean.
    #[default]
    Sem,
    /// 95% confidence interval.
    Ci95,
    /// Range.
    Range,
    /// Interquartile range.
    Iqr,
}

/// Which data the graph shows.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    /// Datasets to plot (empty: all).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<usize>,
    /// Flat X column override (variables tables).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<usize>,
    /// Flat Y columns override (variables tables).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub y: Vec<usize>,
    /// Error bars for summary families.
    pub error_bars: ErrorBars,
    /// Show individual points over summaries.
    pub points: bool,
}

/// An annotation drawn on the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Annotation {
    /// A bracket between two groups with a label (asterisks or a p value).
    SignificanceBracket {
        /// First group (dataset index).
        from: usize,
        /// Second group.
        to: usize,
        /// Label.
        text: String,
        /// Height in data units (`None`: automatic).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
    },
    /// Free text.
    TextLabel {
        /// Text.
        text: String,
        /// X in data units.
        x: f64,
        /// Y in data units.
        y: f64,
        /// Font size in points (`None`: theme size).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_pt: Option<f64>,
    },
    /// A straight line in data units.
    Line {
        /// Start X.
        x1: f64,
        /// Start Y.
        y1: f64,
        /// End X.
        x2: f64,
        /// End Y.
        y2: f64,
        /// Dashed.
        #[serde(default)]
        dashed: bool,
    },
}

/// A graph specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphSpec {
    /// Source table or analysis.
    pub source: Id,
    /// Family name (see `dioptra_plot::GraphFamily`), e.g. `scatter`, `column`, `box`.
    pub family: String,
    /// Graph title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Data mapping.
    #[serde(default)]
    pub mapping: Mapping,
    /// Style.
    #[serde(default)]
    pub style: GraphStyle,
    /// Axes.
    #[serde(default)]
    pub axes: Axes,
    /// Legend.
    #[serde(default)]
    pub legend: Legend,
    /// Annotations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

impl GraphSpec {
    /// Defaults for a family.
    pub fn new(source: Id, family: &str) -> Self {
        GraphSpec {
            source,
            family: family.to_string(),
            title: None,
            mapping: Mapping {
                points: true,
                ..Default::default()
            },
            style: GraphStyle::default(),
            axes: Axes::default(),
            legend: Legend::default(),
            annotations: Vec::new(),
        }
    }
}

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

/// One plotted series.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Series {
    /// Name (dataset name).
    pub name: String,
    /// Dataset index in the source table.
    pub dataset: usize,
    /// X values (category position for column-like data, starting at 1).
    pub x: Vec<f64>,
    /// Y values.
    pub y: Vec<f64>,
}

/// Data extracted from the source for the renderer.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GraphData {
    /// Series.
    pub series: Vec<Series>,
    /// Category names when X is categorical.
    pub categories: Vec<String>,
    /// Default X title.
    pub x_label: String,
    /// Default Y title.
    pub y_label: String,
}

/// Pull the plotted values out of a table (excluded and empty cells are left out).
pub fn extract(spec: &GraphSpec, table: &Table) -> GraphData {
    let mut d = GraphData::default();
    let wanted = |i: usize| spec.mapping.datasets.is_empty() || spec.mapping.datasets.contains(&i);
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
                for c in ds.subcolumns.iter().take(
                    if matches!(ds.format, crate::api::SubcolumnFormat::Values(_)) {
                        usize::MAX
                    } else {
                        1
                    },
                ) {
                    for r in 0..table.rows {
                        let (xv, yv) =
                            (x.map_or((r + 1) as f64, |c| c.usable_at(r)), c.usable_at(r));
                        if xv.is_finite() && yv.is_finite() {
                            s.x.push(xv);
                            s.y.push(yv);
                        }
                    }
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
                (0..table.col_count()).filter(|c| *c != xi).collect()
            } else {
                spec.mapping.y.clone()
            };
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
                for r in 0..table.rows {
                    let (a, b) = (xc.usable_at(r), yc.usable_at(r));
                    if a.is_finite() && b.is_finite() {
                        s.x.push(a);
                        s.y.push(b);
                    }
                }
                d.series.push(s);
            }
        }
        _ => {
            for (i, ds) in table
                .datasets
                .iter()
                .enumerate()
                .filter(|(i, _)| wanted(*i))
            {
                let pos = (d.categories.len() + 1) as f64;
                d.categories.push(ds.name.clone());
                let mut s = Series {
                    name: ds.name.clone(),
                    dataset: i,
                    ..Default::default()
                };
                for c in &ds.subcolumns {
                    for v in c.usable() {
                        s.x.push(pos);
                        s.y.push(v);
                    }
                }
                d.series.push(s);
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
}

/// Turns specs and data into drawings. `dioptra-plot` will implement this.
pub trait Renderer {
    /// Render to SVG for display.
    fn render(&self, spec: &GraphSpec, data: &GraphData) -> Result<Rendered, String>;

    /// Export to a format. The default supports SVG only.
    fn export(
        &self,
        spec: &GraphSpec,
        data: &GraphData,
        format: GraphFormat,
    ) -> Result<(Vec<u8>, &'static str, &'static str), String> {
        match format {
            GraphFormat::Svg => Ok((self.render(spec, data)?.svg.into_bytes(), "image/svg+xml", "svg")),
            GraphFormat::Pdf | GraphFormat::Png { .. } => {
                Err("PDF and PNG export need the plot engine, which is not wired into this build yet; export SVG for now.".into())
            }
        }
    }
}

/// Points per millimetre.
pub const PT_PER_MM: f64 = 72.0 / 25.4;

const OKABE_ITO: [&str; 8] = [
    "#000000", "#E69F00", "#56B4E9", "#009E73", "#F0E442", "#0072B2", "#D55E00", "#CC79A7",
];

/// Minimal placeholder renderer: a frame, two axes with min/max labels, and points.
#[derive(Debug, Default, Clone, Copy)]
pub struct FallbackRenderer;

/// Escape text for SVG.
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt_pt(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl Renderer for FallbackRenderer {
    fn render(&self, spec: &GraphSpec, data: &GraphData) -> Result<Rendered, String> {
        let (wmm, hmm) = (spec.style.size_mm.width, spec.style.size_mm.height);
        if !(wmm > 0.0 && hmm > 0.0) {
            return Err("graph size must be positive".into());
        }
        let (w, h) = (wmm * PT_PER_MM, hmm * PT_PER_MM);
        let font = spec.style.font_pt;
        let (l, r, t, b) = (font * 4.5, font, font * 1.5, font * 4.0);
        let (pw, ph) = ((w - l - r).max(1.0), (h - t - b).max(1.0));
        let tr = |v: f64, s: Scale| match s {
            Scale::Linear => v,
            Scale::Log10 => v.log10(),
            Scale::Log2 => v.log2(),
        };
        let pts: Vec<(f64, f64)> = data
            .series
            .iter()
            .flat_map(|s| {
                s.x.iter()
                    .zip(&s.y)
                    .map(|(x, y)| (tr(*x, spec.axes.x.scale), tr(*y, spec.axes.y.scale)))
            })
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .collect();
        let range =
            |vals: &mut dyn Iterator<Item = f64>, min: Option<f64>, max: Option<f64>, s: Scale| {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for v in vals {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
                if !lo.is_finite() {
                    (lo, hi) = (0.0, 1.0);
                }
                if let Some(m) = min {
                    lo = tr(m, s);
                }
                if let Some(m) = max {
                    hi = tr(m, s);
                }
                if hi <= lo {
                    hi = lo + 1.0;
                }
                let pad = if min.is_none() && max.is_none() {
                    (hi - lo) * 0.05
                } else {
                    0.0
                };
                (lo - pad, hi + pad)
            };
        let (x0, x1) = range(
            &mut pts.iter().map(|p| p.0),
            spec.axes.x.min,
            spec.axes.x.max,
            spec.axes.x.scale,
        );
        let (y0, y1) = range(
            &mut pts.iter().map(|p| p.1),
            spec.axes.y.min,
            spec.axes.y.max,
            spec.axes.y.scale,
        );
        let sx = |v: f64| l + (v - x0) / (x1 - x0) * pw;
        let sy = |v: f64| t + ph - (v - y0) / (y1 - y0) * ph;
        let lw = spec.style.line_pt;
        let mut s = String::with_capacity(4096);
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\" font-family=\"Inter, sans-serif\" font-size=\"{}\">",
            fmt_pt(wmm), fmt_pt(hmm), fmt_pt(w), fmt_pt(h), fmt_pt(font)
        ));
        s.push_str(&format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>",
            fmt_pt(w),
            fmt_pt(h)
        ));
        s.push_str(&format!(
            "<g id=\"axis-x\" stroke=\"#000\" stroke-width=\"{lw}\"><line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/></g>",
            fmt_pt(l), fmt_pt(t + ph), fmt_pt(l + pw), fmt_pt(t + ph)
        ));
        s.push_str(&format!(
            "<g id=\"axis-y\" stroke=\"#000\" stroke-width=\"{lw}\"><line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/></g>",
            fmt_pt(l), fmt_pt(t), fmt_pt(l), fmt_pt(t + ph)
        ));
        let lab = |v: f64, sc: Scale| {
            crate::util::format_sig(
                match sc {
                    Scale::Linear => v,
                    Scale::Log10 => 10f64.powf(v),
                    Scale::Log2 => 2f64.powf(v),
                },
                None,
                3,
            )
        };
        s.push_str(&format!(
            "<g id=\"ticklabels\" fill=\"#000\"><text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text><text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text><text x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text><text x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text></g>",
            fmt_pt(l), fmt_pt(t + ph + font * 1.3), lab(x0, spec.axes.x.scale),
            fmt_pt(l + pw), fmt_pt(t + ph + font * 1.3), lab(x1, spec.axes.x.scale),
            fmt_pt(l - font * 0.4), fmt_pt(t + ph), lab(y0, spec.axes.y.scale),
            fmt_pt(l - font * 0.4), fmt_pt(t + font * 0.7), lab(y1, spec.axes.y.scale),
        ));
        let xt = spec
            .axes
            .x
            .title
            .clone()
            .unwrap_or_else(|| data.x_label.clone());
        let yt = spec
            .axes
            .y
            .title
            .clone()
            .unwrap_or_else(|| data.y_label.clone());
        s.push_str(&format!(
            "<text id=\"axis-x-title\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            fmt_pt(l + pw / 2.0),
            fmt_pt(h - font * 0.6),
            xml_escape(&xt)
        ));
        s.push_str(&format!(
            "<text id=\"axis-y-title\" transform=\"translate({} {}) rotate(-90)\" text-anchor=\"middle\">{}</text>",
            fmt_pt(font * 1.2), fmt_pt(t + ph / 2.0), xml_escape(&yt)
        ));
        for (i, se) in data.series.iter().enumerate() {
            let colour = spec
                .style
                .datasets
                .iter()
                .find(|d| d.dataset == se.dataset)
                .and_then(|d| d.colour.clone())
                .unwrap_or_else(|| OKABE_ITO[i % OKABE_ITO.len()].to_string());
            s.push_str(&format!(
                "<g id=\"series-{}\" fill=\"{}\">",
                se.dataset,
                xml_escape(&colour)
            ));
            for (x, y) in se.x.iter().zip(&se.y) {
                let (tx, ty) = (tr(*x, spec.axes.x.scale), tr(*y, spec.axes.y.scale));
                if tx.is_finite() && ty.is_finite() {
                    s.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                        fmt_pt(sx(tx)),
                        fmt_pt(sy(ty)),
                        fmt_pt(font * 0.25)
                    ));
                }
            }
            s.push_str("</g>");
        }
        if let Some(title) = &spec.title {
            s.push_str(&format!("<text id=\"title\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-weight=\"600\">{}</text>", fmt_pt(w / 2.0), fmt_pt(font * 1.1), xml_escape(title)));
        }
        s.push_str("</svg>");
        Ok(Rendered {
            svg: s,
            width_mm: wmm,
            height_mm: hmm,
        })
    }
}

/// Compose rendered graphs into a grid page with panel letters.
pub fn compose_layout(panels: &[Rendered], columns: usize) -> Rendered {
    let columns = columns.max(1);
    let cell_w = panels
        .iter()
        .map(|p| p.width_mm)
        .fold(0.0, f64::max)
        .max(10.0);
    let cell_h = panels
        .iter()
        .map(|p| p.height_mm)
        .fold(0.0, f64::max)
        .max(10.0);
    let rows = panels.len().div_ceil(columns).max(1);
    let (wmm, hmm) = (
        cell_w * columns.min(panels.len().max(1)) as f64,
        cell_h * rows as f64,
    );
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">",
        fmt_pt(wmm), fmt_pt(hmm), fmt_pt(wmm * PT_PER_MM), fmt_pt(hmm * PT_PER_MM)
    );
    for (i, p) in panels.iter().enumerate() {
        let (cx, cy) = (
            (i % columns) as f64 * cell_w * PT_PER_MM,
            (i / columns) as f64 * cell_h * PT_PER_MM,
        );
        // Re-anchor the panel's own <svg> at its cell.
        let inner = p.svg.replacen(
            "<svg ",
            &format!("<svg x=\"{}\" y=\"{}\" ", fmt_pt(cx), fmt_pt(cy)),
            1,
        );
        let inner = inner.replacen(
            &format!(
                "width=\"{}mm\" height=\"{}mm\"",
                fmt_pt(p.width_mm),
                fmt_pt(p.height_mm)
            ),
            &format!(
                "width=\"{}\" height=\"{}\"",
                fmt_pt(p.width_mm * PT_PER_MM),
                fmt_pt(p.height_mm * PT_PER_MM)
            ),
            1,
        );
        s.push_str(&format!("<g id=\"panel-{i}\">{inner}<text x=\"{}\" y=\"{}\" font-family=\"Inter, sans-serif\" font-size=\"10\" font-weight=\"700\">{}</text></g>", fmt_pt(cx + 2.0), fmt_pt(cy + 10.0), crate::util::letters(i)));
    }
    s.push_str("</svg>");
    Rendered {
        svg: s,
        width_mm: wmm,
        height_mm: hmm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SubcolumnFormat;

    #[test]
    fn fallback_draws_points() {
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
        let spec = GraphSpec::new("t".into(), "scatter");
        let data = extract(&spec, &t);
        assert_eq!(data.series[0].y, vec![2.0, 4.0, 6.0]);
        let r = FallbackRenderer.render(&spec, &data).unwrap();
        assert_eq!(r.svg.matches("<circle").count(), 3);
        assert!(r.svg.contains("id=\"series-0\""));
        let page = compose_layout(&[r.clone(), r], 2);
        assert_eq!(page.svg.matches("<circle").count(), 6);
        assert_eq!(page.width_mm, 170.0);
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
