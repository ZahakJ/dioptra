//! Render option variants of sample graphs for eyeballing:
//! `cargo run -p dioptra-plot --example variants -- <out-dir>`.

use dioptra_plot::samples::sample;
use dioptra_plot::spec::{Arrangement, AxisOrigin, LegendPosition, ViolinInner};
use dioptra_plot::{scene, to_png};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".scratch/variants".into());
    std::fs::create_dir_all(&out).expect("dir");
    let mut all = Vec::new();
    let (mut s, d) = sample("column_grouped").unwrap();
    s.options.arrangement = Arrangement::Stacked;
    all.push(("stacked", s.clone(), d.clone()));
    s.options.arrangement = Arrangement::Percent;
    s.legend.position = LegendPosition::Bottom;
    all.push(("percent", s, d));
    let (mut s, d) = sample("box").unwrap();
    s.options.notch = true;
    s.mapping.points = true;
    all.push(("box-notch-points", s, d));
    let (mut s, d) = sample("violin").unwrap();
    s.options.violin_inner = ViolinInner::Quartiles;
    s.options.truncate = false;
    all.push(("violin-quartiles", s, d));
    let (mut s, mut d) = sample("scatter").unwrap();
    for se in &mut d.series {
        se.y.iter_mut().for_each(|v| *v -= 4.0);
        for r in &mut se.replicates {
            r.iter_mut().for_each(|v| *v -= 4.0);
        }
        se.x.iter_mut().for_each(|v| *v -= 5.0);
    }
    d.curves.clear();
    s.axes.origin = AxisOrigin::Zero;
    s.legend.position = LegendPosition::Top;
    all.push(("origin-zero", s, d));
    let (mut s, d) = sample("histogram").unwrap();
    let (_, d2) = sample("density").unwrap();
    s.options.density = true;
    all.push(("histogram-two", s, d2));
    let _ = d;
    for (n, s, d) in all {
        let png = to_png(&scene(&s, &d).expect("scene"), 150.0).expect("png");
        std::fs::write(format!("{out}/{n}.png"), png).expect("write");
    }
}
