//! Pure-Rust lossless WebP encoder (`image-webp`).
//!
//! Ideal for icons: preserves sharp edges and the alpha channel exactly.

use crate::error::IconError;
use image_webp::{ColorType, EncoderParams, WebPEncoder};

/// Encode RGBA → lossless WebP (VP8L).
pub(crate) fn encode_webp_lossless(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, IconError> {
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(IconError::Encode(format!(
            "invalid dimensions: {width}x{height}"
        )));
    }

    let mut out = Vec::new();
    let mut enc = WebPEncoder::new(&mut out);
    enc.set_params(EncoderParams::default());
    enc.encode(rgba, width, height, ColorType::Rgba8)
        .map_err(|e| IconError::Encode(format!("image-webp: {e}")))?;
    Ok(out)
}
