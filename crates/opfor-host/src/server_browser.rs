//! Offline Matches Server Browser & `hostMultiplayerLocal` launcher for `opfor-host`.
//!
//! Ported cleanly from `project-opfor-bfbc2/src/dll/server_browser.cpp` + `opfor.cpp`:
//! - Loads `config/maps.ini` and filters matches by the active era (`BFBC2`, `Vietnam`, `BF1943`, `BFBC1`).
//! - Handles `postGetServers`, `serverFilterSettings` auto-fill, and `startServerListMatch`.
//! - Launches selected offline matches in-process via `StartRequest` (`type = 1`, `hostMultiplayerLocal`).

#![allow(dead_code, non_snake_case)]

use std::ffi::{CString, c_void};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use opfor_vfs::ModeId;

const OPFOR_SERVER_ID: i32 = 0x4F50_4652; // 'OPFR'

// Verified BFBC2Game.exe build 795745 addresses (from bc2.h)
const UI_MANAGER_GLOBAL: usize = 0x019A_4CA8;
const UI_SERVER_LIST: usize = 0x000C_8958;
const UI_QUERY_DONE: usize = 0x000C_3FBC;

const SERVER_RECORD_SIZE: usize = 0xF0;
const SERVER_RECORD_CTOR: usize = 0x005E_CF60;
const SERVER_RECORD_DTOR: usize = 0x005E_D370;
const REC_ADDRESS: usize = 0x48;
const REC_NAME: usize = 0x54;
const REC_LEVEL: usize = 0x60;
const REC_GAME_MODE: usize = 0x6C;
const REC_PLAYERS: usize = 0x84;
const REC_MAX_PLAYERS: usize = 0x88;
const REC_PING: usize = 0x90;
const REC_ID_A: usize = 0xE0;
const REC_ID_B: usize = 0xE4;

const ADD_SERVER_ROW: usize = 0x0095_6A80;
const WRITE_SERVER_ROWS: usize = 0x0095_8BF0;
const ROW_MODE_TEXT: usize = 0x0095_6C61;
const ROW_MODE_TEXT_END: usize = 0x0095_6C67;
const ROW_MODE_TEXT_BYTES: [u8; 6] = [0x53, 0xE8, 0x59, 0x59, 0x62, 0x00]; // push ebx; call 0xF7C5C0 (MakeString)
const MAKE_STRING: usize = 0x012F_BB40;
const SEND_MENU_EVENT: usize = 0x0092_9A70;
const ACQUIRE_MENU_EVENT: usize = 0x0092_0360;
const EVENT_ADD_STRING: usize = 0x0058_8910;
const EVENT_ADD_VALUE: usize = 0x0058_8940;
const QUEUE_MENU_EVENT: usize = 0x0092_99B0;
const MAKE_BOOL: usize = 0x00F7_C600;

const START_REQUEST_SIZE: usize = 0x110;
const START_REQUEST_CTOR: usize = 0x0075_DF60;
const START_REQUEST_DTOR: usize = 0x0075_EAB0;
const QUEUE_START_REQUEST: usize = 0x0095_6E10;
const START_TYPE_HOST_LOCAL: i32 = 1;
const START_TYPE_SINGLEPLAYER: i32 = 3;

const STRING_ASSIGN_CSTR: usize = 0x0040_14C0;
const STRING_ASSIGN: usize = 0x0042_5080;
const STRING_DTOR: usize = 0x0040_1470;

#[repr(C)]
struct Bc2String {
    begin: *mut u8,
    end: *mut u8,
    capacity: *mut u8,
}

#[derive(Debug, Clone)]
pub struct OfflineMatch {
    pub map_name: String,
    pub mode_name: String,
    pub level_path: String,
    pub mode_key: String,
    pub max_players: i32,
    pub era: ModeId,
}

struct BrowserTimers {
    auto_fill_at: Option<(usize, Instant)>,
    repeat_query_end_at: Option<(usize, Instant)>,
    last_fill: Option<Instant>,
}

static MATCHES: OnceLock<Vec<OfflineMatch>> = OnceLock::new();
static TIMERS: Mutex<BrowserTimers> = Mutex::new(BrowserTimers {
    auto_fill_at: None,
    repeat_query_end_at: None,
    last_fill: None,
});

fn classify_era(section: &str, level_path: &str) -> ModeId {
    let s = section.to_ascii_uppercase();
    let l = level_path.to_ascii_uppercase();
    if s.starts_with("NAM_") || l.contains("/NAM_") {
        ModeId::Vietnam
    } else if s.starts_with("WAKE_")
        || s.starts_with("GUADAL")
        || s.starts_with("IWO_")
        || s.starts_with("CORAL_")
        || l.contains("WAKE_ISLAND")
        || l.contains("GUADAL_CANAL")
        || l.contains("IWO_JIMA")
        || l.contains("CORAL_SEA")
    {
        ModeId::Bf1943
    } else if s.starts_with("BFBC1_")
        || l.contains("MP_BEACHHEAD_")
        || l.contains("MP_HARBOUR_")
        || l.contains("MP_IGNITION_")
        || l.contains("MP_MONASTERY_")
        || l.contains("SP_01_GREENACRES")
        || l.contains("SP_02_TRAITOR")
        || l.contains("SP_03_CROSSROADS")
        || l.contains("SP_05_PAR_FOR_THE_COURSE")
        || l.contains("SP_06_HOT_ZONE")
        || l.contains("SP_07_POW")
        || l.contains("SP_08_GHOST_TOWN")
    {
        ModeId::Bfbc1
    } else {
        ModeId::Bfbc2
    }
}

fn mode_key_and_players(mode_name: &str) -> (&'static str, i32) {
    match mode_name.to_ascii_lowercase().as_str() {
        "conquest" => ("CONQUEST", 32),
        "rush" => ("RUSH", 32),
        "gold rush" => ("RUSH", 24),
        "campaign" => ("SP", 1),
        "squad rush" => ("SQRUSH", 8),
        "squad deathmatch" => ("SQDM", 16),
        "onslaught" => ("RUSH", 4),
        "air superiority" => ("CONQUEST", 24),
        _ => ("CONQUEST", 32),
    }
}

/// Load `config/maps.ini` into the offline match catalogue.
pub fn init(opfor_root: &Path) {
    let path = opfor_root.join("config").join("maps.ini");
    let mut list = Vec::new();
    if let Ok(txt) = fs::read_to_string(&path) {
        let mut section = String::new();
        let mut map_name = String::new();
        for raw in txt.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_string();
                map_name = section.clone();
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                if k.eq_ignore_ascii_case("name") {
                    map_name = v.to_string();
                } else if !section.is_empty() && !v.is_empty() {
                    let (mode_key, max_players) = mode_key_and_players(k);
                    let era = classify_era(&section, v);
                    list.push(OfflineMatch {
                        map_name: map_name.clone(),
                        mode_name: k.to_string(),
                        level_path: v.to_string(),
                        mode_key: mode_key.to_string(),
                        max_players,
                        era,
                    });
                }
            }
        }
    }
    crate::log_line(&format!(
        "server browser: loaded {} offline matches from {}",
        list.len(),
        path.display()
    ));
    let _ = MATCHES.set(list);
}

#[cfg(target_arch = "x86")]
unsafe fn ui_manager() -> *mut c_void {
    unsafe { *(UI_MANAGER_GLOBAL as *const *mut c_void) }
}

#[cfg(target_arch = "x86")]
unsafe fn assign_bc2_string(dst: *mut u8, text: &str) {
    if let Ok(cs) = CString::new(text) {
        type AssignCStrFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
        let f: AssignCStrFn = unsafe { std::mem::transmute(STRING_ASSIGN_CSTR) };
        unsafe { f(dst.cast(), std::ptr::null_mut(), cs.as_ptr().cast()) };
    }
}

#[cfg(target_arch = "x86")]
unsafe fn send_query_end(ui: *mut c_void, success: bool) {
    type AcquireFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type AddStrFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
    type AddValFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, u32);
    type QueueEvtFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void);
    type MakeBoolFn = unsafe extern "cdecl" fn(i32) -> u32;

    let acquire: AcquireFn = unsafe { std::mem::transmute(ACQUIRE_MENU_EVENT) };
    let add_str: AddStrFn = unsafe { std::mem::transmute(EVENT_ADD_STRING) };
    let add_val: AddValFn = unsafe { std::mem::transmute(EVENT_ADD_VALUE) };
    let queue: QueueEvtFn = unsafe { std::mem::transmute(QUEUE_MENU_EVENT) };
    let make_bool: MakeBoolFn = unsafe { std::mem::transmute(MAKE_BOOL) };

    let evt = unsafe { acquire(ui, std::ptr::null_mut()) };
    if evt.is_null() {
        return;
    }
    unsafe {
        add_str(evt, std::ptr::null_mut(), b"queryEnd\0".as_ptr());
        add_val(evt, std::ptr::null_mut(), make_bool(i32::from(success)));
        queue(ui, std::ptr::null_mut(), evt);
    }
}

#[cfg(target_arch = "x86")]
unsafe fn clear_server_list(ui: *mut c_void) {
    type RecDtorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
    let dtor: RecDtorFn = unsafe { std::mem::transmute(SERVER_RECORD_DTOR) };
    let list = unsafe { (ui as *mut u8).add(UI_SERVER_LIST) as *mut *mut u8 };
    unsafe {
        let begin = *list;
        let end = *list.add(1);
        if !begin.is_null() && end > begin {
            let mut cur = begin;
            while cur < end {
                dtor(cur.cast(), std::ptr::null_mut());
                cur = cur.add(SERVER_RECORD_SIZE);
            }
        }
        *list.add(1) = begin;
    }
}

#[cfg(target_arch = "x86")]
pub unsafe fn fill_server_list(ui: *mut c_void) {
    if ui.is_null() {
        return;
    }
    let Some(matches) = MATCHES.get() else {
        return;
    };
    let active_mode = crate::hooks::STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.active))
        .unwrap_or(ModeId::Bfbc2);

    type RecCtorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
    type RecDtorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
    type AddRowFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8, i32);
    type WriteRowsFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, i32);
    type SendEvtFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);

    let ctor: RecCtorFn = unsafe { std::mem::transmute(SERVER_RECORD_CTOR) };
    let dtor: RecDtorFn = unsafe { std::mem::transmute(SERVER_RECORD_DTOR) };
    let add_row: AddRowFn = unsafe { std::mem::transmute(ADD_SERVER_ROW) };
    let write_rows: WriteRowsFn = unsafe { std::mem::transmute(WRITE_SERVER_ROWS) };
    let send_evt: SendEvtFn = unsafe { std::mem::transmute(SEND_MENU_EVENT) };

    unsafe {
        send_evt(ui, std::ptr::null_mut(), b"queryBegin\0".as_ptr());
        clear_server_list(ui);
    }

    let mut listed = 0usize;
    for (idx, m) in matches.iter().enumerate() {
        if m.era != active_mode {
            continue;
        }
        listed += 1;
        let mut rec = [0u8; SERVER_RECORD_SIZE];
        unsafe {
            ctor(rec.as_mut_ptr().cast(), std::ptr::null_mut());
            let title = format!("OPFOR - {} ({})", m.map_name, m.mode_name);
            assign_bc2_string(rec.as_mut_ptr().add(REC_NAME), &title);
            assign_bc2_string(rec.as_mut_ptr().add(REC_ADDRESS), "127.0.0.1");
            assign_bc2_string(rec.as_mut_ptr().add(REC_LEVEL), &m.level_path);
            assign_bc2_string(rec.as_mut_ptr().add(REC_GAME_MODE), &m.mode_key);
            *(rec.as_mut_ptr().add(REC_PLAYERS) as *mut i32) = 0;
            *(rec.as_mut_ptr().add(REC_MAX_PLAYERS) as *mut i32) = m.max_players;
            *(rec.as_mut_ptr().add(REC_PING) as *mut i32) = 0;
            *(rec.as_mut_ptr().add(REC_ID_A) as *mut i32) = OPFOR_SERVER_ID;
            *(rec.as_mut_ptr().add(REC_ID_B) as *mut i32) = idx as i32;
            add_row(ui, std::ptr::null_mut(), rec.as_ptr(), 0);
            dtor(rec.as_mut_ptr().cast(), std::ptr::null_mut());
        }
    }

    unsafe {
        let list_ptr = (ui as *mut u8).add(UI_SERVER_LIST);
        write_rows(ui, std::ptr::null_mut(), list_ptr.cast(), 1);
        *(ui as *mut u8).add(UI_QUERY_DONE) = 1;
        send_query_end(ui, true);
    }

    crate::log_line(&format!(
        "server browser: populated {listed} offline matches for mode={active_mode:?}"
    ));

    if let Ok(mut t) = TIMERS.lock() {
        t.auto_fill_at = None;
        t.last_fill = Some(Instant::now());
        t.repeat_query_end_at = Some((ui as usize, Instant::now() + Duration::from_millis(250)));
    }
}

#[cfg(target_arch = "x86")]
pub unsafe fn queue_host_local(ui: *mut c_void, level_path: &str) {
    if ui.is_null() {
        return;
    }
    type CtorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
    type DtorFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
    type QueueFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
    type StrAssignCStrFn = unsafe extern "fastcall" fn(*mut Bc2String, *mut c_void, *const u8);
    type StrAssignFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const Bc2String);
    type StrDtorFn = unsafe extern "fastcall" fn(*mut Bc2String, *mut c_void);

    let ctor: CtorFn = unsafe { std::mem::transmute(START_REQUEST_CTOR) };
    let dtor: DtorFn = unsafe { std::mem::transmute(START_REQUEST_DTOR) };
    let queue: QueueFn = unsafe { std::mem::transmute(QUEUE_START_REQUEST) };
    let str_assign_c: StrAssignCStrFn = unsafe { std::mem::transmute(STRING_ASSIGN_CSTR) };
    let str_assign: StrAssignFn = unsafe { std::mem::transmute(STRING_ASSIGN) };
    let str_dtor: StrDtorFn = unsafe { std::mem::transmute(STRING_DTOR) };

    let is_sp = level_path.to_ascii_lowercase().contains("/sp_")
        || level_path.to_ascii_lowercase().starts_with("sp_");
    let start_type = if is_sp {
        START_TYPE_SINGLEPLAYER
    } else {
        START_TYPE_HOST_LOCAL
    };
    // `case 3` (0x00951FEC) prepends "Levels/" automatically; passing "Levels/SP_01"
    // produces "Levels/Levels/SP_01" and breaks LevelDescriptionAsset lookup.
    let effective_path = if is_sp {
        level_path
            .strip_prefix("Levels/")
            .or_else(|| level_path.strip_prefix("levels/"))
            .or_else(|| level_path.strip_prefix("Levels\\"))
            .or_else(|| level_path.strip_prefix("levels\\"))
            .unwrap_or(level_path)
    } else {
        level_path
    };
    let Ok(cs) = CString::new(effective_path) else {
        return;
    };

    let mut req = [0u8; START_REQUEST_SIZE];
    let mut lvl = Bc2String {
        begin: std::ptr::null_mut(),
        end: std::ptr::null_mut(),
        capacity: std::ptr::null_mut(),
    };
    unsafe {
        ctor(req.as_mut_ptr().cast(), std::ptr::null_mut());
        *(req.as_mut_ptr() as *mut i32) = start_type;
        str_assign_c(&mut lvl, std::ptr::null_mut(), cs.as_ptr().cast());
        str_assign(req.as_mut_ptr().add(4).cast(), std::ptr::null_mut(), &lvl);
        str_dtor(&mut lvl, std::ptr::null_mut());
        queue(ui, std::ptr::null_mut(), req.as_ptr());
        dtor(req.as_mut_ptr().cast(), std::ptr::null_mut());
    }
    crate::onslaught_hud::on_level_started(level_path);
    crate::log_line(&format!(
        "server browser: queued StartRequest(type={start_type}) level='{effective_path}'"
    ));
}

#[cfg(target_arch = "x86")]
unsafe fn join_offline_match(ui: *mut c_void, args: &str) -> bool {
    if ui.is_null() {
        return false;
    }
    let row_str = args.split(',').next().unwrap_or("").trim();
    let Ok(row) = row_str.parse::<usize>() else {
        return false;
    };
    let list = unsafe { (ui as *mut u8).add(UI_SERVER_LIST) as *const *const u8 };
    let (begin, end) = unsafe { (*list, *list.add(1)) };
    if begin.is_null() || end <= begin {
        return false;
    }
    let count = (end as usize - begin as usize) / SERVER_RECORD_SIZE;
    if row >= count {
        return false;
    }
    let rec = unsafe { begin.add(row * SERVER_RECORD_SIZE) };
    let id_a = unsafe { *(rec.add(REC_ID_A) as *const i32) };
    if id_a != OPFOR_SERVER_ID {
        return false;
    }
    let id_b = unsafe { *(rec.add(REC_ID_B) as *const i32) } as usize;
    if let Some(matches) = MATCHES.get() {
        if let Some(m) = matches.get(id_b) {
            unsafe { queue_host_local(ui, &m.level_path) };
            return true;
        }
    }
    false
}

/// Pick a BFBC1 offline level to launch for Quick Match / Host / Campaign commands.
#[cfg(target_arch = "x86")]
fn resolve_bfbc1_level(args: &str, want_sp: bool) -> String {
    let trimmed = args.split(',').next().unwrap_or("").trim();
    if let Some(matches) = MATCHES.get() {
        if !trimmed.is_empty() {
            let needle = trimmed.to_ascii_lowercase();
            if let Some(m) = matches.iter().find(|m| {
                m.era == ModeId::Bfbc1
                    && (m.level_path.to_ascii_lowercase().contains(&needle)
                        || m.map_name.to_ascii_lowercase().contains(&needle))
            }) {
                return m.level_path.clone();
            }
        }
        if let Some(m) = matches.iter().find(|m| {
            let is_sp = m.level_path.to_ascii_lowercase().contains("/sp_");
            m.era == ModeId::Bfbc1 && is_sp == want_sp
        }) {
            return m.level_path.clone();
        }
    }
    if want_sp {
        "Levels/SP_01_GreenAcres".to_string()
    } else {
        "Levels/MP_Beachhead_Day1".to_string()
    }
}

/// Intercept main-menu FSCommands for the Offline Server Browser.
/// Returns `true` if handled (skip original dispatcher).
#[cfg(target_arch = "x86")]
pub unsafe fn handle_menu_command(name: &str, args: &str) -> bool {
    let ui = unsafe { ui_manager() };
    if ui.is_null() {
        return false;
    }
    let active_mode = crate::hooks::STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.active))
        .unwrap_or(ModeId::Bfbc2);

    if active_mode == ModeId::Bfbc1 {
        match name {
            "startFindServer" => {
                unsafe { fill_server_list(ui) };
                return true;
            }
            "startQuickMatch" | "hostMultiplayer" | "joinMultiplayer" | "joinSession" => {
                let lvl = resolve_bfbc1_level(args, false);
                crate::log_line(&format!("server browser(BFBC1): '{name}' -> host local '{lvl}'"));
                unsafe { queue_host_local(ui, &lvl) };
                return true;
            }
            "newSP" | "continueSP" => {
                let lvl = resolve_bfbc1_level(args, true);
                crate::log_line(&format!("server browser(BFBC1): '{name}' -> start SP '{lvl}'"));
                unsafe { queue_host_local(ui, &lvl) };
                return true;
            }
            _ => {}
        }
    }

    match name {
        "serverFilterSettings" => {
            if let Ok(mut t) = TIMERS.lock() {
                let recently_filled = t
                    .last_fill
                    .is_some_and(|lf| lf.elapsed() < Duration::from_millis(500));
                if !recently_filled {
                    t.auto_fill_at = Some((ui as usize, Instant::now() + Duration::from_millis(150)));
                }
            }
            false
        }
        "postGetServers" => {
            unsafe { fill_server_list(ui) };
            true
        }
        "startServerListMatch" => unsafe { join_offline_match(ui, args) },
        _ => false,
    }
}

/// Called once per UI frame from `detour_ui_flags` to service delayed auto-fill and second `queryEnd`.
#[cfg(target_arch = "x86")]
pub unsafe fn on_ui_frame(ui: *mut c_void) {
    if ui.is_null() {
        return;
    }
    let now = Instant::now();
    let mut do_fill = false;
    let mut do_repeat = false;
    if let Ok(mut t) = TIMERS.lock() {
        if let Some((target, at)) = t.auto_fill_at {
            if now >= at {
                t.auto_fill_at = None;
                do_fill = target == ui as usize;
            }
        }
        if let Some((target, at)) = t.repeat_query_end_at {
            if now >= at {
                t.repeat_query_end_at = None;
                do_repeat = target == ui as usize;
            }
        }
    }
    if do_fill {
        unsafe { fill_server_list(ui) };
    }
    if do_repeat {
        unsafe { send_query_end(ui, true) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn row_mode_text(rec: *const u8, original: *const u8) -> *const u8 {
    if rec.is_null() {
        return original;
    }
    let id_a = unsafe { *(rec.add(REC_ID_A) as *const i32) };
    if id_a != OPFOR_SERVER_ID {
        return original;
    }
    let idx = unsafe { *(rec.add(REC_ID_B) as *const i32) } as usize;
    if let Some(matches) = MATCHES.get() {
        if let Some(m) = matches.get(idx) {
            if m.mode_name.eq_ignore_ascii_case("Onslaught") {
                return b"ONSLAUGHT\0".as_ptr();
            }
            if m.mode_name.eq_ignore_ascii_case("Air Superiority") {
                return b"AIR SUPERIORITY\0".as_ptr();
            }
            if m.mode_name.eq_ignore_ascii_case("Gold Rush") {
                return b"GOLD RUSH\0".as_ptr();
            }
            if m.mode_name.eq_ignore_ascii_case("Campaign") {
                return b"CAMPAIGN\0".as_ptr();
            }
        }
    }
    original
}

/// Patch `0x00956C61` so Onslaught and Air Superiority rows display their true mode text.
#[cfg(target_arch = "x86")]
pub unsafe fn install_hooks() -> usize {
    use crate::hooks;
    use crate::win32::{self, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE};

    if !unsafe { hooks::bytes_match(ROW_MODE_TEXT, &ROW_MODE_TEXT_BYTES) } {
        return 0;
    }
    let stub = unsafe {
        win32::VirtualAlloc(
            std::ptr::null_mut(),
            64,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        ) as *mut u8
    };
    if stub.is_null() {
        return 0;
    }
    let mut code = Vec::with_capacity(40);
    code.push(0x53); // push ebx (original text)
    code.push(0x57); // push edi (record)
    code.push(0xB8); // mov eax, row_mode_text
    code.extend_from_slice(&(row_mode_text as *const () as u32).to_le_bytes());
    code.extend_from_slice(&[0xFF, 0xD0]); // call eax
    code.extend_from_slice(&[0x83, 0xC4, 0x08]); // add esp, 8
    code.push(0x50); // push eax (replaces original push ebx)
    code.push(0xB8); // mov eax, MakeString
    code.extend_from_slice(&(MAKE_STRING as u32).to_le_bytes());
    code.extend_from_slice(&[0xFF, 0xD0]); // call eax
    let jmp_pos = stub as usize + code.len();
    code.push(0xE9);
    let back = (ROW_MODE_TEXT_END as isize - (jmp_pos as isize + 5)) as i32;
    code.extend_from_slice(&back.to_le_bytes());

    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), stub, code.len());
        win32::FlushInstructionCache(win32::GetCurrentProcess(), stub.cast(), code.len());
    }

    let mut site = [0x90u8; 6];
    site[0] = 0xE9;
    let rel = (stub as isize - (ROW_MODE_TEXT as isize + 5)) as i32;
    site[1..5].copy_from_slice(&rel.to_le_bytes());
    if unsafe { hooks::write_code(ROW_MODE_TEXT, &site) } {
        crate::log_line("server browser: mode-column text patch installed (ONSLAUGHT / AIR SUPERIORITY)");
        1
    } else {
        0
    }
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn install_hooks() -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_four_eras() {
        assert_eq!(classify_era("MP_001", "Levels/MP_001"), ModeId::Bfbc2);
        assert_eq!(classify_era("BC1_OASIS", "Levels/BC1_Oasis_CQ"), ModeId::Bfbc2);
        assert_eq!(classify_era("NAM_MP_002", "Levels/NAM_MP_002CQ"), ModeId::Vietnam);
        assert_eq!(classify_era("WAKE_ISLAND_S", "Levels/WAKE_ISLAND_S"), ModeId::Bf1943);
        assert_eq!(classify_era("BFBC1_OASIS", "Levels/MP_Beachhead_Day1"), ModeId::Bfbc1);
        assert_eq!(classify_era("BFBC1_SP_01", "Levels/SP_01_GreenAcres"), ModeId::Bfbc1);
    }
}
