//! Console `HaloAimerData` aim-assist & stick-response tuning while a controller is active.
//!
//! Ported from `project-opfor-bfbc2/src/dll/padaimer.cpp`:
//! - Loads user-imported console aimer parameters from `config/opfor_console_aimer.ini`
//!   (or `engine/opfor_console_aimer.ini`). Nothing copyrighted is shipped.
//! - Background scanner locates live PC `HaloAimerData` instances in committed `PAGE_READWRITE`
//!   private heap pages (matching PC `YawSpeed` 50.0 / 105.0, vtable in `0x00401000..0x01C50000`,
//!   and PC acceleration fields).
//! - Swaps to console stick curves, speeds, acceleration, and `Suck*` aim assist while a gamepad
//!   is active, and restores exact saved PC values when keyboard/mouse takes over.

#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const MAX_AIMERS: usize = 8;
const MAX_FOUND: usize = 32;

const FLOAT_FIELDS: &[(&str, usize)] = &[
    ("AccelerationInputThreshold", 0x6C),
    ("AccelerationMultiplier", 0x70),
    ("AccelerationDamping", 0xA4),
    ("SuckSoftZone", 0xB0),
    ("SuckUserInputMultiplier", 0xA8),
    ("SuckOwnSpeedInfluence", 0xAC),
    ("SuckTargetSpeedInfluence", 0x58),
    ("SuckOwnRequiredMovementForMaximumSuck", 0x54),
    ("SuckStartInputThreshold", 0x50),
    ("SuckZoomingMultiplier", 0x5C),
    ("SuckZoomingPostTime", 0x68),
    ("SuckYaw", 0x64),
    ("SuckPitch", 0x60),
];
const NUM_FLOATS: usize = 13;
const OFF_YAW: usize = 0xB4;
const OFF_PITCH: usize = 0xB8;
const OFF_USE_PITCH_ACCEL: usize = 0xBC;
const OFF_USE_YAW_ACCEL: usize = 0xBD;
const OFF_POLY: usize = 0x84;

const MEM_COMMIT: u32 = 0x1000;
const MEM_PRIVATE: u32 = 0x20000;
const PAGE_READONLY: u32 = 0x02;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_WRITECOPY: u32 = 0x08;
const PAGE_GUARD: u32 = 0x100;

#[repr(C)]
struct MemoryBasicInformation {
    base_address: *mut c_void,
    allocation_base: *mut c_void,
    allocation_protect: u32,
    region_size: usize,
    state: u32,
    protect: u32,
    type_: u32,
}

#[cfg(target_arch = "x86")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn VirtualQuery(
        lp_address: *const c_void,
        lp_buffer: *mut MemoryBasicInformation,
        dw_length: usize,
    ) -> usize;
}

#[derive(Clone, Default)]
struct AimerSpec {
    pc_yaw: f32,
    pc_pitch: f32,
    pc_poly: [f32; 8],
    yaw: f32,
    pitch: f32,
    fields: [f32; NUM_FLOATS],
    poly: [f32; 8],
    use_yaw_accel: bool,
    use_pitch_accel: bool,
}

#[derive(Clone, Copy)]
struct FoundInstance {
    ptr: usize,
    aimer_idx: usize,
    poly_ptr: usize,
    saved_fields: [f32; NUM_FLOATS],
    saved_yaw: f32,
    saved_pitch: f32,
    saved_poly: [f32; 8],
    saved_use: [u8; 2],
}

struct PadAimerState {
    aimers: Vec<AimerSpec>,
    found: Vec<FoundInstance>,
    console_on: bool,
    yaw_scale: f32,
    pitch_scale: f32,
}

static STATE: Mutex<Option<PadAimerState>> = Mutex::new(None);
static RESCAN: AtomicBool = AtomicBool::new(true);

fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn parse_float_array(s: &str) -> [f32; 8] {
    let mut out = [0.0f32; 8];
    for (dst, tok) in out.iter_mut().zip(s.split('/')) {
        *dst = tok.trim().parse::<f32>().unwrap_or(0.0);
    }
    out
}

fn load_console_aimer_ini(path: &Path) -> Vec<AimerSpec> {
    let Ok(txt) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut section = String::new();
    let mut count = 0usize;
    let mut map: std::collections::BTreeMap<usize, AimerSpec> = std::collections::BTreeMap::new();

    for raw in txt.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();
        if section == "aimers" && key.eq_ignore_ascii_case("count") {
            count = val.parse::<usize>().unwrap_or(0).min(MAX_AIMERS);
            continue;
        }
        if let Some(idx_s) = section.strip_prefix("aimer")
            && let Ok(idx) = idx_s.parse::<usize>()
            && idx < MAX_AIMERS
        {
            let entry = map.entry(idx).or_insert_with(|| AimerSpec {
                yaw: 3.5,
                pitch: 2.5,
                ..Default::default()
            });
            match key {
                "pc_YawSpeed" => entry.pc_yaw = val.parse().unwrap_or(0.0),
                "pc_PitchSpeed" => entry.pc_pitch = val.parse().unwrap_or(0.0),
                "pc_InputPolynomial" => entry.pc_poly = parse_float_array(val),
                "YawSpeed" => entry.yaw = val.parse().unwrap_or(3.5),
                "PitchSpeed" => entry.pitch = val.parse().unwrap_or(2.5),
                "InputPolynomial" => entry.poly = parse_float_array(val),
                "UseYawAcceleration" => entry.use_yaw_accel = val.eq_ignore_ascii_case("true"),
                "UsePitchAcceleration" => entry.use_pitch_accel = val.eq_ignore_ascii_case("true"),
                other => {
                    for (fi, &(fname, _)) in FLOAT_FIELDS.iter().enumerate() {
                        if other.eq_ignore_ascii_case(fname) {
                            entry.fields[fi] = val.parse().unwrap_or(0.0);
                            break;
                        }
                    }
                }
            }
        }
    }

    if count == 0 {
        return Vec::new();
    }
    (0..count).filter_map(|i| map.remove(&i)).collect()
}

#[cfg(target_arch = "x86")]
fn readable_range(addr: usize, len: usize) -> bool {
    if addr < 0x10000 || addr.checked_add(len).is_none() {
        return false;
    }
    let mut mi: MemoryBasicInformation = unsafe { std::mem::zeroed() };
    let ok = unsafe { VirtualQuery(addr as *const c_void, &mut mi, std::mem::size_of::<MemoryBasicInformation>()) };
    if ok == 0 || mi.state != MEM_COMMIT {
        return false;
    }
    if (mi.protect & PAGE_GUARD) != 0 || (mi.protect & (PAGE_READWRITE | PAGE_READONLY | PAGE_WRITECOPY)) == 0 {
        return false;
    }
    let lo = mi.base_address as usize;
    addr + len <= lo.saturating_add(mi.region_size)
}

#[cfg(target_arch = "x86")]
fn writable_range(addr: usize, len: usize) -> bool {
    if addr < 0x10000 || addr.checked_add(len).is_none() {
        return false;
    }
    let mut mi: MemoryBasicInformation = unsafe { std::mem::zeroed() };
    let ok = unsafe { VirtualQuery(addr as *const c_void, &mut mi, std::mem::size_of::<MemoryBasicInformation>()) };
    if ok == 0 || mi.state != MEM_COMMIT {
        return false;
    }
    if (mi.protect & PAGE_GUARD) != 0 || (mi.protect & PAGE_READWRITE) == 0 {
        return false;
    }
    let lo = mi.base_address as usize;
    addr + len <= lo.saturating_add(mi.region_size)
}

#[cfg(target_arch = "x86")]
unsafe fn find_array_data(header_addr: usize, want: &[f32; 8]) -> usize {
    if !readable_range(header_addr, 4) {
        return 0;
    }
    let base = unsafe { *(header_addr as *const usize) };
    for skip in [0usize, 4, 8] {
        let cand = base.saturating_add(skip);
        if !writable_range(cand, 32) {
            continue;
        }
        let mut ok = true;
        for (i, &w) in want.iter().enumerate() {
            let v = unsafe { *((cand + i * 4) as *const f32) };
            if !same(v, w) {
                ok = false;
                break;
            }
        }
        if ok {
            return cand;
        }
    }
    0
}

#[cfg(target_arch = "x86")]
unsafe fn match_pc_aimer(base: usize, aimers: &[AimerSpec], preferred: usize) -> Option<usize> {
    if !writable_range(base, 0xC0) {
        return None;
    }
    let vtable = unsafe { *(base as *const usize) };
    if !(0x0040_1000..0x01C5_0000).contains(&vtable) {
        return None;
    }
    let acc_th = unsafe { *((base + 0x6C) as *const f32) };
    let acc_mul = unsafe { *((base + 0x70) as *const f32) };
    let acc_dmp = unsafe { *((base + 0xA4) as *const f32) };
    if !same(acc_th, 1.0) || !same(acc_mul, 0.0) || !same(acc_dmp, 0.0) {
        return None;
    }
    let use_p = unsafe { *((base + OFF_USE_PITCH_ACCEL) as *const u8) };
    let use_y = unsafe { *((base + OFF_USE_YAW_ACCEL) as *const u8) };
    if use_p > 1 || use_y > 1 {
        return None;
    }
    let yaw = unsafe { *((base + OFF_YAW) as *const f32) };
    let pitch = unsafe { *((base + OFF_PITCH) as *const f32) };
    let mut best = None;
    for (i, a) in aimers.iter().enumerate() {
        if same(yaw, a.pc_yaw) && same(pitch, a.pc_pitch) && (best.is_none() || i == preferred) {
            best = Some(i);
        }
    }
    best
}

#[cfg(target_arch = "x86")]
unsafe fn scan_heap(aimers: &[AimerSpec]) -> Vec<FoundInstance> {
    let mut found = Vec::new();
    let yaw_bits = [50.0f32.to_bits(), 105.0f32.to_bits()];
    let mut addr = 0x10000usize;
    let mut mi: MemoryBasicInformation = unsafe { std::mem::zeroed() };
    while found.len() < MAX_FOUND && addr < 0x7FFF_0000 {
        let ok = unsafe { VirtualQuery(addr as *const c_void, &mut mi, std::mem::size_of::<MemoryBasicInformation>()) };
        if ok == 0 || mi.region_size == 0 {
            break;
        }
        let lo = mi.base_address as usize;
        let next = lo.saturating_add(mi.region_size);
        if mi.state == MEM_COMMIT && mi.type_ == MEM_PRIVATE && mi.protect == PAGE_READWRITE {
            let mut q = lo.saturating_add(OFF_YAW);
            while q.saturating_add(16) <= next && found.len() < MAX_FOUND {
                let bits = unsafe { *(q as *const u32) };
                if bits == yaw_bits[0] || bits == yaw_bits[1] {
                    let base = q - OFF_YAW;
                    let pref = found.len() % aimers.len().max(1);
                    if let Some(ai) = unsafe { match_pc_aimer(base, aimers, pref) } {
                        let poly_ptr = unsafe { find_array_data(base + OFF_POLY, &aimers[ai].pc_poly) };
                        found.push(FoundInstance {
                            ptr: base,
                            aimer_idx: ai,
                            poly_ptr,
                            saved_fields: [0.0; NUM_FLOATS],
                            saved_yaw: 0.0,
                            saved_pitch: 0.0,
                            saved_poly: [0.0; 8],
                            saved_use: [0; 2],
                        });
                    }
                }
                q += 4;
            }
        }
        if next <= addr {
            break;
        }
        addr = next;
    }
    if found.len() == aimers.len() {
        found.sort_by_key(|f| f.ptr);
        if found
            .iter()
            .enumerate()
            .all(|(i, f)| unsafe { match_pc_aimer(f.ptr, aimers, i) } == Some(i))
        {
            for (i, f) in found.iter_mut().enumerate() {
                f.aimer_idx = i;
            }
        }
    }
    found
}

#[cfg(target_arch = "x86")]
unsafe fn holds_expected(fd: &FoundInstance, a: &AimerSpec, ours: bool, yaw_s: f32, pitch_s: f32) -> bool {
    if !writable_range(fd.ptr, 0xC0) {
        return false;
    }
    let yaw = unsafe { *((fd.ptr + OFF_YAW) as *const f32) };
    let pitch = unsafe { *((fd.ptr + OFF_PITCH) as *const f32) };
    if ours {
        same(yaw, a.yaw * yaw_s) && same(pitch, a.pitch * pitch_s)
    } else {
        same(yaw, a.pc_yaw) && same(pitch, a.pc_pitch)
    }
}

#[cfg(target_arch = "x86")]
unsafe fn write_tuning(st: &mut PadAimerState, console: bool, old_yaw: f32, old_pitch: f32) {
    for fd in &mut st.found {
        let a = &st.aimers[fd.aimer_idx];
        if !unsafe { holds_expected(fd, a, st.console_on, old_yaw, old_pitch) } {
            RESCAN.store(true, Ordering::Release);
            continue;
        }
        if console && !st.console_on {
            for (k, &(_, off)) in FLOAT_FIELDS.iter().enumerate() {
                fd.saved_fields[k] = unsafe { *((fd.ptr + off) as *const f32) };
            }
            fd.saved_yaw = unsafe { *((fd.ptr + OFF_YAW) as *const f32) };
            fd.saved_pitch = unsafe { *((fd.ptr + OFF_PITCH) as *const f32) };
            fd.saved_use[0] = unsafe { *((fd.ptr + OFF_USE_YAW_ACCEL) as *const u8) };
            fd.saved_use[1] = unsafe { *((fd.ptr + OFF_USE_PITCH_ACCEL) as *const u8) };
            if fd.poly_ptr != 0 && writable_range(fd.poly_ptr, 32) {
                for (i, dst) in fd.saved_poly.iter_mut().enumerate() {
                    *dst = unsafe { *((fd.poly_ptr + i * 4) as *const f32) };
                }
            }
        }
        if console {
            for (k, &(_, off)) in FLOAT_FIELDS.iter().enumerate() {
                unsafe { *((fd.ptr + off) as *mut f32) = a.fields[k] };
            }
            unsafe {
                *((fd.ptr + OFF_YAW) as *mut f32) = a.yaw * st.yaw_scale;
                *((fd.ptr + OFF_PITCH) as *mut f32) = a.pitch * st.pitch_scale;
                *((fd.ptr + OFF_USE_YAW_ACCEL) as *mut u8) = u8::from(a.use_yaw_accel);
                *((fd.ptr + OFF_USE_PITCH_ACCEL) as *mut u8) = u8::from(a.use_pitch_accel);
            }
            if fd.poly_ptr != 0 && writable_range(fd.poly_ptr, 32) {
                for (i, &v) in a.poly.iter().enumerate() {
                    unsafe { *((fd.poly_ptr + i * 4) as *mut f32) = v };
                }
            }
        } else if st.console_on {
            for (k, &(_, off)) in FLOAT_FIELDS.iter().enumerate() {
                unsafe { *((fd.ptr + off) as *mut f32) = fd.saved_fields[k] };
            }
            unsafe {
                *((fd.ptr + OFF_YAW) as *mut f32) = fd.saved_yaw;
                *((fd.ptr + OFF_PITCH) as *mut f32) = fd.saved_pitch;
                *((fd.ptr + OFF_USE_YAW_ACCEL) as *mut u8) = fd.saved_use[0];
                *((fd.ptr + OFF_USE_PITCH_ACCEL) as *mut u8) = fd.saved_use[1];
            }
            if fd.poly_ptr != 0 && writable_range(fd.poly_ptr, 32) {
                for (i, &v) in fd.saved_poly.iter().enumerate() {
                    unsafe { *((fd.poly_ptr + i * 4) as *mut f32) = v };
                }
            }
        }
    }
}

/// Initialize the console aimer tuning scanner from `config/opfor_console_aimer.ini` (or `engine/`).
pub fn install(opfor_root: &Path) -> bool {
    let cand1 = opfor_root.join("config").join("opfor_console_aimer.ini");
    let cand2 = opfor_root.join("engine").join("opfor_console_aimer.ini");
    let ini_path = if cand1.is_file() { cand1 } else { cand2 };
    let aimers = load_console_aimer_ini(&ini_path);
    if aimers.is_empty() {
        crate::log_line("padaimer: no opfor_console_aimer.ini found (pads use default PC aimer)");
        return false;
    }
    let count = aimers.len();
    if let Ok(mut g) = STATE.lock() {
        *g = Some(PadAimerState {
            aimers,
            found: Vec::new(),
            console_on: false,
            yaw_scale: 1.0,
            pitch_scale: 1.0,
        });
    }
    #[cfg(target_arch = "x86")]
    {
        let _ = std::thread::Builder::new()
            .name("opfor-padaimer-scan".into())
            .spawn(|| {
                let mut last_logged: isize = -1;
                loop {
                    std::thread::sleep(Duration::from_secs(3));
                    let need_scan = RESCAN.swap(false, Ordering::AcqRel) || {
                        STATE
                            .lock()
                            .ok()
                            .and_then(|g| {
                                g.as_ref().map(|st| {
                                    st.found.is_empty()
                                        || !st.found.iter().all(|f| unsafe {
                                            holds_expected(
                                                f,
                                                &st.aimers[f.aimer_idx],
                                                st.console_on,
                                                st.yaw_scale,
                                                st.pitch_scale,
                                            )
                                        })
                                })
                            })
                            .unwrap_or(false)
                    };
                    if !need_scan {
                        continue;
                    }
                    let aimers_clone = STATE
                        .lock()
                        .ok()
                        .and_then(|g| g.as_ref().map(|s| s.aimers.clone()))
                        .unwrap_or_default();
                    if aimers_clone.is_empty() {
                        continue;
                    }
                    let newly_found = unsafe { scan_heap(&aimers_clone) };
                    let n = newly_found.len();
                    let has_poly = newly_found.first().is_some_and(|f| f.poly_ptr != 0);
                    if let Ok(mut g) = STATE.lock()
                        && let Some(st) = g.as_mut()
                    {
                        if st.console_on && !st.found.is_empty() {
                            let oy = st.yaw_scale;
                            let op = st.pitch_scale;
                            unsafe { write_tuning(st, false, oy, op) };
                        }
                        if st.console_on || n > 0 {
                            st.console_on = false;
                        }
                        st.found = newly_found;
                    }
                    if n as isize != last_logged {
                        last_logged = n as isize;
                        if n > 0 {
                            crate::log_line(&format!(
                                "padaimer: found {n} HaloAimerData instance(s), polynomial storage {}",
                                if has_poly { "found" } else { "not found" }
                            ));
                        }
                    }
                }
            });
    }
    crate::log_line(&format!("padaimer: loaded {count} console aimer profile(s) from {}", ini_path.display()));
    true
}

/// Switch live `HaloAimerData` instances between console aim-assist tuning (`pad == true`) and PC tuning.
pub fn set_pad_active(pad: bool, yaw_scale: f32, pitch_scale: f32) {
    #[cfg(target_arch = "x86")]
    {
        let Ok(mut g) = STATE.try_lock() else {
            return;
        };
        let Some(st) = g.as_mut() else {
            return;
        };
        if st.aimers.is_empty() {
            return;
        }
        let rescale = pad && st.console_on && (yaw_scale != st.yaw_scale || pitch_scale != st.pitch_scale);
        let old_yaw = st.yaw_scale;
        let old_pitch = st.pitch_scale;
        st.yaw_scale = yaw_scale;
        st.pitch_scale = pitch_scale;
        if pad != st.console_on || rescale {
            unsafe { write_tuning(st, pad, old_yaw, old_pitch) };
            if pad != st.console_on && !st.found.is_empty() {
                crate::log_line(&format!(
                    "padaimer: switched to {} ({} instances)",
                    if pad { "console (pad)" } else { "PC (KBM)" },
                    st.found.len()
                ));
            }
            st.console_on = pad;
        }
    }
    #[cfg(not(target_arch = "x86"))]
    let _ = (pad, yaw_scale, pitch_scale);
}
