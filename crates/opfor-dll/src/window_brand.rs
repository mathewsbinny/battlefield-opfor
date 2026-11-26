//! Runtime window title & OPFOR icon branding hook for `engine/OPFOREngine.exe`.
//!
//! Hooks `USER32.dll` `CreateWindowExA` / `CreateWindowExW` via PE IAT so the
//! Frostbite game window displays `config/opfor.ico` and a dynamic era title
//! (`"Project OPFOR — Battlefield 1943"`, etc.) without modifying the engine EXE on disk.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;

use crate::iat_vfs::patch_iat;
use crate::win32::{self, DWORD, HANDLE, HMODULE, LPCSTR, LPCWSTR, LPVOID};

type HWND = *mut c_void;
type HMENU = *mut c_void;
type HINSTANCE = *mut c_void;
type WPARAM = usize;
type LPARAM = isize;

const WS_CHILD: DWORD = 0x4000_0000;
const WM_SETICON: u32 = 0x0080;
const ICON_SMALL: WPARAM = 0;
const ICON_BIG: WPARAM = 1;
const GCL_HICON: i32 = -14;
const GCL_HICONSM: i32 = -34;
const IMAGE_ICON: u32 = 1;
const LR_LOADFROMFILE: u32 = 0x0010;
const LR_DEFAULTSIZE: u32 = 0x0040;

type CreateWindowExAFn = unsafe extern "system" fn(
    DWORD, LPCSTR, LPCSTR, DWORD, i32, i32, i32, i32, HWND, HMENU, HINSTANCE, LPVOID,
) -> HWND;
type CreateWindowExWFn = unsafe extern "system" fn(
    DWORD, LPCWSTR, LPCWSTR, DWORD, i32, i32, i32, i32, HWND, HMENU, HINSTANCE, LPVOID,
) -> HWND;

#[link(name = "user32")]
unsafe extern "system" {
    fn LoadImageW(
        hInst: HINSTANCE,
        name: LPCWSTR,
        typ: u32,
        cx: i32,
        cy: i32,
        fuLoad: u32,
    ) -> HANDLE;
    fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> isize;
    fn SetClassLongW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> usize;
    fn SetWindowTextW(hWnd: HWND, lpString: LPCWSTR) -> i32;
}

static ORIG_CREATE_WINDOW_A: OnceLock<CreateWindowExAFn> = OnceLock::new();
static ORIG_CREATE_WINDOW_W: OnceLock<CreateWindowExWFn> = OnceLock::new();
static ICON_PATH: OnceLock<String> = OnceLock::new();
static ICON_BIG_H: OnceLock<usize> = OnceLock::new();
static ICON_SMALL_H: OnceLock<usize> = OnceLock::new();

pub fn install(opfor_root: &Path) -> usize {
    let ico = opfor_root.join("config").join("opfor.ico");
    if ico.is_file() {
        let _ = ICON_PATH.set(ico.to_string_lossy().into_owned());
    }
    let exe: HMODULE = unsafe { win32::GetModuleHandleW(std::ptr::null()) };
    if exe.is_null() {
        return 0;
    }
    let mut n = 0usize;
    unsafe {
        if let Some(o) = patch_iat(exe, "user32.dll", "CreateWindowExA", hook_cw_a as *const c_void) {
            let _ = ORIG_CREATE_WINDOW_A.set(std::mem::transmute::<*const c_void, CreateWindowExAFn>(o));
            n += 1;
        }
        if let Some(o) = patch_iat(exe, "user32.dll", "CreateWindowExW", hook_cw_w as *const c_void) {
            let _ = ORIG_CREATE_WINDOW_W.set(std::mem::transmute::<*const c_void, CreateWindowExWFn>(o));
            n += 1;
        }
    }
    n
}

fn current_window_title() -> String {
    if let Ok(guard) = crate::gamemode::STATE.lock() {
        if let Some(st) = guard.as_ref() {
            let name = st.table.spec(st.active).display_name;
            return format!("Project OPFOR — {name}");
        }
    }
    "Project OPFOR".to_string()
}

unsafe fn brand_window(hwnd: HWND, style: DWORD) {
    if hwnd.is_null() || (style & WS_CHILD) != 0 {
        return;
    }
    let title = win32::to_wide_nul(&current_window_title());
    unsafe { SetWindowTextW(hwnd, title.as_ptr()) };

    if let Some(path) = ICON_PATH.get() {
        let wpath = win32::to_wide_nul(path);
        let big = *ICON_BIG_H.get_or_init(|| unsafe {
            LoadImageW(
                std::ptr::null_mut(),
                wpath.as_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            ) as usize
        });
        let small = *ICON_SMALL_H.get_or_init(|| unsafe {
            LoadImageW(
                std::ptr::null_mut(),
                wpath.as_ptr(),
                IMAGE_ICON,
                16,
                16,
                LR_LOADFROMFILE,
            ) as usize
        });
        if big != 0 {
            unsafe {
                SendMessageW(hwnd, WM_SETICON, ICON_BIG, big as LPARAM);
                SetClassLongW(hwnd, GCL_HICON, big as isize);
            }
        }
        if small != 0 {
            unsafe {
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL, small as LPARAM);
                SetClassLongW(hwnd, GCL_HICONSM, small as isize);
            }
        }
    }
}

unsafe extern "system" fn hook_cw_a(
    ex: DWORD,
    cls: LPCSTR,
    name: LPCSTR,
    style: DWORD,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    parent: HWND,
    menu: HMENU,
    inst: HINSTANCE,
    param: LPVOID,
) -> HWND {
    let Some(&orig) = ORIG_CREATE_WINDOW_A.get() else {
        return std::ptr::null_mut();
    };
    let hwnd = unsafe { orig(ex, cls, name, style, x, y, w, h, parent, menu, inst, param) };
    if parent.is_null() && (style & WS_CHILD) == 0 {
        unsafe { brand_window(hwnd, style) };
    }
    hwnd
}

unsafe extern "system" fn hook_cw_w(
    ex: DWORD,
    cls: LPCWSTR,
    name: LPCWSTR,
    style: DWORD,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    parent: HWND,
    menu: HMENU,
    inst: HINSTANCE,
    param: LPVOID,
) -> HWND {
    let Some(&orig) = ORIG_CREATE_WINDOW_W.get() else {
        return std::ptr::null_mut();
    };
    let hwnd = unsafe { orig(ex, cls, name, style, x, y, w, h, parent, menu, inst, param) };
    if parent.is_null() && (style & WS_CHILD) == 0 {
        unsafe { brand_window(hwnd, style) };
    }
    hwnd
}
