//! The SVG writer: clean, grouped, real `<text>`, no CSS, user units in points, physical
//! size in millimetres. Illustrator and Inkscape open the output as editable artwork.

use std::fmt::Write as _;

use crate::scene::{
    Anchor, Color, Dash, Group, MarkerShape, Node, Paint, PathCmd, Scene, Stroke, Text,
};
use crate::text::{Shift, FAMILY, SCRIPT_SCALE, SUB_DROP, SUPER_RISE};

/// Append a number with at most two decimals, trailing zeros trimmed (deterministic).
pub fn num(out: &mut String, v: f64) {
    if !v.is_finite() {
        out.push('0');
        return;
    }
    let h = (v * 100.0).round() as i64;
    if h < 0 {
        out.push('-');
    }
    let a = h.unsigned_abs();
    let (int, frac) = (a / 100, a % 100);
    let _ = write!(out, "{int}");
    if frac != 0 {
        if frac % 10 == 0 {
            let _ = write!(out, ".{}", frac / 10);
        } else {
            let _ = write!(out, ".{frac:02}");
        }
    }
}

/// A number as a string (see [`num`]).
pub fn n(v: f64) -> String {
    let mut s = String::new();
    num(&mut s, v);
    s
}

/// Escape text for XML.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c => o.push(c),
        }
    }
    o
}

fn dasharray(s: &Stroke) -> Option<String> {
    let w = s.width.max(0.5);
    match s.dash {
        Dash::Solid => None,
        Dash::Dashed => Some(format!("{} {}", n(4.0 * w), n(2.5 * w))),
        Dash::Dotted => Some(format!("{} {}", n(w), n(1.8 * w))),
        Dash::DashDot => Some(format!(
            "{} {} {} {}",
            n(4.0 * w),
            n(1.8 * w),
            n(w),
            n(1.8 * w)
        )),
    }
}

fn color_attr(out: &mut String, name: &str, c: Color) {
    let _ = write!(out, " {name}=\"{}\"", c.to_hex());
    if c.a < 255 {
        let _ = write!(out, " {name}-opacity=\"{}\"", n(c.a as f64 / 255.0));
    }
}

fn paint_attrs(out: &mut String, p: &Paint) {
    match p.fill {
        Some(c) => color_attr(out, "fill", c),
        None => out.push_str(" fill=\"none\""),
    }
    if let Some(s) = &p.stroke {
        stroke_attrs(out, s);
    }
}

fn stroke_attrs(out: &mut String, s: &Stroke) {
    color_attr(out, "stroke", s.color);
    out.push_str(" stroke-width=\"");
    num(out, s.width);
    out.push('"');
    if let Some(d) = dasharray(s) {
        let _ = write!(out, " stroke-dasharray=\"{d}\"");
    }
}

fn path_d(out: &mut String, cmds: &[PathCmd]) {
    for (i, c) in cmds.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match *c {
            PathCmd::M(x, y) | PathCmd::L(x, y) => {
                out.push(if matches!(c, PathCmd::M(..)) {
                    'M'
                } else {
                    'L'
                });
                num(out, x);
                out.push(' ');
                num(out, y);
            }
            PathCmd::C(a, b, c2, d, x, y) => {
                out.push('C');
                for (k, v) in [a, b, c2, d, x, y].into_iter().enumerate() {
                    if k > 0 {
                        out.push(' ');
                    }
                    num(out, v);
                }
            }
            PathCmd::A(rx, ry, large, sweep, x, y) => {
                out.push('A');
                num(out, rx);
                out.push(' ');
                num(out, ry);
                let _ = write!(out, " 0 {} {} ", large as u8, sweep as u8);
                num(out, x);
                out.push(' ');
                num(out, y);
            }
            PathCmd::Z => out.push('Z'),
        }
    }
}

/// Marker outline as path commands centred on (x, y) with diameter `d`.
pub fn marker_path(shape: MarkerShape, x: f64, y: f64, d: f64) -> Vec<PathCmd> {
    let r = d / 2.0;
    match shape {
        MarkerShape::Circle => vec![
            PathCmd::M(x - r, y),
            PathCmd::A(r, r, false, true, x + r, y),
            PathCmd::A(r, r, false, true, x - r, y),
            PathCmd::Z,
        ],
        MarkerShape::Square => {
            let h = 0.886 * r;
            vec![
                PathCmd::M(x - h, y - h),
                PathCmd::L(x + h, y - h),
                PathCmd::L(x + h, y + h),
                PathCmd::L(x - h, y + h),
                PathCmd::Z,
            ]
        }
        MarkerShape::TriangleUp | MarkerShape::TriangleDown => {
            let s = if shape == MarkerShape::TriangleUp {
                1.0
            } else {
                -1.0
            };
            let rr = 1.2 * r;
            vec![
                PathCmd::M(x, y - s * rr),
                PathCmd::L(x + 0.866 * rr, y + s * 0.5 * rr),
                PathCmd::L(x - 0.866 * rr, y + s * 0.5 * rr),
                PathCmd::Z,
            ]
        }
        MarkerShape::Diamond => {
            let h = 1.2 * r;
            vec![
                PathCmd::M(x, y - h),
                PathCmd::L(x + h * 0.8, y),
                PathCmd::L(x, y + h),
                PathCmd::L(x - h * 0.8, y),
                PathCmd::Z,
            ]
        }
        MarkerShape::Plus => vec![
            PathCmd::M(x - r, y),
            PathCmd::L(x + r, y),
            PathCmd::M(x, y - r),
            PathCmd::L(x, y + r),
        ],
        MarkerShape::Cross => {
            let h = 0.75 * r;
            vec![
                PathCmd::M(x - h, y - h),
                PathCmd::L(x + h, y + h),
                PathCmd::M(x - h, y + h),
                PathCmd::L(x + h, y - h),
            ]
        }
        MarkerShape::Bar => vec![PathCmd::M(x - r, y), PathCmd::L(x + r, y)],
        MarkerShape::Tick => vec![PathCmd::M(x, y - r), PathCmd::L(x, y + r)],
    }
}

fn write_text(out: &mut String, t: &Text) {
    out.push_str("<text x=\"");
    num(out, t.x);
    out.push_str("\" y=\"");
    num(out, t.baseline_y());
    out.push_str("\" font-size=\"");
    num(out, t.size);
    out.push('"');
    if t.weight.css() != 400 {
        let _ = write!(out, " font-weight=\"{}\"", t.weight.css());
    }
    if t.italic {
        out.push_str(" font-style=\"italic\"");
    }
    match t.anchor {
        Anchor::Start => {}
        Anchor::Middle => out.push_str(" text-anchor=\"middle\""),
        Anchor::End => out.push_str(" text-anchor=\"end\""),
    }
    if t.color != Color::rgb(0, 0, 0) {
        color_attr(out, "fill", t.color);
    }
    if t.rotate != 0.0 {
        out.push_str(" transform=\"rotate(");
        num(out, t.rotate);
        out.push(' ');
        num(out, t.x);
        out.push(' ');
        num(out, t.y);
        out.push_str(")\"");
    }
    out.push('>');
    if t.runs.len() == 1 && t.runs[0].shift == Shift::None {
        out.push_str(&escape(&t.runs[0].text));
    } else {
        let mut offset = 0.0; // current baseline offset (positive = down)
        for r in &t.runs {
            let target = match r.shift {
                Shift::None => 0.0,
                Shift::Super => -SUPER_RISE * t.size,
                Shift::Sub => SUB_DROP * t.size,
            };
            out.push_str("<tspan");
            if target != offset {
                out.push_str(" dy=\"");
                num(out, target - offset);
                out.push('"');
                offset = target;
            }
            if r.shift != Shift::None {
                out.push_str(" font-size=\"");
                num(out, t.size * SCRIPT_SCALE);
                out.push('"');
            }
            out.push('>');
            out.push_str(&escape(&r.text));
            out.push_str("</tspan>");
        }
    }
    out.push_str("</text>");
}

fn write_node(out: &mut String, node: &Node, clips: &mut usize) {
    match node {
        Node::Group(g) => write_group(out, g, clips),
        Node::Line { p1, p2, stroke } => {
            out.push_str("<line x1=\"");
            num(out, p1.0);
            out.push_str("\" y1=\"");
            num(out, p1.1);
            out.push_str("\" x2=\"");
            num(out, p2.0);
            out.push_str("\" y2=\"");
            num(out, p2.1);
            out.push('"');
            stroke_attrs(out, stroke);
            out.push_str("/>");
        }
        Node::Poly {
            points,
            closed,
            paint,
        } => {
            if points.len() < 2 {
                return;
            }
            out.push_str(if *closed {
                "<polygon points=\""
            } else {
                "<polyline points=\""
            });
            for (i, (x, y)) in points.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                num(out, *x);
                out.push(',');
                num(out, *y);
            }
            out.push('"');
            paint_attrs(out, paint);
            if paint.stroke.is_some() {
                out.push_str(" stroke-linejoin=\"round\"");
            }
            out.push_str("/>");
        }
        Node::Rect { rect, paint } => {
            out.push_str("<rect x=\"");
            num(out, rect.x);
            out.push_str("\" y=\"");
            num(out, rect.y);
            out.push_str("\" width=\"");
            num(out, rect.width.max(0.0));
            out.push_str("\" height=\"");
            num(out, rect.height.max(0.0));
            out.push('"');
            paint_attrs(out, paint);
            out.push_str("/>");
        }
        Node::Path { cmds, paint } => {
            out.push_str("<path d=\"");
            path_d(out, cmds);
            out.push('"');
            paint_attrs(out, paint);
            if paint.stroke.is_some() {
                out.push_str(" stroke-linejoin=\"round\"");
            }
            out.push_str("/>");
        }
        Node::Markers {
            shape,
            size,
            paint,
            points,
        } => {
            if points.is_empty() {
                return;
            }
            let mut p = *paint;
            if shape.is_line() {
                // Line-only shapes take their colour from the stroke (or the fill).
                let c = p
                    .stroke
                    .map(|s| s.color)
                    .or(p.fill)
                    .unwrap_or(Color::rgb(0, 0, 0));
                let w = p.stroke.map_or((size * 0.22).max(0.5), |s| s.width);
                p = Paint::stroke(Stroke::solid(c, w));
            }
            out.push_str("<g");
            paint_attrs(out, &p);
            out.push('>');
            let r = size / 2.0;
            for (x, y) in points {
                if *shape == MarkerShape::Circle {
                    out.push_str("<circle cx=\"");
                    num(out, *x);
                    out.push_str("\" cy=\"");
                    num(out, *y);
                    out.push_str("\" r=\"");
                    num(out, r);
                    out.push_str("\"/>");
                } else {
                    out.push_str("<path d=\"");
                    path_d(out, &marker_path(*shape, *x, *y, *size));
                    out.push_str("\"/>");
                }
            }
            out.push_str("</g>");
        }
        Node::Text(t) => write_text(out, t),
        Node::Comment(c) => {
            let _ = write!(out, "<!-- {} -->", c.replace("--", "- -"));
        }
    }
}

fn write_group(out: &mut String, g: &Group, clips: &mut usize) {
    let mut clip_id = None;
    if let Some(c) = g.clip {
        *clips += 1;
        let id = format!("clip-{}", clips);
        out.push_str("<clipPath id=\"");
        out.push_str(&id);
        out.push_str("\"><rect x=\"");
        num(out, c.x);
        out.push_str("\" y=\"");
        num(out, c.y);
        out.push_str("\" width=\"");
        num(out, c.width);
        out.push_str("\" height=\"");
        num(out, c.height);
        out.push_str("\"/></clipPath>");
        clip_id = Some(id);
    }
    out.push_str("<g");
    if let Some(id) = &g.id {
        let _ = write!(out, " id=\"{}\"", escape(id));
    }
    if let Some(t) = &g.target {
        let _ = write!(out, " data-target=\"{}\"", escape(t));
    }
    if let Some((x, y)) = g.translate {
        out.push_str(" transform=\"translate(");
        num(out, x);
        out.push(' ');
        num(out, y);
        out.push_str(")\"");
    }
    if let Some(id) = clip_id {
        let _ = write!(out, " clip-path=\"url(#{id})\"");
    }
    out.push('>');
    for c in &g.children {
        write_node(out, c, clips);
    }
    out.push_str("</g>");
}

/// Write a scene as an SVG document.
pub fn to_svg(scene: &Scene) -> String {
    let mut out = String::with_capacity(16 * 1024);
    let (wmm, hmm) = (scene.width_mm(), scene.height_mm());
    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"");
    // Millimetres to three decimals so that pt → mm → pt round-trips.
    let _ = write!(out, "{}", fmt_mm(wmm));
    out.push_str("mm\" height=\"");
    let _ = write!(out, "{}", fmt_mm(hmm));
    out.push_str("mm\" viewBox=\"0 0 ");
    num(&mut out, scene.width);
    out.push(' ');
    num(&mut out, scene.height);
    let _ = write!(out, "\" font-family=\"{FAMILY}\">");
    for note in &scene.notes {
        let _ = write!(out, "<!-- {} -->", note.replace("--", "- -"));
    }
    if let Some(bg) = scene.background {
        out.push_str("<rect id=\"background\" width=\"");
        num(&mut out, scene.width);
        out.push_str("\" height=\"");
        num(&mut out, scene.height);
        out.push('"');
        color_attr(&mut out, "fill", bg);
        out.push_str("/>");
    }
    let mut clips = 0;
    write_group(&mut out, &scene.root, &mut clips);
    out.push_str("</svg>");
    out
}

fn fmt_mm(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(n(1.0), "1");
        assert_eq!(n(1.5), "1.5");
        assert_eq!(n(1.006), "1.01");
        assert_eq!(n(-0.25), "-0.25");
        assert_eq!(n(-0.001), "0");
        assert_eq!(n(240.944), "240.94");
        assert_eq!(n(10.1), "10.1");
    }
}
