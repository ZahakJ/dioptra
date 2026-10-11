//! `forest`: one row per estimate (top to bottom) with its CI as a horizontal line, a
//! reference line (0, or 1 on log axes), and the numbers printed in a column on the
//! right. Estimates come from the first series' Y with `error` bounds, or from three
//! series read as estimate, lower and upper.

use crate::plot::{AxisPlan, Ctx, Layer, Mark, Plot, TextColumn};
use crate::scene::{Dash, MarkerShape, Paint, Stroke};
use crate::spec::Scale;

fn fmt(v: f64) -> String {
    let a = v.abs();
    let d = if a >= 100.0 {
        0
    } else if a >= 10.0 {
        1
    } else {
        2
    };
    crate::ticks::decimal(v, d)
}

/// Build a forest plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let o = &ctx.spec.options;
    let mut rows: Vec<(String, f64, f64, f64)> = Vec::new();
    if let Some(s) = d.series.first() {
        let label = |k: usize| -> String {
            if let Some(l) = s.labels.get(k).filter(|l| !l.is_empty()) {
                return l.clone();
            }
            let x = s.x.get(k).copied().unwrap_or(k as f64 + 1.0);
            let i = x.round() as usize;
            d.categories
                .get(i.wrapping_sub(1))
                .cloned()
                .unwrap_or_else(|| format!("Row {}", k + 1))
        };
        if !s.error.is_empty() {
            for k in 0..s.y.len() {
                let (lo, hi) = s.error.get(k).copied().unwrap_or((f64::NAN, f64::NAN));
                rows.push((label(k), s.y[k], lo, hi));
            }
        } else if d.series.len() >= 3 {
            let (a, b) = (&d.series[1], &d.series[2]);
            for k in 0..s.y.len() {
                rows.push((
                    label(k),
                    s.y[k],
                    a.y.get(k).copied().unwrap_or(f64::NAN),
                    b.y.get(k).copied().unwrap_or(f64::NAN),
                ));
            }
        } else {
            for k in 0..s.y.len() {
                rows.push((label(k), s.y[k], f64::NAN, f64::NAN));
            }
        }
    }
    rows.retain(|r| r.1.is_finite());
    let log = matches!(ctx.spec.axes.x.scale, Scale::Log10 | Scale::Log2);
    let reference = o.reference.unwrap_or(if log { 1.0 } else { 0.0 });
    let mut p = Plot {
        x: AxisPlan {
            include: vec![reference],
            ..AxisPlan::continuous(if d.y_label.is_empty() {
                "Estimate"
            } else {
                &d.y_label
            })
        },
        y: AxisPlan {
            reverse: true,
            bare: true,
            ..AxisPlan::categorical(rows.iter().map(|r| r.0.clone()).collect(), "")
        },
        right_text: Some(TextColumn {
            header: "Estimate (95% CI)".into(),
            rows: rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let s = if r.2.is_finite() && r.3.is_finite() {
                        format!("{} ({} to {})", fmt(r.1), fmt(r.2), fmt(r.3))
                    } else {
                        fmt(r.1)
                    };
                    (i as f64 + 1.0, s)
                })
                .collect(),
        }),
        ..Default::default()
    };
    let mut refl = Layer::new("reference", "/options/reference");
    refl.marks.push(Mark::VLine {
        x: reference,
        stroke: Stroke::solid(t.reference, t.axis_pt).dashed(Dash::Dashed),
    });
    p.layers.push(refl);
    let st = ctx.style(0, 1, d.series.first().map_or(0, |s| s.dataset), "");
    let colour = if t.mono || st.colour == crate::scene::Color::rgb(0, 0, 0) {
        ctx.fg()
    } else {
        st.colour
    };
    let mut l = Layer::new(
        "series-0",
        format!(
            "/style/datasets/{}",
            d.series.first().map_or(0, |s| s.dataset)
        ),
    );
    for (i, r) in rows.iter().enumerate() {
        let y = i as f64 + 1.0;
        if r.2.is_finite() && r.3.is_finite() {
            l.marks.push(Mark::ErrorBar {
                at: y,
                lo: r.2,
                hi: r.3,
                horizontal: true,
                cap: 0.0,
                stroke: Stroke::solid(colour, t.data_pt),
            });
        }
        l.marks.push(Mark::Points {
            pts: vec![(r.1, y)],
            shape: MarkerShape::Square,
            size: t.marker_pt * 1.3,
            paint: Paint::fill(colour),
        });
    }
    p.layers.push(l);
    p
}
