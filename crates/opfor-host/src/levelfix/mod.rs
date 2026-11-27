//! Essential offline/AI/WW2/BFBC1 level-compatibility guards for `opfor-host`.
//!
//! Organized by Frostbite runtime subsystem:
//! - [`trie`]: Trie concurrency locking, bounds, unlink, and pool guards
//! - [`havok`]: Havok physics, destruction, StaticModelGroup, and `PartSetup` guards
//! - [`net`]: Network, ghost bitstream, loopback, and entity/component guards
//! - [`ui`]: `StrEq`, `UIIngame` Flash-ready fallback, and `ResourceManager` guards
//! - [`render`]: D3D11 SRV (`PS_BIND`/`VS_BIND`), shader, postfx, and mesh LOD guards
//! - [`vo`]: Voiceover (`VO_WALK`), audio mix, AI, blueprint, and SP mission guards

#![allow(dead_code, non_snake_case, unused_imports)]

mod havok;
mod net;
mod render;
mod trie;
mod ui;
mod vo;

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use opfor_vfs::ModeId;

static D3D_LO: AtomicUsize = AtomicUsize::new(0);
static D3D_HI: AtomicUsize = AtomicUsize::new(0);
pub(super) static ORIG_NODE_WALK: OnceLock<
    unsafe extern "fastcall" fn(usize, *mut c_void, usize, usize) -> usize,
> = OnceLock::new();

#[link(name = "kernel32")]
unsafe extern "system" {
    pub(super) fn GetModuleHandleExA(flags: u32, name: *const c_void, out: *mut *mut c_void) -> i32;
    pub(super) fn GetModuleFileNameA(module: *mut c_void, buf: *mut u8, size: u32) -> u32;
    pub(super) fn IsBadReadPtr(ptr: *const c_void, len: usize) -> i32;
    pub(super) fn IsBadWritePtr(ptr: *mut c_void, len: usize) -> i32;
}

#[repr(C)]
pub(super) struct EaString {
    begin: *const u8,
    end: *const u8,
}

#[cfg(target_arch = "x86")]
pub(super) type LevelListWalkFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void);

#[cfg(target_arch = "x86")]
pub(super) unsafe fn view_looks_live(view: usize) -> bool {
    if view < 0x10000 || (view & 3) != 0 {
        return false;
    }
    if unsafe { IsBadReadPtr(view as *const c_void, 4) } != 0 {
        return false;
    }
    let vt = unsafe { *(view as *const usize) };
    if vt < 0x10000 || (vt & 3) != 0 {
        return false;
    }
    let lo = D3D_LO.load(Ordering::Relaxed);
    let hi = D3D_HI.load(Ordering::Relaxed);
    if lo != 0 && vt >= lo && vt < hi {
        return true;
    }
    let mut m: *mut c_void = std::ptr::null_mut();
    // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS (4) | UNCHANGED_REFCOUNT (2)
    if unsafe { GetModuleHandleExA(6, vt as *const c_void, &mut m) } == 0 || m.is_null() {
        return false;
    }
    let mut path = [0u8; 260];
    let n = unsafe { GetModuleFileNameA(m, path.as_mut_ptr(), path.len() as u32) } as usize;
    if n == 0 {
        return false;
    }
    let s = String::from_utf8_lossy(&path[..n]);
    let base = s.rsplit('\\').next().unwrap_or(&s).to_ascii_lowercase();
    if !base.starts_with("d3d1") && base != "renderdoc.dll" {
        return false;
    }
    let m_base = m as usize;
    let lfanew = unsafe { *((m_base + 0x3C) as *const u32) } as usize;
    let size_of_image = unsafe { *((m_base + lfanew + 0x50) as *const u32) } as usize;
    if lo == 0 && size_of_image > 0 {
        D3D_LO.store(m_base, Ordering::Relaxed);
        D3D_HI.store(m_base + size_of_image, Ordering::Relaxed);
        crate::log_line(&format!(
            "levelfix: D3D SRV guard locked to {base} (0x{m_base:08X}..0x{:08X})",
            m_base + size_of_image
        ));
    }
    true
}

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
pub(super) unsafe extern "cdecl" fn opfor_check_views(cache: usize, views_off: u32, first_off: u32, last_off: u32) {
    if cache < 0x10000 {
        return;
    }
    let a = unsafe { *((cache + first_off as usize) as *const u32) };
    let b = unsafe { *((cache + last_off as usize) as *const u32) };
    if a > b || b >= 128 {
        return;
    }
    let slots = (cache + views_off as usize) as *mut usize;
    for i in a..=b {
        let p = unsafe { *slots.add(i as usize) };
        if p != 0 && !unsafe { view_looks_live(p) } {
            unsafe { *slots.add(i as usize) = 0 };
        }
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
pub(super) unsafe extern "cdecl" fn opfor_is_bad_ptr(p: usize) -> u32 {
    if unsafe { IsBadReadPtr(p as *const c_void, 4) } != 0 {
        1
    } else {
        0
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
pub(super) unsafe extern "cdecl" fn opfor_is_bad_pool_ptr(p: usize) -> u32 {
    if p < 0x10000 || unsafe { IsBadWritePtr(p as *mut c_void, 4) } != 0 {
        1
    } else {
        0
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
pub(super) unsafe extern "cdecl" fn opfor_is_bad_write_ptr(p: usize) -> u32 {
    if p < 0x10000
        || unsafe { IsBadWritePtr(p as *mut c_void, 8) } != 0
        || unsafe { IsBadReadPtr((p - 8) as *const c_void, 8) } != 0
    {
        1
    } else {
        0
    }
}

pub unsafe fn install(mode: ModeId) -> (usize, usize) {
    let is_console_port = matches!(mode, ModeId::Bf1943 | ModeId::Bfbc1);
    let is_bfbc1 = mode == ModeId::Bfbc1;
    let expected = if is_bfbc1 {
        272
    } else if is_console_port {
        34
    } else {
        2
    };
    let mut installed = 0usize;
    installed += unsafe { havok::install(mode) };
    installed += unsafe { ui::install(mode) };
    installed += unsafe { vo::install(mode) };
    installed += unsafe { render::install(mode) };
    installed += unsafe { trie::install(mode) };
    installed += unsafe { net::install(mode) };
    (installed, expected)
}
