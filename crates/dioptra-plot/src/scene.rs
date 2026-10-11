//! The device-independent scene: a tree of groups and shapes in points (1/72 inch), with
//! the origin at the top left. Groups that stand for a spec element carry an `id` and a
//! `target` (a JSON pointer into the [`crate::spec::GraphSpec`]) for the graph editor.

use crate::detmath::DetMath;
use serde::{Deserialize, Serialize};

use crate::text::{Run, Weight};

/// An sRGB colour with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 = opaque).
    pub a: u8,
}

impl Color {
    /// Opaque colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// Parse `#rgb`, `#rrggbb` or `#rrggbbaa`.
    pub fn hex(s: &str) -> Option<Color> {
        let h = s.trim().strip_prefix('#')?;
        let p = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
        match h.len() {
            3 => Some(Color::rgb(p(0, 1)? * 17, p(1, 1)? * 17, p(2, 1)? * 17)),
            6 => Some(Color::rgb(p(0, 2)?, p(2, 2)?, p(4, 2)?)),
            8 => Some(Color {
                a: p(6, 2)?,
                ..Color::rgb(p(0, 2)?, p(2, 2)?, p(4, 2)?)
            }),
            _ => None,
        }
    }

    /// `#rrggbb` (alpha is written separately as an opacity).
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Same colour with alpha `a` (0–1).
    pub fn alpha(self, a: f64) -> Color {
        Color {
            a: (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }

    /// Linear mix toward `o` by `t` (0 = self, 1 = o), opaque.
    pub fn mix(self, o: Color, t: f64) -> Color {
        let f = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
        Color::rgb(f(self.r, o.r), f(self.g, o.g), f(self.b, o.b))
    }

    /// Relative luminance (WCAG).
    pub fn luminance(self) -> f64 {
        let c = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf_d(2.4)
            }
        };
        0.2126 * c(self.r) + 0.7152 * c(self.g) + 0.0722 * c(self.b)
    }
}

/// Dash pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dash {
    /// Solid.
    #[default]
    Solid,
    /// Dashed.
    Dashed,
    /// Dotted.
    Dotted,
    /// Dash-dot.
    DashDot,
}

/// A stroke.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    /// Colour.
    pub color: Color,
    /// Width in points.
    pub width: f64,
    /// Dash pattern (scaled by the width).
    pub dash: Dash,
}

impl Stroke {
    /// Solid stroke.
    pub fn solid(color: Color, width: f64) -> Self {
        Stroke {
            color,
            width,
            dash: Dash::Solid,
        }
    }

    /// With a dash pattern.
    pub fn dashed(self, dash: Dash) -> Self {
        Stroke { dash, ..self }
    }
}

/// Fill and stroke of a shape.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Paint {
    /// Fill (`None`: no fill).
    pub fill: Option<Color>,
    /// Stroke (`None`: no stroke).
    pub stroke: Option<Stroke>,
}

impl Paint {
    /// Fill only.
    pub fn fill(c: Color) -> Self {
        Paint {
            fill: Some(c),
            stroke: None,
        }
    }

    /// Stroke only.
    pub fn stroke(s: Stroke) -> Self {
        Paint {
            fill: None,
            stroke: Some(s),
        }
    }

    /// Both.
    pub fn both(c: Color, s: Stroke) -> Self {
        Paint {
            fill: Some(c),
            stroke: Some(s),
        }
    }
}

/// A rectangle in points.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Rect {
    /// New rectangle.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge.
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Union of two rectangles.
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(
            x,
            y,
            self.right().max(o.right()) - x,
            self.bottom().max(o.bottom()) - y,
        )
    }

    /// Whether two rectangles overlap by more than `tol` points in both directions.
    pub fn overlaps(&self, o: &Rect, tol: f64) -> bool {
        self.x + tol < o.right()
            && o.x + tol < self.right()
            && self.y + tol < o.bottom()
            && o.y + tol < self.bottom()
    }

    /// Grow by `d` on every side.
    pub fn inflate(&self, d: f64) -> Rect {
        Rect::new(
            self.x - d,
            self.y - d,
            self.width + 2.0 * d,
            self.height + 2.0 * d,
        )
    }
}

/// Marker shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkerShape {
    /// Circle.
    #[default]
    Circle,
    /// Square.
    Square,
    /// Triangle pointing up.
    TriangleUp,
    /// Triangle pointing down.
    TriangleDown,
    /// Diamond.
    Diamond,
    /// Plus.
    Plus,
    /// Cross (×).
    Cross,
    /// A short horizontal bar.
    Bar,
    /// A short vertical tick.
    Tick,
}

impl MarkerShape {
    /// Parse a marker name; returns the shape and whether it is open (unfilled).
    /// `None` for `none`.
    pub fn parse(name: &str) -> Option<(MarkerShape, bool)> {
        let (base, open) = match name.strip_suffix("_open") {
            Some(b) => (b, true),
            None => (name, false),
        };
        let s = match base {
            "circle" => MarkerShape::Circle,
            "square" => MarkerShape::Square,
            "triangle" | "triangle_up" => MarkerShape::TriangleUp,
            "triangle_down" => MarkerShape::TriangleDown,
            "diamond" => MarkerShape::Diamond,
            "plus" => MarkerShape::Plus,
            "cross" => MarkerShape::Cross,
            "none" => return None,
            _ => MarkerShape::Circle,
        };
        Some((s, open))
    }

    /// Whether the shape is drawn with strokes only.
    pub fn is_line(self) -> bool {
        matches!(
            self,
            MarkerShape::Plus | MarkerShape::Cross | MarkerShape::Bar | MarkerShape::Tick
        )
    }
}

/// Horizontal text anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    /// Left.
    #[default]
    Start,
    /// Centre.
    Middle,
    /// Right.
    End,
}

/// Vertical placement of text relative to its `y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Baseline {
    /// `y` is the alphabetic baseline.
    #[default]
    Alphabetic,
    /// `y` is the middle of capital letters.
    Middle,
    /// `y` is the top of capital letters.
    Top,
    /// `y` is the bottom of descenders.
    Bottom,
}

/// A text node. The writer converts the baseline to an alphabetic `y`, so no SVG
/// baseline attributes are needed.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    /// Anchor point x.
    pub x: f64,
    /// Anchor point y.
    pub y: f64,
    /// Runs.
    pub runs: Vec<Run>,
    /// Size in points.
    pub size: f64,
    /// Weight.
    pub weight: Weight,
    /// Italic.
    pub italic: bool,
    /// Colour.
    pub color: Color,
    /// Horizontal anchor.
    pub anchor: Anchor,
    /// Vertical placement.
    pub baseline: Baseline,
    /// Rotation in degrees about (x, y), counter-clockwise negative as in SVG.
    pub rotate: f64,
}

impl Text {
    /// Width in points.
    pub fn width(&self) -> f64 {
        crate::text::runs_width(&self.runs, self.size, self.weight, self.italic)
    }

    /// Alphabetic baseline y for the chosen placement (unrotated frame).
    pub fn baseline_y(&self) -> f64 {
        let m = crate::text::vmetrics();
        match self.baseline {
            Baseline::Alphabetic => self.y,
            Baseline::Middle => self.y + m.cap_height * self.size / 2.0,
            Baseline::Top => self.y + m.cap_height * self.size,
            Baseline::Bottom => self.y + m.descender * self.size,
        }
    }

    /// Bounding box (cap height to descender, plus scripts) in scene coordinates.
    pub fn bbox(&self) -> Rect {
        let m = crate::text::vmetrics();
        let w = self.width();
        let x0 = match self.anchor {
            Anchor::Start => self.x,
            Anchor::Middle => self.x - w / 2.0,
            Anchor::End => self.x - w,
        };
        let by = self.baseline_y();
        let has_super = self
            .runs
            .iter()
            .any(|r| r.shift == crate::text::Shift::Super);
        let has_sub = self.runs.iter().any(|r| r.shift == crate::text::Shift::Sub);
        let top = by
            - if has_super {
                (crate::text::SUPER_RISE + m.cap_height * crate::text::SCRIPT_SCALE)
                    .max(m.cap_height)
                    * self.size
            } else {
                m.cap_height * self.size
            };
        let bottom = by - m.descender * self.size
            + if has_sub {
                crate::text::SUB_DROP * self.size
            } else {
                0.0
            };
        let r = Rect::new(x0, top, w, bottom - top);
        if self.rotate == 0.0 {
            return r;
        }
        // Rotate the four corners about the anchor point.
        let (s, c) = self.rotate.to_radians().sin_cos_d();
        let pts = [
            (r.x, r.y),
            (r.right(), r.y),
            (r.x, r.bottom()),
            (r.right(), r.bottom()),
        ];
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (px, py) in pts {
            let (dx, dy) = (px - self.x, py - self.y);
            let (rx, ry) = (self.x + dx * c - dy * s, self.y + dx * s + dy * c);
            x0 = x0.min(rx);
            y0 = y0.min(ry);
            x1 = x1.max(rx);
            y1 = y1.max(ry);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// Path commands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCmd {
    /// Move to.
    M(f64, f64),
    /// Line to.
    L(f64, f64),
    /// Cubic Bézier to.
    C(f64, f64, f64, f64, f64, f64),
    /// Elliptical arc to (rx, ry, large-arc, sweep, x, y).
    A(f64, f64, bool, bool, f64, f64),
    /// Close.
    Z,
}

/// A scene node.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A group.
    Group(Group),
    /// A line segment.
    Line {
        /// Start.
        p1: (f64, f64),
        /// End.
        p2: (f64, f64),
        /// Stroke.
        stroke: Stroke,
    },
    /// An open polyline (or a closed polygon when `closed`).
    Poly {
        /// Points.
        points: Vec<(f64, f64)>,
        /// Closed.
        closed: bool,
        /// Paint.
        paint: Paint,
    },
    /// A rectangle.
    Rect {
        /// Geometry.
        rect: Rect,
        /// Paint.
        paint: Paint,
    },
    /// A general path.
    Path {
        /// Commands.
        cmds: Vec<PathCmd>,
        /// Paint.
        paint: Paint,
    },
    /// Many markers of one shape and paint (written compactly).
    Markers {
        /// Shape.
        shape: MarkerShape,
        /// Diameter in points.
        size: f64,
        /// Paint.
        paint: Paint,
        /// Centres.
        points: Vec<(f64, f64)>,
    },
    /// Text.
    Text(Text),
    /// A comment written into the SVG (e.g. level-of-detail notes).
    Comment(String),
}

/// A group of nodes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Group {
    /// Element id (`axis-x`, `series-2`, …).
    pub id: Option<String>,
    /// JSON pointer into the spec (`/axes/x/title`).
    pub target: Option<String>,
    /// Translation applied to the children.
    pub translate: Option<(f64, f64)>,
    /// Clip rectangle in the group's coordinates (rarely used: the engine clips geometry
    /// itself so exported files carry no clipping masks).
    pub clip: Option<Rect>,
    /// Children.
    pub children: Vec<Node>,
}

impl Group {
    /// An identified group.
    pub fn new(id: impl Into<String>, target: impl Into<String>) -> Self {
        Group {
            id: Some(id.into()),
            target: Some(target.into()),
            ..Default::default()
        }
    }

    /// An anonymous group.
    pub fn plain() -> Self {
        Group::default()
    }

    /// Add a node.
    pub fn push(&mut self, n: Node) {
        self.children.push(n);
    }

    /// Bounding box of the children (in the parent's coordinates).
    pub fn bbox(&self) -> Option<Rect> {
        let mut acc: Option<Rect> = None;
        for c in &self.children {
            if let Some(b) = node_bbox(c) {
                acc = Some(match acc {
                    Some(a) => a.union(&b),
                    None => b,
                });
            }
        }
        let (dx, dy) = self.translate.unwrap_or((0.0, 0.0));
        acc.map(|r| Rect::new(r.x + dx, r.y + dy, r.width, r.height))
    }
}

fn pts_bbox(pts: impl Iterator<Item = (f64, f64)>, pad: f64) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut any = false;
    for (x, y) in pts {
        any = true;
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    any.then(|| Rect::new(x0 - pad, y0 - pad, x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad))
}

/// Bounding box of a node.
pub fn node_bbox(n: &Node) -> Option<Rect> {
    match n {
        Node::Group(g) => g.bbox(),
        Node::Line { p1, p2, stroke } => pts_bbox([*p1, *p2].into_iter(), stroke.width / 2.0),
        Node::Poly { points, paint, .. } => pts_bbox(
            points.iter().copied(),
            paint.stroke.map_or(0.0, |s| s.width / 2.0),
        ),
        Node::Rect { rect, .. } => Some(*rect),
        Node::Path { cmds, .. } => pts_bbox(
            cmds.iter().filter_map(|c| match c {
                PathCmd::M(x, y) | PathCmd::L(x, y) => Some((*x, *y)),
                PathCmd::C(_, _, _, _, x, y) => Some((*x, *y)),
                PathCmd::A(_, _, _, _, x, y) => Some((*x, *y)),
                PathCmd::Z => None,
            }),
            0.0,
        ),
        Node::Markers { size, points, .. } => pts_bbox(points.iter().copied(), size / 2.0),
        Node::Text(t) => Some(t.bbox()),
        Node::Comment(_) => None,
    }
}

/// A clickable element for the graph editor: its id, the spec field it edits, and its
/// bounding box in points (scene coordinates; multiply by the SVG's px-per-pt to place
/// selection boxes over the displayed image).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// Element id in the SVG.
    pub id: String,
    /// JSON pointer into the graph spec.
    pub target: String,
    /// Bounding box in points.
    pub bbox: Rect,
}

/// A complete drawing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    /// Width in points.
    pub width: f64,
    /// Height in points.
    pub height: f64,
    /// Background (`None`: transparent).
    pub background: Option<Color>,
    /// Content.
    pub root: Group,
    /// Notes written as comments at the top of the SVG.
    pub notes: Vec<String>,
}

impl Scene {
    /// Width in millimetres.
    pub fn width_mm(&self) -> f64 {
        self.width * 25.4 / 72.0
    }

    /// Height in millimetres.
    pub fn height_mm(&self) -> f64 {
        self.height * 25.4 / 72.0
    }

    /// Every identified group with its target and bounding box, in document order.
    pub fn targets(&self) -> Vec<Target> {
        let mut out = Vec::new();
        collect_targets(&self.root, (0.0, 0.0), &mut out);
        out
    }
}

fn collect_targets(g: &Group, off: (f64, f64), out: &mut Vec<Target>) {
    let (dx, dy) = g.translate.unwrap_or((0.0, 0.0));
    let here = (off.0 + dx, off.1 + dy);
    if let (Some(id), Some(t)) = (&g.id, &g.target) {
        if let Some(b) = g.bbox() {
            out.push(Target {
                id: id.clone(),
                target: t.clone(),
                bbox: Rect::new(b.x + off.0, b.y + off.1, b.width, b.height),
            });
        }
    }
    for c in &g.children {
        if let Node::Group(cg) = c {
            collect_targets(cg, here, out);
        }
    }
}
