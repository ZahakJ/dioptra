//! `raincloud` (Allen et al. 2019): a half violin (the cloud) to the right of each
//! category, a slim box in the middle and the jittered points (the rain) to the left.

use super::boxplot::tukey;
use super::common::{jitter, series_layer, y_title, Cats};
use super::violin::{shape, Side};
use crate::plot::{AxisPlan, Ctx, Mark, Plot};
use crate::scene::{MarkerShape, Paint, Stroke};

/// Build a raincloud plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let t = &ctx.theme;
    let o = &spec.options;
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
        let fill = if t.mono {
            t.background
        } else {
            ctx.tint(st.colour)
        };
        let line = if t.mono { t.axis } else { st.colour };
        let mut l = series_layer(i, s.dataset);
        for (k, x) in s.x.iter().enumerate() {
            let Some((c, w)) = cats.slot(i, *x) else {
                continue;
            };
            let v = s.values_at(k);
            if v.is_empty() {
                continue;
            }
            // Cloud.
            if v.len() >= 2 {
                let (poly, _) = shape(
                    &v,
                    c - w * 0.02,
                    w * 0.5,
                    Side::Right,
                    o.bandwidth,
                    o.truncate,
                );
                l.marks.push(Mark::Polygon {
                    pts: poly,
                    paint: Paint::both(fill, Stroke::solid(line, t.axis_pt)),
                });
            }
            // Box.
            let bc = c - w * 0.1;
            if let Some(tk) = tukey(&v) {
                let bw = w * 0.07;
                let st2 = Stroke::solid(t.axis, t.axis_pt);
                l.marks.push(Mark::Segment {
                    p1: (bc, tk.lo),
                    p2: (bc, tk.q1),
                    stroke: st2,
                });
                l.marks.push(Mark::Segment {
                    p1: (bc, tk.q3),
                    p2: (bc, tk.hi),
                    stroke: st2,
                });
                l.marks.push(Mark::Rect {
                    x0: bc - bw / 2.0,
                    x1: bc + bw / 2.0,
                    y0: tk.q1,
                    y1: tk.q3,
                    paint: Paint::both(t.background, st2),
                });
                l.marks.push(Mark::Segment {
                    p1: (bc - bw / 2.0, tk.median),
                    p2: (bc + bw / 2.0, tk.median),
                    stroke: Stroke::solid(t.axis, t.axis_pt * 1.6),
                });
            }
            // Rain.
            let rc = c - w * 0.3;
            let off = jitter(
                &v,
                w * 0.22 * o.jitter,
                o.seed.wrapping_add(i as u64 * 19 + k as u64),
            );
            l.marks.push(Mark::Points {
                pts: v.iter().zip(&off).map(|(y, f)| (rc + f, *y)).collect(),
                shape: st.marker.unwrap_or(MarkerShape::Circle),
                size: st.size * 0.75,
                paint: Paint::fill(line.alpha(0.85)),
            });
        }
        p.layers.push(l);
    }
    p
}
