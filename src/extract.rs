//! Icon extraction: HICON acquisition + RGBA pixel conversion

use crate::error::IconError;
use crate::resource;
use std::sync::OnceLock;
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Raw RGBA pixel data extracted from an icon.
pub struct IconData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// ── Helpers ──

/// Convert &str to null-terminated UTF-16. Allocated once, reused across calls.
#[inline]
pub(crate) fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ── RAII handle wrappers ──

macro_rules! auto_handle {
    ($vis:vis $name:ident, $type:ty, $drop:expr) => {
        $vis struct $name(pub $type);
        impl Drop for $name {
            fn drop(&mut self) {
                if !self.0.is_invalid() {
                    unsafe { $drop(self.0) };
                }
            }
        }
    };
}

auto_handle!(pub(crate) AutoIcon, HICON, |h| { let _ = DestroyIcon(h); });
auto_handle!(AutoDC, HDC, |h| { let _ = DeleteDC(h); });
auto_handle!(ScreenDC, HDC, |h| { let _ = ReleaseDC(None, h); });
auto_handle!(AutoGdiObj, HGDIOBJ, |h| { let _ = DeleteObject(h); });

// ── PrivateExtractIconsW (undocumented but widely used) ──

type PrivateExtractIconsWFn =
    unsafe extern "system" fn(PCWSTR, i32, i32, i32, *mut HICON, *mut u32, u32, u32) -> u32;

static PRIVATE_EXTRACT_FN: OnceLock<Option<PrivateExtractIconsWFn>> = OnceLock::new();

fn load_private_extract() -> Option<PrivateExtractIconsWFn> {
    *PRIVATE_EXTRACT_FN.get_or_init(|| unsafe {
        let wide = to_wide("user32.dll");
        let lib = LoadLibraryW(PCWSTR(wide.as_ptr())).ok()?;
        let proc = GetProcAddress(lib, PCSTR(b"PrivateExtractIconsW\0".as_ptr()))?;
        Some(std::mem::transmute(proc))
    })
}

// ── Helpers: zero-alloc extension check + BGRA→RGBA ──

/// Check PE-like extension without allocating (no to_lowercase).
#[inline]
fn has_pe_extension(path: &str) -> bool {
    let b = path.as_bytes();
    if b.len() < 4 || b[b.len() - 4] != b'.' { return false; }
    let ext = [b[b.len() - 3] | 0x20, b[b.len() - 2] | 0x20, b[b.len() - 1] | 0x20];
    matches!(ext, [b'e', b'x', b'e'] | [b'd', b'l', b'l'] | [b'i', b'c', b'o'])
}

/// BGRA → RGBA byte swap. Returns true if any pixel has non-zero alpha.
#[inline]
fn bgra_to_rgba(pixels: &mut [u8]) -> bool {
    let mut has_alpha = false;
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2); // B ↔ R
        has_alpha |= px[3] != 0;
    }
    has_alpha
}

/// Cached system icon dimensions (constant for process lifetime).
pub(crate) fn system_icon_size() -> (i32, i32) {
    static SIZE: OnceLock<(i32, i32)> = OnceLock::new();
    *SIZE.get_or_init(|| unsafe {
        (GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON))
    })
}

// ── Public extraction API ──

/// Extract icon as RGBA pixels from any file path.
pub fn extract_icon(path: &str) -> Result<IconData, IconError> {
    extract_icon_at(path, 0)
}

/// Extract icon at a specific index from a file.
pub fn extract_icon_at(path: &str, index: u32) -> Result<IconData, IconError> {
    let wide = to_wide(path);
    let is_pe = has_pe_extension(path);

    if is_pe {
        let optimal = resource::get_max_icon_size_wide(&wide).unwrap_or(256).min(256) as i32;
        if let Some(data) = extract_private(&wide, index as i32, optimal) {
            return Ok(data);
        }
        if optimal != 48 {
            if let Some(data) = extract_private(&wide, index as i32, 48) {
                return Ok(data);
            }
        }
    }

    extract_shell(&wide)
        .ok_or_else(|| IconError::Extract(format!("no icon found: {}", path)))
}

/// Extract icon at a specific size (best-effort).
pub fn extract_icon_with_size(path: &str, size: u32) -> Result<IconData, IconError> {
    let wide = to_wide(path);
    extract_private(&wide, 0, size as i32)
        .or_else(|| extract_shell(&wide))
        .ok_or_else(|| IconError::Extract(format!("no icon found: {}", path)))
}

/// Query the number of icons contained in a file (.exe, .dll, .ico).
/// Returns 0 for non-PE files or files with no icons.
pub fn icon_count(path: &str) -> u32 {
    let func = match load_private_extract() {
        Some(f) => f,
        None => return 0,
    };
    let wide = to_wide(path);
    unsafe {
        func(
            PCWSTR(wide.as_ptr()),
            0, 0, 0,
            std::ptr::null_mut(), std::ptr::null_mut(),
            0, 0,
        )
    }
}

/// Extract the associated icon for a file extension (file need not exist).
/// `ext` should include the dot, e.g. ".pdf", ".docx".
pub fn extract_icon_for_extension(ext: &str) -> Result<IconData, IconError> {
    let name = if ext.starts_with('.') {
        format!("x{ext}")
    } else {
        format!("x.{ext}")
    };
    let wide = to_wide(&name);
    unsafe {
        let mut info = SHFILEINFOW::default();
        if SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
        ) == 0
        {
            return Err(IconError::Extract(format!("no icon for extension: {ext}")));
        }
        let _guard = AutoIcon(info.hIcon);
        let (w, h) = system_icon_size();
        hicon_to_rgba(info.hIcon, w, h)
            .ok_or_else(|| IconError::Extract(format!("icon conversion failed: {ext}")))
    }
}

fn extract_private(wide: &[u16], index: i32, size: i32) -> Option<IconData> {
    let func = load_private_extract()?;
    unsafe {
        let mut hicon = HICON::default();
        let mut icon_id = 0u32;
        if func(
            PCWSTR(wide.as_ptr()),
            index, size, size,
            &mut hicon, &mut icon_id, 1, 0,
        ) == 0
        {
            return None;
        }
        let _guard = AutoIcon(hicon);
        hicon_to_rgba(hicon, size, size)
    }
}

fn extract_shell(wide: &[u16]) -> Option<IconData> {
    unsafe {
        let mut info = SHFILEINFOW::default();
        if SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        ) == 0
        {
            return None;
        }
        let _guard = AutoIcon(info.hIcon);
        let (w, h) = system_icon_size();
        hicon_to_rgba(info.hIcon, w, h)
    }
}

// ── HICON → RGBA conversion (CreateDIBSection zero-copy) ──

fn make_bmi(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) fn hicon_to_rgba(hicon: HICON, width: i32, height: i32) -> Option<IconData> {
    unsafe {
        let screen_dc = ScreenDC(GetDC(None));
        let mem_dc = AutoDC(CreateCompatibleDC(screen_dc.0));

        let bmi = make_bmi(width, height);
        let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbitmap = CreateDIBSection(
            mem_dc.0, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0,
        ).ok()?;
        let hbitmap_guard = AutoGdiObj(hbitmap.into());
        if bits_ptr.is_null() { return None; }

        let old_obj = SelectObject(mem_dc.0, hbitmap_guard.0);
        let rect = RECT { left: 0, top: 0, right: width, bottom: height };
        FillRect(mem_dc.0, &rect, HBRUSH(GetStockObject(BLACK_BRUSH).0));

        if DrawIconEx(mem_dc.0, 0, 0, hicon, width, height, 0, None, DI_NORMAL).is_err() {
            SelectObject(mem_dc.0, old_obj);
            return None;
        }
        let _ = GdiFlush();

        // Zero-copy read from DIBSection, then own the data
        let byte_count = (width * height * 4) as usize;
        let src = std::slice::from_raw_parts(bits_ptr as *const u8, byte_count);
        let mut pixels = src.to_vec();

        let has_alpha = bgra_to_rgba(&mut pixels);
        SelectObject(mem_dc.0, old_obj);

        if !has_alpha {
            apply_mask_alpha(&mem_dc, hicon, width, height, &mut pixels);
        }

        Some(IconData {
            rgba: pixels,
            width: width as u32,
            height: height as u32,
        })
    }
}

unsafe fn apply_mask_alpha(
    mem_dc: &AutoDC,
    hicon: HICON,
    width: i32,
    height: i32,
    pixels: &mut [u8],
) {
    let bmi = make_bmi(width, height);
    let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let Ok(mask_bmp) = CreateDIBSection(
        mem_dc.0, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0,
    ) else { return };
    let mask_guard = AutoGdiObj(mask_bmp.into());
    if bits_ptr.is_null() { return; }

    let old_mask = SelectObject(mem_dc.0, mask_guard.0);
    let _ = DrawIconEx(mem_dc.0, 0, 0, hicon, width, height, 0, None, DI_MASK);
    let _ = GdiFlush();

    let mask = std::slice::from_raw_parts(bits_ptr as *const u8, pixels.len());
    pixels.chunks_exact_mut(4).zip(mask.chunks_exact(4)).for_each(|(px, m)| {
        px[3] = if m[0] == 0 { 255 } else { 0 };
    });

    SelectObject(mem_dc.0, old_mask);
}
