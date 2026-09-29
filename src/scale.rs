//! High-quality RGBA resampling for icon downscale pipelines.
//!
//! Uses `resize::Pixel::RGBA8P` so scaling happens in premultiplied-alpha
//! space — soft shadows and translucent edges do not pick up fringes.

use crate::error::IconError;
use crate::extract::IconData;
use resize::px::RGBA;
use std::path::Path;

/// Resampling filter for the high-quality pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ResizeFilter {
    /// Lanczos3 — sharp, best general default for downscales.
    #[default]
    Lanczos3,
    /// Mitchell–Netravali — softer than Lanczos, fewer ringing artifacts.
    Mitchell,
    /// Catmull-Rom — sharp cubic.
    CatmullRom,
    /// Triangle (bilinear) — smooth, fast.
    Bilinear,
    /// Box / area average — ideal for integer-ratio downscales (256→32).
    Box,
}

impl ResizeFilter {
    fn to_type(self) -> resize::Type {
        match self {
            Self::Lanczos3 => resize::Type::Lanczos3,
            Self::Mitchell => resize::Type::Mitchell,
            Self::CatmullRom => resize::Type::Catrom,
            Self::Bilinear => resize::Type::Triangle,
            // Box kernel with 0.5 support = classic box/area filter.
            Self::Box => resize::Type::Custom(resize::Filter::box_filter(0.5)),
        }
    }
}

/// Resize raw RGBA pixels. `rgba` must be `width*height*4` bytes.
pub fn resize_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    new_width: u32,
    new_height: u32,
    filter: ResizeFilter,
) -> Result<IconData, IconError> {
    let (sw, sh, dw, dh) = (
        width as usize,
        height as usize,
        new_width as usize,
        new_height as usize,
    );
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return Err(IconError::Resize("zero dimension".into()));
    }
    let expected = sw * sh * 4;
    if rgba.len() != expected {
        return Err(IconError::Resize(format!(
            "RGBA buffer size mismatch: expected {expected}, got {}",
            rgba.len()
        )));
    }
    if sw == dw && sh == dh {
        return Ok(IconData {
            rgba: rgba.to_vec(),
            width: new_width,
            height: new_height,
        });
    }

    let src: Vec<RGBA<u8>> = rgba
        .chunks_exact(4)
        .map(|p| RGBA::new(p[0], p[1], p[2], p[3]))
        .collect();
    let mut dst: Vec<RGBA<u8>> = vec![RGBA::new(0, 0, 0, 0); dw * dh];

    let mut resizer = resize::Resizer::new(sw, sh, dw, dh, resize::Pixel::RGBA8P, filter.to_type())
        .map_err(|e| IconError::Resize(format!("resizer init: {e}")))?;
    resizer
        .resize(&src, &mut dst)
        .map_err(|e| IconError::Resize(format!("resize: {e}")))?;

    let mut out = Vec::with_capacity(dw * dh * 4);
    for p in &dst {
        out.extend_from_slice(&[p.r, p.g, p.b, p.a]);
    }

    Ok(IconData {
        rgba: out,
        width: new_width,
        height: new_height,
    })
}

/// Scale an [`IconData`] to `new_width × new_height`.
pub fn resize_icon(
    icon: &IconData,
    new_width: u32,
    new_height: u32,
    filter: ResizeFilter,
) -> Result<IconData, IconError> {
    resize_rgba(
        &icon.rgba,
        icon.width,
        icon.height,
        new_width,
        new_height,
        filter,
    )
}

/// Extract the largest native frame and resample it to `size × size`.
///
/// This is the high-quality path: unlike [`crate::extract_icon_with_size`],
/// GDI never scales the pixels — the full-res frame is the only source.
pub fn extract_icon_best(path: impl AsRef<Path>, size: u32) -> Result<IconData, IconError> {
    extract_icon_best_with(path, size, ResizeFilter::default())
}

/// [`extract_icon_best`] with an explicit filter.
pub fn extract_icon_best_with(
    path: impl AsRef<Path>,
    size: u32,
    filter: ResizeFilter,
) -> Result<IconData, IconError> {
    let size = size.clamp(1, 256);
    let big = crate::extract::extract_icon(path)?;
    if big.width == size && big.height == size {
        return Ok(big);
    }
    resize_icon(&big, size, size, filter)
}

impl IconData {
    /// High-quality resample to `new_width × new_height`.
    pub fn resized(
        &self,
        new_width: u32,
        new_height: u32,
        filter: ResizeFilter,
    ) -> Result<IconData, IconError> {
        resize_icon(self, new_width, new_height, filter)
    }

    /// High-quality resample to a square `size`.
    pub fn resized_square(&self, size: u32, filter: ResizeFilter) -> Result<IconData, IconError> {
        let s = size.clamp(1, 256);
        resize_icon(self, s, s, filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            v.extend_from_slice(&[r, g, b, a]);
        }
        v
    }

    #[test]
    fn resize_same_size_is_identity() {
        let rgba = solid(8, 8, 1, 2, 3, 255);
        let out = resize_rgba(&rgba, 8, 8, 8, 8, ResizeFilter::Lanczos3).unwrap();
        assert_eq!(out.rgba, rgba);
    }

    #[test]
    fn resize_downscale_preserves_opaque_color() {
        // Fully opaque solid red 256→32 should stay red.
        let rgba = solid(256, 256, 200, 10, 10, 255);
        let out = resize_rgba(&rgba, 256, 256, 32, 32, ResizeFilter::Lanczos3).unwrap();
        assert_eq!(out.width, 32);
        assert_eq!(out.height, 32);
        for px in out.rgba.chunks_exact(4) {
            assert_eq!(px[3], 255);
            assert!((px[0] as i32 - 200).abs() <= 2, "R drifted: {}", px[0]);
        }
    }

    #[test]
    fn resize_rejects_bad_buffer() {
        let rgba = vec![0u8; 3];
        assert!(resize_rgba(&rgba, 2, 2, 1, 1, ResizeFilter::Box).is_err());
    }

    #[test]
    fn transparent_pixels_stay_transparent() {
        let mut rgba = solid(64, 64, 255, 255, 255, 0);
        // Opaque center square
        for y in 20..44 {
            for x in 20..44 {
                let i = ((y * 64 + x) * 4) as usize;
                rgba[i + 3] = 255;
            }
        }
        let out = resize_rgba(&rgba, 64, 64, 16, 16, ResizeFilter::Lanczos3).unwrap();
        // Corners must remain (nearly) transparent
        let corner_a = out.rgba[3];
        assert!(corner_a < 16, "corner alpha too high: {corner_a}");
    }
}
