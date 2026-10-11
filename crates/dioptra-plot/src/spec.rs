//! Graph specifications (DESIGN §4.4, §6, §7).
//!
//! These types are the JSON contract with the UI: `dioptra-core` re-exports them, the
//! inspector edits them through RFC 7396 merge patches, and every field has a default so
//! a partial spec deserialises. Optional style fields left out mean "the theme decides".

use serde::{Deserialize, Serialize};

/// Opaque object identifier (same as `dioptra_core::api::Id`).
pub type Id = String;

/// Physical size in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SizeMm {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Default for SizeMm {
    fn default() -> Self {
        SizeMm {
            width: 85.0,
            height: 70.0,
        }
    }
}

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
    /// Categories at positions 1, 2, … (chosen automatically when the data are categorical).
    Categorical,
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

/// How tick labels are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickFormat {
    /// Decimal, switching to scientific for |x| ≥ 10⁶ or ≤ 10⁻⁴; log axes as powers.
    #[default]
    Auto,
    /// Always decimal (1, 10, 100 on log axes).
    Decimal,
    /// Always scientific (`2×10³`).
    Scientific,
    /// Powers of the base (`10³`); only meaningful on log axes.
    Power,
}

/// Tick settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ticks {
    /// Major tick interval in data units (`None`: automatic). On log axes, the interval in
    /// decades (or powers of two).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major: Option<f64>,
    /// Minor ticks between majors (log axes: any value above 0 draws 2…9 per decade).
    pub minor: u32,
    /// Direction.
    pub direction: TickDirection,
    /// Major tick length in points (minor ticks are 60% of this).
    pub length_pt: f64,
    /// Label format.
    pub format: TickFormat,
    /// Fixed decimals for labels (`None`: from the tick interval).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    /// Rotation of tick labels in degrees (`None`: 0, or 45 when categorical labels collide).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_angle: Option<f64>,
}

impl Default for Ticks {
    fn default() -> Self {
        Ticks {
            major: None,
            minor: 0,
            direction: TickDirection::Out,
            length_pt: 3.0,
            format: TickFormat::Auto,
            decimals: None,
            label_angle: None,
        }
    }
}

/// A gap in an axis: values between `from` and `to` are cut out and a gap glyph drawn.
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
    /// Title (`^`/`_` for super/subscripts, `\alpha` for Greek). `None`: from the data.
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
    /// Hide the axis line, ticks and labels.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// Which sides of the plot area carry a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameStyle {
    /// Left and bottom axes only.
    L,
    /// A closed box.
    Box,
    /// No axis lines (ticks and labels stay).
    None,
}

/// Where the axes cross.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisOrigin {
    /// At the edges of the plot area (default).
    #[default]
    Frame,
    /// At zero of the other axis, when zero is in range.
    Zero,
}

/// Both axes and the frame.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Axes {
    /// Horizontal.
    pub x: Axis,
    /// Vertical.
    pub y: Axis,
    /// Frame style (`None`: theme default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameStyle>,
    /// Where the axes cross.
    pub origin: AxisOrigin,
    /// Major gridlines (`None`: theme default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<bool>,
    /// Offset of the axes away from the plot area in points (`None`: theme default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_pt: Option<f64>,
}

/// Legend position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegendPosition {
    /// The family decides (default): right of the plot for most families, inside the
    /// plot area for survival and ROC curves, above a volcano plot.
    #[default]
    Auto,
    /// No legend.
    Hidden,
    /// Right of the plot.
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

/// A manual legend position: the legend's top-left corner as fractions of the plot area
/// (0, 0 = top left of the plot area; 1, 1 = bottom right). Set by dragging.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LegendAt {
    /// Horizontal fraction.
    pub x: f64,
    /// Vertical fraction.
    pub y: f64,
}

/// Legend.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Legend {
    /// Position.
    pub position: LegendPosition,
    /// Title above the entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Manual placement; overrides `position` when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<LegendAt>,
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
    /// Dash-dot.
    DashDot,
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
    /// Marker: `circle`, `square`, `triangle`, `triangle_down`, `diamond`, `plus`,
    /// `cross` or `none`; append `_open` for an unfilled marker (`circle_open`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    /// Marker size (diameter) in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_pt: Option<f64>,
    /// Line style.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_style: Option<LineStyle>,
    /// Line width in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_pt: Option<f64>,
    /// Legend label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Visual style of a graph. Fields left out are supplied by the theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphStyle {
    /// Theme: `instrument`, `night` or `journal`.
    pub theme: String,
    /// Palette: `okabe_ito`, `tol_bright`, `tol_muted`, `tol_light`, `viridis`, `cividis`,
    /// `tol_sunset`, `grey` (`None`: theme default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Size in millimetres.
    pub size_mm: SizeMm,
    /// Tick-label font size in points (titles are one point larger).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_pt: Option<f64>,
    /// Axis line width in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_pt: Option<f64>,
    /// Default marker diameter in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_pt: Option<f64>,
    /// Transparent background (PNG and SVG); the theme colour otherwise.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub transparent: bool,
    /// Per-dataset overrides.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<DatasetStyle>,
}

impl Default for GraphStyle {
    fn default() -> Self {
        GraphStyle {
            theme: "instrument".into(),
            palette: None,
            size_mm: SizeMm::default(),
            font_pt: None,
            line_pt: None,
            marker_pt: None,
            transparent: false,
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
    /// 95% confidence interval of the mean (t distribution).
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
    /// Flat column holding point labels (volcano, forest), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<usize>,
    /// Error bars for summary families.
    pub error_bars: ErrorBars,
    /// Show individual points over summaries.
    pub points: bool,
}

/// Summary drawn by column, strip and estimation families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Summary {
    /// Arithmetic mean.
    #[default]
    Mean,
    /// Median.
    Median,
}

/// How several datasets share a category in column graphs of grouped tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrangement {
    /// Side by side.
    #[default]
    Grouped,
    /// Stacked.
    Stacked,
    /// Stacked and scaled to 100%.
    Percent,
}

/// What a violin carries inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolinInner {
    /// A slim box with the median as a dot.
    #[default]
    Box,
    /// The data points.
    Strip,
    /// Dashed quartile lines.
    Quartiles,
    /// Nothing.
    None,
}

/// Family-specific options. Every field has a default; families ignore what they do not use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphOptions {
    /// Summary statistic (column, strip, before–after mean line).
    pub summary: Summary,
    /// Grouped, stacked or percent-stacked columns.
    pub arrangement: Arrangement,
    /// Join XY points with a line (scatter).
    pub connect: bool,
    /// Notched boxes.
    pub notch: bool,
    /// Violin interior.
    pub violin_inner: ViolinInner,
    /// Cut violins and densities at the data range.
    pub truncate: bool,
    /// KDE bandwidth in data units (`None`: Silverman's rule).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth: Option<f64>,
    /// Histogram bin width (`None`: Freedman–Diaconis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_width: Option<f64>,
    /// Histogram bin count (used when `bin_width` is `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bins: Option<usize>,
    /// Histogram shows density instead of counts.
    pub density: bool,
    /// Jitter width as a fraction of the category slot (0–1).
    pub jitter: f64,
    /// Draw the mean (or median) line on before–after graphs.
    pub mean_line: bool,
    /// Bootstrap resamples (estimation).
    pub bootstrap: usize,
    /// Seed for bootstrap and jitter.
    pub seed: u64,
    /// Cluster heatmap rows (average linkage, Euclidean).
    pub cluster_rows: bool,
    /// Cluster heatmap columns.
    pub cluster_columns: bool,
    /// Print values in heatmap cells.
    pub cell_values: bool,
    /// Survival CI bands.
    pub ci_band: bool,
    /// Survival censor ticks.
    pub censor_ticks: bool,
    /// Survival number-at-risk table.
    pub at_risk: bool,
    /// Volcano: |log2 fold change| threshold.
    pub fc_threshold: f64,
    /// Volcano: p-value threshold.
    pub p_threshold: f64,
    /// Volcano: label this many most significant points (needs labels).
    pub label_top: usize,
    /// Forest: reference line (`None`: 0 on linear axes, 1 on log axes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<f64>,
    /// Donut hole as a fraction of the radius.
    pub donut_hole: f64,
}

impl Default for GraphOptions {
    fn default() -> Self {
        GraphOptions {
            summary: Summary::Mean,
            arrangement: Arrangement::Grouped,
            connect: false,
            notch: false,
            violin_inner: ViolinInner::Box,
            truncate: true,
            bandwidth: None,
            bin_width: None,
            bins: None,
            density: false,
            jitter: 0.5,
            mean_line: true,
            bootstrap: 5000,
            seed: 20_261_011,
            cluster_rows: false,
            cluster_columns: false,
            cell_values: false,
            ci_band: true,
            censor_ticks: true,
            at_risk: true,
            fc_threshold: 1.0,
            p_threshold: 0.05,
            label_top: 5,
            reference: None,
            donut_hole: 0.55,
        }
    }
}

/// An annotation drawn on the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Annotation {
    /// A bracket between two categories (0-based) with a label (asterisks or a p value).
    /// Brackets that would overlap are stacked automatically.
    SignificanceBracket {
        /// First category.
        from: usize,
        /// Second category.
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
    /// A line across the whole plot at a value of one axis.
    ReferenceLine {
        /// `x` (vertical line) or `y` (horizontal line).
        axis: String,
        /// Value in data units.
        value: f64,
        /// Optional label at the end of the line.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// Dashed (default true).
        #[serde(default = "yes")]
        dashed: bool,
    },
}

fn yes() -> bool {
    true
}

/// A graph specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphSpec {
    /// Source table or analysis.
    pub source: Id,
    /// Family name (see [`crate::FAMILIES`]), e.g. `scatter`, `column`, `box`.
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
    /// Family options.
    #[serde(default)]
    pub options: GraphOptions,
    /// Annotations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

impl GraphSpec {
    /// Defaults for a family: individual points are shown over summaries (column graphs
    /// show their points); box plots show outliers only and violins carry a slim box.
    pub fn new(source: Id, family: &str) -> Self {
        GraphSpec {
            source,
            family: family.to_string(),
            title: None,
            mapping: Mapping {
                points: !matches!(family, "box" | "raincloud" | "violin"),
                ..Default::default()
            },
            style: GraphStyle::default(),
            axes: Axes::default(),
            legend: Legend::default(),
            options: GraphOptions::default(),
            annotations: Vec::new(),
        }
    }

    /// Style override for a dataset, if any.
    pub fn dataset_style(&self, dataset: usize) -> Option<&DatasetStyle> {
        self.style.datasets.iter().find(|d| d.dataset == dataset)
    }
}
