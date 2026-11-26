//! Battlefield 1943 (Pacific War) gameplay mechanics for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/` (`inputframe.cpp`, `horn.cpp`, `boatrelease.cpp`, `repair.cpp`):
//! - Shared client input-frame hook (`0x0081ADA8` -> `0x007ED7A0`, `SetEntryInput` @ `0x00612A00`).
//! - Tank horn (`H` key -> `EiaDynamicGadget1` on Sherman / Chi-Ha).
//! - Carrier landing craft (LCVP) cradle release on throttle (`ServerVehicleSpawnEntity` vtable slot 45).
//! - Airfield repair stations (`DamageAreaTriggerEntity` negative-DPS team & allowed-vehicle filtering).

#![allow(dead_code, non_snake_case)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const RESTRICT_CALL: usize = 0x0081_ADA8;
const RESTRICT_CALL_BYTES: [u8; 5] = [0xE8, 0xF3, 0x29, 0xFD, 0xFF];
const RESTRICT_FN: usize = 0x007E_D7A0;
const STATE_SET_FN: usize = 0x0061_2A00;

const HORN_EIA: i32 = 35;
const GEAR_EIA: i32 = 36;
const GEAR_PITCH_LIMIT: f32 = 20.0;
const GEAR_ROLL_LIMIT: f32 = 20.0;
const GEAR_SPEED_LIMIT: f32 = 35.0;
const GEAR_HEIGHT_LIMIT: f32 = 60.0;
const GEAR_PROBE_DROP: f32 = 3.0;
const GEAR_PROBE_LENGTH: f32 = 1600.0;

const AI_UPDATE: usize = 0x0059_E190;
const AI_UPDATE_PROLOGUE: [u8; 7] = [0x56, 0x8B, 0xF1, 0x80, 0x7E, 0x18, 0x00];
const AI_MANAGER_GLOBAL: usize = 0x0189_037C;
const SPHERE_CAST_FN: usize = 0x00B1_45C0;
const EIA_THROTTLE: usize = 0;
const THROTTLE_ON: f32 = 0.01;
const MATCH_RADIUS_SQ: f32 = 16.0; // 4.0m^2

const SPAWNER_VTABLE: usize = 0x0142_96D0;
const SPAWNER_UPDATE_SLOT: usize = 45;
const SPAWNER_UPDATE_FN: usize = 0x0072_BCA0;
const RAISE_EVENT_FN: usize = 0x0100_2950;
const TRANSFORM_FN: usize = 0x0061_0A20;

const REPAIR_HANDLER: usize = 0x0072_9C90;
const REPAIR_HANDLER_PROLOGUE: [u8; 6] = [0x56, 0x57, 0x8B, 0x7C, 0x24, 0x0C];
const REPAIR_AREA_CALL: usize = 0x0072_83CB;
const REPAIR_AREA_CALL_BYTES: [u8; 5] = [0xE8, 0x80, 0x8B, 0xFB, 0xFF];
const REPAIR_AREA_TEST_FN: usize = 0x006E_0F50;

const fn djb2_xor(bytes: &[u8]) -> u32 {
    let mut h: u32 = 5381;
    let mut i = 0;
    while i < bytes.len() {
        h = h.wrapping_mul(33) ^ (bytes[i] as u32);
        i += 1;
    }
    h
}

const HASH_RELEASE_EVENT: u32 = djb2_xor(b"OnOccupantInputTriggered");
const HASH_DAMAGE_AREA: u32 = djb2_xor(b"DamageArea");
const HASH_TEAM1: u32 = djb2_xor(b"Team1");
const HASH_TEAM2: u32 = djb2_xor(b"Team2");
const HASH_TEAM_NEUTRAL: u32 = djb2_xor(b"TeamNeutral");

fn allow_hash(name: &str) -> u32 {
    let mut h = djb2_xor(b"Allow:");
    for &b in name.as_bytes() {
        h = h.wrapping_mul(33) ^ (b as u32);
    }
    h
}

#[derive(Clone, Default)]
struct Station {
    team: i32,
    allowed: Vec<u32>,
}

impl Station {
    fn on_event(&mut self, hash: u32) {
        if hash == HASH_DAMAGE_AREA {
            *self = Self::default();
        } else if hash == HASH_TEAM1 {
            self.team = 1;
        } else if hash == HASH_TEAM2 {
            self.team = 2;
        } else if hash == HASH_TEAM_NEUTRAL {
            self.team = 0;
        } else if !self.allowed.contains(&hash) && self.allowed.len() < 8 {
            self.allowed.push(hash);
        }
    }

    fn repairs(&self, vehicle_team: i32, name_sid: &str) -> bool {
        if self.team != 0 && vehicle_team != self.team {
            return false;
        }
        if self.allowed.is_empty() {
            return true;
        }
        if name_sid.is_empty() {
            return false;
        }
        self.allowed.contains(&allow_hash(name_sid))
    }
}

/// Live pilot-seat telemetry for BF1943 aircraft (native replacement for `HudVehicleTrackerData`).
#[derive(Clone, Copy, Default)]
pub struct PlaneTelemetry {
    pub active: bool,
    pub altitude_m: f32,
    pub speed_mps: f32,
    pub pitch_deg: f32,
    pub roll_deg: f32,
    pub gear_up: bool,
}

static PLANE_TELEMETRY: Mutex<PlaneTelemetry> = Mutex::new(PlaneTelemetry {
    active: false,
    altitude_m: 0.0,
    speed_mps: 0.0,
    pitch_deg: 0.0,
    roll_deg: 0.0,
    gear_up: false,
});

#[must_use]
pub fn plane_telemetry() -> PlaneTelemetry {
    PLANE_TELEMETRY.lock().map(|g| *g).unwrap_or_default()
}

struct BoatRequest {
    at: [f32; 3],
    posted: Instant,
}

struct DriverWatch {
    boat: usize,
    opened: bool,
}

struct GearWatch {
    plane: usize,
    up: bool,
    key_was_down: bool,
    last_pos: [f32; 3],
    last_time: Option<Instant>,
}

static STATIONS: Mutex<Option<HashMap<usize, Station>>> = Mutex::new(None);
static BOAT_REQ: Mutex<Option<BoatRequest>> = Mutex::new(None);
static BOAT_PENDING_HINT: AtomicBool = AtomicBool::new(false);
static DRIVER_WATCH: Mutex<DriverWatch> = Mutex::new(DriverWatch {
    boat: 0,
    opened: false,
});
static GEAR_WATCH: Mutex<GearWatch> = Mutex::new(GearWatch {
    plane: 0,
    up: false,
    key_was_down: false,
    last_pos: [0.0; 3],
    last_time: None,
});

struct GroundProbeState {
    want: bool,
    want_at: [f32; 3],
    height: f32,
    height_time: Option<Instant>,
}

static GROUND_PROBE_WANT: AtomicBool = AtomicBool::new(false);
static GROUND_PROBE: Mutex<GroundProbeState> = Mutex::new(GroundProbeState {
    want: false,
    want_at: [0.0; 3],
    height: -1.0,
    height_time: None,
});

#[cfg(target_arch = "x86")]
type RepairHandlerFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void);
#[cfg(target_arch = "x86")]
type AiUpdateFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, u32);
#[cfg(target_arch = "x86")]
static ORIG_REPAIR_HANDLER: OnceLock<RepairHandlerFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_AI_UPDATE: OnceLock<AiUpdateFn> = OnceLock::new();

#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
    fn GetForegroundWindow() -> *mut c_void;
}

#[inline]
fn is_pacific_active() -> bool {
    crate::hooks::is_pacific_active()
}

#[cfg(target_arch = "x86")]
unsafe fn controlled_entity_sid(player: *mut c_void) -> Option<(usize, String)> {
    if player.is_null() {
        return None;
    }
    let ent = unsafe { *((player as usize + 0xC68) as *const usize) };
    if ent < 0x10000 {
        return None;
    }
    let data = unsafe { *((ent + 0x0C) as *const usize) };
    if data < 0x10000 {
        return None;
    }
    let sid_ptr = unsafe { *((data + 0x210) as *const *const u8) };
    let sid = unsafe { crate::win32::from_ansi_ptr(sid_ptr) }?;
    Some((ent, sid))
}

#[cfg(target_arch = "x86")]
unsafe fn entity_transform(entity: usize) -> Option<[f32; 16]> {
    if entity < 0x10000 {
        return None;
    }
    type TransformFn = unsafe extern "fastcall" fn(usize, *mut c_void) -> *const f32;
    let f: TransformFn = unsafe { std::mem::transmute(TRANSFORM_FN) };
    let m = unsafe { f(entity, std::ptr::null_mut()) };
    if m.is_null() {
        return None;
    }
    let mut out = [0.0f32; 16];
    unsafe { std::ptr::copy_nonoverlapping(m, out.as_mut_ptr(), 16) };
    Some(out)
}

#[cfg(target_arch = "x86")]
unsafe fn entity_position(entity: usize) -> Option<[f32; 3]> {
    let m = unsafe { entity_transform(entity) }?;
    Some([m[12], m[13], m[14]])
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_restrict_input(player: *mut c_void, edx: *mut c_void) {
    type RestrictFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
    type StateSetFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, i32, i32);
    let orig: RestrictFn = unsafe { std::mem::transmute(RESTRICT_FN) };
    unsafe { orig(player, edx) };

    if player.is_null() || !is_pacific_active() {
        return;
    }
    let state = unsafe { *((player as usize + 0xC74) as *const *mut c_void) };
    if state.is_null() {
        return;
    }
    let Some((ent, sid)) = (unsafe { controlled_entity_sid(player) }) else {
        return;
    };

    // 1. Tank horn on H key (Sherman / Chi-Ha)
    if sid == "Sherman" || sid == "Chi-Ha" {
        let down = unsafe { (GetAsyncKeyState(b'H' as i32) as u16 & 0x8000) != 0 && !GetForegroundWindow().is_null() };
        let set_input: StateSetFn = unsafe { std::mem::transmute(STATE_SET_FN) };
        unsafe { set_input(state, std::ptr::null_mut(), HORN_EIA, i32::from(down)) };
    }

    // 2. WW2 Aircraft Landing Gear toggle on G key (Corsair / Zero -> EiaDynamicGadget2)
    if sid == "Corsair" || sid == "Zero" {
        if let Some(m) = unsafe { entity_transform(ent) } {
            let now = Instant::now();
            let key_down = unsafe {
                (GetAsyncKeyState(b'G' as i32) as u16 & 0x8000) != 0 && !GetForegroundWindow().is_null()
            };
            let pitch_deg = m[9].clamp(-1.0, 1.0).asin() * 57.295_78;
            let roll_deg = m[1].atan2(m[5]) * 57.295_78;
            let pos = [m[12], m[13], m[14]];
            let height = if let Ok(mut gp) = GROUND_PROBE.lock() {
                gp.want = true;
                gp.want_at = pos;
                GROUND_PROBE_WANT.store(true, Ordering::Release);
                if gp.height >= 0.0
                    && gp.height_time.is_some_and(|t| t.elapsed() <= Duration::from_millis(500))
                {
                    gp.height
                } else {
                    -1.0
                }
            } else {
                -1.0
            };
            let gear_up = if let Ok(mut gw) = GEAR_WATCH.lock() {
                if gw.plane != ent {
                    gw.plane = ent;
                    gw.up = false;
                    gw.last_pos = pos;
                    gw.last_time = Some(now);
                }
                let speed = if let Some(prev_t) = gw.last_time {
                    let dt = now.duration_since(prev_t).as_secs_f32();
                    if dt > 0.001 {
                        let dx = pos[0] - gw.last_pos[0];
                        let dy = pos[1] - gw.last_pos[1];
                        let dz = pos[2] - gw.last_pos[2];
                        (dx * dx + dy * dy + dz * dz).sqrt() / dt
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                gw.last_pos = pos;
                gw.last_time = Some(now);
                let pressed = key_down && !gw.key_was_down;
                gw.key_was_down = key_down;
                if pressed {
                    let height_ok = height < 0.0 || height >= GEAR_HEIGHT_LIMIT;
                    let may_raise = pitch_deg.abs() <= GEAR_PITCH_LIMIT
                        && roll_deg.abs() <= GEAR_ROLL_LIMIT
                        && speed >= GEAR_SPEED_LIMIT
                        && height_ok;
                    let next = if gw.up { false } else { may_raise };
                    if next != gw.up {
                        gw.up = next;
                        crate::log_line(&format!(
                            "bf1943: landing gear {} ({sid}, speed={speed:.1}m/s, height={height:.0}m)",
                            if next { "UP" } else { "DOWN" }
                        ));
                    }
                }
                if let Ok(mut pt) = PLANE_TELEMETRY.lock() {
                    *pt = PlaneTelemetry {
                        active: true,
                        altitude_m: height.max(0.0),
                        speed_mps: speed,
                        pitch_deg,
                        roll_deg,
                        gear_up: gw.up,
                    };
                }
                gw.up
            } else {
                false
            };
            let set_input: StateSetFn = unsafe { std::mem::transmute(STATE_SET_FN) };
            unsafe { set_input(state, std::ptr::null_mut(), GEAR_EIA, i32::from(gear_up)) };
        }
    } else {
        if let Ok(mut pt) = PLANE_TELEMETRY.lock() {
            pt.active = false;
        }
        if let Ok(mut gw) = GEAR_WATCH.lock() {
            gw.plane = 0;
            gw.up = false;
            gw.key_was_down = false;
        }
        if GROUND_PROBE_WANT.swap(false, Ordering::AcqRel) {
            if let Ok(mut gp) = GROUND_PROBE.lock() {
                gp.want = false;
                gp.height = -1.0;
            }
        }
    }

    // 3. Carrier LCVP landing craft cradle release on throttle
    let throttle = unsafe { *((state as usize + 8 + 4 * EIA_THROTTLE) as *const f32) };
    let boat_ent = if sid == "Landing Craft" { ent } else { 0 };
    let trigger = if let Ok(mut w) = DRIVER_WATCH.lock() {
        if w.boat != boat_ent {
            w.boat = boat_ent;
            w.opened = false;
        }
        if w.boat != 0 && !w.opened && throttle.abs() > THROTTLE_ON {
            w.opened = true;
            true
        } else {
            false
        }
    } else {
        false
    };
    if trigger {
        if let Some(pos) = unsafe { entity_position(ent) } {
            if let Ok(mut req) = BOAT_REQ.lock() {
                *req = Some(BoatRequest {
                    at: pos,
                    posted: Instant::now(),
                });
                BOAT_PENDING_HINT.store(true, Ordering::Release);
                crate::log_line(&format!(
                    "bf1943: LCVP throttle release requested at ({:.1}, {:.1}, {:.1})",
                    pos[0], pos[1], pos[2]
                ));
            }
        }
    }

    // 4. BF1943 Air Raid Bunker auto-fire (`HudData.VehicleType == 7` -> 5.0s AutoFireDelay -> EiaFire)
    let hud_type = unsafe {
        let data_ptr = *((ent + 0x0C) as *const usize);
        if data_ptr > 0x10000 {
            *((data_ptr + 0x208) as *const i32)
        } else {
            -1
        }
    };
    if let Some(fire_level) = crate::airraid::tick_bunker_trigger(hud_type) {
        let set_input: StateSetFn = unsafe { std::mem::transmute(STATE_SET_FN) };
        unsafe { set_input(state, std::ptr::null_mut(), crate::airraid::EIA_FIRE, i32::from(fire_level)) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_spawner_update(
    self_ptr: *mut c_void,
    edx: *mut c_void,
    dt: *const f32,
) {
    type UpdateFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const f32);
    type RaiseFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, u32, i32, i32);
    let orig: UpdateFn = unsafe { std::mem::transmute(SPAWNER_UPDATE_FN) };
    unsafe { orig(self_ptr, edx, dt) };

    if !BOAT_PENDING_HINT.load(Ordering::Acquire) {
        return;
    }
    let req_at = {
        let Ok(mut g) = BOAT_REQ.lock() else { return };
        if let Some(ref r) = *g {
            if r.posted.elapsed() > Duration::from_secs(2) {
                *g = None;
                BOAT_PENDING_HINT.store(false, Ordering::Release);
                return;
            }
            r.at
        } else {
            BOAT_PENDING_HINT.store(false, Ordering::Release);
            return;
        }
    };
    if let Some(sp) = unsafe { entity_position(self_ptr as usize) } {
        let dx = sp[0] - req_at[0];
        let dy = sp[1] - req_at[1];
        let dz = sp[2] - req_at[2];
        if dx * dx + dy * dy + dz * dz <= MATCH_RADIUS_SQ {
            let claimed = if let Ok(mut g) = BOAT_REQ.lock() {
                let had = g.is_some();
                *g = None;
                BOAT_PENDING_HINT.store(false, Ordering::Release);
                had
            } else {
                false
            };
            if claimed {
                let raise: RaiseFn = unsafe { std::mem::transmute(RAISE_EVENT_FN) };
                unsafe { raise(self_ptr, std::ptr::null_mut(), HASH_RELEASE_EVENT, 0, 1) };
                crate::log_line("bf1943: LCVP spawner raised OnOccupantInputTriggered (cradle released)");
            }
        }
    }
}

#[cfg(target_arch = "x86")]
unsafe fn station_dps(trigger: usize) -> f32 {
    if trigger < 0x10000 {
        return 0.0;
    }
    let data = unsafe { *((trigger + 0x0C) as *const usize) };
    if data < 0x10000 {
        return 0.0;
    }
    unsafe { *((data + 0x94) as *const f32) }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_repair_handler(
    trigger: *mut c_void,
    edx: *mut c_void,
    event: *mut c_void,
) {
    if is_pacific_active()
        && !trigger.is_null()
        && !event.is_null()
        && unsafe { station_dps(trigger as usize) } < 0.0
    {
        let hash = unsafe { *((event as usize + 0x0C) as *const u32) };
        if hash != 0 {
            if let Ok(mut g) = STATIONS.lock() {
                let map = g.get_or_insert_with(HashMap::new);
                map.entry(trigger as usize).or_default().on_event(hash);
            }
        }
    }
    if let Some(&orig) = ORIG_REPAIR_HANDLER.get() {
        unsafe { orig(trigger, edx, event) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn station_area_test(
    pos: *const c_void,
    area: *const c_void,
    entity: usize,
    trigger: usize,
) -> u8 {
    type AreaTestFn = unsafe extern "cdecl" fn(*const c_void, *const c_void) -> bool;
    let orig: AreaTestFn = unsafe { std::mem::transmute(REPAIR_AREA_TEST_FN) };
    if !unsafe { orig(pos, area) } {
        return 0;
    }
    if !is_pacific_active() || trigger < 0x10000 || entity < 0x10000 || unsafe { station_dps(trigger) } >= 0.0 {
        return 1;
    }
    let team = unsafe { *((entity + 0x40) as *const i32) };
    let data = unsafe { *((entity + 0x0C) as *const usize) };
    let sid = if data >= 0x10000 {
        let p = unsafe { *((data + 0x210) as *const *const u8) };
        unsafe { crate::win32::from_ansi_ptr(p) }.unwrap_or_default()
    } else {
        String::new()
    };
    let ok = STATIONS
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(&trigger).cloned()))
        .unwrap_or_default()
        .repairs(team, &sid);
    u8::from(ok)
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_ai_update(self_ptr: *mut c_void, edx: *mut c_void, dt: u32) {
    if let Some(&orig) = ORIG_AI_UPDATE.get() {
        unsafe { orig(self_ptr, edx, dt) };
    }
    if !GROUND_PROBE_WANT.load(Ordering::Acquire) {
        return;
    }
    let at = {
        let Ok(gp) = GROUND_PROBE.lock() else { return };
        if !gp.want {
            return;
        }
        gp.want_at
    };
    let ai = unsafe { *(AI_MANAGER_GLOBAL as *const usize) };
    if ai < 0x10000 {
        return;
    }
    let holder = unsafe { *((ai + 0x130) as *const *mut c_void) };
    if holder.is_null() || unsafe { *(holder as *const usize) } == 0 {
        return;
    }
    #[repr(align(16))]
    struct Vec4([f32; 4]);
    let start = Vec4([at[0], at[1] - GEAR_PROBE_DROP, at[2], 0.0]);
    let down = Vec4([0.0, -1.0, 0.0, 0.0]);
    let mut hit_pos = Vec4([0.0; 4]);
    let mut hit_norm = Vec4([0.0; 4]);
    type SphereCastFn = unsafe extern "fastcall" fn(
        *mut c_void,
        *mut c_void,
        *const f32,
        *const f32,
        f32,
        f32,
        *mut f32,
        *mut f32,
    ) -> bool;
    let cast: SphereCastFn = unsafe { std::mem::transmute(SPHERE_CAST_FN) };
    let hit = unsafe {
        cast(
            holder,
            std::ptr::null_mut(),
            start.0.as_ptr(),
            down.0.as_ptr(),
            0.05,
            GEAR_PROBE_LENGTH,
            hit_pos.0.as_mut_ptr(),
            hit_norm.0.as_mut_ptr(),
        )
    };
    let h = if hit {
        (at[1] - hit_pos.0[1]).max(0.0)
    } else {
        GEAR_PROBE_LENGTH + GEAR_PROBE_DROP
    };
    if let Ok(mut gp) = GROUND_PROBE.lock() {
        gp.height = h;
        gp.height_time = Some(Instant::now());
    }
}

#[cfg(target_arch = "x86")]
pub unsafe fn install() -> usize {
    use crate::hooks;
    use crate::win32::{self, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE};

    let mut installed = 0usize;

    // 1. Shared client input-frame hook (`0x0081ADA8`) for Tank Horn + LCVP Boat Release
    if unsafe { hooks::bytes_match(RESTRICT_CALL, &RESTRICT_CALL_BYTES) } {
        let mut call = [0u8; 5];
        call[0] = 0xE8;
        let rel = (detour_restrict_input as *const () as isize - (RESTRICT_CALL as isize + 5)) as i32;
        call[1..5].copy_from_slice(&rel.to_le_bytes());
        if unsafe { hooks::write_code(RESTRICT_CALL, &call) } {
            installed += 1;
            crate::log_line("bf1943: input-frame hook installed (Tank Horn [H] + LCVP throttle release)");
        }
    }

    // 2. ServerVehicleSpawnEntity update slot 45 for LCVP Boat Release
    let spawner_slot = SPAWNER_VTABLE + 4 * SPAWNER_UPDATE_SLOT;
    if unsafe { *(spawner_slot as *const usize) } == SPAWNER_UPDATE_FN {
        let addr = (detour_spawner_update as *const () as usize).to_le_bytes();
        if unsafe { hooks::write_code(spawner_slot, &addr) } {
            installed += 1;
            crate::log_line("bf1943: LCVP boat-release spawner slot hooked");
        }
    }

    // 3. Airfield Repair Stations (`0x00729C90` event handler + `0x007283CB` area test)
    if unsafe { hooks::bytes_match(REPAIR_AREA_CALL, &REPAIR_AREA_CALL_BYTES) } {
        if let Some(tramp) = unsafe {
            hooks::install_detour(
                REPAIR_HANDLER,
                &REPAIR_HANDLER_PROLOGUE,
                detour_repair_handler as *const c_void,
            )
        } {
            let _ = ORIG_REPAIR_HANDLER
                .set(unsafe { std::mem::transmute::<*const c_void, RepairHandlerFn>(tramp) });
            // Allocate tiny cdecl thunk: push [esp+0x20]; push esi; push [esp+0x10]; push [esp+0x10]; call station_area_test; add esp, 0x10; ret
            let thunk = unsafe {
                win32::VirtualAlloc(
                    std::ptr::null_mut(),
                    64,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                ) as *mut u8
            };
            if !thunk.is_null() {
                let mut code = Vec::with_capacity(32);
                code.extend_from_slice(&[0xFF, 0x74, 0x24, 0x20]); // push dword ptr [esp+0x20]
                code.push(0x56); // push esi
                code.extend_from_slice(&[0xFF, 0x74, 0x24, 0x10]); // push dword ptr [esp+0x10]
                code.extend_from_slice(&[0xFF, 0x74, 0x24, 0x10]); // push dword ptr [esp+0x10]
                code.push(0xB8); // mov eax, station_area_test
                code.extend_from_slice(&(station_area_test as *const () as u32).to_le_bytes());
                code.extend_from_slice(&[0xFF, 0xD0]); // call eax
                code.extend_from_slice(&[0x83, 0xC4, 0x10]); // add esp, 0x10
                code.push(0xC3); // ret
                unsafe {
                    std::ptr::copy_nonoverlapping(code.as_ptr(), thunk, code.len());
                    win32::FlushInstructionCache(win32::GetCurrentProcess(), thunk.cast(), code.len());
                }
                let mut call = [0u8; 5];
                call[0] = 0xE8;
                let rel = (thunk as isize - (REPAIR_AREA_CALL as isize + 5)) as i32;
                call[1..5].copy_from_slice(&rel.to_le_bytes());
                if unsafe { hooks::write_code(REPAIR_AREA_CALL, &call) } {
                    installed += 1;
                    crate::log_line("bf1943: airfield repair stations hooked");
                }
            }
        }
    }

    // 4. Landing gear server-side 60m ground-height probe (`0x0059E190` AI update)
    if let Some(tramp) = unsafe {
        hooks::install_detour(
            AI_UPDATE,
            &AI_UPDATE_PROLOGUE,
            detour_ai_update as *const c_void,
        )
    } {
        let _ = ORIG_AI_UPDATE
            .set(unsafe { std::mem::transmute::<*const c_void, AiUpdateFn>(tramp) });
        installed += 1;
        crate::log_line("bf1943: landing gear 60m ground-height probe installed");
    }

    installed
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn install() -> usize {
    0
}
