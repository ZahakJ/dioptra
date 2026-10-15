// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! The project report: data tables (first rows), results sheets, the methods paragraph and
//! the figures, as a Word document or a PDF.
//!
//! The `.docx` is a minimal Office Open XML package written here (ECMA-376 parts:
//! `[Content_Types].xml`, `_rels/.rels`, `word/document.xml`, `word/styles.xml`,
//! `word/_rels/document.xml.rels`, `docProps/core.xml`, PNG figures in `word/media/`).
//! The PDF is set as A4 pages of SVG (text measured with the bundled Inter metrics,
//! figures as vectors) and converted page by page with svg2pdf.

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use dioptra_core::api::{Command, Range, Response, Session, SizeMm};
use dioptra_plot::svg::escape;
use dioptra_plot::text::{plain_width, Weight};
use serde::{Deserialize, Serialize};

use crate::journal::{ok, Targets};

/// Report file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    /// Word (Office Open XML).
    Docx,
    /// PDF.
    Pdf,
}

/// Rows of each data table shown by default.
pub const DEFAULT_ROWS: usize = 20;

/// A4 width and the text width inside 20 mm margins, mm.
const PAGE_W_MM: f64 = 210.0;
const PAGE_H_MM: f64 = 297.0;
const MARGIN_MM: f64 = 20.0;
const TEXT_W_MM: f64 = PAGE_W_MM - 2.0 * MARGIN_MM;
/// Figures are rasterised at this resolution for Word.
const DOCX_DPI: f64 = 300.0;

/// A piece of the report.
#[derive(Debug, Clone)]
pub enum Block {
    /// Heading of level 1–3.
    Heading(u8, String),
    /// Paragraph.
    Para(String),
    /// Small print (notes under a table).
    Note(String),
    /// Table with a header row.
    Table(Vec<String>, Vec<Vec<String>>),
    /// A figure: caption, SVG, size in mm.
    Figure {
        /// Caption.
        caption: String,
        /// The engine's SVG.
        svg: String,
        /// Width, mm.
        width_mm: f64,
        /// Height, mm.
        height_mm: f64,
    },
}

fn table_blocks(session: &mut Session, id: &str, rows: usize) -> Result<Vec<Block>, String> {
    let (title, n_rows, headers, row_titles) = {
        let t = &session.project().tables[id];
        let rt = t.has_row_titles();
        let mut h: Vec<String> = Vec::new();
        if rt {
            h.push(String::new());
        }
        h.extend((0..t.col_count()).map(|c| t.col_header(c)));
        let titles: Vec<String> = (0..t.rows.min(rows))
            .map(|r| t.row_title(r).to_string())
            .collect();
        (t.title.clone(), t.rows, h, rt.then_some(titles))
    };
    let shown = n_rows.min(rows);
    let cols = headers.len() - usize::from(row_titles.is_some());
    let Response::Cells { rows: cells, .. } = ok(session.dispatch(Command::GetCells {
        table: id.to_string(),
        range: Range {
            row0: 0,
            row1: shown,
            col0: 0,
            col1: cols,
        },
    }))?
    else {
        return Err("could not read the table".into());
    };
    let body: Vec<Vec<String>> = cells
        .into_iter()
        .enumerate()
        .map(|(r, row)| {
            let mut out: Vec<String> = Vec::new();
            if let Some(t) = &row_titles {
                out.push(t.get(r).cloned().unwrap_or_default());
            }
            out.extend(row.into_iter().map(|c| {
                let s = if c.d.is_empty() {
                    c.t.unwrap_or_default()
                } else {
                    c.d
                };
                if c.x && !s.is_empty() {
                    format!("{s} (excluded)")
                } else {
                    s
                }
            }));
            out
        })
        .collect();
    let mut v = vec![Block::Heading(2, title), Block::Table(headers, body)];
    if n_rows > shown {
        v.push(Block::Note(format!("First {shown} of {n_rows} rows.")));
    }
    Ok(v)
}

fn results_blocks(session: &mut Session, id: &str) -> Result<Vec<Block>, String> {
    let Response::Results(sheet) = ok(session.dispatch(Command::GetResults { id: id.into() }))?
    else {
        return Ok(Vec::new());
    };
    let style = session.project().p_value_style;
    let mut v = vec![Block::Heading(2, sheet.title.clone())];
    if let Some(m) = &sheet.message {
        v.push(Block::Para(m.clone()));
    }
    for s in &sheet.summary {
        v.push(Block::Para(s.clone()));
    }
    if let Some(m) = &sheet.methods {
        v.push(Block::Note(m.clone()));
    }
    if !sheet.assumptions.is_empty() {
        v.push(Block::Heading(3, "Assumptions".into()));
        v.push(Block::Table(
            vec!["Check".into(), "Status".into(), "Detail".into()],
            sheet
                .assumptions
                .iter()
                .map(|a| vec![a.name.clone(), a.status.label().into(), a.detail.clone()])
                .collect(),
        ));
    }
    for sec in &sheet.sections {
        v.push(Block::Heading(3, sec.title.clone()));
        v.push(Block::Table(
            sec.table.columns.clone(),
            sec.table
                .rows
                .iter()
                .map(|r| r.iter().map(|c| c.display(style)).collect())
                .collect(),
        ));
    }
    Ok(v)
}

fn figure_block(session: &mut Session, id: &str) -> Result<Option<Block>, String> {
    let p = session.project();
    let (caption, scene) = if let Some(g) = p.graphs.get(id) {
        let title = g.title.clone();
        let size = g.spec.style.size_mm;
        // A graph wider than the text is laid out again at the text width.
        let fit = (size.width > TEXT_W_MM).then(|| SizeMm {
            width: TEXT_W_MM,
            height: size.height * TEXT_W_MM / size.width,
        });
        (title, session.graph_scene(id, fit)?)
    } else if let Some(l) = p.layouts.get(id) {
        let title = l.title.clone();
        (title, session.layout_scene(id)?)
    } else {
        return Ok(None);
    };
    Ok(Some(Block::Figure {
        caption,
        svg: dioptra_plot::to_svg(&scene),
        width_mm: scene.width_mm(),
        height_mm: scene.height_mm(),
    }))
}

/// The report's content, in reading order.
pub fn content(session: &mut Session, rows: usize) -> Result<Vec<Block>, String> {
    session.dispatch(Command::RecomputeAll);
    let p = session.project();
    let title = if p.title.trim().is_empty() {
        "Untitled".to_string()
    } else {
        p.title.clone()
    };
    let order = p.order.clone();
    let date: String = dioptra_core::util::now().chars().take(10).collect();
    let mut v = vec![
        Block::Heading(1, title),
        Block::Note(format!("Dioptra {}, {date}.", env!("CARGO_PKG_VERSION"))),
    ];
    let kinds = |s: &Session, f: &dyn Fn(&dioptra_core::project::Project, &str) -> bool| {
        order
            .iter()
            .filter(|id| f(s.project(), id))
            .cloned()
            .collect::<Vec<_>>()
    };
    let tables = kinds(session, &|p, id| p.tables.contains_key(id));
    let analyses = kinds(session, &|p, id| p.analyses.contains_key(id));
    let figures = kinds(session, &|p, id| {
        p.graphs.contains_key(id) || p.layouts.contains_key(id)
    });
    if !tables.is_empty() {
        v.push(Block::Heading(1, "Data".into()));
        for id in &tables {
            v.extend(table_blocks(session, id, rows)?);
        }
    }
    if !analyses.is_empty() {
        if let Ok(m) = crate::methods::methods(session, &Targets::default()) {
            v.push(Block::Heading(1, "Methods".into()));
            for para in m.text.split("\n\n") {
                v.push(Block::Para(para.to_string()));
            }
        }
        v.push(Block::Heading(1, "Results".into()));
        for id in &analyses {
            v.extend(results_blocks(session, id)?);
        }
    }
    if !figures.is_empty() {
        v.push(Block::Heading(1, "Figures".into()));
        let mut n = 0;
        for id in &figures {
            if let Some(Block::Figure {
                caption,
                svg,
                width_mm,
                height_mm,
            }) = figure_block(session, id)?
            {
                n += 1;
                v.push(Block::Figure {
                    caption: format!("Figure {n}. {caption}"),
                    svg,
                    width_mm,
                    height_mm,
                });
            }
        }
    }
    Ok(v)
}

/// Write the report; returns the file name, MIME type and bytes.
pub fn report(
    session: &mut Session,
    format: ReportFormat,
    rows: Option<usize>,
) -> Result<(String, String, Vec<u8>), String> {
    session.require_lab("Journal figures")?;
    let blocks = content(session, rows.unwrap_or(DEFAULT_ROWS).max(1))?;
    let stem = {
        let t = session.project().title.trim().to_string();
        let t: String = t
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
            .collect();
        if t.is_empty() {
            "Untitled".into()
        } else {
            t
        }
    };
    let title = stem.clone();
    match format {
        ReportFormat::Docx => Ok((
            format!("{stem} report.docx"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
            docx(&title, &blocks)?,
        )),
        ReportFormat::Pdf => Ok((
            format!("{stem} report.pdf"),
            "application/pdf".into(),
            pdf(&blocks)?,
        )),
    }
}

// ---- Word -----------------------------------------------------------------------------

fn x(s: &str) -> String {
    escape(s).replace('\'', "&apos;")
}

fn run(text: &str, bold: bool) -> String {
    let rpr = if bold { "<w:rPr><w:b/></w:rPr>" } else { "" };
    let mut out = String::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push_str("<w:r><w:br/></w:r>");
        }
        let _ = write!(
            out,
            "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
            x(line)
        );
    }
    out
}

fn para(style: Option<&str>, body: &str) -> String {
    match style {
        Some(s) => format!("<w:p><w:pPr><w:pStyle w:val=\"{s}\"/></w:pPr>{body}</w:p>"),
        None => format!("<w:p>{body}</w:p>"),
    }
}

const EMU_PER_MM: f64 = 36_000.0;

/// The `word/document.xml` body and the PNG figures it embeds.
fn docx_document(blocks: &[Block]) -> Result<(String, Vec<Vec<u8>>), String> {
    let mut body = String::new();
    let mut media = Vec::new();
    for b in blocks {
        match b {
            Block::Heading(l, t) => {
                body.push_str(&para(Some(&format!("Heading{l}")), &run(t, false)))
            }
            Block::Para(t) => body.push_str(&para(None, &run(t, false))),
            Block::Note(t) => body.push_str(&para(Some("Note"), &run(t, false))),
            Block::Table(head, rows) => {
                body.push_str(
                    "<w:tbl><w:tblPr><w:tblStyle w:val=\"ReportTable\"/><w:tblW w:w=\"0\" w:type=\"auto\"/></w:tblPr><w:tblGrid>",
                );
                for _ in head {
                    body.push_str("<w:gridCol/>");
                }
                body.push_str("</w:tblGrid>");
                let row = |cells: &[String], header: bool| {
                    let mut r = String::from("<w:tr>");
                    if header {
                        r.push_str("<w:trPr><w:tblHeader/></w:trPr>");
                    }
                    for c in cells {
                        let _ = write!(
                            r,
                            "<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr>{}</w:tc>",
                            para(Some("TableText"), &run(c, header))
                        );
                    }
                    r.push_str("</w:tr>");
                    r
                };
                body.push_str(&row(head, true));
                for r in rows {
                    body.push_str(&row(r, false));
                }
                body.push_str("</w:tbl>");
            }
            Block::Figure {
                caption,
                svg,
                width_mm,
                height_mm,
            } => {
                let png = dioptra_plot::output::svg_to_png(svg, *width_mm, *height_mm, DOCX_DPI)
                    .map_err(|e| e.to_string())?;
                media.push(png);
                let n = media.len();
                let s = (TEXT_W_MM / width_mm).min(1.0);
                let (cx, cy) = (
                    (width_mm * s * EMU_PER_MM).round() as i64,
                    (height_mm * s * EMU_PER_MM).round() as i64,
                );
                let _ = write!(
                    body,
                    "<w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{n}\" name=\"Figure {n}\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"figure{n}.png\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"rIdFig{n}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
                );
                body.push_str(&para(Some("Caption"), &run(caption, false)));
            }
        }
    }
    let doc = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1134\" w:right=\"1134\" w:bottom=\"1134\" w:left=\"1134\" w:header=\"567\" w:footer=\"567\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>"
    );
    Ok((doc, media))
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:eastAsia="Arial" w:cs="Arial"/><w:sz w:val="20"/><w:szCs w:val="20"/><w:lang w:val="en-GB"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="264" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="24"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="160" w:after="60"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="20"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Note"><w:name w:val="Note"/><w:basedOn w:val="Normal"/><w:rPr><w:color w:val="595959"/><w:sz w:val="17"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Caption"><w:name w:val="caption"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="240"/></w:pPr><w:rPr><w:sz w:val="18"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="TableText"><w:name w:val="Table Text"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="17"/></w:rPr></w:style><w:style w:type="table" w:styleId="ReportTable"><w:name w:val="Report Table"/><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="8" w:space="0" w:color="000000"/><w:bottom w:val="single" w:sz="8" w:space="0" w:color="000000"/><w:insideH w:val="single" w:sz="2" w:space="0" w:color="BFBFBF"/></w:tblBorders><w:tblCellMar><w:top w:w="20" w:type="dxa"/><w:left w:w="80" w:type="dxa"/><w:bottom w:w="20" w:type="dxa"/><w:right w:w="80" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style></w:styles>"#;

/// A `.docx` package for the blocks.
pub fn docx(title: &str, blocks: &[Block]) -> Result<Vec<u8>, String> {
    use zip::write::SimpleFileOptions;
    let (doc, media) = docx_document(blocks)?;
    let mut rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>",
    );
    for i in 1..=media.len() {
        let _ = write!(
            rels,
            "<Relationship Id=\"rIdFig{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/figure{i}.png\"/>"
        );
    }
    rels.push_str("</Relationships>");
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>Dioptra {}</dc:creator></cp:coreProperties>",
        x(title),
        env!("CARGO_PKG_VERSION")
    );
    let parts: Vec<(String, Vec<u8>)> = vec![
        (
            "[Content_Types].xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#
                .to_vec(),
        ),
        (
            "_rels/.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#
                .to_vec(),
        ),
        ("docProps/core.xml".into(), core.into_bytes()),
        ("word/document.xml".into(), doc.into_bytes()),
        ("word/styles.xml".into(), STYLES.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels".into(), rels.into_bytes()),
    ];
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let deflate = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in parts {
        z.start_file(name, deflate).map_err(|e| e.to_string())?;
        z.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    for (i, png) in media.iter().enumerate() {
        z.start_file(format!("word/media/figure{}.png", i + 1), stored)
            .map_err(|e| e.to_string())?;
        z.write_all(png).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}

// ---- PDF ------------------------------------------------------------------------------

const PT_PER_MM: f64 = 72.0 / 25.4;

struct Pages {
    pages: Vec<String>,
    cur: String,
    y: f64,
    figures: usize,
}

impl Pages {
    fn left() -> f64 {
        MARGIN_MM * PT_PER_MM
    }
    fn width() -> f64 {
        TEXT_W_MM * PT_PER_MM
    }
    fn bottom() -> f64 {
        (PAGE_H_MM - MARGIN_MM) * PT_PER_MM
    }

    fn new() -> Self {
        Pages {
            pages: Vec::new(),
            cur: String::new(),
            y: MARGIN_MM * PT_PER_MM,
            figures: 0,
        }
    }

    fn break_page(&mut self) {
        let body = std::mem::take(&mut self.cur);
        let (w, h) = (PAGE_W_MM * PT_PER_MM, PAGE_H_MM * PT_PER_MM);
        self.pages.push(format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{PAGE_W_MM}mm\" height=\"{PAGE_H_MM}mm\" viewBox=\"0 0 {w:.2} {h:.2}\" font-family=\"Inter\"><rect width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#ffffff\"/>{body}</svg>"
        ));
        self.y = MARGIN_MM * PT_PER_MM;
    }

    fn need(&mut self, h: f64) {
        if self.y + h > Self::bottom() && !self.cur.is_empty() {
            self.break_page();
        }
    }

    fn text(&mut self, x: f64, baseline: f64, size: f64, weight: Weight, colour: &str, s: &str) {
        let fw = if weight == Weight::Bold {
            " font-weight=\"700\""
        } else {
            ""
        };
        let _ = write!(
            self.cur,
            "<text x=\"{x:.2}\" y=\"{baseline:.2}\" font-size=\"{size}\"{fw} fill=\"{colour}\">{}</text>",
            escape(s)
        );
    }

    /// Wrapped text.
    fn para(&mut self, s: &str, size: f64, weight: Weight, colour: &str, after: f64) {
        let lh = size * 1.4;
        for line in wrap(s, size, weight, Self::width()) {
            self.need(lh);
            self.y += lh;
            self.text(
                Self::left(),
                self.y - size * 0.3,
                size,
                weight,
                colour,
                &line,
            );
        }
        self.y += after;
    }

    fn rule(&mut self, y: f64, width: f64) {
        let _ = write!(
            self.cur,
            "<line x1=\"{:.2}\" y1=\"{y:.2}\" x2=\"{:.2}\" y2=\"{y:.2}\" stroke=\"#000000\" stroke-width=\"{width}\"/>",
            Self::left(),
            Self::left() + Self::width()
        );
    }

    fn table(&mut self, head: &[String], rows: &[Vec<String>]) {
        let n = head.len().max(1);
        let pad = 4.0;
        let natural = |size: f64| -> Vec<f64> {
            (0..n)
                .map(|c| {
                    let h = head
                        .get(c)
                        .map_or(0.0, |t| plain_width(t, size, Weight::Bold));
                    rows.iter()
                        .map(|r| {
                            r.get(c)
                                .map_or(0.0, |t| plain_width(t, size, Weight::Regular))
                        })
                        .fold(h, f64::max)
                        + 2.0 * pad
                })
                .collect()
        };
        let mut size: f64 = 8.0;
        let mut widths = natural(size);
        let total: f64 = widths.iter().sum();
        if total > Self::width() {
            size = (size * Self::width() / total).max(5.0);
            widths = natural(size);
        }
        // Still too wide: share the width out and cut long cells.
        let total: f64 = widths.iter().sum();
        if total > Self::width() {
            let k = Self::width() / total;
            for w in &mut widths {
                *w *= k;
            }
        }
        let rh = size * 1.6;
        let draw_row = |p: &mut Pages, cells: &[String], weight: Weight| {
            let mut x0 = Self::left();
            for (c, w) in widths.iter().enumerate() {
                let t = cells.get(c).map(String::as_str).unwrap_or("");
                let t = fit(t, size, weight, w - 2.0 * pad);
                p.text(
                    x0 + pad,
                    p.y + rh - size * 0.45,
                    size,
                    weight,
                    "#000000",
                    &t,
                );
                x0 += w;
            }
            p.y += rh;
        };
        self.need(rh * 2.0 + 2.0);
        self.rule(self.y, 0.75);
        draw_row(self, head, Weight::Bold);
        self.rule(self.y, 0.4);
        for r in rows {
            if self.y + rh > Self::bottom() {
                self.rule(self.y, 0.75);
                self.break_page();
                self.rule(self.y, 0.75);
                draw_row(self, head, Weight::Bold);
                self.rule(self.y, 0.4);
            }
            draw_row(self, r, Weight::Regular);
        }
        self.rule(self.y, 0.75);
        self.y += 8.0;
    }

    fn figure(&mut self, caption: &str, svg: &str, width_mm: f64, height_mm: f64) {
        let s = (TEXT_W_MM / width_mm).min(1.0);
        let (w, h) = (width_mm * s * PT_PER_MM, height_mm * s * PT_PER_MM);
        let max_h = Self::bottom() - MARGIN_MM * PT_PER_MM - 30.0;
        let s2 = (max_h / h).min(1.0);
        let (w, h, scale) = (w * s2, h * s2, s * s2);
        self.need(h + 24.0);
        self.figures += 1;
        let inner = svg
            .find('>')
            .and_then(|start| svg.rfind("</svg>").map(|end| &svg[start + 1..end]))
            .unwrap_or("");
        let inner = inner
            .replace("id=\"clip-", &format!("id=\"f{}clip-", self.figures))
            .replace("url(#clip-", &format!("url(#f{}clip-", self.figures));
        let _ = write!(
            self.cur,
            "<g transform=\"translate({:.2} {:.2}) scale({scale:.5})\">{inner}</g>",
            Self::left() + (Self::width() - w) / 2.0,
            self.y
        );
        self.y += h + 4.0;
        self.para(caption, 8.5, Weight::Regular, "#000000", 10.0);
    }

    fn finish(mut self) -> Vec<String> {
        if !self.cur.is_empty() || self.pages.is_empty() {
            self.break_page();
        }
        self.pages
    }
}

/// Greedy word wrap at `width` points.
fn wrap(s: &str, size: f64, weight: Weight, width: f64) -> Vec<String> {
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if plain_width(&candidate, size, weight) > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Cut text to `width` points with an ellipsis.
fn fit(s: &str, size: f64, weight: Weight, width: f64) -> String {
    if plain_width(s, size, weight) <= width {
        return s.to_string();
    }
    let mut out: String = s.to_string();
    while !out.is_empty() && plain_width(&format!("{out}…"), size, weight) > width {
        out.pop();
    }
    format!("{out}…")
}

/// The report as SVG pages.
pub fn pdf_pages(blocks: &[Block]) -> Vec<String> {
    let mut p = Pages::new();
    for b in blocks {
        match b {
            Block::Heading(1, t) => {
                p.need(60.0);
                p.y += 6.0;
                p.para(t, 15.0, Weight::Bold, "#000000", 4.0);
            }
            Block::Heading(2, t) => {
                p.need(50.0);
                p.y += 4.0;
                p.para(t, 11.5, Weight::Bold, "#000000", 2.0);
            }
            Block::Heading(_, t) => {
                p.need(40.0);
                p.para(t, 9.5, Weight::Bold, "#000000", 1.0);
            }
            Block::Para(t) => p.para(t, 9.5, Weight::Regular, "#000000", 5.0),
            Block::Note(t) => p.para(t, 8.0, Weight::Regular, "#595959", 5.0),
            Block::Table(h, r) => p.table(h, r),
            Block::Figure {
                caption,
                svg,
                width_mm,
                height_mm,
            } => p.figure(caption, svg, *width_mm, *height_mm),
        }
    }
    p.finish()
}

/// The report as a PDF.
pub fn pdf(blocks: &[Block]) -> Result<Vec<u8>, String> {
    dioptra_plot::output::svgs_to_pdf(&pdf_pages(blocks)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal_testkit::{licensed, ok as run, project_with_graphs};
    use serde_json::json;
    use std::io::Read;

    fn sample_project() -> Session {
        let (mut s, _graphs, _layout) = project_with_graphs();
        let t = s.project().tables.keys().next().unwrap().clone();
        let mut spec = run(
            &mut s,
            json!({"type": "default_analysis_spec", "table": t, "kind": "one_way_anova"}),
        );
        spec.as_object_mut().unwrap().remove("type");
        run(
            &mut s,
            json!({"type": "create_analysis", "table": t, "spec": spec}),
        );
        s.dispatch(Command::SetProjectTitle {
            title: "IL-6 & <stress>".into(),
        });
        s
    }

    fn well_formed(xml: &[u8]) {
        let mut r = quick_xml::Reader::from_reader(xml);
        let mut buf = Vec::new();
        let mut depth = 0i32;
        loop {
            match r.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(_)) => depth += 1,
                Ok(quick_xml::events::Event::End(_)) => depth -= 1,
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("not well-formed: {e}"),
            }
            buf.clear();
        }
        assert_eq!(depth, 0);
    }

    #[test]
    fn docx_is_a_valid_package() {
        let mut s = sample_project();
        assert!(report(&mut s, ReportFormat::Docx, None)
            .unwrap_err()
            .contains("Dioptra Lab"));
        licensed(&mut s, true);
        let (name, mime, bytes) = report(&mut s, ReportFormat::Docx, Some(4)).unwrap();
        assert!(name.ends_with("report.docx"), "{name}");
        assert!(mime.contains("wordprocessingml"));
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "docProps/core.xml",
            "word/document.xml",
            "word/styles.xml",
            "word/_rels/document.xml.rels",
            "word/media/figure1.png",
            "word/media/figure2.png",
            "word/media/figure3.png",
        ] {
            assert!(names.iter().any(|n| n == part), "missing {part}: {names:?}");
        }
        for n in &names {
            let mut f = z.by_name(n).unwrap();
            let mut b = Vec::new();
            f.read_to_end(&mut b).unwrap();
            if n.ends_with(".xml") || n.ends_with(".rels") {
                well_formed(&b);
            } else {
                assert_eq!(&b[1..4], b"PNG");
            }
        }
        let mut doc = String::new();
        z.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains("IL-6 &amp; &lt;stress&gt;"));
        assert!(doc.contains("Plasma IL-6"));
        assert!(doc.contains("First 4 of 6 rows."));
        assert!(doc.contains("ANOVA table"));
        assert!(doc.contains("Ordinary one-way ANOVA"));
        assert!(doc.contains("r:embed=\"rIdFig3\""));
        // Every image relationship points at a part that exists.
        let mut rels = String::new();
        z.by_name("word/_rels/document.xml.rels")
            .unwrap()
            .read_to_string(&mut rels)
            .unwrap();
        assert_eq!(rels.matches("relationships/image").count(), 3);
        licensed(&mut s, false);
        assert!(report(&mut s, ReportFormat::Docx, None).is_err());
    }

    #[test]
    fn pdf_has_one_page_per_svg() {
        let mut s = sample_project();
        licensed(&mut s, true);
        let blocks = content(&mut s, 50).unwrap();
        let pages = pdf_pages(&blocks);
        assert!(pages.len() >= 2, "{}", pages.len());
        let (_, mime, bytes) = report(&mut s, ReportFormat::Pdf, None).unwrap();
        assert_eq!(mime, "application/pdf");
        assert!(bytes.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&bytes);
        assert_eq!(text.matches("/MediaBox").count(), pages.len());
    }

    #[test]
    fn wrapping_and_fitting() {
        let lines = wrap(
            "one two three four five six seven",
            10.0,
            Weight::Regular,
            60.0,
        );
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(plain_width(l, 10.0, Weight::Regular) <= 60.0 || !l.contains(' '));
        }
        let f = fit("a very long cell of text", 8.0, Weight::Regular, 30.0);
        assert!(f.ends_with('…') && plain_width(&f, 8.0, Weight::Regular) <= 30.0);
    }
}
