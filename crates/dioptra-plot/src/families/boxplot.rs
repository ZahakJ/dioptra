//! `box`: Tukey box plots (hinges at the quartiles, whiskers to the most extreme value
//! within 1.5 IQR, outliers as points), optional notches (median ± 1.58 IQR/√n).

use super::common::{fill_entry, row_jitter, row_values, series_layer, y_title, Cats};
use super::connect::{assemble, Placed};
use crate::plot::{AxisPlan, Ctx, Layer, Mark, Plot, SeriesStyle};
use crate::scene::{Color, MarkerShape, Paint, Stroke};
use crate::stats::{quantile_sorted, sorted};

/// Five-number summary with Tukey whiskers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tukey {
    /// Lower quartile.
    pub q1: f64,
    /// Median.
    pub median: f64,
    /// Upper quartile.
    pub q3: f64,
    /// Lower whisker end.
    pub lo: f64,
    /// Upper whisker end.
    pub hi: f64,
}

/// Tukey summary of values (type-7 quartiles).
pub fn tukey(v: &[f64]) -> Option<Tukey> {
    let s = sorted(v);
    if s.is_empty() {
        return None;
    }
    let (q1, median, q3) = (
        quantile_sorted(&s, 0.25),
        quantile_sorted(&s, 0.5),
        quantile_sorted(&s, 0.75),
    );
    let iqr = q3 - q1;
    let lo = s
        .iter()
        .copied()
        .find(|x| *x >= q1 - 1.5 * iqr)
        .unwrap_or(s[0]);
    let hi = s
        .iter()
        .rev()
        .copied()
        .find(|x| *x <= q3 + 1.5 * iqr)
        .unwrap_or(s[s.len() - 1]);
    Some(Tukey {
        q1,
        median,
        q3,
        lo,
        hi,
    })
}

/// Draw one box at `c` with width `bw` into a layer.
#[allow(clippy::too_many_arguments)]
pub fn draw_box(
    l: &mut Layer,
    v: &[f64],
    c: f64,
    bw: f64,
    fill: Color,
    line: Color,
    lw: f64,
    notch: bool,
) -> Option<Tukey> {
    let tk = tukey(v)?;
    let n = v.iter().filter(|x| x.is_finite()).count() as f64;
    let st = Stroke::solid(line, lw);
    let (x0, x1) = (c - bw / 2.0, c + bw / 2.0);
    l.marks.push(Mark::Segment {
        p1: (c, tk.q3),
        p2: (c, tk.hi),
        stroke: st,
    });
    l.marks.push(Mark::Segment {
        p1: (c, tk.q1),
        p2: (c, tk.lo),
        stroke: st,
    });
    let cap = bw * 0.25;
    l.marks.push(Mark::Segment {
        p1: (c - cap, tk.hi),
        p2: (c + cap, tk.hi),
        stroke: st,
    });
    l.marks.push(Mark::Segment {
        p1: (c - cap, tk.lo),
        p2: (c + cap, tk.lo),
        stroke: st,
    });
    if notch && n > 1.0 {
        let half = 1.58 * (tk.q3 - tk.q1) / n.sqrt();
        let (a, b) = ((tk.median - half).max(tk.q1), (tk.median + half).min(tk.q3));
        let inner = bw / 4.0;
        l.marks.push(Mark::Polygon {
            pts: vec![
                (x0, tk.q3),
                (x1, tk.q3),
                (x1, b),
                (c + inner, tk.median),
                (x1, a),
                (x1, tk.q1),
                (x0, tk.q1),
                (x0, a),
                (c - inner, tk.median),
                (x0, b),
            ],
            paint: Paint::both(fill, st),
        });
        l.marks.push(Mark::Segment {
            p1: (c - inner, tk.median),
            p2: (c + inner, tk.median),
            stroke: Stroke::solid(line, lw * 1.6),
        });
    } else {
        l.marks.push(Mark::Rect {
            x0,
            x1,
            y0: tk.q1,
            y1: tk.q3,
            paint: Paint::both(fill, st),
        });
        l.marks.push(Mark::Segment {
            p1: (x0, tk.median),
            p2: (x1, tk.median),
            stroke: Stroke::solid(line, lw * 1.6),
        });
    }
    Some(tk)
}

fn outline(ctx: &Ctx<'_>, st: &SeriesStyle) -> Color {
    if ctx.theme.mono {
        ctx.theme.axis
    } else {
        st.colour
    }
}

/// Build a box plot.
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
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let fill = if t.mono {
            t.background
        } else {
            ctx.tint(st.colour)
        };
        let line = outline(ctx, &st);
        let mut l = series_layer(i, s.dataset);
        let mut over = series_layer(i, s.dataset);
        for (k, x) in s.x.iter().enumerate() {
            let Some((c, w)) = cats.slot(i, *x) else {
                continue;
            };
            let v = s.values_at(k);
            let bw = w * spec
                .options
                .bar_width
                .map_or(if cats.clustered { 0.8 } else { 0.6 }, |b| {
                    b.clamp(0.05, 1.0)
                });
            let Some(tk) = draw_box(&mut l, &v, c, bw, fill, line, t.axis_pt, spec.options.notch)
            else {
                continue;
            };
            let shape = st.marker.unwrap_or(MarkerShape::Circle);
            let raw = row_values(s, k);
            let off = if spec.mapping.points {
                row_jitter(
                    &raw,
                    bw * 0.6 * spec.options.jitter,
                    spec.options.seed.wrapping_add(i as u64 * 13 + k as u64),
                )
            } else {
                vec![0.0; raw.len()]
            };
            placed.push(Placed::new(i, c, &raw, &off));
            if spec.mapping.points {
                over.marks.push(Mark::Points {
                    pts: raw
                        .iter()
                        .zip(&off)
                        .filter(|(y, _)| y.is_finite())
                        .map(|(y, o)| (c + o, *y))
                        .collect(),
                    shape,
                    size: st.size * 0.7,
                    paint: Paint::fill(line.alpha(0.75)),
                });
            } else {
                let out: Vec<(f64, f64)> = v
                    .iter()
                    .filter(|y| **y < tk.lo || **y > tk.hi)
                    .map(|y| (c, *y))
                    .collect();
                over.marks.push(Mark::Points {
                    pts: out,
                    shape,
                    size: st.size * 0.8,
                    paint: Paint::both(t.background, Stroke::solid(line, 0.6)),
                });
            }
        }
        bodies.push(l);
        marks.push(over);
        if cats.clustered {
            p.legend.push(fill_entry(ctx, &st, fill));
        }
    }
    assemble(ctx, &mut p, &cats, bodies, marks, &placed);
    p
}
