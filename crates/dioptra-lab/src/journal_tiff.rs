// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! TIFF export: 8-bit RGB, LZW-compressed, resolution in pixels per inch, rendered from the
//! same scene as SVG, PDF and PNG.

use std::io::Cursor;

use dioptra_plot::scene::Scene;
use tiff::encoder::{colortype, Compression, Rational, TiffEncoder};
use tiff::tags::ResolutionUnit;

/// Encode RGB rows as an LZW TIFF at `dpi`.
pub fn encode(width: u32, height: u32, rgb: &[u8], dpi: f64) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    let mut enc = TiffEncoder::new(&mut out)
        .map_err(|e| e.to_string())?
        .with_compression(Compression::Lzw);
    let mut img = enc
        .new_image::<colortype::RGB8>(width, height)
        .map_err(|e| e.to_string())?;
    img.resolution(
        ResolutionUnit::Inch,
        Rational {
            n: (dpi * 100.0).round() as u32,
            d: 100,
        },
    );
    img.write_data(rgb).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// A scene as an LZW TIFF at `dpi`.
pub fn scene_to_tiff(scene: &Scene, dpi: f64) -> Result<Vec<u8>, String> {
    let (w, h, rgb) = dioptra_plot::output::to_rgb(scene, dpi).map_err(|e| e.to_string())?;
    encode(w, h, &rgb, dpi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiff::decoder::{Decoder, DecodingResult};
    use tiff::tags::Tag;

    #[test]
    fn tiff_round_trips_with_lzw_and_resolution() {
        let (spec, data) = dioptra_plot::samples::sample("column").unwrap();
        let scene = dioptra_plot::scene(&spec, &data).unwrap();
        let bytes = scene_to_tiff(&scene, 300.0).unwrap();
        assert_eq!(&bytes[..4], b"II*\0");
        let mut d = Decoder::new(Cursor::new(&bytes)).unwrap();
        let (w, h) = d.dimensions().unwrap();
        assert_eq!(w, 1004); // 85 mm at 300 dpi
        assert_eq!(h, dioptra_plot::output::pixel_size(70.0, 300.0));
        assert_eq!(d.get_tag_u32(Tag::Compression).unwrap(), 5); // LZW
        assert_eq!(d.get_tag_u32(Tag::ResolutionUnit).unwrap(), 2); // inch
        let DecodingResult::U8(px) = d.read_image().unwrap() else {
            panic!("8-bit image expected")
        };
        assert_eq!(px.len(), (w * h * 3) as usize);
        // White paper in the corner, ink somewhere.
        assert_eq!(&px[..3], &[255, 255, 255]);
        assert!(px.chunks(3).any(|p| p[0] < 64 && p[1] < 64 && p[2] < 64));
        // Same pixels as the encoder was given.
        let (_, _, rgb) = dioptra_plot::output::to_rgb(&scene, 300.0).unwrap();
        assert_eq!(px, rgb);
        // LZW makes a flat drawing much smaller than raw RGB.
        assert!(
            bytes.len() < rgb.len() / 4,
            "{} vs {}",
            bytes.len(),
            rgb.len()
        );
    }
}
