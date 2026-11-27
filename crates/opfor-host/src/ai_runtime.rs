//! Pure-Rust AI Runtime Hooks (`force_ai`, `sector_offsets`, `unit_counters`) for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/{forceai,sectoroffsets,unitcounters}.cpp`:
//! - `[dev] force_ai = true`: Forces `LevelData.HasAI` (`+0x3AE`) to `1` at `0x00575046`
//!   (and confirms at `0x005958F0`) so stock multiplayer levels initialize AI & Kynapse pathdata.
//! - `[dev] sector_offsets = X,Z`: Overrides zero `SectorOffsetX`/`SectorOffsetZ` values at
//!   `0x00F20B48` and `0x00F20BB0` when a level lacks its `SectorOffsets` DBX asset.
//! - `[dev] unit_counters_dump = true` (interval `[dev] unit_counters_interval_s = 60`):
//!   Tracks per-pathfinder go-to / resolve / stage-2 / state-7 metrics in a deterministic `BTreeMap`.

#![allow(dead_code, non_snake_case)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

// `force_ai` hook targets (BFBC2 build 795745)
const HAS_AI_TEST: usize = 0x0057_5046;
const HAS_AI_TEST_PROLOGUE: [u8; 7] = [0x80, 0xB9, 0xAE, 0x03, 0x00, 0x00, 0x00];
const SET_AI_ACTIVE: usize = 0x0059_58F0;
const SET_AI_ACTIVE_PROLOGUE: [u8; 5] = [0xB8, 0x01, 0x00, 0x00, 0x00];
const HAS_AI_OFFSET: usize = 0x3AE;

// `sector_offsets` hook targets
const TAKE_SECTOR_X: usize = 0x00F2_0B48;
const TAKE_SECTOR_X_PROLOGUE: [u8; 8] = [0x89, 0x44, 0x24, 0x6C, 0x89, 0x5C, 0x24, 0x7C];
const TAKE_SECTOR_Z: usize = 0x00F2_0BB0;
const TAKE_SECTOR_Z_PROLOGUE: [u8; 11] = [0x89, 0x44, 0x24, 0x6C, 0x89, 0x9C, 0x24, 0x90, 0x00, 0x00, 0x00];

// `unit_counters` Kynapse pathfinder targets
const PF_GOTO: usize = 0x0107_FE80;
const PF_GOTO_PROLOGUE: [u8; 6] = [0x53, 0x56, 0x8B, 0xF1, 0x33, 0xDB];
const PF_GOTO_RESULT: usize = 0x010B_1C07;
const PF_GOTO_RESULT_PROLOGUE: [u8; 8] = [0x3A, 0xC3, 0x88, 0x9E, 0xBA, 0x03, 0x00, 0x00];
const PF_COMPUTE_FALSE: usize = 0x0107_F8D8;
const PF_COMPUTE_FALSE_PROLOGUE: [u8; 5] = [0x5F, 0x5E, 0x5D, 0x32, 0xC0];
const PF_RESOLVE_FALSE: usize = 0x0107_F98D;
const PF_RESOLVE_FALSE_PROLOGUE: [u8; 6] = [0x8B, 0x46, 0x04, 0x8B, 0x48, 0x14];
const PF_START_NOT_IN: usize = 0x0107_FAAB;
const PF_START_NOT_IN_PROLOGUE: [u8; 11] = [0x5F, 0xC7, 0x86, 0x10, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
const PF_DEST_NOT_IN: usize = 0x0107_FAFA;
const PF_DEST_NOT_IN_PROLOGUE: [u8; 11] = [0x5F, 0xC7, 0x86, 0x10, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00];
const PF_EQUAL_A: usize = 0x0107_FC0E;
const PF_EQUAL_A_PROLOGUE: [u8; 10] = [0xC7, 0x86, 0x10, 0x01, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00];
const PF_EQUAL_B: usize = 0x0107_FC8F;
const PF_EQUAL_B_PROLOGUE: [u8; 6] = [0x8B, 0xB6, 0x18, 0x01, 0x00, 0x00];
const PF_STAGE2: usize = 0x0107_FD0F;
const PF_STAGE2_PROLOGUE: [u8; 8] = [0x8B, 0xF8, 0x8B, 0x86, 0x0C, 0x01, 0x00, 0x00];

static mut TRAMP_HAS_AI: usize = 0;
static mut TRAMP_SET_AI: usize = 0;
static mut TRAMP_SEC_X: usize = 0;
static mut TRAMP_SEC_Z: usize = 0;

static mut TRAMP_PF_GOTO: usize = 0;
static mut TRAMP_PF_RESULT: usize = 0;
static mut TRAMP_PF_COMPUTE_FALSE: usize = 0;
static mut TRAMP_PF_RESOLVE_FALSE: usize = 0;
static mut TRAMP_PF_START_NOT_IN: usize = 0;
static mut TRAMP_PF_DEST_NOT_IN: usize = 0;
static mut TRAMP_PF_EQUAL_A: usize = 0;
static mut TRAMP_PF_EQUAL_B: usize = 0;
static mut TRAMP_PF_STAGE2: usize = 0;

static SECTOR_OVERRIDE_X: AtomicI32 = AtomicI32::new(0);
static SECTOR_OVERRIDE_Z: AtomicI32 = AtomicI32::new(0);
static LAST_SEC_X_OVERRIDDEN: AtomicBool = AtomicBool::new(false);
static LAST_SEC_X_OWN: AtomicI32 = AtomicI32::new(0);
static UNIT_DUMP_ENABLED: AtomicBool = AtomicBool::new(false);
static UNIT_START_TIME: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Clone, Default)]
struct UnitStats {
    requests: u32,
    ok: u32,
    fail: u32,
    resolve1: u32,
    resolve2: u32,
    resolve_other: u32,
    stage2_c2: u32,
    stage2_c4: u32,
    state7: u32,
    equal_ids: u32,
    compute_false: u32,
    first_start: [f32; 2],
    last_start: [f32; 2],
    first_target: [f32; 2],
    last_target: [f32; 2],
    walked_m: f64,
}

static UNITS: Mutex<BTreeMap<u32, UnitStats>> = Mutex::new(BTreeMap::new());

struct DevConfig {
    force_ai: bool,
    sector_offsets: Option<(i32, i32)>,
    unit_counters_dump: bool,
    unit_counters_interval_s: u64,
}

fn parse_dev_config(path: &Path) -> DevConfig {
    let mut cfg = DevConfig {
        force_ai: false,
        sector_offsets: None,
        unit_counters_dump: false,
        unit_counters_interval_s: 60,
    };
    let Ok(txt) = fs::read_to_string(path) else {
        return cfg;
    };
    let mut sec = String::new();
    for raw in txt.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            sec = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if sec != "dev" {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim().trim_matches('"');
        let truthy = matches!(val.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on");
        match key.as_str() {
            "force_ai" => cfg.force_ai = truthy,
            "sector_offsets" => {
                if let Some((xs, zs)) = val.split_once(',')
                    && let (Ok(x), Ok(z)) = (xs.trim().parse::<i32>(), zs.trim().parse::<i32>())
                    && (x != 0 || z != 0)
                {
                    cfg.sector_offsets = Some((x, z));
                }
            }
            "unit_counters_dump" => cfg.unit_counters_dump = truthy,
            "unit_counters_interval_s" => {
                if let Ok(s) = val.parse::<u64>() {
                    cfg.unit_counters_interval_s = s.max(5);
                }
            }
            _ => {}
        }
    }
    cfg
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn on_force_ai(level_ptr: usize, at_test: u32) {
    if level_ptr < 0x10000 || level_ptr > 0x7FFF_0000 {
        return;
    }
    let flag = (level_ptr + HAS_AI_OFFSET) as *mut u8;
    let was = unsafe { *flag };
    unsafe { *flag = 1 };
    crate::log_line(&format!(
        "force_ai: level 0x{level_ptr:08X} +0x3AE = {was} -> 1 ({})",
        if at_test != 0 { "HasAI test" } else { "AI set active" }
    ));
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn force_ai_test_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 1",
        "push ecx",
        "call {cb}",
        "add esp, 8",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_force_ai,
        tramp = sym TRAMP_HAS_AI,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn force_ai_set_active_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "mov eax, [esp + 40]", // original [esp+4] = arg1 (level data)
        "push 0",
        "push eax",
        "call {cb}",
        "add esp, 8",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_force_ai,
        tramp = sym TRAMP_SET_AI,
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn on_sector_offset(obj_ptr: usize, cur_val: i32, is_x: u32) -> i32 {
    if obj_ptr < 0x10000 || obj_ptr > 0x7FFF_0000 {
        return cur_val;
    }
    let off = if is_x != 0 { 0x1C } else { 0x18 };
    let own = unsafe { *((obj_ptr + off) as *const i32) };
    if is_x != 0 {
        LAST_SEC_X_OWN.store(own, Ordering::Relaxed);
        LAST_SEC_X_OVERRIDDEN.store(own == 0, Ordering::Relaxed);
        if own == 0 {
            SECTOR_OVERRIDE_X.load(Ordering::Relaxed)
        } else {
            cur_val
        }
    } else {
        let x_done = LAST_SEC_X_OVERRIDDEN.load(Ordering::Relaxed);
        let x_own = LAST_SEC_X_OWN.load(Ordering::Relaxed);
        let gx = SECTOR_OVERRIDE_X.load(Ordering::Relaxed);
        let gz = SECTOR_OVERRIDE_Z.load(Ordering::Relaxed);
        if x_done && own == 0 {
            crate::log_line(&format!("sector_offsets: override X={gx} Z={gz} (asset was 0,0)"));
        } else {
            crate::log_line(&format!(
                "sector_offsets: asset non-zero (X={x_own} Z={own}), {}",
                if x_done || own == 0 { "only zero value overridden" } else { "kept asset" }
            ));
        }
        if own == 0 { gz } else { cur_val }
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sector_x_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 1",
        "push eax",
        "push edx",
        "call {cb}",
        "add esp, 12",
        "mov [esp + 32], eax", // overwrite saved EAX in pushad frame
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_sector_offset,
        tramp = sym TRAMP_SEC_X,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sector_z_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push eax",
        "push edx",
        "call {cb}",
        "add esp, 12",
        "mov [esp + 32], eax",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_sector_offset,
        tramp = sym TRAMP_SEC_Z,
    );
}

#[cfg(target_arch = "x86")]
fn read_xz(ptr: usize) -> Option<[f32; 2]> {
    if ptr < 0x10000 || ptr > 0x7FFF_0000 {
        return None;
    }
    let x = unsafe { *(ptr as *const f32) };
    let z = unsafe { *((ptr + 8) as *const f32) };
    (x.is_finite() && z.is_finite()).then_some([x, z])
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn on_pf_event(kind: u32, pf: u32, aux1: u32, aux2: u32) {
    let Ok(mut map) = UNITS.lock() else {
        return;
    };
    let u = if kind <= 1 {
        map.entry(pf).or_default()
    } else if let Some(u) = map.get_mut(&pf) {
        u
    } else {
        return;
    };
    match kind {
        0 => {
            if let Some(st) = read_xz(aux1 as usize) {
                if u.requests == 0 {
                    u.first_start = st;
                } else {
                    u.walked_m += f64::from((st[0] - u.last_start[0]).hypot(st[1] - u.last_start[1]));
                }
                u.last_start = st;
            }
            if aux2 != 0
                && let Some(tg) = read_xz((aux2 as usize).saturating_add(4))
            {
                if u.requests == 0 {
                    u.first_target = tg;
                }
                u.last_target = tg;
            }
            u.requests += 1;
        }
        1 => {
            if (aux1 & 0xFF) != 0 {
                u.ok += 1;
            } else {
                u.fail += 1;
            }
        }
        2 => u.compute_false += 1,
        3 => {
            let code = if (pf as usize) > 0x10000 && (pf as usize) < 0x7FFF_0000 {
                unsafe { *((pf as usize + 0x110) as *const u32) }
            } else {
                0
            };
            match code {
                1 => u.resolve1 += 1,
                2 => u.resolve2 += 1,
                _ => u.resolve_other += 1,
            }
        }
        4 => u.state7 += 1,
        5 => u.equal_ids += 1,
        6 => {
            if aux1 == 2 {
                u.stage2_c2 += 1;
            } else if aux1 == 4 {
                u.stage2_c4 += 1;
            }
        }
        _ => {}
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_goto_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "mov eax, [esp + 44]", // arg2
        "mov edx, [esp + 40]", // arg1
        "push eax",
        "push edx",
        "push ecx",
        "push 0",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_GOTO,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_result_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push eax",
        "push esi",
        "push 1",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_RESULT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_compute_false_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push 0",
        "push esi",
        "push 2",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_COMPUTE_FALSE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_resolve_false_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push 0",
        "push esi",
        "push 3",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_RESOLVE_FALSE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_start_not_in_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push 0",
        "push esi",
        "push 4",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_START_NOT_IN,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_dest_not_in_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push 0",
        "push esi",
        "push 4",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_DEST_NOT_IN,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_equal_a_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push 0",
        "push esi",
        "push 5",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_EQUAL_A,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_equal_b_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push 0",
        "push esi",
        "push 5",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_EQUAL_B,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pf_stage2_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0",
        "push eax",
        "push esi",
        "push 6",
        "call {cb}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{tramp}]",
        cb = sym on_pf_event,
        tramp = sym TRAMP_PF_STAGE2,
    );
}

/// Dump accumulated per-pathfinder AI telemetry to the host log.
pub fn dump_unit_counters(reason: &str) {
    if !UNIT_DUMP_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut rows: Vec<(u32, UnitStats)> = UNITS
        .lock()
        .ok()
        .map(|m| m.iter().map(|(&k, v)| (k, v.clone())).collect())
        .unwrap_or_default();
    rows.sort_by(|a, b| b.1.requests.cmp(&a.1.requests));
    let elapsed_s = UNIT_START_TIME
        .lock()
        .ok()
        .and_then(|g| g.map(|t| t.elapsed().as_secs()))
        .unwrap_or(0);
    crate::log_line(&format!(
        "unit_counters ({reason}, +{elapsed_s}s): {} pathfinders",
        rows.len()
    ));
    for (pf, u) in rows.iter().take(64) {
        let dist = (u.last_start[0] - u.last_target[0]).hypot(u.last_start[1] - u.last_target[1]);
        crate::log_line(&format!(
            "  unit 0x{pf:08X}: req={} ok={} fail={} | res=({},{},{}) stg2=({},{}) st7={} eq={} cmp={} | walked={:.1}m rem={:.1}m",
            u.requests, u.ok, u.fail, u.resolve1, u.resolve2, u.resolve_other,
            u.stage2_c2, u.stage2_c4, u.state7, u.equal_ids, u.compute_false,
            u.walked_m, dist
        ));
    }
}

/// Install configured `[dev]` AI runtime hooks (`force_ai`, `sector_offsets`, `unit_counters`).
pub unsafe fn install(opfor_root: &Path) -> usize {
    let cfg = parse_dev_config(&opfor_root.join("config").join("opfor.ini"));
    let mut installed = 0usize;

    #[cfg(target_arch = "x86")]
    {
        if cfg.force_ai {
            if let Some(tramp) = unsafe {
                crate::hooks::install_detour(HAS_AI_TEST, &HAS_AI_TEST_PROLOGUE, force_ai_test_entry as *const c_void)
            } {
                unsafe { TRAMP_HAS_AI = tramp as usize };
                installed += 1;
            }
            if let Some(tramp) = unsafe {
                crate::hooks::install_detour(
                    SET_AI_ACTIVE,
                    &SET_AI_ACTIVE_PROLOGUE,
                    force_ai_set_active_entry as *const c_void,
                )
            } {
                unsafe { TRAMP_SET_AI = tramp as usize };
                installed += 1;
            }
            crate::log_line("ai_runtime: force_ai enabled (LevelData.HasAI forced to 1)");
        }

        if let Some((gx, gz)) = cfg.sector_offsets {
            SECTOR_OVERRIDE_X.store(gx, Ordering::Relaxed);
            SECTOR_OVERRIDE_Z.store(gz, Ordering::Relaxed);
            if let Some(tramp) = unsafe {
                crate::hooks::install_detour(TAKE_SECTOR_X, &TAKE_SECTOR_X_PROLOGUE, sector_x_entry as *const c_void)
            } {
                unsafe { TRAMP_SEC_X = tramp as usize };
                installed += 1;
            }
            if let Some(tramp) = unsafe {
                crate::hooks::install_detour(TAKE_SECTOR_Z, &TAKE_SECTOR_Z_PROLOGUE, sector_z_entry as *const c_void)
            } {
                unsafe { TRAMP_SEC_Z = tramp as usize };
                installed += 1;
            }
            crate::log_line(&format!("ai_runtime: sector_offsets override active (X={gx}, Z={gz})"));
        }

        if cfg.unit_counters_dump {
            UNIT_DUMP_ENABLED.store(true, Ordering::Relaxed);
            if let Ok(mut g) = UNIT_START_TIME.lock() {
                *g = Some(Instant::now());
            }
            let hooks: [(usize, &[u8], *const c_void, *mut usize); 9] = [
                (PF_GOTO, &PF_GOTO_PROLOGUE, pf_goto_entry as *const c_void, &raw mut TRAMP_PF_GOTO),
                (PF_GOTO_RESULT, &PF_GOTO_RESULT_PROLOGUE, pf_result_entry as *const c_void, &raw mut TRAMP_PF_RESULT),
                (PF_COMPUTE_FALSE, &PF_COMPUTE_FALSE_PROLOGUE, pf_compute_false_entry as *const c_void, &raw mut TRAMP_PF_COMPUTE_FALSE),
                (PF_RESOLVE_FALSE, &PF_RESOLVE_FALSE_PROLOGUE, pf_resolve_false_entry as *const c_void, &raw mut TRAMP_PF_RESOLVE_FALSE),
                (PF_START_NOT_IN, &PF_START_NOT_IN_PROLOGUE, pf_start_not_in_entry as *const c_void, &raw mut TRAMP_PF_START_NOT_IN),
                (PF_DEST_NOT_IN, &PF_DEST_NOT_IN_PROLOGUE, pf_dest_not_in_entry as *const c_void, &raw mut TRAMP_PF_DEST_NOT_IN),
                (PF_EQUAL_A, &PF_EQUAL_A_PROLOGUE, pf_equal_a_entry as *const c_void, &raw mut TRAMP_PF_EQUAL_A),
                (PF_EQUAL_B, &PF_EQUAL_B_PROLOGUE, pf_equal_b_entry as *const c_void, &raw mut TRAMP_PF_EQUAL_B),
                (PF_STAGE2, &PF_STAGE2_PROLOGUE, pf_stage2_entry as *const c_void, &raw mut TRAMP_PF_STAGE2),
            ];
            for (addr, pro, dst, slot) in hooks {
                if let Some(tramp) = unsafe { crate::hooks::install_detour(addr, pro, dst) } {
                    unsafe { *slot = tramp as usize };
                    installed += 1;
                }
            }
            let interval = cfg.unit_counters_interval_s;
            let _ = std::thread::Builder::new()
                .name("opfor-unit-counters".into())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(interval));
                    dump_unit_counters("interval");
                });
            crate::log_line(&format!("ai_runtime: unit_counters active (interval={interval}s)"));
        }
    }
    let _ = &cfg;
    installed
}
