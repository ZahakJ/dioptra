//! The Cartesian plot model and its layout.
//!
//! A family turns the spec and data into a [`Plot`]: axis plans, layers of marks in data
//! coordinates (sizes in points), legend entries and decorations (colour bar, at-risk
//! table, dendrograms, a right-hand text column). [`render`] then measures every label
//! with the bundled font metrics, sizes the margins so nothing collides, maps the marks
//! to points, clips them geometrically to the plot area, and emits the scene.

use crate::data::GraphData;
use crate::detmath::DetMath;
use crate::scene::{
    Anchor, Baseline, Color, Dash, Group, MarkerShape, Node, Paint, Rect, Scene, Stroke, Text,
};
use crate::spec::{
    AxisOrigin, FrameStyle, GraphSpec, LegendPosition, LineStyle, Scale, TickDirection,
};
use crate::stats::Merge;
use crate::text::{self, Run, Weight};
use crate::theme::{categorical, Ramp, Theme, MONO_MARKERS};
use crate::ticks::{self, TickSet};

/// Outer padding of the figure (pt).
const OUTER: f64 = 4.0;
/// Gap between tick end and tick label (pt).
const LABEL_GAP: f64 = 2.5;
/// Gap between tick labels and axis title (pt).
const TITLE_GAP: f64 = 4.0;
/// Size of an axis-break gap (pt).
const BREAK_GAP: f64 = 5.0;
/// Above this many points a series is decimated to the 0.75 pt grid.
pub const LOD_THRESHOLD: usize = 20_000;

/// How one series looks.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesStyle {
    /// Colour.
    pub colour: Color,
    /// Marker shape (`None`: no marker).
    pub marker: Option<MarkerShape>,
    /// Open (unfilled) marker.
    pub open: bool,
    /// Marker diameter (pt).
    pub size: f64,
    /// Line dash.
    pub dash: Option<Dash>,
    /// Line width (pt).
    pub line_pt: f64,
    /// Legend label (markup).
    pub label: String,
    /// Dataset index.
    pub dataset: usize,
}

impl SeriesStyle {
    /// Paint for a marker of this series.
    pub fn marker_paint(&self, bg: Color) -> Paint {
        let w = (self.size * 0.18).clamp(0.5, 1.0);
        if self.open {
            Paint::both(bg, Stroke::solid(self.colour, w))
        } else {
            Paint::fill(self.colour)
        }
    }

    /// Line stroke of this series.
    pub fn stroke(&self) -> Stroke {
        Stroke::solid(self.colour, self.line_pt).dashed(self.dash.unwrap_or(Dash::Solid))
    }
}

/// Everything a family needs: the spec, the data and the resolved theme.
#[derive(Debug)]
pub struct Ctx<'a> {
    /// Spec.
    pub spec: &'a GraphSpec,
    /// Data.
    pub data: &'a GraphData,
    /// Theme with overrides.
    pub theme: Theme,
    /// Palette name in effect.
    pub palette: String,
}

impl<'a> Ctx<'a> {
    /// New context.
    pub fn new(spec: &'a GraphSpec, data: &'a GraphData) -> Self {
        let theme = Theme::for_spec(spec);
        let palette = spec
            .style
            .palette
            .clone()
            .unwrap_or_else(|| theme.palette.to_string());
        Ctx {
            spec,
            data,
            theme,
            palette,
        }
    }

    /// Categorical colours for `n` items.
    pub fn colours(&self, n: usize) -> Vec<Color> {
        categorical(&self.palette, n, self.theme.name == "night")
    }

    /// Style of the `i`-th of `n` plotted series for `dataset`.
    pub fn style(&self, i: usize, n: usize, dataset: usize, name: &str) -> SeriesStyle {
        let o = self.spec.dataset_style(dataset);
        let base = self.colours(n.max(1))[i % n.max(1)];
        let colour = o
            .and_then(|o| o.colour.as_deref())
            .and_then(Color::hex)
            .unwrap_or(base);
        let (mut marker, mut open) = (Some(MarkerShape::Circle), false);
        if self.theme.mono {
            let (m, op) = MONO_MARKERS[i % MONO_MARKERS.len()];
            marker = Some(m);
            open = op;
        }
        if let Some(m) = o.and_then(|o| o.marker.as_deref()) {
            match MarkerShape::parse(m) {
                Some((s, op)) => {
                    marker = Some(s);
                    open = op;
                }
                None => marker = None,
            }
        }
        let dash = match o.and_then(|o| o.line_style) {
            Some(LineStyle::Solid) => Some(Dash::Solid),
            Some(LineStyle::Dashed) => Some(Dash::Dashed),
            Some(LineStyle::Dotted) => Some(Dash::Dotted),
            Some(LineStyle::DashDot) => Some(Dash::DashDot),
            Some(LineStyle::None) => None,
            None => Some(if self.theme.mono {
                [Dash::Solid, Dash::Dashed, Dash::Dotted, Dash::DashDot][i % 4]
            } else {
                Dash::Solid
            }),
        };
        SeriesStyle {
            colour,
            marker,
            open,
            size: o.and_then(|o| o.marker_pt).unwrap_or(self.theme.marker_pt),
            dash,
            line_pt: o.and_then(|o| o.line_pt).unwrap_or(self.theme.data_pt),
            label: o
                .and_then(|o| o.label.clone())
                .unwrap_or_else(|| name.to_string()),
            dataset,
        }
    }

    /// A colour lightened toward the background for fills behind data.
    pub fn tint(&self, c: Color) -> Color {
        c.mix(self.theme.background, self.theme.tint)
    }

    /// Foreground (axis) colour.
    pub fn fg(&self) -> Color {
        self.theme.axis
    }
}

/// Axis plan from a family.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AxisPlan {
    /// Category labels (non-empty: categorical axis with positions 1..=n).
    pub categories: Vec<String>,
    /// Default title (markup).
    pub title: String,
    /// Values the range must include (e.g. 0 under bars).
    pub include: Vec<f64>,
    /// Fractional padding of an automatic range before rounding to ticks.
    pub pad: f64,
    /// Categorical axis runs top to bottom (y).
    pub reverse: bool,
    /// Use the data range exactly (no tick rounding), e.g. for densities touching 0.
    pub exact: bool,
    /// Draw no axis line and ticks (heatmaps).
    pub bare: bool,
    /// Force a scale (overrides the spec), e.g. linear for a percent axis.
    pub scale: Option<Scale>,
    /// Default major tick interval when the spec sets none (e.g. 25 on percent axes).
    pub major: Option<f64>,
}

impl AxisPlan {
    /// A continuous axis with a title.
    pub fn continuous(title: &str) -> Self {
        AxisPlan {
            title: title.into(),
            pad: 0.04,
            ..Default::default()
        }
    }

    /// A categorical axis.
    pub fn categorical(cats: Vec<String>, title: &str) -> Self {
        AxisPlan {
            categories: cats,
            title: title.into(),
            ..Default::default()
        }
    }
}

/// A secondary y axis on the right whose value `v` sits at primary `v + offset`.
#[derive(Debug, Clone, PartialEq)]
pub struct Secondary {
    /// Offset added to secondary values to place them on the primary scale.
    pub offset: f64,
    /// Title.
    pub title: String,
    /// Data range of secondary values.
    pub range: (f64, f64),
}

/// A mark in data coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    /// Markers.
    Points {
        /// Centres.
        pts: Vec<(f64, f64)>,
        /// Shape.
        shape: MarkerShape,
        /// Diameter (pt).
        size: f64,
        /// Paint.
        paint: Paint,
    },
    /// A polyline (`step`: horizontal-then-vertical steps).
    Line {
        /// Points.
        pts: Vec<(f64, f64)>,
        /// Stroke.
        stroke: Stroke,
        /// Steps.
        step: bool,
    },
    /// A band between two curves sharing x.
    Band {
        /// X.
        x: Vec<f64>,
        /// Lower.
        lo: Vec<f64>,
        /// Upper.
        hi: Vec<f64>,
        /// Fill.
        fill: Color,
        /// Steps.
        step: bool,
    },
    /// A closed polygon.
    Polygon {
        /// Points.
        pts: Vec<(f64, f64)>,
        /// Paint.
        paint: Paint,
    },
    /// A rectangle between data coordinates.
    Rect {
        /// Left.
        x0: f64,
        /// Right.
        x1: f64,
        /// Bottom.
        y0: f64,
        /// Top.
        y1: f64,
        /// Paint.
        paint: Paint,
    },
    /// A fill pattern over a rectangle (drawn as clipped strokes or dots).
    Hatch {
        /// Left.
        x0: f64,
        /// Right.
        x1: f64,
        /// Bottom (NaN: the bottom of the axis).
        y0: f64,
        /// Top.
        y1: f64,
        /// Pattern.
        pattern: crate::spec::FillPattern,
        /// Stroke (dots use its colour).
        stroke: Stroke,
    },
    /// An error bar with caps of `cap` points.
    ErrorBar {
        /// Position on the other axis.
        at: f64,
        /// Low end.
        lo: f64,
        /// High end.
        hi: f64,
        /// Horizontal bar (X error).
        horizontal: bool,
        /// Cap width (pt).
        cap: f64,
        /// Stroke.
        stroke: Stroke,
    },
    /// A segment.
    Segment {
        /// Start.
        p1: (f64, f64),
        /// End.
        p2: (f64, f64),
        /// Stroke.
        stroke: Stroke,
    },
    /// A horizontal line across the plot.
    HLine {
        /// Y.
        y: f64,
        /// Stroke.
        stroke: Stroke,
    },
    /// A vertical line across the plot.
    VLine {
        /// X.
        x: f64,
        /// Stroke.
        stroke: Stroke,
    },
    /// A horizontal segment of fixed point width centred at (x, y).
    Tick {
        /// X.
        x: f64,
        /// Y.
        y: f64,
        /// Half width (pt).
        half: f64,
        /// Stroke.
        stroke: Stroke,
    },
    /// Automatically placed point labels.
    Callouts(Vec<Callout>),
    /// A label at the end of a reference line (`vertical`: the line is at X = value).
    LineLabel {
        /// Vertical line (at X) or horizontal (at Y).
        vertical: bool,
        /// Position on the axis.
        value: f64,
        /// Runs.
        runs: Vec<Run>,
        /// Size.
        size: f64,
        /// Colour.
        color: Color,
    },
    /// Text at a data point, offset in points.
    Label {
        /// X.
        x: f64,
        /// Y.
        y: f64,
        /// Offset (pt).
        dx: f64,
        /// Offset (pt).
        dy: f64,
        /// Runs.
        runs: Vec<Run>,
        /// Size.
        size: f64,
        /// Colour.
        color: Color,
        /// Anchor.
        anchor: Anchor,
        /// Baseline.
        baseline: Baseline,
        /// Weight.
        weight: Weight,
    },
}

/// A label for a data point placed automatically beside it, avoiding other callouts and
/// staying inside the plot area (dropped when there is no room).
#[derive(Debug, Clone, PartialEq)]
pub struct Callout {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Runs.
    pub runs: Vec<Run>,
    /// Size.
    pub size: f64,
    /// Colour.
    pub color: Color,
}

/// A group of marks with an editor target.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// Element id.
    pub id: String,
    /// Spec pointer.
    pub target: String,
    /// Marks.
    pub marks: Vec<Mark>,
}

impl Layer {
    /// New layer.
    pub fn new(id: impl Into<String>, target: impl Into<String>) -> Self {
        Layer {
            id: id.into(),
            target: target.into(),
            marks: Vec::new(),
        }
    }
}

/// Legend swatch.
#[derive(Debug, Clone, PartialEq)]
pub enum Swatch {
    /// A marker.
    Marker(MarkerShape, Paint, f64),
    /// A line, optionally with a marker.
    Line(Stroke, Option<(MarkerShape, Paint, f64)>),
    /// A filled square.
    Fill(Paint),
}

/// One legend entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendEntry {
    /// Label markup.
    pub label: String,
    /// Swatch.
    pub swatch: Swatch,
    /// Dataset (for the editor target).
    pub dataset: Option<usize>,
}

/// A colour bar.
#[derive(Debug, Clone, PartialEq)]
pub struct ColourBar {
    /// Ramp.
    pub ramp: Ramp,
    /// Value at the bottom.
    pub lo: f64,
    /// Value at the top.
    pub hi: f64,
    /// Title.
    pub title: String,
}

/// Numbers at risk.
#[derive(Debug, Clone, PartialEq)]
pub struct AtRiskRow {
    /// Label.
    pub name: String,
    /// Colour.
    pub colour: Color,
    /// Observed times of every subject; the number at risk (time ≥ tick) is printed
    /// under each X major tick.
    pub observed: Vec<f64>,
    /// Precomputed (time, count) pairs, used instead when not empty.
    pub fixed: Vec<(f64, usize)>,
}

/// A dendrogram over categorical positions.
#[derive(Debug, Clone, PartialEq)]
pub struct Dendrogram {
    /// Merges from [`crate::stats::cluster`].
    pub merges: Vec<Merge>,
    /// Leaf order (leaf `order[i]` sits at position i + 1).
    pub order: Vec<usize>,
}

/// A column of text right of the plot, one row per Y position.
#[derive(Debug, Clone, PartialEq)]
pub struct TextColumn {
    /// Header.
    pub header: String,
    /// (y, text).
    pub rows: Vec<(f64, String)>,
}

/// What a family hands to the layout.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plot {
    /// X axis.
    pub x: AxisPlan,
    /// Y axis.
    pub y: AxisPlan,
    /// Secondary Y axis.
    pub y2: Option<Secondary>,
    /// Layers in drawing order.
    pub layers: Vec<Layer>,
    /// Legend entries.
    pub legend: Vec<LegendEntry>,
    /// Show the legend even with one entry.
    pub legend_always: bool,
    /// Where an `auto` legend goes (`None`: right).
    pub legend_auto: Option<LegendPosition>,
    /// Colour bar.
    pub colour_bar: Option<ColourBar>,
    /// At-risk table.
    pub at_risk: Vec<AtRiskRow>,
    /// Column dendrogram (above).
    pub dendro_x: Option<Dendrogram>,
    /// Row dendrogram (right).
    pub dendro_y: Option<Dendrogram>,
    /// Text column right of the plot.
    pub right_text: Option<TextColumn>,
    /// Height/width of the plot area, if fixed.
    pub aspect: Option<f64>,
    /// Frame override.
    pub frame: Option<FrameStyle>,
    /// Notes for the SVG.
    pub notes: Vec<String>,
    /// Draw transposed: the family's X (categories) runs down the left and its Y (values)
    /// along the bottom. `axes.x` and `axes.y` keep describing the family's axes.
    pub horizontal: bool,
}

/// Mapping from data to points along one axis, with breaks.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMap {
    /// Scale.
    pub scale: Scale,
    /// Domain in transformed units.
    pub d0: f64,
    /// Domain end.
    pub d1: f64,
    /// Point at d0.
    pub p0: f64,
    /// Point at d1.
    pub p1: f64,
    /// Segments (t0, t1, q0, q1).
    segs: Vec<(f64, f64, f64, f64)>,
}

impl AxisMap {
    fn new(scale: Scale, d0: f64, d1: f64, p0: f64, p1: f64, breaks: &[(f64, f64)]) -> Self {
        let mut bs: Vec<(f64, f64)> = breaks
            .iter()
            .map(|(a, b)| (a.min(*b), a.max(*b)))
            .filter(|(a, b)| *a > d0 && *b < d1 && b > a)
            .collect();
        bs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for b in bs {
            match merged.last_mut() {
                Some(m) if b.0 <= m.1 => m.1 = m.1.max(b.1),
                _ => merged.push(b),
            }
        }
        let mut segs = Vec::new();
        let total = (p1 - p0).abs();
        let gaps = merged.len() as f64 * BREAK_GAP;
        let kept: f64 = (d1 - d0) - merged.iter().map(|(a, b)| b - a).sum::<f64>();
        let dir = if p1 >= p0 { 1.0 } else { -1.0 };
        let per = ((total - gaps).max(total * 0.5)) / kept.max(1e-300);
        let mut t = d0;
        let mut q = p0;
        for (a, b) in merged.iter().copied().chain(std::iter::once((d1, d1))) {
            let len = (a - t) * per;
            segs.push((t, a, q, q + dir * len));
            q += dir * (len + if a < d1 { BREAK_GAP } else { 0.0 });
            t = b;
        }
        AxisMap {
            scale,
            d0,
            d1,
            p0,
            p1,
            segs,
        }
    }

    /// Transform a data value into axis units (log for log axes; NaN if impossible).
    pub fn tr(&self, v: f64) -> f64 {
        tr(self.scale, v)
    }

    /// Map a data value to points.
    pub fn map(&self, v: f64) -> f64 {
        self.map_t(self.tr(v))
    }

    /// Map a transformed value to points.
    pub fn map_t(&self, t: f64) -> f64 {
        let first = self.segs[0];
        if t <= first.0 || self.segs.len() == 1 {
            return lerp(first, t);
        }
        for (i, s) in self.segs.iter().enumerate() {
            if t <= s.1 {
                return lerp(*s, t);
            }
            if let Some(n) = self.segs.get(i + 1) {
                if t < n.0 {
                    // Inside a gap: snap to the nearer side.
                    return if t - s.1 < n.0 - t { s.3 } else { n.2 };
                }
            }
        }
        lerp(*self.segs.last().expect("segment"), t)
    }

    /// Whether a transformed value falls in a break.
    pub fn in_gap(&self, t: f64) -> bool {
        self.segs
            .windows(2)
            .any(|w| t > w[0].1 + 1e-12 && t < w[1].0 - 1e-12)
    }

    /// The gaps in points (start, end) for drawing break glyphs.
    pub fn gaps(&self) -> Vec<(f64, f64)> {
        self.segs.windows(2).map(|w| (w[0].3, w[1].2)).collect()
    }
}

fn lerp(s: (f64, f64, f64, f64), t: f64) -> f64 {
    if s.1 == s.0 {
        return s.2;
    }
    s.2 + (t - s.0) / (s.1 - s.0) * (s.3 - s.2)
}

/// Transform a value for a scale.
pub fn tr(scale: Scale, v: f64) -> f64 {
    match scale {
        Scale::Log10 => {
            if v > 0.0 {
                v.log10_d()
            } else {
                f64::NAN
            }
        }
        Scale::Log2 => {
            if v > 0.0 {
                v.log2_d()
            } else {
                f64::NAN
            }
        }
        _ => v,
    }
}

/// Inverse of [`tr`].
pub fn untr(scale: Scale, t: f64) -> f64 {
    match scale {
        Scale::Log10 => 10f64.powf_d(t),
        Scale::Log2 => 2f64.powf_d(t),
        _ => t,
    }
}

/// Build a text node from markup.
#[allow(clippy::too_many_arguments)]
pub fn text_node(
    x: f64,
    y: f64,
    s: &str,
    size: f64,
    color: Color,
    anchor: Anchor,
    baseline: Baseline,
    weight: Weight,
) -> Text {
    runs_node(x, y, text::parse(s), size, color, anchor, baseline, weight)
}

/// Build a text node from runs.
#[allow(clippy::too_many_arguments)]
pub fn runs_node(
    x: f64,
    y: f64,
    runs: Vec<Run>,
    size: f64,
    color: Color,
    anchor: Anchor,
    baseline: Baseline,
    weight: Weight,
) -> Text {
    Text {
        x,
        y,
        runs,
        size,
        weight,
        italic: false,
        color,
        anchor,
        baseline,
        rotate: 0.0,
    }
}

fn extend(acc: &mut (f64, f64), v: f64) {
    if v.is_finite() {
        acc.0 = acc.0.min(v);
        acc.1 = acc.1.max(v);
    }
}

/// Extents of all marks in transformed units: (x range, y range).
fn extents(layers: &[Layer], sx: Scale, sy: Scale) -> ((f64, f64), (f64, f64)) {
    let mut ex = (f64::INFINITY, f64::NEG_INFINITY);
    let mut ey = (f64::INFINITY, f64::NEG_INFINITY);
    let p = |x: f64, y: f64, ex: &mut (f64, f64), ey: &mut (f64, f64)| {
        if x.is_finite() {
            extend(ex, tr(sx, x));
        }
        if y.is_finite() {
            extend(ey, tr(sy, y));
        }
    };
    for l in layers {
        for m in &l.marks {
            match m {
                Mark::Points { pts, .. } | Mark::Line { pts, .. } | Mark::Polygon { pts, .. } => {
                    for (x, y) in pts {
                        p(*x, *y, &mut ex, &mut ey);
                    }
                }
                Mark::Band { x, lo, hi, .. } => {
                    for i in 0..x.len() {
                        p(x[i], lo[i], &mut ex, &mut ey);
                        p(x[i], hi[i], &mut ex, &mut ey);
                    }
                }
                Mark::Rect { x0, x1, y0, y1, .. } => {
                    p(*x0, *y0, &mut ex, &mut ey);
                    p(*x1, *y1, &mut ex, &mut ey);
                }
                Mark::ErrorBar {
                    at,
                    lo,
                    hi,
                    horizontal,
                    ..
                } => {
                    if *horizontal {
                        p(*lo, *at, &mut ex, &mut ey);
                        p(*hi, *at, &mut ex, &mut ey);
                    } else {
                        p(*at, *lo, &mut ex, &mut ey);
                        p(*at, *hi, &mut ex, &mut ey);
                    }
                }
                Mark::Segment { p1, p2, .. } => {
                    p(p1.0, p1.1, &mut ex, &mut ey);
                    p(p2.0, p2.1, &mut ex, &mut ey);
                }
                Mark::HLine { y, .. } => p(f64::NAN, *y, &mut ex, &mut ey),
                Mark::VLine { x, .. } => p(*x, f64::NAN, &mut ex, &mut ey),
                Mark::Tick { x, y, .. } => p(*x, *y, &mut ex, &mut ey),
                Mark::Label { .. }
                | Mark::Callouts(_)
                | Mark::LineLabel { .. }
                | Mark::Hatch { .. } => {}
            }
        }
    }
    (ex, ey)
}

/// Highest data value drawn at each category position 1..=n (transformed units).
fn category_tops(layers: &[Layer], n: usize, sy: Scale) -> Vec<f64> {
    let mut tops = vec![f64::NEG_INFINITY; n + 2];
    let mut put = |x: f64, y: f64| {
        let t = tr(sy, y);
        if !x.is_finite() || !t.is_finite() {
            return;
        }
        let c = x.round();
        if c >= 1.0 && (c as usize) <= n && (x - c).abs() <= 0.5 {
            let i = c as usize;
            tops[i] = tops[i].max(t);
        }
    };
    for l in layers {
        for m in &l.marks {
            match m {
                Mark::Points { pts, .. } | Mark::Line { pts, .. } | Mark::Polygon { pts, .. } => {
                    for (x, y) in pts {
                        put(*x, *y);
                    }
                }
                Mark::Rect { x0, x1, y0, y1, .. } => {
                    put((x0 + x1) / 2.0, y0.max(*y1));
                }
                Mark::ErrorBar {
                    at,
                    lo,
                    hi,
                    horizontal: false,
                    ..
                } => put(*at, lo.max(*hi)),
                Mark::Band { x, hi, .. } => {
                    for (a, b) in x.iter().zip(hi) {
                        put(*a, *b);
                    }
                }
                _ => {}
            }
        }
    }
    tops
}

/// Resolved axis: map, ticks, categorical labels.
#[derive(Debug, Clone)]
struct AxisLayout {
    map: AxisMap,
    ticks: TickSet,
    categorical: bool,
    scale: Scale,
}

#[derive(Debug, Clone, Copy)]
enum Side {
    X,
    Y,
}

#[allow(clippy::too_many_arguments)]
fn resolve_axis(
    side: Side,
    plan: &AxisPlan,
    spec_axis: &crate::spec::Axis,
    extent: (f64, f64),
    length: f64,
    tick_pt: f64,
    p0: f64,
    p1: f64,
) -> AxisLayout {
    if !plan.categories.is_empty() {
        let n = plan.categories.len();
        let (d0, d1) = (0.5, n as f64 + 0.5);
        let (q0, q1) = if plan.reverse { (p1, p0) } else { (p0, p1) };
        let major: Vec<f64> = (1..=n).map(|i| i as f64).collect();
        let labels = plan.categories.iter().map(|c| text::parse(c)).collect();
        return AxisLayout {
            map: AxisMap::new(Scale::Linear, d0, d1, q0, q1, &[]),
            ticks: TickSet {
                major,
                labels,
                minor: vec![],
                domain: (d0, d1),
            },
            categorical: true,
            scale: Scale::Categorical,
        };
    }
    let scale = plan.scale.unwrap_or(match spec_axis.scale {
        Scale::Categorical => Scale::Linear,
        s => s,
    });
    let (umin, umax) = (
        spec_axis
            .min
            .map(|v| tr(scale, v))
            .filter(|v| v.is_finite()),
        spec_axis
            .max
            .map(|v| tr(scale, v))
            .filter(|v| v.is_finite()),
    );
    let mut lo = extent.0;
    let mut hi = extent.1;
    for v in &plan.include {
        let t = tr(scale, *v);
        if t.is_finite() {
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    if !lo.is_finite() || !hi.is_finite() {
        (lo, hi) = match scale {
            Scale::Log10 | Scale::Log2 => (0.0, 1.0),
            _ => (0.0, 1.0),
        };
    }
    if hi <= lo {
        let d = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
        lo -= d;
        hi += d;
    }
    let anchored = |v: f64| {
        plan.include
            .iter()
            .any(|i| (tr(scale, *i) - v).abs() < 1e-12)
    };
    let (dlo, dhi, span) = (lo, hi, hi - lo);
    if let Some(m) = umin {
        lo = m;
    }
    if let Some(m) = umax {
        hi = m;
    }
    if hi <= lo {
        hi = lo + 1.0;
    }
    let per = match side {
        Side::X => tick_pt * 5.5,
        Side::Y => tick_pt * 3.6,
    };
    let target = ((length / per).round() as usize).clamp(2, 10);
    let fixed = plan.exact || (umin.is_some() && umax.is_some());
    let mut ts = ticks::ticks(
        scale,
        lo,
        hi,
        target,
        fixed,
        spec_axis.ticks.major.or(plan.major),
        spec_axis.ticks.minor,
        spec_axis.ticks.format,
        spec_axis.ticks.decimals,
    );
    if !fixed {
        // One side fixed by the user: keep it and drop ticks beyond it.
        if let Some(m) = umin {
            ts.domain.0 = m;
        }
        if let Some(m) = umax {
            ts.domain.1 = m;
        }
        let (a, b) = ts.domain;
        let keep: Vec<bool> = ts
            .major
            .iter()
            .map(|v| *v >= a - 1e-9 && *v <= b + 1e-9)
            .collect();
        let mut k = keep.iter();
        ts.labels.retain(|_| *k.next().expect("label"));
        ts.major.retain(|v| *v >= a - 1e-9 && *v <= b + 1e-9);
        ts.minor.retain(|v| *v > a && *v < b);
    }
    // Ticks cover the data; when data sit on (or very near) the last tick, the axis runs a
    // little past it so markers do not sit on the axis line.
    if !plan.exact {
        let need = span * plan.pad;
        if umin.is_none() && !anchored(dlo) && dlo - ts.domain.0 < need {
            ts.domain.0 = dlo - need;
        }
        if umax.is_none() && !anchored(dhi) && ts.domain.1 - dhi < need {
            ts.domain.1 = dhi + need;
        }
    }
    let breaks: Vec<(f64, f64)> = spec_axis
        .breaks
        .iter()
        .map(|b| (tr(scale, b.from), tr(scale, b.to)))
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    let map = AxisMap::new(scale, ts.domain.0, ts.domain.1, p0, p1, &breaks);
    if !breaks.is_empty() {
        let keep: Vec<bool> = ts.major.iter().map(|v| !map.in_gap(*v)).collect();
        let mut k = keep.iter();
        ts.labels.retain(|_| *k.next().expect("label"));
        ts.major.retain(|v| !map.in_gap(*v));
        ts.minor.retain(|v| !map.in_gap(*v));
    }
    thin(&mut ts, &map, side, tick_pt);
    AxisLayout {
        map,
        ticks: ts,
        categorical: false,
        scale,
    }
}

/// Drop every other label (keeping zero) until neighbouring labels have room; the
/// dropped majors stay as minor ticks.
fn thin(ts: &mut TickSet, map: &AxisMap, side: Side, size: f64) {
    let need = |ts: &TickSet| -> f64 {
        match side {
            Side::Y => size * 1.3,
            Side::X => {
                ts.labels
                    .iter()
                    .map(|r| text::runs_width(r, size, Weight::Regular, false))
                    .fold(0.0f64, f64::max)
                    + size * 0.8
            }
        }
    };
    for _ in 0..6 {
        if ts.major.len() < 3 {
            return;
        }
        let gap = ts
            .major
            .windows(2)
            .map(|w| (map.map_t(w[1]) - map.map_t(w[0])).abs())
            .fold(f64::INFINITY, f64::min);
        if gap >= need(ts) {
            return;
        }
        let keep_from = ts
            .major
            .iter()
            .position(|v| v.abs() < 1e-12)
            .map_or(0, |z| z % 2);
        let mut major = Vec::new();
        let mut labels = Vec::new();
        for (i, (v, l)) in ts.major.iter().zip(&ts.labels).enumerate() {
            if i % 2 == keep_from {
                major.push(*v);
                labels.push(l.clone());
            } else {
                ts.minor.push(*v);
            }
        }
        ts.minor.sort_by(f64::total_cmp);
        ts.major = major;
        ts.labels = labels;
    }
}

/// Liang–Barsky clip of a segment to a rectangle.
fn clip_segment(a: (f64, f64), b: (f64, f64), r: &Rect) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-dx, a.0 - r.x),
        (dx, r.right() - a.0),
        (-dy, a.1 - r.y),
        (dy, r.bottom() - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then_some((
        (a.0 + t0 * dx, a.1 + t0 * dy),
        (a.0 + t1 * dx, a.1 + t1 * dy),
    ))
}

/// Clip a polyline into visible pieces.
fn clip_polyline(pts: &[(f64, f64)], r: &Rect) -> Vec<Vec<(f64, f64)>> {
    let inside = |p: &(f64, f64)| p.0 >= r.x && p.0 <= r.right() && p.1 >= r.y && p.1 <= r.bottom();
    if pts.iter().all(inside) {
        return vec![pts.to_vec()];
    }
    let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for w in pts.windows(2) {
        match clip_segment(w[0], w[1], r) {
            Some((a, b)) => {
                if cur
                    .last()
                    .is_none_or(|l| (l.0 - a.0).abs() > 1e-9 || (l.1 - a.1).abs() > 1e-9)
                {
                    if cur.len() > 1 {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(a);
                }
                cur.push(b);
            }
            None => {
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// Sutherland–Hodgman polygon clip.
fn clip_polygon(pts: &[(f64, f64)], r: &Rect) -> Vec<(f64, f64)> {
    let mut poly = pts.to_vec();
    let edges: [(u8, f64); 4] = [(0, r.x), (1, r.right()), (2, r.y), (3, r.bottom())];
    for (k, v) in edges {
        if poly.is_empty() {
            break;
        }
        let ins = |p: &(f64, f64)| match k {
            0 => p.0 >= v,
            1 => p.0 <= v,
            2 => p.1 >= v,
            _ => p.1 <= v,
        };
        let cross = |a: (f64, f64), b: (f64, f64)| {
            let t = match k {
                0 | 1 => (v - a.0) / (b.0 - a.0),
                _ => (v - a.1) / (b.1 - a.1),
            };
            (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
        };
        let mut out = Vec::with_capacity(poly.len() + 4);
        for i in 0..poly.len() {
            let a = poly[(i + poly.len() - 1) % poly.len()];
            let b = poly[i];
            match (ins(&a), ins(&b)) {
                (true, true) => out.push(b),
                (true, false) => out.push(cross(a, b)),
                (false, true) => {
                    out.push(cross(a, b));
                    out.push(b);
                }
                (false, false) => {}
            }
        }
        poly = out;
    }
    poly
}

fn step_pts(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut o = Vec::with_capacity(pts.len() * 2);
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            o.push((p.0, pts[i - 1].1));
        }
        o.push(*p);
    }
    o
}

/// Decimate points to one per cell of a `cell`-point grid (first point wins).
fn decimate(pts: Vec<(f64, f64)>, r: &Rect, cell: f64) -> Vec<(f64, f64)> {
    let cols = (r.width / cell).ceil().max(1.0) as usize + 2;
    let rows = (r.height / cell).ceil().max(1.0) as usize + 2;
    let mut seen = vec![false; cols * rows];
    let mut out = Vec::with_capacity(pts.len().min(cols * rows));
    for p in pts {
        let c = (((p.0 - r.x) / cell).floor() + 1.0).clamp(0.0, (cols - 1) as f64) as usize;
        let rr = (((p.1 - r.y) / cell).floor() + 1.0).clamp(0.0, (rows - 1) as f64) as usize;
        let k = rr * cols + c;
        if !seen[k] {
            seen[k] = true;
            out.push(p);
        }
    }
    out
}

struct Mapper<'a> {
    x: &'a AxisMap,
    y: &'a AxisMap,
    clip: Rect,
}

impl Mapper<'_> {
    fn pt(&self, p: (f64, f64)) -> (f64, f64) {
        (self.x.map(p.0), self.y.map(p.1))
    }
}

/// The point rectangles of a data rectangle: clipped to the plot area and cut where either
/// axis is broken. A NaN bottom (`y0`) or left (`x0`) means the edge of the plot area (bars
/// on log axes).
fn rect_pieces(mp: &Mapper<'_>, x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<Rect> {
    let c = &mp.clip;
    let cl = c.inflate(0.5);
    let a = if x0.is_nan() { c.x } else { mp.x.map(x0) };
    let b = mp.x.map(x1);
    let d = if y0.is_nan() {
        c.bottom()
    } else {
        mp.y.map(y0)
    };
    let e = mp.y.map(y1);
    if ![a, b, d, e].iter().all(|v| v.is_finite()) {
        return Vec::new();
    }
    let (l, r) = (a.min(b).max(cl.x), a.max(b).min(cl.right()));
    let (t, bt) = (d.min(e).max(cl.y), d.max(e).min(cl.bottom()));
    if !(r > l && bt > t) {
        return Vec::new();
    }
    let cut = |span: (f64, f64), gaps: Vec<(f64, f64)>| -> Vec<(f64, f64)> {
        let mut pieces = vec![span];
        for (g0, g1) in gaps {
            let (g0, g1) = (g0.min(g1), g0.max(g1));
            pieces = pieces
                .into_iter()
                .flat_map(|(a, b)| {
                    if g0 >= b || g1 <= a {
                        vec![(a, b)]
                    } else {
                        [(a, g0), (g1, b)]
                            .into_iter()
                            .filter(|(p, q)| q - p > 0.01)
                            .collect()
                    }
                })
                .collect();
        }
        pieces
    };
    let mut out = Vec::new();
    for (xa, xb) in cut((l, r), mp.x.gaps()) {
        for (ya, yb) in cut((t, bt), mp.y.gaps()) {
            out.push(Rect::new(xa, ya, xb - xa, yb - ya));
        }
    }
    out
}

/// Pattern strokes (or dots) filling a rectangle in points, spaced 3 pt apart.
fn hatch_nodes(r: &Rect, pattern: crate::spec::FillPattern, stroke: Stroke, out: &mut Vec<Node>) {
    use crate::spec::FillPattern as F;
    const STEP: f64 = 3.0;
    type Seg = ((f64, f64), (f64, f64));
    let mut lines: Vec<Seg> = Vec::new();
    let (w, h) = (r.width, r.height);
    let diag = |sign: f64, lines: &mut Vec<Seg>| {
        // Lines x = x0 + k*STEP ± (y - y0), spanning the rectangle, clipped to it.
        let n = ((w + h) / STEP).ceil() as i64 + 1;
        for k in -n..=n {
            let x = r.x + k as f64 * STEP;
            let (p1, p2) = if sign > 0.0 {
                ((x, r.bottom()), (x + h, r.y))
            } else {
                ((x, r.y), (x + h, r.bottom()))
            };
            if let Some(seg) = clip_segment(p1, p2, r) {
                lines.push(seg);
            }
        }
    };
    match pattern {
        F::None => return,
        F::Hatch => diag(1.0, &mut lines),
        F::BackHatch => diag(-1.0, &mut lines),
        F::Crosshatch => {
            diag(1.0, &mut lines);
            diag(-1.0, &mut lines);
        }
        F::Horizontal => {
            let mut y = r.bottom() - STEP / 2.0;
            while y > r.y {
                lines.push(((r.x, y), (r.right(), y)));
                y -= STEP;
            }
        }
        F::Vertical => {
            let mut x = r.x + STEP / 2.0;
            while x < r.right() {
                lines.push(((x, r.y), (x, r.bottom())));
                x += STEP;
            }
        }
        F::Dots => {
            let mut pts = Vec::new();
            let mut y = r.bottom() - STEP / 2.0;
            let mut row = 0;
            while y > r.y {
                let mut x = r.x + STEP / 2.0 + if row % 2 == 1 { STEP / 2.0 } else { 0.0 };
                while x < r.right() {
                    pts.push((x, y));
                    x += STEP;
                }
                y -= STEP;
                row += 1;
            }
            out.push(Node::Markers {
                shape: MarkerShape::Circle,
                size: (stroke.width * 1.4).max(0.6),
                paint: Paint::fill(stroke.color),
                points: pts,
            });
            return;
        }
    }
    for (a, b) in lines {
        if (a.0 - b.0).abs() + (a.1 - b.1).abs() > 0.05 {
            out.push(Node::Line {
                p1: a,
                p2: b,
                stroke,
            });
        }
    }
}

/// Split a polyline (in points) where it crosses an axis break: the stretch inside each
/// gap is left out, so lines show the discontinuity as the axis does.
fn cut_at_gaps(
    pts: Vec<(f64, f64)>,
    xgaps: &[(f64, f64)],
    ygaps: &[(f64, f64)],
) -> Vec<Vec<(f64, f64)>> {
    if xgaps.is_empty() && ygaps.is_empty() {
        return vec![pts];
    }
    let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let gaps: Vec<(bool, f64, f64)> = xgaps
        .iter()
        .map(|g| (true, g.0.min(g.1), g.0.max(g.1)))
        .chain(ygaps.iter().map(|g| (false, g.0.min(g.1), g.0.max(g.1))))
        .collect();
    for (i, p) in pts.iter().enumerate() {
        if i == 0 {
            cur.push(*p);
            continue;
        }
        let a = pts[i - 1];
        // Parameter intervals of the segment inside any gap.
        let mut cuts: Vec<(f64, f64)> = Vec::new();
        for (on_x, g0, g1) in &gaps {
            let (u, v) = if *on_x { (a.0, p.0) } else { (a.1, p.1) };
            if (v - u).abs() < 1e-12 {
                continue;
            }
            let t0 = (g0 - u) / (v - u);
            let t1 = (g1 - u) / (v - u);
            let (t0, t1) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
            if t1 > t0 && t0 < 1.0 && t1 > 0.0 {
                cuts.push((t0, t1));
            }
        }
        if cuts.is_empty() {
            cur.push(*p);
            continue;
        }
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        let at = |t: f64| (a.0 + (p.0 - a.0) * t, a.1 + (p.1 - a.1) * t);
        for (t0, t1) in cuts {
            cur.push(at(t0));
            if cur.len() >= 2 {
                out.push(std::mem::take(&mut cur));
            }
            cur.clear();
            cur.push(at(t1));
        }
        cur.push(*p);
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

fn mark_nodes(
    m: &Mark,
    mp: &Mapper<'_>,
    out: &mut Vec<Node>,
    notes: &mut Vec<String>,
    layer_id: &str,
    placed: &mut Vec<Rect>,
) {
    let c = &mp.clip;
    let tol = 0.5;
    let cl = c.inflate(tol);
    match m {
        Mark::Points {
            pts,
            shape,
            size,
            paint,
        } => {
            let mut v: Vec<(f64, f64)> = pts
                .iter()
                .map(|p| mp.pt(*p))
                .filter(|(x, y)| {
                    x.is_finite()
                        && y.is_finite()
                        && *x >= cl.x
                        && *x <= cl.right()
                        && *y >= cl.y
                        && *y <= cl.bottom()
                })
                .collect();
            if v.len() > LOD_THRESHOLD {
                let n0 = v.len();
                v = decimate(v, c, 0.75);
                let note = format!(
                    "{layer_id}: {n0} points drawn as {} after binning to a 0.75 pt grid (level of detail)",
                    v.len()
                );
                out.push(Node::Comment(note.clone()));
                notes.push(note);
            }
            out.push(Node::Markers {
                shape: *shape,
                size: *size,
                paint: *paint,
                points: v,
            });
        }
        Mark::Line { pts, stroke, step } => {
            let mut v: Vec<(f64, f64)> = pts
                .iter()
                .map(|p| mp.pt(*p))
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .collect();
            if *step {
                v = step_pts(&v);
            }
            for piece in cut_at_gaps(v, &mp.x.gaps(), &mp.y.gaps())
                .iter()
                .flat_map(|p| clip_polyline(p, &cl))
            {
                out.push(Node::Poly {
                    points: piece,
                    closed: false,
                    paint: Paint::stroke(*stroke),
                });
            }
        }
        Mark::Band {
            x,
            lo,
            hi,
            fill,
            step,
        } => {
            let mut up: Vec<(f64, f64)> = Vec::new();
            let mut dn: Vec<(f64, f64)> = Vec::new();
            for i in 0..x.len() {
                let (a, b, d) = (mp.x.map(x[i]), mp.y.map(hi[i]), mp.y.map(lo[i]));
                if a.is_finite() && b.is_finite() && d.is_finite() {
                    up.push((a, b));
                    dn.push((a, d));
                }
            }
            if *step {
                up = step_pts(&up);
                dn = step_pts(&dn);
            }
            dn.reverse();
            up.extend(dn);
            let poly = clip_polygon(&up, &cl);
            if poly.len() > 2 {
                out.push(Node::Poly {
                    points: poly,
                    closed: true,
                    paint: Paint::fill(*fill),
                });
            }
        }
        Mark::Polygon { pts, paint } => {
            let v: Vec<(f64, f64)> = pts
                .iter()
                .map(|p| mp.pt(*p))
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .collect();
            let poly = clip_polygon(&v, &cl);
            if poly.len() > 2 {
                out.push(Node::Poly {
                    points: poly,
                    closed: true,
                    paint: *paint,
                });
            }
        }
        Mark::Rect {
            x0,
            x1,
            y0,
            y1,
            paint,
        } => {
            for rect in rect_pieces(mp, *x0, *x1, *y0, *y1) {
                out.push(Node::Rect {
                    rect,
                    paint: *paint,
                });
            }
        }
        Mark::Hatch {
            x0,
            x1,
            y0,
            y1,
            pattern,
            stroke,
        } => {
            for r in rect_pieces(mp, *x0, *x1, *y0, *y1) {
                hatch_nodes(&r, *pattern, *stroke, out);
            }
        }
        Mark::ErrorBar {
            at,
            lo,
            hi,
            horizontal,
            cap,
            stroke,
        } => {
            let (p1, p2) = if *horizontal {
                (mp.pt((*lo, *at)), mp.pt((*hi, *at)))
            } else {
                (mp.pt((*at, *lo)), mp.pt((*at, *hi)))
            };
            if !(p1.0.is_finite() && p1.1.is_finite() && p2.0.is_finite() && p2.1.is_finite()) {
                return;
            }
            if let Some((a, b)) = clip_segment(p1, p2, &cl) {
                out.push(Node::Line {
                    p1: a,
                    p2: b,
                    stroke: *stroke,
                });
                let h = cap / 2.0;
                if h > 0.0 {
                    for (end, orig) in [(a, p1), (b, p2)] {
                        // Caps only where the bar was not clipped.
                        if (end.0 - orig.0).abs() < 1e-6 && (end.1 - orig.1).abs() < 1e-6 {
                            let (q1, q2) = if *horizontal {
                                ((end.0, end.1 - h), (end.0, end.1 + h))
                            } else {
                                ((end.0 - h, end.1), (end.0 + h, end.1))
                            };
                            out.push(Node::Line {
                                p1: q1,
                                p2: q2,
                                stroke: *stroke,
                            });
                        }
                    }
                }
            }
        }
        Mark::Segment { p1, p2, stroke } => {
            if stroke.width <= 0.0 {
                // Extent-only marks (head room for labels) are never drawn.
                return;
            }
            let (a, b) = (mp.pt(*p1), mp.pt(*p2));
            if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) {
                return;
            }
            if let Some((a, b)) = clip_segment(a, b, &cl) {
                out.push(Node::Line {
                    p1: a,
                    p2: b,
                    stroke: *stroke,
                });
            }
        }
        Mark::HLine { y, stroke } => {
            let py = mp.y.map(*y);
            if py.is_finite() && py >= cl.y && py <= cl.bottom() {
                out.push(Node::Line {
                    p1: (c.x, py),
                    p2: (c.right(), py),
                    stroke: *stroke,
                });
            }
        }
        Mark::VLine { x, stroke } => {
            let px = mp.x.map(*x);
            if px.is_finite() && px >= cl.x && px <= cl.right() {
                out.push(Node::Line {
                    p1: (px, c.y),
                    p2: (px, c.bottom()),
                    stroke: *stroke,
                });
            }
        }
        Mark::Tick { x, y, half, stroke } => {
            let (px, py) = mp.pt((*x, *y));
            if px.is_finite() && py.is_finite() && py >= cl.y && py <= cl.bottom() {
                out.push(Node::Line {
                    p1: (px - half, py),
                    p2: (px + half, py),
                    stroke: *stroke,
                });
            }
        }
        Mark::LineLabel {
            vertical,
            value,
            runs,
            size,
            color,
        } => {
            let t = if *vertical {
                let px = mp.x.map(*value);
                runs_node(
                    px + 2.5,
                    c.y + 1.0,
                    runs.clone(),
                    *size,
                    *color,
                    Anchor::Start,
                    Baseline::Top,
                    Weight::Regular,
                )
            } else {
                let py = mp.y.map(*value);
                runs_node(
                    c.right() - 1.0,
                    py - 2.0,
                    runs.clone(),
                    *size,
                    *color,
                    Anchor::End,
                    Baseline::Alphabetic,
                    Weight::Regular,
                )
            };
            if t.x.is_finite() && t.y.is_finite() {
                out.push(Node::Text(t));
            }
        }
        Mark::Callouts(list) => {
            let opts: [(f64, f64, Anchor); 8] = [
                (4.0, 0.0, Anchor::Start),
                (-4.0, 0.0, Anchor::End),
                (3.0, -6.0, Anchor::Start),
                (-3.0, -6.0, Anchor::End),
                (3.0, 6.0, Anchor::Start),
                (-3.0, 6.0, Anchor::End),
                (0.0, -7.5, Anchor::Middle),
                (0.0, 8.0, Anchor::Middle),
            ];
            for co in list {
                let (px, py) = mp.pt((co.x, co.y));
                if !(px.is_finite() && py.is_finite()) {
                    continue;
                }
                for (dx, dy, anchor) in opts {
                    let t = runs_node(
                        px + dx,
                        py + dy,
                        co.runs.clone(),
                        co.size,
                        co.color,
                        anchor,
                        Baseline::Middle,
                        Weight::Regular,
                    );
                    let b = t.bbox();
                    let inside = b.x >= c.x
                        && b.right() <= c.right()
                        && b.y >= c.y
                        && b.bottom() <= c.bottom();
                    if inside && !placed.iter().any(|p| p.overlaps(&b, 0.0)) {
                        placed.push(b.inflate(0.5));
                        out.push(Node::Text(t));
                        break;
                    }
                }
            }
        }
        Mark::Label {
            x,
            y,
            dx,
            dy,
            runs,
            size,
            color,
            anchor,
            baseline,
            weight,
        } => {
            let (px, py) = mp.pt((*x, *y));
            if px.is_finite() && py.is_finite() {
                out.push(Node::Text(runs_node(
                    px + dx,
                    py + dy,
                    runs.clone(),
                    *size,
                    *color,
                    *anchor,
                    *baseline,
                    *weight,
                )));
            }
        }
    }
}

/// Legend geometry.
pub(crate) struct LegendBox {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) horizontal: bool,
}

fn legend_rows(entries: &[LegendEntry], t: &Theme) -> Vec<f64> {
    entries
        .iter()
        .map(|e| text::runs_width(&text::parse(&e.label), t.tick_pt, Weight::Regular, false))
        .collect()
}

const SWATCH_W: f64 = 12.0;

pub(crate) fn legend_box(
    entries: &[LegendEntry],
    title: Option<&str>,
    t: &Theme,
    horizontal: bool,
) -> LegendBox {
    let widths = legend_rows(entries, t);
    let row_h = t.tick_pt * 1.5;
    let title_h = if title.is_some() {
        t.tick_pt * 1.5
    } else {
        0.0
    };
    let title_w = title.map_or(0.0, |s| {
        text::runs_width(&text::parse(s), t.tick_pt, Weight::SemiBold, false)
    });
    if horizontal {
        let w: f64 = widths.iter().map(|w| SWATCH_W + 4.0 + w).sum::<f64>()
            + 10.0 * (widths.len().saturating_sub(1)) as f64;
        LegendBox {
            width: w.max(title_w),
            height: row_h + title_h,
            horizontal,
        }
    } else {
        let w = widths.iter().fold(0.0f64, |a, b| a.max(*b)) + SWATCH_W + 4.0;
        LegendBox {
            width: w.max(title_w),
            height: row_h * entries.len() as f64 + title_h,
            horizontal,
        }
    }
}

fn draw_swatch(s: &Swatch, x: f64, y: f64, g: &mut Group) {
    let cx = x + SWATCH_W / 2.0;
    match s {
        Swatch::Marker(shape, paint, size) => g.push(Node::Markers {
            shape: *shape,
            size: *size,
            paint: *paint,
            points: vec![(cx, y)],
        }),
        Swatch::Line(stroke, marker) => {
            g.push(Node::Line {
                p1: (x, y),
                p2: (x + SWATCH_W, y),
                stroke: *stroke,
            });
            if let Some((shape, paint, size)) = marker {
                g.push(Node::Markers {
                    shape: *shape,
                    size: *size,
                    paint: *paint,
                    points: vec![(cx, y)],
                });
            }
        }
        Swatch::Fill(paint) => {
            let h = 7.0;
            g.push(Node::Rect {
                rect: Rect::new(cx - h / 2.0, y - h / 2.0, h, h),
                paint: *paint,
            })
        }
    }
}

pub(crate) fn draw_legend(
    entries: &[LegendEntry],
    title: Option<&str>,
    t: &Theme,
    x: f64,
    y: f64,
    lb: &LegendBox,
) -> Group {
    let mut g = Group::new("legend", "/legend");
    let row_h = t.tick_pt * 1.5;
    let mut cy = y;
    if let Some(ti) = title {
        g.push(Node::Text(text_node(
            x,
            cy + row_h / 2.0,
            ti,
            t.tick_pt,
            t.text,
            Anchor::Start,
            Baseline::Middle,
            Weight::SemiBold,
        )));
        cy += t.tick_pt * 1.5;
    }
    let widths = legend_rows(entries, t);
    let mut cx = x;
    for (i, e) in entries.iter().enumerate() {
        let target = match e.dataset {
            Some(d) => format!("/style/datasets/{d}"),
            None => "/legend".to_string(),
        };
        let mut item = Group::new(format!("legend-item-{i}"), target);
        let my = cy + row_h / 2.0;
        draw_swatch(&e.swatch, cx, my, &mut item);
        item.push(Node::Text(text_node(
            cx + SWATCH_W + 4.0,
            my,
            &e.label,
            t.tick_pt,
            t.text,
            Anchor::Start,
            Baseline::Middle,
            Weight::Regular,
        )));
        g.push(Node::Group(item));
        if lb.horizontal {
            cx += SWATCH_W + 4.0 + widths[i] + 10.0;
        } else {
            cy += row_h;
        }
    }
    g
}

fn label_dims(ts: &TickSet, size: f64) -> (f64, f64) {
    let m = text::vmetrics();
    let w = ts
        .labels
        .iter()
        .map(|r| text::runs_width(r, size, Weight::Regular, false))
        .fold(0.0f64, f64::max);
    let sup = ts
        .labels
        .iter()
        .any(|r| r.iter().any(|x| x.shift == text::Shift::Super));
    let h = size * (m.cap_height - m.descender) + if sup { size * 0.25 } else { 0.0 };
    (w, h)
}

fn tick_out(dir: TickDirection, len: f64) -> f64 {
    match dir {
        TickDirection::Out => len,
        TickDirection::Cross => len / 2.0,
        TickDirection::In => 0.0,
    }
}

/// Whether categorical x labels need rotating at this plot width.
fn x_needs_rotation(ts: &TickSet, size: f64, pw: f64, n: usize) -> bool {
    if n == 0 {
        return false;
    }
    let slot = pw / n as f64;
    ts.labels
        .iter()
        .any(|r| text::runs_width(r, size, Weight::Regular, false) > slot * 0.92)
}

/// How the X tick labels are set: straight, wrapped onto several lines, or rotated.
struct XLabels {
    rotate: bool,
    lines: Option<Vec<Vec<Vec<Run>>>>,
    height: f64,
    angle: f64,
}

fn wrap_label(src: &str, size: f64, maxw: f64) -> Option<Vec<Vec<Run>>> {
    let width = |s: &str| text::runs_width(&text::parse(s), size, Weight::Regular, false);
    let mut lines: Vec<String> = Vec::new();
    for para in src.split('\n') {
        let mut cur = String::new();
        for word in para.split(' ') {
            let cand = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if cur.is_empty() || width(&cand) <= maxw {
                cur = cand;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        lines.push(cur);
    }
    if lines.len() > 3 || lines.iter().any(|l| width(l) > maxw) {
        return None;
    }
    Some(lines.iter().map(|l| text::parse(l)).collect())
}

fn x_labels(
    spec: &GraphSpec,
    plan: &AxisPlan,
    x: &AxisLayout,
    fs: f64,
    pw: f64,
    show: bool,
) -> XLabels {
    let angle = spec.axes.x.ticks.label_angle.unwrap_or(45.0).abs();
    if !show {
        return XLabels {
            rotate: false,
            lines: None,
            height: 0.0,
            angle,
        };
    }
    let (xlw, xlh) = label_dims(&x.ticks, fs);
    let line_h = fs * 1.2;
    if let Some(a) = spec.axes.x.ticks.label_angle {
        if a != 0.0 {
            let r = a.abs().to_radians();
            return XLabels {
                rotate: true,
                lines: None,
                height: xlw * r.sin_d() + xlh * r.cos_d(),
                angle,
            };
        }
    }
    let n = x.ticks.major.len();
    let explicit = plan.categories.iter().any(|c| c.contains('\n'));
    if x.categorical && n > 0 && (explicit || x_needs_rotation(&x.ticks, fs, pw, n)) {
        let maxw = pw / n as f64 * 0.94;
        let wrapped: Option<Vec<Vec<Vec<Run>>>> = plan
            .categories
            .iter()
            .map(|c| wrap_label(c, fs, maxw))
            .collect();
        if let Some(w) = wrapped {
            let nl = w.iter().map(|l| l.len()).max().unwrap_or(1);
            return XLabels {
                rotate: false,
                height: xlh + (nl as f64 - 1.0) * line_h,
                lines: Some(w),
                angle,
            };
        }
        if spec.axes.x.ticks.label_angle.is_none() {
            let r = angle.to_radians();
            return XLabels {
                rotate: true,
                lines: None,
                height: xlw * r.sin_d() + xlh * r.cos_d(),
                angle,
            };
        }
    }
    XLabels {
        rotate: false,
        lines: None,
        height: xlh,
        angle,
    }
}

/// An axis title set on one to three lines, shrunk when it still does not fit.
#[derive(Debug, Clone, Default)]
struct TitleBlock {
    lines: Vec<Vec<Run>>,
    size: f64,
}

impl TitleBlock {
    fn new(src: &str, size: f64, maxw: f64) -> TitleBlock {
        if src.is_empty() {
            return TitleBlock::default();
        }
        let mut s = size;
        loop {
            if let Some(lines) = wrap_label(src, s, maxw.max(10.0)) {
                return TitleBlock { lines, size: s };
            }
            if s < 5.0 {
                return TitleBlock {
                    lines: vec![text::parse(src)],
                    size: s,
                };
            }
            s *= 0.92;
        }
    }

    /// Extent across the lines (height for X titles, width for rotated Y titles).
    fn depth(&self) -> f64 {
        if self.lines.is_empty() {
            return 0.0;
        }
        let m = text::vmetrics();
        self.size * (m.cap_height - m.descender) + (self.lines.len() as f64 - 1.0) * self.size * 1.2
    }
}

/// Lay out and draw a Cartesian plot.
pub fn render(ctx: &Ctx<'_>, mut plot: Plot) -> Scene {
    let spec = ctx.spec;
    let t = &ctx.theme;
    let w = spec.style.size_mm.width * 72.0 / 25.4;
    let h = spec.style.size_mm.height * 72.0 / 25.4;
    let fs = t.tick_pt;
    let vm = text::vmetrics();
    let title_h = |s: f64| s * (vm.cap_height - vm.descender);

    let sx = if plot.x.categories.is_empty() {
        plot.x.scale.unwrap_or(match spec.axes.x.scale {
            Scale::Categorical => Scale::Linear,
            s => s,
        })
    } else {
        Scale::Linear
    };
    let sy = if plot.y.categories.is_empty() {
        plot.y.scale.unwrap_or(match spec.axes.y.scale {
            Scale::Categorical => Scale::Linear,
            s => s,
        })
    } else {
        Scale::Linear
    };

    // Significance brackets become marks before the ranges are fixed.
    let (_, ey0) = extents(&plot.layers, sx, sy);
    add_brackets(ctx, &mut plot, sy, ey0);
    // Horizontal graphs: everything was laid out with categories along X; turn it so the
    // categories run down the left, and let the spec's value axis (`axes.y`) style the
    // bottom axis.
    let horizontal = plot.horizontal;
    let swapped;
    let (spec, sx, sy) = if horizontal {
        transpose(&mut plot);
        swapped = swap_axes(spec);
        (&swapped, sy, sx)
    } else {
        (spec, sx, sy)
    };
    let (ex, ey) = extents(&plot.layers, sx, sy);

    let x_title = spec
        .axes
        .x
        .title
        .clone()
        .unwrap_or_else(|| plot.x.title.clone());
    let y_title = spec
        .axes
        .y
        .title
        .clone()
        .unwrap_or_else(|| plot.y.title.clone());
    let show_x = !spec.axes.x.hidden;
    let show_y = !spec.axes.y.hidden;
    let tick_len = spec.axes.x.ticks.length_pt.max(0.0);
    let ytick_len = spec.axes.y.ticks.length_pt.max(0.0);
    let xdir = spec.axes.x.ticks.direction;
    let ydir = spec.axes.y.ticks.direction;
    let xout = if plot.x.bare {
        0.0
    } else {
        tick_out(xdir, tick_len)
    };
    let yout = if plot.y.bare {
        0.0
    } else {
        tick_out(ydir, ytick_len)
    };
    let offset = spec.axes.offset_pt.unwrap_or(t.offset_pt).max(0.0);

    // Legend.
    let legend_pos = if plot.legend.len() < 2 && !plot.legend_always {
        LegendPosition::Hidden
    } else if spec.legend.position == LegendPosition::Auto {
        plot.legend_auto.unwrap_or(LegendPosition::Right)
    } else {
        spec.legend.position
    };
    let legend_title = spec.legend.title.as_deref();
    let manual_legend = spec.legend.at.is_some() && legend_pos != LegendPosition::Hidden;
    let lb = legend_box(
        &plot.legend,
        legend_title,
        t,
        matches!(legend_pos, LegendPosition::Top | LegendPosition::Bottom),
    );

    // Fixed reservations.
    let heading = spec.title.as_deref().filter(|s| !s.is_empty());
    // Semibold runs a little wider than the regular metrics used for wrapping.
    let htb = TitleBlock::new(
        heading.unwrap_or(""),
        t.heading_pt,
        (w - 2.0 * OUTER) * 0.93,
    );
    let mut top = OUTER
        + if htb.lines.is_empty() {
            0.0
        } else {
            htb.depth() + 6.0
        };
    if legend_pos == LegendPosition::Top && !manual_legend {
        top += lb.height + 4.0;
    }
    if plot.right_text.is_some() {
        top += fs * 1.6;
    }
    let dendro_h = if plot.dendro_x.is_some() {
        (h * 0.12).max(14.0)
    } else {
        0.0
    };
    top += dendro_h;
    let mut right_fixed = OUTER;
    if legend_pos == LegendPosition::Right && !manual_legend {
        right_fixed += lb.width + 8.0;
    }
    let mut cb_w = 0.0;
    if let Some(cb) = &plot.colour_bar {
        let ts = ticks::ticks(
            Scale::Linear,
            cb.lo,
            cb.hi,
            5,
            true,
            None,
            0,
            crate::spec::TickFormat::Auto,
            None,
        );
        let (lw, _) = label_dims(&ts, fs);
        let tw = text::runs_width(&text::parse(&cb.title), fs, Weight::Regular, false);
        cb_w = (8.0 + 3.0 + LABEL_GAP + lw).max(tw) + 8.0 + 4.0;
        right_fixed += cb_w;
    }
    let dendro_w = if plot.dendro_y.is_some() {
        (w * 0.1).max(14.0)
    } else {
        0.0
    };
    right_fixed += dendro_w;
    let mut rt_w = 0.0;
    if let Some(rt) = &plot.right_text {
        let mw = rt
            .rows
            .iter()
            .map(|(_, s)| text::runs_width(&text::parse(s), fs, Weight::Regular, false))
            .fold(
                text::runs_width(&text::parse(&rt.header), fs, Weight::SemiBold, false),
                f64::max,
            );
        rt_w = mw + 8.0;
        right_fixed += rt_w;
    }
    let at_risk_h = if plot.at_risk.is_empty() {
        0.0
    } else {
        (plot.at_risk.len() as f64 + 1.0) * fs * 1.45 + 6.0
    };
    let mut bottom_fixed = OUTER + at_risk_h;
    if legend_pos == LegendPosition::Bottom && !manual_legend {
        bottom_fixed += lb.height + 6.0;
    }
    let at_risk_label_w = plot
        .at_risk
        .iter()
        .map(|r| text::runs_width(&text::parse(&r.name), fs, Weight::Regular, false))
        .fold(0.0f64, f64::max);

    // Iterate: margins depend on tick labels, which depend on the axis lengths.
    let mut left = 36.0;
    let mut bottom = bottom_fixed + 24.0;
    let mut right = right_fixed + 4.0;
    let mut top_m = top + fs * 0.5;
    let mut xa: Option<AxisLayout> = None;
    let mut ya: Option<AxisLayout> = None;
    let mut rotate = false;
    let mut y2ticks: Option<TickSet> = None;
    let mut ytb = TitleBlock::default();
    let mut xtb = TitleBlock::default();
    for _ in 0..4 {
        let pw = (w - left - right).max(20.0);
        let ph = (h - top_m - bottom).max(20.0);
        let x = resolve_axis(Side::X, &plot.x, &spec.axes.x, ex, pw, fs, left, left + pw);
        let y = resolve_axis(
            Side::Y,
            &plot.y,
            &spec.axes.y,
            ey,
            ph,
            fs,
            top_m + ph,
            top_m,
        );
        // Measure.
        let (ylw, ylh) = if show_y {
            label_dims(&y.ticks, fs)
        } else {
            (0.0, 0.0)
        };
        let (xlw, xlh) = if show_x {
            label_dims(&x.ticks, fs)
        } else {
            (0.0, 0.0)
        };
        let xl = x_labels(spec, &plot.x, &x, fs, pw, show_x);
        rotate = xl.rotate;
        let angle = xl.angle.to_radians();
        let xlab_h = xl.height;
        let _ = (xlw, xlh);
        ytb = if show_y {
            TitleBlock::new(&y_title, t.title_pt, ph)
        } else {
            TitleBlock::default()
        };
        if ytb.size < t.title_pt * 0.85 {
            // A short plot: let the title run past the plot area, within the page.
            ytb = TitleBlock::new(&y_title, t.title_pt, h - 2.0 * OUTER);
        }
        xtb = if show_x {
            TitleBlock::new(&x_title, t.title_pt, pw)
        } else {
            TitleBlock::default()
        };
        let ytitle = if ytb.lines.is_empty() {
            0.0
        } else {
            ytb.depth() + TITLE_GAP
        };
        let xtitle = if xtb.lines.is_empty() {
            0.0
        } else {
            xtb.depth() + TITLE_GAP
        };
        left = OUTER + ytitle + ylw + LABEL_GAP + yout + offset;
        if !plot.at_risk.is_empty() {
            left = left.max(OUTER + at_risk_label_w + 14.0);
        }
        if rotate {
            // Rotated labels hang down-left from the first tick.
            let first_w = x
                .ticks
                .labels
                .first()
                .map_or(0.0, |r| text::runs_width(r, fs, Weight::Regular, false));
            let need = first_w * angle.cos_d() - (x.map.map(1.0) - left);
            left = left.max(OUTER + need);
        }
        bottom = bottom_fixed + xtitle + xlab_h + LABEL_GAP + xout + offset;
        // Room for the last x label and the top y label to overhang.
        let last_over = if rotate || xl.lines.is_some() || x.ticks.major.is_empty() || !show_x {
            0.0
        } else {
            let lw = x
                .ticks
                .labels
                .last()
                .map_or(0.0, |r| text::runs_width(r, fs, Weight::Regular, false));
            let px = x.map.map_t(*x.ticks.major.last().expect("tick"));
            (px + lw / 2.0) - (left + pw)
        };
        let _ = xlw;
        let mut y2w = 0.0;
        if let Some(sec) = &plot.y2 {
            let ts = ticks::ticks(
                Scale::Linear,
                sec.range.0,
                sec.range.1,
                4,
                false,
                None,
                0,
                crate::spec::TickFormat::Auto,
                None,
            );
            let (lw2, _) = label_dims(&ts, fs);
            y2w = yout + LABEL_GAP + lw2 + TITLE_GAP + title_h(t.title_pt) + 2.0;
            y2ticks = Some(ts);
        }
        right = right_fixed + last_over.max(0.0).max(y2w);
        if right_fixed <= OUTER + 0.1 && y2w == 0.0 {
            right = right.max(OUTER + fs * 0.6);
        }
        top_m = top + if ylh > 0.0 { ylh / 2.0 } else { 0.0 };
        xa = Some(x);
        ya = Some(y);
    }
    let mut pw = (w - left - right).max(20.0);
    let mut ph = (h - top_m - bottom).max(20.0);
    if let Some(a) = plot.aspect {
        if ph / pw > a {
            let nh = pw * a;
            top_m += (ph - nh) / 2.0;
            ph = nh;
        } else {
            let nw = ph / a;
            left += (pw - nw) / 2.0;
            pw = nw;
        }
    }
    let x = resolve_axis(Side::X, &plot.x, &spec.axes.x, ex, pw, fs, left, left + pw);
    let y = resolve_axis(
        Side::Y,
        &plot.y,
        &spec.axes.y,
        ey,
        ph,
        fs,
        top_m + ph,
        top_m,
    );
    let _ = (xa, ya);
    let area = Rect::new(left, top_m, pw, ph);
    let xl_final = x_labels(spec, &plot.x, &x, fs, pw, show_x);
    let _ = rotate;

    let mut root = Group::plain();
    let mut notes = plot.notes.clone();

    // Gridlines.
    if spec.axes.grid.unwrap_or(t.grid_on) {
        let mut g = Group::new("grid", "/axes/grid");
        let st = Stroke::solid(t.grid, 0.5);
        if !x.categorical {
            for v in &x.ticks.major {
                let px = x.map.map_t(*v);
                g.push(Node::Line {
                    p1: (px, area.y),
                    p2: (px, area.bottom()),
                    stroke: st,
                });
            }
        }
        if !y.categorical {
            for v in &y.ticks.major {
                let py = y.map.map_t(*v);
                g.push(Node::Line {
                    p1: (area.x, py),
                    p2: (area.right(), py),
                    stroke: st,
                });
            }
        }
        root.push(Node::Group(g));
    }

    // Data layers.
    let mapper = Mapper {
        x: &x.map,
        y: &y.map,
        clip: area,
    };
    let mut data_group = Group::new("plot-area", "/mapping");
    let mut placed: Vec<Rect> = Vec::new();
    for l in &plot.layers {
        let mut g = Group::new(l.id.clone(), l.target.clone());
        for m in &l.marks {
            mark_nodes(m, &mapper, &mut g.children, &mut notes, &l.id, &mut placed);
        }
        data_group.push(Node::Group(g));
    }
    root.push(Node::Group(data_group));

    // Frame and axes.
    let frame = plot.frame.or(spec.axes.frame).unwrap_or(t.frame);
    let ax_stroke = Stroke::solid(t.axis, t.axis_pt);
    let (mut x_axis_y, mut y_axis_x) = (area.bottom() + offset, area.x - offset);
    // `crossed_x`: the X axis runs through y = 0; `crossed_y`: the Y axis through x = 0.
    let (mut crossed_x, mut crossed_y) = (false, false);
    if spec.axes.origin == AxisOrigin::Zero {
        if !y.categorical {
            let z = y.map.map(0.0);
            if z.is_finite() && z > area.y + 0.5 && z < area.bottom() - 0.5 {
                x_axis_y = z;
                crossed_x = true;
            }
        }
        if !x.categorical {
            let z = x.map.map(0.0);
            if z.is_finite() && z > area.x + 0.5 && z < area.right() - 0.5 {
                y_axis_x = z;
                crossed_y = true;
            }
        }
    }
    if show_x {
        let mut g = Group::new("axis-x", "/axes/x");
        if !plot.x.bare {
            if frame != FrameStyle::None {
                axis_line(
                    &mut g,
                    &x.map,
                    true,
                    x_axis_y,
                    area.x,
                    area.right(),
                    ax_stroke,
                );
            }
            ticks_draw(&mut g, &x, true, x_axis_y, xdir, tick_len, ax_stroke);
        }
        let mut lg = Group::new("axis-x-labels", "/axes/x/ticks");
        let ly = x_axis_y + xout + LABEL_GAP;
        let angle = xl_final.angle;
        for (i, (v, r)) in x.ticks.major.iter().zip(&x.ticks.labels).enumerate() {
            // Where the axes cross at zero, the 0 label would sit on the other axis.
            if crossed_y && v.abs() < 1e-12 && x.scale != Scale::Categorical {
                continue;
            }
            let px = x.map.map_t(*v);
            if let Some(lines) = xl_final.lines.as_ref().and_then(|l| l.get(i)) {
                for (k, line) in lines.iter().enumerate() {
                    lg.push(Node::Text(runs_node(
                        px,
                        ly + k as f64 * fs * 1.2,
                        line.clone(),
                        fs,
                        t.muted,
                        Anchor::Middle,
                        Baseline::Top,
                        Weight::Regular,
                    )));
                }
                continue;
            }
            let mut tn = runs_node(
                px,
                ly,
                r.clone(),
                fs,
                t.muted,
                Anchor::Middle,
                Baseline::Top,
                Weight::Regular,
            );
            if xl_final.rotate {
                tn.anchor = Anchor::End;
                tn.baseline = Baseline::Middle;
                tn.rotate = -angle;
                tn.x = px + fs * 0.3 * angle.to_radians().sin_d();
                tn.y = ly + fs * 0.1;
            }
            lg.push(Node::Text(tn));
        }
        g.push(Node::Group(lg));
        root.push(Node::Group(g));
    }
    if show_y {
        let mut g = Group::new("axis-y", "/axes/y");
        if !plot.y.bare {
            if frame != FrameStyle::None {
                axis_line(
                    &mut g,
                    &y.map,
                    false,
                    y_axis_x,
                    area.y,
                    area.bottom(),
                    ax_stroke,
                );
            }
            ticks_draw(&mut g, &y, false, y_axis_x, ydir, ytick_len, ax_stroke);
        }
        let mut lg = Group::new("axis-y-labels", "/axes/y/ticks");
        let lx = y_axis_x - yout - LABEL_GAP;
        for (v, r) in y.ticks.major.iter().zip(&y.ticks.labels) {
            if crossed_x && v.abs() < 1e-12 && y.scale != Scale::Categorical {
                continue;
            }
            let py = y.map.map_t(*v);
            lg.push(Node::Text(runs_node(
                lx,
                py,
                r.clone(),
                fs,
                t.muted,
                Anchor::End,
                Baseline::Middle,
                Weight::Regular,
            )));
        }
        g.push(Node::Group(lg));
        root.push(Node::Group(g));
    }
    if frame == FrameStyle::Box {
        let mut g = Group::new("frame", "/axes/frame");
        g.push(Node::Line {
            p1: (area.x, area.y),
            p2: (area.right(), area.y),
            stroke: ax_stroke,
        });
        g.push(Node::Line {
            p1: (area.right(), area.y),
            p2: (area.right(), area.bottom()),
            stroke: ax_stroke,
        });
        root.push(Node::Group(g));
    }

    // Secondary axis (floating, on the right).
    if let (Some(sec), Some(ts)) = (&plot.y2, &y2ticks) {
        let mut g = Group::new("axis-y2", "/axes/y");
        let ax = area.right();
        let vals: Vec<f64> = ts.major.clone();
        let pys: Vec<f64> = vals.iter().map(|v| y.map.map(v + sec.offset)).collect();
        let (lo, hi) = pys
            .iter()
            .fold((f64::MAX, f64::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
        if lo.is_finite() && hi.is_finite() {
            g.push(Node::Line {
                p1: (ax, lo.max(area.y)),
                p2: (ax, hi.min(area.bottom())),
                stroke: ax_stroke,
            });
        }
        let mut lab_w: f64 = 0.0;
        for (py, r) in pys.iter().zip(&ts.labels) {
            if *py < area.y - 0.5 || *py > area.bottom() + 0.5 {
                continue;
            }
            g.push(Node::Line {
                p1: (ax, *py),
                p2: (
                    ax + yout.max(0.0).max(if ydir == TickDirection::In {
                        0.0
                    } else {
                        ytick_len
                    }),
                    *py,
                ),
                stroke: ax_stroke,
            });
            let tn = runs_node(
                ax + ytick_len + LABEL_GAP,
                *py,
                r.clone(),
                fs,
                t.muted,
                Anchor::Start,
                Baseline::Middle,
                Weight::Regular,
            );
            lab_w = lab_w.max(tn.width());
            g.push(Node::Text(tn));
        }
        let tx = ax + ytick_len + LABEL_GAP + lab_w + TITLE_GAP + title_h(t.title_pt) * 0.8;
        let mut tt = text_node(
            tx,
            (lo + hi) / 2.0,
            &sec.title,
            t.title_pt,
            t.text,
            Anchor::Middle,
            Baseline::Alphabetic,
            Weight::Regular,
        );
        tt.rotate = -90.0;
        let mut tg = Group::new("axis-y2-title", "/axes/y/title");
        tg.push(Node::Text(tt));
        g.push(Node::Group(tg));
        root.push(Node::Group(g));
    }

    // Axis titles.
    if !xtb.lines.is_empty() {
        let lab_h = xl_final.height;
        let ty = x_axis_y.max(area.bottom() + offset) + xout + LABEL_GAP + lab_h + TITLE_GAP;
        let mut g = Group::new("axis-x-title", "/axes/x/title");
        for (k, line) in xtb.lines.iter().enumerate() {
            g.push(Node::Text(runs_node(
                area.x + area.width / 2.0,
                ty + k as f64 * xtb.size * 1.2,
                line.clone(),
                xtb.size,
                t.text,
                Anchor::Middle,
                Baseline::Top,
                Weight::Regular,
            )));
        }
        root.push(Node::Group(g));
    }
    if !ytb.lines.is_empty() {
        let (ylw, _) = if show_y {
            label_dims(&y.ticks, fs)
        } else {
            (0.0, 0.0)
        };
        let tx = y_axis_x.min(area.x - offset) - yout - LABEL_GAP - ylw - TITLE_GAP;
        let n = ytb.lines.len();
        let mut g = Group::new("axis-y-title", "/axes/y/title");
        for (k, line) in ytb.lines.iter().enumerate() {
            // Rotated −90°: the baseline is the right edge, descenders face the labels.
            let len = text::runs_width(line, ytb.size, Weight::Regular, false);
            let cy = (area.y + area.height / 2.0).clamp(
                OUTER + len / 2.0,
                (h - OUTER - len / 2.0).max(OUTER + len / 2.0),
            );
            let mut tn = runs_node(
                0.0,
                cy,
                line.clone(),
                ytb.size,
                t.text,
                Anchor::Middle,
                Baseline::Alphabetic,
                Weight::Regular,
            );
            tn.x = tx + vm.descender * ytb.size - (n - 1 - k) as f64 * ytb.size * 1.2;
            tn.rotate = -90.0;
            g.push(Node::Text(tn));
        }
        root.push(Node::Group(g));
    }

    // Breaks: gap glyphs.
    for (axis, map, is_x) in [(&x, &x.map, true), (&y, &y.map, false)] {
        let _ = axis;
        for (a, b) in map.gaps() {
            let mut g = Group::new(
                if is_x { "axis-x-break" } else { "axis-y-break" },
                if is_x {
                    "/axes/x/breaks"
                } else {
                    "/axes/y/breaks"
                },
            );
            let s = Stroke::solid(t.axis, t.axis_pt);
            let k = 3.0;
            for p in [a, b] {
                if is_x {
                    g.push(Node::Line {
                        p1: (p - 1.5, x_axis_y + k),
                        p2: (p + 1.5, x_axis_y - k),
                        stroke: s,
                    });
                } else {
                    g.push(Node::Line {
                        p1: (y_axis_x - k, p + 1.5),
                        p2: (y_axis_x + k, p - 1.5),
                        stroke: s,
                    });
                }
            }
            root.push(Node::Group(g));
        }
    }

    // Colour bar.
    let mut rx = area.right() + 8.0;
    if let Some(d) = &plot.dendro_y {
        let mut g = Group::new("dendrogram-rows", "/options/cluster_rows");
        draw_dendrogram(
            &mut g,
            d,
            &y.map,
            false,
            rx - 4.0,
            dendro_w - 6.0,
            Stroke::solid(t.axis, t.axis_pt * 0.8),
        );
        root.push(Node::Group(g));
        rx += dendro_w;
    }
    if let Some(d) = &plot.dendro_x {
        let mut g = Group::new("dendrogram-columns", "/options/cluster_columns");
        draw_dendrogram(
            &mut g,
            d,
            &x.map,
            true,
            area.y - 4.0,
            dendro_h - 6.0,
            Stroke::solid(t.axis, t.axis_pt * 0.8),
        );
        root.push(Node::Group(g));
    }
    if let Some(rt) = &plot.right_text {
        let mut g = Group::new("estimates", "/options");
        g.push(Node::Text(text_node(
            rx,
            area.y - fs * 0.5,
            &rt.header,
            fs,
            t.text,
            Anchor::Start,
            Baseline::Alphabetic,
            Weight::SemiBold,
        )));
        for (yv, s) in &rt.rows {
            let py = y.map.map(*yv);
            g.push(Node::Text(text_node(
                rx,
                py,
                s,
                fs,
                t.muted,
                Anchor::Start,
                Baseline::Middle,
                Weight::Regular,
            )));
        }
        root.push(Node::Group(g));
        rx += rt_w;
    }
    if let Some(cb) = &plot.colour_bar {
        let mut g = Group::new("colour-bar", "/style/palette");
        let bw = 8.0;
        let top_y = area.y + fs;
        let bh = (area.height - fs).max(20.0);
        let steps = 64;
        for i in 0..steps {
            let f0 = i as f64 / steps as f64;
            let c = cb.ramp.at(1.0 - (i as f64 + 0.5) / steps as f64);
            g.push(Node::Rect {
                rect: Rect::new(rx, top_y + f0 * bh, bw, bh / steps as f64 + 0.05),
                paint: Paint::fill(c),
            });
        }
        g.push(Node::Rect {
            rect: Rect::new(rx, top_y, bw, bh),
            paint: Paint::stroke(Stroke::solid(t.axis, t.axis_pt * 0.6)),
        });
        let ts = ticks::ticks(
            Scale::Linear,
            cb.lo,
            cb.hi,
            5,
            true,
            None,
            0,
            crate::spec::TickFormat::Auto,
            None,
        );
        for (v, r) in ts.major.iter().zip(&ts.labels) {
            let f = (v - cb.lo) / (cb.hi - cb.lo);
            let py = top_y + bh * (1.0 - f);
            g.push(Node::Line {
                p1: (rx + bw, py),
                p2: (rx + bw + 2.5, py),
                stroke: Stroke::solid(t.axis, t.axis_pt * 0.6),
            });
            g.push(Node::Text(runs_node(
                rx + bw + 2.5 + LABEL_GAP,
                py,
                r.clone(),
                fs,
                t.muted,
                Anchor::Start,
                Baseline::Middle,
                Weight::Regular,
            )));
        }
        if !cb.title.is_empty() {
            g.push(Node::Text(text_node(
                rx,
                area.y,
                &cb.title,
                fs,
                t.text,
                Anchor::Start,
                Baseline::Alphabetic,
                Weight::Regular,
            )));
        }
        root.push(Node::Group(g));
        let _ = cb_w;
    }

    // At-risk table.
    if !plot.at_risk.is_empty() {
        let mut g = Group::new("at-risk", "/options/at_risk");
        let x_lab_h = xl_final.height;
        let xt_h = if xtb.lines.is_empty() {
            0.0
        } else {
            xtb.depth() + TITLE_GAP
        };
        let mut ry = area.bottom() + offset + xout + LABEL_GAP + x_lab_h + xt_h + 6.0 + fs;
        g.push(Node::Text(text_node(
            OUTER,
            ry,
            "Number at risk",
            fs,
            t.text,
            Anchor::Start,
            Baseline::Alphabetic,
            Weight::SemiBold,
        )));
        for row in &plot.at_risk {
            ry += fs * 1.45;
            g.push(Node::Text(text_node(
                left - 14.0,
                ry,
                &row.name,
                fs,
                row.colour,
                Anchor::End,
                Baseline::Alphabetic,
                Weight::Regular,
            )));
            let counts: Vec<(f64, usize)> = if row.fixed.is_empty() {
                x.ticks
                    .major
                    .iter()
                    .map(|tk| {
                        let tv = untr(x.scale, *tk);
                        (
                            tv,
                            row.observed.iter().filter(|o| **o >= tv - 1e-12).count(),
                        )
                    })
                    .collect()
            } else {
                row.fixed.clone()
            };
            for (tv, n) in &counts {
                let px = x.map.map(*tv);
                if px >= area.x - 0.5 && px <= area.right() + 0.5 {
                    g.push(Node::Text(text_node(
                        px,
                        ry,
                        &n.to_string(),
                        fs,
                        t.muted,
                        Anchor::Middle,
                        Baseline::Alphabetic,
                        Weight::Regular,
                    )));
                }
            }
        }
        root.push(Node::Group(g));
    }

    // Legend.
    if legend_pos != LegendPosition::Hidden && !plot.legend.is_empty() {
        let inset = 6.0;
        let (lx, ly) = if let Some(at) = spec.legend.at {
            (area.x + at.x * area.width, area.y + at.y * area.height)
        } else {
            match legend_pos {
                LegendPosition::Right => (w - right_fixed + 8.0 + cb_w * 0.0, area.y),
                LegendPosition::Top => (
                    area.x + (area.width - lb.width) / 2.0,
                    top - lb.height - 4.0,
                ),
                LegendPosition::Bottom => (
                    area.x + (area.width - lb.width) / 2.0,
                    h - OUTER - lb.height,
                ),
                LegendPosition::InsideTopLeft => (area.x + inset, area.y + inset * 0.5),
                LegendPosition::InsideTopRight => {
                    (area.right() - lb.width - inset, area.y + inset * 0.5)
                }
                LegendPosition::InsideBottomLeft => {
                    (area.x + inset, area.bottom() - lb.height - inset)
                }
                LegendPosition::InsideBottomRight => (
                    area.right() - lb.width - inset,
                    area.bottom() - lb.height - inset,
                ),
                LegendPosition::Hidden | LegendPosition::Auto => (0.0, 0.0),
            }
        };
        let lx = if legend_pos == LegendPosition::Right && spec.legend.at.is_none() {
            // After the colour bar, dendrogram and text column.
            area.right()
                + 8.0
                + dendro_w
                + rt_w
                + cb_w
                + plot.y2.as_ref().map_or(0.0, |_| right - right_fixed)
        } else {
            lx
        };
        root.push(Node::Group(draw_legend(
            &plot.legend,
            legend_title,
            t,
            lx,
            ly,
            &lb,
        )));
    }

    // Graph title.
    if !htb.lines.is_empty() {
        let mut g = Group::new("title", "/title");
        for (k, line) in htb.lines.iter().enumerate() {
            let tw = text::runs_width(line, htb.size, Weight::SemiBold, false);
            let cx = (area.x + area.width / 2.0).clamp(
                OUTER + tw / 2.0,
                (w - OUTER - tw / 2.0).max(OUTER + tw / 2.0),
            );
            g.push(Node::Text(runs_node(
                cx,
                OUTER + k as f64 * htb.size * 1.2,
                line.clone(),
                htb.size,
                t.text,
                Anchor::Middle,
                Baseline::Top,
                Weight::SemiBold,
            )));
        }
        root.push(Node::Group(g));
    }

    if horizontal {
        swap_targets(&mut root);
    }
    let mut scene = Scene {
        width: w,
        height: h,
        background: if spec.style.transparent {
            None
        } else {
            Some(t.background)
        },
        root,
        notes: Vec::new(),
    };
    scene.notes = notes;
    scene
}

/// Swap X and Y of every mark and axis plan (horizontal graphs). The category axis moves
/// to the left and runs top to bottom.
fn transpose(plot: &mut Plot) {
    let sw = |p: &mut (f64, f64)| *p = (p.1, p.0);
    for l in &mut plot.layers {
        for m in &mut l.marks {
            match m {
                Mark::Points { pts, .. } | Mark::Line { pts, .. } | Mark::Polygon { pts, .. } => {
                    pts.iter_mut().for_each(sw)
                }
                Mark::Band { .. } => {}
                Mark::Rect { x0, x1, y0, y1, .. } | Mark::Hatch { x0, x1, y0, y1, .. } => {
                    std::mem::swap(x0, y0);
                    std::mem::swap(x1, y1);
                }
                Mark::ErrorBar { horizontal, .. } => *horizontal = !*horizontal,
                Mark::Segment { p1, p2, .. } => {
                    sw(p1);
                    sw(p2);
                }
                Mark::HLine { y, stroke } => {
                    *m = Mark::VLine {
                        x: *y,
                        stroke: *stroke,
                    }
                }
                Mark::VLine { x, stroke } => {
                    *m = Mark::HLine {
                        y: *x,
                        stroke: *stroke,
                    }
                }
                Mark::Tick { x, y, .. } => std::mem::swap(x, y),
                Mark::Callouts(list) => {
                    for c in list {
                        std::mem::swap(&mut c.x, &mut c.y);
                    }
                }
                Mark::LineLabel { vertical, .. } => *vertical = !*vertical,
                Mark::Label {
                    x,
                    y,
                    dx,
                    dy,
                    anchor,
                    baseline,
                    ..
                } => {
                    std::mem::swap(x, y);
                    let (a, b) = (*dx, *dy);
                    *dx = -b;
                    *dy = a;
                    if *anchor == Anchor::Middle && *baseline == Baseline::Alphabetic {
                        // A label above a bracket goes to its right.
                        *anchor = Anchor::Start;
                        *baseline = Baseline::Middle;
                    }
                }
            }
        }
    }
    std::mem::swap(&mut plot.x, &mut plot.y);
    if !plot.y.categories.is_empty() {
        plot.y.reverse = true;
    }
}

/// The spec with its X and Y axes exchanged (horizontal graphs).
fn swap_axes(spec: &GraphSpec) -> GraphSpec {
    let mut s = spec.clone();
    std::mem::swap(&mut s.axes.x, &mut s.axes.y);
    s
}

/// Point the axis groups of a horizontal graph at the spec axis they draw.
fn swap_targets(g: &mut Group) {
    if let Some(t) = &g.target {
        let swapped = if let Some(rest) = t.strip_prefix("/axes/x") {
            Some(format!("/axes/y{rest}"))
        } else {
            t.strip_prefix("/axes/y")
                .map(|rest| format!("/axes/x{rest}"))
        };
        if swapped.is_some() {
            g.target = swapped;
        }
    }
    for c in &mut g.children {
        if let Node::Group(cg) = c {
            swap_targets(cg);
        }
    }
}

fn axis_line(g: &mut Group, map: &AxisMap, horizontal: bool, at: f64, a: f64, b: f64, s: Stroke) {
    let gaps = map.gaps();
    let mut start = a.min(b);
    let end = a.max(b);
    let mut cuts: Vec<(f64, f64)> = gaps.iter().map(|(p, q)| (p.min(*q), p.max(*q))).collect();
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
    for (p, q) in cuts.into_iter().chain(std::iter::once((end, end))) {
        let (p1, p2) = if horizontal {
            ((start, at), (p, at))
        } else {
            ((at, start), (at, p))
        };
        if (p - start).abs() > 0.01 {
            g.push(Node::Line {
                p1,
                p2,
                stroke: Stroke { ..s },
            });
        }
        start = q;
    }
    // Extend by half a line width so the corner joins cleanly.
    if let Some(Node::Line { p1, .. }) = g.children.first_mut() {
        if horizontal {
            p1.0 -= s.width / 2.0;
        } else {
            let _ = p1;
        }
    }
    if !horizontal {
        if let Some(Node::Line { p2, .. }) = g.children.last_mut() {
            p2.1 += s.width / 2.0;
        }
    }
}

fn ticks_draw(
    g: &mut Group,
    a: &AxisLayout,
    horizontal: bool,
    at: f64,
    dir: TickDirection,
    len: f64,
    s: Stroke,
) {
    if len <= 0.0 {
        return;
    }
    let add = |v: f64, l: f64, g: &mut Group| {
        let p = a.map.map_t(v);
        let (o, i) = match dir {
            TickDirection::Out => (l, 0.0),
            TickDirection::In => (0.0, l),
            TickDirection::Cross => (l / 2.0, l / 2.0),
        };
        if horizontal {
            g.push(Node::Line {
                p1: (p, at - i),
                p2: (p, at + o),
                stroke: s,
            });
        } else {
            g.push(Node::Line {
                p1: (at - o, p),
                p2: (at + i, p),
                stroke: s,
            });
        }
    };
    for v in &a.ticks.major {
        add(*v, len, g);
    }
    for v in &a.ticks.minor {
        add(*v, len * 0.6, g);
    }
    let _ = a.scale;
}

/// Draw a dendrogram; `horizontal` = leaves along x, growing upward from `base`.
fn draw_dendrogram(
    g: &mut Group,
    d: &Dendrogram,
    map: &AxisMap,
    horizontal: bool,
    base: f64,
    extent: f64,
    s: Stroke,
) {
    let n = d.order.len();
    if n < 2 || d.merges.is_empty() {
        return;
    }
    let mut pos = vec![0.0; n + d.merges.len()];
    let mut height = vec![0.0; n + d.merges.len()];
    for (i, leaf) in d.order.iter().enumerate() {
        pos[*leaf] = map.map(i as f64 + 1.0);
    }
    let hmax = d
        .merges
        .iter()
        .map(|m| m.height)
        .fold(0.0f64, f64::max)
        .max(1e-12);
    for (k, m) in d.merges.iter().enumerate() {
        let id = n + k;
        pos[id] = (pos[m.a] + pos[m.b]) / 2.0;
        height[id] = m.height / hmax * extent;
        let (ha, hb, hm) = (height[m.a], height[m.b], height[id]);
        let pts = if horizontal {
            vec![
                (pos[m.a], base - ha),
                (pos[m.a], base - hm),
                (pos[m.b], base - hm),
                (pos[m.b], base - hb),
            ]
        } else {
            vec![
                (base + ha, pos[m.a]),
                (base + hm, pos[m.a]),
                (base + hm, pos[m.b]),
                (base + hb, pos[m.b]),
            ]
        };
        g.push(Node::Poly {
            points: pts,
            closed: false,
            paint: Paint::stroke(s),
        });
    }
}

/// Turn `SignificanceBracket` annotations into marks, stacking them so none overlap.
#[allow(clippy::needless_range_loop)]
fn add_brackets(ctx: &Ctx<'_>, plot: &mut Plot, sy: Scale, ey: (f64, f64)) {
    let t = &ctx.theme;
    let n = plot.x.categories.len();
    let brackets: Vec<(usize, usize, usize, String, Option<f64>)> = ctx
        .spec
        .annotations
        .iter()
        .enumerate()
        .filter_map(|(i, a)| match a {
            crate::spec::Annotation::SignificanceBracket { from, to, text, y } => {
                Some((i, (*from).min(*to), (*from).max(*to), text.clone(), *y))
            }
            _ => None,
        })
        .collect();
    if brackets.is_empty() || n == 0 {
        return;
    }
    let tops = category_tops(&plot.layers, n, sy);
    let range = if ey.1 > ey.0 { ey.1 - ey.0 } else { 1.0 };
    // One bracket per step: the label above a line plus the end ticks of the next line up.
    let step = range * 0.105;
    let gap = range * 0.05;
    let mut order: Vec<usize> = (0..brackets.len()).collect();
    order.sort_by_key(|i| (brackets[*i].2 - brackets[*i].1, brackets[*i].1));
    let mut placed: Vec<(f64, f64, f64)> = Vec::new(); // (x0, x1, y) in positions/transformed
    let fg = ctx.fg();
    for i in order {
        let (ai, a, b, text, yfix) = &brackets[i];
        let (x0, x1) = (*a as f64 + 1.0, *b as f64 + 1.0);
        let yt = match yfix.map(|v| tr(sy, v)).filter(|v| v.is_finite()) {
            Some(v) => v,
            None => {
                let mut base = f64::NEG_INFINITY;
                for c in (a + 1)..=(b + 1).min(n) {
                    base = base.max(tops[c]);
                }
                if !base.is_finite() {
                    base = ey.1;
                }
                let mut yv = base + gap;
                for (p0, p1, py) in &placed {
                    if *p0 <= x1 + 0.01 && *p1 >= x0 - 0.01 && yv < py + step {
                        yv = py + step;
                    }
                }
                yv
            }
        };
        placed.push((x0, x1, yt));
        let yv = untr(sy, yt);
        let ytick = untr(sy, yt - range * 0.025);
        let st = Stroke::solid(fg, t.axis_pt);
        let mut l = Layer::new(format!("annotation-{ai}"), format!("/annotations/{ai}"));
        l.marks.push(Mark::Line {
            pts: vec![
                (x0 + 0.04, ytick),
                (x0 + 0.04, yv),
                (x1 - 0.04, yv),
                (x1 - 0.04, ytick),
            ],
            stroke: st,
            step: false,
        });
        l.marks.push(Mark::Label {
            x: (x0 + x1) / 2.0,
            y: yv,
            dx: 0.0,
            dy: -1.5,
            runs: text::parse(text),
            size: t.tick_pt,
            color: t.text,
            anchor: Anchor::Middle,
            baseline: Baseline::Alphabetic,
            weight: Weight::Regular,
        });
        // Head room for the label: an extent-only mark (zero width, never drawn).
        l.marks.push(Mark::Segment {
            p1: ((x0 + x1) / 2.0, untr(sy, yt + step * 0.75)),
            p2: ((x0 + x1) / 2.0, untr(sy, yt + step * 0.75)),
            stroke: Stroke::solid(fg.alpha(0.0), 0.0),
        });
        plot.layers.push(l);
    }
}

/// Free annotations (text, lines, reference lines) as layers.
pub fn annotation_layers(ctx: &Ctx<'_>) -> Vec<Layer> {
    let t = &ctx.theme;
    let mut out = Vec::new();
    for (i, a) in ctx.spec.annotations.iter().enumerate() {
        let mut l = Layer::new(format!("annotation-{i}"), format!("/annotations/{i}"));
        match a {
            crate::spec::Annotation::TextLabel {
                text,
                x,
                y,
                size_pt,
            } => l.marks.push(Mark::Label {
                x: *x,
                y: *y,
                dx: 0.0,
                dy: 0.0,
                runs: text::parse(text),
                size: size_pt.unwrap_or(t.tick_pt),
                color: t.text,
                anchor: Anchor::Start,
                baseline: Baseline::Middle,
                weight: Weight::Regular,
            }),
            crate::spec::Annotation::Line {
                x1,
                y1,
                x2,
                y2,
                dashed,
            } => l.marks.push(Mark::Segment {
                p1: (*x1, *y1),
                p2: (*x2, *y2),
                stroke: Stroke::solid(t.axis, t.axis_pt).dashed(if *dashed {
                    Dash::Dashed
                } else {
                    Dash::Solid
                }),
            }),
            crate::spec::Annotation::ReferenceLine {
                axis,
                value,
                label,
                dashed,
            } => {
                let st = Stroke::solid(t.reference, t.axis_pt).dashed(if *dashed {
                    Dash::Dashed
                } else {
                    Dash::Solid
                });
                if axis == "x" {
                    l.marks.push(Mark::VLine {
                        x: *value,
                        stroke: st,
                    });
                } else {
                    l.marks.push(Mark::HLine {
                        y: *value,
                        stroke: st,
                    });
                }
                if let Some(lb) = label {
                    l.marks.push(Mark::LineLabel {
                        vertical: axis == "x",
                        value: *value,
                        runs: text::parse(lb),
                        size: t.tick_pt * 0.9,
                        color: t.muted,
                    });
                }
            }
            crate::spec::Annotation::SignificanceBracket { .. } => continue,
        }
        out.push(l);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks_cut_the_axis() {
        let m = AxisMap::new(Scale::Linear, 0.0, 100.0, 0.0, 105.0, &[(20.0, 80.0)]);
        // 40 data units kept over 100 pt.
        assert!((m.map(20.0) - 50.0).abs() < 1e-9);
        assert!((m.map(80.0) - 55.0).abs() < 1e-9);
        assert!((m.map(100.0) - 105.0).abs() < 1e-9);
        assert!(m.in_gap(50.0));
        assert_eq!(m.gaps(), vec![(50.0, 55.0)]);
    }

    #[test]
    fn clipping() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let s = clip_segment((-5.0, 5.0), (15.0, 5.0), &r).unwrap();
        assert_eq!(s, ((0.0, 5.0), (10.0, 5.0)));
        assert!(clip_segment((-5.0, -5.0), (-1.0, -1.0), &r).is_none());
        let p = clip_polygon(
            &[(-5.0, -5.0), (15.0, -5.0), (15.0, 15.0), (-5.0, 15.0)],
            &r,
        );
        assert_eq!(p.len(), 4);
    }
}
