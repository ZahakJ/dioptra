//! `pie` and `donut`: parts of a whole, clockwise from twelve o'clock, percentages in the
//! slices that are large enough, and a legend with each part's share. (Offered
//! reluctantly: a column graph of the same numbers is easier to read.)
//!
//! Slice `k` takes its colour from `style.datasets` entries whose `dataset` is `k`.

use super::common::centre;
use crate::detmath::DetMath;
use crate::plot::{draw_legend, legend_box, text_node, Ctx, LegendEntry, Swatch};
use crate::scene::{Anchor, Baseline, Color, Group, Node, Paint, PathCmd, Scene, Stroke};
use crate::spec::LegendPosition;
use crate::text::Weight;

fn parts(ctx: &Ctx<'_>) -> Vec<(String, f64, usize)> {
    let d = ctx.data;
    let o = &ctx.spec.options;
    if d.series.len() == 1 && d.series[0].x.len() > 1 {
        let s = &d.series[0];
        return s
            .x
            .iter()
            .enumerate()
            .map(|(k, x)| {
                let i = x.round() as usize;
                let name = d
                    .categories
                    .get(i.wrapping_sub(1))
                    .cloned()
                    .unwrap_or_else(|| format!("Part {}", k + 1));
                let v = if s.replicates.is_empty() {
                    s.y.get(k).copied().unwrap_or(f64::NAN)
                } else {
                    centre(&s.values_at(k), o.summary)
                };
                (name, v, k)
            })
            .collect();
    }
    d.series
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let v = if s.replicates.is_empty() {
                s.y.iter().copied().filter(|v| v.is_finite()).sum()
            } else {
                centre(&s.all_values(), o.summary)
            };
            (s.name.clone(), v, k)
        })
        .collect()
}

/// Build a pie or donut scene.
pub fn scene(ctx: &Ctx<'_>, donut: bool) -> Scene {
    let spec = ctx.spec;
    let t = &ctx.theme;
    let w = spec.style.size_mm.width * 72.0 / 25.4;
    let h = spec.style.size_mm.height * 72.0 / 25.4;
    let items: Vec<(String, f64, usize)> = parts(ctx)
        .into_iter()
        .filter(|p| p.1.is_finite() && p.1 > 0.0)
        .collect();
    let total: f64 = items.iter().map(|p| p.1).sum();
    let n = items.len();
    let colours = ctx.colours(n.max(1));
    let colour = |k: usize| {
        spec.dataset_style(k)
            .and_then(|o| o.colour.as_deref())
            .and_then(Color::hex)
            .unwrap_or_else(|| {
                if t.mono {
                    Color::rgb(0, 0, 0).mix(t.background, (k as f64 / n.max(2) as f64) * 0.85)
                } else {
                    colours[k % colours.len()]
                }
            })
    };
    let entries: Vec<LegendEntry> = items
        .iter()
        .map(|(name, v, k)| {
            let label = spec
                .dataset_style(*k)
                .and_then(|o| o.label.clone())
                .unwrap_or_else(|| name.clone());
            LegendEntry {
                label: format!("{label}  {:.1}%", v / total * 100.0),
                swatch: Swatch::Fill(Paint::both(colour(*k), Stroke::solid(t.axis, 0.4))),
                dataset: Some(*k),
            }
        })
        .collect();
    let show_legend = spec.legend.position != LegendPosition::Hidden && n > 0;
    let lb = legend_box(&entries, spec.legend.title.as_deref(), t, false);
    let top = 4.0
        + spec
            .title
            .as_ref()
            .map_or(0.0, |_| t.heading_pt * 1.4 + 4.0);
    let right = if show_legend { lb.width + 12.0 } else { 4.0 };
    let (aw, ah) = (w - 8.0 - right, h - top - 4.0);
    let r = (aw.min(ah) / 2.0).max(5.0);
    let (cx, cy) = (4.0 + aw / 2.0, top + ah / 2.0);
    let inner = if donut {
        r * spec.options.donut_hole.clamp(0.1, 0.9)
    } else {
        0.0
    };
    let mut root = Group::plain();
    let mut plot_area = Group::new("plot-area", "/mapping");
    let mut a0 = -std::f64::consts::FRAC_PI_2;
    let sep = Stroke::solid(t.background, 1.0);
    for (i, (_, v, k)) in items.iter().enumerate() {
        let frac = v / total;
        let a1 = a0 + frac * std::f64::consts::TAU;
        let mut g = Group::new(format!("series-{i}"), format!("/style/datasets/{k}"));
        let p = |a: f64, rr: f64| (cx + rr * a.cos_d(), cy + rr * a.sin_d());
        let large = frac > 0.5;
        let cmds = if n == 1 {
            // A full ring cannot be one arc.
            let (s0, s1) = (p(a0, r), p(a0 + std::f64::consts::PI, r));
            let mut c = vec![
                PathCmd::M(s0.0, s0.1),
                PathCmd::A(r, r, false, true, s1.0, s1.1),
                PathCmd::A(r, r, false, true, s0.0, s0.1),
                PathCmd::Z,
            ];
            if donut {
                let (i0, i1) = (p(a0, inner), p(a0 + std::f64::consts::PI, inner));
                c.extend([
                    PathCmd::M(i0.0, i0.1),
                    PathCmd::A(inner, inner, false, false, i1.0, i1.1),
                    PathCmd::A(inner, inner, false, false, i0.0, i0.1),
                    PathCmd::Z,
                ]);
            }
            c
        } else if donut {
            let (o0, o1, i1, i0) = (p(a0, r), p(a1, r), p(a1, inner), p(a0, inner));
            vec![
                PathCmd::M(o0.0, o0.1),
                PathCmd::A(r, r, large, true, o1.0, o1.1),
                PathCmd::L(i1.0, i1.1),
                PathCmd::A(inner, inner, large, false, i0.0, i0.1),
                PathCmd::Z,
            ]
        } else {
            let (o0, o1) = (p(a0, r), p(a1, r));
            vec![
                PathCmd::M(cx, cy),
                PathCmd::L(o0.0, o0.1),
                PathCmd::A(r, r, large, true, o1.0, o1.1),
                PathCmd::Z,
            ]
        };
        let fill = colour(*k);
        g.push(Node::Path {
            cmds,
            paint: Paint::both(fill, sep),
        });
        if frac >= 0.05 {
            let mid = (a0 + a1) / 2.0;
            let rr = if donut { (r + inner) / 2.0 } else { r * 0.64 };
            let (lx, ly) = p(mid, rr);
            let tc = if fill.luminance() > 0.36 {
                Color::rgb(0x1a, 0x1a, 0x1a)
            } else {
                Color::rgb(255, 255, 255)
            };
            g.push(Node::Text(text_node(
                lx,
                ly,
                &format!("{:.0}%", frac * 100.0),
                t.tick_pt,
                tc,
                Anchor::Middle,
                Baseline::Middle,
                Weight::Regular,
            )));
        }
        plot_area.push(Node::Group(g));
        a0 = a1;
    }
    root.push(Node::Group(plot_area));
    if show_legend {
        let lx = cx + r + 12.0;
        let ly = cy - lb.height / 2.0;
        root.push(Node::Group(draw_legend(
            &entries,
            spec.legend.title.as_deref(),
            t,
            lx,
            ly,
            &lb,
        )));
    }
    if let Some(ti) = spec.title.as_deref().filter(|s| !s.is_empty()) {
        let mut g = Group::new("title", "/title");
        g.push(Node::Text(text_node(
            w / 2.0,
            4.0,
            ti,
            t.heading_pt,
            t.text,
            Anchor::Middle,
            Baseline::Top,
            Weight::SemiBold,
        )));
        root.push(Node::Group(g));
    }
    Scene {
        width: w,
        height: h,
        background: if spec.style.transparent {
            None
        } else {
            Some(t.background)
        },
        root,
        notes: vec![],
    }
}
