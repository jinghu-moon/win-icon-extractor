//! PE resource parsing — pure Rust replacement for icon_resolver.c

use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::*;

const RT_GROUP_ICON: u16 = 14;

// RAII guard for HMODULE — prevents leak on panic/early return
struct AutoModule(HMODULE);
impl Drop for AutoModule {
    fn drop(&mut self) {
        unsafe { let _ = FreeLibrary(self.0); }
    }
}

fn load_pe_as_data(wide_path: &[u16]) -> Option<AutoModule> {
    unsafe {
        let pw = PCWSTR(wide_path.as_ptr());
        let hmod = LoadLibraryExW(pw, None, LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE)
            .or_else(|_| LoadLibraryExW(pw, None, LOAD_LIBRARY_AS_DATAFILE))
            .ok()?;
        Some(AutoModule(hmod))
    }
}

/// Get the maximum icon size embedded in a PE file (.exe/.dll).
/// Accepts pre-converted wide path to avoid redundant UTF-16 allocation.
pub fn get_max_icon_size_wide(wide_path: &[u16]) -> Option<u32> {
    let module = load_pe_as_data(wide_path)?;
    let mut max_size: i32 = 0;
    unsafe {
        let _ = EnumResourceNamesW(
            module.0,
            PCWSTR(RT_GROUP_ICON as usize as *const u16),
            Some(enum_icon_group),
            &mut max_size as *mut i32 as isize,
        );
    }
    // AutoModule drops here — FreeLibrary guaranteed
    (max_size > 0).then_some(max_size as u32)
}

/// Convenience wrapper that converts path internally.
pub fn get_max_icon_size(path: &str) -> Option<u32> {
    let wide = crate::extract::to_wide(path);
    get_max_icon_size_wide(&wide)
}

unsafe extern "system" fn enum_icon_group(
    hmodule: HMODULE,
    _lptype: PCWSTR,
    lpname: PCWSTR,
    lparam: isize,
) -> BOOL {
    let max_size = &mut *(lparam as *mut i32);

    let hrsrc = FindResourceW(hmodule, lpname, PCWSTR(RT_GROUP_ICON as usize as *const u16));
    if hrsrc.is_invalid() {
        return BOOL(1);
    }

    let Ok(hglobal) = LoadResource(hmodule, hrsrc) else {
        return BOOL(1);
    };

    let ptr = LockResource(hglobal) as *const u8;
    if ptr.is_null() {
        return BOOL(1);
    }
    let res_size = SizeofResource(hmodule, hrsrc) as usize;

    // GRPICONDIR: header 6 bytes (idReserved + idType + idCount), entries 14 bytes each
    if res_size < 6 { return BOOL(1); }
    let id_count = *(ptr.add(4) as *const u16) as usize;
    let required = 6 + id_count * 14;
    if required > res_size { return BOOL(1); }

    for i in 0..id_count {
        let raw_width = *ptr.add(6 + i * 14);
        let width = if raw_width == 0 { 256 } else { raw_width as i32 };
        if width > *max_size {
            *max_size = width;
            if *max_size >= 256 { return BOOL(0); } // 已达上限，停止枚举
        }
    }

    BOOL(1)
}
