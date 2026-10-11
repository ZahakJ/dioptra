//! `density`: Gaussian kernel density estimates (Silverman bandwidth unless set), one
//! outlined curve with a light fill per series.

use super::common::series_layer;
use crate::plot::{AxisPlan, Ctx, LegendEntry, Mark, Plot, Swatch};
use crate::stats::density_curve;

/// Build a density plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let o = &ctx.spec.options;
    let t = &ctx.theme;
    let mut p = Plot {
        x: AxisPlan::continuous(&d.y_label),
        y: AxisPlan {
            include: vec![0.0],
            ..AxisPlan::continuous("Density")
        },
        ..Default::default()
    };
    let n = d.series.len();
    for (i, s) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, s.dataset, &s.name);
        let v = s.all_values();
        if v.len() < 2 {
            continue;
        }
        let (grid, dens) = density_curve(&v, o.bandwidth, false, 200);
        let mut l = series_layer(i, s.dataset);
        l.marks.push(Mark::Band {
            x: grid.clone(),
            lo: vec![0.0; grid.len()],
            hi: dens.clone(),
            fill: st.colour.alpha(if t.mono { 0.08 } else { 0.15 }),
            step: false,
        });
        l.marks.push(Mark::Line {
            pts: grid.into_iter().zip(dens).collect(),
            stroke: st.stroke(),
            step: false,
        });
        p.layers.push(l);
        p.legend.push(LegendEntry {
            label: st.label.clone(),
            swatch: Swatch::Line(st.stroke(), None),
            dataset: Some(s.dataset),
        });
    }
    p
}
