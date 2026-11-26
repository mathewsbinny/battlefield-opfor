//! Minimal Win32 FFI bindings (zero external crates; works on both i686 and x86_64).

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;

pub type BOOL = i32;
pub type DWORD = u32;
pub type HANDLE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HRESULT = i32;
pub type LPCSTR = *const u8;
pub type LPCWSTR = *const u16;
pub type LPWSTR = *mut u16;
pub type LPVOID = *mut c_void;

pub const DLL_PROCESS_ATTACH: DWORD = 1;
pub const DLL_PROCESS_DETACH: DWORD = 0;
pub const PAGE_READWRITE: DWORD = 0x04;
pub const PAGE_EXECUTE_READWRITE: DWORD = 0x40;
pub const MEM_COMMIT: DWORD = 0x1000;
pub const MEM_RESERVE: DWORD = 0x2000;
pub const INVALID_FILE_ATTRIBUTES: DWORD = 0xFFFF_FFFF;

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn DisableThreadLibraryCalls(hLibModule: HMODULE) -> BOOL;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn FlushInstructionCache(hProcess: HANDLE, lpBaseAddress: *const c_void, dwSize: usize) -> BOOL;
    pub fn VirtualAlloc(
        lpAddress: LPVOID,
        dwSize: usize,
        flAllocationType: DWORD,
        flProtect: DWORD,
    ) -> LPVOID;
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn GetModuleFileNameW(hModule: HMODULE, lpFilename: LPWSTR, nSize: DWORD) -> DWORD;
    pub fn GetSystemDirectoryW(lpBuffer: LPWSTR, uSize: DWORD) -> DWORD;
    pub fn LoadLibraryW(lpLibFileName: LPCWSTR) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> *const c_void;
    pub fn VirtualProtect(
        lpAddress: LPVOID,
        dwSize: usize,
        flNewProtect: DWORD,
        lpflOldProtect: *mut DWORD,
    ) -> BOOL;
}

pub fn to_wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// SAFETY: `ptr` must be a valid NUL-terminated UTF-16 string.
pub unsafe fn from_wide_ptr(ptr: LPCWSTR) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 32768 {
                return None;
            }
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        Some(String::from_utf16_lossy(slice))
    }
}

/// SAFETY: `ptr` must be a valid NUL-terminated ASCII/ANSI string.
pub unsafe fn from_ansi_ptr(ptr: LPCSTR) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 32768 {
                return None;
            }
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        Some(String::from_utf8_lossy(slice).into_owned())
    }
}
