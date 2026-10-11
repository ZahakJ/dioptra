//! Themes and palettes (DESIGN §6). A theme supplies defaults; the spec's style wins.

use crate::scene::{Color, MarkerShape};
use crate::spec::{FrameStyle, GraphSpec, TickDirection};

/// Resolved visual parameters for one graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Theme name.
    pub name: &'static str,
    /// Page background.
    pub background: Color,
    /// Axis lines and ticks.
    pub axis: Color,
    /// Text.
    pub text: Color,
    /// Secondary text (tick labels, at-risk numbers).
    pub muted: Color,
    /// Gridlines.
    pub grid: Color,
    /// Reference and threshold lines.
    pub reference: Color,
    /// Axis line width (pt).
    pub axis_pt: f64,
    /// Data line width (pt).
    pub data_pt: f64,
    /// Tick-label size (pt).
    pub tick_pt: f64,
    /// Axis-title size (pt).
    pub title_pt: f64,
    /// Graph-title size (pt).
    pub heading_pt: f64,
    /// Default marker diameter (pt).
    pub marker_pt: f64,
    /// Default frame.
    pub frame: FrameStyle,
    /// Default gridlines.
    pub grid_on: bool,
    /// Default tick direction.
    pub ticks: TickDirection,
    /// Default palette name.
    pub palette: &'static str,
    /// Black-and-white: series are told apart by marker shape, fill and dash.
    pub mono: bool,
    /// How far summary fills are lightened toward the background (0–1).
    pub tint: f64,
    /// Axis offset from the plot area (pt).
    pub offset_pt: f64,
}

impl Theme {
    /// A named theme (`instrument`, `night`, `journal`; unknown names give `instrument`).
    pub fn named(name: &str) -> Theme {
        match name {
            "night" => Theme {
                name: "night",
                background: Color::rgb(0x16, 0x19, 0x1a),
                axis: Color::rgb(0xc9, 0xcd, 0xca),
                text: Color::rgb(0xe8, 0xea, 0xe8),
                muted: Color::rgb(0xc9, 0xcd, 0xca),
                grid: Color::rgb(0x2e, 0x33, 0x34),
                reference: Color::rgb(0x8a, 0x90, 0x8d),
                axis_pt: 0.75,
                data_pt: 1.0,
                tick_pt: 8.0,
                title_pt: 9.0,
                heading_pt: 10.0,
                marker_pt: 4.0,
                frame: FrameStyle::L,
                grid_on: false,
                ticks: TickDirection::Out,
                palette: "okabe_ito",
                mono: false,
                tint: 0.55,
                offset_pt: 0.0,
            },
            "journal" => Theme {
                name: "journal",
                background: Color::rgb(255, 255, 255),
                axis: Color::rgb(0, 0, 0),
                text: Color::rgb(0, 0, 0),
                muted: Color::rgb(0, 0, 0),
                grid: Color::rgb(0xdd, 0xdd, 0xdd),
                reference: Color::rgb(0x55, 0x55, 0x55),
                axis_pt: 0.5,
                data_pt: 0.75,
                tick_pt: 7.0,
                title_pt: 8.0,
                heading_pt: 9.0,
                marker_pt: 3.5,
                frame: FrameStyle::L,
                grid_on: false,
                ticks: TickDirection::Out,
                palette: "grey",
                mono: true,
                tint: 1.0,
                offset_pt: 0.0,
            },
            _ => Theme {
                name: "instrument",
                background: Color::rgb(255, 255, 255),
                axis: Color::rgb(0x22, 0x22, 0x22),
                text: Color::rgb(0x22, 0x22, 0x22),
                muted: Color::rgb(0x22, 0x22, 0x22),
                grid: Color::rgb(0xe6, 0xe4, 0xe0),
                reference: Color::rgb(0x77, 0x77, 0x77),
                axis_pt: 0.75,
                data_pt: 1.0,
                tick_pt: 8.0,
                title_pt: 9.0,
                heading_pt: 10.0,
                marker_pt: 4.0,
                frame: FrameStyle::L,
                grid_on: false,
                ticks: TickDirection::Out,
                palette: "okabe_ito",
                mono: false,
                tint: 0.7,
                offset_pt: 0.0,
            },
        }
    }

    /// The theme for a spec with its style overrides applied.
    pub fn for_spec(spec: &GraphSpec) -> Theme {
        let mut t = Theme::named(&spec.style.theme);
        if let Some(f) = spec.style.font_pt.filter(|f| *f > 0.0) {
            let d = f - t.tick_pt;
            t.tick_pt = f;
            t.title_pt += d;
            t.heading_pt += d;
        }
        if let Some(l) = spec.style.line_pt.filter(|l| *l > 0.0) {
            t.data_pt *= l / t.axis_pt;
            t.axis_pt = l;
        }
        if let Some(m) = spec.style.marker_pt.filter(|m| *m > 0.0) {
            t.marker_pt = m;
        }
        t
    }
}

/// Palette names in display order.
pub const PALETTES: [&str; 8] = [
    "okabe_ito",
    "tol_bright",
    "tol_muted",
    "tol_light",
    "viridis",
    "cividis",
    "tol_sunset",
    "grey",
];

fn hexes(v: &[&str]) -> Vec<Color> {
    v.iter()
        .map(|h| Color::hex(h).expect("palette colour"))
        .collect()
}

/// Okabe & Ito (2008), ordered for white paper: black first, yellow last.
pub const OKABE_ITO: [&str; 8] = [
    "#000000", "#E69F00", "#56B4E9", "#009E73", "#0072B2", "#D55E00", "#CC79A7", "#F0E442",
];
/// Paul Tol's bright scheme.
pub const TOL_BRIGHT: [&str; 7] = [
    "#4477AA", "#EE6677", "#228833", "#CCBB44", "#66CCEE", "#AA3377", "#BBBBBB",
];
/// Paul Tol's muted scheme.
pub const TOL_MUTED: [&str; 9] = [
    "#332288", "#88CCEE", "#44AA99", "#117733", "#999933", "#DDCC77", "#CC6677", "#882255",
    "#AA4499",
];
/// Paul Tol's light scheme.
pub const TOL_LIGHT: [&str; 9] = [
    "#77AADD", "#EE8866", "#EEDD88", "#FFAABB", "#99DDFF", "#44BB99", "#BBCC33", "#AAAA00",
    "#DDDDDD",
];
/// Viridis, sampled at 11 stops.
pub const VIRIDIS: [&str; 11] = [
    "#440154", "#482475", "#414487", "#355F8D", "#2A788E", "#21918C", "#22A884", "#44BF70",
    "#7AD151", "#BDDF26", "#FDE725",
];
/// Cividis, sampled at 11 stops.
pub const CIVIDIS: [&str; 11] = [
    "#00224E", "#123570", "#3B496C", "#575D6D", "#707173", "#8A8678", "#A59C74", "#C3B369",
    "#E1CC55", "#F2DC4C", "#FEE838",
];
/// Paul Tol's sunset diverging scheme (blue → pale yellow → red).
pub const TOL_SUNSET: [&str; 11] = [
    "#364B9A", "#4A7BB7", "#6EA6CD", "#98CAE1", "#C2E4EF", "#EAECCC", "#FEDA8B", "#FDB366",
    "#F67E4B", "#DD3D2D", "#A50026",
];
/// Greyscale categorical ramp.
pub const GREY: [&str; 5] = ["#000000", "#666666", "#999999", "#BBBBBB", "#333333"];

/// A continuous colour scale from stops.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp(pub Vec<Color>);

impl Ramp {
    /// Named ramp: `viridis`, `cividis`, `tol_sunset`, `grey`.
    pub fn named(name: &str) -> Ramp {
        Ramp(match name {
            "cividis" => hexes(&CIVIDIS),
            "tol_sunset" => hexes(&TOL_SUNSET),
            "grey" => vec![Color::rgb(0xf4, 0xf4, 0xf4), Color::rgb(0x11, 0x11, 0x11)],
            _ => hexes(&VIRIDIS),
        })
    }

    /// Colour at `t` in [0, 1].
    pub fn at(&self, t: f64) -> Color {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let n = self.0.len();
        if n == 1 {
            return self.0[0];
        }
        let f = t * (n - 1) as f64;
        let i = (f.floor() as usize).min(n - 2);
        self.0[i].mix(self.0[i + 1], f - i as f64)
    }
}

/// Categorical colours for `n` series from a palette name. Sequential palettes are sampled
/// evenly (stopping short of the palest end so every colour reads on white).
pub fn categorical(name: &str, n: usize, night: bool) -> Vec<Color> {
    let n = n.max(1);
    let base: Vec<Color> = match name {
        "tol_bright" => hexes(&TOL_BRIGHT),
        "tol_muted" => hexes(&TOL_MUTED),
        "tol_light" => hexes(&TOL_LIGHT),
        "grey" => {
            if night {
                hexes(&["#EEEEEE", "#AAAAAA", "#777777", "#CCCCCC", "#555555"])
            } else {
                hexes(&GREY)
            }
        }
        "viridis" | "cividis" | "tol_sunset" => {
            let r = Ramp::named(name);
            let (a, b) = if name == "tol_sunset" {
                (0.0, 1.0)
            } else {
                (0.0, 0.85)
            };
            return (0..n)
                .map(|i| {
                    r.at(if n == 1 {
                        a
                    } else {
                        a + (b - a) * i as f64 / (n - 1) as f64
                    })
                })
                .collect();
        }
        _ => {
            let mut v = hexes(&OKABE_ITO);
            if night {
                v[0] = Color::rgb(0xe8, 0xea, 0xe8);
            }
            v
        }
    };
    (0..n).map(|i| base[i % base.len()]).collect()
}

/// Marker shapes in the order the journal theme cycles through them.
pub const MONO_MARKERS: [(MarkerShape, bool); 8] = [
    (MarkerShape::Circle, false),
    (MarkerShape::Circle, true),
    (MarkerShape::Square, false),
    (MarkerShape::Square, true),
    (MarkerShape::TriangleUp, false),
    (MarkerShape::TriangleUp, true),
    (MarkerShape::Diamond, false),
    (MarkerShape::Diamond, true),
];
