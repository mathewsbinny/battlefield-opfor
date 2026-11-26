//! Verified x86 inline detour engine & Tier 1 build-795745 patches/hooks for `opfor-host`.
//!
//! Applied directly to the mapped `0x00400000..0x01D13000` Frostbite image BEFORE
//! Frostbite executes its first instruction.

#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use opfor_vfs::{ModeId, ModeTable};

use crate::win32::{self, DWORD, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE};

pub mod bc2_795745 {
    pub const UI_MANAGER_GLOBAL: usize = 0x019A_4CA8;
    pub const TLS_INDEX_VA: usize = 0x0187_AC7C;

    // Local in-memory transport
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

    // Startup profile gameMod string parser (`0x008FF330`)
    pub const PARSE_GAME_MOD: usize = 0x008F_F330;
    pub const PARSE_GAME_MOD_PROLOGUE: [u8; 6] = [0x56, 0x8B, 0x74, 0x24, 0x08, 0x56];

    // Main-menu config / music picker (`0x0075AEA1`: `push "nam_menuConfig"` at +18)
    pub const MENU_CONFIG_PICK: usize = 0x0075_AEA1;
    pub const MENU_CONFIG_PICK_BYTES: [u8; 22] = [
        0x8B, 0x80, 0x8C, 0x8C, 0x0C, 0x00, 0x83, 0xF8, 0x01, 0x8B, 0x0D, 0xA8, 0x4C, 0x9A, 0x01,
        0x75, 0x63, 0x68, 0x64, 0xB1, 0x42, 0x01,
    ];
    pub const MENU_CONFIG_PUSH_IMM_OFFSET: usize = 18;

    // UI manager per-frame flag update (`0x0092A160`)
    pub const UI_FLAGS: usize = 0x0092_A160;
    pub const UI_FLAGS_PROLOGUE: [u8; 10] =
        [0x56, 0x8B, 0xF1, 0x80, 0xBE, 0x2A, 0x3F, 0x0C, 0x00, 0x00];

    // Main-menu FSCommand dispatcher (`0x0077BC10`)
    pub const MAIN_MENU_DISPATCHER: usize = 0x0077_BC10;
    pub const MAIN_MENU_DISPATCHER_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0x08, 0x0A, 0x00, 0x00];

    // Offline <4-player scoring minimum checks
    pub const MIN_PLAYER_CHECKS: [usize; 5] = [
        0x0086_5F36,
        0x0086_FD12,
        0x0084_2E6A,
        0x0084_2E9A,
        0x0083_B591,
    ];
}

pub struct GameModeState {
    pub table: ModeTable,
    pub active: ModeId,
    pub hub_active: bool,
}

impl GameModeState {
    pub fn new(installed_mask: u32) -> Self {
        PACIFIC_ACTIVE.store(false, Ordering::Release);
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
        PACIFIC_ACTIVE.store(resolved == ModeId::Bf1943, Ordering::Release);
        resolved
    }

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
static PACIFIC_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Lock-free check for whether Battlefield 1943 (Pacific War) is the active era.
#[inline]
pub fn is_pacific_active() -> bool {
    PACIFIC_ACTIVE.load(Ordering::Relaxed)
}

static mut LABEL_BUF: [u8; 9] = *b"BFBC2Nam\0";
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
static AUTOSTART_LEVEL: Mutex<Option<String>> = Mutex::new(None);
#[cfg(target_arch = "x86")]
static READY_SIGNALED: AtomicBool = AtomicBool::new(false);

pub fn set_autostart_level(level: &str) {
    #[cfg(target_arch = "x86")]
    if let Ok(mut g) = AUTOSTART_LEVEL.lock() {
        *g = Some(level.to_string());
    }
    #[cfg(not(target_arch = "x86"))]
    let _ = level;
}

const fn rel32(from: usize, to: usize) -> i32 {
    (to as isize - (from as isize + 5)) as i32
}

pub(crate) unsafe fn bytes_match(addr: usize, expected: &[u8]) -> bool {
    let slice = unsafe { std::slice::from_raw_parts(addr as *const u8, expected.len()) };
    slice == expected
}

pub(crate) unsafe fn write_code(addr: usize, bytes: &[u8]) -> bool {
    let mut old: DWORD = 0;
    let ptr = addr as *mut c_void;
    if unsafe { win32::VirtualProtect(ptr, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) } == 0 {
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
        let mut dummy: DWORD = 0;
        win32::VirtualProtect(ptr, bytes.len(), old, &mut dummy);
        win32::FlushInstructionCache(win32::GetCurrentProcess(), ptr, bytes.len());
    }
    true
}

pub(crate) unsafe fn patch_verified(addr: usize, expected: &[u8], replacement: &[u8]) -> bool {
    expected.len() == replacement.len()
        && unsafe { bytes_match(addr, expected) }
        && unsafe { write_code(addr, replacement) }
}

pub(crate) unsafe fn install_detour(
    target: usize,
    expected_prologue: &[u8],
    detour: *const c_void,
) -> Option<*const c_void> {
    let n = expected_prologue.len();
    if n < 5 || n > 32 || !unsafe { bytes_match(target, expected_prologue) } {
        return None;
    }
    let tramp = unsafe {
        win32::VirtualAlloc(
            std::ptr::null_mut(),
            64,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        ) as *mut u8
    };
    if tramp.is_null() {
        return None;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(expected_prologue.as_ptr(), tramp, n);
        let jmp_back = rel32(tramp as usize + n, target + n).to_le_bytes();
        *tramp.add(n) = 0xE9;
        std::ptr::copy_nonoverlapping(jmp_back.as_ptr(), tramp.add(n + 1), 4);
        win32::FlushInstructionCache(win32::GetCurrentProcess(), tramp.cast(), n + 5);

        let mut patch = vec![0x90u8; n];
        patch[0] = 0xE9;
        patch[1..5].copy_from_slice(&rel32(target, detour as usize).to_le_bytes());
        if !write_code(target, &patch) {
            return None;
        }
    }
    Some(tramp.cast())
}

#[cfg(target_arch = "x86")]
unsafe fn sync_mode_buffers(mode: ModeId, label_8: &[u8; 8]) {
    crate::pacific::update_for_mode(mode);
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

#[cfg(target_arch = "x86")]
pub unsafe fn install_all() -> usize {
    if !unsafe { bytes_match(bc2_795745::SET_GAME_MOD, &bc2_795745::SET_GAME_MOD_PROLOGUE) } {
        crate::log_line("ERROR: mapped image does not match BFBC2Game.exe build 795745");
        return 0;
    }

    let mut installed = 0usize;

    // 1. Local in-memory multiplayer transport (`0x0054AF2C`)
    if unsafe { bytes_match(bc2_795745::TRANSPORT_SELECT, &bc2_795745::TRANSPORT_SELECT_BYTES) } {
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
                0x84, 0xDB, 0x75, 0x18, 0x80, 0x7C, 0x24, 0x18, 0x00, 0x75, 0x11, 0x33, 0xD2,
                0x38, 0x54, 0x24, 0x1C, 0x0F, 0x95, 0xC2, 0x42, 0x8B, 0xEA,
            ];
            unsafe {
                std::ptr::copy_nonoverlapping(head.as_ptr(), stub, head.len());
                let p1 = stub.add(head.len());
                *p1 = 0xE9;
                std::ptr::copy_nonoverlapping(
                    rel32(p1 as usize, bc2_795745::TRANSPORT_SELECT_END).to_le_bytes().as_ptr(),
                    p1.add(1),
                    4,
                );
                let p2 = p1.add(5);
                *p2 = 0x33;
                *p2.add(1) = 0xED;
                *p2.add(2) = 0xE9;
                std::ptr::copy_nonoverlapping(
                    rel32(p2.add(2) as usize, bc2_795745::TRANSPORT_SELECT_END)
                        .to_le_bytes()
                        .as_ptr(),
                    p2.add(3),
                    4,
                );
                win32::FlushInstructionCache(win32::GetCurrentProcess(), stub.cast(), 64);

                let mut jump = [0x90u8; 22];
                jump[0] = 0xE9;
                jump[1..5]
                    .copy_from_slice(&rel32(bc2_795745::TRANSPORT_SELECT, stub as usize).to_le_bytes());
                if write_code(bc2_795745::TRANSPORT_SELECT, &jump) {
                    installed += 1;
                }
            }
        }
    }

    // 2. Multi-mode offline gate & redeem popup bypass
    if unsafe {
        patch_verified(bc2_795745::MOD_SWITCH_GATE, &[0x75, 0x16], &[0xEB, 0x23])
            && patch_verified(bc2_795745::NAM_REDEEM_EVENT, &[0x75, 0x10], &[0xEB, 0x10])
    } {
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
        patch_verified(
            bc2_795745::NAM_LABEL_PUSHES,
            &bc2_795745::NAM_LABEL_PUSHES_BYTES,
            &label_patch,
        )
    } {
        installed += 1;
    }

    // 4. `SetGameMod` (`0x0075FCB0`)
    if let Some(tramp) = unsafe {
        install_detour(
            bc2_795745::SET_GAME_MOD,
            &bc2_795745::SET_GAME_MOD_PROLOGUE,
            detour_set_game_mod as *const c_void,
        )
    } {
        let _ = ORIG_SET_GAME_MOD.set(unsafe { std::mem::transmute::<*const c_void, SetGameModFn>(tramp) });
        installed += 1;
    }

    // 5. `MenuExternGetter` (`0x0076EC30`)
    if let Some(tramp) = unsafe {
        install_detour(
            bc2_795745::MENU_EXTERN_GETTER,
            &bc2_795745::MENU_EXTERN_GETTER_PROLOGUE,
            detour_extern_getter as *const c_void,
        )
    } {
        let _ = ORIG_EXTERN_GETTER.set(unsafe { std::mem::transmute::<*const c_void, ExternGetterFn>(tramp) });
        installed += 1;
    }

    // 6. Offline <4-player scoring checks
    let cmp4 = [0x83u8, 0xF8, 0x04, 0x73];
    let cmp0 = [0x83u8, 0xF8, 0x00, 0x73];
    if bc2_795745::MIN_PLAYER_CHECKS
        .iter()
        .all(|&a| unsafe { bytes_match(a, &cmp4) })
    {
        for &a in &bc2_795745::MIN_PLAYER_CHECKS {
            let _ = unsafe { patch_verified(a, &cmp4, &cmp0) };
        }
        installed += 1;
    }

    // 7. `ParseGameMod` (`0x008FF330`)
    if let Some(tramp) = unsafe {
        install_detour(
            bc2_795745::PARSE_GAME_MOD,
            &bc2_795745::PARSE_GAME_MOD_PROLOGUE,
            detour_parse_game_mod as *const c_void,
        )
    } {
        let _ = ORIG_PARSE_GAME_MOD.set(unsafe { std::mem::transmute::<*const c_void, ParseGameModFn>(tramp) });
        installed += 1;
    }

    // 8. Main-menu music config redirect (`0x0075AEA1`)
    if unsafe { bytes_match(bc2_795745::MENU_CONFIG_PICK, &bc2_795745::MENU_CONFIG_PICK_BYTES) } {
        let cfg_addr = (&raw const MENU_CONFIG_BUF) as u32;
        if unsafe {
            write_code(
                bc2_795745::MENU_CONFIG_PICK + bc2_795745::MENU_CONFIG_PUSH_IMM_OFFSET,
                &cfg_addr.to_le_bytes(),
            )
        } {
            installed += 1;
        }
    }

    // 9. `UiFlags` (`0x0092A160`)
    if let Some(tramp) = unsafe {
        install_detour(
            bc2_795745::UI_FLAGS,
            &bc2_795745::UI_FLAGS_PROLOGUE,
            detour_ui_flags as *const c_void,
        )
    } {
        let _ = ORIG_UI_FLAGS.set(unsafe { std::mem::transmute::<*const c_void, UiFlagsFn>(tramp) });
        installed += 1;
    }

    // 10. `MainMenuDispatcher` (`0x0077BC10`)
    if let Some(tramp) = unsafe {
        install_detour(
            bc2_795745::MAIN_MENU_DISPATCHER,
            &bc2_795745::MAIN_MENU_DISPATCHER_PROLOGUE,
            detour_main_menu_disp as *const c_void,
        )
    } {
        let _ = ORIG_MAIN_MENU_DISP.set(unsafe { std::mem::transmute::<*const c_void, MainMenuDispFn>(tramp) });
        installed += 1;
    }

    if let Ok(guard) = STATE.lock() {
        if let Some(st) = guard.as_ref() {
            let spec = st.table.spec(st.active);
            unsafe { sync_mode_buffers(st.active, &spec.label_8) };
            let has_autostart = AUTOSTART_LEVEL
                .lock()
                .ok()
                .is_some_and(|g| g.is_some());
            if !st.hub_active && (st.active != ModeId::Bfbc2 || has_autostart) {
                AUTO_SWITCH_PENDING.store(true, Ordering::Release);
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
    let cname = unsafe { win32::from_ansi_ptr(name) }.unwrap_or_default();
    let cargs = unsafe { win32::from_ansi_ptr(args) }.unwrap_or_default();
    static DISP_LOGS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    if DISP_LOGS.fetch_add(1, Ordering::Relaxed) < 15 {
        crate::log_line(&format!("MainMenuDispatcher: cmd='{cname}' args='{cargs}'"));
    }
    if !menu_self.is_null() {
        let prev = MENU_SELF.swap(menu_self as usize, Ordering::AcqRel);
        if prev == 0 {
            let _ = MENU_LIVE_AT.set(std::time::Instant::now());
            crate::ui_state::set_scene(crate::ui_state::GameScene::InMenu);
            crate::dx_overlay::mark_dirty();
            crate::log_line(&format!(
                "MainMenuDispatcher: captured live menuSelf={menu_self:p} on cmd='{cname}'"
            ));
        } else if cname == "loadEulaFonts" || cname == "startBackgroundMovie" {
            crate::ui_state::set_scene(crate::ui_state::GameScene::InMenu);
            crate::dx_overlay::mark_dirty();
        }
    }
    if unsafe { crate::profile::handle_menu_command(&cname, &cargs) } {
        return;
    }
    if unsafe { crate::persistence::handle_menu_command(&cname, &cargs) } {
        return;
    }
    if unsafe { crate::server_browser::handle_menu_command(&cname, &cargs) } {
        return;
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
    unsafe {
        crate::server_browser::on_ui_frame(ui);
        crate::onslaught_hud::on_ui_frame(ui);
        crate::stats_screens::on_ui_frame(ui);
        crate::ui_state::poll_gamepad();
        crate::prompts::sync_frame();
    }

    // Drain actions triggered from our in-game DX11 custom era menu (BF1943 / BFBC1)
    let (deploy_lvl, switch_era, do_quit) = crate::ui_state::UI_STATE
        .lock()
        .ok()
        .and_then(|mut g| {
            g.as_mut().map(|st| {
                let d = st.pending_deploy_level.take();
                let m = st.pending_mode_switch.take();
                let q = st.pending_quit;
                st.pending_quit = false;
                (d, m, q)
            })
        })
        .unwrap_or((None, None, false));

    if do_quit {
        crate::log_line("ui_state: Exit Game requested from in-game menu");
        std::process::exit(0);
    }
    if let Some(lvl) = deploy_lvl {
        crate::log_line(&format!("ui_state: deploying into '{lvl}' from in-game menu"));
        unsafe { crate::server_browser::queue_host_local(ui, &lvl) };
    }
    if let Some(target_era) = switch_era {
        crate::ui_state::set_era(target_era);
        let menu_self = MENU_SELF.load(Ordering::Acquire) as *mut c_void;
        if !menu_self.is_null() {
            let arg: &[u8] = match target_era {
                ModeId::Vietnam => b"1\0",
                _ => b"0\0",
            };
            if let Some(&disp) = ORIG_MAIN_MENU_DISP.get() {
                unsafe {
                    disp(
                        menu_self,
                        std::ptr::null_mut(),
                        b"changeGameMod\0".as_ptr(),
                        arg.as_ptr(),
                    );
                }
            }
        }
    }

    static UI_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let ui_frame = UI_FRAMES.fetch_add(1, Ordering::Relaxed);
    if ui_frame == 0 {
        crate::log_line("UiFlags: first UI frame tick reached");
    }
    if ui_frame == 60 && !READY_SIGNALED.swap(true, Ordering::AcqRel) {
        crate::signal_engine_ready();
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
                let direct_lvl = AUTOSTART_LEVEL.lock().ok().and_then(|mut g| g.take());
                if let Some(lvl) = direct_lvl {
                    crate::ui_state::set_scene(crate::ui_state::GameScene::InMatch);
                    crate::log_line(&format!(
                        "UiFlags: direct-to-map launch -> queue_host_local('{lvl}')"
                    ));
                    unsafe { crate::server_browser::queue_host_local(ui, &lvl) };
                    let _ = SWITCH_DONE_AT.set(std::time::Instant::now());
                } else {
                    let active = STATE
                        .lock()
                        .ok()
                        .and_then(|g| g.as_ref().map(|s| s.active))
                        .unwrap_or(ModeId::Bfbc2);
                    // Only Vietnam uses Frostbite's native APT changeGameMod('1').
                    // BF1943 & BFBC1 render our hand-crafted in-game DX11 menus (no Vietnam graft!).
                    if active == ModeId::Vietnam {
                        crate::log_line(&format!(
                            "UiFlags: dispatching changeGameMod '1' for Vietnam (menuSelf={menu_self:p})"
                        ));
                        if let Some(&disp) = ORIG_MAIN_MENU_DISP.get() {
                            unsafe {
                                disp(
                                    menu_self,
                                    std::ptr::null_mut(),
                                    b"changeGameMod\0".as_ptr(),
                                    b"1\0".as_ptr(),
                                );
                            }
                            let _ = SWITCH_DONE_AT.set(std::time::Instant::now());
                            crate::log_line("UiFlags: changeGameMod dispatched cleanly");
                        }
                    } else {
                        // BF1943 / BFBC1 in-game custom menu is already live on Present!
                        crate::dx_overlay::mark_dirty();
                        let _ = SWITCH_DONE_AT.set(std::time::Instant::now());
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
    if let Some(name) = unsafe { win32::from_ansi_ptr(name_ptr) } {
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
        if let Some(val) = unsafe { crate::stats_screens::handle_extern(menu, &name) } {
            return val;
        }
    }
    if let Some(&orig) = ORIG_EXTERN_GETTER.get() {
        unsafe { orig(menu, edx, name_ptr) }
    } else {
        0
    }
}
