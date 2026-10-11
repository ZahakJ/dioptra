//! `bland_altman` (Bland & Altman 1986): the difference of two methods against their
//! mean, with the bias and the 95% limits of agreement (bias ± 1.96 SD), each with its
//! 95% CI shaded (SE of the bias s/√n; of a limit √(3s²/n)).

use super::common::series_layer;
use crate::detmath::DetMath;
use crate::plot::{AxisPlan, Ctx, Layer, Mark, Plot};
use crate::scene::{Anchor, Baseline, Dash, MarkerShape, Stroke};
use crate::stats::{mean, sd, t_quantile};
use crate::text::{self, Weight};

/// Build a Bland–Altman plot from the first two series (paired by row).
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let (na, nb) = (
        d.series.first().map_or("A".to_string(), |s| s.name.clone()),
        d.series.get(1).map_or("B".to_string(), |s| s.name.clone()),
    );
    let mut p = Plot {
        x: AxisPlan::continuous(&format!("Mean of {na} and {nb}")),
        y: AxisPlan::continuous(&format!("Difference ({na} − {nb})")),
        ..Default::default()
    };
    let raw = |i: usize| -> Vec<f64> {
        d.series.get(i).map_or(vec![], |s| {
            if s.replicates.len() == 1 {
                s.replicates[0].clone()
            } else {
                s.y.clone()
            }
        })
    };
    let (a, b) = (raw(0), raw(1));
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(&b)
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| ((x + y) / 2.0, x - y))
        .collect();
    if pairs.len() < 3 {
        return p;
    }
    let diffs: Vec<f64> = pairs.iter().map(|p| p.1).collect();
    let n = pairs.len() as f64;
    let (bias, s) = (mean(&diffs), sd(&diffs));
    let tq = t_quantile(0.975, n - 1.0);
    let (lo, hi) = (bias - 1.96 * s, bias + 1.96 * s);
    let (se_b, se_l) = (s / n.sqrt(), (3.0 * s * s / n).sqrt());
    let (xmin, xmax) = pairs.iter().fold((f64::MAX, f64::MIN), |acc, p| {
        (acc.0.min(p.0), acc.1.max(p.0))
    });
    let pad = (xmax - xmin).max(1e-12) * 0.03;
    let (x0, x1) = (xmin - pad, xmax + pad);
    let st = ctx.style(0, 1, d.series[0].dataset, &d.series[0].name);
    let mut bands = Layer::new("agreement", "/options");
    let shade = if t.mono {
        t.grid
    } else {
        st.colour.alpha(0.07)
    };
    for (c, se) in [(bias, se_b), (lo, se_l), (hi, se_l)] {
        bands.marks.push(Mark::Band {
            x: vec![x0, x1],
            lo: vec![c - tq * se; 2],
            hi: vec![c + tq * se; 2],
            fill: shade,
            step: false,
        });
    }
    let fg = ctx.fg();
    bands.marks.push(Mark::Segment {
        p1: (x0, bias),
        p2: (x1, bias),
        stroke: Stroke::solid(fg, t.axis_pt),
    });
    for v in [lo, hi] {
        bands.marks.push(Mark::Segment {
            p1: (x0, v),
            p2: (x1, v),
            stroke: Stroke::solid(fg, t.axis_pt).dashed(Dash::Dashed),
        });
    }
    let fmt = |v: f64| {
        let mag = v.abs().max(s.abs()).max(1e-12);
        let dec = (2.0 - mag.log10_d().floor()).clamp(0.0, 6.0) as usize;
        crate::ticks::decimal(v, dec)
    };
    for (v, name) in [(hi, "+1.96 SD"), (bias, "Bias"), (lo, "−1.96 SD")] {
        bands.marks.push(Mark::Label {
            x: x1,
            y: v,
            dx: -1.0,
            dy: -2.0,
            runs: text::parse(&format!("{name} {}", fmt(v))),
            size: t.tick_pt * 0.9,
            color: t.muted,
            anchor: Anchor::End,
            baseline: Baseline::Alphabetic,
            weight: Weight::Regular,
        });
    }
    p.layers.push(bands);
    let mut l = series_layer(0, d.series[0].dataset);
    l.marks.push(Mark::Points {
        pts: pairs,
        shape: st.marker.unwrap_or(MarkerShape::Circle),
        size: st.size,
        paint: st.marker_paint(t.background),
    });
    p.layers.push(l);
    p
}
