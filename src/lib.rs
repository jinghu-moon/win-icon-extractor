#![cfg(windows)]
//! Extract file icons on Windows — pure Rust, no C dependency.
//!
//! # Quick Start
//! ```no_run
//! // Get raw RGBA pixels
//! let icon = win_icon_extractor::extract_icon(r"C:\Windows\explorer.exe").unwrap();
//! println!("{}x{}, {} bytes", icon.width, icon.height, icon.rgba.len());
//!
//! // Extract by index
//! let icon = win_icon_extractor::extract_icon_at(r"C:\Windows\System32\shell32.dll", 1).unwrap();
//!
//! // High-quality downscale from the largest native frame
//! # #[cfg(feature = "resize")]
//! let icon = win_icon_extractor::extract_icon_best(r"C:\Windows\explorer.exe", 32).unwrap();
//!
//! // Disk cache (feature = "cache")
//! # #[cfg(feature = "cache")]
//! let cache = win_icon_extractor::IconCache::with_app_name("my-app").unwrap();
//! # #[cfg(feature = "cache")]
//! let path = cache.extract_to_file(r"C:\Windows\explorer.exe").unwrap();
//! ```

mod error;
mod extract;
mod resource;
mod stock;

#[cfg(feature = "webp")]
mod encode;

#[cfg(feature = "webp")]
mod webp_pure;

#[cfg(feature = "webp-libwebp")]
mod webp_libwebp;

#[cfg(feature = "png")]
mod png;

#[cfg(feature = "cache")]
mod cache;

#[cfg(feature = "resize")]
mod scale;

use std::path::Path;

// ── Re-exports ──

pub use error::IconError;
pub use extract::IconData;
pub use resource::{
    get_max_icon_size, get_max_icon_size_wide, list_icon_sizes, list_icon_sizes_wide,
};
pub use stock::StockIcon;

#[cfg(feature = "webp")]
pub use encode::WebPOptions;

#[cfg(feature = "png")]
pub use png::{PngFilter, PngOptions};

#[cfg(feature = "cache")]
pub use cache::{extract_icon_for_extension_cached, CacheStats, IconCache, IconCacheBuilder};

#[cfg(all(feature = "cache", any(feature = "webp", feature = "png")))]
pub use cache::ImageFormat;

#[cfg(feature = "resize")]
pub use scale::{
    extract_icon_best, extract_icon_best_with, resize_icon, resize_rgba, ResizeFilter,
};

/// Extract the default icon as raw RGBA pixels.
pub fn extract_icon(path: impl AsRef<Path>) -> Result<IconData, IconError> {
    extract::extract_icon(path)
}

/// Extract icon at a specific index from a file.
///
/// For `index > 0` there is no shell fallback — failing to find that index is an error.
pub fn extract_icon_at(path: impl AsRef<Path>, index: u32) -> Result<IconData, IconError> {
    extract::extract_icon_at(path, index)
}

/// Extract icon at a specific size (best-effort).
///
/// `IconData.width/height` always report the pixels actually returned.
pub fn extract_icon_with_size(path: impl AsRef<Path>, size: u32) -> Result<IconData, IconError> {
    extract::extract_icon_with_size(path, size)
}

/// Encode RGBA pixels to WebP bytes (default options).
#[cfg(feature = "webp")]
pub fn encode_webp(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, IconError> {
    encode::encode_webp(rgba, width, height)
}

/// Encode RGBA pixels to WebP with custom options.
#[cfg(feature = "webp")]
pub fn encode_webp_with(
    rgba: &[u8],
    width: u32,
    height: u32,
    opts: &WebPOptions,
) -> Result<Vec<u8>, IconError> {
    encode::encode_webp_with(rgba, width, height, opts)
}

/// Extract icon and encode to WebP in one step.
#[cfg(feature = "webp")]
pub fn extract_icon_webp(path: impl AsRef<Path>) -> Result<Vec<u8>, IconError> {
    let data = extract::extract_icon(path)?;
    encode::encode_webp(&data.rgba, data.width, data.height)
}

/// Encode RGBA pixels to PNG bytes.
#[cfg(feature = "png")]
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, IconError> {
    png::encode_png(rgba, width, height)
}

/// Encode RGBA pixels to PNG with custom options.
#[cfg(feature = "png")]
pub fn encode_png_with(
    rgba: &[u8],
    width: u32,
    height: u32,
    opts: &PngOptions,
) -> Result<Vec<u8>, IconError> {
    png::encode_png_with(rgba, width, height, opts)
}

/// Extract icon and encode to PNG in one step.
#[cfg(feature = "png")]
pub fn extract_icon_png(path: impl AsRef<Path>) -> Result<Vec<u8>, IconError> {
    let data = extract::extract_icon(path)?;
    png::encode_png(&data.rgba, data.width, data.height)
}

/// Bulk extract icons in parallel. Results are returned in **input order**.
#[cfg(feature = "bulk")]
pub fn extract_icons_bulk(paths: &[&str]) -> Vec<(String, Result<IconData, IconError>)> {
    use rayon::prelude::*;
    paths
        .par_iter()
        .map(|&p| (p.to_string(), extract::extract_icon(p)))
        .collect()
}

/// Query the number of icons in a file (.exe, .dll, .ico).
///
/// `Ok(0)` means the file has no icons. Errors only when the extraction
/// entry point is unavailable.
pub fn icon_count(path: impl AsRef<Path>) -> Result<u32, IconError> {
    extract::icon_count(path)
}

/// Extract the associated icon for a file extension (file need not exist).
/// `ext` should include the dot, e.g. ".pdf", ".docx".
pub fn extract_icon_for_extension(ext: &str) -> Result<IconData, IconError> {
    extract::extract_icon_for_extension(ext)
}

/// Extract the associated icon for a file extension at a specific size (0 = system).
pub fn extract_icon_for_extension_sized(ext: &str, size: u32) -> Result<IconData, IconError> {
    extract::extract_icon_for_extension_sized(ext, size)
}

/// Extract a system stock icon.
pub fn extract_stock_icon(icon: StockIcon) -> Result<IconData, IconError> {
    stock::extract_stock_icon(icon)
}

/// Extract a system stock icon at a specific size.
pub fn extract_stock_icon_sized(icon: StockIcon, size: u32) -> Result<IconData, IconError> {
    stock::extract_stock_icon_sized(icon, size)
}
