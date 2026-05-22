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
//! let icon = win_icon_extractor::extract_icon_at(r"C:\Windows\explorer.exe", 1).unwrap();
//!
//! // With caching (WebP on disk, mtime-aware staleness)
//! let cache = win_icon_extractor::IconCache::with_app_name("my-app").unwrap();
//! let path = cache.extract_to_file(r"C:\Windows\explorer.exe").unwrap();
//! ```

mod base64;
mod error;
mod extract;
mod resource;
mod stock;

#[cfg(feature = "webp")]
mod encode;

#[cfg(feature = "png")]
mod png;

#[cfg(feature = "cache")]
mod cache;

// ── Re-exports ──

pub use base64::{encode_base64, decode_base64, decode_image_base64};
pub use error::IconError;
pub use extract::IconData;
pub use resource::{get_max_icon_size, get_max_icon_size_wide};
pub use stock::StockIcon;

#[cfg(feature = "webp")]
pub use encode::WebPOptions;

#[cfg(feature = "cache")]
pub use cache::{CacheStats, IconCache};

#[cfg(all(feature = "cache", any(feature = "webp", feature = "png")))]
pub use cache::ImageFormat;

/// Extract icon as raw RGBA pixels (index 0).
pub fn extract_icon(path: &str) -> Result<IconData, IconError> {
    extract::extract_icon(path)
}

/// Extract icon at a specific index from a file.
pub fn extract_icon_at(path: &str, index: u32) -> Result<IconData, IconError> {
    extract::extract_icon_at(path, index)
}

/// Extract icon at a specific size (best-effort).
pub fn extract_icon_with_size(path: &str, size: u32) -> Result<IconData, IconError> {
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
    rgba: &[u8], width: u32, height: u32, opts: &WebPOptions,
) -> Result<Vec<u8>, IconError> {
    encode::encode_webp_with(rgba, width, height, opts)
}

/// Extract icon and encode to WebP in one step.
#[cfg(feature = "webp")]
pub fn extract_icon_webp(path: &str) -> Result<Vec<u8>, IconError> {
    let data = extract::extract_icon(path)?;
    encode::encode_webp(&data.rgba, data.width, data.height)
}

#[cfg(feature = "png")]
pub use png::{PngFilter, PngOptions};

/// Encode RGBA pixels to PNG bytes.
#[cfg(feature = "png")]
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, IconError> {
    png::encode_png(rgba, width, height)
}

/// Encode RGBA pixels to PNG with custom options.
#[cfg(feature = "png")]
pub fn encode_png_with(
    rgba: &[u8], width: u32, height: u32, opts: &PngOptions,
) -> Result<Vec<u8>, IconError> {
    png::encode_png_with(rgba, width, height, opts)
}

/// Extract icon and encode to PNG in one step.
#[cfg(feature = "png")]
pub fn extract_icon_png(path: &str) -> Result<Vec<u8>, IconError> {
    let data = extract::extract_icon(path)?;
    png::encode_png(&data.rgba, data.width, data.height)
}

/// Bulk extract icons in parallel, returning a map of path → Result.
#[cfg(feature = "bulk")]
pub fn extract_icons_bulk(
    paths: &[&str],
) -> std::collections::HashMap<String, Result<IconData, IconError>> {
    use rayon::prelude::*;
    paths
        .par_iter()
        .map(|&p| (p.to_string(), extract::extract_icon(p)))
        .collect()
}

/// Query the number of icons in a file (.exe, .dll, .ico).
pub fn icon_count(path: &str) -> u32 {
    extract::icon_count(path)
}

/// Extract the associated icon for a file extension (file need not exist).
/// `ext` should include the dot, e.g. ".pdf", ".docx".
pub fn extract_icon_for_extension(ext: &str) -> Result<IconData, IconError> {
    extract::extract_icon_for_extension(ext)
}

/// Extract a system stock icon.
pub fn extract_stock_icon(icon: StockIcon) -> Result<IconData, IconError> {
    stock::extract_stock_icon(icon)
}

/// Extract a system stock icon at a specific size.
pub fn extract_stock_icon_sized(icon: StockIcon, size: u32) -> Result<IconData, IconError> {
    stock::extract_stock_icon_sized(icon, size as i32)
}

/// 提取图标并将其编码为 WebP 格式的 Base64 字符串。
#[cfg(feature = "webp")]
pub fn extract_icon_webp_base64(path: &str) -> Result<String, IconError> {
    let bytes = extract_icon_webp(path)?;
    Ok(encode_base64(&bytes))
}

/// 提取图标并将其编码为 PNG 格式的 Base64 字符串。
#[cfg(feature = "png")]
pub fn extract_icon_png_base64(path: &str) -> Result<String, IconError> {
    let bytes = extract_icon_png(path)?;
    Ok(encode_base64(&bytes))
}
