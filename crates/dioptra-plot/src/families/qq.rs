//! `qq`: sample quantiles against normal quantiles (Blom positions (i − 3/8)/(n + 1/4)),
//! with a reference line through the quartiles.

use super::common::{marker_entry, series_layer};
use crate::plot::{AxisPlan, Ctx, Mark, Plot};
use crate::scene::{Dash, MarkerShape, Stroke};
use crate::stats::{norm_quantile, quantile_sorted, sorted};

/// Build a normal QQ plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let mut p = Plot {
        x: AxisPlan::continuous("Theoretical quantiles (normal)"),
        y: AxisPlan::continuous(if d.y_label.is_empty() {
            "Sample quantiles"
        } else {
            &d.y_label
        }),
        ..Default::default()
    };
    let n = d.series.len();
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let v = sorted(&s.all_values());
        let m = v.len();
        if m < 2 {
            continue;
        }
        let theo: Vec<f64> = (0..m)
            .map(|k| norm_quantile((k as f64 + 1.0 - 0.375) / (m as f64 + 0.25)))
            .collect();
        let mut l = series_layer(i, s.dataset);
        let (q1s, q3s) = (quantile_sorted(&v, 0.25), quantile_sorted(&v, 0.75));
        let (q1t, q3t) = (norm_quantile(0.25), norm_quantile(0.75));
        let slope = (q3s - q1s) / (q3t - q1t);
        let icpt = q1s - slope * q1t;
        let (a, b) = (theo[0], theo[m - 1]);
        l.marks.push(Mark::Segment {
            p1: (a, icpt + slope * a),
            p2: (b, icpt + slope * b),
            stroke: Stroke::solid(if t.mono { t.axis } else { st.colour }, t.axis_pt)
                .dashed(Dash::Dashed),
        });
        l.marks.push(Mark::Points {
            pts: theo.into_iter().zip(v).collect(),
            shape: st.marker.unwrap_or(MarkerShape::Circle),
            size: st.size,
            paint: st.marker_paint(t.background),
        });
        p.layers.push(l);
        p.legend.push(marker_entry(ctx, &st));
    }
    p
}
