//! First-class Win32 import override table for the re-hosted Frostbite engine.
//!
//! Because `opfor-host` resolves `BFBC2Game.exe`'s Import Table directly at load time,
//! every intercepted OS call (VFS routing, offline network guard, window branding) is
//! wired cleanly before Frostbite executes a single instruction.

#![allow(non_snake_case)]

use std::ffi::{CString, c_void};
use std::path::Path;
use std::sync::OnceLock;

use opfor_vfs::{VfsResolution, VfsRouter};

use crate::win32::{
    self, BOOL, DWORD, HANDLE, HINSTANCE, HMENU, HWND, LPARAM, LPCSTR, LPCWSTR, LPVOID, WPARAM,
};

static ROUTER: OnceLock<VfsRouter> = OnceLock::new();
static ICON_PATH: OnceLock<String> = OnceLock::new();
static USER_DOCS_DIR: OnceLock<String> = OnceLock::new();
static USER_DOCS_BFBC1: OnceLock<String> = OnceLock::new();
static USER_DOCS_BF1943: OnceLock<String> = OnceLock::new();
static ICON_BIG_H: OnceLock<usize> = OnceLock::new();
static ICON_SMALL_H: OnceLock<usize> = OnceLock::new();

const CSIDL_PERSONAL: i32 = 0x0005;
const CSIDL_FLAG_MASK: i32 = 0x00FF;

const WS_CHILD: DWORD = 0x4000_0000;
const WM_SETICON: u32 = 0x0080;
const ICON_SMALL: WPARAM = 0;
const ICON_BIG: WPARAM = 1;
const GCL_HICON: i32 = -14;
const GCL_HICONSM: i32 = -34;
const IMAGE_ICON: u32 = 1;
const LR_LOADFROMFILE: u32 = 0x0010;
const LR_DEFAULTSIZE: u32 = 0x0040;

const AF_INET: u16 = 2;
const WSAEACCES: i32 = 10013;
const ERROR_ACCESS_DENIED: DWORD = 5;

#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: [u8; 4],
    sin_zero: [u8; 8],
}

type ConnectFn = unsafe extern "system" fn(usize, *const c_void, i32) -> i32;
type BindFn = unsafe extern "system" fn(usize, *const c_void, i32) -> i32;
type SendToFn = unsafe extern "system" fn(usize, *const u8, i32, i32, *const c_void, i32) -> i32;
type WinHttpConnectFn = unsafe extern "system" fn(HANDLE, LPCWSTR, u16, DWORD) -> HANDLE;
type ShGetFolderPathAFn = unsafe extern "system" fn(HWND, i32, HANDLE, DWORD, *mut u8) -> i32;
type ShGetFolderPathWFn = unsafe extern "system" fn(HWND, i32, HANDLE, DWORD, *mut u16) -> i32;
type BinkOpenFn = unsafe extern "system" fn(LPCSTR, u32) -> *mut c_void;
type XInputGetStateFn = unsafe extern "system" fn(u32, *mut crate::controller::XInputState) -> u32;

static ORIG_CONNECT: OnceLock<ConnectFn> = OnceLock::new();
static ORIG_BIND: OnceLock<BindFn> = OnceLock::new();
static ORIG_SENDTO: OnceLock<SendToFn> = OnceLock::new();
static ORIG_WINHTTP_CONNECT: OnceLock<WinHttpConnectFn> = OnceLock::new();
static ORIG_SH_GET_FOLDER_A: OnceLock<ShGetFolderPathAFn> = OnceLock::new();
static ORIG_SH_GET_FOLDER_W: OnceLock<ShGetFolderPathWFn> = OnceLock::new();
static ORIG_BINK_OPEN: OnceLock<BinkOpenFn> = OnceLock::new();
static ORIG_XINPUT_GET_STATE: OnceLock<XInputGetStateFn> = OnceLock::new();

#[link(name = "ws2_32")]
unsafe extern "system" {
    fn WSASetLastError(iError: i32);
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetLastError(dwErrCode: DWORD);
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetFolderPathW(hwnd: HWND, csidl: i32, token: HANDLE, flags: DWORD, out: *mut u16) -> i32;
}

fn real_documents_dir(opfor_root: &Path) -> std::path::PathBuf {
    let mut buf = [0u16; 260];
    let hr = unsafe { SHGetFolderPathW(std::ptr::null_mut(), CSIDL_PERSONAL, std::ptr::null_mut(), 0, buf.as_mut_ptr()) };
    if hr == 0 {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        if len > 0 {
            return std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len]));
        }
    }
    opfor_root.join("user").join("Documents")
}

pub fn init(opfor_root: &Path, router: VfsRouter) {
    let _ = ROUTER.set(router);
    let ico = opfor_root.join("config").join("opfor.ico");
    if ico.is_file() {
        let _ = ICON_PATH.set(ico.to_string_lossy().into_owned());
    }
    let opfor_docs = real_documents_dir(opfor_root).join("Project OPFOR");
    // BFBC2/Vietnam uses `Project OPFOR` directly (Frostbite appends `\BFBC2\`),
    // matching existing `Project OPFOR\BFBC2\GameSettings.bin`.
    // BFBC1 and BF1943 use their own subfolders (`Project OPFOR\BFBC1`, `Project OPFOR\BF1943`).
    let docs = opfor_docs.clone();
    let docs_bfbc1 = opfor_docs.join("BFBC1");
    let docs_bf1943 = opfor_docs.join("BF1943");
    let (mut w, mut h, mut fs_flag) = (1920u32, 1080u32, true);
    if let Ok(txt) = std::fs::read_to_string(opfor_root.join("config").join("display.ini")) {
        for raw in txt.lines() {
            if let Some((k, v)) = raw.trim().split_once('=') {
                match k.trim() {
                    "width" => w = v.trim().parse().unwrap_or(1920),
                    "height" => h = v.trim().parse().unwrap_or(1080),
                    "fullscreen" => fs_flag = matches!(v.trim(), "1" | "true" | "yes"),
                    _ => {}
                }
            }
        }
    }
    let default_settings = format!(
        "[WindowSettings]\r\nWidth={w}\r\nHeight={h}\r\nFullscreen={fs_flag}\r\nVSync=false\r\n[Graphics]\r\nDxVersion=auto\r\n"
    );
    // Ensure each isolated Documents root has BFBC2\settings.ini so Frostbite never falls back to 800x600
    // and BFBC1 / BF1943 saves never overwrite BFBC2's savegames.
    for dir in [&docs, &docs_bfbc1, &docs_bf1943] {
        let _ = std::fs::create_dir_all(dir);
        let bfbc2_dir = dir.join("BFBC2");
        let settings_ini = bfbc2_dir.join("settings.ini");
        if !settings_ini.is_file() {
            let _ = std::fs::create_dir_all(&bfbc2_dir);
            let _ = std::fs::write(&settings_ini, &default_settings);
        }
    }
    let _ = USER_DOCS_DIR.set(docs.to_string_lossy().into_owned());
    let _ = USER_DOCS_BFBC1.set(docs_bfbc1.to_string_lossy().into_owned());
    let _ = USER_DOCS_BF1943.set(docs_bf1943.to_string_lossy().into_owned());
}

/// Resolve an imported Win32 function for the mapped Frostbite image, substituting
/// OPFOR's native VFS, offline network guard, userdata isolation, Bink intro skip,
/// and window branding hooks directly.
pub fn resolve_override(
    dll_name: &str,
    func_name: &str,
    real_proc: *const c_void,
) -> *const c_void {
    if dll_name.eq_ignore_ascii_case("kernel32.dll") {
        match func_name {
            "CreateFileA" => return hook_create_file_a as *const c_void,
            "CreateFileW" => return hook_create_file_w as *const c_void,
            "GetFileAttributesA" => return hook_get_attr_a as *const c_void,
            "GetFileAttributesW" => return hook_get_attr_w as *const c_void,
            "GetFileAttributesExW" => return hook_get_attr_ex_w as *const c_void,
            "OutputDebugStringA" => return hook_output_debug_string_a as *const c_void,
            _ => {}
        }
    } else if dll_name.eq_ignore_ascii_case("shell32.dll") {
        unsafe {
            match func_name {
                "SHGetFolderPathA" => {
                    let _ = ORIG_SH_GET_FOLDER_A
                        .set(std::mem::transmute::<*const c_void, ShGetFolderPathAFn>(real_proc));
                    return hook_sh_get_folder_a as *const c_void;
                }
                "SHGetFolderPathW" => {
                    let _ = ORIG_SH_GET_FOLDER_W
                        .set(std::mem::transmute::<*const c_void, ShGetFolderPathWFn>(real_proc));
                    return hook_sh_get_folder_w as *const c_void;
                }
                _ => {}
            }
        }
    } else if dll_name.eq_ignore_ascii_case("binkw32.dll") && func_name == "_BinkOpen@8" {
        unsafe {
            let _ = ORIG_BINK_OPEN
                .set(std::mem::transmute::<*const c_void, BinkOpenFn>(real_proc));
        }
        return hook_bink_open as *const c_void;
    } else if (dll_name.eq_ignore_ascii_case("xinput1_3.dll")
        || dll_name.eq_ignore_ascii_case("xinput1_4.dll")
        || dll_name.eq_ignore_ascii_case("xinput9_1_0.dll"))
        && func_name == "XInputGetState"
    {
        unsafe {
            let _ = ORIG_XINPUT_GET_STATE
                .set(std::mem::transmute::<*const c_void, XInputGetStateFn>(real_proc));
        }
        return hook_xinput_get_state as *const c_void;
    } else if dll_name.eq_ignore_ascii_case("user32.dll") {
        match func_name {
            "CreateWindowExA" => return hook_create_window_a as *const c_void,
            "CreateWindowExW" => return hook_create_window_w as *const c_void,
            _ => {}
        }
    } else if dll_name.eq_ignore_ascii_case("ws2_32.dll") {
        unsafe {
            match func_name {
                "connect" => {
                    let _ = ORIG_CONNECT.set(std::mem::transmute::<*const c_void, ConnectFn>(real_proc));
                    return hook_connect as *const c_void;
                }
                "bind" => {
                    let _ = ORIG_BIND.set(std::mem::transmute::<*const c_void, BindFn>(real_proc));
                    return hook_bind as *const c_void;
                }
                "sendto" => {
                    let _ = ORIG_SENDTO.set(std::mem::transmute::<*const c_void, SendToFn>(real_proc));
                    return hook_sendto as *const c_void;
                }
                _ => {}
            }
        }
    } else if dll_name.eq_ignore_ascii_case("winhttp.dll") && func_name == "WinHttpConnect" {
        unsafe {
            let _ = ORIG_WINHTTP_CONNECT
                .set(std::mem::transmute::<*const c_void, WinHttpConnectFn>(real_proc));
        }
        return hook_winhttp_connect as *const c_void;
    }
    real_proc
}

// --- Debug / VFS Hooks ---

unsafe extern "system" fn hook_output_debug_string_a(lpOutputString: LPCSTR) {
    if let Some(s) = unsafe { win32::from_ansi_ptr(lpOutputString) } {
        let t = s.trim();
        if !t.is_empty() {
            crate::log_line(&format!("fb-dbg: {t}"));
        }
    }
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
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_ansi_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                let low = raw.to_ascii_lowercase();
                if low.ends_with(".fbrb") || low.contains("mp_beachhead") || low.contains("wake_island") {
                    crate::log_line(&format!("vfs[A]: {raw} -> {}", target.display()));
                }
                if let Ok(cs) = CString::new(target.to_string_lossy().as_bytes()) {
                    return unsafe {
                        win32::CreateFileA(
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
        win32::CreateFileA(
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
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_wide_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                let low = raw.to_ascii_lowercase();
                if low.ends_with(".fbrb") || low.contains("mp_beachhead") || low.contains("wake_island") {
                    crate::log_line(&format!("vfs[W]: {raw} -> {}", target.display()));
                }
                let wide = win32::to_wide_nul(&target.to_string_lossy());
                return unsafe {
                    win32::CreateFileW(
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
        win32::CreateFileW(
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
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_ansi_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                let low = raw.to_ascii_lowercase();
                if low.ends_with(".fbrb") || low.contains("mp_beachhead") || low.contains("wake_island") {
                    crate::log_line(&format!("vfs[A]: {raw} -> {}", target.display()));
                }
                if let Ok(cs) = CString::new(target.to_string_lossy().as_bytes()) {
                    return unsafe { win32::GetFileAttributesA(cs.as_ptr().cast()) };
                }
            }
            VfsResolution::Hide => return win32::INVALID_FILE_ATTRIBUTES,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe { win32::GetFileAttributesA(lpFileName) }
}

unsafe extern "system" fn hook_get_attr_w(lpFileName: LPCWSTR) -> DWORD {
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_wide_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                let low = raw.to_ascii_lowercase();
                if low.ends_with(".fbrb") || low.contains("mp_beachhead") || low.contains("wake_island") {
                    crate::log_line(&format!("vfs[W]: {raw} -> {}", target.display()));
                }
                let wide = win32::to_wide_nul(&target.to_string_lossy());
                return unsafe { win32::GetFileAttributesW(wide.as_ptr()) };
            }
            VfsResolution::Hide => return win32::INVALID_FILE_ATTRIBUTES,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe { win32::GetFileAttributesW(lpFileName) }
}

unsafe extern "system" fn hook_get_attr_ex_w(
    lpFileName: LPCWSTR,
    fInfoLevelId: DWORD,
    lpFileInformation: LPVOID,
) -> BOOL {
    if let (Some(router), Some(raw)) = (ROUTER.get(), unsafe { win32::from_wide_ptr(lpFileName) }) {
        match router.resolve(&raw) {
            VfsResolution::Redirect(target) => {
                let low = raw.to_ascii_lowercase();
                if low.ends_with(".fbrb") || low.contains("mp_beachhead") || low.contains("wake_island") {
                    crate::log_line(&format!("vfs[W]: {raw} -> {}", target.display()));
                }
                let wide = win32::to_wide_nul(&target.to_string_lossy());
                return unsafe {
                    win32::GetFileAttributesExW(wide.as_ptr(), fInfoLevelId, lpFileInformation)
                };
            }
            VfsResolution::Hide => return 0,
            VfsResolution::PassThrough => {}
        }
    }
    unsafe { win32::GetFileAttributesExW(lpFileName, fInfoLevelId, lpFileInformation) }
}

// --- Window Branding ---

fn current_window_title() -> String {
    if let Ok(guard) = crate::hooks::STATE.lock() {
        if let Some(st) = guard.as_ref() {
            return format!("Project OPFOR — {}", st.table.spec(st.active).display_name);
        }
    }
    "Project OPFOR".to_string()
}

unsafe fn brand_window(hwnd: HWND, style: DWORD) {
    if hwnd.is_null() || (style & WS_CHILD) != 0 {
        return;
    }
    let title = win32::to_wide_nul(&current_window_title());
    unsafe { win32::SetWindowTextW(hwnd, title.as_ptr()) };
    unsafe { crate::dx_overlay::on_game_window_created(hwnd) };

    if let Some(path) = ICON_PATH.get() {
        let wpath = win32::to_wide_nul(path);
        let big = *ICON_BIG_H.get_or_init(|| unsafe {
            win32::LoadImageW(
                std::ptr::null_mut(),
                wpath.as_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            ) as usize
        });
        let small = *ICON_SMALL_H.get_or_init(|| unsafe {
            win32::LoadImageW(
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
                win32::SendMessageW(hwnd, WM_SETICON, ICON_BIG, big as LPARAM);
                win32::SetClassLongW(hwnd, GCL_HICON, big as isize);
            }
        }
        if small != 0 {
            unsafe {
                win32::SendMessageW(hwnd, WM_SETICON, ICON_SMALL, small as LPARAM);
                win32::SetClassLongW(hwnd, GCL_HICONSM, small as isize);
            }
        }
    }
}

unsafe extern "system" fn hook_create_window_a(
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
    let hwnd = unsafe {
        win32::CreateWindowExA(ex, cls, name, style, x, y, w, h, parent, menu, inst, param)
    };
    if parent.is_null() && (style & WS_CHILD) == 0 {
        unsafe { brand_window(hwnd, style) };
    }
    hwnd
}

unsafe extern "system" fn hook_create_window_w(
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
    let hwnd = unsafe {
        win32::CreateWindowExW(ex, cls, name, style, x, y, w, h, parent, menu, inst, param)
    };
    if parent.is_null() && (style & WS_CHILD) == 0 {
        unsafe { brand_window(hwnd, style) };
    }
    hwnd
}

// --- Offline Network Guard ---

fn is_loopback_v4(addr: [u8; 4]) -> bool {
    addr[0] == 127
}

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim();
    h.eq_ignore_ascii_case("localhost") || h == "::1" || h.starts_with("127.")
}

unsafe extern "system" fn hook_connect(s: usize, name: *const c_void, namelen: i32) -> i32 {
    if !name.is_null() && namelen >= std::mem::size_of::<SockAddrIn>() as i32 {
        let sin = unsafe { &*(name as *const SockAddrIn) };
        if sin.sin_family == AF_INET && !is_loopback_v4(sin.sin_addr) {
            unsafe { WSASetLastError(WSAEACCES) };
            return -1;
        }
    }
    if let Some(&orig) = ORIG_CONNECT.get() {
        unsafe { orig(s, name, namelen) }
    } else {
        -1
    }
}

unsafe extern "system" fn hook_bind(s: usize, name: *const c_void, namelen: i32) -> i32 {
    if let Some(&orig) = ORIG_BIND.get() {
        if !name.is_null() && namelen >= std::mem::size_of::<SockAddrIn>() as i32 {
            let sin = unsafe { &*(name as *const SockAddrIn) };
            if sin.sin_family == AF_INET && sin.sin_addr == [0, 0, 0, 0] {
                let mut local = SockAddrIn {
                    sin_family: sin.sin_family,
                    sin_port: sin.sin_port,
                    sin_addr: [127, 0, 0, 1],
                    sin_zero: sin.sin_zero,
                };
                return unsafe {
                    orig(
                        s,
                        (&raw mut local).cast(),
                        std::mem::size_of::<SockAddrIn>() as i32,
                    )
                };
            }
        }
        return unsafe { orig(s, name, namelen) };
    }
    -1
}

unsafe extern "system" fn hook_sendto(
    s: usize,
    buf: *const u8,
    len: i32,
    flags: i32,
    to: *const c_void,
    tolen: i32,
) -> i32 {
    if !to.is_null() && tolen >= std::mem::size_of::<SockAddrIn>() as i32 {
        let sin = unsafe { &*(to as *const SockAddrIn) };
        if sin.sin_family == AF_INET && !is_loopback_v4(sin.sin_addr) {
            unsafe { WSASetLastError(WSAEACCES) };
            return -1;
        }
    }
    if let Some(&orig) = ORIG_SENDTO.get() {
        unsafe { orig(s, buf, len, flags, to, tolen) }
    } else {
        -1
    }
}

unsafe extern "system" fn hook_winhttp_connect(
    hSession: HANDLE,
    pswzServerName: LPCWSTR,
    nServerPort: u16,
    dwReserved: DWORD,
) -> HANDLE {
    if let Some(host) = unsafe { win32::from_wide_ptr(pswzServerName) } {
        if !is_loopback_host(&host) {
            unsafe { SetLastError(ERROR_ACCESS_DENIED) };
            return std::ptr::null_mut();
        }
    }
    if let Some(&orig) = ORIG_WINHTTP_CONNECT.get() {
        unsafe { orig(hSession, pswzServerName, nServerPort, dwReserved) }
    } else {
        std::ptr::null_mut()
    }
}

// --- Isolated UserData (per-era Documents isolation: BFBC2/Vietnam, BFBC1, BF1943) ---

fn active_user_docs_dir() -> Option<&'static str> {
    let active = crate::hooks::STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.active))
        .unwrap_or(opfor_vfs::ModeId::Bfbc2);
    match active {
        opfor_vfs::ModeId::Bfbc1 => USER_DOCS_BFBC1.get().or_else(|| USER_DOCS_DIR.get()),
        opfor_vfs::ModeId::Bf1943 => USER_DOCS_BF1943.get().or_else(|| USER_DOCS_DIR.get()),
        _ => USER_DOCS_DIR.get(),
    }
    .map(|s| s.as_str())
}

unsafe extern "system" fn hook_sh_get_folder_a(
    hwnd: HWND,
    csidl: i32,
    token: HANDLE,
    flags: DWORD,
    out_path: *mut u8,
) -> i32 {
    if (csidl & CSIDL_FLAG_MASK) == CSIDL_PERSONAL && !out_path.is_null() {
        if let Some(docs) = active_user_docs_dir() {
            let bytes = docs.as_bytes();
            let n = bytes.len().min(259);
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_path, n);
                *out_path.add(n) = 0;
            }
            return 0; // S_OK
        }
    }
    if let Some(&orig) = ORIG_SH_GET_FOLDER_A.get() {
        unsafe { orig(hwnd, csidl, token, flags, out_path) }
    } else {
        -2147467259 // E_FAIL
    }
}

unsafe extern "system" fn hook_sh_get_folder_w(
    hwnd: HWND,
    csidl: i32,
    token: HANDLE,
    flags: DWORD,
    out_path: *mut u16,
) -> i32 {
    if (csidl & CSIDL_FLAG_MASK) == CSIDL_PERSONAL && !out_path.is_null() {
        if let Some(docs) = active_user_docs_dir() {
            let wide = win32::to_wide_nul(docs);
            let n = wide.len().min(260);
            unsafe {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), out_path, n);
                *out_path.add(n - 1) = 0;
            }
            return 0; // S_OK
        }
    }
    if let Some(&orig) = ORIG_SH_GET_FOLDER_W.get() {
        unsafe { orig(hwnd, csidl, token, flags, out_path) }
    } else {
        -2147467259 // E_FAIL
    }
}

// --- Bink Intro Movie Skip (`binkw32.dll` `_BinkOpen@8`) ---

unsafe extern "system" fn hook_bink_open(name: LPCSTR, flags: u32) -> *mut c_void {
    // Only skip stock BFBC2 intro movies for WW2 (1943) and BFBC1.
    let skip_intro = crate::hooks::STATE
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref()
                .map(|s| matches!(s.active, opfor_vfs::ModeId::Bf1943 | opfor_vfs::ModeId::Bfbc1))
        })
        .unwrap_or(false);
    // Note: when BINKFROMMEMORY (0x0400_0000) is set, `name` is a memory pointer, NOT a path.
    if skip_intro && (flags & 0x0400_0000) == 0 {
        if let Some(raw) = unsafe { win32::from_ansi_ptr(name) } {
            let lower = raw.to_ascii_lowercase();
            if lower.contains("ea_logo")
                || lower.contains("dolbydigital")
                || lower.contains("esrb_rating")
                || lower.contains("attraction")
            {
                crate::log_line(&format!("BinkOpen: skipped stock intro '{raw}'"));
                return std::ptr::null_mut();
            }
        }
    }
    if let Some(&orig) = ORIG_BINK_OPEN.get() {
        unsafe { orig(name, flags) }
    } else {
        std::ptr::null_mut()
    }
}

// --- Native Sony DualShock 4 / DualSense HID -> XInput Bridge (`XInputGetState`) ---

unsafe extern "system" fn hook_xinput_get_state(
    user_index: u32,
    state: *mut crate::controller::XInputState,
) -> u32 {
    if let Some(&orig) = ORIG_XINPUT_GET_STATE.get() {
        let rc = unsafe { orig(user_index, state) };
        if rc == 0 {
            return 0;
        }
    }
    // If no XInput pad is connected on slot 0, fall back to native Sony VID_054C HID translation
    if user_index == 0 && !state.is_null() {
        let mut translated = crate::controller::XInputState::default();
        if crate::controller::poll_sony_xinput(&mut translated) {
            unsafe { *state = translated };
            return 0; // ERROR_SUCCESS
        }
    }
    1167 // ERROR_DEVICE_NOT_CONNECTED
}
