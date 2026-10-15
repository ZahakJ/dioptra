//! Multi-panel layouts: several scenes on one page in a grid, with bold panel letters.

use serde::{Deserialize, Serialize};

use crate::scene::{Anchor, Baseline, Group, Node, Paint, Rect, Scene};
use crate::text::Weight;

/// How panel letters are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LetterCase {
    /// A, B, C (the default; Cell Press, Science, PLOS, eLife).
    #[default]
    Upper,
    /// a, b, c (Nature).
    Lower,
    /// No letters.
    None,
}

/// Panel letters: case and size (bold, in the bundled font).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLetters {
    /// Case, or none.
    pub case: LetterCase,
    /// Size in points.
    pub size_pt: f64,
}

impl Default for PanelLetters {
    fn default() -> Self {
        PanelLetters {
            case: LetterCase::Upper,
            size_pt: LETTER_PT,
        }
    }
}

impl PanelLetters {
    /// Upper-case letters at the default size, or none.
    pub fn on(letters: bool) -> Self {
        PanelLetters {
            case: if letters {
                LetterCase::Upper
            } else {
                LetterCase::None
            },
            size_pt: LETTER_PT,
        }
    }

    /// The letter for panel `i` in this case (empty for none).
    pub fn text(&self, i: usize) -> String {
        match self.case {
            LetterCase::Upper => letter(i),
            LetterCase::Lower => letter(i).to_lowercase(),
            LetterCase::None => String::new(),
        }
    }
}

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

/// Default letter size in points.
pub const LETTER_PT: f64 = 10.0;
/// Space around a panel letter, in points.
const LETTER_PAD: f64 = 3.0;

/// Width of the gutter left of every panel that holds the panel letters: the widest letter,
/// measured with the bundled font, plus padding on both sides. Zero without letters.
pub fn letter_gutter(panels: usize, letters: bool) -> f64 {
    letter_gutter_with(panels, &PanelLetters::on(letters))
}

/// [`letter_gutter`] for letters in a given case and size.
pub fn letter_gutter_with(panels: usize, letters: &PanelLetters) -> f64 {
    if letters.case == LetterCase::None || panels == 0 || letters.size_pt <= 0.0 {
        return 0.0;
    }
    let widest = (0..panels)
        .map(|i| crate::text::plain_width(&letters.text(i), letters.size_pt, Weight::Bold))
        .fold(0.0, f64::max);
    (widest + 2.0 * LETTER_PAD).ceil()
}

/// Arrange scenes in a grid of `columns` (cells sized to the largest panel), with panel
/// letters when `letters` is true. Each cell reserves a gutter on its left
/// ([`letter_gutter`]) and the letter sits there at the panel's top-left, outside the
/// panel, so it never covers tick labels or titles. Element ids inside panel `i` are
/// prefixed `p{i}-`; the panel group itself is `panel-{i}` with target `/graphs/{i}` (the
/// layout's list).
pub fn compose_layout(panels: &[Scene], columns: usize, letters: bool) -> Scene {
    compose_layout_with(panels, columns, &PanelLetters::on(letters))
}

/// [`compose_layout`] with the letters in a given case and size.
pub fn compose_layout_with(panels: &[Scene], columns: usize, letters: &PanelLetters) -> Scene {
    let show = letters.case != LetterCase::None && letters.size_pt > 0.0;
    let columns = columns.max(1).min(panels.len().max(1));
    let gutter = letter_gutter_with(panels.len(), letters);
    let pw = panels.iter().map(|p| p.width).fold(0.0, f64::max).max(28.0);
    let cw = pw + gutter;
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
                    rect: Rect::new(gutter, 0.0, p.width, p.height),
                    paint: Paint::fill(bg),
                });
            }
        }
        let mut inner = p.root.clone();
        prefix_ids(&mut inner, &format!("p{i}-"));
        if gutter > 0.0 {
            let mut shifted = Group::plain();
            shifted.translate = Some((gutter, 0.0));
            shifted.push(Node::Group(inner));
            g.push(Node::Group(shifted));
        } else {
            g.push(Node::Group(inner));
        }
        if show {
            let ink = background.map_or(crate::scene::Color::rgb(0, 0, 0), |b| {
                if b.luminance() < 0.2 {
                    crate::scene::Color::rgb(0xe8, 0xea, 0xe8)
                } else {
                    crate::scene::Color::rgb(0, 0, 0)
                }
            });
            let mut lg = Group::new(format!("panel-{i}-letter"), format!("/graphs/{i}"));
            lg.push(Node::Text(crate::plot::text_node(
                LETTER_PAD,
                LETTER_PAD,
                &letters.text(i),
                letters.size_pt,
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

    fn texts(g: &Group, off: (f64, f64), out: &mut Vec<Rect>) {
        let (dx, dy) = g.translate.unwrap_or((0.0, 0.0));
        let off = (off.0 + dx, off.1 + dy);
        for c in &g.children {
            match c {
                Node::Text(t) => {
                    let b = t.bbox();
                    out.push(Rect::new(b.x + off.0, b.y + off.1, b.width, b.height));
                }
                Node::Group(cg) => texts(cg, off, out),
                _ => {}
            }
        }
    }

    #[test]
    fn letters_sit_in_their_own_gutter() {
        let a = crate::samples::sample("column").unwrap();
        let b = crate::samples::sample("scatter").unwrap();
        let sa = crate::scene(&a.0, &a.1).unwrap();
        let sb = crate::scene(&b.0, &b.1).unwrap();
        let page = compose_layout(&[sa, sb], 2, true);
        let gutter = letter_gutter(2, true);
        assert!(gutter > 6.0, "gutter {gutter}");
        let mut panel_texts = Vec::new();
        let mut letter_boxes = Vec::new();
        for c in &page.root.children {
            let Node::Group(pg) = c else { continue };
            let base = pg.translate.unwrap_or((0.0, 0.0));
            for cc in &pg.children {
                let Node::Group(g) = cc else { continue };
                let is_letter = g.id.as_deref().is_some_and(|i| i.ends_with("-letter"));
                let mut v = Vec::new();
                texts(g, base, &mut v);
                if is_letter {
                    letter_boxes.extend(v);
                } else {
                    panel_texts.extend(v);
                }
            }
        }
        assert_eq!(letter_boxes.len(), 2);
        assert!(!panel_texts.is_empty());
        for l in &letter_boxes {
            for t in &panel_texts {
                assert!(!l.overlaps(t, 0.0), "letter {l:?} overlaps {t:?}");
            }
        }
        // The page grows by one gutter per column, no more.
        assert!((page.width - 2.0 * (a.0.style.size_mm.width * 72.0 / 25.4 + gutter)).abs() < 1e-6);
        assert_eq!(letter_gutter(3, false), 0.0);
    }

    #[test]
    fn lower_case_letters_at_a_chosen_size() {
        let a = crate::samples::sample("column").unwrap();
        let s = crate::scene(&a.0, &a.1).unwrap();
        let l = PanelLetters {
            case: LetterCase::Lower,
            size_pt: 8.0,
        };
        let page = compose_layout_with(&[s.clone(), s], 2, &l);
        let svg = crate::to_svg(&page);
        assert!(
            svg.contains(">a</text>") && svg.contains(">b</text>"),
            "{svg}"
        );
        assert!(letter_gutter_with(2, &l) < letter_gutter(2, true));
        let none = PanelLetters {
            case: LetterCase::None,
            size_pt: 8.0,
        };
        assert_eq!(letter_gutter_with(2, &none), 0.0);
    }
}
