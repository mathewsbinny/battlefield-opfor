//! Project OPFOR — Re-Hosted 32-bit x86 Frostbite 1.5 Engine Host (`OPFOREngine.exe`).
//!
//! Linked at `/BASE:0x10000000 /FIXED /LARGEADDRESSAWARE /SAFESEH:NO` so that
//! `0x00400000..0x01D13000` is completely free for mapping `BFBC2Game.exe` (build 795745)
//! at its exact native un-relocated virtual address.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod airraid;
mod bf1943;
mod controller;
mod dx_overlay;
mod era_ui;
mod hooks;
mod imports;
mod levelfix;
mod menu_canvas;
mod onslaught_hud;
mod pacific;
mod pe_loader;
mod persistence;
mod profile;
mod prompts;
mod server_browser;
mod stats_screens;
mod ui_state;
mod win32;

use std::ffi::c_void;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};

use opfor_vfs::{ModeId, VfsRouter, discover_modules};

static LOG_TX: OnceLock<Mutex<Sender<String>>> = OnceLock::new();
static READY_FLAG_PATH: OnceLock<PathBuf> = OnceLock::new();

// Zero-disk-cost BSS arena spanning ~0x00331000..0x01D31000 (26 MB).
// Because `OPFOREngine.exe` links at `/BASE:0x00300000 /FIXED`, our host `.text/.rdata/.data`
// sit safely below `0x00400000`, while this uninitialized BSS block guarantees the Windows
// kernel reserves 100% of Frostbite's `0x00400000..0x01D13000` range before any heap or stack.
#[cfg(target_arch = "x86")]
#[used]
static mut FROSTBITE_ARENA: std::mem::MaybeUninit<[u8; 0x01A0_0000]> =
    std::mem::MaybeUninit::uninit();

// Reserve a zero-initialized per-thread TLS block in `OPFOREngine.exe` (slot 0).
// `BFBC2Game.exe` build 795745 uses only 9 zero-initialized static TLS bytes with
// `_tls_index = 0` (`0x0187AC7C`) and 0 TLS callbacks, so Windows automatically
// provisions `fs:[0x2C][0]` on every thread.
#[cfg(target_arch = "x86")]
thread_local! {
    static FROSTBITE_TLS_SLOT0: std::cell::Cell<[u8; 64]> = const { std::cell::Cell::new([0u8; 64]) };
}

fn init_async_logger(path: PathBuf) {
    let (tx, rx) = mpsc::channel::<String>();
    if LOG_TX.set(Mutex::new(tx)).is_ok() {
        let _ = std::thread::Builder::new()
            .name("opfor-logger".into())
            .spawn(move || {
                while let Ok(line) = rx.recv() {
                    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
                        let _ = writeln!(f, "[OPFOR-HOST] {line}");
                    }
                }
            });
    }
}

pub(crate) fn log_line(msg: &str) {
    if let Some(tx) = LOG_TX.get() {
        if let Ok(g) = tx.lock() {
            let _ = g.send(msg.to_string());
        }
    }
}

pub(crate) fn signal_engine_ready() {
    if let Some(p) = READY_FLAG_PATH.get() {
        let _ = fs::write(p, "ready\n");
        log_line(&format!("engine ready signaled -> {}", p.display()));
    }
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
        if rec.code == 0x406D_1388 {
            // MSVC SetThreadName debugger notification — safe to ignore when no debugger is attached
            return -1; // EXCEPTION_CONTINUE_EXECUTION
        }
        if rec.code == 0xC000_0005
            || rec.code == 0xC000_001D
            || rec.code == 0xC000_0094
            || rec.code == 0xC000_00FD
        {
            let rw = rec.info[0];
            let target = rec.info[1];
            // ContextRecord.Esp is at offset 0xC4 in Win32 x86 CONTEXT
            let mut stack_str = String::new();
            let mut reg_str = String::new();
            let ctx = unsafe { (*ep).context as *const u8 };
            if !ctx.is_null() {
                let edi = unsafe { *(ctx.add(0x9C) as *const usize) };
                let esi = unsafe { *(ctx.add(0xA0) as *const usize) };
                let ebx = unsafe { *(ctx.add(0xA4) as *const usize) };
                let ecx = unsafe { *(ctx.add(0xAC) as *const usize) };
                let eax = unsafe { *(ctx.add(0xB0) as *const usize) };
                let ebp = unsafe { *(ctx.add(0xB4) as *const usize) };
                reg_str = format!(" eax=0x{eax:08X} ecx=0x{ecx:08X} ebx=0x{ebx:08X} esi=0x{esi:08X} edi=0x{edi:08X} ebp=0x{ebp:08X}");
                // Only probe ClassInfo name when crashing at DBX createInstance (0x0049EA77)
                if rec.addr as usize == 0x0049_EA77
                    && (0x0040_0000..0x7FFF_0000).contains(&edi)
                    && (edi & 3) == 0
                {
                    let info_data = unsafe { *((edi + 4) as *const usize) };
                    if (0x0040_0000..0x7FFF_0000).contains(&info_data) && (info_data & 3) == 0 {
                        let name_ptr = unsafe { *(info_data as *const *const u8) };
                        if let Some(name) = unsafe { crate::win32::from_ansi_ptr(name_ptr) } {
                            if !name.is_empty() && name.len() < 96 && name.bytes().all(|b| b.is_ascii_graphic()) {
                                reg_str.push_str(&format!(" class='{name}'"));
                            }
                        }
                    }
                }
                let esp = unsafe { *(ctx.add(0xC4) as *const usize) };
                if esp > 0x10000 && (esp & 3) == 0 {
                    let sp = esp as *const u32;
                    for i in 0..64 {
                        let v = unsafe { *sp.add(i) };
                        if (0x0040_0000..0x01D2_0000).contains(&v) {
                            stack_str.push_str(&format!(" +{i}=0x{v:08X}"));
                        }
                    }
                }
            }
            let line = format!(
                "CRASH code=0x{:08X} eip={:p} rw={rw} target=0x{target:08X}{reg_str} fb_ret=[{}]",
                rec.code, rec.addr, stack_str.trim()
            );
            log_line(&line);
            if let Some(p) = READY_FLAG_PATH.get() {
                if let Some(root) = p.parent().and_then(|d| d.parent()) {
                    let crash_log = root.join("user").join("logs").join("opfor_crash.log");
                    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(crash_log) {
                        let _ = writeln!(f, "[OPFOR-HOST] {line}");
                        let _ = f.flush();
                    }
                }
            }
        }
    }
    0 // EXCEPTION_CONTINUE_SEARCH
}

fn detect_roots() -> (PathBuf, PathBuf) {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let exe_dir = exe.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    // If running from engine/OPFOREngine.exe -> opfor_root is parent; otherwise check ./engine
    if exe_dir.join("Dist").is_dir() || exe_dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("engine")) {
        let root = exe_dir.parent().unwrap_or(&exe_dir).to_path_buf();
        (root, exe_dir)
    } else {
        let engine_dir = exe_dir.join("engine");
        (exe_dir, engine_dir)
    }
}

fn find_payload(engine_dir: &Path) -> Option<PathBuf> {
    for name in ["frostbite.bin", "BFBC2Game.exe"] {
        let cand = engine_dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn main() {
    #[cfg(not(target_arch = "x86"))]
    {
        eprintln!("opfor-host must be compiled for i686-pc-windows-msvc (32-bit x86).");
        std::process::exit(1);
    }

    #[cfg(target_arch = "x86")]
    {
        // Keep `.0fb` arena and `.tls` slot 0 live in the linked image.
        let arena_va = (&raw const FROSTBITE_ARENA) as usize;
        FROSTBITE_TLS_SLOT0.with(|c| {
            let _ = std::hint::black_box(c.get());
        });

        let (root, engine_dir) = detect_roots();
        let _ = std::env::set_current_dir(&engine_dir);

        let log_dir = root.join("user").join("logs");
        let _ = fs::create_dir_all(&log_dir);
        init_async_logger(log_dir.join("opfor_host.log"));
        let _ = READY_FLAG_PATH.set(root.join("cache").join("engine_ready.flag"));

        let host_base = unsafe { win32::GetModuleHandleW(std::ptr::null()) } as usize;
        log_line(&format!(
            "=== OPFOREngine.exe host starting (host_base=0x{host_base:08X}, arena=0x{arena_va:08X}, root={}) ===",
            root.display()
        ));

        unsafe { AddVectoredExceptionHandler(1, veh_logger) };

        // Discover modules & initialize 4-mode table
        let modules = discover_modules(&root.join("modules")).unwrap_or_default();
        let mut installed_mask = ModeId::Bfbc2.bit();
        for m in &modules {
            if m.enabled && m.has_content_bundles() {
                if let Some(mid) = m.mode_id() {
                    installed_mask |= mid.bit();
                }
            }
        }
        if let Ok(mut guard) = hooks::STATE.lock() {
            *guard = Some(hooks::GameModeState::new(installed_mask));
        }

        // Read handoff from OPFOR.exe (`config/launch.ini`)
        let launch_ini = root.join("config").join("launch.ini");
        if let Ok(txt) = fs::read_to_string(&launch_ini) {
            for raw in txt.lines() {
                let line = raw.trim();
                if let Some(mode_str) = line.strip_prefix("mode=") {
                    if let Some(mid) = ModeId::from_id(mode_str) {
                        if let Ok(mut guard) = hooks::STATE.lock() {
                            if let Some(st) = guard.as_mut() {
                                let resolved = st.select_mode(mid);
                                log_line(&format!(
                                    "launch.ini mode={} (menu_code={})",
                                    st.table.spec(resolved).slug,
                                    st.menu_code()
                                ));
                            }
                        }
                    }
                } else if let Some(lvl) = line.strip_prefix("level=") {
                    let lvl = lvl.trim();
                    if !lvl.is_empty() {
                        hooks::set_autostart_level(lvl);
                        log_line(&format!("launch.ini direct autostart level='{lvl}'"));
                    }
                }
            }
        }

        // Initialize in-game Global UI State Manager + DX11 overlay canvas
        let initial_mode = hooks::STATE
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.active))
            .unwrap_or(ModeId::Bfbc2);
        ui_state::init(initial_mode);
        controller::init(&root);
        dx_overlay::init(&root);

        // Only hide stock BFBC2 intro movies / title screen for WW2 (1943) and BFBC1
        let skip_intro = matches!(initial_mode, ModeId::Bf1943 | ModeId::Bfbc1);

        // Initialize era-aware VFS (isolates per-game HUD/menu/minimap/font overlays to active era),
        // import overrides, offline server browser, and Onslaught HUD.
        let router = VfsRouter::for_era(&root, &modules, initial_mode, skip_intro);
        imports::init(&root, router);
        server_browser::init(&root);
        onslaught_hud::init(&root);

        let Some(payload) = find_payload(&engine_dir) else {
            log_line(&format!(
                "FATAL: no Frostbite payload (frostbite.bin / BFBC2Game.exe) in {}",
                engine_dir.display()
            ));
            std::process::exit(2);
        };

        log_line(&format!("mapping Frostbite payload: {}", payload.display()));
        let mapped = match unsafe { pe_loader::map_frostbite_payload(&payload) } {
            Ok(m) => m,
            Err(e) => {
                log_line(&format!("FATAL: PE loader error: {e}"));
                std::process::exit(3);
            }
        };
        log_line(&format!(
            "mapped 0x{:08X}..0x{:08X} ({} sections, {} DLLs, {} imports, {} OPFOR overrides, entry=0x{:08X})",
            mapped.image_base,
            mapped.image_base + mapped.size_of_image,
            mapped.section_count,
            mapped.dll_count,
            mapped.import_count,
            mapped.override_count,
            mapped.entry_point
        ));

        // Bind Frostbite's static `_tls_index` (`0x0187AC7C`) to slot 0 (our `.tls$AAA` block).
        unsafe {
            *(hooks::bc2_795745::TLS_INDEX_VA as *mut u32) = 0;
        }

        // Enable 1 ms Windows scheduler timer resolution for smoother Frostbite frame pacing
        unsafe { win32::timeBeginPeriod(1) };

        // Apply all verified Tier 1 build-795745 patches & detours BEFORE Frostbite runs
        let patched = unsafe { hooks::install_all() };
        let sb_hooks = unsafe { server_browser::install_hooks() };
        let prof_hooks = unsafe { profile::install(&root) };
        let persist_hooks = unsafe { persistence::install(&root) };
        let (lvl_hooks, lvl_expected) = unsafe { levelfix::install(initial_mode) };
        let pac_hooks = unsafe { pacific::install() };
        let bf43_hooks = unsafe { bf1943::install() };
        let ar_hooks = unsafe { airraid::install() };
        let prompt_hooks = unsafe { prompts::install() };
        let (era_ui_hooks, era_ui_expected) = unsafe { era_ui::install(initial_mode) };
        log_line(&format!(
            "Tier 1 build-795745 hooks installed: core={patched}/10, browser={sb_hooks}/1, profile={prof_hooks}/2, persistence={persist_hooks}/2, levelfix={lvl_hooks}/{lvl_expected}, pacific={pac_hooks}/5, bf1943={bf43_hooks}/4, airraid={ar_hooks}/2, prompts={prompt_hooks}/1, era_ui={era_ui_hooks}/{era_ui_expected}"
        ));

        // Enter Frostbite CRT startup (`0x00C52199`: WinMainCRTStartup)
        log_line(&format!("entering Frostbite at 0x{:08X}", mapped.entry_point));
        let entry: unsafe extern "system" fn() -> i32 =
            unsafe { std::mem::transmute(mapped.entry_point) };
        let exit_code = unsafe { entry() };
        log_line(&format!("Frostbite exited with code {exit_code}"));
        std::process::exit(exit_code);
    }
}
