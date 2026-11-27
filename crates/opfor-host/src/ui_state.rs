//! In-Game Global UI State Manager for `OPFOREngine.exe` (`crates/opfor-host`).
//!
//! Models Battlefield 1943's authentic Xbox 360 post-trial main menu adapted for
//! offline Project OPFOR play:
//!   Main Menu (7 buttons):
//!     0. QUICK MATCH        -> Modal popup (Game Mode + Map -> Enter Game)
//!     1. OFFLINE MATCHES    -> Server Browser (all WW2 maps selectable)
//!     2. CONNECT TO A MATCH -> Direct IP modal popup (UI placeholder, not yet functional)
//!     3. HOST PRIVATE MATCH -> Modal popup (Game Mode + Map -> Local MP Host)
//!     4. STATS & AWARDS     -> Offline stats & BF1943 postcards from user/stats.txt
//!     5. HELP & OPTIONS     -> Live Xbox/KBM controls, Kits preview, Era Switch & Exit
//!     6. ACHIEVEMENTS       -> All 12 BF1943 Xbox 360 achievements (200G) tracked offline

#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use opfor_vfs::ModeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDevice {
    KeyboardMouse,
    GamepadXbox,
    GamepadPlayStation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameScene {
    Booting,
    InMenu,
    InMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bf1943Screen {
    Main,
    OfflineMatches,
    StatsAwards,
    HelpOptions,
    Achievements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bf1943Modal {
    None,
    /// Shared Game Mode + Map selection popup used by both Quick Match & Host Private Match.
    MatchSetup { is_private: bool },
    /// Direct IP prompt for "Connect to a Match" (UI-ready, networking TBD).
    DirectIp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacificModeFilter {
    AllModes,
    Conquest,
    AirSuperiority,
}

impl PacificModeFilter {
    pub const ALL: &'static [PacificModeFilter] = &[
        PacificModeFilter::AllModes,
        PacificModeFilter::Conquest,
        PacificModeFilter::AirSuperiority,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AllModes => "ALL MODES",
            Self::Conquest => "CONQUEST",
            Self::AirSuperiority => "AIR SUPERIORITY",
        }
    }

    pub fn matches_map(self, m: &PacificMap) -> bool {
        match self {
            Self::AllModes => true,
            Self::Conquest => m.mode_key == "CONQUEST",
            Self::AirSuperiority => m.mode_key == "AIR SUPERIORITY",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PacificMap {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub mode_key: &'static str,
    pub mode_label: &'static str,
    pub server_name: &'static str,
    pub level_path: &'static str,
    pub briefing: &'static str,
}

pub const PACIFIC_MAPS: &[PacificMap] = &[
    PacificMap {
        title: "WAKE ISLAND",
        subtitle: "CENTRAL PACIFIC · DEC 1941",
        mode_key: "CONQUEST",
        mode_label: "CONQUEST · 24 PLAYERS",
        server_name: "OPFOR PACIFIC #01 — WAKE ATOLL",
        level_path: "Levels/WAKE_ISLAND_S",
        briefing: "Horseshoe coral atoll with carrier LCVP landings, airfield repair stations, Sherman & Chi-Ha armor, and Corsair vs. Zero dogfights.",
    },
    PacificMap {
        title: "GUADALCANAL",
        subtitle: "SOLOMON ISLANDS · AUG 1942",
        mode_key: "CONQUEST",
        mode_label: "CONQUEST · 24 PLAYERS",
        server_name: "OPFOR PACIFIC #02 — HENDERSON RIDGE",
        level_path: "Levels/GUADAL_CANAL",
        briefing: "Jungle ridges, coastal gun batteries, and contested river crossings leading toward Henderson Field.",
    },
    PacificMap {
        title: "IWO JIMA",
        subtitle: "VOLCANO ISLANDS · FEB 1945",
        mode_key: "CONQUEST",
        mode_label: "CONQUEST · 24 PLAYERS",
        server_name: "OPFOR PACIFIC #03 — MT. SURIBACHI",
        level_path: "Levels/IWO_JIMA_S",
        briefing: "Amphibious beach assault beneath Mount Suribachi with fortified pillboxes, armor lanes, and dual-carrier air support.",
    },
    PacificMap {
        title: "CORAL SEA",
        subtitle: "PACIFIC THEATRE · MAY 1942",
        mode_key: "AIR SUPERIORITY",
        mode_label: "AIR SUPERIORITY · 24 PLAYERS",
        server_name: "OPFOR PACIFIC #04 — CORAL SEA SKIES",
        level_path: "Levels/CORAL_SEA",
        briefing: "Pure carrier-to-carrier aerial combat between F4U Corsairs and A6M Zeros over twin Pacific outposts.",
    },
];

pub const BFBC1_MAPS: &[PacificMap] = &[
    PacificMap {
        title: "OASIS",
        subtitle: "GOLD RUSH · LAUNCH MAP #01",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #01 — OASIS",
        level_path: "Levels/MP_Beachhead_Day1",
        briefing: "Classic Bad Company 1 Gold Rush assault across desert oil fields into the fortified riverside town.",
    },
    PacificMap {
        title: "END OF THE LINE",
        subtitle: "GOLD RUSH · LAUNCH MAP #02",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #02 — END OF THE LINE",
        level_path: "Levels/MP_Beachhead_Day2",
        briefing: "Coastal railway yard and harbor warehouses with patrol boats, armor, and close-quarters gold crate defenses.",
    },
    PacificMap {
        title: "DECONSTRUCTION",
        subtitle: "GOLD RUSH · LAUNCH MAP #03",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #03 — DECONSTRUCTION",
        level_path: "Levels/MP_Harbour_Day1",
        briefing: "Industrial construction yards and urban blocks built for heavy Destruction 1.0 armor assaults.",
    },
    PacificMap {
        title: "HARVEST DAY",
        subtitle: "GOLD RUSH · LAUNCH MAP #04",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #04 — HARVEST DAY",
        level_path: "Levels/MP_Harbour_Day2",
        briefing: "Rolling farmland valleys, destructible barns, and heavy armor lanes leading into the central village.",
    },
    PacificMap {
        title: "VALLEY RUN",
        subtitle: "GOLD RUSH · LAUNCH MAP #05",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #05 — VALLEY RUN",
        level_path: "Levels/MP_Ignition_Day1",
        briefing: "High-speed armored push through a fortified valley corridor toward industrial depots.",
    },
    PacificMap {
        title: "FINAL IGNITION",
        subtitle: "GOLD RUSH · LAUNCH MAP #06",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #06 — FINAL IGNITION",
        level_path: "Levels/MP_Ignition_Day2",
        briefing: "Nighttime industrial oil refinery and rail depot under artillery and mechanized assault.",
    },
    PacificMap {
        title: "ASCENSION",
        subtitle: "GOLD RUSH · LAUNCH MAP #07",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #07 — ASCENSION",
        level_path: "Levels/MP_Monastery_Day1",
        briefing: "Uphill urban climb from riverside streets to the hilltop monastery fortress.",
    },
    PacificMap {
        title: "OVER AND OUT",
        subtitle: "GOLD RUSH · LAUNCH MAP #08",
        mode_key: "GOLD RUSH",
        mode_label: "GOLD RUSH · 24 PLAYERS · 5 KITS",
        server_name: "OPFOR BFBC1 #08 — OVER AND OUT",
        level_path: "Levels/MP_Monastery_Day2",
        briefing: "Hillside artillery outpost and wooded approaches overlooking the communications array.",
    },
    PacificMap {
        title: "WELCOME TO BAD COMPANY",
        subtitle: "SINGLEPLAYER CAMPAIGN · MISSION 01",
        mode_key: "CAMPAIGN",
        mode_label: "SINGLEPLAYER · B-COMPANY",
        server_name: "OPFOR BFBC1 SP #01 — GREENACRES",
        level_path: "Levels/SP_01_GreenAcres",
        briefing: "Private Preston Marlowe joins Haggard, Sweetwater, and Sarge on the Serdaristan border.",
    },
    PacificMap {
        title: "ACTA NON VERBA",
        subtitle: "SINGLEPLAYER CAMPAIGN · MISSION 02",
        mode_key: "CAMPAIGN",
        mode_label: "SINGLEPLAYER · B-COMPANY",
        server_name: "OPFOR BFBC1 SP #02 — ZABOGRAD",
        level_path: "Levels/SP_02_Traitor_Of_Zabograd",
        briefing: "B-Company pushes into Zabograd and discovers the Legionnaire gold.",
    },
    PacificMap {
        title: "CROSSING OVER",
        subtitle: "SINGLEPLAYER CAMPAIGN · MISSION 03",
        mode_key: "CAMPAIGN",
        mode_label: "SINGLEPLAYER · B-COMPANY",
        server_name: "OPFOR BFBC1 SP #03 — CROSSROADS",
        level_path: "Levels/SP_03_Crossroads",
        briefing: "Haggard charges across the neutral Serdaristan border in pursuit of the gold convoy.",
    },
    PacificMap {
        title: "PAR FOR THE COURSE",
        subtitle: "SINGLEPLAYER CAMPAIGN · MISSION 04",
        mode_key: "CAMPAIGN",
        mode_label: "SINGLEPLAYER · B-COMPANY",
        server_name: "OPFOR BFBC1 SP #04 — PAR FOR THE COURSE",
        level_path: "Levels/SP_05_Par_For_The_Course",
        briefing: "Assault the dictator's coastal palace and golf course resort.",
    },
    PacificMap {
        title: "AIR FORCE ONE",
        subtitle: "SINGLEPLAYER CAMPAIGN · MISSION 05",
        mode_key: "CAMPAIGN",
        mode_label: "SINGLEPLAYER · B-COMPANY",
        server_name: "OPFOR BFBC1 SP #05 — HOT ZONE",
        level_path: "Levels/SP_06_Hot_Zone",
        briefing: "Steal a Mi-24 Hind gunship and fly deep into Serdaristan territory.",
    },
    PacificMap {
        title: "CRASH AND GRAB",
        subtitle: "SINGLEPLAYER CAMPAIGN · MISSION 06",
        mode_key: "CAMPAIGN",
        mode_label: "SINGLEPLAYER · B-COMPANY",
        server_name: "OPFOR BFBC1 SP #06 — POW",
        level_path: "Levels/SP_07_POW",
        briefing: "Regroup after the crash and rescue the squad through the mountain valley.",
    },
    PacificMap {
        title: "GHOST TOWN",
        subtitle: "SINGLEPLAYER CAMPAIGN · MISSION 07",
        mode_key: "CAMPAIGN",
        mode_label: "SINGLEPLAYER · B-COMPANY",
        server_name: "OPFOR BFBC1 SP #07 — GHOST TOWN",
        level_path: "Levels/SP_08_Ghost_Town",
        briefing: "Final battle through Sadiz against the Legionnaire's Ka-52 and armor.",
    },
];

pub struct InGameUiState {
    pub active_era: ModeId,
    pub scene: GameScene,
    pub input_device: InputDevice,
    pub screen: Bf1943Screen,
    pub modal: Bf1943Modal,
    pub cursor_idx: usize,
    pub modal_cursor: usize,
    pub popup_mode_idx: usize,
    pub popup_map_idx: usize,
    pub bf1943_selected_map: usize,
    pub bf1943_faction_ijn: bool,
    pub stats_zoom_open: bool,
    pub direct_ip: String,
    pub direct_ip_status: Option<&'static str>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub pending_deploy_level: Option<String>,
    pub pending_mode_switch: Option<ModeId>,
    pub pending_quit: bool,
    prev_pad_buttons: u16,
    prev_stick_y: i8,
    prev_stick_x: i8,
}

impl InGameUiState {
    fn new() -> Self {
        Self {
            active_era: ModeId::Bfbc2,
            scene: GameScene::Booting,
            input_device: InputDevice::KeyboardMouse,
            screen: Bf1943Screen::Main,
            modal: Bf1943Modal::None,
            cursor_idx: 0,
            modal_cursor: 0,
            popup_mode_idx: 0,
            popup_map_idx: 0,
            bf1943_selected_map: 0,
            bf1943_faction_ijn: false,
            stats_zoom_open: false,
            direct_ip: "127.0.0.1:19567".to_string(),
            direct_ip_status: None,
            mouse_x: 640.0,
            mouse_y: 360.0,
            pending_deploy_level: None,
            pending_mode_switch: None,
            pending_quit: false,
            prev_pad_buttons: 0,
            prev_stick_y: 0,
            prev_stick_x: 0,
        }
    }

    pub fn custom_menu_active(&self) -> bool {
        self.scene == GameScene::InMenu
            && matches!(self.active_era, ModeId::Bf1943 | ModeId::Bfbc1)
    }

    pub fn popup_mode(&self) -> PacificModeFilter {
        PacificModeFilter::ALL[self.popup_mode_idx % PacificModeFilter::ALL.len()]
    }

    /// Returns indices into `PACIFIC_MAPS` that match the currently selected popup mode filter.
    pub fn popup_filtered_map_indices(&self) -> Vec<usize> {
        let mode = self.popup_mode();
        let list: Vec<usize> = PACIFIC_MAPS
            .iter()
            .enumerate()
            .filter(|(_, m)| mode.matches_map(m))
            .map(|(i, _)| i)
            .collect();
        if list.is_empty() {
            vec![0]
        } else {
            list
        }
    }

    pub fn popup_selected_map_index(&self) -> usize {
        let list = self.popup_filtered_map_indices();
        list[self.popup_map_idx % list.len()]
    }

    pub fn active_maps(&self) -> &'static [PacificMap] {
        if self.active_era == ModeId::Bfbc1 {
            BFBC1_MAPS
        } else {
            PACIFIC_MAPS
        }
    }

    pub fn current_item_count(&self) -> usize {
        match self.modal {
            Bf1943Modal::MatchSetup { .. } => 4, // Mode, Map, Enter/Start, Cancel
            Bf1943Modal::DirectIp => 3,          // IP Input, Connect, Cancel
            Bf1943Modal::None => match self.screen {
                Bf1943Screen::Main => 7,
                Bf1943Screen::OfflineMatches => self.active_maps().len(),
                Bf1943Screen::StatsAwards => 6,
                Bf1943Screen::HelpOptions => 4,
                Bf1943Screen::Achievements => 1,
            },
        }
    }

    pub fn move_cursor(&mut self, delta: i32) {
        let n = self.current_item_count().max(1) as i32;
        if self.modal != Bf1943Modal::None {
            self.modal_cursor = (self.modal_cursor as i32 + delta).rem_euclid(n) as usize;
            return;
        }
        self.cursor_idx = (self.cursor_idx as i32 + delta).rem_euclid(n) as usize;
        if self.screen == Bf1943Screen::OfflineMatches {
            let max_i = self.active_maps().len().saturating_sub(1);
            self.bf1943_selected_map = self.cursor_idx.min(max_i);
        }
    }

    pub fn cycle_horizontal(&mut self, delta: i32) {
        match self.modal {
            Bf1943Modal::MatchSetup { .. } => match self.modal_cursor {
                0 => {
                    let n = PacificModeFilter::ALL.len() as i32;
                    self.popup_mode_idx = (self.popup_mode_idx as i32 + delta).rem_euclid(n) as usize;
                    self.popup_map_idx = 0;
                }
                1 => {
                    let n = self.popup_filtered_map_indices().len().max(1) as i32;
                    self.popup_map_idx = (self.popup_map_idx as i32 + delta).rem_euclid(n) as usize;
                }
                _ => {}
            },
            Bf1943Modal::None => {
                if self.screen == Bf1943Screen::HelpOptions {
                    self.bf1943_faction_ijn = !self.bf1943_faction_ijn;
                } else if self.screen == Bf1943Screen::StatsAwards && !self.stats_zoom_open {
                    self.cursor_idx = (self.cursor_idx as i32 + delta).rem_euclid(6) as usize;
                }
            }
            Bf1943Modal::DirectIp => {}
        }
    }

    pub fn push_ip_char(&mut self, ch: char) {
        if self.modal != Bf1943Modal::DirectIp {
            return;
        }
        if ch == '\x08' {
            self.direct_ip.pop();
            self.direct_ip_status = None;
        } else if (ch.is_ascii_digit() || ch == '.' || ch == ':') && self.direct_ip.len() < 24 {
            self.direct_ip.push(ch);
            self.direct_ip_status = None;
        }
    }

    pub fn go_back(&mut self) {
        if self.modal != Bf1943Modal::None {
            self.modal = Bf1943Modal::None;
            self.direct_ip_status = None;
            return;
        }
        if self.screen == Bf1943Screen::StatsAwards && self.stats_zoom_open {
            self.stats_zoom_open = false;
            return;
        }
        if self.screen != Bf1943Screen::Main {
            self.screen = Bf1943Screen::Main;
            self.cursor_idx = 0;
        }
    }

    pub fn activate_cursor(&mut self) {
        match self.modal {
            Bf1943Modal::MatchSetup { .. } => match self.modal_cursor {
                0 => self.cycle_horizontal(1),
                1 => self.cycle_horizontal(1),
                2 => {
                    let maps = self.active_maps();
                    let map_idx = if self.active_era == ModeId::Bfbc1 {
                        self.bf1943_selected_map.min(maps.len() - 1)
                    } else {
                        self.popup_selected_map_index()
                    };
                    self.bf1943_selected_map = map_idx;
                    self.modal = Bf1943Modal::None;
                    self.pending_deploy_level = Some(maps[map_idx].level_path.to_string());
                }
                _ => {
                    self.modal = Bf1943Modal::None;
                }
            },
            Bf1943Modal::DirectIp => match self.modal_cursor {
                0 => {}
                1 => {
                    self.direct_ip_status = Some(
                        "DIRECT IP MULTIPLAYER NOT YET AVAILABLE — USE OFFLINE MATCHES OR HOST PRIVATE MATCH",
                    );
                }
                _ => {
                    self.modal = Bf1943Modal::None;
                    self.direct_ip_status = None;
                }
            },
            Bf1943Modal::None => match self.screen {
                Bf1943Screen::Main => match self.cursor_idx {
                    0 => {
                        // QUICK MATCH -> Game Mode + Map modal
                        self.modal = Bf1943Modal::MatchSetup { is_private: false };
                        self.modal_cursor = 0;
                    }
                    1 => {
                        // OFFLINE MATCHES -> Server Browser
                        self.screen = Bf1943Screen::OfflineMatches;
                        self.cursor_idx = self.bf1943_selected_map;
                    }
                    2 => {
                        // CONNECT TO A MATCH -> Direct IP modal
                        self.modal = Bf1943Modal::DirectIp;
                        self.modal_cursor = 1;
                        self.direct_ip_status = None;
                    }
                    3 => {
                        // HOST PRIVATE MATCH -> Same Game Mode + Map modal
                        self.modal = Bf1943Modal::MatchSetup { is_private: true };
                        self.modal_cursor = 0;
                    }
                    4 => {
                        // STATS & AWARDS
                        self.screen = Bf1943Screen::StatsAwards;
                        self.cursor_idx = 0;
                    }
                    5 => {
                        // HELP & OPTIONS
                        self.screen = Bf1943Screen::HelpOptions;
                        self.cursor_idx = 0;
                    }
                    _ => {
                        // ACHIEVEMENTS
                        self.screen = Bf1943Screen::Achievements;
                        self.cursor_idx = 0;
                    }
                },
                Bf1943Screen::OfflineMatches => {
                    let maps = self.active_maps();
                    let idx = self.cursor_idx.min(maps.len() - 1);
                    self.bf1943_selected_map = idx;
                    self.pending_deploy_level = Some(maps[idx].level_path.to_string());
                }
                Bf1943Screen::StatsAwards => {
                    self.stats_zoom_open = !self.stats_zoom_open;
                }
                Bf1943Screen::Achievements => {
                    self.go_back();
                }
                Bf1943Screen::HelpOptions => match self.cursor_idx {
                    0 => self.bf1943_faction_ijn = !self.bf1943_faction_ijn,
                    1 => self.pending_mode_switch = Some(ModeId::Bfbc2),
                    2 => self.pending_mode_switch = Some(ModeId::Vietnam),
                    _ => self.pending_quit = true,
                },
            },
        }
    }
}

pub static UI_STATE: Mutex<Option<InGameUiState>> = Mutex::new(None);

pub fn init(initial_era: ModeId) {
    let mut st = InGameUiState::new();
    st.active_era = initial_era;
    if let Ok(mut g) = UI_STATE.lock() {
        *g = Some(st);
    }
}

pub fn set_scene(scene: GameScene) {
    if let Ok(mut g) = UI_STATE.lock() {
        if let Some(st) = g.as_mut() {
            if st.scene != scene {
                crate::log_line(&format!(
                    "ui_state: scene {:?} -> {:?} (era={:?})",
                    st.scene, scene, st.active_era
                ));
                st.scene = scene;
            }
        }
    }
}

pub fn set_era(era: ModeId) {
    if let Ok(mut g) = UI_STATE.lock() {
        if let Some(st) = g.as_mut() {
            st.active_era = era;
        }
    }
}

pub fn note_kbm_input() {
    if let Ok(mut g) = UI_STATE.lock() {
        if let Some(st) = g.as_mut() {
            st.input_device = InputDevice::KeyboardMouse;
        }
    }
}

pub fn note_playstation_input() {
    if let Ok(mut g) = UI_STATE.lock() {
        if let Some(st) = g.as_mut() {
            if st.input_device != InputDevice::GamepadPlayStation {
                st.input_device = InputDevice::GamepadPlayStation;
                crate::dx_overlay::mark_dirty();
            }
        }
    }
}

pub fn current_input_device() -> InputDevice {
    UI_STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.input_device))
        .unwrap_or(InputDevice::KeyboardMouse)
}

pub fn is_custom_menu_active() -> bool {
    UI_STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(InGameUiState::custom_menu_active))
        .unwrap_or(false)
}

// --- Dynamic XInput Polling ---

#[repr(C)]
#[derive(Default)]
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
#[derive(Default)]
struct XInputState {
    packet_number: u32,
    gamepad: XInputGamepad,
}

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;
static XINPUT_FN: OnceLock<Option<XInputGetStateFn>> = OnceLock::new();

fn load_xinput() -> Option<XInputGetStateFn> {
    *XINPUT_FN.get_or_init(|| {
        for dll in [
            b"xinput1_4.dll\0".as_slice(),
            b"xinput1_3.dll\0".as_slice(),
            b"xinput9_1_0.dll\0".as_slice(),
        ] {
            let h = unsafe { crate::win32::LoadLibraryA(dll.as_ptr()) };
            if !h.is_null() {
                let p = unsafe { crate::win32::GetProcAddress(h, b"XInputGetState\0".as_ptr()) };
                if !p.is_null() {
                    return Some(unsafe { std::mem::transmute::<*const c_void, XInputGetStateFn>(p) });
                }
            }
        }
        None
    })
}

pub fn poll_gamepad() {
    let mut xst = XInputState::default();
    let got_xinput = load_xinput()
        .map(|get_state| unsafe { get_state(0, &mut xst) } == 0)
        .unwrap_or(false);
    let mut from_sony_hid = false;
    if !got_xinput {
        let mut sony = crate::controller::XInputState::default();
        if !crate::controller::poll_sony_xinput(&mut sony) {
            return;
        }
        xst.packet_number = sony.packet_number;
        xst.gamepad.buttons = sony.gamepad.buttons;
        xst.gamepad.left_trigger = sony.gamepad.left_trigger;
        xst.gamepad.right_trigger = sony.gamepad.right_trigger;
        xst.gamepad.thumb_lx = sony.gamepad.thumb_lx;
        xst.gamepad.thumb_ly = sony.gamepad.thumb_ly;
        xst.gamepad.thumb_rx = sony.gamepad.thumb_rx;
        xst.gamepad.thumb_ry = sony.gamepad.thumb_ry;
        from_sony_hid = true;
    }
    let b = xst.gamepad.buttons;
    let stick_y: i8 = if xst.gamepad.thumb_ly > 14_000 {
        -1
    } else if xst.gamepad.thumb_ly < -14_000 {
        1
    } else {
        0
    };
    let stick_x: i8 = if xst.gamepad.thumb_lx < -14_000 {
        -1
    } else if xst.gamepad.thumb_lx > 14_000 {
        1
    } else {
        0
    };
    let active = b != 0
        || stick_y != 0
        || stick_x != 0
        || xst.gamepad.left_trigger > 30
        || xst.gamepad.right_trigger > 30;

    let mut dirty = false;
    if let Ok(mut g) = UI_STATE.lock() {
        if let Some(st) = g.as_mut() {
            let rising = b & !st.prev_pad_buttons;
            st.prev_pad_buttons = b;
            let stick_up = stick_y == -1 && st.prev_stick_y != -1;
            let stick_down = stick_y == 1 && st.prev_stick_y != 1;
            let stick_left = stick_x == -1 && st.prev_stick_x != -1;
            let stick_right = stick_x == 1 && st.prev_stick_x != 1;
            st.prev_stick_y = stick_y;
            st.prev_stick_x = stick_x;

            if active {
                let pad_dev = if from_sony_hid {
                    InputDevice::GamepadPlayStation
                } else {
                    crate::controller::active_gamepad_device()
                };
                if st.input_device != pad_dev {
                    st.input_device = pad_dev;
                    dirty = true;
                }
            }
            if st.custom_menu_active() {
                let up = (rising & 0x0001) != 0 || stick_up;
                let down = (rising & 0x0002) != 0 || stick_down;
                let left = (rising & 0x0004) != 0 || stick_left;
                let right = (rising & 0x0008) != 0 || stick_right;
                let confirm = (rising & 0x1000) != 0; // A
                let back = (rising & 0x2000) != 0; // B
                let y_btn = (rising & 0x8000) != 0; // Y

                if up {
                    st.move_cursor(-1);
                    crate::dx_overlay::play_ui_sfx("ui_scroll.wav");
                    dirty = true;
                }
                if down {
                    st.move_cursor(1);
                    crate::dx_overlay::play_ui_sfx("ui_scroll.wav");
                    dirty = true;
                }
                if left {
                    st.cycle_horizontal(-1);
                    crate::dx_overlay::play_ui_sfx("ui_tick.wav");
                    dirty = true;
                }
                if right {
                    st.cycle_horizontal(1);
                    crate::dx_overlay::play_ui_sfx("ui_tick.wav");
                    dirty = true;
                }
                if confirm {
                    st.activate_cursor();
                    crate::dx_overlay::play_ui_sfx("ui_forward.wav");
                    dirty = true;
                }
                if back && (st.modal != Bf1943Modal::None || st.screen != Bf1943Screen::Main) {
                    st.go_back();
                    crate::dx_overlay::play_ui_sfx("ui_backwards.wav");
                    dirty = true;
                }
                if y_btn && st.modal == Bf1943Modal::None && st.screen == Bf1943Screen::HelpOptions {
                    st.bf1943_faction_ijn = !st.bf1943_faction_ijn;
                    crate::dx_overlay::play_ui_sfx("ui_tick.wav");
                    dirty = true;
                }
            }
        }
    }
    if dirty {
        crate::dx_overlay::mark_dirty();
    }
}
