//! Diagnostic gameplay probes (`kill_probe`, `capture_probe`) and live APT display-tree
//! reader (`menutree`) for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/{killprobe,captureprobe,menutree}.cpp`.
//! Strictly gated behind `[debug]` flags in `config/opfor.ini` so normal play incurs zero overhead.

#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

// --- kill_probe targets (BFBC2 build 795745) ---
const KP_DISPATCH: usize = 0x0087_0070;
const KP_DISPATCH_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0x0C, 0x01, 0x00, 0x00];
const KP_KILLED: usize = 0x0086_DC90;
const KP_KILLED_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0x08, 0x01, 0x00, 0x00];
const KP_SCORE_KILL: usize = 0x0086_C070;
const KP_SCORE_KILL_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];
const KP_AWARD: usize = 0x0086_BE20;
const KP_AWARD_PROLOGUE: [u8; 6] = [0x83, 0xEC, 0x0C, 0x56, 0x6A, 0x00];

const MAX_KILL_LINES: u32 = 4000;
static KILL_LINES: AtomicU32 = AtomicU32::new(0);

// --- Live APT display-tree (`menutree`) ---
const APT_TARGET_SIM: usize = 0x01BE_9C5C;

#[cfg(target_arch = "x86")]
type DispatchFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut u8);
#[cfg(target_arch = "x86")]
type KilledFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void, u32, u32, u32);
#[cfg(target_arch = "x86")]
type ScoreKillFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, u32, u32, u32, u32, u32);
#[cfg(target_arch = "x86")]
type AwardFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *const c_void);

#[cfg(target_arch = "x86")]
static ORIG_DISPATCH: OnceLock<DispatchFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_KILLED: OnceLock<KilledFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_SCORE_KILL: OnceLock<ScoreKillFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_AWARD: OnceLock<AwardFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
fn safe_read_u32(base: usize, off: usize) -> Option<u32> {
    let addr = base.checked_add(off)?;
    if !(0x10000..0x7FFF_0000).contains(&addr) {
        return None;
    }
    Some(unsafe { *(addr as *const u32) })
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_dispatch(self_: *mut c_void, edx: *mut c_void, ev: *mut u8) {
    if !ev.is_null() && KILL_LINES.fetch_add(1, Ordering::Relaxed) < MAX_KILL_LINES {
        let base = ev as usize;
        let kind = safe_read_u32(base, 0x88).unwrap_or(0xBADB_AD);
        let p90 = safe_read_u32(base, 0x90).unwrap_or(0);
        let p94 = safe_read_u32(base, 0x94).unwrap_or(0);
        crate::log_line(&format!(
            "killprobe: event {kind} +90=0x{p90:08X} +94=0x{p94:08X}"
        ));
    }
    if let Some(&orig) = ORIG_DISPATCH.get() {
        unsafe { orig(self_, edx, ev) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_killed(
    self_: *mut c_void,
    edx: *mut c_void,
    victim: *mut c_void,
    killer: *mut c_void,
    damage: *mut c_void,
    headshot: u32,
    a5: u32,
    a6: u32,
) {
    if KILL_LINES.fetch_add(1, Ordering::Relaxed) < MAX_KILL_LINES {
        let v_team = safe_read_u32(victim as usize, 0xA8).unwrap_or(0);
        let k_team = safe_read_u32(killer as usize, 0xA8).unwrap_or(0);
        crate::log_line(&format!(
            "killprobe: player_killed victim={victim:p} (team {v_team}) killer={killer:p} (team {k_team}) hs={}",
            headshot & 0xFF
        ));
    }
    if let Some(&orig) = ORIG_KILLED.get() {
        unsafe { orig(self_, edx, victim, killer, damage, headshot, a5, a6) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_score_kill(
    self_: *mut c_void,
    edx: *mut c_void,
    killer: *mut c_void,
    victim: *mut c_void,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
) {
    if KILL_LINES.fetch_add(1, Ordering::Relaxed) < MAX_KILL_LINES {
        crate::log_line(&format!(
            "killprobe: score_kill killer={killer:p} victim={victim:p} args=({a3:#X},{a4:#X},{a5:#X},{a6:#X},{a7:#X})"
        ));
    }
    if let Some(&orig) = ORIG_SCORE_KILL.get() {
        unsafe { orig(self_, edx, killer, victim, a3, a4, a5, a6, a7) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_award(
    self_: *mut c_void,
    edx: *mut c_void,
    player: *mut c_void,
    entry: *const c_void,
) {
    if KILL_LINES.fetch_add(1, Ordering::Relaxed) < MAX_KILL_LINES {
        let key_ptr = safe_read_u32(entry as usize, 0).unwrap_or(0) as *const u8;
        let key = unsafe { crate::win32::from_ansi_ptr(key_ptr) }.unwrap_or_else(|| "(null)".into());
        crate::log_line(&format!("killprobe: award '{key}' player={player:p}"));
    }
    if let Some(&orig) = ORIG_AWARD.get() {
        unsafe { orig(self_, edx, player, entry) };
    }
}

/// Read the name of an `AptCIH` display item (`+0x08` name buffer: u16 len at +2, UTF-8/ANSI at +8).
#[cfg(target_arch = "x86")]
pub fn apt_item_name(item: usize) -> String {
    let Some(buf) = safe_read_u32(item, 0x08).map(|v| v as usize) else {
        return String::new();
    };
    if !(0x10000..0x7FFF_0000).contains(&buf) {
        return String::new();
    }
    let len = unsafe { *((buf + 2) as *const u16) } as usize;
    if len == 0 || len > 128 {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts((buf + 8) as *const u8, len) };
    String::from_utf8_lossy(slice).into_owned()
}

/// Locate the live APT `"menuMC"` root display item (`[[[[0x01BE9C5C]+0x94]+0x20]+0x60]`).
#[cfg(target_arch = "x86")]
pub fn find_menu_root() -> Option<usize> {
    let target = safe_read_u32(APT_TARGET_SIM, 0)? as usize;
    let level = safe_read_u32(target, 0x94)? as usize;
    let ch = safe_read_u32(level, 0x20)? as usize;
    let root = safe_read_u32(ch, 0x60)? as usize;
    if root != 0 && apt_item_name(root) == "menuMC" {
        Some(root)
    } else {
        None
    }
}

/// Install optional `[debug]` probes (`kill_probe = true`).
pub unsafe fn install(opfor_root: &Path) -> usize {
    let mut kill_probe = false;
    if let Ok(txt) = fs::read_to_string(opfor_root.join("config").join("opfor.ini")) {
        let mut sec = String::new();
        for raw in txt.lines() {
            let line = raw.trim();
            if line.starts_with('[') && line.ends_with(']') {
                sec = line[1..line.len() - 1].trim().to_ascii_lowercase();
                continue;
            }
            if sec == "debug"
                && let Some((k, v)) = line.split_once('=')
                && k.trim().eq_ignore_ascii_case("kill_probe")
            {
                kill_probe = matches!(
                    v.trim().trim_matches('"').to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
        }
    }
    if !kill_probe {
        return 0;
    }
    let mut installed = 0usize;
    #[cfg(target_arch = "x86")]
    {
        if let Some(t) = unsafe {
            crate::hooks::install_detour(KP_DISPATCH, &KP_DISPATCH_PROLOGUE, detour_dispatch as *const c_void)
        } {
            let _ = ORIG_DISPATCH.set(unsafe { std::mem::transmute::<*const c_void, DispatchFn>(t) });
            installed += 1;
        }
        if let Some(t) = unsafe {
            crate::hooks::install_detour(KP_KILLED, &KP_KILLED_PROLOGUE, detour_killed as *const c_void)
        } {
            let _ = ORIG_KILLED.set(unsafe { std::mem::transmute::<*const c_void, KilledFn>(t) });
            installed += 1;
        }
        if let Some(t) = unsafe {
            crate::hooks::install_detour(KP_SCORE_KILL, &KP_SCORE_KILL_PROLOGUE, detour_score_kill as *const c_void)
        } {
            let _ = ORIG_SCORE_KILL.set(unsafe { std::mem::transmute::<*const c_void, ScoreKillFn>(t) });
            installed += 1;
        }
        if let Some(t) = unsafe {
            crate::hooks::install_detour(KP_AWARD, &KP_AWARD_PROLOGUE, detour_award as *const c_void)
        } {
            let _ = ORIG_AWARD.set(unsafe { std::mem::transmute::<*const c_void, AwardFn>(t) });
            installed += 1;
        }
        crate::log_line(&format!("probes: kill_probe active ({installed}/4 hooks)"));
    }
    installed
}
