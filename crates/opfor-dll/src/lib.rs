//! Project OPFOR 32-bit `dinput8.dll` proxy + Win32 Option B VFS + 4-Mode Hub core.

#![allow(non_snake_case)]

mod gamemode;
mod hook;
mod iat_vfs;
mod net_guard;
mod win32;
mod window_brand;

use std::ffi::c_void;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use opfor_vfs::{ModeId, VfsRouter, discover_modules};
use win32::{BOOL, DLL_PROCESS_ATTACH, DWORD, HMODULE, HRESULT, LPVOID};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static READY_FLAG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_LOCK: Mutex<()> = Mutex::new(());
static REAL_DINPUT8: OnceLock<usize> = OnceLock::new();

#[allow(dead_code)]
pub(crate) fn signal_engine_ready() {
    if let Some(p) = READY_FLAG_PATH.get() {
        let _ = fs::write(p, "ready\n");
        log_line(&format!("engine ready signaled -> {}", p.display()));
    }
}

pub(crate) fn log_line(msg: &str) {
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let _g = LOG_LOCK.lock();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[OPFOR] {msg}");
    }
}

fn detect_opfor_root(hmodule: HMODULE) -> Option<PathBuf> {
    let mut buf = [0u16; 1024];
    let len = unsafe { win32::GetModuleFileNameW(hmodule, buf.as_mut_ptr(), buf.len() as DWORD) };
    if len == 0 {
        return None;
    }
    let dll_path = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
    // engine/dinput8.dll -> parent = engine/, parent = battlefield-opfor/
    let engine_dir = dll_path.parent()?;
    Some(engine_dir.parent().unwrap_or(engine_dir).to_path_buf())
}

fn init(hmodule: HMODULE) {
    let Some(root) = detect_opfor_root(hmodule) else {
        return;
    };
    let log_dir = root.join("user").join("logs");
    let _ = fs::create_dir_all(&log_dir);
    let _ = LOG_PATH.set(log_dir.join("opfor.log"));
    let _ = READY_FLAG_PATH.set(root.join("cache").join("engine_ready.flag"));

    log_line(&format!("initialized; root={}", root.display()));

    let modules = discover_modules(&root.join("modules")).unwrap_or_default();
    let mut installed_mask = ModeId::Bfbc2.bit();
    for m in &modules {
        if m.enabled {
            if let Some(mid) = m.mode_id()
                && m.has_content_bundles()
            {
                installed_mask |= mid.bit();
            }
            log_line(&format!(
                "module discovered: {} (enabled={}, has_bundles={})",
                m.id,
                m.enabled,
                m.has_content_bundles()
            ));
        }
    }

    if let Ok(mut guard) = gamemode::STATE.lock() {
        *guard = Some(gamemode::GameModeState::new(installed_mask));
    }
    log_line(&format!(
        "4-mode table ready; extern.opforInstalledModes=0x{installed_mask:02x}"
    ));

    // Read handoff written by OPFOR.exe frontend (`config/launch.ini`) BEFORE creating VfsRouter
    // so per-era ingame APT menus (`levels/sp_common/ingame-00.fbrb`, `levels/mp_common/ingame-00.fbrb`,
    // `menu/hud/*`, `menu/shared/fonts_*`) are strictly isolated to the launched game!
    let mut active_mode = ModeId::Bfbc2;
    let launch_ini = root.join("config").join("launch.ini");
    if let Ok(txt) = fs::read_to_string(&launch_ini) {
        for raw in txt.lines() {
            let line = raw.trim();
            if let Some(mode_str) = line.strip_prefix("mode=")
                && let Some(mid) = ModeId::from_id(mode_str)
                && let Ok(mut guard) = gamemode::STATE.lock()
                && let Some(st) = guard.as_mut()
            {
                active_mode = st.select_mode(mid);
                log_line(&format!("launch.ini preselected mode={}", st.table.spec(active_mode).slug));
            } else if let Some(lvl) = line.strip_prefix("level=")
                && !lvl.is_empty()
            {
                log_line(&format!("launch.ini target level={lvl}"));
            }
        }
    }

    let skip_intro = matches!(active_mode, ModeId::Bfbc1 | ModeId::Bf1943);
    let router = VfsRouter::for_era(&root, &modules, active_mode, skip_intro);
    let patched = iat_vfs::install(router, false);
    log_line(&format!(
        "Win32 IAT VFS hooks installed: {patched}/5 slots (era={:?}, skip_intro={skip_intro})",
        active_mode
    ));

    let net_slots = net_guard::install();
    log_line(&format!("Offline network guard installed: {net_slots} IAT slots"));

    let brand_slots = window_brand::install(&root);
    log_line(&format!("Window branding (opfor.ico + era title) installed: {brand_slots} IAT slots"));

    let is_bc2 = unsafe { gamemode::is_bfbc2_795745() };
    log_line(&format!("BFBC2Game.exe build 795745 signature match: {is_bc2}"));
    if is_bc2 {
        let count = unsafe { gamemode::install_all() };
        log_line(&format!("Tier 1 build-795745 hooks/patches installed: {count}/6"));
    }
}

/// Win32 DLL entry point for the `dinput8.dll` VFS & 4-Mode proxy.
///
/// # Safety
/// Invoked directly by the Windows loader with a valid module handle `hmodule`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(hmodule: HMODULE, reason: DWORD, _reserved: LPVOID) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            win32::DisableThreadLibraryCalls(hmodule);
        }
        init(hmodule);
    }
    1
}

/// Real `dinput8.dll` forwarding proxy export required by `BFBC2Game.exe`.
///
/// # Safety
/// Caller must pass valid DirectInput8 COM pointers matching Microsoft's `DirectInput8Create` ABI.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DirectInput8Create(
    hinst: HMODULE,
    dwVersion: DWORD,
    riidltf: *const c_void,
    ppvOut: *mut *mut c_void,
    punkOuter: *mut c_void,
) -> HRESULT {
    let real_mod = (*REAL_DINPUT8.get_or_init(|| {
        let mut sys = [0u16; 512];
        let len = unsafe { win32::GetSystemDirectoryW(sys.as_mut_ptr(), sys.len() as DWORD) };
        let mut path = String::from_utf16_lossy(&sys[..len as usize]);
        path.push_str(r"\dinput8.dll");
        let w = win32::to_wide_nul(&path);
        unsafe { win32::LoadLibraryW(w.as_ptr()) as usize }
    })) as HMODULE;
    if real_mod.is_null() {
        return -2147467259; // E_FAIL
    }
    let proc = unsafe { win32::GetProcAddress(real_mod, c"DirectInput8Create".as_ptr().cast()) };
    if proc.is_null() {
        return -2147467259;
    }
    type DirectInput8CreateFn = unsafe extern "system" fn(
        HMODULE,
        DWORD,
        *const c_void,
        *mut *mut c_void,
        *mut c_void,
    ) -> HRESULT;
    let f: DirectInput8CreateFn = unsafe { std::mem::transmute(proc) };
    unsafe { f(hinst, dwVersion, riidltf, ppvOut, punkOuter) }
}
