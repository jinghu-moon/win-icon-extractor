//! WebP encoding for icon pixel data

use crate::error::IconError;
use libwebp_sys::*;
use std::ffi::c_void;
use std::slice;

/// WebP encoding options.
#[derive(Debug, Clone, Copy)]
pub struct WebPOptions {
    /// Quality factor (0.0–100.0). Default: 75.0
    pub quality: f32,
    /// Compression method (0=fast, 6=slowest/best). Default: 5
    pub method: i32,
    /// Lossless encoding. Default: false
    pub lossless: bool,
    /// Alpha channel quality (0–100). Default: 100
    pub alpha_quality: i32,
    /// Preserve RGB values under transparent areas. Default: false
    pub exact: bool,
}

impl Default for WebPOptions {
    fn default() -> Self {
        Self { quality: 75.0, method: 5, lossless: false, alpha_quality: 100, exact: false }
    }
}

impl WebPOptions {
    /// Best visual quality preset: lossless, max effort.
    pub fn best_quality() -> Self {
        Self { quality: 100.0, method: 6, lossless: true, alpha_quality: 100, exact: true }
    }
}

unsafe extern "C" fn webp_writer(
    data: *const u8,
    data_size: usize,
    picture: *const WebPPicture,
) -> i32 {
    let buf = &mut *((*picture).custom_ptr as *mut Vec<u8>);
    buf.extend_from_slice(slice::from_raw_parts(data, data_size));
    1
}

/// Encode RGBA pixels to WebP with default options.
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
            "RGBA buffer size mismatch: expected {expected}, got {}", rgba.len()
        )));
    }
    unsafe {
        let mut config: WebPConfig = std::mem::zeroed();
        if WebPConfigInitInternal(
            &mut config,
            WebPPreset::WEBP_PRESET_ICON,
            opts.quality,
            WEBP_ENCODER_ABI_VERSION as i32,
        ) == 0
        {
            return Err(IconError::Encode("WebPConfig init failed".into()));
        }
        config.method = opts.method;
        config.lossless = opts.lossless as i32;
        config.alpha_quality = opts.alpha_quality;
        config.exact = opts.exact as i32;
        config.thread_level = 0;

        let mut pic: WebPPicture = std::mem::zeroed();
        if WebPPictureInitInternal(&mut pic, WEBP_ENCODER_ABI_VERSION as i32) == 0 {
            return Err(IconError::Encode("WebPPicture init failed".into()));
        }

        pic.use_argb = 1;
        pic.width = width as i32;
        pic.height = height as i32;

        if WebPPictureImportRGBA(&mut pic, rgba.as_ptr(), width as i32 * 4) == 0 {
            WebPPictureFree(&mut pic);
            return Err(IconError::Encode("RGBA import failed".into()));
        }

        let mut output: Vec<u8> = Vec::with_capacity(4096);
        pic.writer = Some(webp_writer);
        pic.custom_ptr = &mut output as *mut _ as *mut c_void;

        let ok = WebPEncode(&config, &mut pic);
        WebPPictureFree(&mut pic);

        if ok == 1 {
            Ok(output)
        } else {
            Err(IconError::Encode("WebP encode failed".into()))
        }
    }
}
