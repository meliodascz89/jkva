//! Pixel comparison for JKva reference tests.
//!
//! Reference tests compare a screenshot produced by the engine with an expected
//! image. This crate keeps the comparison deterministic and explainable: it reports
//! the number of differing pixels, the largest channel delta, and can write a diff
//! image with the differing pixels painted red.

#![deny(missing_docs)]

/// Result of comparing two images of identical dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiffReport {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Number of pixels whose largest channel delta exceeded the threshold.
    pub differing: u64,
    /// Total number of pixels.
    pub total: u64,
    /// Largest per-channel delta observed, 0-255.
    pub max_delta: u8,
}

impl DiffReport {
    /// Fraction of differing pixels, `0.0..=1.0`.
    #[must_use]
    pub fn ratio(self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.differing as f64 / self.total as f64
    }
}

/// Compares two RGBA8 buffers of the same size.
///
/// A pixel differs when the largest delta of any of its four channels is greater
/// than `threshold`. A threshold of `0` means "any difference counts".
///
/// # Errors
///
/// Returns an error when the buffers do not match the declared dimensions.
pub fn compare_rgba(
    expected: &[u8],
    actual: &[u8],
    width: u32,
    height: u32,
    threshold: u8,
) -> Result<DiffReport, String> {
    let need = width as usize * height as usize * 4;
    if expected.len() != need || actual.len() != need {
        return Err(format!(
            "buffer size mismatch for {width}x{height}: expected {need} bytes, got {} and {}",
            expected.len(),
            actual.len()
        ));
    }
    let mut differing = 0u64;
    let mut max_delta = 0u8;
    for (e, a) in expected.chunks_exact(4).zip(actual.chunks_exact(4)) {
        let delta = (0..4)
            .map(|i| e[i].abs_diff(a[i]))
            .max()
            .unwrap_or_default();
        max_delta = max_delta.max(delta);
        if delta > threshold {
            differing += 1;
        }
    }
    Ok(DiffReport {
        width,
        height,
        differing,
        total: u64::from(width) * u64::from(height),
        max_delta,
    })
}

/// Builds a diff image: the actual pixels, with differing pixels painted red.
///
/// # Errors
///
/// Returns an error when the buffers do not match the declared dimensions.
pub fn diff_rgba(
    expected: &[u8],
    actual: &[u8],
    width: u32,
    height: u32,
    threshold: u8,
) -> Result<Vec<u8>, String> {
    let need = width as usize * height as usize * 4;
    if expected.len() != need || actual.len() != need {
        return Err(format!(
            "buffer size mismatch for {width}x{height}: expected {need} bytes, got {} and {}",
            expected.len(),
            actual.len()
        ));
    }
    let mut out = actual.to_vec();
    for (i, (e, a)) in expected
        .chunks_exact(4)
        .zip(actual.chunks_exact(4))
        .enumerate()
    {
        let differs = (0..4).any(|c| e[c].abs_diff(a[c]) > threshold);
        if differs {
            out[i * 4..i * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
    }
    Ok(out)
}

/// Decodes a PNG file into an RGBA8 buffer and its dimensions.
///
/// Supports 8-bit RGB, RGBA, grayscale and grayscale+alpha. Palette images are
/// expanded. 16-bit images are rejected: reference tests must be 8-bit.
///
/// # Errors
///
/// Returns an error for unreadable files, unsupported bit depths or corrupt data.
pub fn decode_png(path: &std::path::Path) -> Result<(Vec<u8>, u32, u32), String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "{}: only 8-bit PNGs are supported, got {:?}",
            path.display(),
            info.bit_depth
        ));
    }
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err(format!(
                "{}: indexed PNGs should have been expanded",
                path.display()
            ));
        }
    };
    Ok((rgba, info.width, info.height))
}

/// Writes an RGBA8 buffer to a PNG file.
///
/// # Errors
///
/// Returns an error when the file cannot be created or written.
pub fn write_png(
    path: &std::path::Path,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, px: [u8; 4]) -> Vec<u8> {
        px.iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 4)
            .collect()
    }

    #[test]
    fn identical_images_have_zero_difference() {
        let a = solid(4, 3, [10, 20, 30, 255]);
        let b = a.clone();
        let r = compare_rgba(&a, &b, 4, 3, 0).unwrap();
        assert_eq!(r.differing, 0);
        assert_eq!(r.total, 12);
        assert!(r.ratio() < f64::EPSILON);
    }

    #[test]
    fn one_pixel_difference_is_counted() {
        let a = solid(4, 3, [0, 0, 0, 255]);
        let mut b = a.clone();
        b[0] = 255;
        let r = compare_rgba(&a, &b, 4, 3, 0).unwrap();
        assert_eq!(r.differing, 1);
        assert_eq!(r.max_delta, 255);
        assert!((r.ratio() - 1.0 / 12.0).abs() < 1e-9);
    }

    #[test]
    fn threshold_ignores_small_deltas() {
        let a = solid(2, 2, [100, 100, 100, 255]);
        let b = solid(2, 2, [104, 100, 100, 255]);
        // delta 4 is "within" a threshold of 4, and "different" at 3
        assert_eq!(compare_rgba(&a, &b, 2, 2, 4).unwrap().differing, 0);
        assert_eq!(compare_rgba(&a, &b, 2, 2, 3).unwrap().differing, 4);
        assert_eq!(compare_rgba(&a, &b, 2, 2, 3).unwrap().max_delta, 4);
    }

    #[test]
    fn diff_image_paints_changed_pixels_red() {
        let a = solid(2, 1, [0, 0, 0, 255]);
        let mut b = a.clone();
        b[4..8].copy_from_slice(&[9, 9, 9, 255]);
        let d = diff_rgba(&a, &b, 2, 1, 0).unwrap();
        assert_eq!(&d[0..4], &[0, 0, 0, 255]);
        assert_eq!(&d[4..8], &[255, 0, 0, 255]);
    }

    #[test]
    fn dimension_mismatch_is_an_error() {
        let a = vec![0u8; 16];
        let b = vec![0u8; 12];
        assert!(compare_rgba(&a, &b, 2, 2, 0).is_err());
    }
}
