//! Global UI State Manager for Project OPFOR (`OPFOR.exe`).
//!
//! Coordinates first-class mode switching across all 4 Frostbite 1.x eras
//! (`BFBC1`, `BF1943`, `BFBC2`, `Vietnam`) without relying on grafting WW2
//! onto Vietnam mode inside BFBC2's APT menu.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use opfor_vfs::ModeId;

use crate::input_monitor::InputDevice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalScreen {
    Splash,
    Hub,
    Bf1943,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bf1943Tab {
    Play,
    ServerBrowser,
    Kits,
    Soldier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacificFaction {
    Usmc,
    Ijn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserModeFilter {
    All,
    Conquest,
    AirSuperiority,
}

#[derive(Debug, Clone)]
pub struct PacificMapCard {
    pub id: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub mode_label: &'static str,
    pub level_path: &'static str,
    pub thumb_file: &'static str,
    pub max_players: u32,
    pub stamp: &'static str,
    pub briefing: &'static str,
}

pub const PACIFIC_MAPS: &[PacificMapCard] = &[
    PacificMapCard {
        id: "wake_island_s",
        title: "WAKE ISLAND",
        subtitle: "CENTRAL PACIFIC · DEC 1941",
        mode_label: "CONQUEST",
        level_path: "Levels/WAKE_ISLAND_S",
        thumb_file: "thumb_wake_island_s.png",
        max_players: 24,
        stamp: "AIRFIELD · CARRIER",
        briefing: "Horseshoe coral atoll with carrier LCVP landings, airfield repair stations, Sherman & Chi-Ha armor, and Corsair vs. Zero dogfights.",
    },
    PacificMapCard {
        id: "guadal_canal",
        title: "GUADALCANAL",
        subtitle: "SOLOMON ISLANDS · AUG 1942",
        mode_label: "CONQUEST",
        level_path: "Levels/GUADAL_CANAL",
        thumb_file: "thumb_guadal_canal.png",
        max_players: 24,
        stamp: "RIDGE · AIRFIELD",
        briefing: "Dense jungle ridges, coastal batteries, and contested river crossings leading toward Henderson Field.",
    },
    PacificMapCard {
        id: "iwo_jima_s",
        title: "IWO JIMA",
        subtitle: "VOLCANO ISLANDS · FEB 1945",
        mode_label: "CONQUEST",
        level_path: "Levels/IWO_JIMA_S",
        thumb_file: "thumb_iwo_jima_s.png",
        max_players: 24,
        stamp: "MOUNT SURIBACHI",
        briefing: "Amphibious beach assault under Mount Suribachi with fortified pillboxes, tank lanes, and dual-carrier air support.",
    },
    PacificMapCard {
        id: "coral_sea",
        title: "CORAL SEA",
        subtitle: "PACIFIC THEATRE · MAY 1942",
        mode_label: "AIR SUPERIORITY",
        level_path: "Levels/CORAL_SEA",
        thumb_file: "thumb_coral_sea.png",
        max_players: 24,
        stamp: "AIR COMBAT",
        briefing: "Pure carrier-to-carrier aerial warfare between F4U Corsairs and A6M Zeros over twin island outposts.",
    },
];

/// Global UI state shared across the Hub and all era menus.
pub struct GlobalUiState {
    pub screen: GlobalScreen,
    pub active_mode: ModeId,
    pub input_device: InputDevice,
    pub bf1943_tab: Bf1943Tab,
    pub bf1943_selected_map: usize,
    pub bf1943_faction: PacificFaction,
    pub bf1943_selected_kit: usize,
    pub bf1943_browser_filter: BrowserModeFilter,
    pub bf1943_browser_search: String,
    pub local_stats: BTreeMap<String, f32>,
}

impl GlobalUiState {
    pub fn new(opfor_root: &Path) -> Self {
        Self {
            screen: GlobalScreen::Splash,
            active_mode: ModeId::Bfbc2,
            input_device: InputDevice::KeyboardMouse,
            bf1943_tab: Bf1943Tab::Play,
            bf1943_selected_map: 0,
            bf1943_faction: PacificFaction::Usmc,
            bf1943_selected_kit: 0,
            bf1943_browser_filter: BrowserModeFilter::All,
            bf1943_browser_search: String::new(),
            local_stats: load_stats(&opfor_root.join("user").join("stats.txt")),
        }
    }

    pub fn cycle_bf1943_tab(&mut self, delta: i32) {
        let tabs = [
            Bf1943Tab::Play,
            Bf1943Tab::ServerBrowser,
            Bf1943Tab::Kits,
            Bf1943Tab::Soldier,
        ];
        let cur = tabs.iter().position(|&t| t == self.bf1943_tab).unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(tabs.len() as i32) as usize;
        self.bf1943_tab = tabs[next];
    }

    pub fn reload_stats(&mut self, opfor_root: &Path) {
        self.local_stats = load_stats(&opfor_root.join("user").join("stats.txt"));
    }

    /// Switch the global UI state to a target mode.
    /// All installed modes launch `OPFOREngine.exe` directly; BFBC2/Vietnam open their
    /// native PC menus while BF1943/BFBC1 open their in-game custom menus inside `OPFOREngine.exe`.
    pub fn select_mode(&mut self, mode: ModeId) -> Option<ModeId> {
        self.active_mode = mode;
        Some(mode)
    }

    pub fn return_to_hub(&mut self) {
        self.screen = GlobalScreen::Hub;
    }

    pub fn stat(&self, key: &str) -> f32 {
        self.local_stats.get(key).copied().unwrap_or(0.0)
    }
}

fn load_stats(path: &Path) -> BTreeMap<String, f32> {
    let mut map = BTreeMap::new();
    if let Ok(txt) = fs::read_to_string(path) {
        for raw in txt.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=')
                && let Ok(val) = v.trim().parse::<f32>()
            {
                map.insert(k.trim().to_string(), val);
            }
        }
    }
    map
}
