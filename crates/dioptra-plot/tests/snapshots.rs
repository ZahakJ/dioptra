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

/// A two-panel layout: panel letters in their gutters, clear of the tick labels.
#[test]
fn two_panel_layout_matches_its_snapshot() {
    let a = sample("column").unwrap();
    let b = sample("scatter").unwrap();
    let page = dioptra_plot::compose_layout(
        &[
            dioptra_plot::scene(&a.0, &a.1).unwrap(),
            dioptra_plot::scene(&b.0, &b.1).unwrap(),
        ],
        2,
        true,
    );
    let svg = dioptra_plot::to_svg(&page);
    let path = dir().join("layout-two-panel.svg");
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &svg).expect("write snapshot");
        return;
    }
    let old = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "no snapshot at {} (run with UPDATE_SNAPSHOTS=1)",
            path.display()
        )
    });
    assert!(old == svg, "layout differs from {}", path.display());
}

#[test]
fn connecting_lines_match_their_snapshots() {
    let mut problems = Vec::new();
    let matrix = dioptra_plot::samples::connecting_matrix();
    assert!(
        matrix.len() >= 40,
        "the matrix covers styles × modes and variants"
    );
    for (name, spec, data) in &matrix {
        problems.extend(check(name, spec, data));
    }
    for theme in ["night", "journal"] {
        let (_, mut spec, data) = matrix
            .iter()
            .find(|m| m.0 == "connect-bar_points-both")
            .cloned()
            .unwrap();
        spec.style.theme = theme.into();
        problems.extend(check(
            &format!("connect-bar_points-both-{theme}"),
            &spec,
            &data,
        ));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

mod connecting {
    use dioptra_plot::samples::{connecting_matrix, sample};
    use dioptra_plot::spec::{Connect, MissingLines};
    use dioptra_plot::{render_svg, scene, to_pdf, to_png};

    fn find(name: &str) -> (dioptra_plot::spec::GraphSpec, dioptra_plot::data::GraphData) {
        let (_, s, d) = connecting_matrix()
            .into_iter()
            .find(|m| m.0 == name)
            .unwrap_or_else(|| panic!("no {name}"));
        (s, d)
    }

    /// Ids of the groups inside the plot area, in drawing order.
    fn layer_ids(svg: &str) -> Vec<String> {
        let area = &svg[svg.find("id=\"plot-area\"").unwrap()..];
        let mut out = Vec::new();
        let mut rest = area;
        while let Some(i) = rest.find("<g id=\"") {
            rest = &rest[i + 7..];
            let id = &rest[..rest.find('"').unwrap()];
            if id.starts_with("axis") || id == "grid" || id == "frame" || id == "legend" {
                break;
            }
            out.push(id.to_string());
        }
        out
    }

    #[test]
    fn layers_stack_bars_lines_points_summary() {
        let (spec, data) = find("connect-bar_points-both");
        let ids = layer_ids(&render_svg(&spec, &data).unwrap().svg);
        let pos = |p: &str| ids.iter().position(|i| i.starts_with(p)).unwrap();
        let last_bar = ids
            .iter()
            .rposition(|i| i.starts_with("series-") && !i.ends_with("-points"));
        assert!(last_bar.unwrap() < pos("subject-"), "{ids:?}");
        let last_subject = ids.iter().rposition(|i| i.starts_with("subject-")).unwrap();
        assert!(last_subject < pos("series-0-points"), "{ids:?}");
        assert_eq!(ids.last().unwrap(), "connect-summary", "{ids:?}");
    }

    #[test]
    fn missing_values_gap_or_skip() {
        let (mut spec, data) = sample("paired").unwrap();
        // Subject 4 has no week-4 value: with gaps its two halves are each one point
        // long, so it draws nothing; skipping joins baseline to week 8.
        let gap = render_svg(&spec, &data).unwrap();
        assert!(!gap.svg.contains("id=\"subject-4\""));
        assert!(gap.svg.contains("id=\"subject-3\""));
        spec.options.connecting.missing = MissingLines::Skip;
        let skip = render_svg(&spec, &data).unwrap();
        assert!(skip.svg.contains("id=\"subject-4\""));
        assert!(skip
            .targets
            .iter()
            .any(|t| t.id == "subject-4" && t.target == "/options/connecting"));
    }

    fn polylines(svg: &str, id: &str) -> Vec<Vec<(f64, f64)>> {
        let start = svg.find(&format!("id=\"{id}\"")).unwrap();
        let g = &svg[start..start + svg[start..].find("</g>").unwrap()];
        g.split("points=\"")
            .skip(1)
            .map(|p| {
                p[..p.find('"').unwrap()]
                    .split(' ')
                    .map(|xy| {
                        let (x, y) = xy.split_once(',').unwrap();
                        (x.parse().unwrap(), y.parse().unwrap())
                    })
                    .collect()
            })
            .collect()
    }

    fn circles(svg: &str, id: &str) -> Vec<(f64, f64)> {
        let start = svg.find(&format!("id=\"{id}\"")).unwrap();
        let g = &svg[start..start + svg[start..].find("</g></g>").unwrap()];
        g.split("<circle cx=\"")
            .skip(1)
            .map(|c| {
                let x: f64 = c[..c.find('"').unwrap()].parse().unwrap();
                let c = &c[c.find("cy=\"").unwrap() + 4..];
                (x, c[..c.find('"').unwrap()].parse().unwrap())
            })
            .collect()
    }

    #[test]
    fn jittered_lines_meet_their_points() {
        for name in [
            "connect-jitter",
            "connect-jitter-strip",
            "connect-horizontal-box",
        ] {
            let (spec, data) = find(name);
            let svg = render_svg(&spec, &data).unwrap().svg;
            let pts: Vec<(f64, f64)> = (0..3)
                .flat_map(|i| circles(&svg, &format!("series-{i}-points")))
                .collect();
            let mut checked = 0;
            for r in 1..=10 {
                if !svg.contains(&format!("id=\"subject-{r}\"")) {
                    continue;
                }
                for line in polylines(&svg, &format!("subject-{r}")) {
                    for v in line {
                        assert!(
                            pts.iter()
                                .any(|p| (p.0 - v.0).abs() < 0.02 && (p.1 - v.1).abs() < 0.02),
                            "{name}: subject {r} vertex {v:?} has no point"
                        );
                        checked += 1;
                    }
                }
            }
            assert!(checked >= 20, "{name}: {checked}");
        }
    }

    #[test]
    fn grouped_lines_stay_in_their_group() {
        let (spec, data) = find("connect-grouped-within");
        let svg = render_svg(&spec, &data).unwrap().svg;
        // Three groups of five subjects; subject 3 on day 7 is missing under the drug.
        let n = svg.matches("id=\"subject-g").count();
        assert_eq!(n, 14);
        let (spec, data) = find("connect-grouped-across");
        let svg = render_svg(&spec, &data).unwrap().svg;
        assert_eq!(svg.matches("id=\"subject-s0-").count(), 5);
        // The drug's subject 3 misses day 7, so with gaps its line has no two neighbours.
        assert_eq!(svg.matches("id=\"subject-s1-").count(), 4);
    }

    #[test]
    fn horizontal_axis_targets_follow_the_spec() {
        let (spec, data) = find("connect-horizontal");
        let r = render_svg(&spec, &data).unwrap();
        // The bottom axis draws the values: it edits `axes.y`.
        let bottom = r.targets.iter().find(|t| t.id == "axis-x").unwrap();
        assert_eq!(bottom.target, "/axes/y");
        assert!(r.svg.contains("Systolic pressure"));
    }

    #[test]
    fn stacked_bars_draw_no_lines() {
        let (mut spec, data) = sample("paired_grouped").unwrap();
        spec.options.arrangement = dioptra_plot::spec::Arrangement::Stacked;
        spec.options.connecting.connect = Connect::Both;
        let svg = render_svg(&spec, &data).unwrap().svg;
        assert!(!svg.contains("subject-") && !svg.contains("connect-summary"));
    }

    #[test]
    fn pdf_and_png_draw_the_same_scene() {
        let (spec, data) = find("connect-bar_points-both");
        let sc = scene(&spec, &data).unwrap();
        let pdf = to_pdf(&sc).unwrap();
        assert!(pdf.starts_with(b"%PDF-1."));
        let with = to_png(&sc, 150.0).unwrap();
        let mut off = spec.clone();
        off.options.connecting.connect = Connect::None;
        let without = to_png(&scene(&off, &data).unwrap(), 150.0).unwrap();
        assert_eq!(&with[16..24], &without[16..24], "same pixel size");
        assert_ne!(with, without);
    }

    #[test]
    fn the_legacy_family_draws_its_strip_equivalent() {
        let (spec, data) = sample("paired").unwrap();
        let mut legacy = spec.clone();
        legacy.family = "before_after".into();
        legacy.options.connecting = Default::default();
        let mut upgraded = legacy.clone();
        assert!(upgraded.upgrade_legacy());
        assert_eq!(
            render_svg(&legacy, &data).unwrap().svg,
            render_svg(&upgraded, &data).unwrap().svg
        );
    }
}
