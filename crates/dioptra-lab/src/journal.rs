// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Journal presets: column widths, maximum height, type, line widths, raster resolution and
//! file types for the journals in docs/manual/src/graphs/journal-figures.md, and the command
//! that applies one to graphs and layouts.
//!
//! Every number comes from the publisher's author guidelines, checked on 2026-10-14; the
//! URL is beside each entry. Values marked `UNVERIFIED` could not be found on an official
//! page (the page was unreachable or silent) and are common practice instead; the figure
//! check treats them like the others.

use dioptra_core::api::{Command, Response, Session};
use dioptra_core::graph::{GraphSpec, LayoutStyle, LetterCase, SizeMm};
use serde::{Deserialize, Serialize};

/// The smallest text the engine draws, as a fraction of the tick-label size.
pub const SMALLEST_TEXT: f64 = 0.85;

/// Which column width to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Width {
    /// One column.
    Single,
    /// One and a half columns.
    Onehalf,
    /// Full page width.
    Double,
}

/// A journal's figure requirements.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalPreset {
    /// Identifier used in commands (`nature`).
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Single-column width, mm.
    pub single_mm: f64,
    /// 1.5-column width, mm, when the journal has one.
    pub onehalf_mm: Option<f64>,
    /// Full-width (double-column) width, mm.
    pub double_mm: f64,
    /// Maximum figure height, mm.
    pub max_height_mm: Option<f64>,
    /// Typeface rule.
    pub font: &'static str,
    /// Smallest text at final size, pt.
    pub min_font_pt: f64,
    /// Largest text at final size, pt (panel letters excepted).
    pub max_font_pt: Option<f64>,
    /// Thinnest line at final size, pt.
    pub min_line_pt: f64,
    /// Raster resolution for line art (black and white), dpi.
    pub dpi_line_art: f64,
    /// Raster resolution for halftones (photographs), dpi.
    pub dpi_halftone: f64,
    /// Raster resolution for combination art (colour graphs, mixed), dpi.
    pub dpi_combination: f64,
    /// Accepted file types.
    pub file_types: &'static [&'static str],
    /// Notes on files (compression, vector preference).
    pub file_note: &'static str,
    /// Panel letter case.
    pub letters: LetterCase,
    /// Panel letter size, pt.
    pub letter_pt: f64,
}

impl JournalPreset {
    /// Width in mm for a choice, or an error naming the widths the journal has.
    pub fn width_mm(&self, w: Width) -> Result<f64, String> {
        match w {
            Width::Single => Ok(self.single_mm),
            Width::Double => Ok(self.double_mm),
            Width::Onehalf => self.onehalf_mm.ok_or_else(|| {
                format!(
                    "{} has no 1.5-column width; use single ({} mm) or double ({} mm).",
                    self.name, self.single_mm, self.double_mm
                )
            }),
        }
    }

    /// Tick-label size the preset sets: 7 pt, or more when the engine's smallest text
    /// ([`SMALLEST_TEXT`] of the tick labels: point labels, at-risk numbers) would fall
    /// below the journal minimum; one point below the journal maximum when that leaves
    /// the smallest text legal, so axis titles (+1 pt) stay inside it.
    pub fn font_pt(&self) -> f64 {
        let floor = (self.min_font_pt / SMALLEST_TEXT * 10.0 - 1e-9).ceil() / 10.0;
        let mut f = 7.0f64.max(floor);
        if let Some(m) = self.max_font_pt {
            f = f.min(m - 1.0).max(floor);
        }
        f
    }

    /// Axis line width the preset sets. The thinnest strokes the engine draws (minor
    /// ticks, error-bar caps, hatching) are half the axis width, so the axis is twice the
    /// journal minimum, and never below 0.5 pt.
    pub fn line_pt(&self) -> f64 {
        (2.0 * self.min_line_pt.max(0.25)).max(0.5)
    }
}

/// Every preset, in menu order.
pub fn presets() -> &'static [JournalPreset] {
    PRESETS
}

/// A preset by id.
pub fn preset(id: &str) -> Result<&'static JournalPreset, String> {
    PRESETS.iter().find(|p| p.id == id).ok_or_else(|| {
        format!(
            "there is no journal preset \"{id}\"; choose one of: {}",
            PRESETS.iter().map(|p| p.id).collect::<Vec<_>>().join(", ")
        )
    })
}

const PRESETS: &[JournalPreset] = &[
    // Nature Portfolio.
    // https://research-figure-guide.nature.com/figures/building-and-exporting-figure-panels/
    //   "89 mm (single column)", "183 mm (double column)", "maximum height ... 170 mm",
    //   sans-serif (Helvetica or Arial), text 5-7 pt, panel labels "8 pt bold ... a, b, c".
    // https://www.nature.com/documents/nature-final-artwork.pdf: 1.5 column "120 mm or 136 mm".
    // https://research-figure-guide.nature.com/figures/extended-data-formatting-guidelines/
    //   "Lines and strokes should be set between 0.25 and 1 pt" (Extended Data guide).
    // Main figures are vector (AI, EPS, PDF); photos at least 300 dpi (450 recommended).
    // UNVERIFIED: line-art dpi (1000) and combination dpi (450, the figure guide's export
    // minimum); Nature states no separate values.
    JournalPreset {
        id: "nature",
        name: "Nature",
        single_mm: 89.0,
        onehalf_mm: Some(120.0),
        double_mm: 183.0,
        max_height_mm: Some(170.0),
        font: "Sans-serif, preferably Helvetica or Arial",
        min_font_pt: 5.0,
        max_font_pt: Some(7.0),
        min_line_pt: 0.25,
        dpi_line_art: 1000.0,
        dpi_halftone: 300.0,
        dpi_combination: 450.0,
        file_types: &["PDF", "EPS", "AI"],
        file_note: "Vector files with editable text; raster only for Extended Data (TIFF, JPEG).",
        letters: LetterCase::Lower,
        letter_pt: 8.0,
    },
    // Cell Press. https://www.cell.com/figureguidelines (page refuses direct fetches; read
    // from search extracts of that URL): 85 / 114 / 174 mm, "use only Arial", about 6-8 pt,
    // line art 1000 dpi, colour 300 dpi, TIFF "please use LZW compression".
    // UNVERIFIED: maximum height (225 mm), minimum line width (0.25 pt), combination dpi
    // (500), letter size (8 pt). Panel letters are capitals.
    JournalPreset {
        id: "cell",
        name: "Cell Press",
        single_mm: 85.0,
        onehalf_mm: Some(114.0),
        double_mm: 174.0,
        max_height_mm: Some(225.0),
        font: "Arial",
        min_font_pt: 6.0,
        max_font_pt: Some(8.0),
        min_line_pt: 0.25,
        dpi_line_art: 1000.0,
        dpi_halftone: 300.0,
        dpi_combination: 500.0,
        file_types: &["TIFF", "PDF", "EPS", "AI"],
        file_note: "TIFF with LZW compression.",
        letters: LetterCase::Upper,
        letter_pt: 8.0,
    },
    // Science (AAAS). https://www.science.org/content/page/instructions-preparing-revised-manuscript
    //   "5.7 cm (1 column), 12.1 cm (2 columns), or 18.4 cm (3 columns)"; sans-serif,
    //   Helvetica preferred. Read from search extracts (the page refuses direct fetches).
    // https://www.science.org/cms/asset/76887697-528f-4a22-afea-cee904504e76/sciadv_guide_to_preparing_figures_2026.pdf
    //   minimum 6 pt, panel labels 9 pt bold capitals, lines at least 0.28 pt, line art
    //   1200 dpi, greyscale and colour 300 dpi.
    // UNVERIFIED: maximum height (230 mm), maximum font (none stated), combination dpi
    // (600), file types.
    JournalPreset {
        id: "science",
        name: "Science",
        single_mm: 57.0,
        onehalf_mm: Some(121.0),
        double_mm: 184.0,
        max_height_mm: Some(230.0),
        font: "Sans-serif, preferably Helvetica",
        min_font_pt: 6.0,
        max_font_pt: None,
        min_line_pt: 0.28,
        dpi_line_art: 1200.0,
        dpi_halftone: 300.0,
        dpi_combination: 600.0,
        file_types: &["PDF", "EPS", "AI", "TIFF"],
        file_note: "Vector files preferred.",
        letters: LetterCase::Upper,
        letter_pt: 9.0,
    },
    // PLOS. https://journals.plos.org/plosone/s/figures
    //   width 6.68 cm to 19.05 cm, height at most 22.23 cm, "Arial, Times, or Symbol font
    //   in 8-12 point", "300-600 dpi", "TIFF or EPS only", "LZW compression is required",
    //   panel labels (A) or (a).
    // UNVERIFIED: single-column width (132 mm, the PLOS text column), minimum line width
    // (0.25 pt). PLOS defines no 1.5-column width.
    JournalPreset {
        id: "plos",
        name: "PLOS",
        single_mm: 132.0,
        onehalf_mm: None,
        double_mm: 190.5,
        max_height_mm: Some(222.3),
        font: "Arial (Times and Symbol also accepted)",
        min_font_pt: 8.0,
        max_font_pt: Some(12.0),
        min_line_pt: 0.25,
        dpi_line_art: 300.0,
        dpi_halftone: 300.0,
        dpi_combination: 300.0,
        file_types: &["TIFF", "EPS"],
        file_note: "TIFF with LZW compression.",
        letters: LetterCase::Upper,
        letter_pt: 10.0,
    },
    // eLife. https://elife-rp.msubmit.net/html/elife-rp_author_instructions.html
    //   "TIFF, JPG, EPS, AI and PDF", 8-bit RGB TIFF, "minimum dpi of 300 and a minimum
    //   physical width size of 10cm", full-width figures "minimum physical width of 20cm".
    // UNVERIFIED: font sizes (6-12 pt), line width (0.25 pt), panel letter case and size,
    // maximum height (none stated). No 1.5-column width.
    JournalPreset {
        id: "elife",
        name: "eLife",
        single_mm: 100.0,
        onehalf_mm: None,
        double_mm: 200.0,
        max_height_mm: None,
        font: "Sans-serif",
        min_font_pt: 6.0,
        max_font_pt: Some(12.0),
        min_line_pt: 0.25,
        dpi_line_art: 300.0,
        dpi_halftone: 300.0,
        dpi_combination: 300.0,
        file_types: &["TIFF", "PDF", "EPS", "AI", "JPG"],
        file_note: "TIFF in 8-bit RGB.",
        letters: LetterCase::Upper,
        letter_pt: 10.0,
    },
    // ACS journals. https://researcher-resources.acs.org/publish/author_guidelines?coden=jacsat
    //   (Appendix 2, Preparing Graphics): single column "up to 240 points wide (3.33 in.)",
    //   double "between 300 and 504 points", depth at most "660 points ... including the
    //   caption", Helvetica or Arial, lettering "no smaller than 4.5 points", lines "no
    //   thinner than 0.5 point", "Black and white line art, 1200 dpi / Grayscale art,
    //   600 dpi / Color art, 300 dpi".
    // UNVERIFIED: combination dpi (600), file types (TIFF, PDF, EPS), panel letters.
    JournalPreset {
        id: "acs",
        name: "ACS",
        single_mm: 84.7,
        onehalf_mm: None,
        double_mm: 177.8,
        max_height_mm: Some(232.8),
        font: "Helvetica or Arial",
        min_font_pt: 4.5,
        max_font_pt: None,
        min_line_pt: 0.5,
        dpi_line_art: 1200.0,
        dpi_halftone: 600.0,
        dpi_combination: 600.0,
        file_types: &["TIFF", "PDF", "EPS"],
        file_note: "Colour art at 300 dpi, greyscale at 600 dpi.",
        letters: LetterCase::Upper,
        letter_pt: 8.0,
    },
    // Elsevier (generic artwork instructions).
    // https://www.elsevier.com/about/policies-and-standards/author/artwork-and-media-instructions/artwork-sizing
    //   single 90 mm, 1.5 column 140 mm, double 190 mm.
    // https://www.elsevier.com/about/policies-and-standards/author/artwork-and-media-instructions/artwork-overview
    //   Arial/Helvetica (Courier, Symbol, Times also), "7 pt for normal text", "no smaller
    //   than 6 pt for subscript and superscript", "Line weights range from 0.10 pt to 1.5 pt".
    // https://www.elsevier.com/about/policies-and-standards/author/artwork-and-media-instructions/artwork-types
    //   line art 1000 dpi, halftone 300 dpi, combination 500 dpi; EPS, PDF, TIFF or JPEG.
    // UNVERIFIED: maximum height (none stated), panel letters.
    JournalPreset {
        id: "elsevier",
        name: "Elsevier",
        single_mm: 90.0,
        onehalf_mm: Some(140.0),
        double_mm: 190.0,
        max_height_mm: None,
        font: "Arial or Helvetica",
        min_font_pt: 6.0,
        max_font_pt: None,
        min_line_pt: 0.1,
        dpi_line_art: 1000.0,
        dpi_halftone: 300.0,
        dpi_combination: 500.0,
        file_types: &["EPS", "PDF", "TIFF", "JPEG"],
        file_note: "Normal text 7 pt; sub- and superscripts at least 6 pt.",
        letters: LetterCase::Upper,
        letter_pt: 8.0,
    },
    // APS (Physical Review). https://journals.aps.org/authors/style-basics
    //   "single manuscript column (8.5 cm or 3 3/8 in) or 1.5 or 2 columns", "smallest
    //   capital letters and numerals at least 2 mm" (Inter capitals are 0.727 em: 2 mm is
    //   7.8 pt text, rounded to 8 pt), lines "at least 0.18 mm (0.5 point)", scans
    //   "preferably 600 dpi or higher", ".ps, .eps .jpg, and .png", subfigures (a), (b).
    // UNVERIFIED: 1.5-column (130 mm) and double-column (178 mm) widths, maximum height
    // (237 mm).
    JournalPreset {
        id: "aps",
        name: "APS (Physical Review)",
        single_mm: 86.0,
        onehalf_mm: Some(130.0),
        double_mm: 178.0,
        max_height_mm: Some(237.0),
        font: "Any clear sans-serif",
        min_font_pt: 8.0,
        max_font_pt: None,
        min_line_pt: 0.5,
        dpi_line_art: 600.0,
        dpi_halftone: 600.0,
        dpi_combination: 600.0,
        file_types: &["EPS", "PS", "PNG", "JPG"],
        file_note: "Data points at least 1 mm across.",
        letters: LetterCase::Lower,
        letter_pt: 8.0,
    },
    // The Journal of Physiology. https://jp.msubmit.net/cgi-bin/main.plex?form_type=display_requirements
    //   fit "17cm x 23cm", "either a full-width (17cm) or half-width (8.5cm)", "sans serif
    //   font (e.g. Arial) such that the final size will be 8pt", "A, B, C etc. for panel
    //   labels", vector preferred, otherwise "600 dpi or higher".
    // UNVERIFIED: minimum line width (0.25 pt), file types (PDF, EPS, TIFF). No 1.5-column
    // width.
    JournalPreset {
        id: "jphysiol",
        name: "The Journal of Physiology",
        single_mm: 85.0,
        onehalf_mm: None,
        double_mm: 170.0,
        max_height_mm: Some(230.0),
        font: "Sans-serif, e.g. Arial",
        min_font_pt: 8.0,
        max_font_pt: None,
        min_line_pt: 0.25,
        dpi_line_art: 600.0,
        dpi_halftone: 600.0,
        dpi_combination: 600.0,
        file_types: &["PDF", "EPS", "TIFF"],
        file_note: "Vector files preferred.",
        letters: LetterCase::Upper,
        letter_pt: 10.0,
    },
];

/// Graphs and layouts a command applies to: `"all"` or a list of ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Targets {
    /// Every graph and layout (`"all"`), or the whole project (`"project"`).
    Named(Every),
    /// These ids.
    Ids(Vec<String>),
}

/// The word that means everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Every {
    /// Every graph and layout.
    All,
    /// The whole project.
    Project,
}

impl Default for Targets {
    fn default() -> Self {
        Targets::Named(Every::All)
    }
}

/// The graph spec with a preset applied at a width.
pub fn styled_spec(spec: &GraphSpec, p: &JournalPreset, width_mm: f64) -> GraphSpec {
    let mut s = spec.clone();
    let SizeMm { width, height } = s.style.size_mm;
    let mut h = if width > 0.0 {
        height * width_mm / width
    } else {
        height
    };
    if let Some(m) = p.max_height_mm {
        h = h.min(m);
    }
    s.style.size_mm = SizeMm {
        width: width_mm,
        height: h.clamp(10.0, 1000.0),
    };
    s.style.font_pt = Some(p.font_pt());
    s.style.line_pt = Some(p.line_pt());
    // Slides theme on dark paper does not print.
    if s.style.theme == "night" {
        s.style.theme = "instrument".into();
    }
    s
}

/// The layout style a preset sets at a width.
pub fn layout_style(p: &JournalPreset, width_mm: f64) -> LayoutStyle {
    LayoutStyle {
        width_mm: Some(width_mm),
        font_pt: Some(p.font_pt()),
        line_pt: Some(p.line_pt()),
        letters: p.letters,
        letter_pt: Some(p.letter_pt),
    }
}

/// Apply a preset to graphs and layouts in one undo step; returns the ids changed.
pub fn apply(
    session: &mut Session,
    preset_id: &str,
    width: Width,
    targets: &Targets,
) -> Result<Vec<String>, String> {
    session.require_lab("Journal figures")?;
    let p = preset(preset_id)?;
    let w = p.width_mm(width)?;
    let project = session.project();
    let ids: Vec<String> = match targets {
        Targets::Named(_) => project
            .order
            .iter()
            .filter(|id| project.graphs.contains_key(*id) || project.layouts.contains_key(*id))
            .cloned()
            .collect(),
        Targets::Ids(v) => v.clone(),
    };
    let mut cmds = Vec::new();
    for id in &ids {
        if let Some(g) = project.graphs.get(id) {
            cmds.push(Command::UpdateGraph {
                id: id.clone(),
                spec: styled_spec(&g.spec, p, w),
            });
        } else if let Some(l) = project.layouts.get(id) {
            cmds.push(Command::UpdateLayout {
                id: id.clone(),
                graphs: l.graphs.clone(),
                columns: l.columns,
                style: Some(layout_style(p, w)),
            });
        } else {
            return Err(format!("no graph or layout with id {id}"));
        }
    }
    if cmds.is_empty() {
        return Err("The project has no graphs or layouts.".into());
    }
    session.batch(&format!("Apply {} preset", p.name), cmds)?;
    Ok(ids)
}

/// The scene a graph or layout exports to.
fn scene_of(
    session: &mut Session,
    id: &str,
    size: Option<SizeMm>,
) -> Result<dioptra_plot::Scene, String> {
    if session.project().graphs.contains_key(id) {
        session.graph_scene(id, size)
    } else if session.project().layouts.contains_key(id) {
        session.layout_scene(id)
    } else {
        Err(format!("no graph or layout with id {id}"))
    }
}

/// Check a graph or layout as exported.
pub fn check(
    session: &mut Session,
    id: &str,
    preset_id: Option<&str>,
    raster: Option<crate::check::RasterExport>,
) -> Result<crate::check::FigureCheck, String> {
    session.require_lab("Journal figures")?;
    let p = preset_id.map(preset).transpose()?;
    let scene = scene_of(session, id, None)?;
    Ok(crate::check::check_scene(&scene, p, raster))
}

/// A graph or layout as an LZW TIFF; returns the file name and bytes.
pub fn export_tiff(
    session: &mut Session,
    id: &str,
    dpi: f64,
    size: Option<SizeMm>,
) -> Result<(String, Vec<u8>), String> {
    session.require_lab("Journal figures")?;
    if !(36.0..=2400.0).contains(&dpi) {
        return Err("The resolution must be between 36 and 2400 dpi.".into());
    }
    let scene = scene_of(session, id, size)?;
    let p = session.project();
    let title = p
        .graphs
        .get(id)
        .map(|g| g.title.clone())
        .or_else(|| p.layouts.get(id).map(|l| l.title.clone()))
        .unwrap_or_default();
    let stem: String = title
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let stem = if stem.trim().is_empty() {
        "figure".to_string()
    } else {
        stem.trim().to_string()
    };
    Ok((
        format!("{stem}.tiff"),
        crate::journal_tiff::scene_to_tiff(&scene, dpi)?,
    ))
}

/// A command's response as an error when it failed.
pub(crate) fn ok(r: Response) -> Result<Response, String> {
    match r {
        Response::Error { message } => Err(message),
        r => Ok(r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal_testkit::{licensed, project_with_graphs};

    #[test]
    fn every_preset_is_consistent() {
        for p in presets() {
            assert!(p.single_mm < p.double_mm, "{}", p.id);
            if let Some(h) = p.onehalf_mm {
                assert!(p.single_mm < h && h < p.double_mm, "{}", p.id);
            }
            if let Some(m) = p.max_font_pt {
                assert!(p.font_pt() <= m, "{}", p.id);
            }
            assert!(
                p.font_pt() * SMALLEST_TEXT >= p.min_font_pt - 1e-9,
                "{}",
                p.id
            );
            assert!(
                p.line_pt() / 2.0 >= p.min_line_pt.max(0.25) - 1e-9,
                "{}",
                p.id
            );
            assert!(!p.file_types.is_empty());
        }
        assert!(preset("nope").is_err());
        assert!(preset("plos").unwrap().width_mm(Width::Onehalf).is_err());
    }

    #[test]
    fn preset_applies_to_graphs_and_layouts_in_one_undo_step() {
        let (mut s, graphs, layout) = project_with_graphs();
        assert!(apply(&mut s, "nature", Width::Single, &Targets::default())
            .unwrap_err()
            .contains("Dioptra Lab"));
        licensed(&mut s, true);
        let ids = apply(&mut s, "nature", Width::Single, &Targets::default()).unwrap();
        assert_eq!(ids.len(), 3);
        let p = s.project();
        for g in &graphs {
            let st = &p.graphs[g].spec.style;
            assert_eq!(st.size_mm.width, 89.0);
            assert!(st.size_mm.height <= 170.0);
            assert_eq!(st.font_pt, Some(6.0));
            assert_eq!(st.line_pt, Some(0.5));
        }
        let l = &p.layouts[&layout].style;
        assert_eq!(l.width_mm, Some(89.0));
        assert_eq!(l.letters, LetterCase::Lower);
        assert_eq!(l.letter_pt, Some(8.0));
        // The layout page is exactly one column wide.
        let page = s.layout_scene(&layout).unwrap();
        assert!((page.width_mm() - 89.0).abs() < 1e-6, "{}", page.width_mm());
        // One undo takes everything back.
        s.dispatch(Command::Undo);
        let p = s.project();
        for g in &graphs {
            assert_eq!(p.graphs[g].spec.style.size_mm.width, 85.0);
            assert_eq!(p.graphs[g].spec.style.font_pt, None);
        }
        assert!(p.layouts[&layout].style.is_default());
        // Only some targets; a bad id changes nothing.
        apply(
            &mut s,
            "cell",
            Width::Double,
            &Targets::Ids(vec![graphs[0].clone()]),
        )
        .unwrap();
        assert_eq!(
            s.project().graphs[&graphs[0]].spec.style.size_mm.width,
            174.0
        );
        assert_eq!(
            s.project().graphs[&graphs[1]].spec.style.size_mm.width,
            85.0
        );
        let before = s.project().graphs[&graphs[0]].spec.clone();
        assert!(apply(
            &mut s,
            "science",
            Width::Single,
            &Targets::Ids(vec![graphs[0].clone(), "missing".into()]),
        )
        .is_err());
        assert_eq!(s.project().graphs[&graphs[0]].spec, before);
        licensed(&mut s, false);
        assert!(apply(&mut s, "cell", Width::Single, &Targets::default()).is_err());
    }

    #[test]
    fn check_and_tiff_through_json() {
        let (mut s, graphs, layout) = project_with_graphs();
        let cmd =
            serde_json::json!({"type": "lab_check_figure", "id": graphs[0], "preset": "nature"})
                .to_string();
        assert!(crate::dispatch_json(&mut s, &cmd).contains("part of Dioptra Lab"));
        let presets = crate::dispatch_json(&mut s, r#"{"type":"lab_journal_presets"}"#);
        assert!(presets.contains("\"nature\"") && presets.contains("\"jphysiol\""));
        licensed(&mut s, true);
        let r: serde_json::Value =
            serde_json::from_str(&crate::dispatch_json(&mut s, &cmd)).unwrap();
        assert_eq!(r["type"], "lab_figure_check", "{r}");
        // The default figure is 85 mm: not a Nature width, and 8 pt text is above its 7 pt.
        assert!(r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule"] == "size"));
        let apply = serde_json::json!({"type": "lab_apply_journal_preset", "preset": "nature", "width": "single", "targets": "all"}).to_string();
        let r: serde_json::Value =
            serde_json::from_str(&crate::dispatch_json(&mut s, &apply)).unwrap();
        assert_eq!(r["type"], "lab_changed", "{r}");
        for id in [&graphs[0], &graphs[1], &layout] {
            let c = check(&mut s, id, Some("nature"), None).unwrap();
            let errors: Vec<_> = c
                .issues
                .iter()
                .filter(|i| i.severity == crate::check::Severity::Error)
                .collect();
            assert!(errors.is_empty(), "{id}: {errors:?}");
        }
        let tiff =
            serde_json::json!({"type": "lab_export_tiff", "id": layout, "dpi": 300}).to_string();
        let r: serde_json::Value =
            serde_json::from_str(&crate::dispatch_json(&mut s, &tiff)).unwrap();
        assert_eq!(r["mime"], "image/tiff", "{r}");
        assert!(r["name"].as_str().unwrap().ends_with(".tiff"));
    }

    #[test]
    fn targets_parse_from_json() {
        let t: Targets = serde_json::from_str("\"all\"").unwrap();
        assert_eq!(t, Targets::Named(Every::All));
        let t: Targets = serde_json::from_str("[\"a\",\"b\"]").unwrap();
        assert_eq!(t, Targets::Ids(vec!["a".into(), "b".into()]));
    }
}
