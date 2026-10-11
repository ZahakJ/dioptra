//! `kaplan_meier`: survival step curves in percent with censor ticks, 95% CI bands
//! (exponential Greenwood) and a number-at-risk table under the X axis. Each series holds
//! X = time and Y = event code (1 = event, 0 = censored). Precomputed curves in
//! `GraphData::curves` (survival as fractions or percent) are drawn instead when present.

use crate::plot::{AtRiskRow, AxisPlan, Ctx, Layer, LegendEntry, Mark, Plot, Swatch};
use crate::scene::{MarkerShape, Paint, Stroke};
use crate::stats::kaplan_meier;

/// Build a Kaplan–Meier plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let o = &ctx.spec.options;
    let mut p = Plot {
        x: AxisPlan {
            include: vec![0.0],
            ..AxisPlan::continuous(if d.x_label.is_empty() {
                "Time"
            } else {
                &d.x_label
            })
        },
        y: AxisPlan {
            include: vec![0.0, 100.0],
            exact: true,
            major: Some(25.0),
            ..AxisPlan::continuous("Percent survival")
        },
        legend_auto: Some(crate::spec::LegendPosition::InsideTopRight),
        ..Default::default()
    };
    let n = d.series.len().max(d.curves.len());
    if !d.curves.is_empty() {
        for (i, c) in d.curves.iter().enumerate() {
            let ds = c.dataset.unwrap_or(i);
            let st = ctx.style(i, n, ds, &c.name);
            let k = if c.y.iter().any(|v| *v > 1.0) {
                1.0
            } else {
                100.0
            };
            let mut l = Layer::new(format!("series-{i}"), format!("/style/datasets/{ds}"));
            if o.ci_band && c.lo.len() == c.x.len() && c.hi.len() == c.x.len() {
                l.marks.push(Mark::Band {
                    x: c.x.clone(),
                    lo: c.lo.iter().map(|v| v * k).collect(),
                    hi: c.hi.iter().map(|v| v * k).collect(),
                    fill: st.colour.alpha(0.14),
                    step: c.step,
                });
            }
            l.marks.push(Mark::Line {
                pts: c.x.iter().zip(&c.y).map(|(x, y)| (*x, y * k)).collect(),
                stroke: st.stroke(),
                step: c.step,
            });
            p.layers.push(l);
            p.legend.push(LegendEntry {
                label: st.label.clone(),
                swatch: Swatch::Line(st.stroke(), None),
                dataset: Some(ds),
            });
        }
        for tb in &d.tables {
            let st = ctx.style(tb.dataset.min(n.saturating_sub(1)), n, tb.dataset, &tb.name);
            p.at_risk.push(AtRiskRow {
                name: tb.name.clone(),
                colour: st.colour,
                observed: vec![],
                fixed: tb
                    .times
                    .iter()
                    .copied()
                    .zip(tb.counts.iter().copied())
                    .collect(),
            });
        }
        return p;
    }
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let km = kaplan_meier(&s.x, &s.y);
        if km.observed.is_empty() {
            continue;
        }
        let mut l = Layer::new(
            format!("series-{i}"),
            format!("/style/datasets/{}", s.dataset),
        );
        if o.ci_band {
            l.marks.push(Mark::Band {
                x: km.times.clone(),
                lo: km.lo.iter().map(|v| v * 100.0).collect(),
                hi: km.hi.iter().map(|v| v * 100.0).collect(),
                fill: st.colour.alpha(if t.mono { 0.08 } else { 0.11 }),
                step: true,
            });
        }
        l.marks.push(Mark::Line {
            pts: km
                .times
                .iter()
                .zip(&km.survival)
                .map(|(x, y)| (*x, y * 100.0))
                .collect(),
            stroke: st.stroke(),
            step: true,
        });
        if o.censor_ticks && !km.censored.is_empty() {
            l.marks.push(Mark::Points {
                pts: km.censored.iter().map(|(x, y)| (*x, y * 100.0)).collect(),
                shape: MarkerShape::Tick,
                size: t.marker_pt * 1.1,
                paint: Paint::stroke(Stroke::solid(st.colour, st.line_pt)),
            });
        }
        p.layers.push(l);
        p.legend.push(LegendEntry {
            label: st.label.clone(),
            swatch: Swatch::Line(st.stroke(), None),
            dataset: Some(s.dataset),
        });
        if o.at_risk {
            p.at_risk.push(AtRiskRow {
                name: st.label.clone(),
                colour: st.colour,
                observed: km.observed.clone(),
                fixed: vec![],
            });
        }
    }
    p
}
