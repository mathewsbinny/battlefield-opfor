//! Minimal Win32 FFI bindings for the 32-bit x86 `opfor-host` engine loader.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;

pub type BOOL = i32;
pub type DWORD = u32;
pub type HANDLE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HWND = *mut c_void;
pub type HMENU = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type LPVOID = *mut c_void;
pub type LPCSTR = *const u8;
pub type LPCWSTR = *const u16;
pub type WPARAM = usize;
pub type LPARAM = isize;

pub const MEM_COMMIT: DWORD = 0x0000_1000;
pub const MEM_RESERVE: DWORD = 0x0000_2000;

pub const PAGE_NOACCESS: DWORD = 0x01;
pub const PAGE_READONLY: DWORD = 0x02;
pub const PAGE_READWRITE: DWORD = 0x04;
pub const PAGE_EXECUTE: DWORD = 0x10;
pub const PAGE_EXECUTE_READ: DWORD = 0x20;
pub const PAGE_EXECUTE_READWRITE: DWORD = 0x40;

pub const INVALID_FILE_ATTRIBUTES: DWORD = 0xFFFF_FFFF;

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn VirtualAlloc(
        lpAddress: LPVOID,
        dwSize: usize,
        flAllocationType: DWORD,
        flProtect: DWORD,
    ) -> LPVOID;
    pub fn VirtualProtect(
        lpAddress: LPVOID,
        dwSize: usize,
        flNewProtect: DWORD,
        lpflOldProtect: *mut DWORD,
    ) -> BOOL;
    pub fn FlushInstructionCache(
        hProcess: HANDLE,
        lpBaseAddress: *const c_void,
        dwSize: usize,
    ) -> BOOL;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn LoadLibraryA(lpLibFileName: LPCSTR) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> *const c_void;
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn GetModuleFileNameW(hModule: HMODULE, lpFilename: *mut u16, nSize: DWORD) -> DWORD;
    pub fn CreateFileA(
        lpFileName: LPCSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: LPVOID,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn CreateFileW(
        lpFileName: LPCWSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: LPVOID,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn GetFileAttributesA(lpFileName: LPCSTR) -> DWORD;
    pub fn GetFileAttributesW(lpFileName: LPCWSTR) -> DWORD;
    pub fn GetFileAttributesExW(
        lpFileName: LPCWSTR,
        fInfoLevelId: DWORD,
        lpFileInformation: LPVOID,
    ) -> BOOL;
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn CreateWindowExA(
        dwExStyle: DWORD,
        lpClassName: LPCSTR,
        lpWindowName: LPCSTR,
        dwStyle: DWORD,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: LPVOID,
    ) -> HWND;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: LPCWSTR,
        lpWindowName: LPCWSTR,
        dwStyle: DWORD,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: LPVOID,
    ) -> HWND;
    pub fn LoadImageW(
        hInst: HINSTANCE,
        name: LPCWSTR,
        typ: u32,
        cx: i32,
        cy: i32,
        fuLoad: u32,
    ) -> HANDLE;
    pub fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> isize;
    pub fn SetClassLongW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> usize;
    pub fn SetWindowTextW(hWnd: HWND, lpString: LPCWSTR) -> i32;
}

#[link(name = "winmm")]
unsafe extern "system" {
    pub fn timeBeginPeriod(uPeriod: u32) -> u32;
    pub fn timeEndPeriod(uPeriod: u32) -> u32;
}

pub fn to_wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub unsafe fn from_wide_ptr(ptr: LPCWSTR) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 && len < 4096 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        Some(String::from_utf16_lossy(slice))
    }
}

pub unsafe fn from_ansi_ptr(ptr: LPCSTR) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 && len < 4096 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        Some(String::from_utf8_lossy(slice).into_owned())
    }
}
