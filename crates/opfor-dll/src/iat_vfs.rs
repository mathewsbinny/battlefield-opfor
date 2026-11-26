//! Lightweight Win32 IAT VFS hook for Option B (`modules/<era>/` + `cache/`).
//!
//! Patches `KERNEL32.dll` imports (`CreateFileA/W`, `GetFileAttributesA/W`,
//! `GetFileAttributesExW`) in the host executable (`BFBC2Game.exe`) without
//! touching any Frostbite engine internals.

#![allow(non_snake_case)]

use std::ffi::{CString, c_void};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use opfor_vfs::{VfsResolution, VfsRouter};

use crate::win32::{
    self, BOOL, DWORD, HANDLE, HMODULE, LPCSTR, LPCWSTR, LPVOID, PAGE_READWRITE,
};

static ROUTER: OnceLock<VfsRouter> = OnceLock::new();
static LOG_REDIRECTS: AtomicBool = AtomicBool::new(false);

type CreateFileAFn = unsafe extern "system" fn(
    LPCSTR, DWORD, DWORD, LPVOID, DWORD, DWORD, HANDLE,
) -> HANDLE;
type CreateFileWFn = unsafe extern "system" fn(
    LPCWSTR, DWORD, DWORD, LPVOID, DWORD, DWORD, HANDLE,
) -> HANDLE;
type GetFileAttributesAFn = unsafe extern "system" fn(LPCSTR) -> DWORD;
type GetFileAttributesWFn = unsafe extern "system" fn(LPCWSTR) -> DWORD;
type GetFileAttributesExWFn = unsafe extern "system" fn(LPCWSTR, DWORD, LPVOID) -> BOOL;

static ORIG_CREATE_FILE_A: OnceLock<CreateFileAFn> = OnceLock::new();
static ORIG_CREATE_FILE_W: OnceLock<CreateFileWFn> = OnceLock::new();
static ORIG_GET_ATTR_A: OnceLock<GetFileAttributesAFn> = OnceLock::new();
static ORIG_GET_ATTR_W: OnceLock<GetFileAttributesWFn> = OnceLock::new();
static ORIG_GET_ATTR_EX_W: OnceLock<GetFileAttributesExWFn> = OnceLock::new();

/// Install the Win32 IAT VFS hooks into the main executable image.
/// Returns the number of patched IAT slots.
pub fn install(router: VfsRouter, log_redirects: bool) -> usize {
    let _ = ROUTER.set(router);
    LOG_REDIRECTS.store(log_redirects, Ordering::Relaxed);

    let exe = unsafe { win32::GetModuleHandleW(std::ptr::null()) };
    if exe.is_null() {
        return 0;
    }

    let mut count = 0usize;
    unsafe {
        if let Some(orig) = patch_iat(exe, "kernel32.dll", "CreateFileA", hook_create_file_a as *const c_void) {
            let _ = ORIG_CREATE_FILE_A.set(std::mem::transmute::<*const c_void, CreateFileAFn>(orig));
            count += 1;
        }
        if let Some(orig) = patch_iat(exe, "kernel32.dll", "CreateFileW", hook_create_file_w as *const c_void) {
            let _ = ORIG_CREATE_FILE_W.set(std::mem::transmute::<*const c_void, CreateFileWFn>(orig));
            count += 1;
        }
        if let Some(orig) = patch_iat(exe, "kernel32.dll", "GetFileAttributesA", hook_get_attr_a as *const c_void) {
            let _ = ORIG_GET_ATTR_A.set(std::mem::transmute::<*const c_void, GetFileAttributesAFn>(orig));
            count += 1;
        }
        if let Some(orig) = patch_iat(exe, "kernel32.dll", "GetFileAttributesW", hook_get_attr_w as *const c_void) {
            let _ = ORIG_GET_ATTR_W.set(std::mem::transmute::<*const c_void, GetFileAttributesWFn>(orig));
            count += 1;
        }
        if let Some(orig) = patch_iat(exe, "kernel32.dll", "GetFileAttributesExW", hook_get_attr_ex_w as *const c_void) {
            let _ = ORIG_GET_ATTR_EX_W.set(std::mem::transmute::<*const c_void, GetFileAttributesExWFn>(orig));
            count += 1;
        }
    }
    count
}

unsafe extern "system" fn hook_create_file_a(
    lpFileName: LPCSTR,
    dwDesiredAccess: DWORD,
    dwShareMode: DWORD,
    lpSecurityAttributes: LPVOID,
    dwCreationDisposition: DWORD,
    dwFlagsAndAttributes: DWORD,
    hTemplateFile: HANDLE,
) -> HANDLE {
    let orig = *ORIG_CREATE_FILE_A.get().expect("CreateFileA orig");
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_ansi_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                if LOG_REDIRECTS.load(Ordering::Relaxed) {
                    crate::log_line(&format!("VFS CreateFileA: {raw} -> {}", target.display()));
                }
                if let Ok(cs) = CString::new(target.to_string_lossy().as_bytes()) {
                    return unsafe {
                        orig(
                            cs.as_ptr().cast(),
                            dwDesiredAccess,
                            dwShareMode,
                            lpSecurityAttributes,
                            dwCreationDisposition,
                            dwFlagsAndAttributes,
                            hTemplateFile,
                        )
                    };
                }
            }
            VfsResolution::Hide => return (-1isize) as HANDLE,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe {
        orig(
            lpFileName,
            dwDesiredAccess,
            dwShareMode,
            lpSecurityAttributes,
            dwCreationDisposition,
            dwFlagsAndAttributes,
            hTemplateFile,
        )
    }
}

unsafe extern "system" fn hook_create_file_w(
    lpFileName: LPCWSTR,
    dwDesiredAccess: DWORD,
    dwShareMode: DWORD,
    lpSecurityAttributes: LPVOID,
    dwCreationDisposition: DWORD,
    dwFlagsAndAttributes: DWORD,
    hTemplateFile: HANDLE,
) -> HANDLE {
    let orig = *ORIG_CREATE_FILE_W.get().expect("CreateFileW orig");
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_wide_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                if LOG_REDIRECTS.load(Ordering::Relaxed) {
                    crate::log_line(&format!("VFS CreateFileW: {raw} -> {}", target.display()));
                }
                let wide = win32::to_wide_nul(&target.to_string_lossy());
                return unsafe {
                    orig(
                        wide.as_ptr(),
                        dwDesiredAccess,
                        dwShareMode,
                        lpSecurityAttributes,
                        dwCreationDisposition,
                        dwFlagsAndAttributes,
                        hTemplateFile,
                    )
                };
            }
            VfsResolution::Hide => return (-1isize) as HANDLE,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe {
        orig(
            lpFileName,
            dwDesiredAccess,
            dwShareMode,
            lpSecurityAttributes,
            dwCreationDisposition,
            dwFlagsAndAttributes,
            hTemplateFile,
        )
    }
}

unsafe extern "system" fn hook_get_attr_a(lpFileName: LPCSTR) -> DWORD {
    let orig = *ORIG_GET_ATTR_A.get().expect("GetFileAttributesA orig");
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_ansi_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                if let Ok(cs) = CString::new(target.to_string_lossy().as_bytes()) {
                    return unsafe { orig(cs.as_ptr().cast()) };
                }
            }
            VfsResolution::Hide => return win32::INVALID_FILE_ATTRIBUTES,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe { orig(lpFileName) }
}

unsafe extern "system" fn hook_get_attr_w(lpFileName: LPCWSTR) -> DWORD {
    let orig = *ORIG_GET_ATTR_W.get().expect("GetFileAttributesW orig");
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_wide_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                let wide = win32::to_wide_nul(&target.to_string_lossy());
                return unsafe { orig(wide.as_ptr()) };
            }
            VfsResolution::Hide => return win32::INVALID_FILE_ATTRIBUTES,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe { orig(lpFileName) }
}

unsafe extern "system" fn hook_get_attr_ex_w(
    lpFileName: LPCWSTR,
    fInfoLevelId: DWORD,
    lpFileInformation: LPVOID,
) -> BOOL {
    let orig = *ORIG_GET_ATTR_EX_W.get().expect("GetFileAttributesExW orig");
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_wide_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                let wide = win32::to_wide_nul(&target.to_string_lossy());
                return unsafe { orig(wide.as_ptr(), fInfoLevelId, lpFileInformation) };
            }
            VfsResolution::Hide => return 0,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe { orig(lpFileName, fInfoLevelId, lpFileInformation) }
}

/// Minimal 32-bit/64-bit PE IAT patcher.
///
/// SAFETY: `module` must be a valid mapped PE image base.
pub(crate) unsafe fn patch_iat(
    module: HMODULE,
    dll_name: &str,
    func_name: &str,
    replacement: *const c_void,
) -> Option<*const c_void> {
    let base = module as *const u8;
    // DOS header: e_magic == "MZ", e_lfanew @ 0x3C
    if unsafe { *base != b'M' || *base.add(1) != b'Z' } {
        return None;
    }
    let lfanew = unsafe { *(base.add(0x3C) as *const u32) } as usize;
    let nt = unsafe { base.add(lfanew) };
    if unsafe { *(nt as *const u32) != 0x0000_4550 } {
        return None; // "PE\0\0"
    }
    let opt = unsafe { nt.add(24) };
    let magic = unsafe { *(opt as *const u16) };
    // DataDirectory[1] (Import Table) RVA: +96 (PE32) or +112 (PE32+)
    let import_dir_off = match magic {
        0x10B => 96 + 8,
        0x20B => 112 + 8,
        _ => return None,
    };
    let import_rva = unsafe { *(opt.add(import_dir_off) as *const u32) } as usize;
    if import_rva == 0 {
        return None;
    }

    #[repr(C)]
    struct ImportDescriptor {
        orig_first_thunk: u32,
        time_date_stamp: u32,
        forwarder_chain: u32,
        name_rva: u32,
        first_thunk: u32,
    }

    let mut desc = unsafe { base.add(import_rva) as *const ImportDescriptor };
    unsafe {
        while (*desc).name_rva != 0 {
            let name_ptr = base.add((*desc).name_rva as usize);
            if let Some(mod_name) = win32::from_ansi_ptr(name_ptr) {
                if mod_name.eq_ignore_ascii_case(dll_name) {
                    let orig_rva = if (*desc).orig_first_thunk != 0 {
                        (*desc).orig_first_thunk
                    } else {
                        (*desc).first_thunk
                    } as usize;
                    let mut orig_thunk = base.add(orig_rva) as *const usize;
                    let mut iat_thunk = base.add((*desc).first_thunk as usize) as *mut *const c_void;

                    let ordinal_flag: usize = 1usize << (usize::BITS - 1);
                    while *orig_thunk != 0 {
                        let val = *orig_thunk;
                        if (val & ordinal_flag) == 0 {
                            let imp_name_ptr = base.add(val + 2); // skip Hint (u16)
                            if let Some(imp_name) = win32::from_ansi_ptr(imp_name_ptr) {
                                if imp_name == func_name {
                                    let old_fn = *iat_thunk;
                                    let mut old_prot: DWORD = 0;
                                    if win32::VirtualProtect(
                                        iat_thunk.cast(),
                                        std::mem::size_of::<*const c_void>(),
                                        PAGE_READWRITE,
                                        &mut old_prot,
                                    ) != 0
                                    {
                                        *iat_thunk = replacement;
                                        let mut dummy: DWORD = 0;
                                        win32::VirtualProtect(
                                            iat_thunk.cast(),
                                            std::mem::size_of::<*const c_void>(),
                                            old_prot,
                                            &mut dummy,
                                        );
                                        return Some(old_fn);
                                    }
                                }
                            }
                        }
                        orig_thunk = orig_thunk.add(1);
                        iat_thunk = iat_thunk.add(1);
                    }
                }
            }
            desc = desc.add(1);
        }
    }
    None
}
