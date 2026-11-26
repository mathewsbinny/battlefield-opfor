//! Console Onslaught co-op stopwatch HUD & personal-best times (`user/onslaught_times.ini`).
//!
//! Ported from `project-opfor-bfbc2/src/dll/onslaught_hud.cpp`:
//! - Detects Onslaught matches via `UiManager -> 0x0092AB10 (+0x6C == "onslaught")`.
//! - Sends `coopRoundTime(HH:MM:SS:t, isRunning)` and `coopOldBestRoundTime(best)` on the UI tick.
//! - Stops the clock and persists personal best times when local stats commit at round end.

#![allow(dead_code, non_snake_case)]

use std::collections::BTreeMap;
use std::ffi::{CString, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const UI_MANAGER_GLOBAL: usize = 0x019A_4CA8;
const MODE_OWNER_FN: usize = 0x0092_AB10;
const ACQUIRE_MENU_EVENT: usize = 0x0092_0360;
const EVENT_ADD_STRING: usize = 0x0058_8910;
const EVENT_ADD_VALUE: usize = 0x0058_8940;
const QUEUE_MENU_EVENT: usize = 0x0092_99B0;
const MAKE_BOOL: usize = 0x00F7_C600;

#[repr(C)]
struct EaString {
    begin: *const u8,
    end: *const u8,
}

#[derive(PartialEq, Eq)]
enum ClockState {
    Idle,
    Running,
    Finished,
}

struct OnslaughtState {
    file: PathBuf,
    level: String,
    best_times: BTreeMap<String, f64>,
    state: ClockState,
    started_at: Option<Instant>,
    last_send: Option<Instant>,
}

static STATE: Mutex<Option<OnslaughtState>> = Mutex::new(None);

pub fn init(opfor_root: &Path) {
    let file = opfor_root.join("user").join("onslaught_times.ini");
    let mut best_times = BTreeMap::new();
    if let Ok(txt) = fs::read_to_string(&file) {
        for raw in txt.lines() {
            if let Some((k, v)) = raw.trim().split_once('=') {
                if let Ok(secs) = v.trim().parse::<f64>() {
                    best_times.insert(k.trim().to_string(), secs);
                }
            }
        }
    }
    if let Ok(mut g) = STATE.lock() {
        *g = Some(OnslaughtState {
            file,
            level: String::new(),
            best_times,
            state: ClockState::Idle,
            started_at: None,
            last_send: None,
        });
    }
}

pub fn on_level_started(level: &str) {
    if let Ok(mut g) = STATE.lock() {
        if let Some(st) = g.as_mut() {
            st.level = level.to_string();
            st.state = ClockState::Idle;
            st.started_at = None;
            st.last_send = None;
        }
    }
}

fn format_time(secs: f64) -> String {
    let total_tenths = (secs.max(0.0) * 10.0) as u64;
    let tenths = total_tenths % 10;
    let total_secs = total_tenths / 10;
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = total_secs / 3600;
    format!("{h:02}:{m:02}:{s:02}:{tenths}")
}

#[cfg(target_arch = "x86")]
unsafe fn in_onslaught(ui: *mut c_void) -> bool {
    if ui.is_null() {
        return false;
    }
    type ModeOwnerFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> *const u8;
    let f: ModeOwnerFn = unsafe { std::mem::transmute(MODE_OWNER_FN) };
    let owner = unsafe { f(ui, std::ptr::null_mut()) };
    if owner.is_null() {
        return false;
    }
    let s = unsafe { &*(owner.add(0x6C) as *const EaString) };
    if s.begin.is_null() || s.end <= s.begin {
        return false;
    }
    let len = s.end as usize - s.begin as usize;
    len == 9 && unsafe { std::slice::from_raw_parts(s.begin, 9) == b"onslaught" }
}

#[cfg(target_arch = "x86")]
unsafe fn send_hud_event(ui: *mut c_void, name: &str, text: &str, running: Option<bool>) {
    if ui.is_null() {
        return;
    }
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

    let Ok(cn) = CString::new(name) else { return };
    let Ok(ct) = CString::new(text) else { return };
    let ev = unsafe { acquire(ui, std::ptr::null_mut()) };
    if ev.is_null() {
        return;
    }
    unsafe {
        add_str(ev, std::ptr::null_mut(), cn.as_ptr().cast());
        add_str(ev, std::ptr::null_mut(), ct.as_ptr().cast());
        if let Some(r) = running {
            add_val(ev, std::ptr::null_mut(), make_bool(i32::from(r)));
        }
        queue(ui, std::ptr::null_mut(), ev);
    }
}

pub fn on_round_ended() {
    let mut finish_msg: Option<String> = None;
    if let Ok(mut g) = STATE.lock() {
        if let Some(st) = g.as_mut() {
            if st.state == ClockState::Running {
                if let Some(start) = st.started_at {
                    let elapsed = start.elapsed().as_secs_f64();
                    st.state = ClockState::Finished;
                    let fmt = format_time(elapsed);
                    finish_msg = Some(fmt.clone());
                    if !st.level.is_empty() {
                        let prev = st.best_times.get(&st.level).copied();
                        if prev.is_none_or(|b| elapsed < b) {
                            st.best_times.insert(st.level.clone(), elapsed);
                            let mut out = String::from("[best]\n");
                            for (k, v) in &st.best_times {
                                out.push_str(&format!("{k}={v:.2}\n"));
                            }
                            if let Some(p) = st.file.parent() {
                                let _ = fs::create_dir_all(p);
                            }
                            let _ = fs::write(&st.file, out);
                        }
                    }
                    crate::log_line(&format!("onslaught: round finished in {fmt}"));
                }
            }
        }
    }
    #[cfg(target_arch = "x86")]
    if let Some(fmt) = finish_msg {
        let ui = unsafe { *(UI_MANAGER_GLOBAL as *const *mut c_void) };
        unsafe { send_hud_event(ui, "coopRoundTime", &fmt, Some(false)) };
    }
}

#[cfg(target_arch = "x86")]
pub unsafe fn on_ui_frame(ui: *mut c_void) {
    if !unsafe { in_onslaught(ui) } {
        return;
    }
    let mut send_best: Option<String> = None;
    let mut send_time: Option<String> = None;
    if let Ok(mut g) = STATE.lock() {
        if let Some(st) = g.as_mut() {
            match st.state {
                ClockState::Idle => {
                    st.state = ClockState::Running;
                    st.started_at = Some(Instant::now());
                    st.last_send = Some(Instant::now());
                    if let Some(&best) = st.best_times.get(&st.level) {
                        send_best = Some(format_time(best));
                    }
                    send_time = Some(format_time(0.0));
                }
                ClockState::Running => {
                    if st.last_send.is_none_or(|ls| ls.elapsed() >= Duration::from_secs(1)) {
                        st.last_send = Some(Instant::now());
                        if let Some(start) = st.started_at {
                            send_time = Some(format_time(start.elapsed().as_secs_f64()));
                        }
                    }
                }
                ClockState::Finished => {}
            }
        }
    }
    if let Some(b) = send_best {
        unsafe { send_hud_event(ui, "coopOldBestRoundTime", &b, None) };
    }
    if let Some(t) = send_time {
        unsafe { send_hud_event(ui, "coopRoundTime", &t, Some(true)) };
    }
}
