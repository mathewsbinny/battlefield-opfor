//! Dynamic Input Device Monitor (Keyboard/Mouse <-> XInput Gamepad) for `OPFOR.exe`.
//!
//! Dynamically loads `xinput1_4.dll` (falling back to `xinput9_1_0.dll` / `xinput1_3.dll`)
//! with zero external crate dependencies, detects active controller vs. KBM usage every frame,
//! and emits edge-triggered navigation actions for the Global UI State Manager.

#![allow(non_snake_case, dead_code)]

use std::ffi::c_void;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDevice {
    KeyboardMouse,
    GamepadXbox,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PadNavEvents {
    pub any_activity: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub confirm: bool, // A
    pub back: bool,    // B
    pub lb: bool,      // Left Bumper
    pub rb: bool,      // Right Bumper
    pub y: bool,       // Y button
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct XInputGamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct XInputState {
    packet_number: u32,
    gamepad: XInputGamepad,
}

const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const DPAD_LEFT: u16 = 0x0004;
const DPAD_RIGHT: u16 = 0x0008;
const LB: u16 = 0x0100;
const RB: u16 = 0x0200;
const BTN_A: u16 = 0x1000;
const BTN_B: u16 = 0x2000;
const BTN_Y: u16 = 0x8000;
const STICK_DEADZONE: i16 = 14_000;

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryA(name: *const u8) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, proc: *const u8) -> *const c_void;
}

static XINPUT_GET_STATE: OnceLock<Option<XInputGetStateFn>> = OnceLock::new();

fn load_xinput() -> Option<XInputGetStateFn> {
    *XINPUT_GET_STATE.get_or_init(|| {
        for dll in [
            b"xinput1_4.dll\0".as_slice(),
            b"xinput9_1_0.dll\0".as_slice(),
            b"xinput1_3.dll\0".as_slice(),
        ] {
            let h = unsafe { LoadLibraryA(dll.as_ptr()) };
            if !h.is_null() {
                let p = unsafe { GetProcAddress(h, b"XInputGetState\0".as_ptr()) };
                if !p.is_null() {
                    return Some(unsafe { std::mem::transmute::<*const c_void, XInputGetStateFn>(p) });
                }
            }
        }
        None
    })
}

pub struct InputMonitor {
    prev_buttons: u16,
    prev_stick_dir: i8, // -1 left, 0 center, +1 right
}

impl InputMonitor {
    pub fn new() -> Self {
        Self {
            prev_buttons: 0,
            prev_stick_dir: 0,
        }
    }

    pub fn poll_gamepad(&mut self) -> PadNavEvents {
        let mut ev = PadNavEvents::default();
        let Some(get_state) = load_xinput() else {
            return ev;
        };
        let mut st = XInputState::default();
        if unsafe { get_state(0, &mut st) } != 0 {
            self.prev_buttons = 0;
            self.prev_stick_dir = 0;
            return ev;
        }

        let b = st.gamepad.buttons;
        let rising = b & !self.prev_buttons;
        self.prev_buttons = b;

        let stick_dir = if st.gamepad.thumb_lx < -STICK_DEADZONE {
            -1
        } else if st.gamepad.thumb_lx > STICK_DEADZONE {
            1
        } else {
            0
        };
        let stick_left = stick_dir == -1 && self.prev_stick_dir != -1;
        let stick_right = stick_dir == 1 && self.prev_stick_dir != 1;
        self.prev_stick_dir = stick_dir;

        ev.left = (rising & DPAD_LEFT) != 0 || stick_left;
        ev.right = (rising & DPAD_RIGHT) != 0 || stick_right;
        ev.up = (rising & DPAD_UP) != 0;
        ev.down = (rising & DPAD_DOWN) != 0;
        ev.confirm = (rising & BTN_A) != 0;
        ev.back = (rising & BTN_B) != 0;
        ev.lb = (rising & LB) != 0;
        ev.rb = (rising & RB) != 0;
        ev.y = (rising & BTN_Y) != 0;
        ev.any_activity = b != 0
            || stick_dir != 0
            || st.gamepad.thumb_ly.abs() > STICK_DEADZONE
            || st.gamepad.left_trigger > 30
            || st.gamepad.right_trigger > 30;
        ev
    }
}
