//! Layout battery: for many random specs (sizes, fonts, data magnitudes, scales, long
//! titles, legend positions), tick labels never overlap each other or the axis titles,
//! and titles and labels stay on the page.

use dioptra_plot::samples::sample;
use dioptra_plot::scene::{Group, Node, Rect, Scene};
use dioptra_plot::spec::{LegendPosition, Scale, TickDirection};
use dioptra_plot::stats::Rng;

fn find<'a>(g: &'a Group, id: &str) -> Option<&'a Group> {
    if g.id.as_deref() == Some(id) {
        return Some(g);
    }
    g.children.iter().find_map(|c| match c {
        Node::Group(cg) => find(cg, id),
        _ => None,
    })
}

fn texts(g: &Group, out: &mut Vec<Rect>) {
    for c in &g.children {
        match c {
            Node::Text(t) => out.push(t.bbox()),
            Node::Group(cg) => texts(cg, out),
            _ => {}
        }
    }
}

/// Text boxes in a frame turned with the text, so parallel rotated labels compare exactly
/// (their axis-aligned boxes would overlap even when the labels do not).
fn text_frames(g: &Group, out: &mut Vec<Rect>) {
    for c in &g.children {
        match c {
            Node::Text(t) => {
                let mut flat = t.clone();
                flat.rotate = 0.0;
                let b = flat.bbox();
                let (s, co) = (-t.rotate).to_radians().sin_cos();
                let (ax, ay) = (t.x * co - t.y * s, t.x * s + t.y * co);
                out.push(Rect::new(b.x - t.x + ax, b.y - t.y + ay, b.width, b.height));
            }
            Node::Group(cg) => text_frames(cg, out),
            _ => {}
        }
    }
}

fn frames(s: &Scene, id: &str) -> Vec<Rect> {
    let mut v = Vec::new();
    if let Some(g) = find(&s.root, id) {
        text_frames(g, &mut v);
    }
    v
}

fn boxes(s: &Scene, id: &str) -> Vec<Rect> {
    let mut v = Vec::new();
    if let Some(g) = find(&s.root, id) {
        texts(g, &mut v);
    }
    v
}

const WORDS: [&str; 8] = [
    "Concentration",
    "of",
    "[Ca^{2+}]_i",
    "(\\mu M)",
    "relative",
    "fluorescence",
    "\\Delta F/F_0",
    "units",
];

fn title(rng: &mut Rng) -> String {
    let n = 1 + rng.below(5);
    (0..n)
        .map(|_| WORDS[rng.below(WORDS.len())])
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn labels_never_collide() {
    let mut rng = Rng::new(7);
    let families = [
        "scatter",
        "line",
        "column",
        "column_grouped",
        "box",
        "strip",
        "histogram",
        "residuals",
        "kaplan_meier",
        "violin",
        "qq",
    ];
    let positions = [
        LegendPosition::Auto,
        LegendPosition::Right,
        LegendPosition::Top,
        LegendPosition::Bottom,
        LegendPosition::InsideTopLeft,
        LegendPosition::Hidden,
    ];
    let mut checked = 0;
    for case in 0..240 {
        let fam = families[case % families.len()];
        let (mut spec, mut data) = sample(fam).unwrap();
        spec.style.size_mm.width = 55.0 + rng.uniform() * 130.0;
        spec.style.size_mm.height = 45.0 + rng.uniform() * 90.0;
        spec.style.font_pt = Some(6.0 + (rng.uniform() * 6.0).round());
        spec.axes.x.title = Some(title(&mut rng));
        spec.axes.y.title = Some(title(&mut rng));
        if rng.uniform() < 0.3 {
            spec.title = Some(title(&mut rng));
        }
        spec.legend.position = positions[rng.below(positions.len())];
        spec.axes.y.ticks.direction =
            [TickDirection::Out, TickDirection::In, TickDirection::Cross][rng.below(3)];
        // Data magnitudes from 1e-6 to 1e7 stress tick-label widths.
        let mag = 10f64.powi(rng.below(14) as i32 - 6);
        for s in &mut data.series {
            s.y.iter_mut().for_each(|v| *v *= mag);
            for r in &mut s.replicates {
                r.iter_mut().for_each(|v| *v *= mag);
            }
            s.error.iter_mut().for_each(|e| *e = (e.0 * mag, e.1 * mag));
        }
        for c in &mut data.curves {
            c.y.iter_mut().for_each(|v| *v *= mag);
            c.lo.iter_mut().for_each(|v| *v *= mag);
            c.hi.iter_mut().for_each(|v| *v *= mag);
        }
        if fam == "scatter" && rng.uniform() < 0.4 {
            spec.axes.y.scale = Scale::Log10;
        }
        let sc = dioptra_plot::scene(&spec, &data).unwrap();
        let what = format!(
            "case {case} ({fam}, {:.0}×{:.0} mm, font {:?})",
            spec.style.size_mm.width, spec.style.size_mm.height, spec.style.font_pt
        );
        let xl = boxes(&sc, "axis-x-labels");
        let yl = boxes(&sc, "axis-y-labels");
        let xt = boxes(&sc, "axis-x-title");
        let yt = boxes(&sc, "axis-y-title");
        let tt = boxes(&sc, "title");
        let xf = frames(&sc, "axis-x-labels");
        for (i, a) in xf.iter().enumerate() {
            for b in xf.iter().skip(i + 1) {
                assert!(
                    !a.overlaps(b, 0.3),
                    "{what}: x tick labels overlap {a:?} {b:?}"
                );
            }
        }
        for a in &xl {
            for t in &xt {
                assert!(
                    !a.overlaps(t, 0.0),
                    "{what}: x tick label touches the x title"
                );
            }
            for t in &yt {
                assert!(
                    !a.overlaps(t, 0.0),
                    "{what}: x tick label touches the y title"
                );
            }
        }
        for (i, a) in yl.iter().enumerate() {
            for b in yl.iter().skip(i + 1) {
                assert!(!a.overlaps(b, 0.3), "{what}: y tick labels overlap");
            }
            for t in yt.iter().chain(&xt) {
                assert!(!a.overlaps(t, 0.0), "{what}: y tick label touches a title");
            }
        }
        for t in xt.iter().chain(&yt).chain(&tt) {
            assert!(
                t.x >= -0.5 && t.y >= -0.5,
                "{what}: title off the page ({t:?})"
            );
            assert!(
                t.right() <= sc.width + 0.5 && t.bottom() <= sc.height + 0.5,
                "{what}: title off the page ({t:?})"
            );
        }
        for t in xl.iter().chain(&yl) {
            assert!(
                t.x >= -0.5 && t.right() <= sc.width + 0.5,
                "{what}: tick label off the page ({t:?})"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 240);
}
