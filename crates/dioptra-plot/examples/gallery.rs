//! Render every sample graph to SVG and PNG for eyeballing.
//!
//! `cargo run -p dioptra-plot --example gallery -- <out-dir> [theme]`

use dioptra_plot::{render_svg, samples::sample, scene, to_png};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| ".scratch/gallery".into());
    let theme = args.next().unwrap_or_else(|| "instrument".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let names = [
        "scatter",
        "line",
        "column",
        "column_grouped",
        "box",
        "violin",
        "strip",
        "raincloud",
        "before_after",
        "estimation",
        "superplot",
        "histogram",
        "density",
        "heatmap",
        "kaplan_meier",
        "roc",
        "bland_altman",
        "volcano",
        "forest",
        "correlation_matrix",
        "pie",
        "donut",
        "qq",
        "residuals",
    ];
    for n in names {
        let (mut spec, data) = sample(n).expect("sample");
        spec.style.theme = theme.clone();
        let r = render_svg(&spec, &data).expect("render");
        std::fs::write(format!("{out}/{n}.svg"), &r.svg).expect("write");
        let png = to_png(&scene(&spec, &data).expect("scene"), 200.0).expect("png");
        std::fs::write(format!("{out}/{n}.png"), png).expect("write");
    }
}
