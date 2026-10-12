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
    /// Fill pattern drawn over bars and boxes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<FillPattern>,
}

/// A fill pattern for bars and boxes, drawn as clipped strokes so SVG, PDF and PNG carry
/// the same geometry (no SVG `<pattern>` element).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillPattern {
    /// No pattern.
    #[default]
    None,
    /// Diagonal lines rising to the right (`/`).
    Hatch,
    /// Diagonal lines falling to the right (`\`).
    BackHatch,
    /// Both diagonals.
    Crosshatch,
    /// Horizontal lines.
    Horizontal,
    /// Vertical lines.
    Vertical,
    /// A grid of dots.
    Dots,
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

/// What connecting lines join on column, strip, box and violin graphs (Prism's
/// before–after lines and "connect means").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Connect {
    /// No connecting lines.
    #[default]
    None,
    /// One line per subject: the values of a row (Prism's convention) joined across the
    /// categories, drawn beneath the points.
    Individuals,
    /// The means joined across the categories, drawn above everything else.
    Means,
    /// The medians joined across the categories.
    Medians,
    /// Subject lines and the summary line (mean, or median when the graph's summary is
    /// the median).
    Both,
}

impl Connect {
    /// Whether subject lines are drawn.
    pub fn individuals(self) -> bool {
        matches!(self, Connect::Individuals | Connect::Both)
    }
}

/// How subject lines are coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectColour {
    /// One colour for every line (`colour`, or the axis colour).
    #[default]
    Single,
    /// Each segment takes the colour of the data set it starts from.
    Dataset,
    /// Each subject (row) its own colour from `palette`.
    Subject,
    /// Subjects coloured by the value of a grouping column (`group_column`), with a legend.
    Group,
}

/// What a missing value does to a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingLines {
    /// Leave a gap where a value is missing.
    #[default]
    Gap,
    /// Join the values on either side of the missing one.
    Skip,
}

/// Which way lines run on grouped graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectDirection {
    /// Within each group (row), across its data sets: subcolumn k of every data set is
    /// subject k.
    #[default]
    WithinGroups,
    /// Across the groups, separately for each data set: Prism's "connect each replicate".
    AcrossGroups,
}

/// Connecting lines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectingLines {
    /// What is joined.
    pub connect: Connect,
    /// Subject line style.
    pub style: LineStyle,
    /// Subject line width in points (`None`: the theme's axis width).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_pt: Option<f64>,
    /// Subject line colouring.
    pub colour_by: ConnectColour,
    /// Colour for `single` (`#rrggbb`; `None`: the axis colour).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// Palette for `subject` and `group` (`None`: the graph's palette).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Table data set holding the subject groups for `group` (column tables).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_column: Option<usize>,
    /// Opacity of subject lines (0–1).
    pub opacity: f64,
    /// Missing values.
    pub missing: MissingLines,
    /// Direction on grouped graphs.
    pub direction: ConnectDirection,
    /// Summary line style.
    pub summary_style: LineStyle,
    /// Summary line width in points (`None`: twice the theme's data width).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_width_pt: Option<f64>,
    /// Summary line colour (`None`: the axis colour, or the data set's on grouped graphs
    /// joined across groups).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_colour: Option<String>,
}

impl Default for ConnectingLines {
    fn default() -> Self {
        ConnectingLines {
            connect: Connect::None,
            style: LineStyle::Solid,
            width_pt: None,
            colour_by: ConnectColour::Single,
            colour: None,
            palette: None,
            group_column: None,
            opacity: 0.55,
            missing: MissingLines::Gap,
            direction: ConnectDirection::WithinGroups,
            summary_style: LineStyle::Solid,
            summary_width_pt: None,
            summary_colour: None,
        }
    }
}

impl ConnectingLines {
    /// True when nothing is drawn (left out of saved specs).
    pub fn is_off(&self) -> bool {
        *self == ConnectingLines::default()
    }
}

/// Which way categorical graphs run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    /// Categories along X, values up the Y axis.
    #[default]
    Vertical,
    /// Categories down the left, values along the bottom. `axes.y` still describes the
    /// value axis and `axes.x` the category axis, as in Prism.
    Horizontal,
}

/// Which half of an error bar is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorDirection {
    /// Both halves (on a dark bar fill only the upper half shows).
    #[default]
    Both,
    /// Away from zero only (above a positive bar).
    Above,
    /// Toward zero only.
    Below,
}

/// How bars are filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarFill {
    /// The data-set colour lightened toward the background (default).
    #[default]
    Tint,
    /// The data-set colour itself.
    Solid,
    /// No fill: the outline only.
    Empty,
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
    /// Connecting lines (column, strip, box, violin).
    #[serde(skip_serializing_if = "ConnectingLines::is_off")]
    pub connecting: ConnectingLines,
    /// Vertical or horizontal (column, strip, box, violin).
    #[serde(skip_serializing_if = "is_default")]
    pub orientation: Orientation,
    /// Error-bar halves drawn (column, strip).
    #[serde(skip_serializing_if = "is_default")]
    pub error_direction: ErrorDirection,
    /// Error-bar cap width in points (`None`: the family's default; 0: no caps).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_pt: Option<f64>,
    /// Bar (or box) width as a fraction of its slot, 0.05–1 (`None`: the family's default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar_width: Option<f64>,
    /// Width of each category's slot as a fraction of the category spacing, 0.2–1; the
    /// rest is the gap between categories or groups (`None`: 0.84).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_width: Option<f64>,
    /// Bar fill.
    #[serde(skip_serializing_if = "is_default")]
    pub bar_fill: BarFill,
    /// Draw the mean (or median) line on strip graphs.
    #[serde(skip_serializing_if = "is_true")]
    pub summary_line: bool,
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
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
            connecting: ConnectingLines::default(),
            orientation: Orientation::Vertical,
            error_direction: ErrorDirection::Both,
            cap_pt: None,
            bar_width: None,
            group_width: None,
            bar_fill: BarFill::Tint,
            summary_line: true,
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
    ///
    /// Two presets are accepted as well as family names: `paired` (bars of the mean with
    /// the points and a line per subject — the default graph of paired and repeated-measures
    /// analyses) and `before_after` (points joined per subject with the mean line, no bars:
    /// Prism's before–after graph). Both become ordinary specs of the `column` and `strip`
    /// families with connecting lines.
    pub fn new(source: Id, family: &str) -> Self {
        match family {
            "paired" => {
                let mut s = GraphSpec::base(source, "column");
                s.options.connecting.connect = Connect::Individuals;
                s.options.jitter = 0.0;
                s
            }
            "before_after" => {
                let mut s = GraphSpec::base(source, "before_after");
                s.upgrade_legacy();
                s
            }
            _ => GraphSpec::base(source, family),
        }
    }

    fn base(source: Id, family: &str) -> Self {
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

    /// Rewrite a spec of the retired `before_after` family (file format 1.0) as the `strip`
    /// spec that draws the same graph: unjittered points, a line per subject, the mean (or
    /// median) line when `mean_line` was on, no error bars and no per-category summary tick.
    /// Returns whether anything changed. Specs of other families are left alone.
    pub fn upgrade_legacy(&mut self) -> bool {
        if self.family != "before_after" {
            return false;
        }
        self.family = "strip".into();
        self.mapping.points = true;
        self.mapping.error_bars = ErrorBars::None;
        let o = &mut self.options;
        o.jitter = 0.0;
        o.summary_line = false;
        o.connecting.connect = if o.mean_line {
            Connect::Both
        } else {
            Connect::Individuals
        };
        o.connecting.opacity = 0.4;
        true
    }

    /// Style override for a dataset, if any.
    pub fn dataset_style(&self, dataset: usize) -> Option<&DatasetStyle> {
        self.style.datasets.iter().find(|d| d.dataset == dataset)
    }
}
