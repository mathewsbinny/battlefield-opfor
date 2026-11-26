//! Offline Persistence, Local Stats (`user/stats.txt`), & Weapon/License Unlocks for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/persistence.cpp`:
//! - Loads & saves `user/stats.txt` (key=value float map) on join and round-end commit.
//! - Hooks `Unlocks::compute` (`0x008639D0`) to feed local stats into Frostbite's unlock engine.
//! - Hooks `Unlocks::applyLicenses` (`0x008527D0`) to grant `"nam"` on Vietnam and all licensed
//!   weapons (M1 Garand / M1911 / Thompson) on WW2 (1943) levels.

#![allow(dead_code, non_snake_case)]

use std::collections::BTreeMap;
use std::ffi::{CString, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

const UNLOCKS_COMPUTE: usize = 0x0086_39D0;
const UNLOCKS_COMPUTE_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0x68, 0x01, 0x00, 0x00];
const APPLY_LICENSES: usize = 0x0085_27D0;
const APPLY_LICENSES_PROLOGUE: [u8; 8] = [0x83, 0xEC, 0x08, 0x83, 0x7C, 0x24, 0x0C, 0x00];

const STATS_SERVICE_GLOBAL: usize = 0x0156_6B64;
const STATS_LISTENER_GLOBAL: usize = 0x0156_6B68;
const LISTENER_COMMIT_SLOT: usize = 0x20 / 4;

const FIND_PLAYER_STATS: usize = 0x0084_8AC0;
const STATS_SET: usize = 0x007D_26C0;
const TREE_INCREMENT: usize = 0x0042_C300;

const PLAYER_ID_OFF: usize = 0x18;
const STATS_OWNER_ID_OFF: usize = 0x08;
const STATS_MAP_ANCHOR: usize = 0x9C;
const STATS_MAP_FIRST: usize = 0xA0;

const STATE_BITS_OFF: usize = 0x94;
const STATE_BIT_COUNT_OFF: usize = 0x9C;
const STATE_CHANGED_OFF: usize = 0xA4;

const UI_MANAGER_GLOBAL: usize = 0x019A_4CA8;
const SEND_MENU_EVENT: usize = 0x0092_9A70;
const ACQUIRE_MENU_EVENT: usize = 0x0092_0360;
const EVENT_ADD_STRING: usize = 0x0058_8910;
const EVENT_ADD_VALUE: usize = 0x0058_8940;
const QUEUE_MENU_EVENT: usize = 0x0092_99B0;
const MAKE_BOOL: usize = 0x00F7_C600;

struct Promo {
    code: &'static str,
    weapon: &'static str,
    display: &'static str,
    bit: i32,
}

const PROMOS: &[Promo] = &[
    Promo { code: "v3teran", weapon: "f2000", display: "FN F2000", bit: 3 },
    Promo { code: "your3mynextt4rget", weapon: "qbu88", display: "QBU-88", bit: 11 },
    Promo { code: "try4ndrunf0rcov3r", weapon: "m60", display: "M60", bit: 19 },
    Promo { code: "cov3r1ngthecorn3r", weapon: "uzi", display: "Silenced Uzi", bit: 15 },
    Promo { code: "aw3somef1rep0wer", weapon: "u12", display: "USAS-12", bit: 7 },
];

pub fn snapshot_stats() -> BTreeMap<String, f32> {
    STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.stats.clone()))
        .unwrap_or_default()
}

struct PersistState {
    stats_path: PathBuf,
    stats: BTreeMap<String, f32>,
}

static STATE: Mutex<Option<PersistState>> = Mutex::new(None);
static LOCAL_PLAYER_ID: AtomicU32 = AtomicU32::new(u32::MAX);

#[cfg(target_arch = "x86")]
type UnlocksComputeFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void);
#[cfg(target_arch = "x86")]
type ApplyLicensesFn =
    unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void);
#[cfg(target_arch = "x86")]
type StatsCommitFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, i32);

#[cfg(target_arch = "x86")]
static ORIG_COMPUTE: OnceLock<UnlocksComputeFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_APPLY_LICENSES: OnceLock<ApplyLicensesFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_COMMIT: OnceLock<StatsCommitFn> = OnceLock::new();

fn parse_stats_txt(txt: &str) -> BTreeMap<String, f32> {
    let mut map = BTreeMap::new();
    for raw in txt.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if let Ok(val) = v.trim().parse::<f32>() {
                map.insert(k.trim().to_string(), val);
            }
        }
    }
    map
}

fn is_local_name(name: &str) -> bool {
    let profile = crate::profile::current_name();
    name.eq_ignore_ascii_case("Human player")
        || (!profile.is_empty() && name.eq_ignore_ascii_case(&profile))
}

pub unsafe fn install(opfor_root: &Path) -> usize {
    let stats_path = opfor_root.join("user").join("stats.txt");
    let stats = fs::read_to_string(&stats_path)
        .map(|t| parse_stats_txt(&t))
        .unwrap_or_default();
    crate::log_line(&format!(
        "persist: loaded {} local stats from {}",
        stats.len(),
        stats_path.display()
    ));
    if let Ok(mut g) = STATE.lock() {
        *g = Some(PersistState { stats_path, stats });
    }

    let mut installed = 0usize;
    #[cfg(target_arch = "x86")]
    {
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                UNLOCKS_COMPUTE,
                &UNLOCKS_COMPUTE_PROLOGUE,
                detour_unlocks_compute as *const c_void,
            )
        } {
            let _ = ORIG_COMPUTE
                .set(unsafe { std::mem::transmute::<*const c_void, UnlocksComputeFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                APPLY_LICENSES,
                &APPLY_LICENSES_PROLOGUE,
                detour_apply_licenses as *const c_void,
            )
        } {
            let _ = ORIG_APPLY_LICENSES
                .set(unsafe { std::mem::transmute::<*const c_void, ApplyLicensesFn>(tramp) });
            installed += 1;
        }
    }
    installed
}

#[cfg(target_arch = "x86")]
unsafe fn hook_stats_commit() {
    if ORIG_COMMIT.get().is_some() {
        return;
    }
    let listener = unsafe { *(STATS_LISTENER_GLOBAL as *const *mut c_void) };
    if listener.is_null() {
        return;
    }
    let vtable = unsafe { *(listener as *const *mut usize) };
    if vtable.is_null() {
        return;
    }
    let slot_addr = unsafe { vtable.add(LISTENER_COMMIT_SLOT) } as usize;
    let orig_addr = unsafe { *(slot_addr as *const usize) };
    if ORIG_COMMIT
        .set(unsafe { std::mem::transmute::<usize, StatsCommitFn>(orig_addr) })
        .is_ok()
    {
        let detour_bytes = (detour_stats_commit as *const () as usize).to_le_bytes();
        if unsafe { crate::hooks::write_code(slot_addr, &detour_bytes) } {
            crate::log_line(&format!(
                "persist: hooked stats commit vtable slot @ 0x{slot_addr:08X}"
            ));
        }
    }
}

#[cfg(target_arch = "x86")]
unsafe fn save_stats(stats: *mut c_void) {
    if stats.is_null() {
        return;
    }
    type TreeNextFn = unsafe extern "cdecl" fn(*mut c_void) -> *mut c_void;
    let next: TreeNextFn = unsafe { std::mem::transmute(TREE_INCREMENT) };
    let end = stats as usize + STATS_MAP_ANCHOR;
    let mut node = unsafe { *((stats as usize + STATS_MAP_FIRST) as *const usize) };
    let mut snapshot = BTreeMap::new();
    let mut steps = 0usize;
    while node != 0 && node != end && steps < 20_000 {
        let name_ptr = unsafe { *((node + 0x10) as *const *const u8) };
        let value = unsafe { *((node + 0x14) as *const f32) };
        if value != 0.0 {
            if let Some(name) = unsafe { crate::win32::from_ansi_ptr(name_ptr) } {
                if !name.is_empty() {
                    snapshot.insert(name, value);
                }
            }
        }
        node = unsafe { next(node as *mut c_void) as usize };
        steps += 1;
    }

    if let Ok(mut g) = STATE.lock() {
        if let Some(st) = g.as_mut() {
            if let Some(parent) = st.stats_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut out = String::from(
                "; OPFOR local stats (written at round end). name=value, non-zero values only.\n",
            );
            for (k, v) in &snapshot {
                out.push_str(&format!("{k}={v}\n"));
            }
            if fs::write(&st.stats_path, out).is_ok() {
                crate::log_line(&format!("persist: saved {} stats to {}", snapshot.len(), st.stats_path.display()));
            }
            st.stats = snapshot;
        }
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_stats_commit(
    listener: *mut c_void,
    edx: *mut c_void,
    stats: *mut c_void,
    flag: i32,
) {
    if !stats.is_null() {
        let owner = unsafe { *((stats as usize + STATS_OWNER_ID_OFF) as *const u32) };
        let name = unsafe { crate::win32::from_ansi_ptr((stats as usize + STATS_OWNER_ID_OFF + 8) as *const u8) }
            .unwrap_or_default();
        if owner == LOCAL_PLAYER_ID.load(Ordering::Relaxed) || is_local_name(&name) {
            unsafe { save_stats(stats) };
            crate::onslaught_hud::on_round_ended();
        }
    }
    if let Some(&orig) = ORIG_COMMIT.get() {
        unsafe { orig(listener, edx, stats, flag) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_unlocks_compute(
    mgr: *mut c_void,
    edx: *mut c_void,
    player: *mut c_void,
    stats: *mut c_void,
) {
    let Some(&orig) = ORIG_COMPUTE.get() else {
        return;
    };
    let service = unsafe { *(STATS_SERVICE_GLOBAL as *const *mut c_void) };
    if !stats.is_null() || service.is_null() || player.is_null() {
        unsafe { orig(mgr, edx, player, stats) };
        return;
    }

    type FindStatsFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const c_void) -> *mut c_void;
    type StatsSetFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8, f32) -> *mut f32;
    let find_stats: FindStatsFn = unsafe { std::mem::transmute(FIND_PLAYER_STATS) };
    let stats_set: StatsSetFn = unsafe { std::mem::transmute(STATS_SET) };

    let id_ptr = (player as usize + PLAYER_ID_OFF) as *const c_void;
    let player_stats = unsafe { find_stats(service, std::ptr::null_mut(), id_ptr) };
    if player_stats.is_null() {
        unsafe { orig(mgr, edx, player, stats) };
        return;
    }

    let id = unsafe { *(id_ptr as *const u32) };
    let name = unsafe { crate::win32::from_ansi_ptr((player as usize + PLAYER_ID_OFF + 8) as *const u8) }
        .unwrap_or_default();
    let local = is_local_name(&name);
    if local {
        LOCAL_PLAYER_ID.store(id, Ordering::Relaxed);
    }
    unsafe { hook_stats_commit() };
    if !local {
        unsafe { orig(mgr, edx, player, stats) };
        return;
    }

    if let Ok(g) = STATE.lock() {
        if let Some(st) = g.as_ref() {
            for (k, &v) in &st.stats {
                if let Ok(ck) = CString::new(k.as_str()) {
                    unsafe { stats_set(player_stats, std::ptr::null_mut(), ck.as_ptr().cast(), v) };
                }
            }
        }
    }
    unsafe { orig(mgr, edx, player, player_stats) };
}

#[cfg(target_arch = "x86")]
#[repr(C)]
struct GameArray {
    vtbl: *const usize,
    pad: [u32; 3],
    data: *const *const u8,
}

#[cfg(target_arch = "x86")]
unsafe fn read_bc2_str(ptr: *const u8) -> String {
    let b = unsafe { *(ptr as *const *const u8) };
    let e = unsafe { *(ptr.add(4) as *const *const u8) };
    if b.is_null() || e <= b || (e as usize - b as usize) > 512 {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(b, e as usize - b as usize) };
    String::from_utf8_lossy(slice).into_owned()
}

#[cfg(target_arch = "x86")]
fn has_token(list: &str, token: Option<&str>) -> bool {
    match token {
        None => !list.trim().is_empty(),
        Some(t) => list.split_whitespace().any(|w| w.eq_ignore_ascii_case(t)),
    }
}

#[cfg(target_arch = "x86")]
unsafe fn grant_licensed(table: *mut c_void, state: *mut u8, token: Option<&str>, unlock_all: bool) {
    if table.is_null() || state.is_null() {
        return;
    }
    let words = unsafe { *(state.add(STATE_BITS_OFF) as *const *mut u32) };
    let bit_count = unsafe { *(state.add(STATE_BIT_COUNT_OFF) as *const i32) };
    let cats = unsafe { *(table as *const *const u8) };
    if words.is_null() || cats.is_null() || bit_count <= 0 {
        return;
    }
    type CountFn = unsafe extern "fastcall" fn(*const GameArray, *mut c_void) -> u32;
    let ca = unsafe { &*(cats.add(0x18) as *const GameArray) };
    let count_fn: CountFn = unsafe { std::mem::transmute(*ca.vtbl) };
    let nc = unsafe { count_fn(ca, std::ptr::null_mut()) }.min(256);
    let mut granted = 0usize;
    for i in 0..nc {
        let cat = unsafe { *ca.data.add(i as usize) };
        if cat.is_null() {
            continue;
        }
        let ia = unsafe { &*(cat.add(0x18) as *const GameArray) };
        let ic_fn: CountFn = unsafe { std::mem::transmute(*ia.vtbl) };
        let ni = unsafe { ic_fn(ia, std::ptr::null_mut()) }.min(1024);
        for j in 0..ni {
            let item = unsafe { *ia.data.add(j as usize) };
            if item.is_null() {
                continue;
            }
            if !unlock_all {
                let lic1 = unsafe { read_bc2_str(item.add(0x18)) };
                let lic2 = unsafe { read_bc2_str(item.add(0x2C)) };
                if !has_token(&lic1, token) && !has_token(&lic2, token) {
                    continue;
                }
            }
            let bit = unsafe { *(item.add(0x4C) as *const i32) };
            if bit >= 0 && bit < bit_count {
                let w = (bit as usize) >> 5;
                let mask = 1u32 << ((bit as usize) & 31);
                unsafe {
                    if (*words.add(w) & mask) == 0 {
                        *words.add(w) |= mask;
                        granted += 1;
                    }
                }
            }
        }
    }
    if granted > 0 {
        unsafe { *state.add(STATE_CHANGED_OFF) = 1 };
        crate::log_line(&format!(
            "persist: granted {granted} unlock bits (all_unlocked={unlock_all}, filter={:?})",
            token
        ));
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_apply_licenses(
    table: *mut c_void,
    edx: *mut c_void,
    player: *mut c_void,
    stats: *mut c_void,
    state: *mut c_void,
) {
    if let Some(&orig) = ORIG_APPLY_LICENSES.get() {
        unsafe { orig(table, edx, player, stats, state) };
    }
    // Unlock all progression, promo, and licensed items (including SPECACT kits, Limited Edition,
    // Veteran, Vietnam, kit shortcuts, and WW2 weapons) by default across every era.
    unsafe { grant_licensed(table, state.cast(), None, true) };
    if !state.is_null() {
        let s = state as *mut u8;
        let words = unsafe { *(s.add(STATE_BITS_OFF) as *const *mut u32) };
        let bit_count = unsafe { *(s.add(STATE_BIT_COUNT_OFF) as *const i32) };
        if !words.is_null() && bit_count > 0 {
            let mut changed = false;
            for p in PROMOS {
                if p.bit >= 0 && p.bit < bit_count {
                    let w = (p.bit as usize) >> 5;
                    let mask = 1u32 << ((p.bit as usize) & 31);
                    unsafe {
                        if (*words.add(w) & mask) == 0 {
                            *words.add(w) |= mask;
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                unsafe { *s.add(STATE_CHANGED_OFF) = 1 };
            }
        }
    }
}

fn normalize_promo(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(target_arch = "x86")]
unsafe fn send_redeem_result(ui: *mut c_void, success: bool, message: &str) {
    type SendEvtFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
    type AcquireFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type AddStrFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
    type AddValFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, u32);
    type QueueEvtFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void);
    type MakeBoolFn = unsafe extern "cdecl" fn(i32) -> u32;

    let send: SendEvtFn = unsafe { std::mem::transmute(SEND_MENU_EVENT) };
    let acquire: AcquireFn = unsafe { std::mem::transmute(ACQUIRE_MENU_EVENT) };
    let add_str: AddStrFn = unsafe { std::mem::transmute(EVENT_ADD_STRING) };
    let add_val: AddValFn = unsafe { std::mem::transmute(EVENT_ADD_VALUE) };
    let queue: QueueEvtFn = unsafe { std::mem::transmute(QUEUE_MENU_EVENT) };
    let make_bool: MakeBoolFn = unsafe { std::mem::transmute(MAKE_BOOL) };

    unsafe { send(ui, std::ptr::null_mut(), b"redeemStart\0".as_ptr()) };
    let evt = unsafe { acquire(ui, std::ptr::null_mut()) };
    if evt.is_null() {
        return;
    }
    let Ok(cmsg) = CString::new(message) else { return };
    unsafe {
        add_str(evt, std::ptr::null_mut(), b"redeemStop\0".as_ptr());
        add_val(evt, std::ptr::null_mut(), make_bool(i32::from(success)));
        add_str(evt, std::ptr::null_mut(), cmsg.as_ptr().cast());
        queue(ui, std::ptr::null_mut(), evt);
    }
}

/// Handle offline `redeemCode` FSCommand from the main menu's Redeem Code dialog.
#[cfg(target_arch = "x86")]
pub unsafe fn handle_menu_command(name: &str, args: &str) -> bool {
    if name != "redeemCode" {
        return false;
    }
    let ui = unsafe { *(UI_MANAGER_GLOBAL as *const *mut c_void) };
    if ui.is_null() {
        return true;
    }
    let code = normalize_promo(args);
    if let Some(p) = PROMOS.iter().find(|p| p.code == code) {
        crate::log_line(&format!("persist: redeemed promo code for {} ({})", p.weapon, p.display));
        let msg = format!("{} is unlocked (all promo & SPECACT items unlocked by default).", p.display);
        unsafe { send_redeem_result(ui, true, &msg) };
    } else {
        crate::log_line(&format!("persist: rejected redeem code ({} chars)", code.len()));
        unsafe { send_redeem_result(ui, false, "DIALOG_REDEEM_FAILED") };
    }
    true
}
