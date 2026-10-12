//! `strip`: every point, jittered deterministically, with a mean (or median) line and
//! error bars — Prism's "scatter dot plot".

use super::common::{
    bar_stroke, centre, directed, marker_entry, push_error, row_jitter, row_values, series_error,
    series_layer, y_title, Cats,
};
use super::connect::{assemble, Placed};
use crate::plot::{AxisPlan, Ctx, Mark, Plot};
use crate::scene::{MarkerShape, Stroke};

/// Build a strip plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let t = &ctx.theme;
    let cats = Cats::for_spec(d, spec);
    let mut p = Plot {
        x: AxisPlan::categorical(
            cats.labels.clone(),
            if cats.clustered { &d.x_label } else { "" },
        ),
        y: AxisPlan::continuous(&y_title(d)),
        horizontal: spec.options.orientation == crate::spec::Orientation::Horizontal,
        ..Default::default()
    };
    let n = d.series.len();
    let (mut bodies, mut marks, mut placed) = (Vec::new(), Vec::new(), Vec::new());
    let cap = spec.options.cap_pt.unwrap_or(6.0).max(0.0);
    let hw_frac = spec.options.bar_width.map_or(0.56, |b| b.clamp(0.05, 1.0));
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let mut l = series_layer(i, s.dataset);
        for (k, x) in s.x.iter().enumerate() {
            let Some((c, w)) = cats.slot(i, *x) else {
                continue;
            };
            let v = s.values_at(k);
            if v.is_empty() {
                continue;
            }
            let raw = row_values(s, k);
            let off = row_jitter(
                &raw,
                w * 0.6 * spec.options.jitter,
                spec.options.seed.wrapping_add(i as u64 * 31 + k as u64),
            );
            placed.push(Placed::new(i, c, &raw, &off));
            if spec.mapping.points {
                l.marks.push(Mark::Points {
                    pts: raw
                        .iter()
                        .zip(&off)
                        .filter(|(y, _)| y.is_finite())
                        .map(|(y, o)| (c + o, *y))
                        .collect(),
                    shape: st.marker.unwrap_or(MarkerShape::Circle),
                    size: st.size,
                    paint: st.marker_paint(t.background),
                });
            }
            let m = centre(&v, spec.options.summary);
            if m.is_finite() {
                let hw = w * hw_frac / 2.0;
                if spec.options.summary_line || !spec.mapping.points {
                    l.marks.push(Mark::Segment {
                        p1: (c - hw, m),
                        p2: (c + hw, m),
                        stroke: Stroke::solid(ctx.fg(), t.data_pt * 1.25),
                    });
                }
                if let Some((lo, hi)) = series_error(s, k, m, spec.mapping.error_bars) {
                    let (lo, hi) = directed(lo, hi, m, spec.options.error_direction);
                    push_error(&mut l, c, lo, hi, cap, bar_stroke(ctx));
                }
            }
        }
        bodies.push(series_layer(i, s.dataset));
        marks.push(l);
        if cats.clustered {
            p.legend.push(marker_entry(ctx, &st));
        }
    }
    assemble(ctx, &mut p, &cats, bodies, marks, &placed);
    p
}
