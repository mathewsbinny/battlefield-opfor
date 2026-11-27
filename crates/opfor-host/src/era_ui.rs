//! Era-specific ingame APT `FSCommand` compatibility shim for `opfor-host` (BFBC1 & BF1943).
//!
//! `frostbite.bin` (BFBC2 build 795745) still implements most BFBC1 & BF1943 `FSCommand`s
//! natively, but removed a small set of legacy commands used by their original APT HUD/menu
//! movies:
//!
//! - **Shared (BFBC1 + BF1943):**
//!   - `setArray`, `setArrayCount`, `setArrayIndex`, `refreshArray` -> tracked in `EraUiState`
//!     and cleanly acknowledged.
//!   - `clearDeathMessages`, `clearLocalDeathMessages` -> bridged to BFBC2's `clearServerMessages`.
//! - **BFBC1-only:**
//!   - `setActiveAwardInRound` -> tracked and cleanly acknowledged for MP `igm_mp.res`.
//! - **BF1943-only:**
//!   - `popupRequest`, `requestFLEntry`, `setFLListPageNr`, `setLBListPageNr`, `startMusic`,
//!     `startTutorial` -> cleanly handled/acknowledged so BF1943 `igm_mp_cq.res` / `common_screens.res`
//!     never stall on unhandled `FSCommand`s.
//!
//! Hooks `IngameFSCommandDispatcher` (`0x00753540`, vtable `0x0142B228` slot +0x18,
//! `thiscall(hud_self, name, args)`, `ret 8`). Active when `initial_mode` is `Bfbc1` or `Bf1943`.

#![allow(dead_code, non_snake_case)]

use std::collections::HashSet;
#[cfg(target_arch = "x86")]
use std::ffi::c_void;
use std::sync::Mutex;
#[cfg(target_arch = "x86")]
use std::sync::OnceLock;

use opfor_vfs::ModeId;

/// Verified `frostbite.bin` (BFBC2 build 795745) ingame HUD/menu `FSCommand` dispatcher.
/// Vtable `0x0142B228`, slot `+0x18` (`thiscall(this, const char* name, const char* args)`, `ret 8`).
pub const INGAME_FSCOMMAND_DISPATCHER: usize = 0x0075_3540;
/// First 9 bytes: `sub esp, 0x0C; push ebx; push ebp; mov ebp, [esp+0x18]` (position-independent).
pub const INGAME_FSCOMMAND_PROLOGUE: [u8; 9] =
    [0x83, 0xEC, 0x0C, 0x53, 0x55, 0x8B, 0x6C, 0x24, 0x18];

/// Classification of an era-specific ingame `FSCommand`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShimAction {
    /// Pass straight through to Frostbite's native `IngameFSCommandDispatcher`.
    PassThrough,
    /// Intercept and update `ArrayBindingState` (`setArray*` / `refreshArray`).
    HandleArray,
    /// Forward to BFBC2's native `clearServerMessages` handler.
    BridgeClearMessages,
    /// Track active MP in-round award ID (BFBC1) and acknowledge cleanly.
    HandleAward,
    /// Intercept BF1943-only menu/list/music/tutorial commands and acknowledge cleanly.
    HandleBf1943Legacy,
}

/// Classify an ingame `FSCommand` for the given active era.
#[must_use]
pub fn classify_fscommand(mode: ModeId, name: &str) -> ShimAction {
    match mode {
        ModeId::Bfbc1 | ModeId::Bf1943 => match name {
            "setArray" | "setArrayCount" | "setArrayIndex" | "refreshArray" => {
                ShimAction::HandleArray
            }
            "clearDeathMessages" | "clearLocalDeathMessages" => ShimAction::BridgeClearMessages,
            "setActiveAwardInRound" if mode == ModeId::Bfbc1 => ShimAction::HandleAward,
            "popupRequest"
            | "requestFLEntry"
            | "setFLListPageNr"
            | "setLBListPageNr"
            | "startMusic"
            | "startTutorial"
                if mode == ModeId::Bf1943 =>
            {
                ShimAction::HandleBf1943Legacy
            }
            _ => ShimAction::PassThrough,
        },
        _ => ShimAction::PassThrough,
    }
}

/// Runtime state tracked for removed BFBC1 & BF1943 `UIArray` / menu `FSCommand`s.
#[derive(Debug)]
pub struct EraUiState {
    pub mode: ModeId,
    pub active_array: String,
    pub array_count: i32,
    pub array_index: i32,
    pub active_award_id: String,
    pub fl_page: i32,
    pub lb_page: i32,
    pub logged_cmds: HashSet<String>,
}

impl EraUiState {
    #[must_use]
    pub fn new(mode: ModeId) -> Self {
        Self {
            mode,
            active_array: String::new(),
            array_count: 0,
            array_index: 0,
            active_award_id: String::new(),
            fl_page: 0,
            lb_page: 0,
            logged_cmds: HashSet::new(),
        }
    }

    pub fn apply_array_cmd(&mut self, name: &str, args: &str) -> bool {
        match name {
            "setArray" => self.active_array = args.to_string(),
            "setArrayCount" => self.array_count = args.trim().parse::<i32>().unwrap_or(0),
            "setArrayIndex" => self.array_index = args.trim().parse::<i32>().unwrap_or(0),
            "refreshArray" => {}
            _ => return false,
        }
        let key = format!("{name}:{}", self.active_array);
        self.logged_cmds.insert(key)
    }

    pub fn apply_award_cmd(&mut self, args: &str) -> bool {
        self.active_award_id = args.to_string();
        self.logged_cmds.insert("setActiveAwardInRound".to_string())
    }

    pub fn apply_bf1943_legacy(&mut self, name: &str, args: &str) -> bool {
        match name {
            "setFLListPageNr" => self.fl_page = args.trim().parse::<i32>().unwrap_or(0),
            "setLBListPageNr" => self.lb_page = args.trim().parse::<i32>().unwrap_or(0),
            _ => {}
        }
        self.logged_cmds.insert(name.to_string())
    }
}

static UI_STATE: Mutex<Option<EraUiState>> = Mutex::new(None);

#[cfg(target_arch = "x86")]
type IngameFsCmdFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *const u8, *const u8);

#[cfg(target_arch = "x86")]
static ORIG_INGAME_FSCMD: OnceLock<IngameFsCmdFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_ingame_fscommand(
    hud_self: *mut c_void,
    edx: *mut c_void,
    name_ptr: *const u8,
    args_ptr: *const u8,
) {
    let name = unsafe { crate::win32::from_ansi_ptr(name_ptr) }.unwrap_or_default();
    let args = unsafe { crate::win32::from_ansi_ptr(args_ptr) }.unwrap_or_default();
    let mode = UI_STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.mode))
        .unwrap_or(ModeId::Bfbc2);

    match classify_fscommand(mode, &name) {
        ShimAction::PassThrough => {
            if let Some(&orig) = ORIG_INGAME_FSCMD.get() {
                unsafe { orig(hud_self, edx, name_ptr, args_ptr) };
            }
        }
        ShimAction::HandleArray => {
            let should_log = UI_STATE
                .lock()
                .ok()
                .and_then(|mut g| g.as_mut().map(|s| s.apply_array_cmd(&name, &args)))
                .unwrap_or(false);
            if should_log {
                crate::log_line(&format!("era_ui({mode:?}): shimmed FSCommand:{name}('{args}')"));
            }
        }
        ShimAction::BridgeClearMessages => {
            let should_log = UI_STATE
                .lock()
                .ok()
                .and_then(|mut g| g.as_mut().map(|s| s.logged_cmds.insert(name.clone())))
                .unwrap_or(false);
            if should_log {
                crate::log_line(&format!(
                    "era_ui({mode:?}): bridging FSCommand:{name} -> clearServerMessages"
                ));
            }
            if let Some(&orig) = ORIG_INGAME_FSCMD.get() {
                unsafe {
                    orig(
                        hud_self,
                        edx,
                        b"clearServerMessages\0".as_ptr(),
                        args_ptr,
                    );
                }
            }
        }
        ShimAction::HandleAward => {
            let should_log = UI_STATE
                .lock()
                .ok()
                .and_then(|mut g| g.as_mut().map(|s| s.apply_award_cmd(&args)))
                .unwrap_or(false);
            if should_log {
                crate::log_line(&format!(
                    "era_ui({mode:?}): shimmed FSCommand:setActiveAwardInRound('{args}')"
                ));
            }
        }
        ShimAction::HandleBf1943Legacy => {
            let should_log = UI_STATE
                .lock()
                .ok()
                .and_then(|mut g| g.as_mut().map(|s| s.apply_bf1943_legacy(&name, &args)))
                .unwrap_or(false);
            if should_log {
                crate::log_line(&format!("era_ui({mode:?}): shimmed FSCommand:{name}('{args}')"));
            }
        }
    }
}

/// Install the era ingame `FSCommand` shim when `initial_mode` is `Bfbc1` or `Bf1943`.
/// Returns `(installed, expected)`.
#[cfg(target_arch = "x86")]
pub unsafe fn install(initial_mode: ModeId) -> (usize, usize) {
    if !matches!(initial_mode, ModeId::Bfbc1 | ModeId::Bf1943) {
        return (0, 0);
    }
    if let Ok(mut g) = UI_STATE.lock() {
        *g = Some(EraUiState::new(initial_mode));
    }
    if let Some(tramp) = unsafe {
        crate::hooks::install_detour(
            INGAME_FSCOMMAND_DISPATCHER,
            &INGAME_FSCOMMAND_PROLOGUE,
            detour_ingame_fscommand as *const c_void,
        )
    } {
        let _ = ORIG_INGAME_FSCMD.set(unsafe {
            std::mem::transmute::<*const c_void, IngameFsCmdFn>(tramp)
        });
        crate::log_line(&format!(
            "era_ui({initial_mode:?}): detour IngameFSCommandDispatcher @ 0x00753540 installed"
        ));
        return (1, 1);
    }
    crate::log_line(&format!(
        "era_ui({initial_mode:?}): WARNING failed to detour IngameFSCommandDispatcher @ 0x00753540"
    ));
    (0, 1)
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn install(initial_mode: ModeId) -> (usize, usize) {
    if matches!(initial_mode, ModeId::Bfbc1 | ModeId::Bf1943) {
        (0, 1)
    } else {
        (0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_bfbc1_fscommands() {
        for c in ["setArray", "setArrayCount", "setArrayIndex", "refreshArray"] {
            assert_eq!(classify_fscommand(ModeId::Bfbc1, c), ShimAction::HandleArray);
        }
        for c in ["clearDeathMessages", "clearLocalDeathMessages"] {
            assert_eq!(
                classify_fscommand(ModeId::Bfbc1, c),
                ShimAction::BridgeClearMessages
            );
        }
        assert_eq!(
            classify_fscommand(ModeId::Bfbc1, "setActiveAwardInRound"),
            ShimAction::HandleAward
        );
        // BF1943-only commands pass through in BFBC1 mode
        assert_eq!(
            classify_fscommand(ModeId::Bfbc1, "startTutorial"),
            ShimAction::PassThrough
        );
        for c in ["pauseGame", "enableMinimap", "zoomMinimap", "selectSpawnPoint", "enterMenu"] {
            assert_eq!(classify_fscommand(ModeId::Bfbc1, c), ShimAction::PassThrough);
        }
    }

    #[test]
    fn classifies_bf1943_fscommands() {
        for c in ["setArray", "setArrayCount", "setArrayIndex", "refreshArray"] {
            assert_eq!(classify_fscommand(ModeId::Bf1943, c), ShimAction::HandleArray);
        }
        for c in ["clearDeathMessages", "clearLocalDeathMessages"] {
            assert_eq!(
                classify_fscommand(ModeId::Bf1943, c),
                ShimAction::BridgeClearMessages
            );
        }
        for c in [
            "popupRequest",
            "requestFLEntry",
            "setFLListPageNr",
            "setLBListPageNr",
            "startMusic",
            "startTutorial",
        ] {
            assert_eq!(
                classify_fscommand(ModeId::Bf1943, c),
                ShimAction::HandleBf1943Legacy
            );
        }
        // BFBC2 mode passes everything through untouched
        assert_eq!(
            classify_fscommand(ModeId::Bfbc2, "setArray"),
            ShimAction::PassThrough
        );
    }

    #[test]
    fn era_ui_state_tracks_values_and_dedups_logs() {
        let mut st = EraUiState::new(ModeId::Bf1943);
        assert!(st.apply_array_cmd("setArray", "scoreboard"));
        assert!(!st.apply_array_cmd("setArray", "scoreboard"));
        assert!(st.apply_array_cmd("setArrayCount", "24"));
        assert!(st.apply_array_cmd("setArrayIndex", "5"));
        assert!(st.apply_bf1943_legacy("setFLListPageNr", "2"));
        assert!(st.apply_bf1943_legacy("setLBListPageNr", "4"));
        assert_eq!(st.active_array, "scoreboard");
        assert_eq!(st.array_count, 24);
        assert_eq!(st.array_index, 5);
        assert_eq!(st.fl_page, 2);
        assert_eq!(st.lb_page, 4);
    }
}
