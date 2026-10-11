//! `roc`: empirical ROC curves (higher values indicate disease), the chance diagonal and
//! the AUC in the legend. The first series holds the controls; every later series is a
//! patient group compared with them. Precomputed curves in `GraphData::curves` (as
//! fractions or percent) are drawn instead when present.

use crate::plot::{AxisPlan, Ctx, Layer, LegendEntry, Mark, Plot, Swatch};
use crate::scene::{Dash, Stroke};
use crate::stats::roc;

/// Name, dataset, points in percent and AUC (when computed here).
type RocCurve = (String, usize, Vec<(f64, f64)>, Option<f64>);

/// Build an ROC plot.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let ax = |s: &str| AxisPlan {
        include: vec![0.0, 100.0],
        exact: true,
        major: Some(25.0),
        ..AxisPlan::continuous(s)
    };
    let mut p = Plot {
        x: ax("100% − Specificity%"),
        y: ax("Sensitivity%"),
        aspect: Some(1.0),
        legend_always: true,
        legend_auto: Some(crate::spec::LegendPosition::InsideBottomRight),
        ..Default::default()
    };
    let mut diag = Layer::new("chance", "/mapping");
    diag.marks.push(Mark::Segment {
        p1: (0.0, 0.0),
        p2: (100.0, 100.0),
        stroke: Stroke::solid(t.reference, t.axis_pt).dashed(Dash::Dashed),
    });
    p.layers.push(diag);
    let mut curves: Vec<RocCurve> = Vec::new();
    if !d.curves.is_empty() {
        for c in &d.curves {
            let pct = c.x.iter().chain(&c.y).any(|v| *v > 1.0);
            let k = if pct { 1.0 } else { 100.0 };
            curves.push((
                c.name.clone(),
                c.dataset.unwrap_or(0),
                c.x.iter().zip(&c.y).map(|(x, y)| (x * k, y * k)).collect(),
                None,
            ));
        }
    } else if let Some(ctrl) = d.series.first() {
        let cv = ctrl.all_values();
        for s in d.series.iter().skip(1) {
            let (pts, auc) = roc(&cv, &s.all_values());
            if pts.is_empty() {
                continue;
            }
            curves.push((
                s.name.clone(),
                s.dataset,
                pts.into_iter()
                    .map(|(x, y)| (x * 100.0, y * 100.0))
                    .collect(),
                Some(auc),
            ));
        }
    }
    let n = curves.len();
    for (i, (name, ds, pts, auc)) in curves.into_iter().enumerate() {
        let st = ctx.style(i, n, ds, &name);
        let stroke = Stroke {
            width: t.data_pt * 1.2,
            ..st.stroke()
        };
        let mut l = Layer::new(format!("series-{i}"), format!("/style/datasets/{ds}"));
        l.marks.push(Mark::Line {
            pts,
            stroke,
            step: false,
        });
        p.layers.push(l);
        let label = match auc {
            Some(a) => format!("{} (AUC = {:.3})", st.label, a),
            None => st.label.clone(),
        };
        p.legend.push(LegendEntry {
            label,
            swatch: Swatch::Line(stroke, None),
            dataset: Some(ds),
        });
    }
    p
}
