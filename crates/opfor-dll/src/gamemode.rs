//! Tier 1 4-mode engine hooks & offline patches for `BFBC2Game.exe` build 795745.
//!
//! Installs 7 verified x86 patches & inline detours:
//! 1. Local in-memory transport (`0x0054AF2C`) for single-process offline multiplayer
//! 2. Vietnam / multi-mode offline gate (`0x0075FCC9`) & redeem popup bypass (`0x0075FE35`)
//! 3. Dynamic 8-byte `gameModChange` label pushes (`0x0075FDB7`: `"BFBC2Nam"` / `"BFBC2143"` / `"BFBC2BC1"`)
//! 4. `SetGameMod` (`0x0075FCB0`) 4-mode routing detour
//! 5. `MenuExternGetter` (`0x0076EC30`) OPFOR externs
//! 6. Offline `<4`-player scoring minimum patch (5 sites)
//! 7. `ParseGameMod` (`0x008FF330`) startup mode override (boots straight into Vietnam / BF1943 / BFBC1)

#![allow(dead_code, non_snake_case)]

#[cfg(target_arch = "x86")]
use std::ffi::c_void;
#[cfg(target_arch = "x86")]
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
#[cfg(target_arch = "x86")]
use std::sync::OnceLock;

use opfor_vfs::{ModeId, ModeTable};

/// Verified `BFBC2Game.exe` build 795745 (image base `0x00400000`) hook targets.
pub mod bc2_795745 {
    pub const UI_MANAGER_GLOBAL: usize = 0x019A_4CA8;
    pub const UI_GAME_MOD_OFFSET: usize = 0x000C_8C8C;

    // Local in-memory transport (patches.cpp)
    pub const TRANSPORT_SELECT: usize = 0x0054_AF2C;
    pub const TRANSPORT_SELECT_END: usize = 0x0054_AF42;
    pub const TRANSPORT_SELECT_BYTES: [u8; 22] = [
        0x84, 0xDB, 0x74, 0x04, 0x33, 0xED, 0xEB, 0x0E, 0x33, 0xD2, 0x38, 0x54, 0x24, 0x1C, 0x0F,
        0x95, 0xC2, 0x83, 0xC2, 0x01, 0x8B, 0xEA,
    ];

    // Mode switch & Vietnam offline gate
    pub const SET_GAME_MOD: usize = 0x0075_FCB0;
    pub const SET_GAME_MOD_PROLOGUE: [u8; 8] = [0x83, 0xEC, 0x0C, 0x55, 0x8B, 0x6C, 0x24, 0x14];
    pub const MOD_SWITCH_GATE: usize = 0x0075_FCC9;
    pub const NAM_REDEEM_EVENT: usize = 0x0075_FE35;
    pub const NAM_LABEL_PUSHES: usize = 0x0075_FDB7;
    pub const NAM_LABEL_PUSHES_BYTES: [u8; 10] =
        [0x68, 0x74, 0xB2, 0x42, 0x01, 0x68, 0x6C, 0xB2, 0x42, 0x01];

    // Main-menu extern getter & APT value constructors
    pub const MENU_EXTERN_GETTER: usize = 0x0076_EC30;
    pub const MENU_EXTERN_GETTER_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0xDC, 0x00, 0x00, 0x00];
    pub const MAKE_INT: usize = 0x00F7_C5D0;
    pub const MAKE_BOOL: usize = 0x00F7_C600;

    // Startup profile gameMod string parser (`0x008FF330`: "BC2"->0, "VIETNAM"->1)
    pub const PARSE_GAME_MOD: usize = 0x008F_F330;
    pub const PARSE_GAME_MOD_PROLOGUE: [u8; 6] = [0x56, 0x8B, 0x74, 0x24, 0x08, 0x56];

    // Main-menu config / music picker (`0x0075AEA1`: `push "nam_menuConfig"` at +17/+18)
    pub const MENU_CONFIG_PICK: usize = 0x0075_AEA1;
    pub const MENU_CONFIG_PICK_BYTES: [u8; 22] = [
        0x8B, 0x80, 0x8C, 0x8C, 0x0C, 0x00, 0x83, 0xF8, 0x01, 0x8B, 0x0D, 0xA8, 0x4C, 0x9A, 0x01,
        0x75, 0x63, 0x68, 0x64, 0xB1, 0x42, 0x01,
    ];
    pub const MENU_CONFIG_PUSH_IMM_OFFSET: usize = 18;

    // UI manager per-frame flag update (`0x0092A160`, thiscall(ui))
    pub const UI_FLAGS: usize = 0x0092_A160;
    pub const UI_FLAGS_PROLOGUE: [u8; 10] = [0x56, 0x8B, 0xF1, 0x80, 0xBE, 0x2A, 0x3F, 0x0C, 0x00, 0x00];

    // Main-menu FSCommand dispatcher (`0x0077BC10`, thiscall(menuSelf, name, args), ret 8)
    pub const MAIN_MENU_DISPATCHER: usize = 0x0077_BC10;
    pub const MAIN_MENU_DISPATCHER_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0x08, 0x0A, 0x00, 0x00];

    // Offline <4-player scoring minimum checks (patches.cpp)
    pub const MIN_PLAYER_CHECKS: [usize; 5] = [
        0x0086_5F36,
        0x0086_FD12,
        0x0084_2E6A,
        0x0084_2E9A,
        0x0083_B591,
    ];
}

/// Live 4-mode controller state shared across `dinput8.dll` hooks.
pub struct GameModeState {
    pub table: ModeTable,
    pub active: ModeId,
    pub hub_active: bool,
}

impl GameModeState {
    #[must_use]
    pub fn new(installed_mask: u32) -> Self {
        Self {
            table: ModeTable::new(installed_mask),
            active: ModeId::Bfbc2,
            hub_active: true,
        }
    }

    pub fn select_mode(&mut self, requested: ModeId) -> ModeId {
        let resolved = self.table.resolve_request(requested);
        self.active = resolved;
        self.hub_active = false;
        resolved
    }

    #[must_use]
    pub const fn menu_code(&self) -> i32 {
        match self.active {
            ModeId::Bfbc2 => 0,
            ModeId::Vietnam => 1,
            ModeId::Bf1943 => 3,
            ModeId::Bfbc1 => 4,
        }
    }
}

pub static STATE: Mutex<Option<GameModeState>> = Mutex::new(None);

/// Fixed-address 8-byte + NUL label buffer pointed to by `0x0075FDB7`.
static mut LABEL_BUF: [u8; 9] = *b"BFBC2Nam\0";

/// Fixed-address main-menu config/music name buffer pointed to by `0x0075AEB3`.
/// Holds `"pacific_menuConfig\0"` when BF1943 is active or `"nam_menuConfig\0"` otherwise.
static mut MENU_CONFIG_BUF: [u8; 32] = {
    let mut b = [0u8; 32];
    let src = b"nam_menuConfig";
    let mut i = 0;
    while i < src.len() {
        b[i] = src[i];
        i += 1;
    }
    b
};

#[cfg(target_arch = "x86")]
type SetGameModFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, i32);
#[cfg(target_arch = "x86")]
type ExternGetterFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8) -> u32;
#[cfg(target_arch = "x86")]
type ParseGameModFn = unsafe extern "cdecl" fn(*const u8) -> i32;
#[cfg(target_arch = "x86")]
type UiFlagsFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
#[cfg(target_arch = "x86")]
type MainMenuDispFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8, *const u8);
#[cfg(target_arch = "x86")]
type MakeIntFn = unsafe extern "cdecl" fn(i32) -> u32;

#[cfg(target_arch = "x86")]
static ORIG_SET_GAME_MOD: OnceLock<SetGameModFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_EXTERN_GETTER: OnceLock<ExternGetterFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_PARSE_GAME_MOD: OnceLock<ParseGameModFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_UI_FLAGS: OnceLock<UiFlagsFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_MAIN_MENU_DISP: OnceLock<MainMenuDispFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
static MENU_SELF: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_arch = "x86")]
static MENU_LIVE_AT: OnceLock<std::time::Instant> = OnceLock::new();
#[cfg(target_arch = "x86")]
static SWITCH_DONE_AT: OnceLock<std::time::Instant> = OnceLock::new();
#[cfg(target_arch = "x86")]
static AUTO_SWITCH_PENDING: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "x86")]
static READY_SIGNALED: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "x86")]
static EXTERN_LOG_COUNT: AtomicU32 = AtomicU32::new(0);

#[cfg(target_arch = "x86")]
unsafe fn sync_mode_buffers(mode: ModeId, label_8: &[u8; 8]) {
    unsafe {
        LABEL_BUF[0..8].copy_from_slice(label_8);
        LABEL_BUF[8] = 0;
        let cfg: &[u8] = match mode {
            ModeId::Bf1943 => b"pacific_menuConfig\0",
            _ => b"nam_menuConfig\0",
        };
        MENU_CONFIG_BUF[..cfg.len()].copy_from_slice(cfg);
    }
}

/// Verify that the host process is `BFBC2Game.exe` build 795745 before applying code patches.
#[cfg(target_arch = "x86")]
pub unsafe fn is_bfbc2_795745() -> bool {
    let exe = unsafe { crate::win32::GetModuleHandleW(std::ptr::null()) } as usize;
    if exe != 0x0040_0000 {
        return false;
    }
    unsafe { crate::hook::bytes_match(bc2_795745::SET_GAME_MOD, &bc2_795745::SET_GAME_MOD_PROLOGUE) }
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn is_bfbc2_795745() -> bool {
    false
}

#[cfg(target_arch = "x86")]
#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    next: *mut ExceptionRecord,
    addr: *mut c_void,
    num_params: u32,
    info: [usize; 15],
}

#[cfg(target_arch = "x86")]
#[repr(C)]
struct ExceptionPointers {
    record: *mut ExceptionRecord,
    context: *mut c_void,
}

#[cfg(target_arch = "x86")]
unsafe extern "system" {
    fn AddVectoredExceptionHandler(
        first: u32,
        handler: unsafe extern "system" fn(*mut ExceptionPointers) -> i32,
    ) -> *mut c_void;
}

#[cfg(target_arch = "x86")]
unsafe extern "system" fn veh_logger(ep: *mut ExceptionPointers) -> i32 {
    if !ep.is_null() {
        let rec = unsafe { &*(*ep).record };
        if rec.code == 0xC000_0005 || rec.code == 0xC000_001D || rec.code == 0xC000_0094 {
            let rw = rec.info[0];
            let target = rec.info[1];
            crate::log_line(&format!(
                "CRASH code=0x{:08X} eip={:p} rw={rw} target=0x{target:08X}",
                rec.code, rec.addr
            ));
        }
    }
    0 // EXCEPTION_CONTINUE_SEARCH
}

/// Install all 7 verified Tier 1 build-795745 patches & inline detours.
#[cfg(target_arch = "x86")]
pub unsafe fn install_all() -> usize {
    use crate::hook;
    use crate::win32::{self, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE};

    unsafe { AddVectoredExceptionHandler(1, veh_logger) };
    let mut installed = 0usize;

    // 1. Local in-memory multiplayer transport (`0x0054AF2C`)
    if unsafe { hook::bytes_match(bc2_795745::TRANSPORT_SELECT, &bc2_795745::TRANSPORT_SELECT_BYTES) } {
        let stub = unsafe {
            win32::VirtualAlloc(
                std::ptr::null_mut(),
                64,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            ) as *mut u8
        };
        if !stub.is_null() {
            let head: [u8; 23] = [
                0x84, 0xDB, // test bl, bl
                0x75, 0x18, // jnz inmemory
                0x80, 0x7C, 0x24, 0x18, 0x00, // cmp byte ptr [esp+18h], 0 (flagD4)
                0x75, 0x11, // jnz inmemory
                0x33, 0xD2, // xor edx, edx
                0x38, 0x54, 0x24, 0x1C, // cmp [esp+1Ch], dl (flagD5)
                0x0F, 0x95, 0xC2, // setnz dl
                0x42, // inc edx
                0x8B, 0xEA, // mov ebp, edx
            ];
            unsafe {
                std::ptr::copy_nonoverlapping(head.as_ptr(), stub, head.len());
                let p1 = stub.add(head.len());
                *p1 = 0xE9;
                let r1 = hook::rel32(p1 as usize, bc2_795745::TRANSPORT_SELECT_END).to_le_bytes();
                std::ptr::copy_nonoverlapping(r1.as_ptr(), p1.add(1), 4);
                let p2 = p1.add(5);
                *p2 = 0x33;
                *p2.add(1) = 0xED; // xor ebp, ebp (mode 0 = in-memory)
                *p2.add(2) = 0xE9;
                let r2 = hook::rel32(p2.add(2) as usize, bc2_795745::TRANSPORT_SELECT_END).to_le_bytes();
                std::ptr::copy_nonoverlapping(r2.as_ptr(), p2.add(3), 4);
                win32::FlushInstructionCache(win32::GetCurrentProcess(), stub.cast(), 64);

                let mut jump = [0x90u8; 22];
                jump[0] = 0xE9;
                let rj = hook::rel32(bc2_795745::TRANSPORT_SELECT, stub as usize).to_le_bytes();
                jump[1..5].copy_from_slice(&rj);
                if hook::write_code(bc2_795745::TRANSPORT_SELECT, &jump) {
                    crate::log_line("patch local-transport @ 0x0054AF2C installed");
                    installed += 1;
                }
            }
        }
    }

    // 2. Vietnam / multi-mode offline gate & redeem popup bypass
    if unsafe {
        hook::patch_verified(bc2_795745::MOD_SWITCH_GATE, &[0x75, 0x16], &[0xEB, 0x23])
            && hook::patch_verified(bc2_795745::NAM_REDEEM_EVENT, &[0x75, 0x10], &[0xEB, 0x10])
    } {
        crate::log_line("patch multi-mode offline gate @ 0x0075FCC9 / 0x0075FE35 installed");
        installed += 1;
    }

    // 3. Dynamic 8-byte label buffer pushes (`0x0075FDB7`)
    let begin_addr = (&raw const LABEL_BUF) as usize;
    let end_addr = begin_addr + 8;
    let mut label_patch = [0u8; 10];
    label_patch[0] = 0x68;
    label_patch[1..5].copy_from_slice(&(end_addr as u32).to_le_bytes());
    label_patch[5] = 0x68;
    label_patch[6..10].copy_from_slice(&(begin_addr as u32).to_le_bytes());
    if unsafe {
        hook::patch_verified(
            bc2_795745::NAM_LABEL_PUSHES,
            &bc2_795745::NAM_LABEL_PUSHES_BYTES,
            &label_patch,
        )
    } {
        crate::log_line("patch 4-mode label pushes @ 0x0075FDB7 installed");
        installed += 1;
    }

    // 4. `SetGameMod` (`0x0075FCB0`) 4-mode detour
    if let Some(tramp) = unsafe {
        hook::install_detour(
            bc2_795745::SET_GAME_MOD,
            &bc2_795745::SET_GAME_MOD_PROLOGUE,
            detour_set_game_mod as *const c_void,
        )
    } {
        let _ = ORIG_SET_GAME_MOD.set(unsafe { std::mem::transmute::<*const c_void, SetGameModFn>(tramp) });
        crate::log_line("detour SetGameMod @ 0x0075FCB0 installed");
        installed += 1;
    }

    // 5. `MenuExternGetter` (`0x0076EC30`) OPFOR externs detour
    if let Some(tramp) = unsafe {
        hook::install_detour(
            bc2_795745::MENU_EXTERN_GETTER,
            &bc2_795745::MENU_EXTERN_GETTER_PROLOGUE,
            detour_extern_getter as *const c_void,
        )
    } {
        let _ = ORIG_EXTERN_GETTER.set(unsafe { std::mem::transmute::<*const c_void, ExternGetterFn>(tramp) });
        crate::log_line("detour MenuExternGetter @ 0x0076EC30 installed");
        installed += 1;
    }

    // 6. Offline <4-player scoring patch (5 sites: `cmp eax, 4; jae` -> `cmp eax, 0; jae`)
    let cmp4 = [0x83u8, 0xF8, 0x04, 0x73];
    let cmp0 = [0x83u8, 0xF8, 0x00, 0x73];
    if bc2_795745::MIN_PLAYER_CHECKS
        .iter()
        .all(|&addr| unsafe { hook::bytes_match(addr, &cmp4) })
    {
        for &addr in &bc2_795745::MIN_PLAYER_CHECKS {
            let _ = unsafe { hook::patch_verified(addr, &cmp4, &cmp0) };
        }
        crate::log_line("patch offline-scoring (5 min-player checks) installed");
        installed += 1;
    }

    // 7. `ParseGameMod` (`0x008FF330`) startup mode override
    if let Some(tramp) = unsafe {
        hook::install_detour(
            bc2_795745::PARSE_GAME_MOD,
            &bc2_795745::PARSE_GAME_MOD_PROLOGUE,
            detour_parse_game_mod as *const c_void,
        )
    } {
        let _ = ORIG_PARSE_GAME_MOD.set(unsafe { std::mem::transmute::<*const c_void, ParseGameModFn>(tramp) });
        crate::log_line("detour ParseGameMod @ 0x008FF330 installed");
        installed += 1;
    }

    // 8. Main-menu music/config redirect (`0x0075AEA1`: `pacific_menuConfig` vs `nam_menuConfig`)
    if unsafe { hook::bytes_match(bc2_795745::MENU_CONFIG_PICK, &bc2_795745::MENU_CONFIG_PICK_BYTES) } {
        let cfg_addr = (&raw const MENU_CONFIG_BUF) as u32;
        if unsafe {
            hook::write_code(
                bc2_795745::MENU_CONFIG_PICK + bc2_795745::MENU_CONFIG_PUSH_IMM_OFFSET,
                &cfg_addr.to_le_bytes(),
            )
        } {
            crate::log_line("patch menu-music config @ 0x0075AEA1 installed");
            installed += 1;
        }
    }

    // 9. `UiFlags` (`0x0092A160`) per-frame UI tick for automatic era switch on launch
    if let Some(tramp) = unsafe {
        hook::install_detour(
            bc2_795745::UI_FLAGS,
            &bc2_795745::UI_FLAGS_PROLOGUE,
            detour_ui_flags as *const c_void,
        )
    } {
        let _ = ORIG_UI_FLAGS.set(unsafe { std::mem::transmute::<*const c_void, UiFlagsFn>(tramp) });
        crate::log_line("detour UiFlags @ 0x0092A160 installed");
        installed += 1;
    }

    // 10. `MainMenuDispatcher` (`0x0077BC10`) to capture live `menuSelf` for FSCommand dispatch
    if let Some(tramp) = unsafe {
        hook::install_detour(
            bc2_795745::MAIN_MENU_DISPATCHER,
            &bc2_795745::MAIN_MENU_DISPATCHER_PROLOGUE,
            detour_main_menu_disp as *const c_void,
        )
    } {
        let _ = ORIG_MAIN_MENU_DISP.set(unsafe { std::mem::transmute::<*const c_void, MainMenuDispFn>(tramp) });
        crate::log_line("detour MainMenuDispatcher @ 0x0077BC10 installed");
        installed += 1;
    }

    // Pre-sync label + music config buffers and arm auto-switch for launch.ini mode
    if let Ok(guard) = STATE.lock() {
        if let Some(st) = guard.as_ref() {
            let spec = st.table.spec(st.active);
            unsafe { sync_mode_buffers(st.active, &spec.label_8) };
            if !st.hub_active && st.active != ModeId::Bfbc2 {
                AUTO_SWITCH_PENDING.store(true, Ordering::Release);
                crate::log_line(&format!(
                    "auto-switch armed for launch mode={} (menu_code={})",
                    spec.slug,
                    st.menu_code()
                ));
            }
        }
    }

    installed
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn install_all() -> usize {
    0
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_main_menu_disp(
    menu_self: *mut c_void,
    edx: *mut c_void,
    name: *const u8,
    args: *const u8,
) {
    if !menu_self.is_null() {
        let prev = MENU_SELF.swap(menu_self as usize, Ordering::AcqRel);
        if prev == 0 {
            let _ = MENU_LIVE_AT.set(std::time::Instant::now());
            let cname = unsafe { crate::win32::from_ansi_ptr(name) }.unwrap_or_default();
            crate::log_line(&format!(
                "MainMenuDispatcher: captured live menuSelf={menu_self:p} on cmd='{cname}'"
            ));
        }
    }
    if let Some(&orig) = ORIG_MAIN_MENU_DISP.get() {
        unsafe { orig(menu_self, edx, name, args) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_ui_flags(ui: *mut c_void, edx: *mut c_void) {
    if let Some(&orig) = ORIG_UI_FLAGS.get() {
        unsafe { orig(ui, edx) };
    }
    if ui.is_null() {
        return;
    }
    let menu_self = MENU_SELF.load(Ordering::Acquire) as *mut c_void;
    if menu_self.is_null() {
        return;
    }
    if AUTO_SWITCH_PENDING.load(Ordering::Acquire) {
        if let Some(since) = MENU_LIVE_AT.get() {
            if since.elapsed() >= std::time::Duration::from_millis(600)
                && AUTO_SWITCH_PENDING.swap(false, Ordering::AcqRel)
            {
                let desired_code = STATE
                    .lock()
                    .ok()
                    .and_then(|g| g.as_ref().map(GameModeState::menu_code));
                if let Some(code) = desired_code {
                    if code != 0 {
                        let arg: &[u8] = match code {
                            1 => b"1\0",
                            3 => b"3\0",
                            4 => b"4\0",
                            _ => b"0\0",
                        };
                        crate::log_line(&format!(
                            "UiFlags: dispatching changeGameMod '{code}' after {:?} (menuSelf={menu_self:p})",
                            since.elapsed()
                        ));
                        if let Some(&disp) = ORIG_MAIN_MENU_DISP.get() {
                            unsafe {
                                disp(
                                    menu_self,
                                    std::ptr::null_mut(),
                                    b"changeGameMod\0".as_ptr(),
                                    arg.as_ptr(),
                                );
                            }
                            let _ = SWITCH_DONE_AT.set(std::time::Instant::now());
                            crate::log_line("UiFlags: changeGameMod dispatched cleanly");
                        }
                    }
                }
            }
        }
    } else if !READY_SIGNALED.load(Ordering::Acquire) {
        let ready = if let Some(done_at) = SWITCH_DONE_AT.get() {
            done_at.elapsed() >= std::time::Duration::from_millis(450)
        } else if let Some(since) = MENU_LIVE_AT.get() {
            since.elapsed() >= std::time::Duration::from_millis(600)
        } else {
            false
        };
        if ready && !READY_SIGNALED.swap(true, Ordering::AcqRel) {
            crate::signal_engine_ready();
        }
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn detour_parse_game_mod(s: *const u8) -> i32 {
    if let Ok(guard) = STATE.lock() {
        if let Some(st) = guard.as_ref() {
            if !st.hub_active {
                let spec = st.table.spec(st.active);
                unsafe { sync_mode_buffers(st.active, &spec.label_8) };
                return st.active.engine_mode();
            }
        }
    }
    if let Some(&orig) = ORIG_PARSE_GAME_MOD.get() {
        unsafe { orig(s) }
    } else {
        0
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_set_game_mod(ui: *mut c_void, edx: *mut c_void, mode: i32) {
    let requested = ModeId::from_menu_code(mode);
    let mut engine_mode = requested.engine_mode();
    if let Ok(mut guard) = STATE.lock() {
        if let Some(st) = guard.as_mut() {
            let resolved = st.select_mode(requested);
            engine_mode = resolved.engine_mode();
            let spec = st.table.spec(resolved);
            unsafe { sync_mode_buffers(resolved, &spec.label_8) };
            crate::log_line(&format!(
                "SetGameMod: request={mode} -> mode={} (engine={engine_mode}, label={})",
                spec.slug,
                std::str::from_utf8(&spec.label_8).unwrap_or("?")
            ));
        }
    }
    if let Some(&orig) = ORIG_SET_GAME_MOD.get() {
        unsafe { orig(ui, edx, engine_mode) };
        crate::log_line("SetGameMod: orig returned cleanly");
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_extern_getter(
    menu: *mut c_void,
    edx: *mut c_void,
    name_ptr: *const u8,
) -> u32 {
    if let Some(name) = unsafe { crate::win32::from_ansi_ptr(name_ptr) } {
        if EXTERN_LOG_COUNT.fetch_add(1, Ordering::Relaxed) < 5 {
            crate::log_line(&format!("MenuExternGetter: '{name}'"));
        }
        let make_int: MakeIntFn = unsafe { std::mem::transmute(bc2_795745::MAKE_INT) };
        if let Ok(guard) = STATE.lock() {
            if let Some(st) = guard.as_ref() {
                match name.as_str() {
                    "opforInstalledModes" => return unsafe { make_int(st.table.installed_mask() as i32) },
                    "opforActiveMode" => return unsafe { make_int(st.active as i32) },
                    "opforGameMod" => return unsafe { make_int(st.menu_code()) },
                    "opforHubActive" => return unsafe { make_int(i32::from(st.hub_active)) },
                    "opforPacific" => return unsafe { make_int(i32::from(st.active == ModeId::Bf1943)) },
                    "opforBfbc1" => return unsafe { make_int(i32::from(st.active == ModeId::Bfbc1)) },
                    "opforKitCount" => return unsafe { make_int(i32::from(st.table.spec(st.active).kit_count)) },
                    "opforNoHealthRegen" => return unsafe { make_int(i32::from(st.active == ModeId::Bfbc1)) },
                    "opforHasAutoInjector" => return unsafe { make_int(i32::from(st.active == ModeId::Bfbc1)) },
                    _ => {}
                }
            }
        }
    }
    if let Some(&orig) = ORIG_EXTERN_GETTER.get() {
        unsafe { orig(menu, edx, name_ptr) }
    } else {
        0
    }
}
