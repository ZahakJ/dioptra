//! `volcano`: log₂ fold change against −log₁₀ p, with threshold lines, up/down/not
//! significant colouring and labels for the most significant points.
//!
//! Y values that are all in (0, 1] are read as p values and transformed; otherwise they
//! are taken to be −log₁₀ p already.

use crate::detmath::DetMath;
use crate::plot::{AxisPlan, Callout, Ctx, Layer, LegendEntry, Mark, Plot, Swatch};
use crate::scene::{Color, Dash, MarkerShape, Paint, Stroke};
use crate::text;

/// Build a volcano plot from the first series.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let o = &ctx.spec.options;
    let mut p = Plot {
        x: AxisPlan::continuous("log_2 fold change"),
        y: AxisPlan {
            include: vec![0.0],
            ..AxisPlan::continuous("−log_{10} p")
        },
        legend_auto: Some(crate::spec::LegendPosition::Top),
        ..Default::default()
    };
    let Some(s) = d.series.first() else { return p };
    let are_p =
        s.y.iter()
            .filter(|v| v.is_finite())
            .all(|v| *v > 0.0 && *v <= 1.0);
    let pts: Vec<(usize, f64, f64)> =
        s.x.iter()
            .zip(&s.y)
            .enumerate()
            .filter(|(_, (x, y))| x.is_finite() && y.is_finite())
            .map(|(i, (x, y))| (i, *x, if are_p { -y.max(1e-300).log10_d() } else { *y }))
            .collect();
    let ythr = -o.p_threshold.max(1e-300).log10_d();
    let (up, down, ns): (Color, Color, Color) = if t.mono {
        (t.axis, t.axis, Color::rgb(0xaa, 0xaa, 0xaa))
    } else {
        (
            Color::rgb(0xD5, 0x5E, 0x00),
            Color::rgb(0x00, 0x72, 0xB2),
            if t.name == "night" {
                Color::rgb(0x5a, 0x60, 0x5d)
            } else {
                Color::rgb(0xbb, 0xbb, 0xbb)
            },
        )
    };
    let mut groups: [Vec<(f64, f64)>; 3] = [vec![], vec![], vec![]];
    let mut sig: Vec<(usize, f64, f64)> = Vec::new();
    for (i, x, y) in &pts {
        let g = if *y >= ythr && *x >= o.fc_threshold {
            0
        } else if *y >= ythr && *x <= -o.fc_threshold {
            1
        } else {
            2
        };
        groups[g].push((*x, *y));
        if g < 2 {
            sig.push((*i, *x, *y));
        }
    }
    let mut th = Layer::new("thresholds", "/options/p_threshold");
    let st = Stroke::solid(t.reference, t.axis_pt * 0.8).dashed(Dash::Dashed);
    th.marks.push(Mark::HLine {
        y: ythr,
        stroke: st,
    });
    th.marks.push(Mark::VLine {
        x: o.fc_threshold,
        stroke: st,
    });
    th.marks.push(Mark::VLine {
        x: -o.fc_threshold,
        stroke: st,
    });
    p.layers.push(th);
    let names = ["Up", "Down", "Not significant"];
    let size = t.marker_pt * 0.8;
    for (g, (c, name)) in [up, down, ns].iter().zip(names).enumerate().rev() {
        let mut l = Layer::new(format!("series-{g}"), "/options/fc_threshold".to_string());
        let paint = if t.mono && g == 1 {
            Paint::both(t.background, Stroke::solid(*c, t.fixed(0.6)))
        } else {
            Paint::fill(c.alpha(if g == 2 { 0.7 } else { 0.9 }))
        };
        let count = groups[g].len();
        l.marks.push(Mark::Points {
            pts: std::mem::take(&mut groups[g]),
            shape: MarkerShape::Circle,
            size,
            paint,
        });
        p.layers.push(l);
        p.legend.insert(
            0,
            LegendEntry {
                label: format!("{name} ({count})"),
                swatch: Swatch::Marker(MarkerShape::Circle, paint, size * 1.2),
                dataset: None,
            },
        );
    }
    if !s.labels.is_empty() && o.label_top > 0 {
        sig.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        let mut l = Layer::new("labels", "/options/label_top");
        let mut list = Vec::new();
        for (i, x, y) in sig.iter().take(o.label_top) {
            let Some(name) = s.labels.get(*i).filter(|n| !n.is_empty()) else {
                continue;
            };
            list.push(Callout {
                x: *x,
                y: *y,
                runs: text::parse(name),
                size: t.tick_pt * 0.85,
                color: t.text,
            });
        }
        l.marks.push(Mark::Callouts(list));
        p.layers.push(l);
    }
    p
}
