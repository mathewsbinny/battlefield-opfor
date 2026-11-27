//! Pure-Rust Era UI, Minimap, Icon, APT & Dual KBM/Gamepad Input Parity Pipeline for `opfor-cli`.
//!
//! Zero Python or external repo dependencies — runs identically on any clean user PC via
//! `Setup.exe` -> `utility/opfor-cli.exe era-ui-input-parity --era <bfbc1|bf1943> --opfor <dir>`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use opfor_apt::bigf;

use crate::bfbc1::{FbrbEntry, flag_val, read_fbrb, write_fbrb};

const SOLDIER_CONCEPT: &str = "input/bfbc2soldierinputconcepts/5780a065-4637-4fe3-9466-f8e589382cb9";
const SOLDIER_MAPPING: &str = "input/inputmappings/bfbc2soldierinputmapping/ea5c6c02-95a6-44d3-9c9c-3338661d2075";
const VEHICLE_CONCEPT: &str = "input/bfbc2vehicleinputconcepts/9c6433f0-a3ac-4c41-b29c-90f890ae5e05";
const VEHICLE_MAPPING: &str = "input/inputmappings/bfbc2vehicleinputmapping/b37cb2d9-4357-48b6-a31f-9eeff349ea62";
const AIR_CONCEPT: &str = "input/bfbc2airinputconcepts/b1785db2-a92e-4586-a3ba-990960197678";
const AIR_MAPPING: &str = "input/inputmappings/bfbc2airinputmapping/a67cdf7a-94e4-435d-b072-85a622ddb7ed";
const GUNNER_MAPPING: &str = "input/inputmappings/bfbc2gunnerinputmapping/330b0df6-c990-44a2-8a91-61ee6aff294c";

const BF43_ICON_REMAP: &[(&str, &str)] = &[
    ("UIHudIcon_BombRaidEnemy", "UIHudIcon_ArtilleryStrikeMinimap"),
    ("UIHudIcon_BombRaidFriendly", "UIHudIcon_ArtilleryStrikeNametag"),
    ("UIHudIcon_BombRaidNeutral", "UIHudIcon_ArtilleryTarget"),
    ("UIHudIcon_BombRaidSquad", "UIHudIcon_RadioOrderHighlight"),
    ("UIHudIcon_BunkerEnemy", "UIHudIcon_UAVEnemy"),
    ("UIHudIcon_BunkerFriendly", "UIHudIcon_UAVFriendly"),
    ("UIHudIcon_BunkerNeutral", "UIHudIcon_UAVNeutral"),
    ("UIHudIcon_BunkerSquad", "UIHudIcon_UAVSquad"),
    ("UIHudIcon_StationaryNeutral", "UIHudIcon_Stationary"),
    ("UIHudIcon_StationaryFriendly", "UIHudIcon_StationaryWeapon"),
    ("UIHudIcon_StationaryEnemy", "UIHudIcon_StationaryUAV"),
    ("UIHudIcon_StationarySquad", "UIHudIcon_SecondaryObjective"),
    ("UIHudIcon_StrategicNeutral", "UIHudIcon_Strategic"),
    ("UIHudIcon_StrategicFriendly", "UIHudIcon_ObjectiveDefender"),
    ("UIHudIcon_StrategicEnemy", "UIHudIcon_ObjectiveAttacker"),
    ("UIHudIcon_StrategicSquad", "UIHudIcon_ObjectiveDefend"),
    ("UIHudIcon_ConquestFlagHotzone", "UIHudIcon_EnemyFlagUnderAttack"),
    ("UIHudIcon_ConquestFlagHotzoneSmall", "UIHudIcon_FriendlyFlagUnderAttack"),
    ("UIHudIcon_ConquestFlagEnemySmall", "UIHudIcon_EnemyFlag"),
    ("UIHudIcon_ConquestFlagFriendlySmall", "UIHudIcon_FriendlyFlag"),
    ("UIHudIcon_ConquestFlagNeutralSmall", "UIHudIcon_NeutralFlag"),
    ("UIHudIcon_ConquestFlagEnemyUncapSmall", "UIHudIcon_EnemyFlagLit"),
    ("UIHudIcon_ConquestFlagFriendlyUncapSmall", "UIHudIcon_FriendlyFlagLit"),
    ("UIHudIcon_OrderAttackSmall", "UIHudIcon_ObjectiveDestroy"),
    ("UIHudIcon_OrderDefendSmall", "UIHudIcon_ObjectiveGoto"),
];

/// Deterministic FNV-1a-derived UUIDv4-formatted GUID string (uppercase).
pub fn deterministic_guid(seed: &str) -> String {
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0x8422_2325_cbf2_9ce4;
    for &b in seed.as_bytes() {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(0x0000_0100_0000_01b3);
        h2 = h2.wrapping_mul(0x0000_0100_0000_01b3) ^ (b as u64);
    }
    let b1 = h1.to_be_bytes();
    let b2 = h2.to_be_bytes();
    format!(
        "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-4{:01X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        b1[0], b1[1], b1[2], b1[3], b1[4], b1[5],
        b1[6] & 0x0F, b1[7],
        (b2[0] & 0x3F) | 0x80, b2[1],
        b2[2], b2[3], b2[4], b2[5], b2[6], b2[7],
    )
}

fn dbx_to_xml(_xenolith: &Path, dbx_bytes: &[u8]) -> Result<String, String> {
    x360_frostbite::dbx::parse(dbx_bytes)
        .map(|d| d.to_xml())
        .map_err(|e| format!("dbx-to-xml failed: {e}"))
}

fn xml_to_dbx(_xenolith: &Path, xml: &str) -> Result<Vec<u8>, String> {
    x360_frostbite::dbx::from_xml(xml)
        .map(|d| d.to_bytes())
        .map_err(|e| format!("xml-to-dbx failed: {e}"))
}

fn rename_xenon_to_win32_str(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if let Some(pos) = lower.find("_textures_xenon") {
        let mut out = String::with_capacity(s.len());
        out.push_str(&s[..pos]);
        out.push_str("_textures_Win32");
        out.push_str(&s[pos + 15..]);
        out
    } else {
        s.to_string()
    }
}

fn patch_xenon_to_win32_bytes(data: &mut [u8]) {
    let needle = b"_textures_xenon";
    let repl = b"_textures_Win32";
    if data.len() < needle.len() {
        return;
    }
    let mut i = 0;
    while i + needle.len() <= data.len() {
        if data[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            data[i..i + repl.len()].copy_from_slice(repl);
            i += repl.len();
        } else {
            i += 1;
        }
    }
}

fn upsert_fbrb(entries: &mut Vec<FbrbEntry>, name: String, ext: String, flags: u32, data: Vec<u8>) {
    let key = name.to_ascii_lowercase();
    if let Some(e) = entries.iter_mut().find(|e| e.name.to_ascii_lowercase() == key) {
        e.data = data;
        if !ext.is_empty() {
            e.ext = ext;
        }
        e.flags = flags;
    } else {
        entries.push(FbrbEntry {
            name,
            ext,
            flags,
            data,
        });
    }
}

// --- Pure-Rust EA BIGF / APT binary helpers (delegating BIGF I/O to `opfor_apt::bigf`) ---

fn read_u32_le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn write_u32_le(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn read_cstr(b: &[u8], off: usize) -> String {
    if off >= b.len() {
        return String::new();
    }
    let nul = b[off..].iter().position(|&x| x == 0).unwrap_or(0);
    String::from_utf8_lossy(&b[off..off + nul]).into_owned()
}

/// Fix any big-endian `StaticText` (ctype==10) record/glyph arrays left by `xenolith`.
pub fn fix_apt_stext_endian(res_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut members = bigf::parse(res_bytes)?;
    let apt_idx = members
        .iter()
        .position(|e| e.name.to_ascii_lowercase().ends_with(".apt"))
        .ok_or("no .apt")?;
    let const_idx = members
        .iter()
        .position(|e| e.name.to_ascii_lowercase().ends_with(".const"))
        .ok_or("no .const")?;
    let mv_off = read_u32_le(&members[const_idx].data, 20) as usize;
    let a = &mut members[apt_idx].data;
    let n_chars = read_u32_le(a, mv_off + 20) as usize;
    let ctab_off = read_u32_le(a, mv_off + 24) as usize;
    let mut fixed = 0usize;
    for idx in 0..n_chars {
        let cp = read_u32_le(a, ctab_off + idx * 4) as usize;
        if cp == 0 || cp + 56 > a.len() || read_u32_le(a, cp) != 10 {
            continue;
        }
        let rec_cnt = read_u32_le(a, cp + 48) as usize;
        let rec_ptr = read_u32_le(a, cp + 52) as usize;
        if rec_cnt == 0 || rec_ptr == 0 || rec_ptr + rec_cnt * 56 > a.len() {
            continue;
        }
        for r in 0..rec_cnt {
            let ro = rec_ptr + r * 56;
            let g_ptr = read_u32_le(a, ro + 52);
            let g_ptr_sw = g_ptr.swap_bytes() as usize;
            if (g_ptr as usize) >= a.len() && g_ptr_sw > 0 && g_ptr_sw < a.len() {
                for w in 0..14 {
                    let v = u32::from_be_bytes([a[ro + w * 4], a[ro + w * 4 + 1], a[ro + w * 4 + 2], a[ro + w * 4 + 3]]);
                    write_u32_le(a, ro + w * 4, v);
                }
                let g_cnt_new = read_u32_le(a, ro + 48) as usize;
                let g_ptr_new = read_u32_le(a, ro + 52) as usize;
                if g_ptr_new + g_cnt_new * 4 <= a.len() {
                    for gw in 0..g_cnt_new {
                        let o = g_ptr_new + gw * 4;
                        let gv = u32::from_be_bytes([a[o], a[o + 1], a[o + 2], a[o + 3]]);
                        write_u32_le(a, o, gv);
                    }
                }
                fixed += 1;
            }
        }
    }
    if fixed == 0 {
        return Ok(res_bytes.to_vec());
    }
    Ok(bigf::write(&members))
}

/// Tint BFBC2 `Menu/platform_buttons.res` `ButtonsHolder` (char 67) `aText`/`bText`/`yText`
/// dynamic PC keycap boxes in-place, leaving Xbox 360 ('Xenon') & PS3 ('Ps3') sprites untouched.
pub fn tint_platform_buttons_res(res_bytes: &[u8], argb_tint: u32) -> Result<Vec<u8>, String> {
    let mut members = bigf::parse(res_bytes)?;
    let apt_idx = members
        .iter()
        .position(|e| e.name.to_ascii_lowercase().ends_with(".apt"))
        .ok_or("no .apt")?;
    let const_idx = members
        .iter()
        .position(|e| e.name.to_ascii_lowercase().ends_with(".const"))
        .ok_or("no .const")?;
    let mv_off = read_u32_le(&members[const_idx].data, 20) as usize;
    let a = &mut members[apt_idx].data;
    let n_chars = read_u32_le(a, mv_off + 20) as usize;
    let ctab_off = read_u32_le(a, mv_off + 24) as usize;
    if 67 >= n_chars {
        return Err("platform_buttons missing char 67 (ButtonsHolder)".into());
    }
    let ch67 = read_u32_le(a, ctab_off + 67 * 4) as usize;
    let frames_ptr = read_u32_le(a, ch67 + 12) as usize;
    let item_cnt = read_u32_le(a, frames_ptr) as usize;
    let items_ptr = read_u32_le(a, frames_ptr + 4) as usize;
    let mut tinted = 0usize;
    for k in 0..item_cnt {
        let it_off = read_u32_le(a, items_ptr + k * 4) as usize;
        if it_off + 64 <= a.len() && read_u32_le(a, it_off) == 3 {
            let name_ptr = read_u32_le(a, it_off + 52) as usize;
            let nm = read_cstr(a, name_ptr);
            if matches!(nm.as_str(), "aText" | "bText" | "yText") {
                let flags = read_u32_le(a, it_off + 4) | 0x08; // PlaceFlagHasColorTransform
                write_u32_le(a, it_off + 4, flags);
                write_u32_le(a, it_off + 40, argb_tint);
                tinted += 1;
            }
        }
    }
    if tinted != 3 {
        return Err(format!("expected 3 PC keycap boxes in platform_buttons, tinted {tinted}"));
    }
    Ok(bigf::write(&members))
}

const MOUSE_HITTEST_STRINGS: [&str; 24] = [
    "_root",
    "__opforMouseInit",
    "__opforHoverClip",
    "Mouse",
    "addListener",
    "onMouseMove",
    "onMouseDown",
    "_xmouse",
    "_ymouse",
    "_visible",
    "hitTest",
    "items",
    "length",
    "selectItem",
    "onRollOver",
    "onPress",
    "onRelease",
    "__ml",
    "__k",
    "__c",
    "__arr",
    "__i",
    "__it",
    "__h",
];

/// Verified 524-byte Frame-0 ActionScript 2 `Mouse.addListener` hover/click hit-test payload
/// (483B bytecode + 41B inline C-string table; 11 relocatable `u32` offsets relative to `ao`).
const MOUSE_HITTEST_BLOB: [u8; 524] = [
    0x88, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0xa4, 0x3f, 0x06, 0x00, 0xa2, 0x00, 0x1c, 0xaf,
    0x01, 0x73, 0x49, 0x9d, 0xcd, 0x01, 0x00, 0x00, 0xa2, 0x00, 0x1c, 0xa2, 0x01, 0x73, 0x4f, 0x59,
    0x43, 0xa6, 0x00, 0x00, 0x9f, 0x3f, 0x06, 0x00, 0xa2, 0x11, 0x1c, 0xa2, 0x05, 0x9b, 0x00, 0x00,
    0x9e, 0x3f, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x01, 0x00, 0x00,
    0x32, 0x54, 0x76, 0x98, 0x78, 0x56, 0x34, 0x12, 0xa2, 0x00, 0x1c, 0x55, 0x4c, 0x5b, 0x49, 0x9d,
    0xfc, 0x00, 0x00, 0x00, 0xa6, 0x00, 0x00, 0x00, 0x9a, 0x3f, 0x06, 0x00, 0xa2, 0x00, 0x1c, 0xa2,
    0x12, 0x1c, 0x4e, 0xa6, 0x96, 0x3f, 0x06, 0x00, 0xa2, 0x13, 0x1c, 0xaf, 0x0b, 0xa6, 0x00, 0x00,
    0x90, 0x3f, 0x06, 0x00, 0xa2, 0x14, 0x1c, 0x5c, 0x49, 0x9d, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00,
    0xa2, 0x13, 0x1c, 0xaf, 0x0d, 0x5c, 0x49, 0x9d, 0x74, 0x00, 0x00, 0x00, 0x59, 0xa6, 0x00, 0x00,
    0x8c, 0x3f, 0x06, 0x00, 0xa2, 0x15, 0x1c, 0xa2, 0x14, 0x1c, 0xaf, 0x0c, 0x48, 0xb8, 0x00, 0x00,
    0xa8, 0xff, 0xff, 0xff, 0xa2, 0x14, 0x1c, 0xa2, 0x15, 0x1c, 0x4e, 0xa6, 0x87, 0x3f, 0x06, 0x00,
    0xa2, 0x16, 0x1c, 0xaf, 0x09, 0xb8, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x73, 0xa2, 0x00, 0x1c,
    0xaf, 0x08, 0xa2, 0x00, 0x1c, 0xaf, 0x07, 0xb5, 0x03, 0xa2, 0x16, 0x1c, 0xb3, 0x0a, 0xb8, 0x00,
    0x18, 0x00, 0x00, 0x00, 0xa2, 0x15, 0x1c, 0x5a, 0xa2, 0x13, 0x1c, 0xb2, 0x0d, 0xa2, 0x00, 0x1c,
    0xa2, 0x02, 0xa2, 0x16, 0x1c, 0x4f, 0x99, 0x00, 0x53, 0x00, 0x00, 0x00, 0xa2, 0x15, 0x1c, 0x50,
    0xa6, 0x00, 0x00, 0x00, 0x83, 0x3f, 0x06, 0x00, 0x99, 0x00, 0x00, 0x00, 0x94, 0xff, 0xff, 0xff,
    0xa2, 0x13, 0x1c, 0xaf, 0x09, 0xb8, 0x00, 0x00, 0x40, 0xff, 0xff, 0xff, 0xa2, 0x13, 0x1c, 0xaf,
    0x0e, 0x5c, 0x49, 0x9d, 0x34, 0xff, 0xff, 0xff, 0x73, 0xa2, 0x00, 0x1c, 0xaf, 0x08, 0xa2, 0x00,
    0x1c, 0xaf, 0x07, 0xb5, 0x03, 0xa2, 0x13, 0x1c, 0xb3, 0x0a, 0xb8, 0x00, 0x1c, 0xff, 0xff, 0xff,
    0x59, 0xa2, 0x13, 0x1c, 0xb2, 0x0e, 0xa2, 0x00, 0x1c, 0xa2, 0x02, 0xa2, 0x13, 0x1c, 0x4f, 0x4c,
    0x5b, 0x49, 0x9d, 0x00, 0x08, 0x00, 0x00, 0x00, 0x17, 0x99, 0x00, 0x00, 0xef, 0xff, 0xff, 0xff,
    0x17, 0x00, 0x4f, 0xa2, 0x11, 0x1c, 0xa2, 0x06, 0x9b, 0x00, 0x00, 0x00, 0x82, 0x3f, 0x06, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x67, 0x00, 0x00, 0x00, 0x32, 0x54, 0x76, 0x98,
    0x78, 0x56, 0x34, 0x12, 0xa2, 0x00, 0x1c, 0xaf, 0x02, 0xa6, 0x00, 0x00, 0x7e, 0x3f, 0x06, 0x00,
    0xa2, 0x17, 0x1c, 0x5c, 0x49, 0x9d, 0x00, 0x00, 0x4e, 0x00, 0x00, 0x00, 0xa2, 0x17, 0x1c, 0xaf,
    0x09, 0xb8, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x73, 0xa2, 0x00, 0x1c, 0xaf, 0x08, 0xa2, 0x00,
    0x1c, 0xaf, 0x07, 0xb5, 0x03, 0xa2, 0x17, 0x1c, 0xb3, 0x0a, 0xb8, 0x00, 0x2a, 0x00, 0x00, 0x00,
    0xa2, 0x17, 0x1c, 0xaf, 0x0f, 0x5c, 0x49, 0x9d, 0x0c, 0x00, 0x00, 0x00, 0x59, 0xa2, 0x17, 0x1c,
    0xb2, 0x0f, 0x99, 0x00, 0x12, 0x00, 0x00, 0x00, 0xa2, 0x17, 0x1c, 0xaf, 0x10, 0x5c, 0x49, 0x9d,
    0x06, 0x00, 0x00, 0x00, 0x59, 0xa2, 0x17, 0x1c, 0xb2, 0x10, 0x00, 0x4f, 0xa2, 0x11, 0x1c, 0x5a,
    0xa2, 0x03, 0x1c, 0xb2, 0x04, 0x00, 0x5f, 0x5f, 0x68, 0x00, 0x00, 0x5f, 0x5f, 0x69, 0x00, 0x5f,
    0x5f, 0x69, 0x74, 0x00, 0x5f, 0x5f, 0x69, 0x00, 0x5f, 0x5f, 0x61, 0x72, 0x72, 0x00, 0x5f, 0x5f,
    0x63, 0x00, 0x5f, 0x5f, 0x6b, 0x00, 0x00, 0x5f, 0x5f, 0x6d, 0x6c, 0x00,
];

const MOUSE_HITTEST_RELOCS: [(usize, u32); 11] = [
    (8, 524),
    (36, 519),
    (48, 518),
    (88, 514),
    (100, 510),
    (112, 504),
    (144, 500),
    (172, 495),
    (244, 491),
    (348, 490),
    (380, 486),
];

/// Inject the Frame-0 ActionScript 2 `Mouse.addListener` hover/click hit-test into a console-ported
/// EA APT `SwfMovie` `.res` archive. Idempotent (skips if `__opforMouseInit` is already present).
pub fn inject_apt_mouse_hittest(res_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut members = bigf::parse(res_bytes)?;
    let apt_idx = members
        .iter()
        .position(|e| e.name.to_ascii_lowercase().ends_with(".apt"))
        .ok_or("no .apt")?;
    let const_idx = members
        .iter()
        .position(|e| e.name.to_ascii_lowercase().ends_with(".const"))
        .ok_or("no .const")?;

    // Idempotency check
    if members[const_idx].data.windows(17).any(|w| w == b"__opforMouseInit\0") {
        return Ok(res_bytes.to_vec());
    }

    // 1. Intern our 24 constant strings into `.const` without moving existing strings
    let cst = &mut members[const_idx].data;
    if cst.len() < 32 {
        return Err("truncated .const".into());
    }
    let mv_off = read_u32_le(cst, 20) as usize;
    let c_cnt = read_u32_le(cst, 24) as usize;
    let tbl_off = read_u32_le(cst, 28) as usize;
    if tbl_off + c_cnt * 8 > cst.len() {
        return Err("invalid .const table".into());
    }
    let mut entries: Vec<(u32, u32)> = Vec::with_capacity(c_cnt + 24);
    for i in 0..c_cnt {
        let o = tbl_off + i * 8;
        entries.push((read_u32_le(cst, o), read_u32_le(cst, o + 4)));
    }
    let mut pool_idxs = [0u32; 24];
    for (k, &s) in MOUSE_HITTEST_STRINGS.iter().enumerate() {
        let existing = entries.iter().enumerate().find_map(|(i, &(t, v))| {
            if t == 1 && (v as usize) < cst.len() && read_cstr(cst, v as usize) == s {
                Some(i as u32)
            } else {
                None
            }
        });
        if let Some(idx) = existing {
            pool_idxs[k] = idx;
        } else {
            let str_off = cst.len() as u32;
            cst.extend_from_slice(s.as_bytes());
            cst.push(0);
            let idx = entries.len() as u32;
            entries.push((1, str_off));
            pool_idxs[k] = idx;
        }
    }
    while cst.len() % 4 != 0 {
        cst.push(0);
    }
    let new_tbl_off = cst.len() as u32;
    for (t, v) in &entries {
        cst.extend_from_slice(&t.to_le_bytes());
        cst.extend_from_slice(&v.to_le_bytes());
    }
    write_u32_le(cst, 24, entries.len() as u32);
    write_u32_le(cst, 28, new_tbl_off);

    // 2. Append Frame-0 Action item + relocated bytecode + 24 const indices to `.apt`
    let apt = &mut members[apt_idx].data;
    if mv_off + 16 > apt.len() {
        return Err("invalid Movie offset in .apt".into());
    }
    let fp = read_u32_le(apt, mv_off + 12) as usize;
    if fp + 8 > apt.len() {
        return Err("invalid Frame 0 table in .apt".into());
    }
    let ic = read_u32_le(apt, fp) as usize;
    let ip = read_u32_le(apt, fp + 4) as usize;
    if ip + ic * 4 > apt.len() {
        return Err("invalid Frame 0 items in .apt".into());
    }
    let mut existing_items = Vec::with_capacity(ic + 1);
    for i in 0..ic {
        existing_items.push(read_u32_le(apt, ip + i * 4));
    }

    while apt.len() % 4 != 0 {
        apt.push(0);
    }
    let new_ip = apt.len() as u32;
    let io = new_ip + ((ic + 1) * 4) as u32;
    let ao = io + 8;

    for &it in &existing_items {
        apt.extend_from_slice(&it.to_le_bytes());
    }
    apt.extend_from_slice(&io.to_le_bytes());
    // FrameItem: type=1 (Action), action_offset=ao
    apt.extend_from_slice(&1u32.to_le_bytes());
    apt.extend_from_slice(&ao.to_le_bytes());

    let mut blob = MOUSE_HITTEST_BLOB;
    for &(off, rel) in &MOUSE_HITTEST_RELOCS {
        write_u32_le(&mut blob, off, ao + rel);
    }
    apt.extend_from_slice(&blob);
    for &idx in &pool_idxs {
        apt.extend_from_slice(&idx.to_le_bytes());
    }
    write_u32_le(apt, fp, (ic + 1) as u32);
    write_u32_le(apt, fp + 4, new_ip);

    Ok(bigf::write(&members))
}

// --- XML/DBX Parity Transformations ---

fn map_bc1_input_ref(ref_lower: &str) -> (&'static str, &'static str) {
    if ref_lower.contains("helivehicleinput") || ref_lower.contains("air") {
        (AIR_CONCEPT, AIR_MAPPING)
    } else if ref_lower.contains("gunnervehicleinput")
        || ref_lower.contains("artilleryvehicleinput")
        || ref_lower.contains("laserguidedbombinput")
    {
        (VEHICLE_CONCEPT, GUNNER_MAPPING)
    } else if ref_lower.contains("carvehicleinput")
        || ref_lower.contains("tankvehicleinput")
        || ref_lower.contains("boat")
    {
        (VEHICLE_CONCEPT, VEHICLE_MAPPING)
    } else {
        (SOLDIER_CONCEPT, SOLDIER_MAPPING)
    }
}

fn upgrade_input_xml(xml: &str) -> (String, usize, usize) {
    let mut out = String::with_capacity(xml.len() + 256);
    let mut up = 0usize;
    let mut eia = 0usize;
    for line in xml.lines() {
        if line.contains("name=\"InputActionMaps\"") && line.contains("ref=\"") {
            let indent_len = line.len() - line.trim_start().len();
            let indent = &line[..indent_len];
            if let Some(s) = line.find("ref=\"") {
                let rest = &line[s + 5..];
                if let Some(e) = rest.find('"') {
                    let ref_val = &rest[..e];
                    if !ref_val.is_empty() && !ref_val.eq_ignore_ascii_case("null") {
                        let (cref, mref) = map_bc1_input_ref(&ref_val.to_ascii_lowercase());
                        out.push_str(&format!(
                            "{indent}<field ref=\"{cref}\" name=\"InputConceptDefinition\" />\n\
                             {indent}<field ref=\"{mref}\" name=\"InputMapping\" />\n"
                        ));
                        up += 1;
                        continue;
                    }
                }
            }
        }
        if line.contains("EiaCycleSecondaryInventory") {
            out.push_str(&line.replace("EiaCycleSecondaryInventory", "EiaToggleWeapon"));
            out.push('\n');
            eia += 1;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    (out, up, eia)
}

fn expand_healthbars_xml(xml: &str, era_tag: &str) -> String {
    if !xml.contains(">UIHudIcon_Healthbar<") && !xml.contains(">UIHudIcon_HealthbarBackground<") {
        return xml.to_string();
    }
    let mut remove_guids = HashSet::new();
    let mut extra_instances = String::new();
    let mut extra_refs = Vec::new();
    let mut out = String::with_capacity(xml.len() + 4096);
    let mut cur_inst: Option<String> = None;

    for line in xml.lines() {
        if line.trim_start().starts_with("<instance ") {
            cur_inst = Some(format!("{line}\n"));
            if !line.trim_end().ends_with("</instance>") {
                continue;
            }
        } else if let Some(ref mut buf) = cur_inst {
            buf.push_str(line);
            buf.push('\n');
        }
        if cur_inst.is_some() {
            if line.trim_end().ends_with("</instance>") {
                let inst = cur_inst.take().unwrap();
                let target_base = if inst.contains(">UIHudIcon_Healthbar<") {
                    Some("UIHudIcon_Healthbar")
                } else if inst.contains(">UIHudIcon_HealthbarBackground<") {
                    Some("UIHudIcon_HealthbarBackground")
                } else {
                    None
                };
                if let Some(base) = target_base {
                    if let Some(gs) = inst.find("guid=\"") {
                        let r = &inst[gs + 6..];
                        if let Some(ge) = r.find('"') {
                            let orig_guid = r[..ge].to_string();
                            remove_guids.insert(orig_guid.to_ascii_lowercase());
                            for col in ["White", "Blue", "Green", "Red"] {
                                let ng = deterministic_guid(&format!("{era_tag}:{base}:{col}"));
                                let cloned = inst
                                    .replace(&orig_guid, &ng)
                                    .replace(&format!(">{base}<"), &format!(">{base}{col}<"));
                                extra_instances.push_str(&cloned);
                                extra_refs.push(ng);
                            }
                        }
                    }
                } else {
                    out.push_str(&inst);
                }
            }
            continue;
        }
        if line.trim() == "</partition>" {
            out.push_str(&extra_instances);
        }
        out.push_str(line);
        out.push('\n');
    }

    // Update <item ref="..." /> entries referencing removed generic Healthbar GUIDs
    let mut final_xml = String::with_capacity(out.len() + 512);
    let mut inserted_refs = false;
    for line in out.lines() {
        let lower = line.to_ascii_lowercase();
        if remove_guids.iter().any(|g| lower.contains(&format!("ref=\"{g}\""))) {
            if !inserted_refs {
                let indent = &line[..line.len() - line.trim_start().len()];
                for ng in &extra_refs {
                    final_xml.push_str(&format!("{indent}<item ref=\"{ng}\" />\n"));
                }
                inserted_refs = true;
            }
            continue;
        }
        final_xml.push_str(line);
        final_xml.push('\n');
    }
    final_xml
}

fn remap_bf43_icons_xml(xml: &str) -> String {
    let mut s = xml.to_string();
    for &(from, to) in BF43_ICON_REMAP {
        s = s.replace(&format!(">{from}<"), &format!(">{to}<"));
    }
    s
}

fn make_minimap_texture_xml(lv: &str, part_g: &str, inst_g: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?>\n\
         <partition guid=\"{part_g}\" primaryInstance=\"{inst_g}\" exportMode=\"All\">\n\
         \t<instance guid=\"{inst_g}\" type=\"Render.TextureAsset\" id=\"minimapTexture\">\n\
         \t\t<field name=\"Name\">Levels/{lv}/UI/minimapTexture</field>\n\
         \t\t<field name=\"MemoryReportGroup\">MemoryReportGroup_default</field>\n\
         \t\t<field name=\"OutputFormat\">TofDXT1</field>\n\
         \t\t<complex name=\"NormalMapGeneration\">\n\
         \t\t\t<field name=\"NormalMapConversionEnabled\">false</field>\n\
         \t\t\t<field name=\"MinZ\">0</field>\n\
         \t\t\t<field name=\"Scale\">1</field>\n\
         \t\t\t<field name=\"FilterKernel\">Nmf3x3</field>\n\
         \t\t\t<field name=\"HeightConversionMethod\">HcmAverageRGB</field>\n\
         \t\t\t<field name=\"AlphaResult\">ArtUnchanged</field>\n\
         \t\t\t<field name=\"Wrap\">false</field>\n\
         \t\t\t<field name=\"InvertX\">false</field>\n\
         \t\t\t<field name=\"InvertY\">true</field>\n\
         \t\t\t<field name=\"InvertZ\">false</field>\n\
         \t\t\t<field name=\"AddHeightMap\">false</field>\n\
         \t\t\t<field name=\"NormalMapSwapRGB\">false</field>\n\
         \t\t</complex>\n\
         \t\t<field name=\"Gamma\">TagDefault</field>\n\
         \t\t<field name=\"Usage\">TuDefault</field>\n\
         \t\t<field name=\"SkipMipmaps\">666</field>\n\
         \t\t<field name=\"SkipMipmapsPs3\">666</field>\n\
         \t\t<field name=\"SkipMipmapsXenon\">666</field>\n\
         \t\t<field name=\"ResourceName\" />\n\
         \t\t<field name=\"SharpenMipmapsAlpha\">false</field>\n\
         \t\t<field name=\"GenerateMipmaps\">false</field>\n\
         \t\t<field name=\"ResizeToPowerOf2\">true</field>\n\
         \t\t<field name=\"Ps3MemoryType\">PmtDefault</field>\n\
         \t</instance>\n\
         </partition>\n"
    )
}

#[allow(clippy::too_many_arguments)]
fn build_era_ingame(
    xenolith: &Path,
    base_fbrb: &Path,
    hud_fbrb: &Path,
    dst_fbrb: &Path,
    era_tag: &str,
    keycap_tint: u32,
    aliases: &[(&str, &str)],
    bfbc1_minimap: bool,
    remap_bf43: bool,
) -> Result<(), String> {
    let (ver, mut entries) = read_fbrb(base_fbrb)?;
    let (_, hud_entries) = read_fbrb(hud_fbrb)?;

    // Preserve BFBC2 hud_root.dbx XML before overlaying era HUD
    let bc2_hud_xml = if bfbc1_minimap {
        entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("menu/hud_root.dbx"))
            .map(|e| dbx_to_xml(xenolith, &e.data))
            .transpose()?
    } else {
        None
    };

    for mut he in hud_entries {
        let dst_name = rename_xenon_to_win32_str(&he.name);
        if dst_name.to_ascii_lowercase().ends_with(".dbx") {
            patch_xenon_to_win32_bytes(&mut he.data);
        }
        if matches!(
            dst_name.to_ascii_lowercase().as_str(),
            "menu/igm_sp.res"
                | "menu/igm_mp.res"
                | "menu/igm_mp_cq.res"
                | "menu/settings.res"
                | "menu/common_screens.res"
        ) {
            if let Ok(fixed) = fix_apt_stext_endian(&he.data) {
                he.data = fixed;
            }
            if let Ok(injected) = inject_apt_mouse_hittest(&he.data) {
                he.data = injected;
            }
        }
        upsert_fbrb(&mut entries, dst_name, he.ext, he.flags, he.data);
    }

    for &(src, dst) in aliases {
        for ext in [".res", ".dbx"] {
            let s_key = format!("{src}{ext}").to_ascii_lowercase();
            if let Some(found) = entries.iter().find(|e| e.name.to_ascii_lowercase() == s_key).cloned() {
                upsert_fbrb(&mut entries, format!("{dst}{ext}"), found.ext, found.flags, found.data);
            }
        }
    }

    // Upgrade BFBC1 hud_root.dbx: circular minimap + ZoomConversion + merge BFBC2 UINametags
    if bfbc1_minimap
        && let (Some(bc2_xml), Some(hr)) = (
            bc2_hud_xml,
            entries.iter_mut().find(|e| e.name.eq_ignore_ascii_case("menu/hud_root.dbx")),
        )
    {
        let mut xml = dbx_to_xml(xenolith, &hr.data)?;
        if let (Some(s1), Some(s2)) = (xml.find("<complex name=\"Minimap\">"), bc2_xml.find("<complex name=\"Minimap\">"))
            && let (Some(e1_rel), Some(e2_rel)) = (xml[s1..].find("\t\t</complex>"), bc2_xml[s2..].find("\t\t</complex>"))
        {
            let mut bc2_mm = bc2_xml[s2..s2 + e2_rel + 12].to_string();
            // Apply BFBC1 authentic Default position/size + Circular radar mask
            bc2_mm = bc2_mm
                .replacen("<complex name=\"Position\">111/609</complex>", "<complex name=\"Position\">178/583</complex>", 1)
                .replacen("<complex name=\"Size\">162/162</complex>", "<complex name=\"Size\">174/174</complex>", 1)
                .replacen("<field name=\"ShapeType\">UIMinimapShapeType_Square</field>", "<field name=\"ShapeType\">UIMinimapShapeType_Circle</field>", 1)
                .replacen("textures/ui/minimap/minimapoverlaysquare/d052db87-ead1-480a-8e70-4de2bcc138fb", "textures/ui/minimap/minimapoverlay/62512178-e7c0-4530-a548-9688173eda6c", 1);
            xml.replace_range(s1..s1 + e1_rel + 12, &bc2_mm);
        }
        hr.data = xml_to_dbx(xenolith, &xml)?;
    }

    // Healthbar 4-color cloning + BF1943 icon remapping
    if let Some(ic) = entries.iter_mut().find(|e| e.name.eq_ignore_ascii_case("menu/minimap/icons_data.dbx")) {
        let mut xml = dbx_to_xml(xenolith, &ic.data)?;
        xml = expand_healthbars_xml(&xml, era_tag);
        if remap_bf43 {
            xml = remap_bf43_icons_xml(&xml);
        }
        ic.data = xml_to_dbx(xenolith, &xml)?;
    }
    if remap_bf43
        && let Some(hr) = entries.iter_mut().find(|e| e.name.eq_ignore_ascii_case("menu/hud_root.dbx"))
    {
        let xml = remap_bf43_icons_xml(&dbx_to_xml(xenolith, &hr.data)?);
        hr.data = xml_to_dbx(xenolith, &xml)?;
    }

    // Tint BFBC2 Menu/platform_buttons.res dynamic PC keycap boxes
    if let Some(pb) = entries.iter_mut().find(|e| e.name.eq_ignore_ascii_case("menu/platform_buttons.res")) {
        pb.data = tint_platform_buttons_res(&pb.data, keycap_tint)?;
    }

    if let Some(parent) = dst_fbrb.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    write_fbrb(dst_fbrb, ver, &entries, true)?;
    println!("[era-parity] Built {} ({} entries)", dst_fbrb.display(), entries.len());
    Ok(())
}

/// Enforce authentic BFBC1 health behavior (`RegenerateHealth = 0.0f`) on `SoldierEntityData`
/// instances so Frostbite 1.5's BFBC2 auto-regeneration default does not override Auto-Injector.
fn disable_soldier_regen_xml(xml: &str) -> (String, usize) {
    if !xml.contains("GameSharedResources.SoldierEntityData") {
        return (xml.to_string(), 0);
    }
    let mut out = String::with_capacity(xml.len() + 64);
    let mut in_soldier = false;
    let mut saw_regen = false;
    let mut changes = 0usize;
    for line in xml.split_inclusive('\n') {
        if line.contains("<instance ") {
            in_soldier = line.contains("GameSharedResources.SoldierEntityData");
            saw_regen = false;
        } else if line.contains("</instance>") {
            in_soldier = false;
        }
        if in_soldier && line.contains("name=\"RegenerateHealth\"") {
            saw_regen = true;
            if !line.contains(">0</field>") {
                let indent: String = line.chars().take_while(|c| *c == '\t' || *c == ' ').collect();
                out.push_str(&format!("{indent}<field name=\"RegenerateHealth\">0</field>\n"));
                changes += 1;
                continue;
            }
        }
        out.push_str(line);
        if in_soldier
            && !saw_regen
            && line.contains("<field name=\"MaxHealth\">")
            && !xml.contains("name=\"RegenerateHealth\"")
        {
            let indent: String = line.chars().take_while(|c| *c == '\t' || *c == ' ').collect();
            out.push_str(&format!("{indent}<field name=\"RegenerateHealth\">0</field>\n"));
            saw_regen = true;
            changes += 1;
        }
    }
    (out, changes)
}

fn patch_bfbc1_levels(xenolith: &Path, opfor: &Path) -> Result<(), String> {
    let lv_root = opfor.join("modules").join("bfbc1").join("levels");
    let Ok(rd) = fs::read_dir(&lv_root) else {
        return Ok(());
    };
    let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for lvdir in dirs {
        let lv = lvdir.file_name().unwrap_or_default().to_string_lossy().to_string();
        if lv.eq_ignore_ascii_case("sp_common") || lv.eq_ignore_ascii_case("mp_common") {
            continue;
        }
        let l0 = lvdir.join("level-00.fbrb");
        let ig = lvdir.join("ingame-00.fbrb");
        if !l0.is_file() || !ig.is_file() {
            continue;
        }

        let part_g = deterministic_guid(&format!("{}:minimapTexture:part", lv.to_ascii_lowercase()));
        let inst_g = deterministic_guid(&format!("{}:minimapTexture:inst", lv.to_ascii_lowercase()));
        let ref_s = format!("levels/{}/ui/minimaptexture/{}", lv.to_ascii_lowercase(), inst_g.to_ascii_lowercase());

        // 1. ingame-00.fbrb: copy *_minimap.res -> Levels/<lv>/UI/minimapTexture.res
        let (ig_ver, mut ig_entries) = read_fbrb(&ig)?;
        let tex_key = format!("levels/{}/ui/minimaptexture.res", lv.to_ascii_lowercase());
        if !ig_entries.iter().any(|e| e.name.eq_ignore_ascii_case(&tex_key))
            && let Some(src) = ig_entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("_minimap.res")).cloned()
        {
            upsert_fbrb(&mut ig_entries, format!("Levels/{lv}/UI/minimapTexture.res"), src.ext, src.flags, src.data);
            write_fbrb(&ig, ig_ver, &ig_entries, true)?;
        }

        // 2. level-00.fbrb: MinimapTexture DBX + LevelData/Description refs + dual KBM/Gamepad inputs
        let (l0_ver, mut l0_entries) = read_fbrb(&l0)?;
        let mut changed = false;
        let dbx_key = format!("levels/{}/ui/minimaptexture.dbx", lv.to_ascii_lowercase());
        if !l0_entries.iter().any(|e| e.name.eq_ignore_ascii_case(&dbx_key)) {
            let mm_bin = xml_to_dbx(xenolith, &make_minimap_texture_xml(&lv, &part_g, &inst_g))?;
            upsert_fbrb(&mut l0_entries, format!("Levels/{lv}/UI/minimapTexture.dbx"), "<non-resource>".into(), 0, mm_bin);
            changed = true;
        }

        let lv_key = format!("levels/{}.dbx", lv.to_ascii_lowercase());
        let desc_key = format!("levels/{}_description.dbx", lv.to_ascii_lowercase());
        let mut up_total = 0usize;
        let mut regen_total = 0usize;
        for e in &mut l0_entries {
            let nl = e.name.to_ascii_lowercase();
            if !nl.ends_with(".dbx") {
                continue;
            }
            let has_input = e.data.windows(15).any(|w| w == b"InputActionMaps")
                || e.data.windows(26).any(|w| w == b"EiaCycleSecondaryInventory");
            let is_soldier = nl.starts_with("objects/soldiers/")
                && e.data.windows(17).any(|w| w == b"SoldierEntityData");
            if nl == desc_key || nl == lv_key || has_input || is_soldier {
                let mut xml = dbx_to_xml(xenolith, &e.data)?;
                let mut e_mod = false;
                if nl == desc_key && !xml.to_ascii_lowercase().contains(&ref_s) {
                    if let Some(pos) = xml.find("name=\"MinimapTexture\"")
                        && let Some(rs) = xml[..pos].rfind("ref=\"")
                    {
                        let re_end = rs + 5 + xml[rs + 5..].find('"').unwrap_or(0);
                        xml.replace_range(rs + 5..re_end, &ref_s);
                        e_mod = true;
                    }
                } else if nl == lv_key
                    && !xml.to_ascii_lowercase().contains(&ref_s)
                    && let Some(pos) = xml.find("<complex name=\"MinimapCameraOffset\">")
                {
                    let ins = format!("\t\t\t<field ref=\"{ref_s}\" name=\"MinimapTexture\" />\n");
                    xml.insert_str(pos, &ins);
                    e_mod = true;
                }
                if has_input {
                    let (uxml, u, eia) = upgrade_input_xml(&xml);
                    if u > 0 || eia > 0 {
                        xml = uxml;
                        up_total += u;
                        e_mod = true;
                    }
                }
                if is_soldier {
                    let (rxml, rc) = disable_soldier_regen_xml(&xml);
                    if rc > 0 {
                        xml = rxml;
                        regen_total += rc;
                        e_mod = true;
                    }
                }
                if e_mod {
                    e.data = xml_to_dbx(xenolith, &xml)?;
                    changed = true;
                }
            }
        }
        if changed {
            write_fbrb(&l0, l0_ver, &l0_entries, true)?;
            println!(
                "[era-parity] Patched {lv} (minimap + {up_total} dual KBM/Gamepad inputs + {regen_total} no-regen soldiers)"
            );
        } else {
            println!("[era-parity] {lv} already up to date");
        }
    }
    Ok(())
}

/// Patch BFBC1 `mainmenu-00.fbrb` in-place: fix StaticText big-endian records, inject Frame-0
/// ActionScript mouse hover/click hit-test into `main_root`/`main_nav`/`settings`, and tint
/// `platform_buttons.res` PC keycaps if present.
fn patch_bfbc1_mainmenu(mainmenu_fbrb: &Path) -> Result<(), String> {
    if !mainmenu_fbrb.is_file() {
        return Ok(());
    }
    let (ver, mut entries) = read_fbrb(mainmenu_fbrb)?;
    let before_len = entries.len();
    entries.retain(|e| {
        !matches!(
            e.ext.as_str(),
            "XenonVertexShader" | "XenonPixelShader" | "ShaderDatabase"
        )
    });
    let stripped_xenon = before_len - entries.len();
    let mut modified = stripped_xenon;
    for e in &mut entries {
        let nl = e.name.to_ascii_lowercase();
        if matches!(nl.as_str(), "menu/main_root.res" | "menu/main_nav.res" | "menu/settings.res") {
            let orig = e.data.clone();
            if let Ok(fixed) = fix_apt_stext_endian(&e.data) {
                e.data = fixed;
            }
            if let Ok(injected) = inject_apt_mouse_hittest(&e.data) {
                e.data = injected;
            }
            if e.data != orig {
                modified += 1;
            }
        } else if nl == "menu/platform_buttons.res" {
            let orig = e.data.clone();
            if let Ok(tinted) = tint_platform_buttons_res(&e.data, 0xFFE8_C868) {
                e.data = tinted;
                if e.data != orig {
                    modified += 1;
                }
            }
        }
    }
    if modified > 0 {
        write_fbrb(mainmenu_fbrb, ver, &entries, true)?;
        println!(
            "[era-parity] Patched {} ({modified} APT movies with StaticText + Frame-0 mouse hit-test)",
            mainmenu_fbrb.display()
        );
    } else {
        println!("[era-parity] {} already up to date", mainmenu_fbrb.display());
    }
    Ok(())
}

pub fn cmd_era_ui_input_parity(default_root: &Path, args: &[String]) -> Result<(), String> {
    let opfor = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let era = flag_val(args, "--era")
        .map(|p| p.to_string_lossy().to_ascii_lowercase())
        .ok_or("usage: opfor-cli era-ui-input-parity --era <bfbc1|bf1943> --opfor <dir> [--check-only]")?;
    let check_only = args.iter().any(|a| a == "--check-only");
    let xenolith = opfor.join("utility").join("xenolith.exe");
    if !xenolith.is_file() {
        return Err(format!("missing {}", xenolith.display()));
    }
    if check_only {
        println!("[CHECK] pure-rust era-ui-input-parity era={era} opfor={} xenolith=OK", opfor.display());
        return Ok(());
    }

    let bc2_dist = opfor.join("engine").join("Dist").join("win32");
    match era.as_str() {
        "bfbc1" => {
            let mod_dir = opfor.join("modules").join("bfbc1");
            let hud_sp = mod_dir.join("menu").join("hud").join("hud_singleplayer-00.fbrb");
            let hud_mp = mod_dir.join("menu").join("hud").join("hud_multiplayer-00.fbrb");
            if hud_sp.is_file() {
                build_era_ingame(
                    &xenolith,
                    &bc2_dist.join("levels").join("sp_common").join("ingame-00.fbrb"),
                    &hud_sp,
                    &mod_dir.join("levels").join("sp_common").join("ingame-00.fbrb"),
                    "bfbc1_sp_common",
                    0xFFE8_C868,
                    &[],
                    true,
                    false,
                )?;
            }
            if hud_mp.is_file() {
                build_era_ingame(
                    &xenolith,
                    &bc2_dist.join("levels").join("mp_common").join("ingame-00.fbrb"),
                    &hud_mp,
                    &mod_dir.join("levels").join("mp_common").join("ingame-00.fbrb"),
                    "bfbc1_mp_common",
                    0xFFE8_C868,
                    &[],
                    true,
                    false,
                )?;
            }
            patch_bfbc1_mainmenu(&mod_dir.join("menu").join("main").join("mainmenu-00.fbrb"))?;
            patch_bfbc1_levels(&xenolith, &opfor)?;
        }
        "bf1943" => {
            let mod_dir = opfor.join("modules").join("bf1943");
            let hud_cq = mod_dir.join("menu").join("hud").join("hud_multiplayer_cq-00.fbrb");
            if hud_cq.is_file() {
                for common in ["mp_common", "ww2_common"] {
                    build_era_ingame(
                        &xenolith,
                        &bc2_dist.join("levels").join("mp_common").join("ingame-00.fbrb"),
                        &hud_cq,
                        &mod_dir.join("levels").join(common).join("ingame-00.fbrb"),
                        &format!("bf1943_{common}"),
                        0xFFF2_E6C8,
                        &[("Menu/hud_root_cq", "Menu/hud_root"), ("Menu/igm_mp_cq", "Menu/igm_mp")],
                        false,
                        true,
                    )?;
                }
            }
        }
        other => return Err(format!("unknown --era `{other}` (expected bfbc1 or bf1943)")),
    }
    println!("[era-parity] Pure-Rust era-ui-input-parity ({era}) complete!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opfor_apt::bigf::BigfEntry;

    #[test]
    fn bigf_roundtrip_preserves_members() {
        let orig = vec![
            BigfEntry {
                name: "test.apt".to_string(),
                data: b"Apt Data:74\x1A\0\0\0\0".to_vec(),
            },
            BigfEntry {
                name: "test.const".to_string(),
                data: b"Apt constant file\x1A\0\0".to_vec(),
            },
        ];
        let packed = bigf::write(&orig);
        let unpacked = bigf::parse(&packed).unwrap();
        assert_eq!(unpacked, orig);
    }

    #[test]
    fn upgrades_bfbc1_input_xml_to_dual_kbm_gamepad() {
        let xml = "\t\t<field ref=\"Objects/Vehicles/Common/Input/HeliVehicleInput/1234\" name=\"InputActionMaps\" />\n\t\t<field name=\"Action\">EiaCycleSecondaryInventory</field>\n";
        let (out, up, eia) = upgrade_input_xml(xml);
        assert_eq!(up, 1);
        assert_eq!(eia, 1);
        assert!(out.contains(AIR_CONCEPT));
        assert!(out.contains(AIR_MAPPING));
        assert!(out.contains("EiaToggleWeapon"));
        assert!(!out.contains("InputActionMaps"));
    }

    #[test]
    fn expands_healthbars_and_remaps_bf43_icons() {
        let xml = "<partition>\n<instance guid=\"AAA\" type=\"UI.Icon\"><field name=\"Icon\">UIHudIcon_Healthbar</field></instance>\n<instance guid=\"BBB\" type=\"UI.Icon\"><field name=\"Icon\">UIHudIcon_BunkerEnemy</field></instance>\n</partition>\n";
        let e = expand_healthbars_xml(xml, "bf1943_mp_common");
        let r = remap_bf43_icons_xml(&e);
        assert!(r.contains("UIHudIcon_HealthbarWhite"));
        assert!(r.contains("UIHudIcon_HealthbarBlue"));
        assert!(r.contains("UIHudIcon_HealthbarGreen"));
        assert!(r.contains("UIHudIcon_HealthbarRed"));
        assert!(r.contains("UIHudIcon_UAVEnemy"));
        assert!(!r.contains(">UIHudIcon_Healthbar<"));
    }

    #[test]
    fn disables_bfbc1_soldier_auto_regen() {
        let xml = "<partition>\n\t<instance guid=\"AAA\" type=\"GameSharedResources.SoldierEntityData\" id=\"Specialist\">\n\t\t<field name=\"MaxHealth\">1120403456</field>\n\t</instance>\n</partition>\n";
        let (out, c1) = disable_soldier_regen_xml(xml);
        assert_eq!(c1, 1);
        assert!(out.contains("<field name=\"RegenerateHealth\">0</field>"));
        // Idempotent on second run
        let (_, c2) = disable_soldier_regen_xml(&out);
        assert_eq!(c2, 0);
    }
}
