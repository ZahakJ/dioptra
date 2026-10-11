//! `residuals`: observed minus fitted against X. The fit is the series' curve in
//! `GraphData::curves` when there is one (interpolated), otherwise a least-squares line.

use super::common::{marker_entry, series_layer};
use crate::data::Curve;
use crate::plot::{AxisPlan, Ctx, Layer, Mark, Plot};
use crate::scene::{Dash, MarkerShape, Stroke};
use crate::stats::linear_fit;

fn interp(c: &Curve, x: f64) -> f64 {
    let pts: Vec<(f64, f64)> = c.x.iter().copied().zip(c.y.iter().copied()).collect();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (x >= a.0 && x <= b.0) || (x <= a.0 && x >= b.0) {
            if b.0 == a.0 {
                return a.1;
            }
            return a.1 + (x - a.0) / (b.0 - a.0) * (b.1 - a.1);
        }
    }
    f64::NAN
}

/// Build a residual plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let mut p = Plot {
        x: AxisPlan::continuous(&d.x_label),
        y: AxisPlan::continuous("Residual"),
        ..Default::default()
    };
    let mut zero = Layer::new("zero", "/mapping");
    zero.marks.push(Mark::HLine {
        y: 0.0,
        stroke: Stroke::solid(t.reference, t.axis_pt).dashed(Dash::Dashed),
    });
    p.layers.push(zero);
    let n = d.series.len();
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        // Pairs (x, observed).
        let mut obs: Vec<(f64, f64)> = Vec::new();
        for (k, x) in s.x.iter().enumerate() {
            for y in s.values_at(k) {
                obs.push((*x, y));
            }
        }
        let curve = d.curves.iter().find(|c| c.dataset == Some(s.dataset));
        let fit: Box<dyn Fn(f64) -> f64> = match curve {
            Some(c) => Box::new(move |x| interp(c, x)),
            None => {
                let xs: Vec<f64> = obs.iter().map(|o| o.0).collect();
                let ys: Vec<f64> = obs.iter().map(|o| o.1).collect();
                match linear_fit(&xs, &ys) {
                    Some((a, b)) => Box::new(move |x| a + b * x),
                    None => continue,
                }
            }
        };
        let pts: Vec<(f64, f64)> = obs
            .iter()
            .map(|(x, y)| (*x, y - fit(*x)))
            .filter(|(_, r)| r.is_finite())
            .collect();
        let mut l = series_layer(i, s.dataset);
        l.marks.push(Mark::Points {
            pts,
            shape: st.marker.unwrap_or(MarkerShape::Circle),
            size: st.size,
            paint: st.marker_paint(t.background),
        });
        p.layers.push(l);
        p.legend.push(marker_entry(ctx, &st));
    }
    p
}
