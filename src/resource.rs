//! PE resource parsing — pure Rust replacement for icon_resolver.c

use crate::error::IconError;
use std::path::Path;
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::*;

const RT_GROUP_ICON: u16 = 14;

// RAII guard for HMODULE — prevents leak on panic/early return
struct AutoModule(HMODULE);
impl Drop for AutoModule {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

fn load_pe_as_data(wide_path: &[u16]) -> Option<AutoModule> {
    unsafe {
        let pw = PCWSTR(wide_path.as_ptr());
        let hmod = LoadLibraryExW(
            pw,
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
        .or_else(|_| LoadLibraryExW(pw, None, LOAD_LIBRARY_AS_DATAFILE))
        .ok()?;
        Some(AutoModule(hmod))
    }
}

/// Collect unique icon widths declared in all RT_GROUP_ICON resources.
/// Returns a sorted, de-duplicated list. `0` entries mean 256px.
pub fn list_icon_sizes_wide(wide_path: &[u16]) -> Result<Vec<u32>, IconError> {
    let module = load_pe_as_data(wide_path)
        .ok_or_else(|| IconError::Extract("failed to load PE resources".into()))?;

    let mut sizes: Vec<u32> = Vec::new();
    unsafe {
        let _ = EnumResourceNamesW(
            Some(module.0),
            PCWSTR(RT_GROUP_ICON as usize as *const u16),
            Some(enum_icon_group_sizes),
            &mut sizes as *mut Vec<u32> as isize,
        );
    }
    sizes.sort_unstable();
    sizes.dedup();
    Ok(sizes)
}

/// Convenience wrapper that converts path internally.
pub fn list_icon_sizes(path: impl AsRef<Path>) -> Result<Vec<u32>, IconError> {
    let wide = crate::extract::path_to_wide(path.as_ref());
    list_icon_sizes_wide(&wide)
}

/// Get the maximum icon size embedded in a PE file (.exe/.dll).
/// Accepts pre-converted wide path to avoid redundant UTF-16 allocation.
pub fn get_max_icon_size_wide(wide_path: &[u16]) -> Option<u32> {
    let module = load_pe_as_data(wide_path)?;
    let mut max_size: i32 = 0;
    unsafe {
        let _ = EnumResourceNamesW(
            Some(module.0),
            PCWSTR(RT_GROUP_ICON as usize as *const u16),
            Some(enum_icon_group),
            &mut max_size as *mut i32 as isize,
        );
    }
    (max_size > 0).then_some(max_size as u32)
}

/// Convenience wrapper that converts path internally.
pub fn get_max_icon_size(path: impl AsRef<Path>) -> Option<u32> {
    let wide = crate::extract::path_to_wide(path.as_ref());
    get_max_icon_size_wide(&wide)
}

/// Parse a single GRPICONDIR: return widths of every entry, or empty on malformed data.
unsafe fn parse_group_widths(ptr: *const u8, res_size: usize) -> Vec<u32> {
    if res_size < 6 {
        return Vec::new();
    }
    let id_count = *(ptr.add(4) as *const u16) as usize;
    let required = 6 + id_count * 14;
    if required > res_size {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(id_count);
    for i in 0..id_count {
        let raw_width = *ptr.add(6 + i * 14);
        let width = if raw_width == 0 {
            256u32
        } else {
            raw_width as u32
        };
        out.push(width);
    }
    out
}

unsafe extern "system" fn enum_icon_group(
    hmodule: HMODULE,
    _lptype: PCWSTR,
    lpname: PCWSTR,
    lparam: isize,
) -> BOOL {
    let max_size = &mut *(lparam as *mut i32);

    let hrsrc = FindResourceW(
        Some(hmodule),
        lpname,
        PCWSTR(RT_GROUP_ICON as usize as *const u16),
    );
    if hrsrc.is_invalid() {
        return BOOL(1);
    }

    let Ok(hglobal) = LoadResource(Some(hmodule), hrsrc) else {
        return BOOL(1);
    };

    let ptr = LockResource(hglobal) as *const u8;
    if ptr.is_null() {
        return BOOL(1);
    }
    let res_size = SizeofResource(Some(hmodule), hrsrc) as usize;

    for width in parse_group_widths(ptr, res_size) {
        let w = width.min(256) as i32;
        if w > *max_size {
            *max_size = w;
            if *max_size >= 256 {
                return BOOL(0); // already at cap
            }
        }
    }

    BOOL(1)
}

unsafe extern "system" fn enum_icon_group_sizes(
    hmodule: HMODULE,
    _lptype: PCWSTR,
    lpname: PCWSTR,
    lparam: isize,
) -> BOOL {
    let sizes = &mut *(lparam as *mut Vec<u32>);

    let hrsrc = FindResourceW(
        Some(hmodule),
        lpname,
        PCWSTR(RT_GROUP_ICON as usize as *const u16),
    );
    if hrsrc.is_invalid() {
        return BOOL(1);
    }

    let Ok(hglobal) = LoadResource(Some(hmodule), hrsrc) else {
        return BOOL(1);
    };

    let ptr = LockResource(hglobal) as *const u8;
    if ptr.is_null() {
        return BOOL(1);
    }
    let res_size = SizeofResource(Some(hmodule), hrsrc) as usize;

    for width in parse_group_widths(ptr, res_size) {
        sizes.push(width.min(256));
    }

    BOOL(1)
}
