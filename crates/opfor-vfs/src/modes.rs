//! 4-Mode engine table generalizing OPFOR's game-mode architecture across:
//! - Mode 0: Battlefield: Bad Company 2 (`BC2`)
//! - Mode 1: BFBC2: Vietnam (`VIETNAM`)
//! - Mode 2: Battlefield 1943 (`PACIFIC`)
//! - Mode 3: Battlefield: Bad Company 1 (`BFBC1`)
//!
//! All sub-modes that ride on `UiManager+0xC8C8C == 1` share an exact 8-byte
//! `gameModChange` frame label so the `0x0075FDB7` push begin/end pair stays valid.

/// Canonical OPFOR era / game mode identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ModeId {
    Bfbc2 = 0,
    Vietnam = 1,
    Bf1943 = 2,
    Bfbc1 = 3,
}

impl ModeId {
    /// All four playable eras in chronological blade order for the 4-Blade Hub.
    pub const HUB_ORDER: [ModeId; 4] = [
        ModeId::Bf1943,
        ModeId::Vietnam,
        ModeId::Bfbc1,
        ModeId::Bfbc2,
    ];

    /// Parse from a CLI/manifest string id.
    #[must_use]
    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bfbc2" | "bc2" | "base" => Some(Self::Bfbc2),
            "vietnam" | "nam" => Some(Self::Vietnam),
            "bf1943" | "1943" | "pacific" | "ww2" => Some(Self::Bf1943),
            "bfbc1" | "bc1" => Some(Self::Bfbc1),
            _ => None,
        }
    }

    /// Parse from `changeGameMod` / `opforEnterMode` numeric argument.
    #[must_use]
    pub fn from_menu_code(code: i32) -> Self {
        match code {
            0 => Self::Bfbc2,
            1 => Self::Vietnam,
            2 | 3 => Self::Bf1943, // keeps DICE's BFBC2143 (3) & Beach (2) compatibility
            4 => Self::Bfbc1,
            _ => Self::Bfbc2,
        }
    }

    /// Bitmask bit used in `extern.opforInstalledModes`.
    #[must_use]
    pub const fn bit(self) -> u32 {
        1u32 << (self as u32)
    }

    /// Underlying `BFBC2Game.exe` `UiManager+0xC8C8C` engine mode (`0` or `1`).
    /// Modes 2 (BF1943) and 3 (BFBC1) ride on engine mode `1` until all 33
    /// mode-reader sites are expanded to native 4-way tables.
    #[must_use]
    pub const fn engine_mode(self) -> i32 {
        match self {
            Self::Bfbc2 => 0,
            Self::Vietnam | Self::Bf1943 | Self::Bfbc1 => 1,
        }
    }
}

/// Static per-mode specification for `dinput8.dll` hooks and the 4-Blade Hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeSpec {
    pub id: ModeId,
    pub slug: &'static str,
    pub display_name: &'static str,
    pub matchmaking_mod_id: &'static str,
    /// Fixed 8-byte `gameModChange` label (`"BFBC2Nam"`, `"BFBC2143"`, `"BFBC2BC1"`).
    pub label_8: [u8; 8],
    pub menu_config: &'static str,
    pub hud_config: &'static str,
    pub game_tips: &'static str,
    pub kit_count: u8,
    pub accent_hex: &'static str,
}

/// Full 4-mode routing table.
#[derive(Debug, Clone)]
pub struct ModeTable {
    specs: [ModeSpec; 4],
    installed_mask: u32,
}

impl Default for ModeTable {
    fn default() -> Self {
        Self::new(ModeId::Bfbc2.bit())
    }
}

impl ModeTable {
    /// Construct the default 4-mode table with a given installed bitmask.
    #[must_use]
    pub fn new(installed_mask: u32) -> Self {
        let specs = [
            ModeSpec {
                id: ModeId::Bfbc2,
                slug: "bfbc2",
                display_name: "Battlefield: Bad Company 2",
                matchmaking_mod_id: "BC2",
                label_8: *b"BFBC2\0\0\0",
                menu_config: "menuConfig",
                hud_config: "hudConfig",
                game_tips: "Menu/GameTips/GameTips",
                kit_count: 4,
                accent_hex: "#f07b19",
            },
            ModeSpec {
                id: ModeId::Vietnam,
                slug: "vietnam",
                display_name: "BFBC2: Vietnam",
                matchmaking_mod_id: "VIETNAM",
                label_8: *b"BFBC2Nam",
                menu_config: "nam_menuConfig",
                hud_config: "nam_hudConfig",
                game_tips: "Menu/GameTips/nam_GameTips",
                kit_count: 4,
                accent_hex: "#d96b27",
            },
            ModeSpec {
                id: ModeId::Bf1943,
                slug: "bf1943",
                display_name: "Battlefield 1943",
                matchmaking_mod_id: "PACIFIC",
                label_8: *b"BFBC2143",
                menu_config: "pacific_menuConfig",
                hud_config: "bf1943_hudConfig",
                game_tips: "Menu/GameTips/pacific_GameTips",
                kit_count: 3,
                accent_hex: "#3ba6a8",
            },
            ModeSpec {
                id: ModeId::Bfbc1,
                slug: "bfbc1",
                display_name: "Battlefield: Bad Company",
                matchmaking_mod_id: "BFBC1",
                label_8: *b"BFBC2BC1",
                menu_config: "bc1_menuConfig",
                hud_config: "bc1_hudConfig",
                game_tips: "Menu/GameTips/bc1_GameTips",
                kit_count: 5,
                accent_hex: "#e0a82e",
            },
        ];
        Self {
            specs,
            installed_mask: installed_mask | ModeId::Bfbc2.bit(),
        }
    }

    #[must_use]
    pub fn spec(&self, id: ModeId) -> &ModeSpec {
        &self.specs[id as usize]
    }

    #[must_use]
    pub const fn installed_mask(&self) -> u32 {
        self.installed_mask
    }

    #[must_use]
    pub const fn is_installed(&self, id: ModeId) -> bool {
        (self.installed_mask & id.bit()) != 0
    }

    /// Resolve a requested mode; if not installed, safely falls back to BFBC2.
    #[must_use]
    pub fn resolve_request(&self, requested: ModeId) -> ModeId {
        if self.is_installed(requested) {
            requested
        } else {
            ModeId::Bfbc2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_byte_labels_match_vietnam_length() {
        let table = ModeTable::new(0x0F);
        assert_eq!(&table.spec(ModeId::Vietnam).label_8, b"BFBC2Nam");
        assert_eq!(&table.spec(ModeId::Bf1943).label_8, b"BFBC2143");
        assert_eq!(&table.spec(ModeId::Bfbc1).label_8, b"BFBC2BC1");
    }

    #[test]
    fn uninstalled_mode_falls_back_to_bfbc2() {
        let table = ModeTable::new(ModeId::Bfbc2.bit() | ModeId::Vietnam.bit());
        assert_eq!(table.resolve_request(ModeId::Vietnam), ModeId::Vietnam);
        assert_eq!(table.resolve_request(ModeId::Bfbc1), ModeId::Bfbc2);
        assert_eq!(table.spec(ModeId::Bfbc1).kit_count, 5);
    }
}
