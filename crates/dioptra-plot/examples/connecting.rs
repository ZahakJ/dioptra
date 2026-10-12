//! Render the connecting-line matrix (`samples::connecting_matrix`) to SVG and PNG.
//!
//! `cargo run -p dioptra-plot --example connecting -- <out-dir> [theme] [dpi]`

use dioptra_plot::{render_svg, samples::connecting_matrix, scene, to_png};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| ".scratch/connecting".into());
    let theme = args.next().filter(|t| t != "-");
    let dpi: f64 = args.next().and_then(|d| d.parse().ok()).unwrap_or(150.0);
    std::fs::create_dir_all(&out).expect("out dir");
    for (name, mut spec, data) in connecting_matrix() {
        if let Some(t) = &theme {
            spec.style.theme = t.clone();
        }
        let svg = render_svg(&spec, &data).expect("render").svg;
        std::fs::write(format!("{out}/{name}.svg"), svg).expect("write svg");
        let png = to_png(&scene(&spec, &data).expect("scene"), dpi).expect("png");
        std::fs::write(format!("{out}/{name}.png"), png).expect("write png");
    }
}
