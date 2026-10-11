//! `scatter` (XY points, optional connecting line, error bars on both axes, fit curves
//! with bands) and `line` (the same with lines on).

use super::common::{centre, marker_entry, series_error, series_layer};
use crate::data::Curve;
use crate::plot::{AxisPlan, Ctx, Layer, LegendEntry, Mark, Plot, Swatch};
use crate::scene::{Dash, MarkerShape, Stroke};
use crate::spec::ErrorBars;

/// Curves (fits, bands) as layers under the data.
pub fn curve_layers(ctx: &Ctx<'_>, curves: &[Curve], plot: &mut Plot) -> Vec<LegendEntry> {
    let mut entries = Vec::new();
    let n = ctx.data.series.len();
    for (ci, c) in curves.iter().enumerate() {
        let colour = c
            .dataset
            .and_then(|d| {
                ctx.data
                    .series
                    .iter()
                    .position(|s| s.dataset == d)
                    .map(|i| ctx.style(i, n, d, "").colour)
            })
            .unwrap_or_else(|| ctx.fg());
        let mut l = Layer::new(format!("curve-{ci}"), "/mapping".to_string());
        if !c.lo.is_empty() && c.lo.len() == c.x.len() && c.hi.len() == c.x.len() {
            l.marks.push(Mark::Band {
                x: c.x.clone(),
                lo: c.lo.clone(),
                hi: c.hi.clone(),
                fill: colour.alpha(0.16),
                step: c.step,
            });
        }
        let st = Stroke::solid(colour, ctx.theme.data_pt);
        l.marks.push(Mark::Line {
            pts: c.x.iter().copied().zip(c.y.iter().copied()).collect(),
            stroke: st,
            step: c.step,
        });
        plot.layers.push(l);
        let named_like_series = ctx.data.series.iter().any(|s| s.name == c.name);
        if !c.name.is_empty() && !named_like_series {
            entries.push(LegendEntry {
                label: c.name.clone(),
                swatch: Swatch::Line(st, None),
                dataset: c.dataset,
            });
        }
    }
    entries
}

/// Build a scatter or line plot.
pub fn plot(ctx: &Ctx<'_>, line: bool) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let mut p = Plot {
        x: AxisPlan::continuous(&d.x_label),
        y: AxisPlan::continuous(&d.y_label),
        ..Default::default()
    };
    let curve_entries = curve_layers(ctx, &d.curves, &mut p);
    let n = d.series.len();
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let mut l = series_layer(i, s.dataset);
        let has_reps = s
            .replicates
            .iter()
            .any(|r| r.iter().filter(|v| v.is_finite()).count() > 1);
        let mut pts: Vec<(f64, f64)> = Vec::with_capacity(s.x.len());
        let bar = Stroke::solid(st.colour, ctx.theme.axis_pt);
        for (k, x) in s.x.iter().enumerate() {
            let y = if has_reps {
                centre(&s.values_at(k), spec.options.summary)
            } else {
                s.y.get(k).copied().unwrap_or(f64::NAN)
            };
            if !(x.is_finite() && y.is_finite()) {
                continue;
            }
            pts.push((*x, y));
            if let Some((lo, hi)) = series_error(s, k, y, spec.mapping.error_bars) {
                if has_reps || !s.error.is_empty() {
                    l.marks.push(Mark::ErrorBar {
                        at: *x,
                        lo,
                        hi,
                        horizontal: false,
                        cap: st.size,
                        stroke: bar,
                    });
                }
            }
            if let Some((lo, hi)) = s.x_error.get(k) {
                l.marks.push(Mark::ErrorBar {
                    at: y,
                    lo: *lo,
                    hi: *hi,
                    horizontal: true,
                    cap: st.size,
                    stroke: bar,
                });
            }
            if has_reps && spec.mapping.points && spec.mapping.error_bars == ErrorBars::None {
                let raw: Vec<(f64, f64)> = s.values_at(k).into_iter().map(|v| (*x, v)).collect();
                l.marks.push(Mark::Points {
                    pts: raw,
                    shape: st.marker.unwrap_or(MarkerShape::Circle),
                    size: st.size * 0.7,
                    paint: crate::scene::Paint::fill(st.colour.alpha(0.45)),
                });
            }
        }
        let connect = line || spec.options.connect;
        let stroke = st.stroke();
        if connect && st.dash.is_some() {
            let mut sorted = pts.clone();
            sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
            l.marks.insert(
                0,
                Mark::Line {
                    pts: sorted,
                    stroke,
                    step: false,
                },
            );
        }
        if let Some(shape) = st.marker {
            let size = if line { st.size * 0.85 } else { st.size };
            l.marks.push(Mark::Points {
                pts,
                shape,
                size,
                paint: st.marker_paint(ctx.theme.background),
            });
        }
        p.layers.push(l);
        p.legend.push(if connect && st.dash.is_some() {
            LegendEntry {
                label: st.label.clone(),
                swatch: Swatch::Line(
                    Stroke {
                        dash: st.dash.unwrap_or(Dash::Solid),
                        ..stroke
                    },
                    st.marker
                        .map(|m| (m, st.marker_paint(ctx.theme.background), st.size * 0.85)),
                ),
                dataset: Some(s.dataset),
            }
        } else {
            marker_entry(ctx, &st)
        });
    }
    p.legend.extend(curve_entries);
    p
}
