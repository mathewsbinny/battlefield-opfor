//! Battlefield 1943 Air Raid & BF1943-only DBX Class Registration for `opfor-host`.
//!
//! Ported directly from `opfor-airraid/src/dll/airraid.cpp` + `airraid_rules.h` for build 795745:
//! - Registers 3 BF1943-only DBX classes into Frostbite 1.5's runtime reflection table (`0x004872C0`)
//!   before the type map & isKindOf ranges are built:
//!   1. `GameSharedResources.TeamVehicleProjectileEntityData` (+ server entity creator & lifecycle)
//!   2. `GameSharedResources.AirRaidEffectsData` (subclass of `WeaponEffectsData`)
//!   3. `GameSharedResources.ScorePartComponentData` (subclass of `PartComponentData` + server/client creators)
//! - Hooks the minimap icon selector (`0x0090A920`) so BF1943 Air Raid bombers (`type 6`, always visible)
//!   and Air Raid bunkers (`type 7`) show their authentic Pacific War minimap icons.
//! - Drives the Air Raid bunker seat auto-fire (`AutoFireDelay = 5.0s`, `EiaFire = 8`) from `bf1943.rs`.

#![allow(dead_code, non_snake_case, static_mut_refs)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

// Marker checked by Wake Island / BF1943 installers that emit native Air Raid DBX classes.
#[used]
pub static AIRRAID_DLL_MARKER: &[u8] =
    b"OPFOR-AIRRAID-CLASSES-2 TeamVehicleProjectileEntityData AirRaidEffectsData ScorePartComponentData\0";

// --- BFBC2Game.exe build 795745 addresses ---
const TYPE_MAP_BUILD: usize = 0x0048_72C0;
const TYPE_MAP_BUILD_BYTES: [u8; 9] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8, 0x83, 0xEC, 0x2C];
const CLASS_INFO_CTOR: usize = 0x0050_0880;
const PROJECTILE_TYPE: usize = 0x01BF_BFD8;
const VEHICLE_ASSET_TYPE: usize = 0x01C0_6E18;
const FLOAT32_TYPE: usize = 0x01BF_03E4;
const BOOLEAN_TYPE: usize = 0x01BF_0354;
const ARRAY_BASE_TYPE: usize = 0x01BF_02DC;
const STRING_TYPE: usize = 0x01BF_0404;
const TYPE_INFO_MODULE: u32 = 0x0151_65DC;

const VP_DATA_CTOR: usize = 0x0139_CEE0;
const VP_DATA_NEW: usize = 0x00B3_9F30;
const ALLOC_FN: usize = 0x004E_4EB0;
const REF_ARRAY_INIT: usize = 0x004E_4910;
const REF_ARRAY_VTABLE: u32 = 0x0146_423C;
const PROJECTILE_DATA_VTABLE: usize = 0x0145_EB3C;

const VP_CREATOR_VTABLE: usize = 0x0142_7720;
const CREATOR_REGISTRY: usize = 0x0071_C7F0;
const REGISTER_CREATOR: usize = 0x0071_C790;
const VP_CREATE: usize = 0x0071_37B0;
const VP_ENTITY_VTABLE: usize = 0x0142_8B60;
const SLOT_UPDATE: usize = 43;
const SLOT_SPAWN: usize = 58;
const ENTITY_SLOTS: usize = 62;
const VP_UPDATE: usize = 0x0071_1460;
const VP_SPAWN: usize = 0x0072_3E80;

const HANDLE_OWNER: usize = 0x0054_A4C0;
const HANDLE_VEHICLE: usize = 0x0054_2620;
const ENTER_VEHICLE: usize = 0x0057_25E0;
const EXIT_VEHICLE: usize = 0x0056_D480;
const DESTROY_SLOT: usize = 0x98 / 4;
const GAME_CONTEXT: usize = 0x0054_7BF0;

const DATA_SIZE: usize = 0xD0;
const OFF_TIME_TO_LIVE: usize = 0xA0;
const OFF_EXIT_AFTER_FIRE: usize = 0xB0;
const OFF_ENTER_DELAY: usize = 0xB4;
const OFF_VEHICLES: usize = 0xB8;
const OFF_UNSPAWN_ON_DESTROY: usize = 0xCC;

const ENTITY_DATA: usize = 0x0C;
const ENTITY_OWNER: usize = 0x50;
const ENTITY_VEHICLE: usize = 0x80;
const PLAYER_TEAM: usize = 0xA8;
const PLAYER_VEHICLE: usize = 0xC6C;
const MAX_PROJECTILES_IN_WORLD: usize = 1;

const WEAPON_CHANNEL: u32 = 0x1A;
const WEAPON_FIRED: u32 = 0x12;

// AirRaidEffectsData
const WEAPON_EFFECTS_TYPE: usize = 0x01BF_C464;
const WEAPON_EFFECTS_NEW: usize = 0x0052_F2E0;
const WEAPON_EFFECTS_VTABLE: usize = 0x0145_ECA8;

// ScorePartComponentData
const PART_TYPE: usize = 0x01BF_F308;
const PART_NEW: usize = 0x00B4_BF80;
const PART_DATA_VTABLE: usize = 0x0146_4268;
const PART_SERVER_CREATOR: usize = 0x0142_6A80;
const PART_CLIENT_CREATOR: usize = 0x0143_5144;
const CLIENT_REGISTRY: usize = 0x008B_6B10;
const REGISTER_CLIENT_CREATOR: usize = 0x008B_6A60;
const EMPTY_STRING: u32 = 0x0154_C450;
const SCORE_SIZE: usize = 0xC0;
const OFF_SCORE_STRING: usize = 0xB0;
const OFF_SCORE: usize = 0xBC;

// Minimap selector (`0x0090A920`)
const MINIMAP_SELECTOR: usize = 0x0090_A920;
const MINIMAP_SELECTOR_BYTES: [u8; 6] = [0x51, 0x53, 0x8B, 0x5C, 0x24, 0x0C];
const TEAM_TEST: usize = 0x008F_F9E0;
const ENTITY_DATA_SLOT: usize = 0x120;
const HUD_VEHICLE_TYPE: usize = 0x208;

const HUD_BOMB_RAID: i32 = 6;
const HUD_BUNKER: i32 = 7;
const BC2_ICON_UAV: i32 = 0x76;
const BC2_ICON_AIRPLANE: i32 = 0x7C;

pub const EIA_FIRE: i32 = 8;
const AUTO_FIRE_DELAY: f32 = 5.0;
const AUTO_FIRE_PRESS: f32 = 0.25;

// --- Reflection records (24 bytes each, matching BFBC2 build 795745) ---
#[repr(C)]
struct TypeInfo {
    name: *const u8,
    category: u16,
    size: u16,
    module: u32,
    flags: u32,
    zero: [u32; 2],
}
unsafe impl Sync for TypeInfo {}

#[repr(C)]
struct FieldInfo {
    name: *const u8,
    flags: u32,
    ty: usize,
    element: usize,
    offset: u32,
    zero: u32,
}
unsafe impl Sync for FieldInfo {}

#[repr(C, align(16))]
struct AlignedClassInfo([u8; 0x40]);

static TYPE_INFO_TVP: TypeInfo = TypeInfo {
    name: b"TeamVehicleProjectileEntityData\0".as_ptr(),
    category: 0x35,
    size: DATA_SIZE as u16,
    module: TYPE_INFO_MODULE,
    flags: 0x0001_0410,
    zero: [0, 0],
};

static FIELDS_TVP: [FieldInfo; 4] = [
    FieldInfo {
        name: b"Vehicles\0".as_ptr(),
        flags: 0x48,
        ty: ARRAY_BASE_TYPE,
        element: VEHICLE_ASSET_TYPE,
        offset: OFF_VEHICLES as u32,
        zero: 0,
    },
    FieldInfo {
        name: b"EnterDelay\0".as_ptr(),
        flags: 0,
        ty: FLOAT32_TYPE,
        element: 0,
        offset: OFF_ENTER_DELAY as u32,
        zero: 0,
    },
    FieldInfo {
        name: b"ExitAfterFireDelay\0".as_ptr(),
        flags: 0,
        ty: FLOAT32_TYPE,
        element: 0,
        offset: OFF_EXIT_AFTER_FIRE as u32,
        zero: 0,
    },
    FieldInfo {
        name: b"OnProjectileDestroyedUnspawnVehicle\0".as_ptr(),
        flags: 0,
        ty: BOOLEAN_TYPE,
        element: 0,
        offset: OFF_UNSPAWN_ON_DESTROY as u32,
        zero: 0,
    },
];

static TYPE_INFO_FX: TypeInfo = TypeInfo {
    name: b"AirRaidEffectsData\0".as_ptr(),
    category: 0x35,
    size: 0x1C,
    module: TYPE_INFO_MODULE,
    flags: 0x0001_0004,
    zero: [0, 0],
};

static TYPE_INFO_SCORE: TypeInfo = TypeInfo {
    name: b"ScorePartComponentData\0".as_ptr(),
    category: 0x35,
    size: SCORE_SIZE as u16,
    module: TYPE_INFO_MODULE,
    flags: 0x0001_0210,
    zero: [0, 0],
};

static FIELDS_SCORE: [FieldInfo; 2] = [
    FieldInfo {
        name: b"ScoreString\0".as_ptr(),
        flags: 0,
        ty: STRING_TYPE,
        element: 0,
        offset: OFF_SCORE_STRING as u32,
        zero: 0,
    },
    FieldInfo {
        name: b"Score\0".as_ptr(),
        flags: 0,
        ty: FLOAT32_TYPE,
        element: 0,
        offset: OFF_SCORE as u32,
        zero: 0,
    },
];

static mut CLASS_INFO_TVP: AlignedClassInfo = AlignedClassInfo([0; 0x40]);
static mut DATA_VTABLE_TVP: [*mut c_void; 7] = [std::ptr::null_mut(); 7];
static mut CREATOR_VTABLE_TVP: [*mut c_void; 5] = [std::ptr::null_mut(); 5];
static mut CREATOR_TVP: [*mut c_void; 1] = [std::ptr::null_mut(); 1];
static mut ENTITY_VTABLE_TVP: [*mut c_void; ENTITY_SLOTS] = [std::ptr::null_mut(); ENTITY_SLOTS];

static mut CLASS_INFO_FX: AlignedClassInfo = AlignedClassInfo([0; 0x40]);
static mut DATA_VTABLE_FX: [*mut c_void; 7] = [std::ptr::null_mut(); 7];

static mut CLASS_INFO_SCORE: AlignedClassInfo = AlignedClassInfo([0; 0x40]);
static mut DATA_VTABLE_SCORE: [*mut c_void; 7] = [std::ptr::null_mut(); 7];
static mut SERVER_VTABLE_SCORE: [*mut c_void; 5] = [std::ptr::null_mut(); 5];
static mut CLIENT_VTABLE_SCORE: [*mut c_void; 5] = [std::ptr::null_mut(); 5];
static mut SERVER_CREATOR_SCORE: [*mut c_void; 1] = [std::ptr::null_mut(); 1];
static mut CLIENT_CREATOR_SCORE: [*mut c_void; 1] = [std::ptr::null_mut(); 1];

static REGISTERED: AtomicBool = AtomicBool::new(false);
static LISTENING: AtomicBool = AtomicBool::new(false);
static ICON_LOGGED_RAID: AtomicBool = AtomicBool::new(false);
static ICON_LOGGED_BUNKER: AtomicBool = AtomicBool::new(false);

type TypeMapFn = unsafe extern "C" fn() -> i32;
static ORIG_TYPE_MAP_BUILD: OnceLock<TypeMapFn> = OnceLock::new();

type SelectorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut i32, *mut u8, *mut u8) -> bool;
static ORIG_SELECTOR: OnceLock<SelectorFn> = OnceLock::new();

#[inline]
fn pacific_active() -> bool {
    crate::hooks::is_pacific_active()
}

// --- Pure Air Raid Lifecycle Rules (from airraid_rules.h) ---
const ACT_ENTER: u32 = 1 << 0;
const ACT_WATCH_FIRE: u32 = 1 << 1;
const ACT_TIMEOUT_MSG: u32 = 1 << 2;
const ACT_STOP_WATCH: u32 = 1 << 3;
const ACT_UNSPAWN_VEH: u32 = 1 << 4;
const ACT_EXIT_OWNER: u32 = 1 << 5;
const ACT_UNSPAWN_OWNER_VEH: u32 = 1 << 6;
const ACT_DESTROY_PROJ: u32 = 1 << 7;

#[derive(Clone, Copy, Default)]
struct ProjState {
    age: f32,
    last_fire_age: f32,
    entered: bool,
    done: bool,
}

#[derive(Clone, Copy)]
struct LiveSlot {
    entity: *mut c_void,
    data: *mut c_void,
    state: ProjState,
    watched: *mut c_void,
}

impl LiveSlot {
    const fn empty() -> Self {
        Self {
            entity: std::ptr::null_mut(),
            data: std::ptr::null_mut(),
            state: ProjState {
                age: 0.0,
                last_fire_age: 0.0,
                entered: false,
                done: false,
            },
            watched: std::ptr::null_mut(),
        }
    }
}

static mut LIVE_SLOTS: [LiveSlot; 16] = [LiveSlot::empty(); 16];

#[derive(Clone, Copy)]
struct CarrierSlot {
    data: *mut c_void,
    index: u32,
    vp: *mut c_void,
}

static mut CARRIER_SLOTS: [CarrierSlot; 8] = [CarrierSlot {
    data: std::ptr::null_mut(),
    index: 0,
    vp: std::ptr::null_mut(),
}; 8];

#[repr(C)]
struct FireListener {
    vtable: *mut *mut c_void,
    room: [u8; 60],
}
static mut LISTENER_VTABLE: [*mut c_void; 3] = [std::ptr::null_mut(); 3];
static mut FIRE_LISTENER: FireListener = FireListener {
    vtable: std::ptr::null_mut(),
    room: [0; 60],
};

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn tvp_data_type(_self: *mut c_void) -> *mut c_void {
    (&raw mut CLASS_INFO_TVP).cast()
}

#[cfg(target_arch = "x86")]
unsafe extern "C" fn tvp_data_new(mut mem: *mut c_void) -> *mut c_void {
    type AllocFn = unsafe extern "C" fn(usize) -> *mut c_void;
    type CtorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, i32) -> *mut c_void;
    type RefArrInitFn = unsafe extern "fastcall" fn(*mut c_void);
    if mem.is_null() {
        let alloc: AllocFn = unsafe { std::mem::transmute(ALLOC_FN) };
        mem = unsafe { alloc(DATA_SIZE) };
    }
    if mem.is_null() {
        return std::ptr::null_mut();
    }
    let ctor: CtorFn = unsafe { std::mem::transmute(VP_DATA_CTOR) };
    unsafe {
        ctor(mem, std::ptr::null_mut(), 0);
        *(mem as *mut *mut c_void) = (&raw mut DATA_VTABLE_TVP).cast();
        *((mem as usize + OFF_EXIT_AFTER_FIRE) as *mut f32) = 0.0;
        *((mem as usize + OFF_ENTER_DELAY) as *mut f32) = 0.0;
        let arr = (mem as usize + OFF_VEHICLES) as *mut c_void;
        let init: RefArrInitFn = std::mem::transmute(REF_ARRAY_INIT);
        init(arr);
        *(arr as *mut u32) = REF_ARRAY_VTABLE;
        *((mem as usize + OFF_UNSPAWN_ON_DESTROY) as *mut u8) = 0;
    }
    mem
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn fx_data_type(_self: *mut c_void) -> *mut c_void {
    (&raw mut CLASS_INFO_FX).cast()
}

#[cfg(target_arch = "x86")]
unsafe extern "C" fn fx_data_new(mem: *mut c_void) -> *mut c_void {
    type NewFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
    let f: NewFn = unsafe { std::mem::transmute(WEAPON_EFFECTS_NEW) };
    let out = unsafe { f(mem) };
    if !out.is_null() {
        unsafe { *(out as *mut *mut c_void) = (&raw mut DATA_VTABLE_FX).cast() };
    }
    out
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn score_data_type(_self: *mut c_void) -> *mut c_void {
    (&raw mut CLASS_INFO_SCORE).cast()
}

#[cfg(target_arch = "x86")]
unsafe extern "C" fn score_data_new(mut mem: *mut c_void) -> *mut c_void {
    type AllocFn = unsafe extern "C" fn(usize) -> *mut c_void;
    type NewFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
    if mem.is_null() {
        let alloc: AllocFn = unsafe { std::mem::transmute(ALLOC_FN) };
        mem = unsafe { alloc(SCORE_SIZE) };
    }
    if mem.is_null() {
        return std::ptr::null_mut();
    }
    let part_new: NewFn = unsafe { std::mem::transmute(PART_NEW) };
    unsafe {
        part_new(mem);
        *(mem as *mut *mut c_void) = (&raw mut DATA_VTABLE_SCORE).cast();
        *((mem as usize + OFF_SCORE_STRING) as *mut u32) = EMPTY_STRING;
        *((mem as usize + OFF_SCORE_STRING + 4) as *mut u32) = EMPTY_STRING;
        *((mem as usize + OFF_SCORE_STRING + 8) as *mut u32) = EMPTY_STRING + 1;
        *((mem as usize + OFF_SCORE) as *mut f32) = 0.0;
    }
    mem
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn listener_dtor(self_ptr: *mut c_void, _edx: *mut c_void, _flags: i32) -> *mut c_void {
    self_ptr
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn listener_priority(_self: *mut c_void) -> u16 {
    0x64
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn listener_message(
    _self: *mut c_void,
    _edx: *mut c_void,
    channel: u32,
    id: u32,
    _a3: u32,
    msg: *mut c_void,
) {
    if channel != WEAPON_CHANNEL || id != WEAPON_FIRED || msg.is_null() {
        return;
    }
    let vt = unsafe { *(msg as *const *const usize) };
    type GetVehFn = unsafe extern "fastcall" fn(*mut c_void) -> *mut c_void;
    let get_veh: GetVehFn = unsafe { std::mem::transmute(*vt.add(0x2C / 4)) };
    let vehicle = unsafe { get_veh(msg) };
    unsafe {
        for slot in &mut LIVE_SLOTS {
            if !slot.entity.is_null()
                && !slot.watched.is_null()
                && slot.watched == vehicle
                && slot.state.entered
                && !slot.state.done
            {
                slot.state.last_fire_age = slot.state.age;
                crate::log_line(&format!(
                    "air raid: t={:.2} bomb release (projectile {:p})",
                    slot.state.age, slot.entity
                ));
            }
        }
    }
}

#[cfg(target_arch = "x86")]
unsafe fn subscribe_listener() {
    if LISTENING.swap(true, Ordering::AcqRel) {
        return;
    }
    unsafe {
        LISTENER_VTABLE[0] = listener_dtor as *mut c_void;
        LISTENER_VTABLE[1] = listener_message as *mut c_void;
        LISTENER_VTABLE[2] = listener_priority as *mut c_void;
        FIRE_LISTENER.vtable = (&raw mut LISTENER_VTABLE).cast();
        type CtxFn = unsafe extern "C" fn() -> *mut c_void;
        let get_ctx: CtxFn = std::mem::transmute(GAME_CONTEXT);
        let ctx = get_ctx();
        let mgr = if ctx.is_null() {
            std::ptr::null_mut()
        } else {
            *((ctx as usize + 0x18) as *const *mut c_void)
        };
        if mgr.is_null() {
            LISTENING.store(false, Ordering::Release);
            return;
        }
        let vt = *(mgr as *const *const usize);
        type SubFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, u32, *mut c_void);
        let sub: SubFn = std::mem::transmute(*vt.add(2));
        sub(mgr, std::ptr::null_mut(), WEAPON_CHANNEL, (&raw mut FIRE_LISTENER).cast());
        crate::log_line("air raid: release listener subscribed (channel 0x1A, weapon fired 0x12)");
    }
}

#[cfg(target_arch = "x86")]
unsafe fn carrier_for(data: *mut c_void, index: u32) -> *mut c_void {
    unsafe {
        for c in &CARRIER_SLOTS {
            if !c.vp.is_null() && c.data == data && c.index == index {
                return c.vp;
            }
        }
        type NewFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
        let new_vp: NewFn = std::mem::transmute(VP_DATA_NEW);
        let vp = new_vp(std::ptr::null_mut());
        if vp.is_null() {
            return std::ptr::null_mut();
        }
        std::ptr::copy_nonoverlapping(
            (data as *const u8).add(0x0C),
            (vp as *mut u8).add(0x0C),
            OFF_EXIT_AFTER_FIRE - 0x0C,
        );
        *((vp as usize + 0xB0) as *mut f32) = *((data as usize + OFF_ENTER_DELAY) as *const f32);
        let arr_ptr = *((data as usize + OFF_VEHICLES + 0x10) as *const *const *mut c_void);
        *((vp as usize + 0xB4) as *mut *mut c_void) = *arr_ptr.add(index as usize);
        for c in &mut CARRIER_SLOTS {
            if c.vp.is_null() {
                *c = CarrierSlot { data, index, vp };
                break;
            }
        }
        vp
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn spawn_detour(
    self_ptr: *mut c_void,
    _edx: *mut c_void,
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
    a8: u32,
    a9: u32,
    a10: u32,
) {
    type HandleFn = unsafe extern "fastcall" fn(*mut c_void) -> *mut c_void;
    type SpawnFn = unsafe extern "fastcall" fn(
        *mut c_void, *mut c_void, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32,
    );
    unsafe {
        let data = *((self_ptr as usize + ENTITY_DATA) as *const *mut c_void);
        let mut slot_idx = None;
        for (i, s) in LIVE_SLOTS.iter_mut().enumerate() {
            if s.entity == self_ptr {
                *s = LiveSlot::empty();
                s.entity = self_ptr;
                slot_idx = Some(i);
                break;
            }
        }
        if slot_idx.is_none() {
            for (i, s) in LIVE_SLOTS.iter_mut().enumerate() {
                if s.entity.is_null() {
                    *s = LiveSlot::empty();
                    s.entity = self_ptr;
                    slot_idx = Some(i);
                    break;
                }
            }
        }
        let get_owner: HandleFn = std::mem::transmute(HANDLE_OWNER);
        let player = get_owner((self_ptr as usize + ENTITY_OWNER) as *mut c_void);
        let team = if player.is_null() {
            0
        } else {
            *((player as usize + PLAYER_TEAM) as *const i32)
        };
        let v_count = *((data as usize + OFF_VEHICLES + 0x0C) as *const u32) as i32;
        let idx = if team > 0 && team <= v_count { team - 1 } else { -1 };
        let others = LIVE_SLOTS
            .iter()
            .filter(|s| !s.entity.is_null() && s.data == data)
            .count();
        if let Some(i) = slot_idx {
            LIVE_SLOTS[i].data = data;
        }
        if idx < 0 || others >= MAX_PROJECTILES_IN_WORLD {
            return;
        }
        let vp = carrier_for(data, idx as u32);
        if vp.is_null() {
            return;
        }
        *((self_ptr as usize + ENTITY_DATA) as *mut *mut c_void) = vp;
        let orig_spawn: SpawnFn = std::mem::transmute(VP_SPAWN);
        orig_spawn(self_ptr, std::ptr::null_mut(), a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10);
        *((self_ptr as usize + ENTITY_DATA) as *mut *mut c_void) = data;
        crate::log_line(&format!("air raid: fired by team {team} -> bombers Vehicles[{idx}]"));
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn update_detour(self_ptr: *mut c_void, _edx: *mut c_void, dt_ptr: *const f32) {
    type HandleFn = unsafe extern "fastcall" fn(*mut c_void) -> *mut c_void;
    type UpdateFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const f32);
    type EnterFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, i32);
    type ExitFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, i32);
    type DestroyFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, i32);

    unsafe {
        let data = *((self_ptr as usize + ENTITY_DATA) as *const *mut c_void);
        let mut slot_ptr: *mut LiveSlot = std::ptr::null_mut();
        for s in &mut LIVE_SLOTS {
            if s.entity == self_ptr {
                slot_ptr = s;
                break;
            }
        }
        if slot_ptr.is_null() {
            let orig: UpdateFn = std::mem::transmute(VP_UPDATE);
            orig(self_ptr, std::ptr::null_mut(), dt_ptr);
            return;
        }
        let slot = &mut *slot_ptr;
        slot.data = data;
        let exit_delay = *((data as usize + OFF_EXIT_AFTER_FIRE) as *const f32);
        let enter_delay = *((data as usize + OFF_ENTER_DELAY) as *const f32);
        let ttl = *((data as usize + OFF_TIME_TO_LIVE) as *const f32);
        let unspawn_on_destroy = *((data as usize + OFF_UNSPAWN_ON_DESTROY) as *const u8) != 0;

        let get_owner: HandleFn = std::mem::transmute(HANDLE_OWNER);
        let get_veh: HandleFn = std::mem::transmute(HANDLE_VEHICLE);
        let player = get_owner((self_ptr as usize + ENTITY_OWNER) as *mut c_void);
        let vehicle = get_veh((self_ptr as usize + ENTITY_VEHICLE) as *mut c_void);
        let owner_in_veh = !player.is_null() && !(*((player as usize + PLAYER_VEHICLE) as *const *mut c_void)).is_null();

        if slot.state.entered && !LISTENING.load(Ordering::Acquire) {
            subscribe_listener();
        }

        let dt = *dt_ptr;
        let mut act = 0u32;
        if player.is_null() {
            if !vehicle.is_null() {
                act |= ACT_STOP_WATCH | ACT_UNSPAWN_VEH;
            }
            act |= ACT_DESTROY_PROJ;
        } else if !vehicle.is_null() {
            slot.state.age += dt;
            if !slot.state.entered && enter_delay < slot.state.age {
                act |= ACT_ENTER | ACT_WATCH_FIRE;
                slot.state.entered = true;
            }
            if ttl != 0.0 && ttl < slot.state.age {
                act |= ACT_TIMEOUT_MSG | ACT_STOP_WATCH | ACT_UNSPAWN_VEH;
            }
            if !slot.state.done
                && slot.state.entered
                && exit_delay >= 0.0
                && slot.state.last_fire_age != 0.0
                && (slot.state.last_fire_age - (slot.state.age - exit_delay)) < 0.0
            {
                act |= ACT_EXIT_OWNER;
                if unspawn_on_destroy && owner_in_veh {
                    act |= ACT_UNSPAWN_OWNER_VEH;
                }
                slot.state.done = true;
            }
        } else {
            if !slot.state.done && unspawn_on_destroy && owner_in_veh {
                act |= ACT_UNSPAWN_OWNER_VEH;
            }
            act |= ACT_DESTROY_PROJ;
        }

        if (act & ACT_ENTER) != 0 && !player.is_null() && !vehicle.is_null() {
            let enter: EnterFn = std::mem::transmute(ENTER_VEHICLE);
            enter(player, std::ptr::null_mut(), vehicle, 0);
        }
        if (act & ACT_WATCH_FIRE) != 0 {
            slot.watched = vehicle;
            subscribe_listener();
        }
        if (act & ACT_EXIT_OWNER) != 0 && !player.is_null() {
            let exit_v: ExitFn = std::mem::transmute(EXIT_VEHICLE);
            exit_v(player, std::ptr::null_mut(), 1);
        }
        if (act & (ACT_UNSPAWN_VEH | ACT_UNSPAWN_OWNER_VEH)) != 0 && !vehicle.is_null() {
            let vt = *(vehicle as *const *const usize);
            let destroy: DestroyFn = std::mem::transmute(*vt.add(DESTROY_SLOT));
            destroy(vehicle, std::ptr::null_mut(), 1);
        }
        if (act & ACT_DESTROY_PROJ) != 0 {
            *slot = LiveSlot::empty();
            let vt = *(self_ptr as *const *const usize);
            let destroy: DestroyFn = std::mem::transmute(*vt.add(DESTROY_SLOT));
            destroy(self_ptr, std::ptr::null_mut(), 1);
        }
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn creator_create(
    creator: *mut c_void,
    _edx: *mut c_void,
    a1: *mut c_void,
    a2: *mut c_void,
) -> *mut c_void {
    type CreateFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
    let create: CreateFn = unsafe { std::mem::transmute(VP_CREATE) };
    let e = unsafe { create(creator, std::ptr::null_mut(), a1, a2) };
    if !e.is_null() {
        unsafe { *(e as *mut *mut c_void) = (&raw mut ENTITY_VTABLE_TVP).cast() };
    }
    e
}

#[cfg(target_arch = "x86")]
unsafe fn register_classes() -> bool {
    type CtorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const TypeInfo, usize, *const c_void, *const FieldInfo) -> *mut c_void;
    type RegGetFn = unsafe extern "C" fn() -> *mut c_void;
    type RegAddFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void);

    unsafe {
        // 1. TeamVehicleProjectileEntityData
        std::ptr::copy_nonoverlapping(PROJECTILE_DATA_VTABLE as *const *mut c_void, DATA_VTABLE_TVP.as_mut_ptr(), 7);
        DATA_VTABLE_TVP[2] = tvp_data_type as *mut c_void;
        let ctor: CtorFn = std::mem::transmute(CLASS_INFO_CTOR);
        ctor(
            (&raw mut CLASS_INFO_TVP).cast(),
            std::ptr::null_mut(),
            &TYPE_INFO_TVP,
            PROJECTILE_TYPE,
            tvp_data_new as *const c_void,
            FIELDS_TVP.as_ptr(),
        );

        std::ptr::copy_nonoverlapping(VP_CREATOR_VTABLE as *const *mut c_void, CREATOR_VTABLE_TVP.as_mut_ptr(), 5);
        CREATOR_VTABLE_TVP[1] = tvp_data_type as *mut c_void;
        CREATOR_VTABLE_TVP[4] = creator_create as *mut c_void;
        CREATOR_TVP[0] = CREATOR_VTABLE_TVP.as_mut_ptr().cast();

        std::ptr::copy_nonoverlapping(VP_ENTITY_VTABLE as *const *mut c_void, ENTITY_VTABLE_TVP.as_mut_ptr(), ENTITY_SLOTS);
        if ENTITY_VTABLE_TVP[SLOT_UPDATE] as usize != VP_UPDATE || ENTITY_VTABLE_TVP[SLOT_SPAWN] as usize != VP_SPAWN {
            crate::log_line("air raid: VehicleProjectile vtable mismatch; skipping creator registration");
            return false;
        }
        ENTITY_VTABLE_TVP[SLOT_UPDATE] = update_detour as *mut c_void;
        ENTITY_VTABLE_TVP[SLOT_SPAWN] = spawn_detour as *mut c_void;
        let get_reg: RegGetFn = std::mem::transmute(CREATOR_REGISTRY);
        let add_reg: RegAddFn = std::mem::transmute(REGISTER_CREATOR);
        let registry = get_reg();
        add_reg(registry, std::ptr::null_mut(), CREATOR_TVP.as_mut_ptr().cast());

        // 2. AirRaidEffectsData
        std::ptr::copy_nonoverlapping(WEAPON_EFFECTS_VTABLE as *const *mut c_void, DATA_VTABLE_FX.as_mut_ptr(), 7);
        DATA_VTABLE_FX[2] = fx_data_type as *mut c_void;
        ctor(
            (&raw mut CLASS_INFO_FX).cast(),
            std::ptr::null_mut(),
            &TYPE_INFO_FX,
            WEAPON_EFFECTS_TYPE,
            fx_data_new as *const c_void,
            std::ptr::null(),
        );

        // 3. ScorePartComponentData (+ server & client creators)
        std::ptr::copy_nonoverlapping(PART_DATA_VTABLE as *const *mut c_void, DATA_VTABLE_SCORE.as_mut_ptr(), 7);
        DATA_VTABLE_SCORE[2] = score_data_type as *mut c_void;
        ctor(
            (&raw mut CLASS_INFO_SCORE).cast(),
            std::ptr::null_mut(),
            &TYPE_INFO_SCORE,
            PART_TYPE,
            score_data_new as *const c_void,
            FIELDS_SCORE.as_ptr(),
        );
        std::ptr::copy_nonoverlapping(PART_SERVER_CREATOR as *const *mut c_void, SERVER_VTABLE_SCORE.as_mut_ptr(), 5);
        std::ptr::copy_nonoverlapping(PART_CLIENT_CREATOR as *const *mut c_void, CLIENT_VTABLE_SCORE.as_mut_ptr(), 5);
        SERVER_VTABLE_SCORE[1] = score_data_type as *mut c_void;
        CLIENT_VTABLE_SCORE[1] = score_data_type as *mut c_void;
        SERVER_CREATOR_SCORE[0] = SERVER_VTABLE_SCORE.as_mut_ptr().cast();
        CLIENT_CREATOR_SCORE[0] = CLIENT_VTABLE_SCORE.as_mut_ptr().cast();
        add_reg(registry, std::ptr::null_mut(), SERVER_CREATOR_SCORE.as_mut_ptr().cast());
        let get_cli: RegGetFn = std::mem::transmute(CLIENT_REGISTRY);
        let add_cli: RegAddFn = std::mem::transmute(REGISTER_CLIENT_CREATOR);
        add_cli(get_cli(), std::ptr::null_mut(), CLIENT_CREATOR_SCORE.as_mut_ptr().cast());
    }
    true
}

#[cfg(target_arch = "x86")]
unsafe extern "C" fn detour_type_map_build() -> i32 {
    if !REGISTERED.swap(true, Ordering::AcqRel) {
        let ok = unsafe { register_classes() };
        crate::log_line(&format!(
            "air raid: registered TeamVehicleProjectileEntityData + AirRaidEffectsData + ScorePartComponentData (ok={ok})"
        ));
    }
    if let Some(&orig) = ORIG_TYPE_MAP_BUILD.get() {
        unsafe { orig() }
    } else {
        0
    }
}

fn bc2_minimap_icon(hud_type: i32, enemy: bool, squad: bool, friendly: bool) -> i32 {
    let base = if hud_type == HUD_BOMB_RAID {
        BC2_ICON_UAV
    } else if hud_type == HUD_BUNKER {
        BC2_ICON_AIRPLANE
    } else {
        return -1;
    };
    if enemy {
        base + 2
    } else if squad {
        base + 3
    } else if friendly {
        base + 1
    } else {
        base
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_minimap_selector(
    hud: *mut c_void,
    _edx: *mut c_void,
    entity: *mut c_void,
    icon: *mut i32,
    a3: *mut u8,
    a4: *mut u8,
) -> bool {
    if !entity.is_null() && pacific_active() {
        unsafe {
            let slot = (entity as usize + ENTITY_DATA_SLOT) as *mut c_void;
            let vt = *(slot as *const *const usize);
            type GetDataFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> *const u8;
            let get_data: GetDataFn = std::mem::transmute(*vt);
            let data = get_data(slot, std::ptr::null_mut());
            if !data.is_null() {
                let hud_type = *(data.add(HUD_VEHICLE_TYPE) as *const i32);
                if hud_type == HUD_BOMB_RAID || hud_type == HUD_BUNKER {
                    let mut friendly = 0u8;
                    let mut squad = 0u8;
                    *a4 = 0;
                    type TeamTestFn = unsafe extern "fastcall" fn(
                        *mut c_void, *mut c_void, *mut c_void, *mut u8, *mut u8, *mut u8, *mut u8,
                    );
                    let team_test: TeamTestFn = std::mem::transmute(TEAM_TEST);
                    team_test(hud, std::ptr::null_mut(), entity, a4, &mut friendly, &mut squad, a3);
                    *icon = bc2_minimap_icon(hud_type, *a4 != 0, squad != 0, friendly != 0);
                    if hud_type == HUD_BOMB_RAID {
                        *a4 = 0;
                        *a3 = 1;
                    }
                    let flag = if hud_type == HUD_BUNKER { &ICON_LOGGED_BUNKER } else { &ICON_LOGGED_RAID };
                    if !flag.swap(true, Ordering::AcqRel) {
                        crate::log_line(&format!(
                            "air raid: minimap icon 0x{:X} for vehicle type {hud_type}",
                            *icon
                        ));
                    }
                    return true;
                }
            }
        }
    }
    if let Some(&orig) = ORIG_SELECTOR.get() {
        unsafe { orig(hud, std::ptr::null_mut(), entity, icon, a3, a4) }
    } else {
        false
    }
}

// --- Bunker Auto-Fire Tick (called from bf1943.rs input-frame hook) ---
struct BunkerState {
    in_bunker: bool,
    armed: bool,
    timer: f32,
    press: f32,
    last_tick: Option<Instant>,
}

static BUNKER_STATE: std::sync::Mutex<BunkerState> = std::sync::Mutex::new(BunkerState {
    in_bunker: false,
    armed: false,
    timer: 0.0,
    press: 0.0,
    last_tick: None,
});

/// Called every client input frame from `bf1943.rs`. Returns `Some(fire_level)` when the local
/// player is seated in a BF1943 Air Raid bunker (`HudData.VehicleType == 7`).
pub fn tick_bunker_trigger(hud_type: i32) -> Option<bool> {
    let in_bunker = pacific_active() && hud_type == HUD_BUNKER;
    let Ok(mut st) = BUNKER_STATE.lock() else {
        return None;
    };
    let now = Instant::now();
    let dt = st
        .last_tick
        .map(|t| now.duration_since(t).as_secs_f32())
        .unwrap_or(0.0);
    st.last_tick = Some(now);
    let dt = if (0.0..=0.25).contains(&dt) { dt } else { 0.0 };

    if in_bunker && !st.in_bunker {
        st.armed = true;
        st.timer = AUTO_FIRE_DELAY;
        st.press = 0.0;
        crate::log_line("air raid: bunker seat taken -> auto-fire armed (5.0s)");
    } else if !in_bunker && st.in_bunker {
        if st.armed {
            crate::log_line("air raid: bunker left before auto-fire -> disarmed");
        }
        st.armed = false;
        st.timer = 0.0;
        st.press = 0.0;
    }
    st.in_bunker = in_bunker;
    if !in_bunker {
        return None;
    }
    if st.armed {
        st.timer -= dt;
        if st.timer <= 0.0 {
            st.armed = false;
            st.press = AUTO_FIRE_PRESS;
            crate::log_line("air raid: auto-fire -> bunker trigger pressed (0.25s)");
        }
    }
    let level = if st.press > 0.0 {
        st.press -= dt;
        true
    } else {
        false
    };
    Some(level)
}

#[cfg(target_arch = "x86")]
pub unsafe fn install() -> usize {
    use crate::hooks;
    let mut installed = 0usize;

    if let Some(tramp) = unsafe {
        hooks::install_detour(
            TYPE_MAP_BUILD,
            &TYPE_MAP_BUILD_BYTES,
            detour_type_map_build as *const c_void,
        )
    } {
        let _ = ORIG_TYPE_MAP_BUILD.set(unsafe { std::mem::transmute::<*const c_void, TypeMapFn>(tramp) });
        installed += 1;
        crate::log_line("air raid: type-map hook installed (TeamVehicleProjectileEntityData + AirRaidEffectsData + ScorePartComponentData)");
    }

    if let Some(tramp) = unsafe {
        hooks::install_detour(
            MINIMAP_SELECTOR,
            &MINIMAP_SELECTOR_BYTES,
            detour_minimap_selector as *const c_void,
        )
    } {
        let _ = ORIG_SELECTOR.set(unsafe { std::mem::transmute::<*const c_void, SelectorFn>(tramp) });
        installed += 1;
        crate::log_line("air raid: minimap icon selector hooked (0x0090A920)");
    }

    installed
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn install() -> usize {
    0
}
