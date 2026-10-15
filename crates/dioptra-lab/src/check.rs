// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! The figure check: what a journal's production office would send back, found before
//! export. It reads the rendered scene (the same one SVG, PDF, PNG and TIFF are written
//! from), so it checks what is exported:
//!
//! - text below the journal minimum (or above its maximum) at the size the figure is
//!   printed: a figure wider than the journal's full width is reduced to fit, and its
//!   text with it; super- and subscripts count at their own size;
//! - lines thinner than the journal minimum, and never thinner than 0.25 pt;
//! - raster resolution below what the journal asks for (PNG and TIFF export);
//! - pairs of data-set colours that a reader with deuteranopia, protanopia or tritanopia
//!   cannot tell apart: each colour is simulated with Machado, Oliveira & Fernandes
//!   (2009), doi:10.1109/TVCG.2009.113, severity 1, on linear RGB, and the pair is
//!   flagged when its CIEDE2000 difference (Sharma, Wu & Dalal 2005,
//!   doi:10.1002/col.20070) is at least [`DELTA_E`] in normal vision and below it in the
//!   simulation;
//! - panels of a layout whose text or axis lines differ in size;
//! - a figure wider or taller than the journal allows.

use std::collections::BTreeMap;

use dioptra_core::graph::Target;
use dioptra_plot::scene::{Color, Group, Node, Paint, Scene};
use dioptra_plot::text::{Shift, SCRIPT_SCALE};
use serde::{Deserialize, Serialize};

use crate::journal::JournalPreset;

/// Two colours closer than this (CIEDE2000) look the same in a small mark. 10 is about
/// the difference between two neighbouring Okabe–Ito colours under the worst simulation
/// that still reads as different in a legend.
pub const DELTA_E: f64 = 10.0;

/// The absolute thinnest line, pt, whatever the journal.
pub const MIN_LINE_PT: f64 = 0.25;

/// Smallest text without a preset, pt.
pub const MIN_FONT_PT: f64 = 6.0;

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The journal will send it back.
    Error,
    /// Worth fixing.
    Warning,
    /// For information.
    Info,
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    /// Rule: `font_size`, `line_width`, `resolution`, `colour_vision`, `panel_consistency`,
    /// `size`.
    pub rule: String,
    /// Severity.
    pub severity: Severity,
    /// What is wrong, in a sentence.
    pub message: String,
    /// Scene element id (the graph editor selects it), when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    /// JSON pointer into the graph spec of that element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Raster export to check the resolution of.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RasterExport {
    /// Dots per inch.
    pub dpi: f64,
}

/// The result of a check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigureCheck {
    /// Findings, most serious first.
    pub issues: Vec<Issue>,
    /// Exported width, mm.
    pub width_mm: f64,
    /// Exported height, mm.
    pub height_mm: f64,
    /// Scale the journal applies to fit its full width (1 when it fits).
    pub scale: f64,
}

// ---- colour ----------------------------------------------------------------------

fn lin(v: u8) -> f64 {
    let v = v as f64 / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Colour-vision deficiency to simulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cvd {
    /// Red-blind.
    Protan,
    /// Green-blind.
    Deutan,
    /// Blue-blind.
    Tritan,
}

impl Cvd {
    fn name(self) -> &'static str {
        match self {
            Cvd::Protan => "protanopia",
            Cvd::Deutan => "deuteranopia",
            Cvd::Tritan => "tritanopia",
        }
    }

    /// Machado et al. (2009), Table 1 (severity 1.0), applied to linear RGB.
    fn matrix(self) -> [[f64; 3]; 3] {
        match self {
            Cvd::Protan => [
                [0.152_286, 1.052_583, -0.204_868],
                [0.114_503, 0.786_281, 0.099_216],
                [-0.003_882, -0.048_116, 1.051_998],
            ],
            Cvd::Deutan => [
                [0.367_322, 0.860_646, -0.227_968],
                [0.280_085, 0.672_501, 0.047_413],
                [-0.011_820, 0.042_940, 0.968_881],
            ],
            Cvd::Tritan => [
                [1.255_528, -0.076_749, -0.178_779],
                [-0.078_411, 0.930_809, 0.147_602],
                [0.004_733, 0.691_367, 0.303_900],
            ],
        }
    }
}

/// Linear RGB of a colour, simulated for `cvd` when given.
pub fn linear_rgb(c: Color, cvd: Option<Cvd>) -> [f64; 3] {
    let v = [lin(c.r), lin(c.g), lin(c.b)];
    match cvd {
        None => v,
        Some(d) => {
            let m = d.matrix();
            let f = |r: [f64; 3]| (r[0] * v[0] + r[1] * v[1] + r[2] * v[2]).clamp(0.0, 1.0);
            [f(m[0]), f(m[1]), f(m[2])]
        }
    }
}

/// CIELAB (D65) of linear RGB.
pub fn lab(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b;
    let z = (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 colour difference (Sharma, Wu & Dalal 2005), kL = kC = kH = 1.
pub fn delta_e2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    use std::f64::consts::PI;
    let (l1, a1, b1) = (a[0], a[1], a[2]);
    let (l2, a2, b2) = (b[0], b[1], b[2]);
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cb = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cb.powi(7) / (cb.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = ((a1p * a1p + b1 * b1).sqrt(), (a2p * a2p + b2 * b2).sqrt());
    let hue = |b: f64, a: f64| {
        if b == 0.0 && a == 0.0 {
            0.0
        } else {
            let h = b.atan2(a).to_degrees();
            if h < 0.0 {
                h + 360.0
            } else {
                h
            }
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dh = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh.to_radians() / 2.0).sin();
    let lbp = (l1 + l2) / 2.0;
    let cbp = (c1p + c2p) / 2.0;
    let hbp = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hbp - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hbp).to_radians().cos()
        + 0.32 * (3.0 * hbp + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hbp - 63.0).to_radians().cos();
    let d_theta = 30.0 * (-((hbp - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cbp.powi(7) / (cbp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lbp - 50.0).powi(2) / (20.0 + (lbp - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cbp;
    let sh = 1.0 + 0.015 * cbp * t;
    let rt = -(2.0 * d_theta * PI / 180.0).sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh))
        .sqrt()
}

/// CIEDE2000 between two colours as seen with `cvd` (or normal vision).
pub fn colour_difference(a: Color, b: Color, cvd: Option<Cvd>) -> f64 {
    delta_e2000(lab(linear_rgb(a, cvd)), lab(linear_rgb(b, cvd)))
}

// ---- walking the scene -----------------------------------------------------------

/// Where a node sits: the nearest identified group.
#[derive(Debug, Clone, Default)]
struct Owner {
    id: Option<String>,
    target: Option<String>,
}

fn walk<'a>(g: &'a Group, owner: &Owner, f: &mut impl FnMut(&'a Node, &Owner)) {
    let here = if g.id.is_some() {
        Owner {
            id: g.id.clone(),
            target: g.target.clone(),
        }
    } else {
        owner.clone()
    };
    for c in &g.children {
        match c {
            Node::Group(cg) => walk(cg, &here, f),
            n => f(n, &here),
        }
    }
}

/// Widths of the visible strokes of a node. Strokes in the background colour (gaps between
/// pie slices or heatmap cells) and fully transparent ones are not ink.
fn strokes(n: &Node, background: Option<Color>) -> Vec<f64> {
    let ink = |s: &dioptra_plot::scene::Stroke| {
        s.color.a > 0 && Some(Color { a: 255, ..s.color }) != background
    };
    let p = |paint: &Paint| {
        paint
            .stroke
            .filter(ink)
            .map(|s| s.width)
            .into_iter()
            .collect()
    };
    match n {
        Node::Line { stroke, .. } if ink(stroke) => vec![stroke.width],
        Node::Poly { paint, .. } | Node::Rect { paint, .. } | Node::Path { paint, .. } => p(paint),
        Node::Markers { paint, .. } => p(paint),
        _ => Vec::new(),
    }
}

/// A readable name for a scene element id.
pub fn element_name(id: &str) -> String {
    let (panel, rest) = match id.strip_prefix('p').and_then(|r| r.split_once('-')) {
        Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => {
            let i: usize = n.parse().unwrap_or(0);
            (Some(dioptra_plot::compose::letter(i)), rest.to_string())
        }
        _ => (None, id.to_string()),
    };
    let num = |p: &str| {
        rest.strip_prefix(p)
            .and_then(|n| n.parse::<usize>().ok())
            .map(|n| n + 1)
    };
    let name = match rest.as_str() {
        "axis-x" => "X axis".into(),
        "axis-y" => "Y axis".into(),
        "axis-y2" => "Difference axis".into(),
        "axis-x-labels" => "X tick labels".into(),
        "axis-y-labels" => "Y tick labels".into(),
        "axis-x-title" => "X axis title".into(),
        "axis-y-title" | "axis-y2-title" => "Y axis title".into(),
        "legend" => "Legend".into(),
        "title" => "Graph title".into(),
        "at-risk" => "Number-at-risk table".into(),
        "colour-bar" => "Colour bar".into(),
        "estimates" => "Estimates column".into(),
        "frame" => "Frame".into(),
        "grid" => "Gridlines".into(),
        "plot-area" => "Data".into(),
        "labels" => "Point labels".into(),
        r if r.starts_with("panel-") && r.ends_with("-letter") => "Panel letter".into(),
        r if r.starts_with("panel-") => {
            let i: usize = r[6..].parse().unwrap_or(0);
            return format!("Panel {}", dioptra_plot::compose::letter(i));
        }
        _ => {
            if let Some(n) = num("series-") {
                format!("Data set {n}")
            } else if let Some(n) = num("legend-item-") {
                format!("Legend entry {n}")
            } else if let Some(n) = num("annotation-") {
                format!("Annotation {n}")
            } else if rest.starts_with("subject-") || rest.starts_with("connect") {
                "Connecting lines".into()
            } else {
                "Plot".into()
            }
        }
    };
    match panel {
        Some(l) => format!("Panel {l}: {name}"),
        None => name,
    }
}

fn pt(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s} pt")
}

// ---- the rules ---------------------------------------------------------------------

/// Check a rendered figure against a preset (or general rules without one). `raster`
/// asks for the resolution rule; `panels` marks a layout (panel groups `panel-{i}`).
pub fn check_scene(
    scene: &Scene,
    preset: Option<&JournalPreset>,
    raster: Option<RasterExport>,
) -> FigureCheck {
    let mut issues = Vec::new();
    let (w, h) = (scene.width_mm(), scene.height_mm());
    let (journal, needs, asks) = match preset {
        Some(p) => (p.name.to_string(), "needs", "asks"),
        None => ("journals".to_string(), "need", "ask"),
    };

    // Size: a figure wider than the full width is reduced to fit.
    let mut scale = 1.0;
    if let Some(p) = preset {
        if w > p.double_mm + 0.5 {
            scale = p.double_mm / w;
            issues.push(Issue {
                rule: "size".into(),
                severity: Severity::Error,
                message: format!(
                    "The figure is {w:.0} mm wide; {} prints at most {} mm, so it will be reduced to {:.0}%, text and lines with it.",
                    p.name,
                    p.double_mm,
                    scale * 100.0
                ),
                element: None,
                target: Some("/style/size_mm".into()),
            });
        } else {
            let widths = [Some(p.single_mm), p.onehalf_mm, Some(p.double_mm)];
            if !widths.iter().flatten().any(|c| (c - w).abs() <= 1.0) {
                let list: Vec<String> =
                    widths.iter().flatten().map(|c| format!("{c} mm")).collect();
                issues.push(Issue {
                    rule: "size".into(),
                    severity: Severity::Info,
                    message: format!(
                        "The figure is {w:.0} mm wide; {} columns are {}.",
                        p.name,
                        list.join(", ")
                    ),
                    element: None,
                    target: Some("/style/size_mm".into()),
                });
            }
        }
        if let Some(m) = p.max_height_mm {
            if h * scale > m + 0.5 {
                issues.push(Issue {
                    rule: "size".into(),
                    severity: Severity::Error,
                    message: format!(
                        "The figure is {:.0} mm tall; {} allows at most {m} mm.",
                        h * scale,
                        p.name
                    ),
                    element: None,
                    target: Some("/style/size_mm".into()),
                });
            }
        }
    }

    // Text and lines, gathered per element.
    let min_font = preset.map_or(MIN_FONT_PT, |p| p.min_font_pt);
    let max_font = preset.and_then(|p| p.max_font_pt);
    let min_line = preset
        .map_or(MIN_LINE_PT, |p| p.min_line_pt)
        .max(MIN_LINE_PT);
    // element id → (smallest text, smallest script, largest text, thinnest line, target)
    type Extremes = (f64, f64, f64, f64, Option<String>);
    let mut per: BTreeMap<String, Extremes> = BTreeMap::new();
    walk(&scene.root, &Owner::default(), &mut |n, o| {
        let key = o.id.clone().unwrap_or_default();
        let e =
            per.entry(key.clone())
                .or_insert((f64::MAX, f64::MAX, 0.0, f64::MAX, o.target.clone()));
        if let Node::Text(t) = n {
            // Panel letters are set by the layout, at the journal's letter size.
            if key.ends_with("-letter") {
                return;
            }
            for r in &t.runs {
                if r.text.trim().is_empty() {
                    continue;
                }
                if r.shift == Shift::None {
                    e.0 = e.0.min(t.size * scale);
                    e.2 = e.2.max(t.size * scale);
                } else {
                    e.1 = e.1.min(t.size * SCRIPT_SCALE * scale);
                }
            }
        }
        for s in strokes(n, scene.background) {
            if s > 0.0 {
                e.3 = e.3.min(s * scale);
            }
        }
    });
    for (id, (lo, script, hi, thin, target)) in &per {
        let element = (!id.is_empty()).then(|| id.clone());
        let name = element_name(id);
        if *lo < min_font - 1e-6 {
            issues.push(Issue {
                rule: "font_size".into(),
                severity: Severity::Error,
                message: format!(
                    "{name}: text is {} at the printed size; {journal} {needs} at least {}.",
                    pt(*lo),
                    pt(min_font)
                ),
                element: element.clone(),
                target: target.clone(),
            });
        } else if *script < min_font - 1e-6 {
            issues.push(Issue {
                rule: "font_size".into(),
                severity: Severity::Warning,
                message: format!(
                    "{name}: a superscript or subscript is {} at the printed size; {journal} {needs} at least {}.",
                    pt(*script),
                    pt(min_font)
                ),
                element: element.clone(),
                target: target.clone(),
            });
        }
        if let Some(m) = max_font {
            if *hi > m + 1e-6 {
                issues.push(Issue {
                    rule: "font_size".into(),
                    severity: Severity::Warning,
                    message: format!(
                        "{name}: text is {} at the printed size; {journal} allows at most {}.",
                        pt(*hi),
                        pt(m)
                    ),
                    element: element.clone(),
                    target: target.clone(),
                });
            }
        }
        if *thin < min_line - 1e-6 {
            issues.push(Issue {
                rule: "line_width".into(),
                severity: Severity::Error,
                message: format!(
                    "{name}: a line is {} at the printed size; lines must be at least {}.",
                    pt(*thin),
                    pt(min_line)
                ),
                element: element.clone(),
                target: target.clone(),
            });
        }
    }

    // Raster resolution.
    if let Some(r) = raster {
        let colour = uses_colour(scene);
        let (need, kind) = match preset {
            Some(p) if colour => (p.dpi_combination, "colour or combination art"),
            Some(p) => (p.dpi_line_art, "black-and-white line art"),
            None if colour => (300.0, "colour art"),
            None => (600.0, "line art"),
        };
        if r.dpi < need {
            issues.push(Issue {
                rule: "resolution".into(),
                severity: Severity::Error,
                message: format!(
                    "{} dpi is below the {need:.0} dpi {journal} {asks} for {kind}; export at {need:.0} dpi or as a vector file.",
                    r.dpi
                ),
                element: None,
                target: None,
            });
        }
    }

    // Colour vision.
    issues.extend(colour_pairs(scene));

    // Panels of a layout.
    issues.extend(panel_consistency(scene));

    issues.sort_by_key(|i| i.severity);
    FigureCheck {
        issues,
        width_mm: w,
        height_mm: h,
        scale,
    }
}

fn is_grey(c: Color) -> bool {
    let (mx, mn) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    mx - mn <= 8
}

fn uses_colour(scene: &Scene) -> bool {
    let mut colour = false;
    walk(&scene.root, &Owner::default(), &mut |n, _| {
        let cs: Vec<Color> = match n {
            Node::Line { stroke, .. } => vec![stroke.color],
            Node::Poly { paint, .. }
            | Node::Rect { paint, .. }
            | Node::Path { paint, .. }
            | Node::Markers { paint, .. } => paint
                .fill
                .into_iter()
                .chain(paint.stroke.map(|s| s.color))
                .collect(),
            Node::Text(t) => vec![t.color],
            _ => Vec::new(),
        };
        colour |= cs.iter().any(|c| !is_grey(*c));
    });
    colour
}

/// The colour of each plotted data set (`series-{i}` groups): its markers, else its
/// lines, else its fills.
fn series_colours(scene: &Scene) -> Vec<(String, Option<String>, Color)> {
    fn find<'a>(g: &'a Group, out: &mut Vec<&'a Group>) {
        let is_series = g.id.as_deref().is_some_and(|id| {
            id.rsplit('-')
                .next()
                .is_some_and(|n| n.parse::<usize>().is_ok())
                && (id.starts_with("series-") || id.contains("-series-"))
        });
        if is_series {
            out.push(g);
            return;
        }
        for c in &g.children {
            if let Node::Group(cg) = c {
                find(cg, out);
            }
        }
    }
    let mut groups = Vec::new();
    find(&scene.root, &mut groups);
    let mut out = Vec::new();
    for g in groups {
        let (mut marker, mut line, mut fill) = (None, None, None);
        walk(g, &Owner::default(), &mut |n, _| match n {
            Node::Markers { paint, .. } => {
                marker = marker.or(paint.fill.or(paint.stroke.map(|s| s.color)));
            }
            Node::Line { stroke, .. } => line = line.or(Some(stroke.color)),
            Node::Poly { paint, .. } | Node::Path { paint, .. } => {
                line = line.or(paint.stroke.map(|s| s.color));
                fill = fill.or(paint.fill);
            }
            Node::Rect { paint, .. } => fill = fill.or(paint.fill),
            _ => {}
        });
        if let Some(c) = marker.or(line).or(fill) {
            out.push((
                g.id.clone().unwrap_or_default(),
                g.target.clone(),
                Color { a: 255, ..c },
            ));
        }
    }
    out
}

fn colour_pairs(scene: &Scene) -> Vec<Issue> {
    let cols = series_colours(scene);
    let mut issues = Vec::new();
    for i in 0..cols.len() {
        for j in i + 1..cols.len() {
            let (a, b) = (cols[i].2, cols[j].2);
            // Panels of a layout are compared within the panel only.
            let panel = |id: &str| id.split_once("-series-").map(|(p, _)| p.to_string());
            if panel(&cols[i].0) != panel(&cols[j].0) {
                continue;
            }
            if colour_difference(a, b, None) < DELTA_E {
                continue;
            }
            let lost: Vec<&str> = [Cvd::Deutan, Cvd::Protan, Cvd::Tritan]
                .into_iter()
                .filter(|d| colour_difference(a, b, Some(*d)) < DELTA_E)
                .map(Cvd::name)
                .collect();
            if lost.is_empty() {
                continue;
            }
            issues.push(Issue {
                rule: "colour_vision".into(),
                severity: Severity::Warning,
                message: format!(
                    "{} ({}) and {} ({}) look alike with {}. Use a colour-blind-safe palette such as Okabe–Ito, or tell them apart by marker or line style.",
                    element_name(&cols[i].0),
                    a.to_hex(),
                    element_name(&cols[j].0),
                    b.to_hex(),
                    lost.join(" and ")
                ),
                element: Some(cols[j].0.clone()),
                target: cols[j].1.clone(),
            });
        }
    }
    issues
}

/// Most common value (rounded to 0.05) of a list.
fn mode(v: &[f64]) -> Option<f64> {
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for x in v {
        *counts.entry((x * 20.0).round() as i64).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(k, n)| (*n, -*k))
        .map(|(k, _)| k as f64 / 20.0)
}

fn panel_consistency(scene: &Scene) -> Vec<Issue> {
    let panels: Vec<&Group> = scene
        .root
        .children
        .iter()
        .filter_map(|c| match c {
            Node::Group(g)
                if g.id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("panel-") && !id.ends_with("-letter")) =>
            {
                Some(g)
            }
            _ => None,
        })
        .collect();
    if panels.len() < 2 {
        return Vec::new();
    }
    let mut fonts = Vec::new();
    let mut axes = Vec::new();
    for g in &panels {
        let (mut sizes, mut widths) = (Vec::new(), Vec::new());
        walk(g, &Owner::default(), &mut |n, o| {
            let id = o.id.as_deref().unwrap_or("");
            if id.ends_with("-letter") {
                return;
            }
            if let Node::Text(t) = n {
                if id.ends_with("axis-x-labels") || id.ends_with("axis-y-labels") {
                    sizes.push(t.size);
                }
            }
            if id.ends_with("axis-x") || id.ends_with("axis-y") {
                if let Node::Line { stroke, .. } = n {
                    widths.push(stroke.width);
                }
            }
        });
        fonts.push(mode(&sizes));
        axes.push(mode(&widths));
    }
    let mut issues = Vec::new();
    for (what, vals, rule_target) in [
        ("tick labels", &fonts, "/style/font_pt"),
        ("axis lines", &axes, "/style/line_pt"),
    ] {
        let known: Vec<f64> = vals.iter().flatten().copied().collect();
        let Some(common) = mode(&known) else { continue };
        for (i, v) in vals.iter().enumerate() {
            let Some(v) = v else { continue };
            if (v - common).abs() > 0.05 {
                issues.push(Issue {
                    rule: "panel_consistency".into(),
                    severity: Severity::Warning,
                    message: format!(
                        "Panel {}: {what} are {} while the other panels use {}. Set one size for the layout.",
                        dioptra_plot::compose::letter(i),
                        pt(*v),
                        pt(common)
                    ),
                    element: panels[i].id.clone(),
                    target: Some(format!("/graphs/{i}{rule_target}")),
                });
            }
        }
    }
    issues
}

/// Whether a target from a check exists in the editor's list (so a click can select it).
pub fn selectable(targets: &[Target], issue: &Issue) -> bool {
    issue
        .element
        .as_ref()
        .is_some_and(|e| targets.iter().any(|t| &t.id == e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::preset;
    use dioptra_plot::samples::sample;

    fn scene_of(family: &str, f: impl FnOnce(&mut dioptra_plot::spec::GraphSpec)) -> Scene {
        let (mut spec, data) = sample(family).unwrap();
        f(&mut spec);
        dioptra_plot::scene(&spec, &data).unwrap()
    }

    fn rules(c: &FigureCheck, rule: &str) -> Vec<Issue> {
        c.issues
            .iter()
            .filter(|i| i.rule == rule)
            .cloned()
            .collect()
    }

    #[test]
    fn ciede2000_matches_sharma_test_data() {
        // Sharma, Wu & Dalal (2005), Table 1, pairs 1, 7 and 17.
        let cases = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
        ];
        for (a, b, want) in cases {
            assert!((delta_e2000(a, b) - want).abs() < 1e-4, "{a:?} {b:?}");
        }
    }

    #[test]
    fn simulation_merges_red_and_green_but_not_okabe_ito() {
        let red = Color::hex("#D62728").unwrap();
        let green = Color::hex("#2CA02C").unwrap();
        assert!(colour_difference(red, green, None) > 30.0);
        assert!(colour_difference(red, green, Some(Cvd::Deutan)) < DELTA_E);
        // Neighbouring Okabe–Ito colours stay apart in every simulation.
        let oi = [
            "#E69F00", "#56B4E9", "#009E73", "#0072B2", "#D55E00", "#CC79A7",
        ];
        for (i, a) in oi.iter().enumerate() {
            for b in &oi[i + 1..] {
                for d in [Cvd::Deutan, Cvd::Protan, Cvd::Tritan] {
                    let e =
                        colour_difference(Color::hex(a).unwrap(), Color::hex(b).unwrap(), Some(d));
                    assert!(e >= DELTA_E, "{a} {b} {d:?}: {e}");
                }
            }
        }
    }

    #[test]
    fn small_text_fails_and_default_text_passes() {
        let nature = preset("nature").unwrap();
        let ok = scene_of("column", |s| {
            s.style.size_mm.width = 89.0;
            s.style.font_pt = Some(nature.font_pt());
            s.style.line_pt = Some(nature.line_pt());
        });
        let c = check_scene(&ok, Some(nature), None);
        assert!(rules(&c, "font_size").is_empty(), "{:?}", c.issues);
        assert!(rules(&c, "line_width").is_empty(), "{:?}", c.issues);
        assert!(rules(&c, "size").is_empty(), "{:?}", c.issues);
        let small = scene_of("column", |s| s.style.font_pt = Some(4.0));
        let c = check_scene(&small, Some(nature), None);
        let f = rules(&c, "font_size");
        assert!(
            f.iter()
                .any(|i| i.element.as_deref() == Some("axis-y-labels")
                    && i.severity == Severity::Error
                    && i.target.as_deref() == Some("/axes/y/ticks")),
            "{f:?}"
        );
        // Text above Nature's 7 pt maximum.
        let big = scene_of("column", |s| s.style.font_pt = Some(9.0));
        let c = check_scene(&big, Some(nature), None);
        assert!(rules(&c, "font_size")
            .iter()
            .any(|i| i.severity == Severity::Warning));
    }

    #[test]
    fn a_wide_figure_is_checked_at_its_printed_size() {
        let nature = preset("nature").unwrap();
        // 8 pt on a 366 mm figure prints at 4 pt in a 183 mm column.
        let wide = scene_of("column", |s| {
            s.style.size_mm.width = 366.0;
            s.style.font_pt = Some(6.0);
        });
        let c = check_scene(&wide, Some(nature), None);
        assert!((c.scale - 0.5).abs() < 1e-9);
        assert_eq!(rules(&c, "size")[0].severity, Severity::Error);
        assert!(!rules(&c, "font_size").is_empty());
        let tall = scene_of("column", |s| s.style.size_mm.height = 200.0);
        assert!(rules(&check_scene(&tall, Some(nature), None), "size")
            .iter()
            .any(|i| i.message.contains("tall")));
    }

    #[test]
    fn preset_line_width_holds_in_every_family() {
        for p in crate::journal::presets() {
            for family in dioptra_plot::FAMILIES {
                let Some((mut spec, data)) = sample(family) else {
                    continue;
                };
                spec.style.line_pt = Some(p.line_pt());
                spec.style.font_pt = Some(p.font_pt());
                let scene = dioptra_plot::scene(&spec, &data).unwrap();
                let c = check_scene(&scene, Some(p), None);
                let bad: Vec<_> = c
                    .issues
                    .iter()
                    .filter(|i| {
                        i.rule == "line_width"
                            || (i.rule == "font_size" && i.severity == Severity::Error)
                    })
                    .collect();
                assert!(bad.is_empty(), "{} {family}: {bad:?}", p.id);
            }
        }
    }

    #[test]
    fn thin_lines_fail() {
        let thin = scene_of("scatter", |s| s.style.line_pt = Some(0.2));
        let c = check_scene(&thin, None, None);
        let l = rules(&c, "line_width");
        assert!(
            l.iter().any(|i| i.element.as_deref() == Some("axis-x")),
            "{l:?}"
        );
        let fine = scene_of("scatter", |s| s.style.line_pt = Some(0.75));
        assert!(rules(&check_scene(&fine, None, None), "line_width").is_empty());
        // ACS asks for 0.5 pt.
        let acs = preset("acs").unwrap();
        let mid = scene_of("scatter", |s| s.style.line_pt = Some(0.4));
        assert!(!rules(&check_scene(&mid, Some(acs), None), "line_width").is_empty());
    }

    #[test]
    fn raster_resolution() {
        let cell = preset("cell").unwrap();
        let colour = scene_of("column", |_| {});
        let low = check_scene(&colour, Some(cell), Some(RasterExport { dpi: 300.0 }));
        assert_eq!(rules(&low, "resolution").len(), 1);
        let high = check_scene(&colour, Some(cell), Some(RasterExport { dpi: 600.0 }));
        assert!(rules(&high, "resolution").is_empty());
        // Black-and-white art needs line-art resolution.
        let mono = scene_of("column", |s| s.style.theme = "journal".into());
        let c = check_scene(&mono, Some(cell), Some(RasterExport { dpi: 600.0 }));
        assert!(rules(&c, "resolution")[0].message.contains("1000 dpi"));
        // No raster export, no rule.
        assert!(rules(&check_scene(&colour, Some(cell), None), "resolution").is_empty());
    }

    #[test]
    fn colour_pairs_that_vanish_for_colour_blind_readers() {
        let bad = scene_of("scatter", |s| {
            s.style.datasets = vec![
                dioptra_plot::spec::DatasetStyle {
                    dataset: 0,
                    colour: Some("#D62728".into()),
                    ..Default::default()
                },
                dioptra_plot::spec::DatasetStyle {
                    dataset: 1,
                    colour: Some("#2CA02C".into()),
                    ..Default::default()
                },
            ];
        });
        let c = check_scene(&bad, None, None);
        let v = rules(&c, "colour_vision");
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].message.contains("deuteranopia"), "{}", v[0].message);
        assert_eq!(v[0].element.as_deref(), Some("series-1"));
        let good = scene_of("scatter", |_| {});
        assert!(rules(&check_scene(&good, None, None), "colour_vision").is_empty());
    }

    #[test]
    fn panels_with_different_fonts() {
        let a = scene_of("column", |s| s.style.font_pt = Some(7.0));
        let b = scene_of("scatter", |s| s.style.font_pt = Some(9.0));
        let c = scene_of("box", |s| s.style.font_pt = Some(7.0));
        let page = dioptra_plot::compose_layout(&[a.clone(), b, c.clone()], 3, true);
        let r = check_scene(&page, None, None);
        let p = rules(&r, "panel_consistency");
        assert_eq!(p.len(), 1, "{p:?}");
        assert_eq!(p[0].element.as_deref(), Some("panel-1"));
        let same = dioptra_plot::compose_layout(&[a, c], 2, true);
        assert!(rules(&check_scene(&same, None, None), "panel_consistency").is_empty());
    }

    #[test]
    fn element_names() {
        assert_eq!(element_name("axis-x-labels"), "X tick labels");
        assert_eq!(element_name("p1-series-0"), "Panel B: Data set 1");
        assert_eq!(element_name("panel-2"), "Panel C");
    }
}
