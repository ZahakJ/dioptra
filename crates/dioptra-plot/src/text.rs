//! Typography: the bundled Inter fonts, their metrics, and the small markup used in titles
//! and labels (`^{…}`/`_{…}` super/subscripts, `\alpha` Greek and a few symbols).
//!
//! Widths come from the fonts' own advance widths (parsed with `ttf-parser`), so layout in
//! points matches what usvg/resvg and PDF viewers draw, apart from kerning (a fraction of a
//! point over a label).

use std::sync::OnceLock;

/// Inter Regular (OFL), embedded.
pub const INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
/// Inter SemiBold.
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");
/// Inter Bold.
pub const INTER_BOLD: &[u8] = include_bytes!("../fonts/Inter-Bold.ttf");
/// Inter Italic.
pub const INTER_ITALIC: &[u8] = include_bytes!("../fonts/Inter-Italic.ttf");

/// The font family name written into SVG.
pub const FAMILY: &str = "Inter";

/// Font weight classes the engine uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Weight {
    /// 400.
    #[default]
    Regular,
    /// 600.
    SemiBold,
    /// 700.
    Bold,
}

impl Weight {
    /// CSS weight number.
    pub fn css(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::SemiBold => 600,
            Weight::Bold => 700,
        }
    }
}

/// Vertical metrics as fractions of the em.
#[derive(Debug, Clone, Copy)]
pub struct VMetrics {
    /// Ascender (positive).
    pub ascender: f64,
    /// Descender (negative).
    pub descender: f64,
    /// Cap height.
    pub cap_height: f64,
    /// x-height.
    pub x_height: f64,
}

struct Face {
    face: ttf_parser::Face<'static>,
    upem: f64,
    table: Vec<f32>,
}

const TABLE_LEN: usize = 0x400;

fn load(bytes: &'static [u8]) -> Face {
    let face = ttf_parser::Face::parse(bytes, 0).expect("bundled Inter parses");
    let upem = face.units_per_em() as f64;
    let mut table = vec![-1.0f32; TABLE_LEN];
    for (c, slot) in table.iter_mut().enumerate() {
        if let Some(ch) = char::from_u32(c as u32) {
            if let Some(g) = face.glyph_index(ch) {
                if let Some(a) = face.glyph_hor_advance(g) {
                    *slot = (a as f64 / upem) as f32;
                }
            }
        }
    }
    Face { face, upem, table }
}

fn faces() -> &'static [Face; 4] {
    static F: OnceLock<[Face; 4]> = OnceLock::new();
    F.get_or_init(|| {
        [
            load(INTER_REGULAR),
            load(INTER_SEMIBOLD),
            load(INTER_BOLD),
            load(INTER_ITALIC),
        ]
    })
}

fn face(w: Weight, italic: bool) -> &'static Face {
    let f = faces();
    if italic {
        return &f[3];
    }
    match w {
        Weight::Regular => &f[0],
        Weight::SemiBold => &f[1],
        Weight::Bold => &f[2],
    }
}

/// Vertical metrics of Inter Regular.
pub fn vmetrics() -> VMetrics {
    static M: OnceLock<VMetrics> = OnceLock::new();
    *M.get_or_init(|| {
        let f = &faces()[0];
        let u = f.upem;
        VMetrics {
            ascender: f.face.ascender() as f64 / u,
            descender: f.face.descender() as f64 / u,
            cap_height: f.face.capital_height().map_or(0.727, |c| c as f64 / u),
            x_height: f.face.x_height().map_or(0.545, |c| c as f64 / u),
        }
    })
}

/// Advance width of one character in ems (0.55 em when the font lacks it).
pub fn char_advance(c: char, w: Weight, italic: bool) -> f64 {
    let f = face(w, italic);
    let i = c as usize;
    if i < TABLE_LEN {
        let v = f.table[i];
        if v >= 0.0 {
            return v as f64;
        }
        return 0.55;
    }
    f.face
        .glyph_index(c)
        .and_then(|g| f.face.glyph_hor_advance(g))
        .map_or(0.55, |a| a as f64 / f.upem)
}

/// Width of plain text in points.
pub fn plain_width(s: &str, size: f64, w: Weight) -> f64 {
    s.chars().map(|c| char_advance(c, w, false)).sum::<f64>() * size
}

/// Baseline shift of a text run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shift {
    /// On the baseline.
    #[default]
    None,
    /// Superscript.
    Super,
    /// Subscript.
    Sub,
}

/// A run of text sharing one baseline shift.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Run {
    /// Text (already converted to Unicode).
    pub text: String,
    /// Shift.
    pub shift: Shift,
}

/// Size of super/subscripts relative to the base size.
pub const SCRIPT_SCALE: f64 = 0.7;
/// Superscript rise as a fraction of the base size.
pub const SUPER_RISE: f64 = 0.36;
/// Subscript drop as a fraction of the base size.
pub const SUB_DROP: f64 = 0.16;

/// Width of runs in points.
pub fn runs_width(runs: &[Run], size: f64, w: Weight, italic: bool) -> f64 {
    runs.iter()
        .map(|r| {
            let s = if r.shift == Shift::None {
                size
            } else {
                size * SCRIPT_SCALE
            };
            r.text
                .chars()
                .map(|c| char_advance(c, w, italic))
                .sum::<f64>()
                * s
        })
        .sum()
}

/// `\name` → Unicode.
pub fn symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ε",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "omicron" => "ο",
        "pi" => "π",
        "rho" => "ρ",
        "sigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "φ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "degree" | "deg" => "°",
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "cdot" => "·",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "infty" => "∞",
        "minus" => "−",
        "permil" => "‰",
        "angstrom" => "Å",
        "prime" => "′",
        "to" | "rightarrow" => "→",
        "micro" => "µ",
        _ => return None,
    })
}

fn push(runs: &mut Vec<Run>, shift: Shift, s: &str) {
    if s.is_empty() {
        return;
    }
    match runs.last_mut() {
        Some(r) if r.shift == shift => r.text.push_str(s),
        _ => runs.push(Run {
            text: s.to_string(),
            shift,
        }),
    }
}

/// Parse label markup into runs:
/// - `^{…}` and `_{…}`: superscript and subscript groups; `^2`, `_i`: one character.
/// - `\alpha` … `\omega`, `\Delta`, `\mu`, `\degree`, `\pm`, `\times`, … : symbols.
/// - `\^`, `\_`, `\\`, `\{`, `\}`: literal characters.
///
/// A `^` or `_` followed by a space or at the end of the text is kept literally.
pub fn parse(s: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    parse_into(&chars, &mut i, Shift::None, false, &mut runs);
    runs
}

fn parse_into(c: &[char], i: &mut usize, shift: Shift, group: bool, runs: &mut Vec<Run>) {
    while *i < c.len() {
        let ch = c[*i];
        match ch {
            '}' if group => {
                *i += 1;
                return;
            }
            '\\' => {
                let j = *i + 1;
                if j < c.len() && "^_\\{}".contains(c[j]) {
                    push(runs, shift, &c[j].to_string());
                    *i = j + 1;
                    continue;
                }
                let mut k = j;
                while k < c.len() && c[k].is_ascii_alphabetic() {
                    k += 1;
                }
                let name: String = c[j..k].iter().collect();
                if let Some(sym) = symbol(&name) {
                    push(runs, shift, sym);
                    // As in TeX, `\alpha{}` or one space ends the name and is swallowed.
                    if k + 1 < c.len() && c[k] == '{' && c[k + 1] == '}' {
                        k += 2;
                    } else if k < c.len() && c[k] == ' ' {
                        k += 1;
                    }
                    *i = k;
                } else {
                    push(runs, shift, "\\");
                    *i = j;
                }
            }
            '^' | '_' if shift == Shift::None => {
                let s = if ch == '^' { Shift::Super } else { Shift::Sub };
                let j = *i + 1;
                if j < c.len() && c[j] == '{' {
                    *i = j + 1;
                    parse_into(c, i, s, true, runs);
                } else if j < c.len() && !c[j].is_whitespace() {
                    if c[j] == '\\' {
                        // `^\circ`-style: one escaped symbol.
                        *i = j;
                        let mut sub = Vec::new();
                        let mut k = 0;
                        let mut end = j + 1;
                        while end < c.len() && c[end].is_ascii_alphabetic() {
                            end += 1;
                        }
                        let piece: Vec<char> = c[j..end.max(j + 2).min(c.len())].to_vec();
                        parse_into(&piece, &mut k, s, false, &mut sub);
                        for r in sub {
                            push(runs, s, &r.text);
                        }
                        *i = j + piece.len();
                    } else {
                        // A run of a sign plus digits counts as one script (`^-1`, `^10`).
                        let mut k = j;
                        if c[k] == '-' || c[k] == '+' {
                            k += 1;
                        }
                        if k < c.len() && c[k].is_ascii_digit() {
                            while k < c.len() && (c[k].is_ascii_digit() || c[k] == '.') {
                                k += 1;
                            }
                        } else {
                            k = j + 1;
                        }
                        let t: String = c[j..k]
                            .iter()
                            .map(|ch| if *ch == '-' { '−' } else { *ch })
                            .collect();
                        push(runs, s, &t);
                        *i = k;
                    }
                } else {
                    push(runs, shift, &ch.to_string());
                    *i += 1;
                }
            }
            _ => {
                let mut b = [0u8; 4];
                let t = if shift != Shift::None && ch == '-' {
                    "−"
                } else {
                    ch.encode_utf8(&mut b)
                };
                push(runs, shift, t);
                *i += 1;
            }
        }
    }
}

/// Plain Unicode text of runs (for accessibility titles and tests).
pub fn plain(runs: &[Run]) -> String {
    runs.iter().map(|r| r.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(t: &str, s: Shift) -> Run {
        Run {
            text: t.into(),
            shift: s,
        }
    }

    #[test]
    fn scripts_and_symbols() {
        assert_eq!(parse("Conc. (\\mu M)"), vec![r("Conc. (μM)", Shift::None)]);
        assert_eq!(
            parse("m^2"),
            vec![r("m", Shift::None), r("2", Shift::Super)]
        );
        assert_eq!(
            parse("s^{-1}"),
            vec![r("s", Shift::None), r("−1", Shift::Super)]
        );
        assert_eq!(
            parse("s^-1 x"),
            vec![
                r("s", Shift::None),
                r("−1", Shift::Super),
                r(" x", Shift::None)
            ]
        );
        assert_eq!(
            parse("V_{max} (nM)"),
            vec![
                r("V", Shift::None),
                r("max", Shift::Sub),
                r(" (nM)", Shift::None)
            ]
        );
        assert_eq!(
            parse("x_i + 1"),
            vec![
                r("x", Shift::None),
                r("i", Shift::Sub),
                r(" + 1", Shift::None)
            ]
        );
        assert_eq!(parse("37 \\degree C"), vec![r("37 °C", Shift::None)]);
        assert_eq!(
            parse("\\Delta F/F_0"),
            vec![r("ΔF/F", Shift::None), r("0", Shift::Sub)]
        );
        assert_eq!(parse("a \\pm{} b"), vec![r("a ± b", Shift::None)]);
        assert_eq!(
            parse("10^{\\alpha}"),
            vec![r("10", Shift::None), r("α", Shift::Super)]
        );
        assert_eq!(
            parse("cost \\^ \\_ \\\\"),
            vec![r("cost ^ _ \\", Shift::None)]
        );
        assert_eq!(parse("trailing ^"), vec![r("trailing ^", Shift::None)]);
        assert_eq!(parse("\\unknown"), vec![r("\\unknown", Shift::None)]);
        assert_eq!(
            parse("CO_2 level"),
            vec![
                r("CO", Shift::None),
                r("2", Shift::Sub),
                r(" level", Shift::None)
            ]
        );
    }

    #[test]
    fn widths_are_from_the_font() {
        let w = plain_width("0", 10.0, Weight::Regular);
        assert!(w > 4.0 && w < 8.0, "{w}");
        // Tabular-ish digits in Inter are close but not equal; "W" is wider than "i".
        assert!(plain_width("W", 10.0, Weight::Regular) > plain_width("i", 10.0, Weight::Regular));
        assert!(plain_width("A", 10.0, Weight::Bold) >= plain_width("A", 10.0, Weight::Regular));
        // Greek and superscripts are covered.
        assert!(char_advance('α', Weight::Regular, false) != 0.55);
        assert!(char_advance('⁴', Weight::Regular, false) != 0.55);
        let m = vmetrics();
        assert!(m.cap_height > 0.6 && m.cap_height < 0.8);
    }
}
