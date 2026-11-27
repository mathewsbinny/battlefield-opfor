//! Voice-over event probe & BF1943 MultiKill call-out carrier (`voprobe`) for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/voprobe.cpp`:
//! - Hooks `VoSystem::raise` (`0x00637770`) to trigger BF1943's MultiKill call-out (`ETGold` = 40)
//!   when the local soldier achieves `[vo] multikill_count` (default 3) kills within
//!   `[vo] multikill_window_ms` (default 5000 ms).
//! - When `[debug] vo_probe = true`, also hooks `VoDb::match` (`0x00636010`) and logs VO event
//!   enum names from `0x01C33688` along with matched trigger-row counts.

#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const VO_RAISE: usize = 0x0063_7770;
const VO_RAISE_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];
const VO_MATCH: usize = 0x0063_6010;
const VO_MATCH_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8];

const SERVER_VO_GLOBAL: usize = 0x0156_6B6C;
const CLIENT_VO_GLOBAL: usize = 0x0157_1408;
const VO_TYPE_TABLE: usize = 0x01C3_3688;
const VO_TYPE_COUNT: i32 = 125;

const ET_SOLDIER_KILLED: i32 = 0x11;
const ET_MULTIKILL_CARRIER: i32 = 40; // ETGold
const MAX_PROBE_LINES: u32 = 400;

#[cfg(target_arch = "x86")]
type RaiseFn = unsafe extern "fastcall" fn(
    *mut c_void,
    *mut c_void,
    i32,
    *mut c_void,
    *mut c_void,
    *const f32,
    i32,
);
#[cfg(target_arch = "x86")]
type MatchFn = unsafe extern "fastcall" fn(
    *mut c_void,
    *mut c_void,
    i32,
    *mut c_void,
    *mut c_void,
    i32,
) -> i32;

#[cfg(target_arch = "x86")]
static ORIG_RAISE: OnceLock<RaiseFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_MATCH: OnceLock<MatchFn> = OnceLock::new();

static PROBE_ENABLED: AtomicBool = AtomicBool::new(false);
static PROBE_LINES: AtomicU32 = AtomicU32::new(0);
static MULTI_COUNT: AtomicU32 = AtomicU32::new(3);
static MULTI_WINDOW_MS: AtomicU32 = AtomicU32::new(5000);

thread_local! {
    static LAST_MATCHED: std::cell::Cell<i32> = const { std::cell::Cell::new(-1) };
}

struct KillTracker {
    last_killer: usize,
    timestamps: Vec<Instant>,
}

static KILL_TRACKER: Mutex<KillTracker> = Mutex::new(KillTracker {
    last_killer: 0,
    timestamps: Vec::new(),
});

fn record_multikill(inflictor: usize) -> bool {
    let Ok(mut tr) = KILL_TRACKER.lock() else {
        return false;
    };
    let now = Instant::now();
    let window = Duration::from_millis(u64::from(MULTI_WINDOW_MS.load(Ordering::Relaxed)));
    let need = MULTI_COUNT.load(Ordering::Relaxed) as usize;
    if tr.last_killer != inflictor {
        tr.last_killer = inflictor;
        tr.timestamps.clear();
    }
    tr.timestamps.retain(|&t| now.saturating_duration_since(t) <= window);
    tr.timestamps.push(now);
    if tr.timestamps.len() >= need {
        tr.timestamps.clear();
        true
    } else {
        false
    }
}

#[cfg(target_arch = "x86")]
unsafe fn vo_type_name(kind: i32) -> &'static str {
    if !(0..VO_TYPE_COUNT).contains(&kind) {
        return "?";
    }
    let ptr = unsafe { *((VO_TYPE_TABLE + (kind as usize) * 24) as *const *const u8) };
    if (ptr as usize) < 0x0040_0000 || (ptr as usize) > 0x01D2_0000 {
        return "?";
    }
    let mut len = 0usize;
    while len < 64 && unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(slice).unwrap_or("?")
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_match(
    db: *mut c_void,
    edx: *mut c_void,
    kind: i32,
    params: *mut c_void,
    rows: *mut c_void,
    max: i32,
) -> i32 {
    let n = if let Some(&orig) = ORIG_MATCH.get() {
        unsafe { orig(db, edx, kind, params, rows, max) }
    } else {
        0
    };
    LAST_MATCHED.with(|c| c.set(n));
    n
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_raise(
    vo: *mut c_void,
    edx: *mut c_void,
    kind: i32,
    inflictor: *mut c_void,
    victim: *mut c_void,
    pos: *const f32,
    mode: i32,
) {
    LAST_MATCHED.with(|c| c.set(-1));
    if let Some(&orig) = ORIG_RAISE.get() {
        unsafe { orig(vo, edx, kind, inflictor, victim, pos, mode) };
        let client_vo = unsafe { *(CLIENT_VO_GLOBAL as *const *mut c_void) };
        if kind == ET_SOLDIER_KILLED
            && vo == client_vo
            && !inflictor.is_null()
            && !pos.is_null()
            && record_multikill(inflictor as usize)
        {
            unsafe { orig(vo, edx, ET_MULTIKILL_CARRIER, inflictor, victim, pos, mode) };
            crate::log_line(&format!(
                "vo: multikill call-out raised ({} kills in {} ms)",
                MULTI_COUNT.load(Ordering::Relaxed),
                MULTI_WINDOW_MS.load(Ordering::Relaxed)
            ));
        }
    }
    if PROBE_ENABLED.load(Ordering::Relaxed)
        && PROBE_LINES.fetch_add(1, Ordering::Relaxed) < MAX_PROBE_LINES
    {
        let srv_vo = unsafe { *(SERVER_VO_GLOBAL as *const *mut c_void) };
        let cli_vo = unsafe { *(CLIENT_VO_GLOBAL as *const *mut c_void) };
        let side = if vo == srv_vo {
            "server"
        } else if vo == cli_vo {
            "client"
        } else {
            "other"
        };
        let matched = LAST_MATCHED.with(std::cell::Cell::get);
        let name = unsafe { vo_type_name(kind) };
        crate::log_line(&format!("vo_probe: {side} {name} ({kind}) -> {matched} row(s) matched"));
    }
}

/// Install the VO multikill carrier and optional `[debug] vo_probe = true` logger.
pub unsafe fn install(opfor_root: &Path) -> usize {
    let mut probe = false;
    let mut multi_cnt = 3u32;
    let mut multi_ms = 5000u32;
    if let Ok(txt) = fs::read_to_string(opfor_root.join("config").join("opfor.ini")) {
        let mut sec = String::new();
        for raw in txt.lines() {
            let line = raw.trim();
            if line.starts_with('[') && line.ends_with(']') {
                sec = line[1..line.len() - 1].trim().to_ascii_lowercase();
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().trim_matches('"');
            if sec == "debug" && key == "vo_probe" {
                probe = matches!(val.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on");
            } else if sec == "vo" && key == "multikill_count" {
                multi_cnt = val.parse::<u32>().unwrap_or(3).clamp(2, 8);
            } else if sec == "vo" && key == "multikill_window_ms" {
                multi_ms = val.parse::<u32>().unwrap_or(5000).clamp(1000, 30_000);
            }
        }
    }
    PROBE_ENABLED.store(probe, Ordering::Relaxed);
    MULTI_COUNT.store(multi_cnt, Ordering::Relaxed);
    MULTI_WINDOW_MS.store(multi_ms, Ordering::Relaxed);

    let mut installed = 0usize;
    #[cfg(target_arch = "x86")]
    {
        if probe
            && let Some(tramp) = unsafe {
                crate::hooks::install_detour(VO_MATCH, &VO_MATCH_PROLOGUE, detour_match as *const c_void)
            }
        {
            let _ = ORIG_MATCH.set(unsafe { std::mem::transmute::<*const c_void, MatchFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(VO_RAISE, &VO_RAISE_PROLOGUE, detour_raise as *const c_void)
        } {
            let _ = ORIG_RAISE.set(unsafe { std::mem::transmute::<*const c_void, RaiseFn>(tramp) });
            installed += 1;
        }
    }
    installed
}
