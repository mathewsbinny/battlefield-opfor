//! Offline Weapons & Gadgets / My Stats / Vehicles menu screens for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/stats_screens.cpp`:
//! - Answers `extern.playerStats`, `extern.unlockGrid`, `extern.vehicleGrid`, and
//!   `extern.weaponProficiency$<id>` offline from `user/stats.txt` + Frostbite's static
//!   unlock tree, rank table, score buckets, and vehicle list.
//! - Marks all progression, promo, and licensed items (including SPECACT kits) as unlocked
//!   by default so Weapons & Gadgets reflects the live in-match unlock state.
//! - Sends the `unlockInfo` menu event shortly after `playerStats` is queried so Weapons &
//!   Gadgets populates its lists immediately offline.

#![allow(dead_code, non_snake_case)]

use std::collections::BTreeMap;
use std::ffi::{CString, c_void};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const UI_MANAGER_GLOBAL: usize = 0x019A_4CA8;
const UI_STATS_HOLDER: usize = 0x000C_38D4;
const UI_GAME_MOD: usize = 0x000C_8C8C;
const UI_PERSISTENCE: usize = 0x000C_3D50;
const RANK_TABLE_OFF: usize = 0x120;
const BUCKET_LIST_OFF: usize = 0x144;

const SHARED_CONFIG_FN: usize = 0x0092_0860;
const TREE_OFFSET_MOD1: usize = 0x90;
const TREE_OFFSET_OTHER: usize = 0xB0;
const TREE_ARRAY_OFF: usize = 0x10;
const FIND_WEAPON_DATA_FN: usize = 0x0093_C550;
const MENU_DATA_OFF: usize = 0x94;
const VEHICLE_LIST_OFF: usize = 0x88;
const VEHICLE_ARRAY_OFF: usize = 0x10;
const SPECIALIZATION_TYPE: usize = 0x01C0_4FD8;

const ONLINE_SINGLETON: usize = 0x0157_13D8;
const ONLINE_PROFILE_OFF: usize = 0x54;
const PROFILE_VETERAN_OFF: usize = 0x2860;

const MAKE_TEXT: usize = 0x00F7_C5C0;
const MAKE_INT: usize = 0x00F7_C5D0;
const MAKE_FLOAT: usize = 0x00F7_C5E0;
const MAKE_BOOL: usize = 0x00F7_C600;
const MAKE_ARRAY: usize = 0x00F7_C610;
const UNDEFINED: usize = 0x00F7_C620;
const SEND_MENU_EVENT: usize = 0x0092_9A70;

#[repr(C)]
struct Bc2String {
    begin: *mut u8,
    end: *mut u8,
    capacity: *mut u8,
}

#[derive(Default)]
struct UnlockInfoTimer {
    last_grid: Option<Instant>,
    pending_at: Option<Instant>,
}

static UNLOCK_TIMER: Mutex<UnlockInfoTimer> = Mutex::new(UnlockInfoTimer {
    last_grid: None,
    pending_at: None,
});

#[cfg(target_arch = "x86")]
fn in_code(p: usize) -> bool {
    (0x0040_1000..0x0138_5B7B).contains(&p)
}

#[cfg(target_arch = "x86")]
fn in_const(p: usize) -> bool {
    (0x0140_5000..0x014E_F110).contains(&p)
}

#[cfg(target_arch = "x86")]
unsafe fn virtual_ok(obj: *const u8, slot: usize) -> bool {
    if (obj as usize) < 0x10000 {
        return false;
    }
    let vtbl = unsafe { *(obj as *const usize) };
    if !in_const(vtbl) {
        return false;
    }
    let fn_ptr = unsafe { *(vtbl as *const usize).add(slot) };
    in_code(fn_ptr)
}

#[cfg(target_arch = "x86")]
unsafe fn read_array(obj: *const u8) -> Option<(usize, *const u8)> {
    if !unsafe { virtual_ok(obj, 0) } {
        return None;
    }
    type CountFn = unsafe extern "fastcall" fn(*const u8, *mut c_void) -> u32;
    let vtbl = unsafe { *(obj as *const *const usize) };
    let count_fn: CountFn = unsafe { std::mem::transmute(*vtbl) };
    let count = unsafe { count_fn(obj, std::ptr::null_mut()) } as usize;
    let data = unsafe { *(obj.add(8) as *const *const u8) };
    if count < 100_000 && (count == 0 || !data.is_null()) {
        Some((count, data))
    } else {
        None
    }
}

#[cfg(target_arch = "x86")]
unsafe fn make_int(v: i32) -> u32 {
    type F = unsafe extern "cdecl" fn(i32) -> u32;
    let f: F = unsafe { std::mem::transmute(MAKE_INT) };
    unsafe { f(v) }
}

#[cfg(target_arch = "x86")]
unsafe fn make_float(v: f32) -> u32 {
    type F = unsafe extern "cdecl" fn(f32) -> u32;
    let f: F = unsafe { std::mem::transmute(MAKE_FLOAT) };
    unsafe { f(v) }
}

#[cfg(target_arch = "x86")]
unsafe fn make_bool(v: bool) -> u32 {
    type F = unsafe extern "cdecl" fn(i32) -> u32;
    let f: F = unsafe { std::mem::transmute(MAKE_BOOL) };
    unsafe { f(i32::from(v)) }
}

#[cfg(target_arch = "x86")]
unsafe fn make_text(s: &str) -> u32 {
    type F = unsafe extern "cdecl" fn(*const u8) -> u32;
    let f: F = unsafe { std::mem::transmute(MAKE_TEXT) };
    let cs = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
    unsafe { f(cs.as_ptr().cast()) }
}

#[cfg(target_arch = "x86")]
unsafe fn make_array(vals: &[u32]) -> u32 {
    type F = unsafe extern "cdecl" fn(i32, *const u32) -> u32;
    let f: F = unsafe { std::mem::transmute(MAKE_ARRAY) };
    unsafe { f(vals.len() as i32, vals.as_ptr()) }
}

#[cfg(target_arch = "x86")]
unsafe fn make_undefined() -> u32 {
    type F = unsafe extern "cdecl" fn() -> u32;
    let f: F = unsafe { std::mem::transmute(UNDEFINED) };
    unsafe { f() }
}

#[cfg(target_arch = "x86")]
unsafe fn find_weapon_data(ui: *mut c_void, id: &str) -> *const u8 {
    let mut buf = id.as_bytes().to_vec();
    buf.push(0);
    let n = buf.len() - 1;
    let s = Bc2String {
        begin: buf.as_mut_ptr(),
        end: unsafe { buf.as_mut_ptr().add(n) },
        capacity: unsafe { buf.as_mut_ptr().add(n + 1) },
    };
    type FindFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const Bc2String) -> *const u8;
    let f: FindFn = unsafe { std::mem::transmute(FIND_WEAPON_DATA_FN) };
    unsafe { f(ui, std::ptr::null_mut(), &s) }
}

fn stat_get(s: &BTreeMap<String, f32>, key: &str, default: f32) -> f32 {
    s.get(key).copied().unwrap_or(default)
}

fn star_level(s: &BTreeMap<String, f32>, id: &str) -> i32 {
    let kills = stat_get(s, &format!("ki_{id}"), 0.0);
    if kills >= 1000.0 {
        11
    } else if kills >= 100.0 {
        ((kills / 100.0) as i32).min(10)
    } else if kills >= 50.0 {
        2
    } else if kills >= 25.0 {
        1
    } else {
        0
    }
}

#[cfg(target_arch = "x86")]
unsafe fn build_player_stats(ui: *mut c_void, stats: &BTreeMap<String, f32>) -> u32 {
    let mut ranks = Vec::new();
    let pers = unsafe { *((ui as usize + UI_PERSISTENCE) as *const *const u8) };
    if !pers.is_null() {
        if let Some((cnt, data)) = unsafe { read_array(pers.add(RANK_TABLE_OFF)) } {
            for i in 0..cnt.min(80) {
                let pts = unsafe { *(data.add(i * 0x3C + 0x24) as *const i32) };
                ranks.push(pts);
            }
        }
    }
    let rank_thresh = |r: i32| -> i32 {
        if r >= 0 && (r as usize) < ranks.len() {
            ranks[r as usize]
        } else {
            0
        }
    };

    let prof = unsafe { *((ONLINE_SINGLETON + ONLINE_PROFILE_OFF) as *const *const u8) };
    let veteran = if !prof.is_null() {
        unsafe { *(prof.add(PROFILE_VETERAN_OFF) as *const i32) }
    } else {
        0
    };

    let rank = stat_get(stats, "rank", 0.0).round() as i32;
    let mut v = vec![unsafe { make_undefined() }; 26];
    unsafe {
        v[0] = make_int(stat_get(stats, "score", 0.0).round() as i32);
        v[1] = make_int(stat_get(stats, "kills", 0.0).round() as i32);
        v[2] = make_int(stat_get(stats, "deaths", 0.0).round() as i32);
        v[3] = make_int(veteran);
        v[4] = make_int(rank);
        v[5] = make_int(rank_thresh(rank));
        v[6] = make_int(rank + 1);
        v[7] = make_int(rank_thresh(rank + 1));

        let bucket_names = ["sc_assault", "sc_demo", "sc_support", "sc_recon", "sc_vehicle", "sc_award"];
        for (k, &bname) in bucket_names.iter().enumerate() {
            v[20 + k] = make_int(stat_get(stats, bname, 0.0).round() as i32);
        }
        make_array(&v)
    }
}

#[cfg(target_arch = "x86")]
unsafe fn build_unlock_grid(ui: *mut c_void, stats: &BTreeMap<String, f32>) -> u32 {
    type ConfigFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> *const u8;
    let get_cfg: ConfigFn = unsafe { std::mem::transmute(SHARED_CONFIG_FN) };
    let config = unsafe { get_cfg(ui, std::ptr::null_mut()) };
    let mut rows = Vec::new();
    if !config.is_null() {
        let game_mod = unsafe { *((ui as usize + UI_GAME_MOD) as *const i32) };
        let tree_at = if game_mod == 1 {
            TREE_OFFSET_MOD1 + TREE_ARRAY_OFF
        } else {
            TREE_OFFSET_OTHER + TREE_ARRAY_OFF
        };
        if let Some((cnt, data)) = unsafe { read_array(config.add(tree_at)) } {
            for i in 0..cnt.min(256) {
                let e = unsafe { data.add(i * 0x3C) };
                let name = unsafe { crate::win32::from_ansi_ptr(*(e.add(0x28) as *const *const u8)) }
                    .unwrap_or_default();
                let x = unsafe { *(e.add(0x34) as *const i32) };
                let y = unsafe { *(e.add(0x38) as *const i32) };
                let item = unsafe { *(e.add(0x24) as *const *const u8) };
                let has_item = !item.is_null();
                let (id, item38, is_spec) = if has_item {
                    let id = unsafe { crate::win32::from_ansi_ptr(*(item.add(0x40) as *const *const u8)) }
                        .unwrap_or_else(|| name.clone());
                    let i38 = unsafe { *(item.add(0x38) as *const i32) };
                    let spec = if unsafe { virtual_ok(item, 2) } {
                        type TypeFn = unsafe extern "fastcall" fn(*const u8, *mut c_void) -> usize;
                        let vtbl = unsafe { *(item as *const *const usize) };
                        let tf: TypeFn = unsafe { std::mem::transmute(*vtbl.add(2)) };
                        unsafe { tf(item, std::ptr::null_mut()) == SPECIALIZATION_TYPE }
                    } else {
                        false
                    };
                    (id, i38, spec)
                } else {
                    (name.clone(), 0, false)
                };

                let wd = unsafe { find_weapon_data(ui, if has_item { &id } else { &name }) };
                let (d_name, d_desc, d_icon) = if !wd.is_null() {
                    unsafe {
                        (
                            crate::win32::from_ansi_ptr(*(wd.add(0x48) as *const *const u8)),
                            crate::win32::from_ansi_ptr(*(wd.add(0x6C) as *const *const u8)),
                            crate::win32::from_ansi_ptr(*(wd.add(0x3C) as *const *const u8)),
                        )
                    }
                } else {
                    (None, None, None)
                };

                // All items (including SPECACT kits & promo weapons) shown as unlocked by default
                let row = unsafe {
                    [
                        make_text(&id),
                        d_name.as_deref().map_or_else(|| make_undefined(), |s| make_text(s)),
                        d_desc.as_deref().map_or_else(|| make_undefined(), |s| make_text(s)),
                        make_bool(true),
                        make_int(if !has_item { 2 } else if is_spec { 1 } else { 0 }),
                        make_undefined(),
                        make_int(if has_item { star_level(stats, &id) } else { 0 }),
                        make_int(y),
                        make_int(x),
                        make_int(if has_item { item38 } else { 0 }),
                        d_icon.as_deref().map_or_else(|| make_undefined(), |s| make_text(s)),
                        make_bool(true),
                        make_bool(false),
                        make_int(-1),
                        make_int(-1),
                        make_text("0/0"),
                        make_int(0),
                        make_int(0),
                    ]
                };
                rows.push(unsafe { make_array(&row) });
            }
        }
    }
    unsafe { make_array(&rows) }
}

#[cfg(target_arch = "x86")]
unsafe fn build_vehicle_grid(menu: *mut c_void, stats: &BTreeMap<String, f32>) -> u32 {
    let mut rows = Vec::new();
    if !menu.is_null() {
        let menu_data = unsafe { *((menu as usize + MENU_DATA_OFF) as *const *const u8) };
        if !menu_data.is_null() {
            if let Some((cnt, data)) =
                unsafe { read_array(menu_data.add(VEHICLE_LIST_OFF + VEHICLE_ARRAY_OFF)) }
            {
                for i in 0..cnt.min(128) {
                    let v = unsafe { data.add(i * 0x48) };
                    let id = unsafe { crate::win32::from_ansi_ptr(*(v as *const *const u8)) }
                        .unwrap_or_default();
                    let stat = unsafe { crate::win32::from_ansi_ptr(*(v.add(0x24) as *const *const u8)) }
                        .unwrap_or_default();
                    let group = unsafe { *(v.add(0x10) as *const i32) };
                    let column = unsafe { *(v.add(0x14) as *const i32) };
                    let s3c = unsafe { crate::win32::from_ansi_ptr(*(v.add(0x3C) as *const *const u8)) }
                        .unwrap_or_default();
                    let s18 = unsafe { crate::win32::from_ansi_ptr(*(v.add(0x18) as *const *const u8)) }
                        .unwrap_or_default();
                    let s30 = unsafe { crate::win32::from_ansi_ptr(*(v.add(0x30) as *const *const u8)) }
                        .unwrap_or_default();
                    let p = unsafe { *(v.add(0x0C) as *const *const u8) };
                    let sc = if !p.is_null() {
                        unsafe { crate::win32::from_ansi_ptr(*(p.add(0x0C) as *const *const u8)) }
                    } else {
                        None
                    };
                    let row = unsafe {
                        [
                            make_text(&stat),
                            make_int(column),
                            make_int(group),
                            make_text(&s3c),
                            sc.as_deref().map_or_else(|| make_undefined(), |s| make_text(s)),
                            make_int(star_level(stats, &id)),
                            make_text(&id),
                            make_text(&s18),
                            make_text(&s30),
                        ]
                    };
                    rows.push(unsafe { make_array(&row) });
                }
            }
        }
    }
    unsafe { make_array(&rows) }
}

#[cfg(target_arch = "x86")]
unsafe fn build_proficiency() -> u32 {
    let mut v = vec![unsafe { make_undefined() }; 9];
    v[5] = unsafe { make_int(0) };
    unsafe { make_array(&v) }
}

/// Answer `playerStats`, `unlockGrid`, `vehicleGrid`, and `weaponProficiency$<id>` offline.
#[cfg(target_arch = "x86")]
pub unsafe fn handle_extern(menu: *mut c_void, name: &str) -> Option<u32> {
    let ui = unsafe { *(UI_MANAGER_GLOBAL as *const *mut c_void) };
    if ui.is_null() {
        return None;
    }
    // Only answer when EA's online stats holder is absent (always offline).
    if !unsafe { *((ui as usize + UI_STATS_HOLDER) as *const *mut c_void) }.is_null() {
        return None;
    }
    let key = name.split('$').next().unwrap_or(name);
    match key {
        "weaponProficiency" => Some(unsafe { build_proficiency() }),
        "playerStats" => {
            if let Ok(mut t) = UNLOCK_TIMER.lock() {
                let quiet = t.last_grid.is_none_or(|lg| lg.elapsed() >= Duration::from_millis(2500));
                if quiet && t.pending_at.is_none() {
                    t.pending_at = Some(Instant::now() + Duration::from_millis(250));
                }
            }
            let stats = crate::persistence::snapshot_stats();
            Some(unsafe { build_player_stats(ui, &stats) })
        }
        "unlockGrid" => {
            if let Ok(mut t) = UNLOCK_TIMER.lock() {
                t.last_grid = Some(Instant::now());
                t.pending_at = None;
            }
            let stats = crate::persistence::snapshot_stats();
            Some(unsafe { build_unlock_grid(ui, &stats) })
        }
        "vehicleGrid" => {
            let stats = crate::persistence::snapshot_stats();
            Some(unsafe { build_vehicle_grid(menu, &stats) })
        }
        _ => None,
    }
}

/// Send delayed `unlockInfo` menu event so Weapons & Gadgets populates its lists offline.
#[cfg(target_arch = "x86")]
pub unsafe fn on_ui_frame(ui: *mut c_void) {
    if ui.is_null() {
        return;
    }
    let fire = if let Ok(mut t) = UNLOCK_TIMER.lock() {
        if t.pending_at.is_some_and(|at| Instant::now() >= at) {
            t.pending_at = None;
            true
        } else {
            false
        }
    } else {
        false
    };
    if fire && unsafe { *((ui as usize + UI_STATS_HOLDER) as *const *mut c_void) }.is_null() {
        type SendEvtFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
        let send: SendEvtFn = unsafe { std::mem::transmute(SEND_MENU_EVENT) };
        unsafe { send(ui, std::ptr::null_mut(), b"unlockInfo\0".as_ptr()) };
    }
}
