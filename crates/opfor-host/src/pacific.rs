//! Pacific War (BF1943) visual & audio polish for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/gamemode.cpp`:
//! - Game Tips redirect (`0x00751BC2`, `0x007776EA`): `Menu/GameTips/pacific_GameTips` on WW2.
//! - Minimap combat-area overlay bypass (`0x00921CF7` -> `0x00921E29`) on WW2 maps.
//! - Wake ocean minimap extension shader (`0x00921CDB`, `Systems/UI/Shaders/PacificMinimap_Wake`).
//! - BF1943 award stings (`PlayUiSound` @ `0x00F909C0`) + HUD rank-up sting wrapper (vtable slot `0x0142B774`).

#![allow(dead_code, non_snake_case)]

use std::ffi::{CString, c_void};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use opfor_vfs::ModeId;

const TIPS_SITES: [usize; 2] = [0x0075_1BC2, 0x0077_76EA];
const TIPS_BYTES: [u8; 19] = [
    0x83, 0xBE, 0x8C, 0x8C, 0x0C, 0x00, 0x01, 0xB8, 0xCC, 0xAD, 0x42, 0x01, 0x74, 0x05, 0xB8,
    0xB4, 0xAD, 0x42, 0x01,
];
const TIPS_MOV_OFFSET: usize = 7;
const NAM_TIPS_PTR: usize = 0x0142_ADCC;

const MAP_IMAGE_START: usize = 0x0092_1CDB;
const MAP_OVERLAY_START: usize = 0x0092_1CF7;
const MAP_OVERLAY_AFTER: usize = 0x0092_1E29;
const MAP_OVERLAY_START_BYTES: [u8; 6] = [0x8B, 0x0D, 0xA8, 0x4C, 0x9A, 0x01];
const MAP_OVERLAY_SHADER_BYTES: [u8; 6] = [0x8B, 0x8E, 0xC4, 0x64, 0x02, 0x00];
const MAP_OVERLAY_END_BYTES: [u8; 3] = [0x83, 0x3E, 0x01];
const RENDERER_GETTER: usize = 0x00F8_2BC0;
const RENDERER_GETTER_BYTES: [u8; 4] = [0x8B, 0x41, 0x0C, 0xC3];
const SHADER_LOOKUP_SITE: usize = 0x0092_14B4;
const SHADER_LOOKUP_BYTES: [u8; 11] = [0x8B, 0x0D, 0x18, 0x2B, 0x57, 0x01, 0x8B, 0x11, 0x8B, 0x52, 0x1C];
const SHADER_MANAGER_GLOBAL: usize = 0x0157_2B18;

const PLAY_UI_SOUND: usize = 0x00F9_09C0;
const PLAY_UI_SOUND_PROLOGUE: [u8; 6] = [0x83, 0xEC, 0x18, 0x53, 0x33, 0xDB];
const HUD_RANK_SLOT: usize = 0x0142_B774;
const HUD_RANK_AWARD: usize = 0x0073_EB90;
const HUD_RANK_AWARD_BYTES: [u8; 9] = [0x53, 0x8B, 0x1D, 0xA8, 0x4C, 0x9A, 0x01, 0x56, 0x57];

const STRING_ASSIGN_CSTR: usize = 0x0040_14C0;
const STRING_DTOR: usize = 0x0040_1470;

const STING_RANK: &str = "Sound/Beach_Music/BMUS_USSting02A_Wave";

static PACIFIC_TIPS_CSTR: &[u8] = b"Menu/GameTips/pacific_GameTips\0";
static TIPS_NAME_PTR: AtomicPtr<u8> = AtomicPtr::new(NAM_TIPS_PTR as *mut u8);

#[repr(C)]
struct Bc2String {
    begin: *mut u8,
    end: *mut u8,
    capacity: *mut u8,
}

#[cfg(target_arch = "x86")]
type PlayUiSoundFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut Bc2String, i32);
#[cfg(target_arch = "x86")]
static ORIG_PLAY_UI_SOUND: OnceLock<PlayUiSoundFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static HUD_RANK_ORIG: AtomicU32 = AtomicU32::new(HUD_RANK_AWARD as u32);
static STINGS_ON: AtomicBool = AtomicBool::new(false);
static STING_LOGS: AtomicU32 = AtomicU32::new(0);

/// Engine-heap `eastl::string`s for each sting (made once on the UI thread, never freed).
static STING_STRINGS: Mutex<Vec<(&'static str, usize)>> = Mutex::new(Vec::new());

struct WakeShaderCache {
    renderer: usize,
    manager: usize,
    state: usize,
    tried: bool,
}
static WAKE_CACHE: Mutex<WakeShaderCache> = Mutex::new(WakeShaderCache {
    renderer: 0,
    manager: 0,
    state: 0,
    tried: false,
});

#[inline]
fn pacific_active() -> bool {
    crate::hooks::is_pacific_active()
}

pub fn update_for_mode(mode: ModeId) {
    let ptr = if mode == ModeId::Bf1943 {
        PACIFIC_TIPS_CSTR.as_ptr() as *mut u8
    } else {
        NAM_TIPS_PTR as *mut u8
    };
    TIPS_NAME_PTR.store(ptr, Ordering::Relaxed);
}

fn leaf_lower(name: &str) -> String {
    let leaf = name.rsplit(['/', '\\']).next().unwrap_or(name);
    leaf.to_ascii_lowercase()
}

fn award_sting(name: &str) -> Option<&'static str> {
    match leaf_lower(name).as_str() {
        "award_dogtag_01_wave" => Some("Sound/Beach_Music/BMUS_USSting02B_Wave"),
        "award_unlock_separate_01_wave" => Some("Sound/Beach_Music/BMUS_JPSting02A_Wave"),
        "award_trophies_01_wave" => Some("Sound/Beach_Music/BMUS_JPSting02B_Wave"),
        _ => None,
    }
}

fn rank_sting_replaces(sound: Option<&str>) -> bool {
    match sound {
        None => true,
        Some(s) if s.is_empty() => true,
        Some(s) => leaf_lower(s) == "award_unlock_separate_01_wave",
    }
}

#[cfg(target_arch = "x86")]
unsafe fn read_bc2_string(s: *const Bc2String) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let (b, e) = unsafe { ((*s).begin, (*s).end) };
    if b.is_null() || e <= b || (e as usize - b as usize) > 512 {
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(b, e as usize - b as usize) };
    Some(String::from_utf8_lossy(slice).into_owned())
}

#[cfg(target_arch = "x86")]
unsafe fn sting_string(name: &'static str) -> *mut Bc2String {
    let Ok(mut g) = STING_STRINGS.lock() else {
        return std::ptr::null_mut();
    };
    if let Some(&(_, p)) = g.iter().find(|(n, _)| *n == name) {
        return p as *mut Bc2String;
    }
    let Ok(cs) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    let s = Box::leak(Box::new(Bc2String {
        begin: std::ptr::null_mut(),
        end: std::ptr::null_mut(),
        capacity: std::ptr::null_mut(),
    }));
    type AssignFn = unsafe extern "fastcall" fn(*mut Bc2String, *mut c_void, *const u8);
    let assign: AssignFn = unsafe { std::mem::transmute(STRING_ASSIGN_CSTR) };
    unsafe { assign(s, std::ptr::null_mut(), cs.as_ptr().cast()) };
    if s.begin.is_null() {
        return std::ptr::null_mut();
    }
    let p = s as *mut Bc2String;
    g.push((name, p as usize));
    p
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_play_ui_sound(
    ui: *mut c_void,
    edx: *mut c_void,
    mut name: *mut Bc2String,
    music: i32,
) {
    if (music & 0xFF) == 0 && STINGS_ON.load(Ordering::Relaxed) && pacific_active() {
        if let Some(n) = unsafe { read_bc2_string(name) } {
            if let Some(sting) = award_sting(&n) {
                let s = unsafe { sting_string(sting) };
                if !s.is_null() {
                    if STING_LOGS.fetch_add(1, Ordering::Relaxed) < 40 {
                        crate::log_line(&format!("pacific: award sting {n} -> {sting}"));
                    }
                    name = s;
                }
            }
        }
    }
    if let Some(&orig) = ORIG_PLAY_UI_SOUND.get() {
        unsafe { orig(ui, edx, name, music) };
    }
}

/// cdecl(String* sound) -> String*: the HUD rank call's 4th argument, replaced by BF1943's rank sting.
#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn rank_sting_for(sound: *mut Bc2String) -> *mut Bc2String {
    if !STINGS_ON.load(Ordering::Relaxed) || !pacific_active() {
        return sound;
    }
    let current = unsafe { read_bc2_string(sound) };
    if rank_sting_replaces(current.as_deref()) {
        let s = unsafe { sting_string(STING_RANK) };
        if !s.is_null() {
            if STING_LOGS.fetch_add(1, Ordering::Relaxed) < 40 {
                crate::log_line("pacific: HUD rank-up -> BF1943 rank sting");
            }
            return s;
        }
    }
    sound
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn should_hide_map_overlay() -> u8 {
    u8::from(pacific_active())
}

/// cdecl(packet, minimap): bind the `PacificMinimap_Wake` shader state to the map image packet on WW2 maps.
#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn configure_wake_map_image(packet: usize, minimap: usize) {
    if packet < 0x10000 || minimap < 0x10000 || !pacific_active() {
        return;
    }
    let owner = unsafe { *((minimap + 4) as *const usize) };
    if owner < 0x10000 {
        return;
    }
    let renderer = unsafe { *((owner + 12) as *const usize) };
    let manager = unsafe { *(SHADER_MANAGER_GLOBAL as *const usize) };
    if renderer < 0x10000 || manager < 0x10000 {
        return;
    }
    let Ok(mut c) = WAKE_CACHE.lock() else { return };
    if c.renderer != renderer || c.manager != manager {
        c.renderer = renderer;
        c.manager = manager;
        c.state = 0;
        c.tried = false;
    }
    if c.state == 0 && !c.tried {
        c.tried = true;
        type AssignFn = unsafe extern "fastcall" fn(*mut Bc2String, *mut c_void, *const u8);
        type DtorFn = unsafe extern "fastcall" fn(*mut Bc2String, *mut c_void);
        type LookupFn = unsafe extern "fastcall" fn(usize, *mut c_void, *const Bc2String, i32) -> usize;
        type RefFn = unsafe extern "fastcall" fn(usize, *mut c_void);
        type CreateFn = unsafe extern "fastcall" fn(usize, *mut c_void, usize) -> usize;
        let assign: AssignFn = unsafe { std::mem::transmute(STRING_ASSIGN_CSTR) };
        let dtor: DtorFn = unsafe { std::mem::transmute(STRING_DTOR) };
        let mut name = Bc2String {
            begin: std::ptr::null_mut(),
            end: std::ptr::null_mut(),
            capacity: std::ptr::null_mut(),
        };
        unsafe {
            assign(&mut name, std::ptr::null_mut(), b"Systems/UI/Shaders/PacificMinimap_Wake\0".as_ptr());
            let mvt = *(manager as *const *const usize);
            let lookup: LookupFn = std::mem::transmute(*mvt.add(7));
            let program = lookup(manager, std::ptr::null_mut(), &name, 1);
            dtor(&mut name, std::ptr::null_mut());
            if program >= 0x10000 {
                let pvt = *(program as *const *const usize);
                let add_ref: RefFn = std::mem::transmute(*pvt);
                let release: RefFn = std::mem::transmute(*pvt.add(1));
                add_ref(program, std::ptr::null_mut());
                let rvt = *(renderer as *const *const usize);
                let create: CreateFn = std::mem::transmute(*rvt.add(8));
                c.state = create(renderer, std::ptr::null_mut(), program);
                release(program, std::ptr::null_mut());
            }
        }
        crate::log_line(&format!(
            "pacific: Wake ocean minimap shader {}",
            if c.state != 0 { "active" } else { "unavailable; native image retained" }
        ));
    }
    if c.state != 0 {
        unsafe { *((packet + 0xC4) as *mut usize) = c.state };
    }
}

/// Append `pushfd; pushad; mov ebx,esp; sub esp,528; and esp,-16; fxsave [esp]`.
fn emit_save(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x9C, 0x60, 0x8B, 0xDC, 0x81, 0xEC, 0x10, 0x02, 0x00, 0x00, 0x83, 0xE4, 0xF0]);
    code.extend_from_slice(&[0x0F, 0xAE, 0x04, 0x24]);
}

/// Append `fxrstor [esp]; mov esp,ebx`.
fn emit_restore_fpu(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x0F, 0xAE, 0x0C, 0x24, 0x8B, 0xE3]);
}

fn emit_jmp(code: &mut Vec<u8>, base: usize, target: usize) {
    let pos = base + code.len();
    code.push(0xE9);
    code.extend_from_slice(&((target as isize - (pos as isize + 5)) as i32).to_le_bytes());
}

#[cfg(target_arch = "x86")]
unsafe fn alloc_exec(size: usize) -> *mut u8 {
    use crate::win32::{self, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE};
    unsafe {
        win32::VirtualAlloc(std::ptr::null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE)
            as *mut u8
    }
}

#[cfg(target_arch = "x86")]
unsafe fn commit_stub(stub: *mut u8, code: &[u8]) {
    use crate::win32;
    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), stub, code.len());
        win32::FlushInstructionCache(win32::GetCurrentProcess(), stub.cast(), code.len());
    }
}

#[cfg(target_arch = "x86")]
pub unsafe fn install() -> usize {
    use crate::hooks;

    let mut installed = 0usize;

    // 1. Pacific War Game Tips (`0x00751BC2`, `0x007776EA`)
    if TIPS_SITES.iter().all(|&s| unsafe { hooks::bytes_match(s, &TIPS_BYTES) }) {
        let ptr_addr = (&raw const TIPS_NAME_PTR) as u32;
        let mut p = [0u8; 5];
        p[0] = 0xA1; // mov eax, [moffs32]
        p[1..5].copy_from_slice(&ptr_addr.to_le_bytes());
        let ok = TIPS_SITES
            .iter()
            .all(|&site| unsafe { hooks::write_code(site + TIPS_MOV_OFFSET, &p) });
        if ok {
            installed += 1;
            crate::log_line("pacific: GameTips redirect installed (Menu/GameTips/pacific_GameTips)");
        }
    }

    let overlay_layout_ok = unsafe {
        hooks::bytes_match(MAP_OVERLAY_START, &MAP_OVERLAY_START_BYTES)
            && hooks::bytes_match(0x0092_1E01, &MAP_OVERLAY_SHADER_BYTES)
            && hooks::bytes_match(MAP_OVERLAY_AFTER, &MAP_OVERLAY_END_BYTES)
    };

    // 2. Minimap combat-area overlay bypass (`0x00921CF7` -> `0x00921E29`)
    if overlay_layout_ok {
        let stub = unsafe { alloc_exec(128) };
        if !stub.is_null() {
            if let Some(tramp) = unsafe {
                hooks::install_detour(MAP_OVERLAY_START, &MAP_OVERLAY_START_BYTES, stub as *const c_void)
            } {
                let base = stub as usize;
                let mut code = Vec::with_capacity(96);
                emit_save(&mut code);
                code.push(0xB8);
                code.extend_from_slice(&(should_hide_map_overlay as *const () as u32).to_le_bytes());
                code.extend_from_slice(&[0xFF, 0xD0]); // call eax
                emit_restore_fpu(&mut code);
                code.extend_from_slice(&[0x84, 0xC0]); // test al, al
                code.extend_from_slice(&[0x74, 0x07]); // jz stock (+7)
                code.extend_from_slice(&[0x61, 0x9D]); // popad; popfd
                emit_jmp(&mut code, base, MAP_OVERLAY_AFTER);
                code.extend_from_slice(&[0x61, 0x9D]); // stock: popad; popfd
                emit_jmp(&mut code, base, tramp as usize);
                unsafe { commit_stub(stub, &code) };
                installed += 1;
                crate::log_line("pacific: minimap combat-area overlay bypass installed");
            }
        }
    }

    // 3. Wake ocean minimap extension shader (`0x00921CDB`)
    if overlay_layout_ok
        && unsafe {
            hooks::bytes_match(MAP_IMAGE_START, &MAP_OVERLAY_START_BYTES)
                && hooks::bytes_match(RENDERER_GETTER, &RENDERER_GETTER_BYTES)
                && hooks::bytes_match(SHADER_LOOKUP_SITE, &SHADER_LOOKUP_BYTES)
        }
    {
        let stub = unsafe { alloc_exec(128) };
        if !stub.is_null() {
            if let Some(tramp) = unsafe {
                hooks::install_detour(MAP_IMAGE_START, &MAP_OVERLAY_START_BYTES, stub as *const c_void)
            } {
                let base = stub as usize;
                let mut code = Vec::with_capacity(96);
                emit_save(&mut code);
                code.extend_from_slice(&[0x56, 0x57]); // push esi (minimap); push edi (packet)
                code.push(0xB8);
                code.extend_from_slice(&(configure_wake_map_image as *const () as u32).to_le_bytes());
                code.extend_from_slice(&[0xFF, 0xD0]); // call eax
                code.extend_from_slice(&[0x83, 0xC4, 0x08]); // add esp, 8
                emit_restore_fpu(&mut code);
                code.extend_from_slice(&[0x61, 0x9D]); // popad; popfd
                emit_jmp(&mut code, base, tramp as usize);
                unsafe { commit_stub(stub, &code) };
                installed += 1;
                crate::log_line("pacific: Wake ocean minimap shader hook installed");
            }
        }
    }

    // 4. BF1943 award stings (`PlayUiSound` @ `0x00F909C0`)
    if let Some(tramp) = unsafe {
        hooks::install_detour(PLAY_UI_SOUND, &PLAY_UI_SOUND_PROLOGUE, detour_play_ui_sound as *const c_void)
    } {
        let _ = ORIG_PLAY_UI_SOUND.set(unsafe { std::mem::transmute::<*const c_void, PlayUiSoundFn>(tramp) });
        STINGS_ON.store(true, Ordering::Relaxed);
        installed += 1;
        crate::log_line("pacific: BF1943 award stings installed (dog tags, insignia, pins/stars)");

        // 5. HUD rank-up sting wrapper (vtable slot `0x0142B774` -> `0x0073EB90`)
        let slot = unsafe { *(HUD_RANK_SLOT as *const usize) };
        if slot == HUD_RANK_AWARD && unsafe { hooks::bytes_match(HUD_RANK_AWARD, &HUD_RANK_AWARD_BYTES) } {
            let stub = unsafe { alloc_exec(64) };
            if !stub.is_null() {
                let mut code = Vec::with_capacity(48);
                code.push(0x51); // push ecx
                code.extend_from_slice(&[0x8B, 0x44, 0x24, 0x14]); // mov eax, [esp+14h] (4th arg)
                code.push(0x50); // push eax
                code.push(0xB8);
                code.extend_from_slice(&(rank_sting_for as *const () as u32).to_le_bytes());
                code.extend_from_slice(&[0xFF, 0xD0]); // call eax
                code.extend_from_slice(&[0x83, 0xC4, 0x04]); // add esp, 4
                code.push(0x59); // pop ecx
                code.extend_from_slice(&[0x89, 0x44, 0x24, 0x10]); // mov [esp+10h], eax
                code.extend_from_slice(&[0xFF, 0x25]); // jmp dword ptr [HUD_RANK_ORIG]
                code.extend_from_slice(&((&raw const HUD_RANK_ORIG) as u32).to_le_bytes());
                unsafe { commit_stub(stub, &code) };
                if unsafe { hooks::write_code(HUD_RANK_SLOT, &(stub as u32).to_le_bytes()) } {
                    installed += 1;
                    crate::log_line("pacific: HUD rank-up sting wrapper installed");
                }
            }
        }
    }

    if let Ok(g) = crate::hooks::STATE.lock() {
        if let Some(st) = g.as_ref() {
            update_for_mode(st.active);
        }
    }

    installed
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn install() -> usize {
    0
}
