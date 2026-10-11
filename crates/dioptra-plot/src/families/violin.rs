//! `violin`: Gaussian kernel density (Silverman bandwidth unless set) mirrored about the
//! category, cut at the data range by default, with a slim box, the points or quartile
//! lines inside.

use super::boxplot::tukey;
use super::common::{fill_entry, jitter, series_layer, y_title, Cats};
use crate::plot::{AxisPlan, Ctx, Mark, Plot};
use crate::scene::{Dash, MarkerShape, Paint, Stroke};
use crate::spec::ViolinInner;
use crate::stats::{density_curve, quantile_sorted, sorted};

/// A list of (x, y) points.
pub type Points = Vec<(f64, f64)>;

/// Which side(s) of the centre a density is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Both (a violin).
    Both,
    /// Right only (a raincloud's cloud).
    Right,
}

/// Outline of a density at `c` whose widest point is `half` wide. Returns the polygon and
/// a function-like table (y, half-width) for placing things inside.
pub fn shape(
    v: &[f64],
    c: f64,
    half: f64,
    side: Side,
    bw: Option<f64>,
    truncate: bool,
) -> (Points, Points) {
    let (grid, dens) = density_curve(v, bw, truncate, 80);
    if grid.is_empty() {
        return (vec![], vec![]);
    }
    let max = dens.iter().fold(0.0f64, |a, b| a.max(*b)).max(1e-300);
    let prof: Vec<(f64, f64)> = grid
        .iter()
        .zip(&dens)
        .map(|(y, d)| (*y, d / max * half))
        .collect();
    let mut pts: Vec<(f64, f64)> = prof.iter().map(|(y, w)| (c + w, *y)).collect();
    match side {
        Side::Both => {
            for (y, w) in prof.iter().rev() {
                pts.push((c - w, *y));
            }
        }
        Side::Right => {
            pts.push((c, prof[prof.len() - 1].0));
            pts.push((c, prof[0].0));
        }
    }
    (pts, prof)
}

/// Half-width of a profile at y (linear interpolation).
pub fn width_at(prof: &[(f64, f64)], y: f64) -> f64 {
    for w in prof.windows(2) {
        if y >= w[0].0 && y <= w[1].0 {
            let f = if w[1].0 > w[0].0 {
                (y - w[0].0) / (w[1].0 - w[0].0)
            } else {
                0.0
            };
            return w[0].1 + f * (w[1].1 - w[0].1);
        }
    }
    0.0
}

/// Build a violin plot.
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
    let o = &spec.options;
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
            if v.len() < 2 {
                continue;
            }
            let half = w * 0.45;
            let (poly, prof) = shape(&v, c, half, Side::Both, o.bandwidth, o.truncate);
            l.marks.push(Mark::Polygon {
                pts: poly,
                paint: Paint::both(fill, Stroke::solid(line, t.axis_pt)),
            });
            let inner = if spec.mapping.points && o.violin_inner == ViolinInner::Box {
                // `points` on a violin means "show the data": a strip inside.
                ViolinInner::Strip
            } else {
                o.violin_inner
            };
            match inner {
                ViolinInner::Box => {
                    if let Some(tk) = tukey(&v) {
                        let bw = (half * 0.18).max(0.02);
                        let dark = if t.mono {
                            t.axis
                        } else {
                            t.axis.mix(st.colour, 0.25)
                        };
                        l.marks.push(Mark::Segment {
                            p1: (c, tk.lo),
                            p2: (c, tk.hi),
                            stroke: Stroke::solid(dark, t.axis_pt),
                        });
                        l.marks.push(Mark::Rect {
                            x0: c - bw / 2.0,
                            x1: c + bw / 2.0,
                            y0: tk.q1,
                            y1: tk.q3,
                            paint: Paint::fill(dark),
                        });
                        l.marks.push(Mark::Points {
                            pts: vec![(c, tk.median)],
                            shape: MarkerShape::Circle,
                            size: (st.size * 0.6).max(2.0),
                            paint: Paint::fill(t.background),
                        });
                    }
                }
                ViolinInner::Strip => {
                    let off = jitter(&v, 1.0, o.seed.wrapping_add(i as u64 * 17 + k as u64));
                    let pts = v
                        .iter()
                        .zip(&off)
                        .map(|(y, f)| (c + f * width_at(&prof, *y) * 0.8, *y))
                        .collect();
                    l.marks.push(Mark::Points {
                        pts,
                        shape: st.marker.unwrap_or(MarkerShape::Circle),
                        size: st.size * 0.65,
                        paint: Paint::fill(line),
                    });
                }
                ViolinInner::Quartiles => {
                    let s2 = sorted(&v);
                    for (q, dash) in [
                        (0.25, Dash::Dashed),
                        (0.5, Dash::Solid),
                        (0.75, Dash::Dashed),
                    ] {
                        let y = quantile_sorted(&s2, q);
                        let hw = width_at(&prof, y);
                        l.marks.push(Mark::Segment {
                            p1: (c - hw, y),
                            p2: (c + hw, y),
                            stroke: Stroke::solid(line, t.axis_pt).dashed(dash),
                        });
                    }
                }
                ViolinInner::None => {}
            }
        }
        p.layers.push(l);
        if cats.clustered {
            p.legend.push(fill_entry(ctx, &st, fill));
        }
    }
    p
}
