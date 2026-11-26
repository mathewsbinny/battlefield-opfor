//! Dynamic In-Game Button Prompt Switching (`Win32` KBM <-> `Xenon` Xbox Controller).
//!
//! Ported from `project-opfor-bfbc2/src/dll/prompts.cpp`:
//! - Patches `extern.platform` push immediate at `0x0095B7D3` (`push 0x0140B670`) to switch
//!   dynamically between `"Win32"` and `"Xenon"` on every UI frame based on `ui_state::current_input_device()`.

#![allow(dead_code, non_snake_case)]

use std::sync::atomic::{AtomicBool, Ordering};

use crate::ui_state::InputDevice;

const PLATFORM_PUSH: usize = 0x0095_B7D3;
const PLATFORM_PUSH_BYTES: [u8; 5] = [0x68, 0x70, 0xB6, 0x40, 0x01];

static WIN32_STR: &[u8] = b"Win32\0";
static XENON_STR: &[u8] = b"Xenon\0";

static PATCHED: AtomicBool = AtomicBool::new(false);
static LAST_WAS_PAD: AtomicBool = AtomicBool::new(false);

pub unsafe fn install() -> usize {
    #[cfg(target_arch = "x86")]
    {
        if unsafe { crate::hooks::bytes_match(PLATFORM_PUSH, &PLATFORM_PUSH_BYTES) } {
            let ptr = (WIN32_STR.as_ptr() as u32).to_le_bytes();
            if unsafe { crate::hooks::write_code(PLATFORM_PUSH + 1, &ptr) } {
                PATCHED.store(true, Ordering::Release);
                crate::log_line("prompts: dynamic extern.platform ('Win32' <-> 'Xenon') installed");
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
        let is_pad = crate::ui_state::current_input_device() == InputDevice::GamepadXbox;
        if LAST_WAS_PAD.swap(is_pad, Ordering::AcqRel) != is_pad {
            let ptr = if is_pad {
                XENON_STR.as_ptr() as u32
            } else {
                WIN32_STR.as_ptr() as u32
            };
            let _ = unsafe { crate::hooks::write_code(PLATFORM_PUSH + 1, &ptr.to_le_bytes()) };
            crate::log_line(&format!(
                "prompts: switched in-game platform prompts -> {}",
                if is_pad { "Xenon (Gamepad)" } else { "Win32 (KBM)" }
            ));
        }
    }
}
