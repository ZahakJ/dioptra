//! `column`: bars of the mean (or median) with error bars and the data points on top by
//! default; grouped, stacked or percent-stacked for grouped tables.

use super::common::{
    bar_stroke, centre, fill_entry, jitter, push_error, series_error, series_layer, y_title, Cats,
};
use crate::plot::{AxisPlan, Ctx, Mark, Plot};
use crate::scene::{MarkerShape, Paint, Stroke};
use crate::spec::Arrangement;

/// Build a column plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let t = &ctx.theme;
    let cats = Cats::of(d);
    let arrangement = if cats.clustered {
        spec.options.arrangement
    } else {
        Arrangement::Grouped
    };
    let stacked = arrangement != Arrangement::Grouped;
    let mut p = Plot {
        x: AxisPlan::categorical(
            cats.labels.clone(),
            if cats.clustered { &d.x_label } else { "" },
        ),
        y: AxisPlan {
            include: vec![0.0],
            ..AxisPlan::continuous(&if arrangement == Arrangement::Percent {
                "Percent".to_string()
            } else {
                y_title(d)
            })
        },
        ..Default::default()
    };
    let n = d.series.len();
    // Totals per category for percent stacking; running bases for stacking.
    let mut totals = vec![0.0f64; cats.xs.len() + 1];
    let summary = |s: &crate::data::Series, k: usize| {
        if s.replicates.is_empty() {
            s.y.get(k).copied().unwrap_or(f64::NAN)
        } else {
            centre(&s.values_at(k), spec.options.summary)
        }
    };
    for s in &d.series {
        for (k, x) in s.x.iter().enumerate() {
            if let Some(c) = cats.index(*x) {
                let v = summary(s, k);
                if v.is_finite() {
                    totals[c as usize - 1] += v;
                }
            }
        }
    }
    let mut bases = vec![0.0f64; cats.xs.len() + 1];
    let log_y = matches!(
        spec.axes.y.scale,
        crate::spec::Scale::Log10 | crate::spec::Scale::Log2
    );
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let fill = if t.mono {
            if cats.clustered {
                st.colour.mix(t.background, 0.15 * (i % 2) as f64)
            } else {
                t.background
            }
        } else {
            ctx.tint(st.colour)
        };
        let outline = if t.mono { t.axis } else { st.colour };
        let mut l = series_layer(i, s.dataset);
        let mut errs = Vec::new();
        for (k, x) in s.x.iter().enumerate() {
            let Some((c, w)) = (if stacked {
                cats.index(*x).map(|c| (c, 0.84))
            } else {
                cats.slot(i, *x)
            }) else {
                continue;
            };
            let mut m = summary(s, k);
            if !m.is_finite() {
                continue;
            }
            let bw = if cats.clustered && !stacked {
                w * 0.88
            } else {
                w * 0.78
            };
            let ci = cats.index(*x).map_or(0, |v| v as usize - 1);
            let (y0, y1) = if stacked {
                if arrangement == Arrangement::Percent && totals[ci] != 0.0 {
                    m = m / totals[ci] * 100.0;
                }
                let b = bases[ci];
                bases[ci] += m;
                (b, b + m)
            } else {
                (if log_y { f64::NAN } else { 0.0 }, m)
            };
            l.marks.push(Mark::Rect {
                x0: c - bw / 2.0,
                x1: c + bw / 2.0,
                y0,
                y1,
                paint: Paint::both(fill, Stroke::solid(outline, t.axis_pt)),
            });
            if stacked {
                continue;
            }
            if spec.mapping.points && !s.replicates.is_empty() {
                let vals = s.values_at(k);
                let off = jitter(
                    &vals,
                    bw * 0.64 * spec.options.jitter,
                    spec.options.seed.wrapping_add(i as u64 * 7 + k as u64),
                );
                let pts: Vec<(f64, f64)> =
                    vals.iter().zip(&off).map(|(v, o)| (c + o, *v)).collect();
                let shape = st.marker.unwrap_or(MarkerShape::Circle);
                l.marks.push(Mark::Points {
                    pts,
                    shape,
                    size: st.size * 0.8,
                    paint: if t.mono {
                        Paint::both(t.background, Stroke::solid(t.axis, 0.6))
                    } else {
                        Paint::fill(st.colour)
                    },
                });
            }
            if let Some((lo, hi)) = series_error(s, k, m, spec.mapping.error_bars) {
                // On a dark bar the lower half would vanish: draw the upper half only.
                let dark = (fill.luminance() - t.background.luminance()).abs() > 0.6;
                errs.push((c, if dark { m } else { lo }, hi, bw));
            }
        }
        for (c, lo, hi, _bw) in errs {
            push_error(&mut l, c, lo, hi, 5.0, bar_stroke(ctx));
        }
        p.layers.push(l);
        if cats.clustered {
            p.legend.push(fill_entry(ctx, &st, fill));
        }
    }
    p
}
