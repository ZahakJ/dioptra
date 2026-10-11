//! `superplot` (Lord et al. 2020): every observation as a small point coloured by its
//! biological replicate (subgroup), each replicate's mean as a large point in the same
//! colour, and the mean ± error of the replicate means — so the error bar reflects the
//! number of replicates, not the number of cells.

use super::common::{centre, error_bounds, jitter, Cats};
use crate::plot::{AxisPlan, Ctx, Layer, LegendEntry, Mark, Plot, Swatch};
use crate::scene::{MarkerShape, Paint, Stroke};
use crate::theme::MONO_MARKERS;

/// Build a SuperPlot.
#[allow(clippy::needless_range_loop)]
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let t = &ctx.theme;
    let cats = Cats::of(d);
    let mut p = Plot {
        x: AxisPlan::categorical(cats.labels.clone(), ""),
        y: AxisPlan::continuous(&d.y_label),
        legend_always: true,
        ..Default::default()
    };
    let ngroups = d
        .series
        .iter()
        .flat_map(|s| s.groups.iter().copied())
        .max()
        .map_or(1, |m| m + 1)
        .max(d.group_names.len())
        .max(1);
    let colours = ctx.colours(ngroups);
    let shape_of = |g: usize| {
        if t.mono {
            MONO_MARKERS[g % MONO_MARKERS.len()]
        } else {
            (MarkerShape::Circle, false)
        }
    };
    for (i, s) in d.series.iter().enumerate() {
        let mut l = Layer::new(
            format!("series-{i}"),
            format!("/style/datasets/{}", s.dataset),
        );
        let Some(c) = s.x.first().and_then(|x| cats.index(*x)) else {
            continue;
        };
        let raw: Vec<f64> = s.replicates.iter().flatten().copied().collect();
        let groups: Vec<usize> = if s.groups.len() == raw.len() {
            s.groups.clone()
        } else {
            vec![0; raw.len()]
        };
        let off = jitter(
            &raw,
            0.6 * spec.options.jitter,
            spec.options.seed.wrapping_add(i as u64 * 23),
        );
        let mut means = Vec::new();
        for g in 0..ngroups {
            let idx: Vec<usize> = (0..raw.len())
                .filter(|k| groups[*k] == g && raw[*k].is_finite())
                .collect();
            if idx.is_empty() {
                continue;
            }
            let (shape, open) = shape_of(g);
            let col = colours[g];
            l.marks.push(Mark::Points {
                pts: idx.iter().map(|k| (c + off[*k], raw[*k])).collect(),
                shape,
                size: t.marker_pt * 0.6,
                paint: if open {
                    Paint::stroke(Stroke::solid(col.alpha(0.6), 0.5))
                } else {
                    Paint::fill(col.alpha(0.45))
                },
            });
            let vals: Vec<f64> = idx.iter().map(|k| raw[*k]).collect();
            let m = centre(&vals, spec.options.summary);
            let mx = c + idx.iter().map(|k| off[*k]).sum::<f64>() / idx.len() as f64 * 0.6;
            means.push((g, mx, m));
        }
        for (g, mx, m) in &means {
            let (shape, _) = shape_of(*g);
            l.marks.push(Mark::Points {
                pts: vec![(*mx, *m)],
                shape,
                size: t.marker_pt * 1.9,
                paint: Paint::both(colours[*g], Stroke::solid(t.axis, 0.75)),
            });
        }
        let mv: Vec<f64> = means.iter().map(|m| m.2).collect();
        let grand = centre(&mv, spec.options.summary);
        if grand.is_finite() {
            l.marks.push(Mark::Segment {
                p1: (c - 0.3, grand),
                p2: (c + 0.3, grand),
                stroke: Stroke::solid(t.axis, t.data_pt * 1.4),
            });
            if let Some((lo, hi)) = error_bounds(&mv, grand, spec.mapping.error_bars) {
                l.marks.push(Mark::ErrorBar {
                    at: c,
                    lo,
                    hi,
                    horizontal: false,
                    cap: 7.0,
                    stroke: Stroke::solid(t.axis, t.axis_pt * 1.2),
                });
            }
        }
        p.layers.push(l);
    }
    for g in 0..ngroups {
        let (shape, _) = shape_of(g);
        p.legend.push(LegendEntry {
            label: d
                .group_names
                .get(g)
                .cloned()
                .unwrap_or_else(|| format!("Replicate {}", g + 1)),
            swatch: Swatch::Marker(
                shape,
                Paint::both(colours[g], Stroke::solid(t.axis, 0.6)),
                t.marker_pt * 1.4,
            ),
            dataset: None,
        });
    }
    p
}
