//! `before_after`: paired values joined by a line per subject (rows align across
//! datasets), points coloured by condition, and an optional heavy mean (or median) line.

use super::common::{centre, series_layer, y_title, Cats};
use crate::plot::{AxisPlan, Ctx, Layer, Mark, Plot};
use crate::scene::{MarkerShape, Paint, Stroke};

/// Build a before–after plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let t = &ctx.theme;
    let cats = Cats::of(d);
    let mut p = Plot {
        x: AxisPlan::categorical(cats.labels.clone(), ""),
        y: AxisPlan::continuous(&y_title(d)),
        ..Default::default()
    };
    // Column (position, raw row-aligned values) per series, first X only.
    let cols: Vec<(f64, Vec<f64>)> = d
        .series
        .iter()
        .filter_map(|s| {
            let x = *s.x.first()?;
            let c = cats.index(x)?;
            let v = s.replicates.first().cloned().unwrap_or_else(|| s.y.clone());
            Some((c, v))
        })
        .collect();
    let rows = cols.iter().map(|c| c.1.len()).max().unwrap_or(0);
    let mut lines = Layer::new("pairs", "/mapping");
    let pair_colour = ctx.fg().alpha(0.4);
    for r in 0..rows {
        let pts: Vec<(f64, f64)> = cols
            .iter()
            .filter_map(|(c, v)| v.get(r).copied().filter(|y| y.is_finite()).map(|y| (*c, y)))
            .collect();
        if pts.len() >= 2 {
            lines.marks.push(Mark::Line {
                pts,
                stroke: Stroke::solid(pair_colour, t.axis_pt),
                step: false,
            });
        }
    }
    p.layers.push(lines);
    let n = d.series.len();
    let mut means = Vec::new();
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let mut l = series_layer(i, s.dataset);
        if let Some((c, v)) =
            s.x.first()
                .and_then(|x| cats.index(*x))
                .map(|c| (c, s.values_at(0)))
        {
            l.marks.push(Mark::Points {
                pts: v.iter().map(|y| (c, *y)).collect(),
                shape: st.marker.unwrap_or(MarkerShape::Circle),
                size: st.size,
                paint: st.marker_paint(t.background),
            });
            let m = centre(&v, spec.options.summary);
            if m.is_finite() {
                means.push((c, m));
            }
        }
        p.layers.push(l);
    }
    if spec.options.mean_line && means.len() >= 2 {
        let mut l = Layer::new("summary", "/options/mean_line");
        l.marks.push(Mark::Line {
            pts: means.clone(),
            stroke: Stroke::solid(ctx.fg(), t.data_pt * 2.2),
            step: false,
        });
        l.marks.push(Mark::Points {
            pts: means,
            shape: MarkerShape::Square,
            size: t.marker_pt * 1.4,
            paint: Paint::fill(ctx.fg()),
        });
        p.layers.push(l);
    }
    p
}
