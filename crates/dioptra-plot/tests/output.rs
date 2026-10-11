//! PDF and PNG output: PDFs start with a header and embed font programs; PNGs have the
//! pixel size the millimetres and DPI ask for (rounded to the nearest pixel) and record
//! the DPI in a pHYs chunk.

use dioptra_plot::output::pixel_size;
use dioptra_plot::samples::sample;
use dioptra_plot::{compose_layout, scene, to_pdf, to_png};

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn every_pdf_has_a_header_and_embedded_fonts() {
    for f in [
        "scatter",
        "column",
        "kaplan_meier",
        "heatmap",
        "pie",
        "forest",
        "volcano",
    ] {
        let (spec, data) = sample(f).unwrap();
        let pdf = to_pdf(&scene(&spec, &data).unwrap()).unwrap();
        assert!(pdf.starts_with(b"%PDF-1."), "{f}: header");
        assert!(
            contains(&pdf, b"/FontFile2") || contains(&pdf, b"/FontFile3"),
            "{f}: no embedded font program"
        );
        assert!(contains(&pdf, b"%%EOF"), "{f}: trailer");
        // The page is the figure size in points (85 mm = 240.94 pt).
        assert!(contains(&pdf, b"/MediaBox [0 0 240.94"), "{f}: page size");
    }
}

#[test]
fn png_dimensions_follow_mm_and_dpi() {
    let (spec, data) = sample("scatter").unwrap();
    let sc = scene(&spec, &data).unwrap();
    for dpi in [72.0, 150.0, 300.0, 600.0] {
        let png = to_png(&sc, dpi).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(w, pixel_size(85.0, dpi));
        assert_eq!(h, pixel_size(70.0, dpi));
        // pHYs right after IHDR, pixels per metre.
        assert_eq!(&png[37..41], b"pHYs");
        let ppm = u32::from_be_bytes(png[41..45].try_into().unwrap());
        assert_eq!(ppm, (dpi / 0.0254).round() as u32);
    }
    // 85 mm at 300 dpi: 85 / 25.4 × 300 = 1003.94 → 1004 px.
    assert_eq!(pixel_size(85.0, 300.0), 1004);
    assert_eq!(pixel_size(70.0, 300.0), 827);
}

#[test]
fn png_is_not_blank() {
    let (spec, data) = sample("column").unwrap();
    let png = to_png(&scene(&spec, &data).unwrap(), 72.0).unwrap();
    // A white image of this size compresses to well under a kilobyte.
    assert!(png.len() > 4000, "{} bytes", png.len());
}

#[test]
fn layouts_compose_with_letters_and_export() {
    let a = sample("column").unwrap();
    let b = sample("scatter").unwrap();
    let c = sample("kaplan_meier").unwrap();
    let scenes = vec![
        scene(&a.0, &a.1).unwrap(),
        scene(&b.0, &b.1).unwrap(),
        scene(&c.0, &c.1).unwrap(),
    ];
    let page = compose_layout(&scenes, 2, true);
    assert!((page.width_mm() - 170.0).abs() < 1e-9);
    assert!((page.height_mm() - 140.0).abs() < 1e-9);
    let svg = dioptra_plot::to_svg(&page);
    for l in ["A", "B", "C"] {
        assert!(
            svg.contains(&format!("font-weight=\"700\">{l}</text>")),
            "panel letter {l}"
        );
    }
    assert!(svg.contains("id=\"p1-axis-x\""));
    let ids: Vec<String> = page.targets().into_iter().map(|t| t.id).collect();
    let mut dedup = ids.clone();
    dedup.sort();
    dedup.dedup();
    assert_eq!(ids.len(), dedup.len(), "ids are unique");
    let pdf = to_pdf(&page).unwrap();
    assert!(pdf.starts_with(b"%PDF-1."));
}
