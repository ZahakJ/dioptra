//! Rasterise SVG files written by the engine (for looking at snapshots).
//!
//! `cargo run -p dioptra-plot --example svg2png -- <dpi> <file.svg>...` writes `<file>.png`.

fn mm(svg: &str, attr: &str) -> f64 {
    let key = format!("{attr}=\"");
    let i = svg.find(&key).expect("size attribute") + key.len();
    let j = svg[i..].find("mm\"").expect("mm unit");
    svg[i..i + j].parse().expect("number")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dpi: f64 = args.next().and_then(|d| d.parse().ok()).unwrap_or(150.0);
    for path in args {
        let svg = std::fs::read_to_string(&path).expect("read");
        let png =
            dioptra_plot::output::svg_to_png(&svg, mm(&svg, "width"), mm(&svg, "height"), dpi)
                .expect("png");
        std::fs::write(path.replace(".svg", ".png"), png).expect("write");
    }
}
