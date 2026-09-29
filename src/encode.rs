//! WebP encoding — shared options + backend dispatch.
//!
//! Backends:
//! - `webp` (default): pure-Rust lossless via `image-webp` — no C dependency
//! - `webp-libwebp`: optional `libwebp-sys` for lossy / full control

use crate::error::IconError;

/// WebP encoding options.
///
/// Fields other than [`lossless`](Self::lossless) are only applied by the
/// `webp-libwebp` backend. The pure-Rust backend always encodes lossless
/// (ideal for icons: sharp edges + alpha).
#[derive(Debug, Clone, Copy)]
pub struct WebPOptions {
    /// Quality factor (0.0–100.0) for lossy. Default: 75.0
    pub quality: f32,
    /// Compression method (0=fast, 6=slowest/best). Default: 5
    pub method: i32,
    /// Lossless encoding. Default: true (icon-friendly).
    pub lossless: bool,
    /// Alpha channel quality (0–100). Default: 100
    pub alpha_quality: i32,
    /// Preserve RGB values under transparent areas. Default: false
    pub exact: bool,
}

impl Default for WebPOptions {
    fn default() -> Self {
        Self {
            quality: 75.0,
            method: 5,
            lossless: true, // pure-Rust friendly default
            alpha_quality: 100,
            exact: false,
        }
    }
}

impl WebPOptions {
    /// Best visual quality preset: lossless, max effort.
    pub fn best_quality() -> Self {
        Self {
            quality: 100.0,
            method: 6,
            lossless: true,
            alpha_quality: 100,
            exact: true,
        }
    }

    /// Fast lossy preset (requires `webp-libwebp`).
    pub fn lossy(quality: f32) -> Self {
        Self {
            quality,
            method: 3,
            lossless: false,
            alpha_quality: 90,
            exact: false,
        }
    }
}

/// Encode RGBA pixels to WebP bytes (default options).
pub fn encode_webp(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, IconError> {
    encode_webp_with(rgba, width, height, &WebPOptions::default())
}

/// Encode RGBA pixels to WebP with custom options.
pub fn encode_webp_with(
    rgba: &[u8],
    width: u32,
    height: u32,
    opts: &WebPOptions,
) -> Result<Vec<u8>, IconError> {
    let expected = (width as usize) * (height as usize) * 4;
    if rgba.len() != expected {
        return Err(IconError::Encode(format!(
            "RGBA buffer size mismatch: expected {expected}, got {}",
            rgba.len()
        )));
    }

    // Lossy (or explicit libwebp knobs) → libwebp backend when compiled in.
    #[cfg(feature = "webp-libwebp")]
    {
        if !opts.lossless {
            return super::webp_libwebp::encode_webp_libwebp(rgba, width, height, opts);
        }
    }

    #[cfg(not(feature = "webp-libwebp"))]
    {
        if !opts.lossless {
            return Err(IconError::Encode(
                "lossy WebP requires the `webp-libwebp` feature; \
                 or set WebPOptions.lossless = true (pure-Rust backend)"
                    .into(),
            ));
        }
    }

    // Pure-Rust lossless (always available with feature "webp").
    super::webp_pure::encode_webp_lossless(rgba, width, height)
}
