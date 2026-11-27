//! `ui` subsystem compatibility guards for `opfor-host`.
#![allow(dead_code, non_snake_case, unused_imports)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use opfor_vfs::ModeId;

use super::*;

const STR_EQ: usize = 0x0043_04D0;

const STR_EQ_PROLOGUE: [u8; 6] = [0x8B, 0x4C, 0x24, 0x08, 0x8B, 0xC1];

const CONQUEST_LITERAL: usize = 0x0142_ABD8;

// Invalid allocator `this` fallback (`0x0040A410`: `mov eax, [ecx]; mov eax, [eax]; jmp eax`)
const SP_ALLOC_GUARD: usize = 0x0040_A410;

const SP_ALLOC_PROLOGUE: [u8; 6] = [0x8B, 0x01, 0x8B, 0x00, 0xFF, 0xE0];

#[cfg(target_arch = "x86")]
type StrEqFn = unsafe extern "cdecl" fn(*const EaString, *const u8) -> bool;

#[cfg(target_arch = "x86")]
static ORIG_STR_EQ: OnceLock<StrEqFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_alloc_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x01000000",
        "jb 2f",
        "cmp ecx, 0x7fff0000",
        "jae 2f",
        "mov eax, dword ptr [ecx]",
        "mov eax, dword ptr [eax]",
        "jmp eax",
        "2:",
        "mov eax, dword ptr [esp + 4]",
        "push eax",
        "mov eax, 0x00420db0",
        "call eax",
        "add esp, 4",
        "ret 8",
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn detour_str_eq(s: *const EaString, text: *const u8) -> bool {
    if (s as usize) < 0x10000 || (text as usize) < 0x10000 {
        return false;
    }
    let eq = if let Some(&orig) = ORIG_STR_EQ.get() {
        unsafe { orig(s, text) }
    } else {
        false
    };
    if eq || text as usize != CONQUEST_LITERAL || s.is_null() {
        return eq;
    }
    let (b, e) = unsafe { ((*s).begin, (*s).end) };
    if b.is_null() || e <= b {
        return false;
    }
    let n = e as usize - b as usize;
    if n == 9 {
        unsafe { std::slice::from_raw_parts(b, 9) == b"onslaught" }
    } else if n == 14 {
        unsafe { std::slice::from_raw_parts(b, 14) == b"airsuperiority" }
    } else {
        false
    }
}

pub(super) unsafe fn install(mode: ModeId) -> usize {
    let mut installed = 0usize;
    let is_console_port = matches!(mode, ModeId::Bf1943 | ModeId::Bfbc1);
    let is_bfbc1 = mode == ModeId::Bfbc1;
    #[cfg(target_arch = "x86")]
    {
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                STR_EQ,
                &STR_EQ_PROLOGUE,
                detour_str_eq as *const c_void,
            )
        } {
            let _ = ORIG_STR_EQ.set(unsafe { std::mem::transmute::<*const c_void, StrEqFn>(tramp) });
            installed += 1;
        }
        if !is_console_port {
            return installed;
        }
        if !is_bfbc1 {
            return installed;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ALLOC_GUARD,
                &SP_ALLOC_PROLOGUE,
                sp_alloc_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
    }
    let _ = (is_console_port, is_bfbc1);
    installed
}
