//! Timing benchmarks (ignored by default; numbers go into docs/PERFORMANCE.md):
//! `cargo test -p dioptra-plot --release --test bench -- --ignored --nocapture`

use std::time::Instant;

use dioptra_plot::data::{GraphData, Series, TableKind};
use dioptra_plot::samples::sample;
use dioptra_plot::spec::GraphSpec;
use dioptra_plot::stats::Rng;
use dioptra_plot::{render_svg, scene, to_pdf, to_png};

fn cloud(n: usize) -> (GraphSpec, GraphData) {
    let mut rng = Rng::new(1);
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    for _ in 0..n {
        let a = rng.uniform() * 10.0;
        x.push(a);
        y.push(a * 0.8 + (rng.uniform() + rng.uniform() + rng.uniform() - 1.5) * 3.0);
    }
    let spec = GraphSpec::new("bench".into(), "scatter");
    let data = GraphData {
        series: vec![Series::xy("Cloud", 0, x, y)],
        x_label: "X".into(),
        y_label: "Y".into(),
        kind: TableKind::Xy,
        ..Default::default()
    };
    (spec, data)
}

/// Median wall time in milliseconds over `runs` runs (after one warm-up).
fn time<F: FnMut() -> usize>(runs: usize, mut f: F) -> (f64, usize) {
    let mut out = f();
    let mut v = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        out = f();
        v.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    v.sort_by(f64::total_cmp);
    (v[v.len() / 2], out)
}

#[test]
#[ignore]
fn bench_render() {
    for n in [10_000, 100_000] {
        let (spec, data) = cloud(n);
        let (ms, bytes) = time(15, || render_svg(&spec, &data).unwrap().svg.len());
        println!("scatter {n} points → SVG: {ms:.1} ms ({bytes} bytes)");
    }
    let (spec, data) = sample("column").unwrap();
    let (ms, _) = time(30, || render_svg(&spec, &data).unwrap().svg.len());
    println!("column sample → SVG: {ms:.2} ms");
    let sc = scene(&spec, &data).unwrap();
    let (ms, bytes) = time(15, || to_png(&sc, 300.0).unwrap().len());
    println!("column 85 × 70 mm → PNG 300 dpi: {ms:.1} ms ({bytes} bytes)");
    let (ms, bytes) = time(15, || to_pdf(&sc).unwrap().len());
    println!("column 85 × 70 mm → PDF: {ms:.1} ms ({bytes} bytes)");
    let (spec, data) = cloud(10_000);
    let sc = scene(&spec, &data).unwrap();
    let (ms, _) = time(9, || to_png(&sc, 300.0).unwrap().len());
    println!("scatter 10,000 points → PNG 300 dpi: {ms:.1} ms");
    let (ms, bytes) = time(9, || to_pdf(&sc).unwrap().len());
    println!("scatter 10,000 points → PDF: {ms:.1} ms ({bytes} bytes)");
    let (spec, data) = sample("volcano").unwrap();
    let (ms, _) = time(15, || render_svg(&spec, &data).unwrap().svg.len());
    println!("volcano 1,500 genes → SVG: {ms:.2} ms");
}
