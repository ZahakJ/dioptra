//! `strip`: every point, jittered deterministically, with a mean (or median) line and
//! error bars — Prism's "scatter dot plot".

use super::common::{
    bar_stroke, centre, jitter, marker_entry, push_error, series_error, series_layer, y_title, Cats,
};
use crate::plot::{AxisPlan, Ctx, Mark, Plot};
use crate::scene::{MarkerShape, Stroke};

/// Build a strip plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let t = &ctx.theme;
    let cats = Cats::of(d);
    let mut p = Plot {
        x: AxisPlan::categorical(
            cats.labels.clone(),
            if cats.clustered { &d.x_label } else { "" },
        ),
        y: AxisPlan::continuous(&y_title(d)),
        ..Default::default()
    };
    let n = d.series.len();
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
            let off = jitter(
                &v,
                w * 0.6 * spec.options.jitter,
                spec.options.seed.wrapping_add(i as u64 * 31 + k as u64),
            );
            l.marks.push(Mark::Points {
                pts: v.iter().zip(&off).map(|(y, o)| (c + o, *y)).collect(),
                shape: st.marker.unwrap_or(MarkerShape::Circle),
                size: st.size,
                paint: st.marker_paint(t.background),
            });
            let m = centre(&v, spec.options.summary);
            if m.is_finite() {
                let hw = w * 0.28;
                l.marks.push(Mark::Segment {
                    p1: (c - hw, m),
                    p2: (c + hw, m),
                    stroke: Stroke::solid(ctx.fg(), t.data_pt * 1.25),
                });
                if let Some((lo, hi)) = series_error(s, k, m, spec.mapping.error_bars) {
                    push_error(&mut l, c, lo, hi, 6.0, bar_stroke(ctx));
                }
            }
        }
        p.layers.push(l);
        if cats.clustered {
            p.legend.push(marker_entry(ctx, &st));
        }
    }
    p
}
