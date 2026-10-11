//! Print the editor targets of a sample graph: `cargo run -p dioptra-plot --example targets -- column`.
fn main() {
    let f = std::env::args().nth(1).unwrap_or_else(|| "column".into());
    let (spec, data) = dioptra_plot::samples::sample(&f).expect("sample");
    let r = dioptra_plot::render_svg(&spec, &data).expect("render");
    for t in r.targets {
        println!(
            "{:<22} {:<26} x={:.1} y={:.1} w={:.1} h={:.1}",
            t.id, t.target, t.bbox.x, t.bbox.y, t.bbox.width, t.bbox.height
        );
    }
    println!("{}", serde_json::to_string(&spec).unwrap());
}
