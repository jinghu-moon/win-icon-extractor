//! Icon extraction: HICON acquisition + RGBA pixel conversion

use crate::error::IconError;
use crate::resource;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::{
    AssocQueryStringW, SHGetFileInfoW, ASSOCF_INIT_DEFAULTTOSTAR, ASSOCSTR_DEFAULTICON,
    SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES,
};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Raw RGBA pixel data extracted from an icon.
#[derive(Clone)]
pub struct IconData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// ── Helpers ──

/// Convert an OsStr to a null-terminated UTF-16 buffer.
#[inline]
pub(crate) fn to_wide(s: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}

#[inline]
pub(crate) fn path_to_wide(path: &Path) -> Vec<u16> {
    to_wide(path.as_os_str())
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
auto_handle!(AutoDC, HDC, |h| {
    let _ = DeleteDC(h);
});
auto_handle!(ScreenDC, HDC, |h| {
    let _ = ReleaseDC(None, h);
});
auto_handle!(AutoGdiObj, HGDIOBJ, |h| {
    let _ = DeleteObject(h);
});

// ── PrivateExtractIconsW (undocumented but widely used) ──

type PrivateExtractIconsWFn =
    unsafe extern "system" fn(PCWSTR, i32, i32, i32, *mut HICON, *mut u32, u32, u32) -> u32;
type RawGetProcAddressFn = unsafe extern "system" fn() -> isize;

static PRIVATE_EXTRACT_FN: OnceLock<Option<PrivateExtractIconsWFn>> = OnceLock::new();

fn load_private_extract() -> Option<PrivateExtractIconsWFn> {
    *PRIVATE_EXTRACT_FN.get_or_init(|| unsafe {
        let wide = to_wide(OsStr::new("user32.dll"));
        let lib = LoadLibraryW(PCWSTR(wide.as_ptr())).ok()?;
        let proc = GetProcAddress(lib, PCSTR(b"PrivateExtractIconsW\0".as_ptr()))?;
        Some(std::mem::transmute::<
            RawGetProcAddressFn,
            PrivateExtractIconsWFn,
        >(proc))
    })
}

// ── PE / ICO detection ──

/// Fast extension check without allocating.
#[inline]
fn ext_eq(path: &Path, ascii_ext: &[u8]) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    if ext.len() != ascii_ext.len() {
        return false;
    }
    ext.as_bytes()
        .iter()
        .zip(ascii_ext)
        .all(|(a, b)| (*a | 0x20) == (*b | 0x20))
}

#[inline]
fn is_ico_file(path: &Path) -> bool {
    ext_eq(path, b"ico")
}

/// PE-like extensions worth scanning for RT_GROUP_ICON.
#[inline]
fn has_pe_extension(path: &Path) -> bool {
    ext_eq(path, b"exe")
        || ext_eq(path, b"dll")
        || ext_eq(path, b"ocx")
        || ext_eq(path, b"cpl")
        || ext_eq(path, b"scr")
        || ext_eq(path, b"mun")
        || ext_eq(path, b"mui")
        || ext_eq(path, b"drv")
        || ext_eq(path, b"efi")
}

/// Read the DOS magic (`MZ`) without loading the whole file.
fn has_mz_header(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 2];
    f.read_exact(&mut magic).is_ok() && magic == *b"MZ"
}

/// Whether this path is worth a PE resource scan for max icon size.
fn should_scan_pe_resources(path: &Path) -> bool {
    if is_ico_file(path) {
        return false;
    }
    if has_pe_extension(path) {
        return true;
    }
    has_mz_header(path)
}

// ── BGRA→RGBA ──

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
    *SIZE.get_or_init(|| unsafe { (GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON)) })
}

// ── Process-level max-size cache ──

fn max_size_cache() -> &'static Mutex<HashMap<std::path::PathBuf, u32>> {
    static CACHE: OnceLock<Mutex<HashMap<std::path::PathBuf, u32>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_max_icon_size(path: &Path) -> Option<u32> {
    {
        let map = max_size_cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(v) = map.get(path) {
            return Some(*v);
        }
    }
    let wide = path_to_wide(path);
    let size = resource::get_max_icon_size_wide(&wide)?;
    let mut map = max_size_cache().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(path.to_path_buf(), size);
    Some(size)
}

// ── Public extraction API ──

/// Extract the default icon as RGBA pixels from any file path.
pub fn extract_icon(path: impl AsRef<Path>) -> Result<IconData, IconError> {
    extract_icon_at(path, 0)
}

/// Extract icon at a specific index from a file.
///
/// `index` is the icon index (or resource id) understood by `PrivateExtractIconsW`.
/// For `index > 0` this function does **not** fall back to the shell default icon —
/// a fallback would silently return the wrong icon.
pub fn extract_icon_at(path: impl AsRef<Path>, index: u32) -> Result<IconData, IconError> {
    let path = path.as_ref();
    let wide = path_to_wide(path);

    if should_scan_pe_resources(path) {
        let optimal = cached_max_icon_size(path).unwrap_or(256).min(256) as i32;
        if let Some(data) = extract_private(&wide, index as i32, optimal) {
            return Ok(data);
        }
        if optimal != 48 {
            if let Some(data) = extract_private(&wide, index as i32, 48) {
                return Ok(data);
            }
        }
    } else {
        // .ico / non-PE: PrivateExtractIconsW handles these directly
        if let Some(data) = extract_private(&wide, index as i32, 256) {
            return Ok(data);
        }
        if let Some(data) = extract_private(&wide, index as i32, 48) {
            return Ok(data);
        }
    }

    // Shell fallback is only honest for the default icon (index 0).
    if index == 0 {
        if let Some(data) = extract_shell(&wide) {
            return Ok(data);
        }
    }

    Err(IconError::Extract(format!(
        "no icon found: {} (index {index})",
        path.display()
    )))
}

/// Extract icon at a specific size (best-effort).
///
/// If only a smaller native icon exists, the returned [`IconData`] still reports
/// the actual `width`/`height` obtained (possibly scaled by GDI).
pub fn extract_icon_with_size(path: impl AsRef<Path>, size: u32) -> Result<IconData, IconError> {
    let path = path.as_ref();
    let wide = path_to_wide(path);
    let size = size.clamp(1, 256) as i32;

    if let Some(data) = extract_private(&wide, 0, size) {
        return Ok(data);
    }
    // Last resort: shell icon at system size (actual dims are in the result).
    extract_shell(&wide).ok_or_else(|| {
        IconError::Extract(format!(
            "no icon found: {} (requested {size}px)",
            path.display()
        ))
    })
}

/// Query the number of icons in a file (.exe, .dll, .ico).
///
/// Returns `Ok(0)` when the file simply has no icons. Errors when the
/// underlying extraction entry point is unavailable.
pub fn icon_count(path: impl AsRef<Path>) -> Result<u32, IconError> {
    let func = load_private_extract()
        .ok_or_else(|| IconError::Extract("PrivateExtractIconsW unavailable".into()))?;
    let wide = path_to_wide(path.as_ref());
    let n = unsafe {
        func(
            PCWSTR(wide.as_ptr()),
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            0,
        )
    };
    Ok(n)
}

/// Extract the associated icon for a file extension (file need not exist).
/// `ext` should include the dot, e.g. ".pdf", ".docx".
pub fn extract_icon_for_extension(ext: &str) -> Result<IconData, IconError> {
    extract_icon_for_extension_sized(ext, 0)
}

/// Extract the associated icon for a file extension at a specific size.
///
/// Size 0 means the system large-icon size. Larger sizes prefer the association's
/// default icon (PE resource) so 256px icons stay sharp.
pub fn extract_icon_for_extension_sized(ext: &str, size: u32) -> Result<IconData, IconError> {
    // Prefer the real default icon (PE + index) when a custom size is requested.
    if size > 0 {
        if let Some(data) = extract_ext_icon_via_assoc(ext, size) {
            return Ok(data);
        }
    }

    let name = if ext.starts_with('.') {
        format!("x{ext}")
    } else {
        format!("x.{ext}")
    };
    let wide = to_wide(OsStr::new(&name));
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
        let (w, h) = if size > 0 {
            let s = size.clamp(1, 256) as i32;
            (s, s)
        } else {
            system_icon_size()
        };
        hicon_to_rgba(info.hIcon, w, h)
            .ok_or_else(|| IconError::Extract(format!("icon conversion failed: {ext}")))
    }
}

/// Resolve `EXT → DefaultIcon path,index` and extract at the requested size.
fn extract_ext_icon_via_assoc(ext: &str, size: u32) -> Option<IconData> {
    let key = if ext.starts_with('.') {
        ext.to_string()
    } else {
        format!(".{ext}")
    };
    let key_wide = to_wide(OsStr::new(&key));

    let mut buf = vec![0u16; 512];
    let mut cch = buf.len() as u32;
    unsafe {
        if AssocQueryStringW(
            ASSOCF_INIT_DEFAULTTOSTAR,
            ASSOCSTR_DEFAULTICON,
            PCWSTR(key_wide.as_ptr()),
            PCWSTR::null(),
            Some(windows::core::PWSTR(buf.as_mut_ptr())),
            &mut cch,
        )
        .is_err()
        {
            return None;
        }
    }

    let s = String::from_utf16_lossy(&buf[..cch as usize]);
    let s = s.trim_end_matches('\0');
    let (file, index) = match s.rfind(',') {
        Some(i) => {
            let idx: i32 = s[i + 1..].trim().parse().ok()?;
            (&s[..i], idx)
        }
        None => (s, 0),
    };
    let file = file.trim_matches('"');
    let wide = to_wide(OsStr::new(file));
    let sz = size.clamp(1, 256) as i32;
    extract_private(&wide, index, sz)
}

fn extract_private(wide: &[u16], index: i32, size: i32) -> Option<IconData> {
    let func = load_private_extract()?;
    unsafe {
        let mut hicon = HICON::default();
        let mut icon_id = 0u32;
        if func(
            PCWSTR(wide.as_ptr()),
            index,
            size,
            size,
            &mut hicon,
            &mut icon_id,
            1,
            0,
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

// ── HICON → RGBA ──

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

/// Convert HICON to RGBA.
///
/// Prefers `GetIconInfo` + `GetDIBits` so 32bpp icons keep a real alpha channel.
/// Falls back to `DrawIconEx` when the bitmap cannot be read or a different
/// size must be synthesized by GDI.
pub(crate) fn hicon_to_rgba(hicon: HICON, width: i32, height: i32) -> Option<IconData> {
    unsafe {
        if let Some(data) = hicon_to_rgba_bitmaps(hicon, width, height) {
            return Some(data);
        }
        hicon_to_rgba_draw(hicon, width, height)
    }
}

/// True-alpha path via ICONINFO bitmaps. Returns None when size differs from
/// the request (caller then scales via DrawIconEx).
unsafe fn hicon_to_rgba_bitmaps(hicon: HICON, width: i32, height: i32) -> Option<IconData> {
    let mut info = ICONINFO::default();
    if GetIconInfo(hicon, &mut info).is_err() {
        return None;
    }
    let _mask_guard = AutoGdiObj(info.hbmMask.into());
    if info.hbmColor.is_invalid() {
        return None;
    }
    let _color_guard = AutoGdiObj(info.hbmColor.into());

    let mut bm = BITMAP::default();
    let got = GetObjectW(
        HGDIOBJ(info.hbmColor.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut BITMAP as *mut _),
    );
    if got == 0 {
        return None;
    }

    let bw = bm.bmWidth;
    let bh = bm.bmHeight.unsigned_abs() as i32;
    if bw <= 0 || bh <= 0 {
        return None;
    }
    // Only trust this path when the bitmap already matches the request.
    if width > 0 && height > 0 && (bw != width || bh != height) {
        return None;
    }
    if bm.bmBitsPixel != 32 {
        return None;
    }

    let screen_dc = ScreenDC(GetDC(None));
    let mut bmi = make_bmi(bw, bh);
    let byte_count = (bw * bh * 4) as usize;
    let mut pixels = vec![0u8; byte_count];
    let lines = GetDIBits(
        screen_dc.0,
        info.hbmColor,
        0,
        bh as u32,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    if lines == 0 {
        return None;
    }

    let has_alpha = bgra_to_rgba(&mut pixels);
    if !has_alpha {
        apply_mask_from_hbitmap(screen_dc.0, info.hbmMask, bw, bh, &mut pixels);
    }

    Some(IconData {
        rgba: pixels,
        width: bw as u32,
        height: bh as u32,
    })
}

/// GDI fallback: DrawIconEx onto a DIB (used for scaling / odd formats).
unsafe fn hicon_to_rgba_draw(hicon: HICON, width: i32, height: i32) -> Option<IconData> {
    let screen_dc = ScreenDC(GetDC(None));
    let mem_dc = AutoDC(CreateCompatibleDC(Some(screen_dc.0)));

    let bmi = make_bmi(width, height);
    let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbitmap =
        CreateDIBSection(Some(mem_dc.0), &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0).ok()?;
    let hbitmap_guard = AutoGdiObj(hbitmap.into());
    if bits_ptr.is_null() {
        return None;
    }

    let old_obj = SelectObject(mem_dc.0, hbitmap_guard.0);
    let rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    FillRect(mem_dc.0, &rect, HBRUSH(GetStockObject(BLACK_BRUSH).0));

    if DrawIconEx(mem_dc.0, 0, 0, hicon, width, height, 0, None, DI_NORMAL).is_err() {
        SelectObject(mem_dc.0, old_obj);
        return None;
    }
    let _ = GdiFlush();

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

/// Derive alpha from an HBITMAP AND-mask (0 = opaque).
unsafe fn apply_mask_from_hbitmap(
    hdc: HDC,
    hbm_mask: HBITMAP,
    width: i32,
    height: i32,
    pixels: &mut [u8],
) {
    if hbm_mask.is_invalid() {
        for px in pixels.chunks_exact_mut(4) {
            px[3] = 255;
        }
        return;
    }

    let mut bmi = make_bmi(width, height);
    let byte_count = (width * height * 4) as usize;
    let mut mask = vec![0u8; byte_count];
    let lines = GetDIBits(
        hdc,
        hbm_mask,
        0,
        height as u32,
        Some(mask.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    if lines == 0 {
        for px in pixels.chunks_exact_mut(4) {
            px[3] = 255;
        }
        return;
    }

    pixels
        .chunks_exact_mut(4)
        .zip(mask.chunks_exact(4))
        .for_each(|(px, m)| {
            px[3] = if m[0] == 0 { 255 } else { 0 };
        });
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
    let Ok(mask_bmp) =
        CreateDIBSection(Some(mem_dc.0), &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)
    else {
        return;
    };
    let mask_guard = AutoGdiObj(mask_bmp.into());
    if bits_ptr.is_null() {
        return;
    }

    let old_mask = SelectObject(mem_dc.0, mask_guard.0);
    let _ = DrawIconEx(mem_dc.0, 0, 0, hicon, width, height, 0, None, DI_MASK);
    let _ = GdiFlush();

    let mask = std::slice::from_raw_parts(bits_ptr as *const u8, pixels.len());
    pixels
        .chunks_exact_mut(4)
        .zip(mask.chunks_exact(4))
        .for_each(|(px, m)| {
            px[3] = if m[0] == 0 { 255 } else { 0 };
        });

    SelectObject(mem_dc.0, old_mask);
}
