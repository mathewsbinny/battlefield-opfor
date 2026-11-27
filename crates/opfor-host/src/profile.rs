//! Offline Player Profile subsystem (`user/profile.ini`) for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/profile.cpp`:
//! - Handles `presenceRequestOnline` (opens offline profile dialog) and `presenceLoginAccount` (saves).
//! - Hooks `Client::sendJoin` (`0x0055DFB0`) to inject soldier name and clan tag.
//! - Replaces offline identity vtable slot `0x01433B3C` (`0x00841930`) so the scoreboard shows
//!   the player's profile name instead of `"Human player"`.

#![allow(dead_code, non_snake_case)]

use std::ffi::{CString, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const CLIENT_SEND_JOIN: usize = 0x0055_DFB0;
const CLIENT_SEND_JOIN_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF8];
const CLIENT_PLAYER_NAME_OFF: usize = 0x1604;

const IDENTITY_SLOT: usize = 0x0143_3B3C;
const IDENTITY_METHOD: usize = 0x0084_1930;

const ONLINE_SINGLETON: usize = 0x0157_13D8;
const ONLINE_PROFILE_OFF: usize = 0x54;
const SET_PROFILE_CLAN_TAG: usize = 0x0059_EDD0;

const STRING_ASSIGN_CSTR: usize = 0x0040_14C0;
const STRING_ASSIGN: usize = 0x0042_5080;
const STRING_DTOR: usize = 0x0040_1470;

const UI_MANAGER_GLOBAL: usize = 0x019A_4CA8;
const SEND_MENU_EVENT: usize = 0x0092_9A70;
const ACQUIRE_MENU_EVENT: usize = 0x0092_0360;
const EVENT_ADD_STRING: usize = 0x0058_8910;
const EVENT_ADD_VALUE: usize = 0x0058_8940;
const QUEUE_MENU_EVENT: usize = 0x0092_99B0;
const MAKE_STRING: usize = 0x012F_BB40;

#[repr(C)]
struct Bc2String {
    begin: *mut u8,
    end: *mut u8,
    capacity: *mut u8,
}

struct ProfileData {
    ini_path: PathBuf,
    name: String,
    clan_tag: String,
}

static PROFILE: Mutex<Option<ProfileData>> = Mutex::new(None);

#[cfg(target_arch = "x86")]
type ClientSendJoinFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void);
#[cfg(target_arch = "x86")]
type IdentityFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, i32, *mut u8) -> bool;

#[cfg(target_arch = "x86")]
static ORIG_SEND_JOIN: OnceLock<ClientSendJoinFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_IDENTITY: OnceLock<IdentityFn> = OnceLock::new();

fn clean_name(raw: &str) -> String {
    let s: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .take(20)
        .collect();
    if s.len() >= 2 {
        s
    } else {
        "Player".to_string()
    }
}

fn clean_tag(raw: &str) -> String {
    let t = raw.trim();
    if t == "-1" {
        return String::new();
    }
    t.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(8)
        .collect()
}

fn load_ini(path: &Path) -> (String, String) {
    let mut name = "Player".to_string();
    let mut tag = "OPFOR".to_string();
    if let Ok(txt) = fs::read_to_string(path) {
        for raw in txt.lines() {
            let line = raw.trim();
            if let Some((k, v)) = line.split_once('=') {
                match k.trim().to_ascii_lowercase().as_str() {
                    "name" => name = clean_name(v),
                    "clantag" | "clan_tag" => tag = clean_tag(v),
                    _ => {}
                }
            }
        }
    }
    (name, tag)
}

fn save_ini(p: &ProfileData) {
    if let Some(parent) = p.ini_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = format!("[profile]\nname={}\nclantag={}\n", p.name, p.clan_tag);
    let _ = fs::write(&p.ini_path, content);
}

pub fn current_name() -> String {
    PROFILE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.name.clone()))
        .unwrap_or_else(|| "Player".to_string())
}

pub fn current_tag() -> String {
    PROFILE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.clan_tag.clone()))
        .unwrap_or_default()
}

pub unsafe fn install(opfor_root: &Path) -> usize {
    let ini_path = opfor_root.join("user").join("profile.ini");
    let (name, clan_tag) = load_ini(&ini_path);
    crate::log_line(&format!("profile: loaded name='{name}', clantag='{clan_tag}'"));
    if let Ok(mut g) = PROFILE.lock() {
        *g = Some(ProfileData {
            ini_path,
            name,
            clan_tag,
        });
    }

    let mut installed = 0usize;
    #[cfg(target_arch = "x86")]
    {
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                CLIENT_SEND_JOIN,
                &CLIENT_SEND_JOIN_PROLOGUE,
                detour_client_send_join as *const c_void,
            )
        } {
            let _ = ORIG_SEND_JOIN
                .set(unsafe { std::mem::transmute::<*const c_void, ClientSendJoinFn>(tramp) });
            installed += 1;
        }

        let cur_slot = unsafe { *(IDENTITY_SLOT as *const usize) };
        if cur_slot == IDENTITY_METHOD {
            let _ = ORIG_IDENTITY.set(unsafe { std::mem::transmute(IDENTITY_METHOD) });
            let detour_addr = (detour_identity as *const () as usize).to_le_bytes();
            if unsafe { crate::hooks::write_code(IDENTITY_SLOT, &detour_addr) } {
                installed += 1;
                crate::log_line("profile: scoreboard identity slot patched (replaces 'Human player')");
            }
        }
    }
    installed
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_identity(
    self_ptr: *mut c_void,
    edx: *mut c_void,
    arg: i32,
    out: *mut u8,
) -> bool {
    let ok = if let Some(&orig) = ORIG_IDENTITY.get() {
        unsafe { orig(self_ptr, edx, arg, out) }
    } else {
        false
    };
    if ok && !out.is_null() {
        let name = current_name();
        let bytes = name.as_bytes();
        let n = bytes.len().min(0x80);
        unsafe {
            let dst = out.add(8);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, n);
            *dst.add(n) = 0;
        }
    }
    ok
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_client_send_join(client: *mut c_void, edx: *mut c_void) {
    crate::ui_state::set_scene(crate::ui_state::GameScene::InMatch);
    if !client.is_null() {
        type StrAssignCStrFn = unsafe extern "fastcall" fn(*mut Bc2String, *mut c_void, *const u8);
        type StrAssignFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const Bc2String);
        type StrDtorFn = unsafe extern "fastcall" fn(*mut Bc2String, *mut c_void);
        type SetClanFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const Bc2String);

        let assign_c: StrAssignCStrFn = unsafe { std::mem::transmute(STRING_ASSIGN_CSTR) };
        let assign: StrAssignFn = unsafe { std::mem::transmute(STRING_ASSIGN) };
        let dtor: StrDtorFn = unsafe { std::mem::transmute(STRING_DTOR) };

        let name = current_name();
        let tag = current_tag();
        if let Ok(cname) = CString::new(name.as_str()) {
            let mut tmp = Bc2String {
                begin: std::ptr::null_mut(),
                end: std::ptr::null_mut(),
                capacity: std::ptr::null_mut(),
            };
            unsafe {
                assign_c(&mut tmp, std::ptr::null_mut(), cname.as_ptr().cast());
                assign(
                    (client as *mut u8).add(CLIENT_PLAYER_NAME_OFF).cast(),
                    std::ptr::null_mut(),
                    &tmp,
                );
                dtor(&mut tmp, std::ptr::null_mut());
            }
        }
        let prof_obj = unsafe { *((ONLINE_SINGLETON + ONLINE_PROFILE_OFF) as *const *mut c_void) };
        if !prof_obj.is_null() {
            if let Ok(ctag) = CString::new(tag.as_str()) {
                let mut tmp = Bc2String {
                    begin: std::ptr::null_mut(),
                    end: std::ptr::null_mut(),
                    capacity: std::ptr::null_mut(),
                };
                let set_clan: SetClanFn = unsafe { std::mem::transmute(SET_PROFILE_CLAN_TAG) };
                unsafe {
                    assign_c(&mut tmp, std::ptr::null_mut(), ctag.as_ptr().cast());
                    set_clan(prof_obj, std::ptr::null_mut(), &tmp);
                    dtor(&mut tmp, std::ptr::null_mut());
                }
            }
        }
        crate::log_line(&format!("profile: Client::sendJoin as '{name}' [{tag}]"));
    }
    if let Some(&orig) = ORIG_SEND_JOIN.get() {
        unsafe { orig(client, edx) };
    }
}

#[cfg(target_arch = "x86")]
unsafe fn send_simple_menu_event(ui: *mut c_void, event_cstr: &'static [u8]) {
    type SendEvtFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
    let send_evt: SendEvtFn = unsafe { std::mem::transmute(SEND_MENU_EVENT) };
    unsafe { send_evt(ui, std::ptr::null_mut(), event_cstr.as_ptr()) };
}

#[cfg(target_arch = "x86")]
pub unsafe fn handle_menu_command(name: &str, args: &str) -> bool {
    let ui = unsafe { *(UI_MANAGER_GLOBAL as *const *mut c_void) };
    if ui.is_null() {
        return false;
    }
    let active_mode = crate::hooks::STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.active))
        .unwrap_or(opfor_vfs::ModeId::Bfbc2);

    // BFBC1's main_root.res / main_nav.res use Xbox 360 presence + memory-card handshakes.
    // Answer them immediately offline so the menu unlocks Multiplayer & Campaign without stalling.
    if active_mode == opfor_vfs::ModeId::Bfbc1 {
        match name {
            "presenceRequestOnline"
            | "readyToLogin"
            | "presenceCreateAccount"
            | "presenceRestartLogin" => {
                unsafe { send_simple_menu_event(ui, b"presenceConnected\0") };
                crate::log_line(&format!(
                    "profile(BFBC1): '{name}' -> sent offline 'presenceConnected'"
                ));
                return true;
            }
            "memCardResponse" => {
                unsafe { send_simple_menu_event(ui, b"memcardPopupHide\0") };
                crate::log_line("profile(BFBC1): 'memCardResponse' -> sent 'memcardPopupHide'");
                return true;
            }
            "querySavedGames" => {
                unsafe { send_simple_menu_event(ui, b"querySavedGamesEnd\0") };
                crate::log_line("profile(BFBC1): 'querySavedGames' -> sent 'querySavedGamesEnd'");
                return true;
            }
            _ => {}
        }
    }

    match name {
        "presenceRequestOnline" => {
            type AcquireFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> *mut c_void;
            type AddStrFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8);
            type AddValFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, u32);
            type QueueEvtFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void);
            type MakeStrFn = unsafe extern "cdecl" fn(*const u8) -> u32;

            let acquire: AcquireFn = unsafe { std::mem::transmute(ACQUIRE_MENU_EVENT) };
            let add_str: AddStrFn = unsafe { std::mem::transmute(EVENT_ADD_STRING) };
            let add_val: AddValFn = unsafe { std::mem::transmute(EVENT_ADD_VALUE) };
            let queue: QueueEvtFn = unsafe { std::mem::transmute(QUEUE_MENU_EVENT) };
            let make_str: MakeStrFn = unsafe { std::mem::transmute(MAKE_STRING) };

            let evt = unsafe { acquire(ui, std::ptr::null_mut()) };
            if !evt.is_null() {
                let n = CString::new(current_name()).unwrap_or_else(|_| CString::new("Player").unwrap());
                let t = CString::new(current_tag()).unwrap_or_else(|_| CString::new("").unwrap());
                unsafe {
                    add_str(evt, std::ptr::null_mut(), b"presenceLoginAccount\0".as_ptr());
                    add_val(evt, std::ptr::null_mut(), make_str(n.as_ptr().cast()));
                    add_val(evt, std::ptr::null_mut(), make_str(t.as_ptr().cast()));
                    queue(ui, std::ptr::null_mut(), evt);
                }
            }
            true
        }
        "presenceLoginAccount" => {
            let mut parts = args.split(',');
            let raw_name = parts.next().unwrap_or("");
            let raw_tag = parts.next().unwrap_or("");
            if let Ok(mut g) = PROFILE.lock() {
                if let Some(p) = g.as_mut() {
                    p.name = clean_name(raw_name);
                    p.clan_tag = clean_tag(raw_tag);
                    save_ini(p);
                    crate::log_line(&format!(
                        "profile: saved name='{}', clantag='{}'",
                        p.name, p.clan_tag
                    ));
                }
            }
            true
        }
        _ => false,
    }
}
