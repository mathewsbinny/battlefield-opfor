//! Pure-Rust outbound network guard for `BFBC2Game.exe` (zero outbound traffic).
//!
//! Policy (matching `project-opfor-bfbc2/src/dll/net.cpp`):
//! - `WS2_32.dll` `connect` / `sendto`: allow loopback (`127.0.0.0/8`, `::1`) only.
//! - `WS2_32.dll` `bind`: rewrite `0.0.0.0` (`INADDR_ANY`) to `127.0.0.1` so the
//!   in-process server never listens on a public interface.
//! - `WS2_32.dll` `gethostbyname` / `getaddrinfo`: allow `"localhost"` and `127.*` only.
//! - `WINHTTP.dll` `WinHttpConnect` / `WinHttpSendRequest`: always fail (`NULL` / `0`).

#![allow(non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::iat_vfs::patch_iat;
use crate::win32::{self, BOOL, DWORD, HMODULE, LPCSTR, LPCWSTR, LPVOID};

type SOCKET = usize;
const AF_INET: u16 = 2;
const AF_INET6: u16 = 23;
const SOCKET_ERROR: i32 = -1;
const WSAEACCES: i32 = 10013;
const WSAHOST_NOT_FOUND: i32 = 11001;

#[repr(C)]
struct SockAddr {
    sa_family: u16,
    sa_data: [u8; 14],
}

#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: [u8; 4],
    sin_zero: [u8; 8],
}

#[repr(C)]
struct SockAddrIn6 {
    sin6_family: u16,
    sin6_port: u16,
    sin6_flowinfo: u32,
    sin6_addr: [u8; 16],
    sin6_scope_id: u32,
}

type ConnectFn = unsafe extern "system" fn(SOCKET, *const SockAddr, i32) -> i32;
type SendToFn = unsafe extern "system" fn(SOCKET, *const u8, i32, i32, *const SockAddr, i32) -> i32;
type BindFn = unsafe extern "system" fn(SOCKET, *const SockAddr, i32) -> i32;
type GetHostByNameFn = unsafe extern "system" fn(LPCSTR) -> LPVOID;
type GetAddrInfoFn = unsafe extern "system" fn(LPCSTR, LPCSTR, LPVOID, *mut LPVOID) -> i32;

static ORIG_CONNECT: OnceLock<ConnectFn> = OnceLock::new();
static ORIG_SENDTO: OnceLock<SendToFn> = OnceLock::new();
static ORIG_BIND: OnceLock<BindFn> = OnceLock::new();
static ORIG_GETHOSTBYNAME: OnceLock<GetHostByNameFn> = OnceLock::new();
static ORIG_GETADDRINFO: OnceLock<GetAddrInfoFn> = OnceLock::new();
static BLOCKED_COUNT: AtomicU32 = AtomicU32::new(0);

#[link(name = "ws2_32")]
unsafe extern "system" {
    fn WSASetLastError(iError: i32);
}

/// Pure loopback check for IPv4/IPv6 `sockaddr`.
unsafe fn is_loopback(sa: *const SockAddr) -> bool {
    if sa.is_null() {
        return true;
    }
    let fam = unsafe { (*sa).sa_family };
    if fam == AF_INET {
        let sin = unsafe { &*(sa as *const SockAddrIn) };
        sin.sin_addr[0] == 127
    } else if fam == AF_INET6 {
        let sin6 = unsafe { &*(sa as *const SockAddrIn6) };
        sin6.sin6_addr == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
    } else {
        false
    }
}

/// Pure loopback check for DNS hostnames (`localhost` or `127.x.x.x`).
#[must_use]
pub fn is_local_hostname(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.eq_ignore_ascii_case("localhost") || trimmed == "::1" {
        return true;
    }
    if let Some(first) = trimmed.split('.').next() {
        if first == "127" && trimmed.split('.').count() == 4 {
            return trimmed.split('.').all(|p| p.parse::<u8>().is_ok());
        }
    }
    false
}

fn log_blocked(op: &str, detail: &str) {
    let n = BLOCKED_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    crate::log_line(&format!("net BLOCKED {op} {detail} [#{n}]"));
}

/// Install the Win32 IAT network guard on `WS2_32.dll` and `WINHTTP.dll`.
pub fn install() -> usize {
    let exe: HMODULE = unsafe { win32::GetModuleHandleW(std::ptr::null()) };
    if exe.is_null() {
        return 0;
    }
    let mut n = 0usize;
    unsafe {
        if let Some(o) = patch_iat(exe, "ws2_32.dll", "connect", hook_connect as *const c_void) {
            let _ = ORIG_CONNECT.set(std::mem::transmute::<*const c_void, ConnectFn>(o));
            n += 1;
        }
        if let Some(o) = patch_iat(exe, "ws2_32.dll", "sendto", hook_sendto as *const c_void) {
            let _ = ORIG_SENDTO.set(std::mem::transmute::<*const c_void, SendToFn>(o));
            n += 1;
        }
        if let Some(o) = patch_iat(exe, "ws2_32.dll", "bind", hook_bind as *const c_void) {
            let _ = ORIG_BIND.set(std::mem::transmute::<*const c_void, BindFn>(o));
            n += 1;
        }
        if let Some(o) = patch_iat(exe, "ws2_32.dll", "gethostbyname", hook_gethostbyname as *const c_void) {
            let _ = ORIG_GETHOSTBYNAME.set(std::mem::transmute::<*const c_void, GetHostByNameFn>(o));
            n += 1;
        }
        if let Some(o) = patch_iat(exe, "ws2_32.dll", "getaddrinfo", hook_getaddrinfo as *const c_void) {
            let _ = ORIG_GETADDRINFO.set(std::mem::transmute::<*const c_void, GetAddrInfoFn>(o));
            n += 1;
        }
        if patch_iat(exe, "winhttp.dll", "WinHttpConnect", hook_winhttp_connect as *const c_void).is_some() {
            n += 1;
        }
        if patch_iat(exe, "winhttp.dll", "WinHttpSendRequest", hook_winhttp_send as *const c_void).is_some() {
            n += 1;
        }
    }
    n
}

unsafe extern "system" fn hook_connect(s: SOCKET, name: *const SockAddr, namelen: i32) -> i32 {
    if !unsafe { is_loopback(name) } {
        log_blocked("connect", "non-loopback");
        unsafe { WSASetLastError(WSAEACCES) };
        return SOCKET_ERROR;
    }
    if let Some(&orig) = ORIG_CONNECT.get() {
        unsafe { orig(s, name, namelen) }
    } else {
        SOCKET_ERROR
    }
}

unsafe extern "system" fn hook_sendto(
    s: SOCKET,
    buf: *const u8,
    len: i32,
    flags: i32,
    to: *const SockAddr,
    tolen: i32,
) -> i32 {
    if !unsafe { is_loopback(to) } {
        log_blocked("sendto", "non-loopback");
        unsafe { WSASetLastError(WSAEACCES) };
        return SOCKET_ERROR;
    }
    if let Some(&orig) = ORIG_SENDTO.get() {
        unsafe { orig(s, buf, len, flags, to, tolen) }
    } else {
        SOCKET_ERROR
    }
}

unsafe extern "system" fn hook_bind(s: SOCKET, name: *const SockAddr, namelen: i32) -> i32 {
    let Some(&orig) = ORIG_BIND.get() else {
        return SOCKET_ERROR;
    };
    if !name.is_null() && unsafe { (*name).sa_family } == AF_INET {
        let sin = unsafe { &*(name as *const SockAddrIn) };
        if sin.sin_addr == [0, 0, 0, 0] {
            let mut local = SockAddrIn {
                sin_family: AF_INET,
                sin_port: sin.sin_port,
                sin_addr: [127, 0, 0, 1],
                sin_zero: [0; 8],
            };
            return unsafe { orig(s, (&raw mut local).cast(), namelen) };
        }
    }
    unsafe { orig(s, name, namelen) }
}

unsafe extern "system" fn hook_gethostbyname(name: LPCSTR) -> LPVOID {
    let host = unsafe { win32::from_ansi_ptr(name) }.unwrap_or_default();
    if !is_local_hostname(&host) {
        log_blocked("gethostbyname", &host);
        unsafe { WSASetLastError(WSAHOST_NOT_FOUND) };
        return std::ptr::null_mut();
    }
    if let Some(&orig) = ORIG_GETHOSTBYNAME.get() {
        unsafe { orig(name) }
    } else {
        std::ptr::null_mut()
    }
}

unsafe extern "system" fn hook_getaddrinfo(
    node: LPCSTR,
    service: LPCSTR,
    hints: LPVOID,
    res: *mut LPVOID,
) -> i32 {
    let host = unsafe { win32::from_ansi_ptr(node) }.unwrap_or_default();
    if !host.is_empty() && !is_local_hostname(&host) {
        log_blocked("getaddrinfo", &host);
        return WSAHOST_NOT_FOUND;
    }
    if let Some(&orig) = ORIG_GETADDRINFO.get() {
        unsafe { orig(node, service, hints, res) }
    } else {
        WSAHOST_NOT_FOUND
    }
}

unsafe extern "system" fn hook_winhttp_connect(
    _session: LPVOID,
    server: LPCWSTR,
    _port: u16,
    _reserved: DWORD,
) -> LPVOID {
    let host = unsafe { win32::from_wide_ptr(server) }.unwrap_or_default();
    log_blocked("WinHttpConnect", &host);
    std::ptr::null_mut()
}

unsafe extern "system" fn hook_winhttp_send(
    _req: LPVOID,
    _hdrs: LPCWSTR,
    _hdrs_len: DWORD,
    _opt: LPVOID,
    _opt_len: DWORD,
    _total: DWORD,
    _ctx: usize,
) -> BOOL {
    log_blocked("WinHttpSendRequest", "");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hostname_rules() {
        assert!(is_local_hostname("localhost"));
        assert!(is_local_hostname("LOCALHOST"));
        assert!(is_local_hostname("127.0.0.1"));
        assert!(is_local_hostname("::1"));
        assert!(!is_local_hostname("fesl.ea.com"));
        assert!(!is_local_hostname("192.168.1.1"));
    }
}
