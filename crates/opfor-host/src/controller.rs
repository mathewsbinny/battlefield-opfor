//! Three-way controller & prompt device detection + native Sony DualShock 4 / DualSense
//! HID-to-XInput translation (`KeyboardMouse` / `GamepadXbox` / `GamepadPlayStation`).
//!
//! - Detects connected Sony PlayStation controllers (`VID_054C`: DualShock 3, DualShock 4,
//!   DualSense, DualSense Edge) via pure Win32 `GetRawInputDeviceList` + `GetRawInputDeviceInfoW`.
//! - Parses standard USB HID reports from DualShock 4 (`0x05C4`/`0x09CC`) and DualSense
//!   (`0x0CE6`/`0x0DF2`) via `WM_INPUT` into a live `XInputState` so PlayStation controllers
//!   work plug-and-play in Frostbite even without DS4Windows or Steam Input.
//! - Honors optional `controller_prompts=auto|xbox|ps3` in `config/opfor.ini` (default `auto`).

#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::ui_state::InputDevice;

pub const SONY_VENDOR_ID: u32 = 0x054C;
pub const PID_DS3: u32 = 0x0268;
pub const PID_DS4_V1: u32 = 0x05C4;
pub const PID_DS4_V2: u32 = 0x09CC;
pub const PID_DUALSENSE: u32 = 0x0CE6;
pub const PID_DUALSENSE_EDGE: u32 = 0x0DF2;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct XInputGamepad {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct XInputState {
    pub packet_number: u32,
    pub gamepad: XInputGamepad,
}

/// User preference from `config/opfor.ini` (`controller_prompts=auto|xbox|ps3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PromptPref {
    Auto = 0,
    Xbox = 1,
    PlayStation = 2,
}

static PROMPT_PREF: AtomicU8 = AtomicU8::new(PromptPref::Auto as u8);
static SONY_HID_DETECTED: AtomicBool = AtomicBool::new(false);
static SONY_STATE: Mutex<Option<XInputState>> = Mutex::new(None);
static SONY_PKT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// Parse `controller_prompts=` from `opfor.ini` text.
#[must_use]
pub fn parse_prompt_pref(ini_text: &str) -> PromptPref {
    for raw in ini_text.lines() {
        let line = raw.trim();
        if let Some(val) = line.strip_prefix("controller_prompts=") {
            match val.trim().to_ascii_lowercase().as_str() {
                "ps3" | "ps4" | "ps5" | "playstation" | "sony" | "dualshock" | "dualsense" => {
                    return PromptPref::PlayStation;
                }
                "xbox" | "xbox360" | "xenon" | "xinput" => {
                    return PromptPref::Xbox;
                }
                _ => return PromptPref::Auto,
            }
        }
    }
    PromptPref::Auto
}

/// Resolve which gamepad prompt family (`GamepadXbox` vs `GamepadPlayStation`) should be active.
#[must_use]
pub fn resolve_gamepad_device(pref: PromptPref, sony_hid_present: bool) -> InputDevice {
    match pref {
        PromptPref::PlayStation => InputDevice::GamepadPlayStation,
        PromptPref::Xbox => InputDevice::GamepadXbox,
        PromptPref::Auto => {
            if sony_hid_present {
                InputDevice::GamepadPlayStation
            } else {
                InputDevice::GamepadXbox
            }
        }
    }
}

/// Current active gamepad device family for XInput/HID controller activity.
#[must_use]
pub fn active_gamepad_device() -> InputDevice {
    let pref = match PROMPT_PREF.load(Ordering::Relaxed) {
        1 => PromptPref::Xbox,
        2 => PromptPref::PlayStation,
        _ => PromptPref::Auto,
    };
    resolve_gamepad_device(pref, SONY_HID_DETECTED.load(Ordering::Relaxed))
}

fn u8_axis_to_i16(v: u8, invert: bool) -> i16 {
    let centered = (v as i32) - 128;
    let scaled = (centered * 257).clamp(-32768, 32767);
    if invert {
        (-scaled).clamp(-32768, 32767) as i16
    } else {
        scaled as i16
    }
}

fn hat_to_xinput_dpad(hat: u8) -> u16 {
    match hat & 0x0F {
        0 => 0x0001,          // Up
        1 => 0x0001 | 0x0008, // Up-Right
        2 => 0x0008,          // Right
        3 => 0x0002 | 0x0008, // Down-Right
        4 => 0x0002,          // Down
        5 => 0x0002 | 0x0004, // Down-Left
        6 => 0x0004,          // Left
        7 => 0x0001 | 0x0004, // Up-Left
        _ => 0,
    }
}

/// Translate a Sony DualShock 4 (`0x05C4`/`0x09CC`) or DualSense (`0x0CE6`/`0x0DF2`) USB HID
/// input report (`report_id == 0x01`) into a standard `XInputGamepad`.
#[must_use]
pub fn parse_sony_hid_report(pid: u32, report: &[u8]) -> Option<XInputGamepad> {
    if report.first().copied() != Some(0x01) {
        return None;
    }
    let is_dualsense = matches!(pid, PID_DUALSENSE | PID_DUALSENSE_EDGE);
    let (b1, b2, lt, rt) = if is_dualsense {
        if report.len() < 10 {
            return None;
        }
        (report[8], report[9], report[5], report[6])
    } else {
        // DualShock 4 USB report 0x01
        if report.len() < 10 {
            return None;
        }
        (report[5], report[6], report[8], report[9])
    };

    let mut buttons: u16 = hat_to_xinput_dpad(b1);
    if (b1 & 0x10) != 0 { buttons |= 0x4000; } // Square -> X
    if (b1 & 0x20) != 0 { buttons |= 0x1000; } // Cross  -> A
    if (b1 & 0x40) != 0 { buttons |= 0x2000; } // Circle -> B
    if (b1 & 0x80) != 0 { buttons |= 0x8000; } // Triangle -> Y
    if (b2 & 0x01) != 0 { buttons |= 0x0100; } // L1 -> LB
    if (b2 & 0x02) != 0 { buttons |= 0x0200; } // R1 -> RB
    if (b2 & 0x10) != 0 { buttons |= 0x0020; } // Share/Create -> Back
    if (b2 & 0x20) != 0 { buttons |= 0x0010; } // Options -> Start
    if (b2 & 0x40) != 0 { buttons |= 0x0040; } // L3 -> Left Thumb
    if (b2 & 0x80) != 0 { buttons |= 0x0080; } // R3 -> Right Thumb

    Some(XInputGamepad {
        buttons,
        left_trigger: lt,
        right_trigger: rt,
        thumb_lx: u8_axis_to_i16(report[1], false),
        thumb_ly: u8_axis_to_i16(report[2], true),
        thumb_rx: u8_axis_to_i16(report[3], false),
        thumb_ry: u8_axis_to_i16(report[4], true),
    })
}

/// Copy the latest translated Sony HID `XInputState` if a Sony controller has reported input.
pub fn poll_sony_xinput(out: &mut XInputState) -> bool {
    if let Ok(g) = SONY_STATE.lock() {
        if let Some(st) = *g {
            *out = st;
            return true;
        }
    }
    false
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawInputDeviceList {
    h_device: *mut c_void,
    dw_type: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawInputDevice {
    us_usage_page: u16,
    us_usage: u16,
    dw_flags: u32,
    hwnd_target: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawInputHeader {
    dw_type: u32,
    dw_size: u32,
    h_device: *mut c_void,
    w_param: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawHidHeader {
    dw_size_hid: u32,
    dw_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RidDeviceInfoHid {
    dw_vendor_id: u32,
    dw_product_id: u32,
    dw_version_number: u32,
    us_usage_page: u16,
    us_usage: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RidDeviceInfo {
    cb_size: u32,
    dw_type: u32,
    hid: RidDeviceInfoHid,
    _pad: [u8; 16],
}

const RIM_TYPEHID: u32 = 2;
const RIDI_DEVICEINFO: u32 = 0x2000_000B;
const RID_INPUT: u32 = 0x1000_0003;
const RIDEV_INPUTSINK: u32 = 0x0000_0100;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetRawInputDeviceList(
        pRawInputDeviceList: *mut RawInputDeviceList,
        puiNumDevices: *mut u32,
        cbSize: u32,
    ) -> u32;
    fn GetRawInputDeviceInfoW(
        hDevice: *mut c_void,
        uiCommand: u32,
        pData: *mut c_void,
        pcbSize: *mut u32,
    ) -> u32;
    fn RegisterRawInputDevices(
        pRawInputDevices: *const RawInputDevice,
        uiNumDevices: u32,
        cbSize: u32,
    ) -> i32;
    fn GetRawInputData(
        hRawInput: *mut c_void,
        uiCommand: u32,
        pData: *mut c_void,
        pcbSize: *mut u32,
        cbSizeHeader: u32,
    ) -> u32;
}

/// Register the game window to receive RawInput HID gamepad/joystick reports (`WM_INPUT`).
pub fn register_raw_input(hwnd: *mut c_void) {
    let devs = [
        RawInputDevice {
            us_usage_page: 0x01,
            us_usage: 0x05, // Gamepad
            dw_flags: RIDEV_INPUTSINK,
            hwnd_target: hwnd,
        },
        RawInputDevice {
            us_usage_page: 0x01,
            us_usage: 0x04, // Joystick
            dw_flags: RIDEV_INPUTSINK,
            hwnd_target: hwnd,
        },
    ];
    let ok = unsafe {
        RegisterRawInputDevices(
            devs.as_ptr(),
            devs.len() as u32,
            std::mem::size_of::<RawInputDevice>() as u32,
        )
    };
    if ok != 0 {
        crate::log_line("controller: RawInput HID gamepad/joystick listener registered");
    }
}

/// Process a `WM_INPUT` message; if it's a Sony `VID_054C` controller report, translate to `XInputState`.
pub fn handle_wm_input(lparam: isize) {
    let hraw = lparam as *mut c_void;
    let hdr_sz = std::mem::size_of::<RawInputHeader>() as u32;
    let mut size: u32 = 0;
    if unsafe { GetRawInputData(hraw, RID_INPUT, std::ptr::null_mut(), &mut size, hdr_sz) } != 0
        || size < hdr_sz + 8
        || size > 512
    {
        return;
    }
    let mut buf = vec![0u8; size as usize];
    if unsafe {
        GetRawInputData(
            hraw,
            RID_INPUT,
            buf.as_mut_ptr().cast::<c_void>(),
            &mut size,
            hdr_sz,
        )
    } == u32::MAX
    {
        return;
    }
    let hdr = unsafe { *(buf.as_ptr().cast::<RawInputHeader>()) };
    if hdr.dw_type != RIM_TYPEHID {
        return;
    }
    let mut info = RidDeviceInfo {
        cb_size: std::mem::size_of::<RidDeviceInfo>() as u32,
        dw_type: 0,
        hid: RidDeviceInfoHid {
            dw_vendor_id: 0,
            dw_product_id: 0,
            dw_version_number: 0,
            us_usage_page: 0,
            us_usage: 0,
        },
        _pad: [0u8; 16],
    };
    let mut info_sz = info.cb_size;
    if unsafe {
        GetRawInputDeviceInfoW(
            hdr.h_device,
            RIDI_DEVICEINFO,
            (&mut info as *mut RidDeviceInfo).cast::<c_void>(),
            &mut info_sz,
        )
    } == u32::MAX
        || info.hid.dw_vendor_id != SONY_VENDOR_ID
    {
        return;
    }
    SONY_HID_DETECTED.store(true, Ordering::Release);
    let hid_hdr_off = hdr_sz as usize;
    let hid_hdr = unsafe { *(buf.as_ptr().add(hid_hdr_off).cast::<RawHidHeader>()) };
    let data_off = hid_hdr_off + std::mem::size_of::<RawHidHeader>();
    let rep_len = hid_hdr.dw_size_hid as usize;
    if data_off + rep_len > buf.len() {
        return;
    }
    let report = &buf[data_off..data_off + rep_len];
    if let Some(gp) = parse_sony_hid_report(info.hid.dw_product_id, report) {
        let active = gp.buttons != 0
            || gp.left_trigger > 30
            || gp.right_trigger > 30
            || gp.thumb_lx.unsigned_abs() > 7849
            || gp.thumb_ly.unsigned_abs() > 7849
            || gp.thumb_rx.unsigned_abs() > 8689
            || gp.thumb_ry.unsigned_abs() > 8689;
        let pkt = SONY_PKT.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut g) = SONY_STATE.lock() {
            *g = Some(XInputState {
                packet_number: pkt,
                gamepad: gp,
            });
        }
        if active {
            crate::ui_state::note_playstation_input();
        }
    }
}

/// Scan connected HID devices for a Sony PlayStation controller (`VID_054C`).
pub fn refresh_sony_hid_presence() -> bool {
    let mut count: u32 = 0;
    let entry_sz = std::mem::size_of::<RawInputDeviceList>() as u32;
    let rc = unsafe { GetRawInputDeviceList(std::ptr::null_mut(), &mut count, entry_sz) };
    if rc == u32::MAX || count == 0 {
        SONY_HID_DETECTED.store(false, Ordering::Release);
        return false;
    }
    let mut list = vec![RawInputDeviceList::default(); count as usize];
    let got = unsafe { GetRawInputDeviceList(list.as_mut_ptr(), &mut count, entry_sz) };
    if got == u32::MAX {
        return SONY_HID_DETECTED.load(Ordering::Relaxed);
    }
    let mut found_sony = false;
    for dev in &list[..got as usize] {
        if dev.dw_type != RIM_TYPEHID {
            continue;
        }
        let mut info = RidDeviceInfo {
            cb_size: std::mem::size_of::<RidDeviceInfo>() as u32,
            dw_type: 0,
            hid: RidDeviceInfoHid {
                dw_vendor_id: 0,
                dw_product_id: 0,
                dw_version_number: 0,
                us_usage_page: 0,
                us_usage: 0,
            },
            _pad: [0u8; 16],
        };
        let mut sz = info.cb_size;
        let r = unsafe {
            GetRawInputDeviceInfoW(
                dev.h_device,
                RIDI_DEVICEINFO,
                (&mut info as *mut RidDeviceInfo).cast::<c_void>(),
                &mut sz,
            )
        };
        if r != u32::MAX
            && info.dw_type == RIM_TYPEHID
            && info.hid.dw_vendor_id == SONY_VENDOR_ID
            && info.hid.us_usage_page == 0x01
            && matches!(info.hid.us_usage, 0x04 | 0x05)
        {
            found_sony = true;
            break;
        }
    }
    SONY_HID_DETECTED.store(found_sony, Ordering::Release);
    found_sony
}

/// Initialize controller prompt preferences from `config/opfor.ini` and scan Sony HID devices.
pub fn init(opfor_root: &Path) {
    let ini_path = opfor_root.join("config").join("opfor.ini");
    let pref = std::fs::read_to_string(&ini_path)
        .map(|t| parse_prompt_pref(&t))
        .unwrap_or(PromptPref::Auto);
    PROMPT_PREF.store(pref as u8, Ordering::Release);
    let sony = refresh_sony_hid_presence();
    crate::log_line(&format!(
        "controller: initialized (pref={pref:?}, sony_vid_054c_hid={sony}, active_pad={:?})",
        active_gamepad_device()
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prompt_pref_values() {
        assert_eq!(parse_prompt_pref("controller_prompts=ps3\n"), PromptPref::PlayStation);
        assert_eq!(parse_prompt_pref("controller_prompts=DualSense\n"), PromptPref::PlayStation);
        assert_eq!(parse_prompt_pref("controller_prompts=xbox360\n"), PromptPref::Xbox);
        assert_eq!(parse_prompt_pref("controller_prompts=auto\n"), PromptPref::Auto);
        assert_eq!(parse_prompt_pref("other=1\n"), PromptPref::Auto);
    }

    #[test]
    fn resolves_three_way_gamepad_device() {
        assert_eq!(
            resolve_gamepad_device(PromptPref::Auto, true),
            InputDevice::GamepadPlayStation
        );
        assert_eq!(
            resolve_gamepad_device(PromptPref::Auto, false),
            InputDevice::GamepadXbox
        );
        assert_eq!(
            resolve_gamepad_device(PromptPref::PlayStation, false),
            InputDevice::GamepadPlayStation
        );
        assert_eq!(
            resolve_gamepad_device(PromptPref::Xbox, true),
            InputDevice::GamepadXbox
        );
    }

    #[test]
    fn parses_ds4_and_dualsense_hid_reports_to_xinput() {
        // DS4 USB report 0x01: Cross (0x20) + D-Pad Up (0x00), L1 (0x01), L2=200, R2=255
        let ds4 = [0x01, 128, 0, 255, 128, 0x20, 0x01, 0, 200, 255];
        let gp = parse_sony_hid_report(PID_DS4_V2, &ds4).unwrap();
        assert_eq!(gp.buttons, 0x1000 | 0x0001 | 0x0100); // A | Up | LB
        assert_eq!(gp.left_trigger, 200);
        assert_eq!(gp.right_trigger, 255);
        assert!(gp.thumb_ly > 30_000); // Y inverted up -> positive
        assert!(gp.thumb_rx > 30_000);

        // DualSense USB report 0x01: Circle (0x40) + hat neutral (0x08), Options (0x20), L2=10, R2=180
        let ds5 = [0x01, 128, 128, 128, 128, 10, 180, 0, 0x48, 0x20];
        let gp5 = parse_sony_hid_report(PID_DUALSENSE, &ds5).unwrap();
        assert_eq!(gp5.buttons, 0x2000 | 0x0010); // B | Start
        assert_eq!(gp5.left_trigger, 10);
        assert_eq!(gp5.right_trigger, 180);
    }
}
