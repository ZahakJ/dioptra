//! `estimation`: a Gardner–Altman plot (Gardner & Altman 1986; Ho et al. 2019). The raw
//! data sit on the left with mean ± SD drawn as a gapped line beside each group; on the
//! right, a floating axis anchored at the control mean shows each group's mean
//! difference from the first (control) group, its bootstrap distribution and its
//! bias-corrected and accelerated 95% CI (seeded, 5000 resamples by default).

use super::common::{jitter, series_layer, Cats};
use super::violin::{shape, Side};
use crate::plot::{AxisPlan, Ctx, Layer, Mark, Plot, Secondary};
use crate::scene::{Dash, MarkerShape, Paint, Stroke};
use crate::stats::{bootstrap_mean_diff, mean, sd};

/// Build an estimation plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let spec = ctx.spec;
    let t = &ctx.theme;
    let o = &spec.options;
    let cats = Cats::of(d);
    let k = d.series.len();
    let groups: Vec<Vec<f64>> = d.series.iter().map(|s| s.all_values()).collect();
    let mut labels = cats.labels.clone();
    let control_name = d.series.first().map_or(String::new(), |s| s.name.clone());
    for s in d.series.iter().skip(1) {
        labels.push(format!("{} − {}", s.name, control_name));
    }
    let mut p = Plot {
        x: AxisPlan::categorical(labels, ""),
        y: AxisPlan::continuous(&d.y_label),
        ..Default::default()
    };
    let cm = groups.first().map_or(f64::NAN, |g| mean(g));
    let all: Vec<f64> = groups.iter().flatten().copied().collect();
    let (lo_all, hi_all) = all
        .iter()
        .fold((f64::MAX, f64::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
    let gap = (hi_all - lo_all).abs().max(1e-12) * 0.012;
    // Raw data.
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, k, s.dataset, &s.name);
        let mut l = series_layer(i, s.dataset);
        let c = i as f64 + 1.0;
        let v = &groups[i];
        let off = jitter(v, 0.44 * o.jitter, o.seed.wrapping_add(i as u64));
        l.marks.push(Mark::Points {
            pts: v
                .iter()
                .zip(&off)
                .map(|(y, f)| (c - 0.06 + f, *y))
                .collect(),
            shape: st.marker.unwrap_or(MarkerShape::Circle),
            size: st.size,
            paint: st.marker_paint(t.background),
        });
        let (m, s_d) = (mean(v), sd(v));
        if m.is_finite() && s_d.is_finite() {
            let gx = c + 0.3;
            let stroke = Stroke::solid(ctx.fg(), t.data_pt * 1.4);
            l.marks.push(Mark::Segment {
                p1: (gx, m - s_d),
                p2: (gx, m - gap),
                stroke,
            });
            l.marks.push(Mark::Segment {
                p1: (gx, m + gap),
                p2: (gx, m + s_d),
                stroke,
            });
        }
        p.layers.push(l);
    }
    // Differences.
    let mut range = (0.0f64, 0.0f64);
    if cm.is_finite() {
        let mut refl = Layer::new("reference", "/options");
        let last = (2 * k).saturating_sub(1).max(1) as f64 + 0.45;
        refl.marks.push(Mark::Segment {
            p1: (0.75, cm),
            p2: (last, cm),
            stroke: Stroke::solid(t.reference, t.axis_pt).dashed(Dash::Dashed),
        });
        p.layers.push(refl);
        for j in 1..k {
            let Some(b) = bootstrap_mean_diff(
                &groups[0],
                &groups[j],
                o.bootstrap,
                o.seed.wrapping_add(j as u64),
            ) else {
                continue;
            };
            let st = ctx.style(j, k, d.series[j].dataset, &d.series[j].name);
            let pos = (k + j) as f64;
            let mut l = Layer::new(format!("difference-{j}"), "/options/bootstrap".to_string());
            let shifted: Vec<f64> = b.replicates.iter().map(|r| r + cm).collect();
            let (poly, _) = shape(&shifted, pos - 0.02, 0.38, Side::Right, None, false);
            l.marks.push(Mark::Polygon {
                pts: poly,
                paint: Paint::fill(if t.mono { t.grid } else { ctx.tint(st.colour) }),
            });
            l.marks.push(Mark::Segment {
                p1: (j as f64 + 1.0, cm + b.estimate),
                p2: (pos, cm + b.estimate),
                stroke: Stroke::solid(t.reference, t.axis_pt * 0.8).dashed(Dash::Dotted),
            });
            l.marks.push(Mark::Segment {
                p1: (pos, cm + b.lo),
                p2: (pos, cm + b.hi),
                stroke: Stroke::solid(ctx.fg(), t.data_pt * 1.6),
            });
            l.marks.push(Mark::Points {
                pts: vec![(pos, cm + b.estimate)],
                shape: MarkerShape::Circle,
                size: t.marker_pt * 1.5,
                paint: Paint::fill(ctx.fg()),
            });
            range.0 = range.0.min(b.lo).min(b.replicates[0]);
            range.1 = range
                .1
                .max(b.hi)
                .max(*b.replicates.last().expect("replicates"));
            p.layers.push(l);
        }
        p.y2 = Some(Secondary {
            offset: cm,
            title: if spec.options.summary == crate::spec::Summary::Median {
                "Difference".into()
            } else {
                "Mean difference".into()
            },
            range,
        });
        p.notes.push(format!(
            "estimation: {} bootstrap resamples, seed {}, BCa 95% intervals",
            o.bootstrap.max(100),
            o.seed
        ));
    }
    p
}
