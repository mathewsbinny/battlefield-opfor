//! Dynamic In-Game Button Prompt Switching (`Win32` KBM <-> `Xenon` Xbox 360 <-> `PS3` PlayStation).
//!
//! Patches `extern.platform` push immediate at `0x0095B7D3` (`push 0x0140B670`) to switch
//! dynamically between `"Win32"`, `"Xenon"`, and `"PS3"` on every UI frame based on
//! `ui_state::current_input_device()`.

#![allow(dead_code, non_snake_case)]

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::ui_state::InputDevice;

const PLATFORM_PUSH: usize = 0x0095_B7D3;
const PLATFORM_PUSH_BYTES: [u8; 5] = [0x68, 0x70, 0xB6, 0x40, 0x01];

static WIN32_STR: &[u8] = b"Win32\0";
static XENON_STR: &[u8] = b"Xenon\0";
static PS3_STR: &[u8] = b"PS3\0";

static PATCHED: AtomicBool = AtomicBool::new(false);
static LAST_DEVICE: AtomicU8 = AtomicU8::new(0); // 0=KBM, 1=Xbox, 2=PS3

fn device_code(dev: InputDevice) -> u8 {
    match dev {
        InputDevice::KeyboardMouse => 0,
        InputDevice::GamepadXbox => 1,
        InputDevice::GamepadPlayStation => 2,
    }
}

pub unsafe fn install() -> usize {
    #[cfg(target_arch = "x86")]
    {
        if unsafe { crate::hooks::bytes_match(PLATFORM_PUSH, &PLATFORM_PUSH_BYTES) } {
            let ptr = (WIN32_STR.as_ptr() as u32).to_le_bytes();
            if unsafe { crate::hooks::write_code(PLATFORM_PUSH + 1, &ptr) } {
                PATCHED.store(true, Ordering::Release);
                LAST_DEVICE.store(0, Ordering::Release);
                crate::log_line(
                    "prompts: dynamic extern.platform ('Win32' <-> 'Xenon' <-> 'PS3') installed",
                );
                return 1;
            }
        }
    }
    0
}

/// Sync Frostbite's `extern.platform` string with the active input device on every UI frame.
pub unsafe fn sync_frame() {
    #[cfg(target_arch = "x86")]
    if PATCHED.load(Ordering::Acquire) {
        let dev = crate::ui_state::current_input_device();
        let code = device_code(dev);
        if LAST_DEVICE.swap(code, Ordering::AcqRel) != code {
            let (ptr, label) = match dev {
                InputDevice::KeyboardMouse => (WIN32_STR.as_ptr() as u32, "Win32 (KBM)"),
                InputDevice::GamepadXbox => (XENON_STR.as_ptr() as u32, "Xenon (Xbox 360)"),
                InputDevice::GamepadPlayStation => (PS3_STR.as_ptr() as u32, "PS3 (PlayStation)"),
            };
            let _ = unsafe { crate::hooks::write_code(PLATFORM_PUSH + 1, &ptr.to_le_bytes()) };
            crate::padaimer::set_pad_active(code != 0, 1.0, 1.0);
            crate::log_line(&format!("prompts: switched in-game platform prompts -> {label}"));
        }
    }
}
