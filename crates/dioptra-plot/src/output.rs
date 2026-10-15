//! PNG and PDF output. Both parse the engine's own SVG with usvg against a font database
//! that holds only the bundled Inter files, so output is identical on every platform and
//! in the browser build (no system fonts are ever consulted).

use std::sync::{Arc, OnceLock};

use crate::scene::Scene;
use crate::svg::to_svg;
use crate::text::{INTER_BOLD, INTER_ITALIC, INTER_REGULAR, INTER_SEMIBOLD};

/// Errors from PNG/PDF output.
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    /// The SVG could not be parsed (an engine bug).
    #[error("could not read the drawing: {0}")]
    Parse(String),
    /// The raster would be empty or too large.
    #[error("{0}")]
    Size(String),
    /// PDF conversion failed.
    #[error("could not write the PDF: {0}")]
    Pdf(String),
    /// PNG encoding failed.
    #[error("could not encode the PNG: {0}")]
    Png(String),
}

/// The font database with only the bundled Inter faces.
pub fn font_database() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        for f in [INTER_REGULAR, INTER_SEMIBOLD, INTER_BOLD, INTER_ITALIC] {
            db.load_font_data(f.to_vec());
        }
        db.set_sans_serif_family("Inter");
        db.set_serif_family("Inter");
        db.set_monospace_family("Inter");
        Arc::new(db)
    })
    .clone()
}

fn parse(svg: &str) -> Result<usvg::Tree, OutputError> {
    let opt = usvg::Options {
        // User units are points; 72 dpi makes `mm` sizes come out in points too.
        dpi: 72.0,
        font_family: "Inter".into(),
        fontdb: font_database(),
        ..Default::default()
    };
    usvg::Tree::from_str(svg, &opt).map_err(|e| OutputError::Parse(e.to_string()))
}

/// Pixel size of a raster: millimetres / 25.4 × dpi, rounded to the nearest pixel
/// (85 mm at 300 dpi is 1003.9 → 1004 px).
pub fn pixel_size(mm: f64, dpi: f64) -> u32 {
    (mm / 25.4 * dpi).round().max(1.0) as u32
}

/// Render a scene to PNG at `dpi`, with the resolution recorded in a `pHYs` chunk.
pub fn to_png(scene: &Scene, dpi: f64) -> Result<Vec<u8>, OutputError> {
    svg_to_png(&to_svg(scene), scene.width_mm(), scene.height_mm(), dpi)
}

/// Render an SVG written by this engine to PNG.
pub fn svg_to_png(
    svg: &str,
    width_mm: f64,
    height_mm: f64,
    dpi: f64,
) -> Result<Vec<u8>, OutputError> {
    let png = rasterise(svg, width_mm, height_mm, dpi)?
        .encode_png()
        .map_err(|e| OutputError::Png(e.to_string()))?;
    Ok(insert_phys(png, dpi))
}

fn rasterise(
    svg: &str,
    width_mm: f64,
    height_mm: f64,
    dpi: f64,
) -> Result<resvg::tiny_skia::Pixmap, OutputError> {
    if !(dpi > 0.0 && dpi <= 2400.0) {
        return Err(OutputError::Size("DPI must be between 1 and 2400".into()));
    }
    let tree = parse(svg)?;
    let (w, h) = (pixel_size(width_mm, dpi), pixel_size(height_mm, dpi));
    if (w as u64) * (h as u64) > 400_000_000 {
        return Err(OutputError::Size(format!(
            "a {w} × {h} pixel image is too large; lower the DPI"
        )));
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| OutputError::Size("the image would be empty".into()))?;
    let ts = tree.size();
    let transform =
        resvg::tiny_skia::Transform::from_scale(w as f32 / ts.width(), h as f32 / ts.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Pixels of a rendered scene: width, height and 8-bit RGB rows with the drawing
/// composited onto white (journals want opaque artwork; a transparent background
/// becomes white). The same rasteriser as [`to_png`], so the two match pixel for pixel
/// where the PNG is opaque.
pub fn to_rgb(scene: &Scene, dpi: f64) -> Result<(u32, u32, Vec<u8>), OutputError> {
    let pm = rasterise(&to_svg(scene), scene.width_mm(), scene.height_mm(), dpi)?;
    let mut out = Vec::with_capacity(pm.width() as usize * pm.height() as usize * 3);
    for p in pm.pixels() {
        // Premultiplied: over white is c + (255 - a).
        let inv = 255 - p.alpha();
        out.extend_from_slice(&[
            p.red().saturating_add(inv),
            p.green().saturating_add(inv),
            p.blue().saturating_add(inv),
        ]);
    }
    Ok((pm.width(), pm.height(), out))
}

/// Render a scene to a one-page PDF with the fonts embedded as subsets.
pub fn to_pdf(scene: &Scene) -> Result<Vec<u8>, OutputError> {
    svg_to_pdf(&to_svg(scene))
}

/// Convert an SVG written by this engine to PDF.
pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, OutputError> {
    let tree = parse(svg)?;
    let conv = svg2pdf::ConversionOptions {
        embed_text: true,
        ..Default::default()
    };
    let page = svg2pdf::PageOptions { dpi: 72.0 };
    svg2pdf::to_pdf(&tree, conv, page).map_err(|e| OutputError::Pdf(e.to_string()))
}

/// Several SVGs written by this engine as the pages of one PDF, each page the size of
/// its SVG, fonts embedded as subsets (each page through svg2pdf, placed as a form
/// XObject).
pub fn svgs_to_pdf(pages: &[String]) -> Result<Vec<u8>, OutputError> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
    if pages.is_empty() {
        return Err(OutputError::Pdf("a PDF needs at least one page".into()));
    }
    let mut alloc = Ref::new(1);
    let catalog = alloc.bump();
    let tree_id = alloc.bump();
    let mut pdf = Pdf::new();
    let mut kids = Vec::new();
    let mut chunks = Vec::new();
    for svg in pages {
        let tree = parse(svg)?;
        let (w, h) = (tree.size().width(), tree.size().height());
        let conv = svg2pdf::ConversionOptions {
            embed_text: true,
            ..Default::default()
        };
        let (chunk, xref) =
            svg2pdf::to_chunk(&tree, conv).map_err(|e| OutputError::Pdf(e.to_string()))?;
        let mut map = std::collections::HashMap::new();
        let chunk = chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
        let xref = map[&xref];
        let page_id = alloc.bump();
        let content_id = alloc.bump();
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, w, h));
        page.parent(tree_id);
        page.contents(content_id);
        page.resources().x_objects().pair(Name(b"P"), xref);
        page.finish();
        let mut content = Content::new();
        content
            .transform([w, 0.0, 0.0, h, 0.0, 0.0])
            .x_object(Name(b"P"));
        pdf.stream(content_id, &content.finish());
        kids.push(page_id);
        chunks.push(chunk);
    }
    pdf.catalog(catalog).pages(tree_id);
    pdf.pages(tree_id)
        .kids(kids.iter().copied())
        .count(kids.len() as i32);
    for c in &chunks {
        pdf.extend(c);
    }
    Ok(pdf.finish())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in bytes {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Insert a `pHYs` chunk (pixels per metre) right after IHDR.
fn insert_phys(png: Vec<u8>, dpi: f64) -> Vec<u8> {
    // Signature (8) + IHDR chunk (4 len + 4 type + 13 data + 4 crc) = 33 bytes.
    if png.len() < 33 || &png[12..16] != b"IHDR" {
        return png;
    }
    let ppm = (dpi / 0.0254).round() as u32;
    let mut chunk = Vec::with_capacity(21);
    chunk.extend_from_slice(&9u32.to_be_bytes());
    let mut body = Vec::with_capacity(13);
    body.extend_from_slice(b"pHYs");
    body.extend_from_slice(&ppm.to_be_bytes());
    body.extend_from_slice(&ppm.to_be_bytes());
    body.push(1); // unit: metre
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    let mut out = Vec::with_capacity(png.len() + chunk.len());
    out.extend_from_slice(&png[..33]);
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[33..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_reference() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn pixel_rounding() {
        assert_eq!(pixel_size(85.0, 300.0), 1004);
        assert_eq!(pixel_size(174.0, 300.0), 2055);
        assert_eq!(pixel_size(25.4, 72.0), 72);
    }
}
