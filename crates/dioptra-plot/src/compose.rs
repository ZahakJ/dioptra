//! Multi-panel layouts: several scenes on one page in a grid, with bold panel letters.

use crate::scene::{Anchor, Baseline, Group, Node, Paint, Rect, Scene};
use crate::text::Weight;

/// Panel letter for index `i`: A … Z, then AA, AB, ….
pub fn letter(i: usize) -> String {
    let mut n = i + 1;
    let mut s = Vec::new();
    while n > 0 {
        let r = (n - 1) % 26;
        s.push((b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    s.iter().rev().collect()
}

fn prefix_ids(g: &mut Group, p: &str) {
    if let Some(id) = &g.id {
        g.id = Some(format!("{p}{id}"));
    }
    for c in &mut g.children {
        if let Node::Group(cg) = c {
            prefix_ids(cg, p);
        }
    }
}

/// Arrange scenes in a grid of `columns` (cells sized to the largest panel), with panel
/// letters when `letters` is true. Element ids inside panel `i` are prefixed `p{i}-`; the
/// panel group itself is `panel-{i}` with target `/graphs/{i}` (the layout's list).
pub fn compose_layout(panels: &[Scene], columns: usize, letters: bool) -> Scene {
    let columns = columns.max(1).min(panels.len().max(1));
    let cw = panels.iter().map(|p| p.width).fold(0.0, f64::max).max(28.0);
    let ch = panels
        .iter()
        .map(|p| p.height)
        .fold(0.0, f64::max)
        .max(28.0);
    let rows = panels.len().div_ceil(columns).max(1);
    let background = panels.first().and_then(|p| p.background);
    let mut root = Group::plain();
    let mut notes = Vec::new();
    for (i, p) in panels.iter().enumerate() {
        let (x, y) = ((i % columns) as f64 * cw, (i / columns) as f64 * ch);
        let mut g = Group::new(format!("panel-{i}"), format!("/graphs/{i}"));
        g.translate = Some((x, y));
        if let Some(bg) = p.background {
            if Some(bg) != background {
                g.push(Node::Rect {
                    rect: Rect::new(0.0, 0.0, p.width, p.height),
                    paint: Paint::fill(bg),
                });
            }
        }
        let mut inner = p.root.clone();
        prefix_ids(&mut inner, &format!("p{i}-"));
        g.push(Node::Group(inner));
        if letters {
            let ink = background.map_or(crate::scene::Color::rgb(0, 0, 0), |b| {
                if b.luminance() < 0.2 {
                    crate::scene::Color::rgb(0xe8, 0xea, 0xe8)
                } else {
                    crate::scene::Color::rgb(0, 0, 0)
                }
            });
            let mut lg = Group::new(format!("panel-{i}-letter"), format!("/graphs/{i}"));
            lg.push(Node::Text(crate::plot::text_node(
                3.0,
                3.0,
                &letter(i),
                10.0,
                ink,
                Anchor::Start,
                Baseline::Top,
                Weight::Bold,
            )));
            g.push(Node::Group(lg));
        }
        notes.extend(p.notes.iter().map(|n| format!("panel {}: {n}", letter(i))));
        root.push(Node::Group(g));
    }
    Scene {
        width: cw * columns as f64,
        height: ch * rows as f64,
        background,
        root,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters() {
        assert_eq!(letter(0), "A");
        assert_eq!(letter(25), "Z");
        assert_eq!(letter(26), "AA");
        assert_eq!(letter(27), "AB");
    }
}
