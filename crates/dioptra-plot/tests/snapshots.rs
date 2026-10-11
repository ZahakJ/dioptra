//! Golden SVG files for every family (instrument theme) and for scatter and column in all
//! three themes, plus a few axis features. Output must match byte for byte: the engine
//! is deterministic (fixed seeds, no hash-map ordering, bundled font metrics).
//!
//! Regenerate with `UPDATE_SNAPSHOTS=1 cargo test -p dioptra-plot --test snapshots`.

use std::path::PathBuf;

use dioptra_plot::samples::sample;
use dioptra_plot::spec::{AxisBreak, AxisOrigin, FrameStyle, LegendPosition, Scale, TickDirection};
use dioptra_plot::{data::GraphData, render_svg, spec::GraphSpec};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn check(name: &str, spec: &GraphSpec, data: &GraphData) -> Option<String> {
    let svg = render_svg(spec, data).expect("render").svg;
    let path = dir().join(format!("{name}.svg"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(dir()).expect("dir");
        std::fs::write(&path, &svg).expect("write snapshot");
        return None;
    }
    match std::fs::read_to_string(&path) {
        Ok(old) if old == svg => None,
        Ok(_) => Some(format!("{name}: output differs from {}", path.display())),
        Err(_) => Some(format!(
            "{name}: no snapshot at {} (run with UPDATE_SNAPSHOTS=1)",
            path.display()
        )),
    }
}

const FAMILIES: [&str; 24] = [
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

#[test]
fn every_family_matches_its_snapshot() {
    let mut problems = Vec::new();
    for f in FAMILIES {
        let (spec, data) = sample(f).expect("sample");
        problems.extend(check(&format!("{f}-instrument"), &spec, &data));
    }
    for theme in ["night", "journal"] {
        for f in ["scatter", "column", "column_grouped"] {
            let (mut spec, data) = sample(f).expect("sample");
            spec.style.theme = theme.into();
            problems.extend(check(&format!("{f}-{theme}"), &spec, &data));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn axis_features_match_their_snapshots() {
    let mut problems = Vec::new();
    // Log axis with minors and a box frame, ticks in.
    let (mut spec, mut data) = sample("scatter").unwrap();
    for s in &mut data.series {
        s.x = s.x.iter().map(|x| 10f64.powf(x / 4.0)).collect();
    }
    data.curves.clear();
    spec.axes.x.scale = Scale::Log10;
    spec.axes.x.ticks.minor = 1;
    spec.axes.x.title = Some("Dose (nM)".into());
    spec.axes.frame = Some(FrameStyle::Box);
    spec.axes.x.ticks.direction = TickDirection::In;
    spec.axes.y.ticks.direction = TickDirection::In;
    spec.legend.position = LegendPosition::InsideTopLeft;
    spec.title = Some("Dose response".into());
    problems.extend(check("feature-log-box", &spec, &data));
    // A broken Y axis.
    let (mut spec, data) = sample("column").unwrap();
    spec.annotations.clear();
    spec.axes.y.breaks = vec![AxisBreak { from: 2.0, to: 5.0 }];
    spec.axes.y.min = Some(0.0);
    problems.extend(check("feature-break", &spec, &data));
    // Axes through zero, gridlines, a reference line and a text label.
    let (mut spec, data) = sample("residuals").unwrap();
    spec.family = "scatter".into();
    spec.axes.origin = AxisOrigin::Zero;
    spec.axes.grid = Some(true);
    spec.annotations = vec![
        dioptra_plot::spec::Annotation::ReferenceLine {
            axis: "y".into(),
            value: 10.0,
            label: Some("LLOQ".into()),
            dashed: true,
        },
        dioptra_plot::spec::Annotation::TextLabel {
            text: "R^2 = 0.98".into(),
            x: 2.0,
            y: 15.0,
            size_pt: None,
        },
    ];
    problems.extend(check("feature-origin-grid", &spec, &data));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn rendering_is_deterministic() {
    for f in ["estimation", "strip", "volcano", "heatmap"] {
        let (spec, data) = sample(f).unwrap();
        let a = render_svg(&spec, &data).unwrap().svg;
        let b = render_svg(&spec, &data).unwrap().svg;
        assert_eq!(a, b, "{f}");
    }
}
