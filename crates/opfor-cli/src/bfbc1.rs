//! Pure-Rust BFBC1 Singleplayer & UI Parity Toolchain for Project OPFOR:
//! - `bfbc1-ui-parity`: Era-isolated BFBC1 fonts (`modules/bfbc1/menu/shared/fonts_us-00.fbrb`),
//!   additive BFBC1 English localization merge (`cache/overlay-00.fbrb`), and per-level BFBC1 SP
//!   loading music (`modules/bfbc1/levels/sp_*/loader-00.fbrb`).
//! - `bfbc1-movies`: Read-only Xbox 360 XDVDFS ISO extraction + 64B -> 72B Frostbite 1.5
//!   `RES\x02BinkTexture` header upgrade for all 9 BFBC1 campaign & intro/credits movies.
//! - `bfbc1-navmeshes`: Read-only Xbox 360 ISO extraction + `xenolith upgrade-hpd` conversion
//!   for all 13 BFBC1 Kynogon HPD v1 -> Frostbite 1.5 HPD v2 navmeshes.
#![deny(unsafe_code)]

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use x360_containers::xiso;
use x360_frostbite::dbx;
use x360_frostbite::fbrb::{FbrbArchive, FbrbEntry as XbFbrbEntry};

const BFBC1_CAMPAIGN_MOVIES: &[&str] = &[
    "output/xenon/movies/greenacres.res",
    "output/xenon/movies/zabograd.res",
    "output/xenon/movies/crossroads.res",
    "output/xenon/movies/parcourse.res",
    "output/xenon/movies/hotzone.res",
    "output/xenon/movies/pow.res",
    "output/xenon/movies/ghosttown.res",
    "output/xenon/movies/intro.res",
    "output/xenon/movies/credits.res",
];

const BFBC1_MP_LEVELS: &[&str] = &[
    "mp_beachhead_day1",
    "mp_beachhead_day2",
    "mp_harbour_day1",
    "mp_harbour_day2",
    "mp_ignition_day1",
    "mp_ignition_day2",
    "mp_monastery_day1",
    "mp_monastery_day2",
];

const BFBC1_SP_LEVELS: &[&str] = &[
    "sp_01_greenacres",
    "sp_02_traitor_of_zabograd",
    "sp_03_crossroads",
    "sp_05_par_for_the_course",
    "sp_06_hot_zone",
    "sp_07_pow",
    "sp_08_ghost_town",
];

const BFBC1_SP_MOVIE_MAP: &[(&str, &str)] = &[
    ("sp_01_greenacres", "greenacres.res"),
    ("sp_02_traitor_of_zabograd", "zabograd.res"),
    ("sp_03_crossroads", "crossroads.res"),
    ("sp_05_par_for_the_course", "parcourse.res"),
    ("sp_06_hot_zone", "hotzone.res"),
    ("sp_07_pow", "pow.res"),
    ("sp_08_ghost_town", "ghosttown.res"),
];

// ---------------------------------------------------------------------------
// Read-only Xbox 360 XDVDFS ISO reader (delegating to `x360-containers::xiso`)
// ---------------------------------------------------------------------------

pub struct IsoIndex {
    pub files: BTreeMap<String, (u64, u64)>,
}

pub fn index_iso(iso_path: &Path) -> Result<IsoIndex, String> {
    let mut fp = File::open(iso_path).map_err(|e| format!("open ISO {}: {e}", iso_path.display()))?;
    let base = xiso::find_partition(&mut fp).map_err(|e| {
        format!(
            "not a recognized Xbox 360 XDVDFS ISO ({e}): {}",
            iso_path.display()
        )
    })?;
    let (root_sector, root_size) = xiso::root(&mut fp, base)
        .map_err(|e| format!("read XDVDFS root: {e}"))?
        .ok_or_else(|| format!("truncated XDVDFS root descriptor: {}", iso_path.display()))?;
    let (entries, _) = xiso::walk(&mut fp, base, root_sector, root_size, b"")
        .map_err(|e| format!("walk XDVDFS directory tree: {e}"))?;
    let mut files = BTreeMap::new();
    for (raw_path, sector, size) in entries {
        let rel = String::from_utf8_lossy(&raw_path).to_ascii_lowercase();
        files.insert(rel, (base + u64::from(sector) * xiso::SECTOR, u64::from(size)));
    }
    Ok(IsoIndex { files })
}

pub(crate) fn copy_iso_range(fp: &mut File, offset: u64, size: u64, out: &mut File) -> Result<(), String> {
    fp.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("ISO seek: {e}"))?;
    let mut remaining = size;
    let mut buf = vec![0u8; 4 * 1024 * 1024];
    while remaining > 0 {
        let want = (remaining as usize).min(buf.len());
        fp.read_exact(&mut buf[..want])
            .map_err(|e| format!("ISO read: {e}"))?;
        out.write_all(&buf[..want])
            .map_err(|e| format!("write: {e}"))?;
        remaining -= want as u64;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Frostbite FbRB v2 archive reader & writer (delegating to `x360-frostbite::fbrb`)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct FbrbEntry {
    pub(crate) name: String,
    pub(crate) ext: String,
    pub(crate) flags: u32,
    pub(crate) data: Vec<u8>,
}

pub(crate) fn read_fbrb(path: &Path) -> Result<(u32, Vec<FbrbEntry>), String> {
    let raw = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let arc = FbrbArchive::parse(&raw).map_err(|e| format!("{}: {e}", path.display()))?;
    let entries = arc
        .entries
        .into_iter()
        .map(|e| FbrbEntry {
            name: e.path,
            ext: e.ext,
            flags: e.flags,
            data: e.data,
        })
        .collect();
    Ok((arc.version, entries))
}

pub(crate) fn write_fbrb(path: &Path, version: u32, entries: &[FbrbEntry], compress_payload: bool) -> Result<(), String> {
    let arc = FbrbArchive {
        version,
        entries: entries
            .iter()
            .map(|e| XbFbrbEntry {
                path: e.name.clone(),
                ext: e.ext.clone(),
                flags: e.flags,
                data: e.data.clone(),
            })
            .collect(),
    };
    let bytes = arc
        .to_bytes(compress_payload)
        .map_err(|e| format!("encode {}: {e}", path.display()))?;
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).map_err(|e| format!("mkdir {}: {e}", p.display()))?;
    }
    fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Frostbite Localization binary parser & serializer
// ---------------------------------------------------------------------------

type LocBinTable = (Vec<u8>, BTreeMap<u32, Vec<u8>>);

fn parse_loc_bin(b: &[u8]) -> Result<LocBinTable, String> {
    if b.len() < 20 {
        return Err("localization binary too small".into());
    }
    let magic = u32::from_le_bytes(b[0..4].try_into().unwrap());
    if magic != 0x0003_9000 {
        return Err(format!("unexpected localization magic {magic:#010x}"));
    }
    let count = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let table_off = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let str_off = u32::from_le_bytes(b[16..20].try_into().unwrap()) as usize;
    let base = 8usize;
    let tag = b[20..base + table_off].to_vec();

    let mut entries = BTreeMap::new();
    for i in 0..count {
        let pos = base + table_off + i * 8;
        let h = u32::from_le_bytes(b[pos..pos + 4].try_into().unwrap());
        let off = u32::from_le_bytes(b[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let s_start = base + str_off + off;
        let s_end = b[s_start..]
            .iter()
            .position(|&c| c == 0)
            .map(|n| s_start + n)
            .unwrap_or(b.len());
        entries.insert(h, b[s_start..s_end].to_vec());
    }
    Ok((tag, entries))
}

fn build_loc_bin(tag: &[u8], entries: &BTreeMap<u32, Vec<u8>>) -> Vec<u8> {
    let count = entries.len() as u32;
    let table_off = (12 + tag.len()) as u32;
    let str_off = table_off + count * 8;

    let mut table = Vec::with_capacity(entries.len() * 8);
    let mut str_blob = Vec::new();
    for (&h, text) in entries {
        let off = str_blob.len() as u32;
        table.extend_from_slice(&h.to_le_bytes());
        table.extend_from_slice(&off.to_le_bytes());
        str_blob.extend_from_slice(text);
        str_blob.push(0);
    }

    let mut body = Vec::with_capacity(12 + tag.len() + table.len() + str_blob.len());
    body.extend_from_slice(&count.to_le_bytes());
    body.extend_from_slice(&table_off.to_le_bytes());
    body.extend_from_slice(&str_off.to_le_bytes());
    body.extend_from_slice(tag);
    body.extend_from_slice(&table);
    body.extend_from_slice(&str_blob);

    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&0x0003_9000u32.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

// ---------------------------------------------------------------------------
// CLI flag helper
// ---------------------------------------------------------------------------

pub(crate) fn flag_val(args: &[String], flag: &str) -> Option<PathBuf> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| PathBuf::from(&w[1]))
}

// ---------------------------------------------------------------------------
// Command 1: `bfbc1-ui-parity`
// ---------------------------------------------------------------------------

pub fn cmd_bfbc1_ui_parity(default_root: &Path, args: &[String]) -> Result<(), String> {
    let opfor = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let bfbc2 = flag_val(args, "--bfbc2")
        .unwrap_or_else(|| PathBuf::from(r"C:\Games\Battlefield Bad Company 2"));
    let iso = flag_val(args, "--iso");

    let xenolith = opfor.join("utility").join("xenolith.exe");
    let bfbc1_overlay_x360 = opfor
        .join("work")
        .join("bfbc1")
        .join("extracted")
        .join("dist")
        .join("xenon")
        .join("build_overlay-00.fbrb");
    let bc2_pkg_fonts = bfbc2
        .join("Package")
        .join("menu")
        .join("shared")
        .join("fonts_us-00.fbrb");
    let out_bfbc1_fonts = opfor
        .join("modules")
        .join("bfbc1")
        .join("menu")
        .join("shared")
        .join("fonts_us-00.fbrb");
    let cache_overlay = opfor.join("cache").join("overlay-00.fbrb");

    // Extract build_overlay-00.fbrb & HUD bundles from ISO if not already unpacked
    if let Some(ref iso_path) = iso
        && iso_path.is_file()
    {
        let idx = index_iso(iso_path)?;
        let mut fp = File::open(iso_path).map_err(|e| e.to_string())?;
        if !bfbc1_overlay_x360.is_file()
            && let Some(&(off, sz)) = idx.files.get("dist/xenon/build_overlay-00.fbrb")
        {
            if let Some(parent) = bfbc1_overlay_x360.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = File::create(&bfbc1_overlay_x360).map_err(|e| e.to_string())?;
            copy_iso_range(&mut fp, off, sz, &mut out)?;
            println!("[fonts] Extracted dist/xenon/build_overlay-00.fbrb from ISO ({sz} B)");
        }
        // Also extract & port BFBC1 SP + MP HUD bundles (`hud_singleplayer-00.fbrb`, `hud_multiplayer-00.fbrb`)
        for (iso_rel, mod_rel) in [
            ("dist/xenon/menu/hud/hud_singleplayer-00.fbrb", "menu/hud/hud_singleplayer-00.fbrb"),
            ("dist/xenon/menu/hud/hud_multiplayer-00.fbrb", "menu/hud/hud_multiplayer-00.fbrb"),
        ] {
            let dst_mod = opfor.join("modules").join("bfbc1").join(mod_rel.replace('/', "\\"));
            if !dst_mod.is_file()
                && let Some(&(off, sz)) = idx.files.get(iso_rel)
            {
                let tmp_raw = std::env::temp_dir().join(format!(
                    "opfor_bc1_hud_raw_{}_{}.fbrb",
                    std::process::id(),
                    dst_mod.file_name().unwrap_or_default().to_string_lossy()
                ));
                let mut out_f = File::create(&tmp_raw).map_err(|e| e.to_string())?;
                copy_iso_range(&mut fp, off, sz, &mut out_f)?;
                drop(out_f);
                if let Some(parent) = dst_mod.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let res = Command::new(&xenolith)
                    .args([
                        "port-fbrb",
                        "--bfbc1",
                        &tmp_raw.display().to_string(),
                        &dst_mod.display().to_string(),
                    ])
                    .output()
                    .map_err(|e| format!("run xenolith port-fbrb on {iso_rel}: {e}"))?;
                let _ = fs::remove_file(&tmp_raw);
                if !res.status.success() {
                    return Err(format!(
                        "xenolith port-fbrb failed on {iso_rel}: {}",
                        String::from_utf8_lossy(&res.stderr)
                    ));
                }
                println!("[hud] Extracted & ported {iso_rel} -> {}", dst_mod.display());
            }
        }
    }

    // 1. Port BFBC1 build_overlay-00.fbrb to PC via xenolith
    let tmp_ported = std::env::temp_dir().join(format!(
        "opfor_bfbc1_overlay_pc_{}.fbrb",
        std::process::id()
    ));
    let out = Command::new(&xenolith)
        .args([
            "port-fbrb",
            "--bfbc1",
            &bfbc1_overlay_x360.display().to_string(),
            &tmp_ported.display().to_string(),
        ])
        .output()
        .map_err(|e| format!("run xenolith port-fbrb: {e}"))?;
    if !out.status.success() {
        let _ = fs::remove_file(&tmp_ported);
        return Err(format!(
            "xenolith port-fbrb failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    println!("[fonts] {}", String::from_utf8_lossy(&out.stdout).trim());

    // 2. Build modules/bfbc1/menu/shared/fonts_us-00.fbrb
    let (ver_bc2, bc2_entries) = read_fbrb(&bc2_pkg_fonts)?;
    let (_, bfbc1_entries) = read_fbrb(&tmp_ported)?;
    let _ = fs::remove_file(&tmp_ported);

    let bc2_count = bc2_entries.len();
    let mut merged_fonts = bc2_entries;
    let mut existing_lower: HashSet<String> = merged_fonts
        .iter()
        .map(|e| e.name.to_ascii_lowercase())
        .collect();
    let mut added_fonts = 0usize;
    let mut bfbc1_loc_bin: Option<Vec<u8>> = None;

    for e in bfbc1_entries {
        let low = e.name.to_ascii_lowercase();
        if low.ends_with("localization/bin/language_english_normal.bin") {
            bfbc1_loc_bin = Some(e.data);
            continue;
        }
        let clean = if low.starts_with("output/xenon/") {
            e.name["output/xenon/".len()..].to_owned()
        } else {
            e.name
        };
        let clow = clean.to_ascii_lowercase();
        if (clow.starts_with("fonts/us/")
            || clow == "fonts/us_fonts.dbx"
            || clow == "fonts/font_pattern.dbx"
            || clow == "fonts/font_pattern.res")
            && existing_lower.insert(clow)
        {
            merged_fonts.push(FbrbEntry {
                name: clean,
                ext: e.ext,
                flags: e.flags,
                data: e.data,
            });
            added_fonts += 1;
        }
    }

    write_fbrb(&out_bfbc1_fonts, ver_bc2, &merged_fonts, true)?;
    println!(
        "[fonts] Wrote {} ({} entries: {} BC2 + {} BFBC1)",
        out_bfbc1_fonts.display(),
        merged_fonts.len(),
        bc2_count,
        added_fonts
    );

    // 3. Additive merge of BFBC1 English localization strings into cache/overlay-00.fbrb
    let bfbc1_loc = bfbc1_loc_bin.ok_or("BFBC1 language_english_normal.bin not found")?;
    let (_, bfbc1_strings) = parse_loc_bin(&bfbc1_loc)?;
    let (ver_ov, mut ov_entries) = read_fbrb(&cache_overlay)?;
    let mut merged_count = 0usize;
    let mut total_count = 0usize;
    for e in &mut ov_entries {
        if e.name.eq_ignore_ascii_case("localization/bin/language_english_normal.bin") {
            let (tag, mut cur) = parse_loc_bin(&e.data)?;
            let before = cur.len();
            for (h, s) in &bfbc1_strings {
                cur.entry(*h).or_insert_with(|| s.clone());
            }
            merged_count = cur.len() - before;
            total_count = cur.len();
            e.data = build_loc_bin(&tag, &cur);
        }
    }
    write_fbrb(&cache_overlay, ver_ov, &ov_entries, false)?;
    println!(
        "[loc] Updated {}: +{merged_count} BFBC1 English strings (total {total_count})",
        cache_overlay.display()
    );

    // 4. Wire per-level BFBC1 SP loading music into loader-00.fbrb
    let mut swapped = 0usize;
    for lvl in BFBC1_SP_LEVELS {
        let ldir = opfor.join("modules").join("bfbc1").join("levels").join(lvl);
        let ui_fbrb = ldir.join("ui").join("singleplayer-00.fbrb");
        let loader_fbrb = ldir.join("loader-00.fbrb");
        if !(ui_fbrb.is_file() && loader_fbrb.is_file()) {
            continue;
        }
        let (_, ui_entries) = read_fbrb(&ui_fbrb)?;
        let Some(wave) = ui_entries
            .into_iter()
            .find(|e| e.ext == "Wave" && e.name.to_ascii_lowercase().contains("loading"))
        else {
            continue;
        };
        let (ver_ld, mut ld_entries) = read_fbrb(&loader_fbrb)?;
        let mut did_swap = false;
        for e in &mut ld_entries {
            if e.ext == "Wave" && e.name.to_ascii_lowercase().contains("loading_music") {
                e.data = wave.data.clone();
                did_swap = true;
            }
        }
        if did_swap {
            write_fbrb(&loader_fbrb, ver_ld, &ld_entries, true)?;
            swapped += 1;
            println!(
                "[loader] {lvl}: wired {} ({} B) -> loader-00.fbrb",
                wave.name,
                wave.data.len()
            );
        }
    }
    println!(
        "[loader] Updated {swapped}/{} BFBC1 SP loader-00.fbrb bundles",
        BFBC1_SP_LEVELS.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Command 2: `bfbc1-movies`
// ---------------------------------------------------------------------------

pub fn cmd_bfbc1_movies(default_root: &Path, args: &[String]) -> Result<(), String> {
    let opfor = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let iso = flag_val(args, "--iso")
        .ok_or("usage: opfor-cli bfbc1-movies --iso <bfbc1.iso> [--opfor <dir>]")?;
    let out_dir = opfor.join("modules").join("bfbc1").join("movies");
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let mut fb15_hdr = [0u8; 72];
    fb15_hdr[..15].copy_from_slice(b"RES\x02BinkTexture");

    let idx = index_iso(&iso)?;
    let mut fp = File::open(&iso).map_err(|e| format!("open ISO: {e}"))?;
    let mut extracted = 0usize;

    for rel in BFBC1_CAMPAIGN_MOVIES {
        let Some(&(off, sz)) = idx.files.get(*rel) else {
            println!("[movies] MISSING on ISO: {rel}");
            continue;
        };
        let name = Path::new(rel).file_name().unwrap();
        let dest = out_dir.join(name);
        if dest.is_file()
            && fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) == sz + 8
        {
            println!("[movies] SKIP (already up-to-date): {}", name.to_string_lossy());
            extracted += 1;
            continue;
        }
        fp.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
        let mut hdr64 = [0u8; 64];
        fp.read_exact(&mut hdr64).map_err(|e| e.to_string())?;
        if &hdr64[..15] != b"RES\x02BinkTexture" {
            return Err(format!("unexpected header in {rel}"));
        }
        let mut peek = [0u8; 4];
        fp.read_exact(&mut peek).map_err(|e| e.to_string())?;
        if &peek != b"BIKi" && &peek != b"KB2i" {
            return Err(format!("expected BIKi after 64B header in {rel}"));
        }
        let mut out = File::create(&dest).map_err(|e| e.to_string())?;
        out.write_all(&fb15_hdr).map_err(|e| e.to_string())?;
        out.write_all(&peek).map_err(|e| e.to_string())?;
        copy_iso_range(&mut fp, off + 68, sz - 68, &mut out)?;
        println!(
            "[movies] Extracted + upgraded header (64B->72B): {} ({} KB)",
            name.to_string_lossy(),
            (sz + 8) / 1024
        );
        extracted += 1;
    }
    println!(
        "[movies] Done: {extracted}/{} movies ready in {}",
        BFBC1_CAMPAIGN_MOVIES.len(),
        out_dir.display()
    );
    wire_bfbc1_sp_loader_movies(&opfor)?;
    Ok(())
}

/// Replace the BFBC2 donor `BinkMemory` movie inside each BFBC1 SP level's `loader-00.fbrb`
/// with the raw `BIKi` stream from that mission's native BFBC1 movie (`modules/bfbc1/movies/*.res`).
pub fn wire_bfbc1_sp_loader_movies(opfor: &Path) -> Result<(), String> {
    let movies_dir = opfor.join("modules").join("bfbc1").join("movies");
    let mut updated = 0usize;
    for &(lv, movie_file) in BFBC1_SP_MOVIE_MAP {
        let movie_path = movies_dir.join(movie_file);
        let loader_path = opfor
            .join("modules")
            .join("bfbc1")
            .join("levels")
            .join(lv)
            .join("loader-00.fbrb");
        if !(movie_path.is_file() && loader_path.is_file()) {
            continue;
        }
        let movie_bytes = fs::read(&movie_path).map_err(|e| format!("read {}: {e}", movie_path.display()))?;
        if movie_bytes.len() <= 76 || &movie_bytes[..15] != b"RES\x02BinkTexture" {
            continue;
        }
        let raw_bink = &movie_bytes[72..];
        if &raw_bink[..4] != b"BIKi" && &raw_bink[..4] != b"KB2i" {
            continue;
        }
        let (ver, mut entries) = read_fbrb(&loader_path)?;
        let mut changed = false;
        for e in &mut entries {
            if e.ext == "BinkMemory"
                && (e.data.len() != raw_bink.len() || e.data.get(..16) != raw_bink.get(..16))
            {
                e.data = raw_bink.to_vec();
                changed = true;
            }
        }
        if changed {
            write_fbrb(&loader_path, ver, &entries, true)?;
            updated += 1;
            println!(
                "[loader] {lv}: embedded native BFBC1 movie {movie_file} ({} KB) into loader-00.fbrb",
                raw_bink.len() / 1024
            );
        }
    }
    println!("[loader] Native BFBC1 SP loading movies updated: {updated}/{}", BFBC1_SP_MOVIE_MAP.len());
    Ok(())
}

fn extract_loading_screens_block(xml: &str) -> Option<String> {
    let start = xml.find("<array name=\"LoadingScreens\">")?;
    let rest = &xml[start..];
    let end_rel = rest.find("</array>")?;
    Some(rest[..end_rel + "</array>".len()].to_owned())
}

fn patch_mp_description_loading_screens(xml: &str, loading_screens_block: &str) -> String {
    let mut out = xml.to_owned();
    // Replace empty or existing LoadingScreens array with BFBC1's native LoadingScreens block
    if let Some(s) = out.find("<array name=\"LoadingScreens\"") {
        if let Some(rel_sc) = out[s..].find("/>")
            && out[s..s + rel_sc].find('>').is_none()
        {
            out.replace_range(s..s + rel_sc + 2, loading_screens_block);
        } else if let Some(rel_end) = out[s..].find("</array>") {
            out.replace_range(s..s + rel_end + "</array>".len(), loading_screens_block);
        }
    }
    // Clear donor LoadingMovie ref -> null so Frostbite displays LoadingScreens textures
    if let Some(end_idx) = out.find("\" name=\"LoadingMovie\"")
        && let Some(start_idx) = out[..end_idx].rfind("ref=\"")
    {
        out.replace_range(start_idx + 5..end_idx, "null");
    }
    out
}

/// Replace BFBC2's donor `MP01_CQ.res` BinkMovie in all 8 BFBC1 MP `loader-00.fbrb` bundles
/// with each map's native BFBC1 static loading textures (`MP*_b`, `MP*_m`, `MP*_map`, `MP*_map2`)
/// from `ui/multiplayer-00.fbrb`, and update `LoadingScreens` / clear `LoadingMovie` in
/// `cache/startup-00.fbrb` and each level's `level-00.fbrb` description DBX.
pub fn wire_bfbc1_mp_loader_screens(opfor: &Path) -> Result<(), String> {
    let startup_path = opfor.join("cache").join("startup-00.fbrb");

    let dbx_to_xml = |bytes: &[u8]| -> Result<String, String> {
        dbx::parse(bytes).map(|d| d.to_xml()).map_err(|e| format!("dbx-to-xml: {e}"))
    };
    let xml_to_dbx = |xml: &str| -> Result<Vec<u8>, String> {
        dbx::from_xml(xml).map(|d| d.to_bytes()).map_err(|e| format!("xml-to-dbx: {e}"))
    };

    let mut startup_loaded = if startup_path.is_file() {
        Some(read_fbrb(&startup_path)?)
    } else {
        None
    };
    let mut startup_dirty = false;
    let mut updated_loaders = 0usize;

    for &lv in BFBC1_MP_LEVELS {
        let lv_dir = opfor.join("modules").join("bfbc1").join("levels").join(lv);
        let ui_path = lv_dir.join("ui").join("multiplayer-00.fbrb");
        let loader_path = lv_dir.join("loader-00.fbrb");
        if !(ui_path.is_file() && loader_path.is_file()) {
            continue;
        }
        let (_, ui_entries) = read_fbrb(&ui_path)?;
        let ui_prefix = format!("levels/{lv}/ui/");
        let desc_key = format!("levels/{lv}_description.dbx");

        // 1. Collect native BFBC1 loading screen textures + their .dbx descriptors from ui/multiplayer-00.fbrb
        let screen_entries: Vec<FbrbEntry> = ui_entries
            .iter()
            .filter(|e| {
                let nl = e.name.to_ascii_lowercase();
                nl.starts_with(&ui_prefix) && (e.ext == "ITexture" || nl.ends_with(".dbx"))
            })
            .cloned()
            .collect();

        // Extract BFBC1's native <array name="LoadingScreens">...</array> block
        let screens_block = ui_entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&desc_key))
            .and_then(|e| dbx_to_xml(&e.data).ok())
            .and_then(|xml| extract_loading_screens_block(&xml));

        // 2. Rebuild loader-00.fbrb: strip BFBC2 donor BinkMemory, keep native Wave + add BFBC1 screen textures
        let (lver, old_loader) = read_fbrb(&loader_path)?;
        let mut new_loader: Vec<FbrbEntry> = old_loader
            .into_iter()
            .filter(|e| {
                let nl = e.name.to_ascii_lowercase();
                e.ext != "BinkMemory" && !nl.starts_with(&ui_prefix)
            })
            .collect();
        let added_screens = screen_entries.len();
        new_loader.extend(screen_entries);
        write_fbrb(&loader_path, lver, &new_loader, true)?;
        updated_loaders += 1;

        // 3. Patch LoadingScreens & clear LoadingMovie in cache/startup-00.fbrb + level-00.fbrb
        if let Some(ref block) = screens_block {
            if let Some((_, ref mut st_ents)) = startup_loaded {
                for e in st_ents.iter_mut() {
                    if e.name.eq_ignore_ascii_case(&desc_key) {
                        let xml = dbx_to_xml(&e.data)?;
                        let patched = patch_mp_description_loading_screens(&xml, block);
                        if patched != xml {
                            e.data = xml_to_dbx(&patched)?;
                            startup_dirty = true;
                        }
                    }
                }
            }
            let level_path = lv_dir.join("level-00.fbrb");
            if level_path.is_file() {
                let (lv_ver, mut lv_ents) = read_fbrb(&level_path)?;
                let mut lv_dirty = false;
                for e in &mut lv_ents {
                    if e.name.eq_ignore_ascii_case(&desc_key) {
                        let xml = dbx_to_xml(&e.data)?;
                        let patched = patch_mp_description_loading_screens(&xml, block);
                        if patched != xml {
                            e.data = xml_to_dbx(&patched)?;
                            lv_dirty = true;
                        }
                    }
                }
                if lv_dirty {
                    write_fbrb(&level_path, lv_ver, &lv_ents, true)?;
                }
            }
        }
        println!(
            "[loader] {lv}: native BFBC1 MP loader-00.fbrb ({added_screens} screen assets, 0 donor BinkMemory)"
        );
    }

    if startup_dirty && let Some((st_ver, st_ents)) = startup_loaded {
        write_fbrb(&startup_path, st_ver, &st_ents, true)?;
        println!("[loader] Updated {} with BFBC1 MP LoadingScreens", startup_path.display());
    }

    println!(
        "[loader] Native BFBC1 MP loading screens wired: {updated_loaders}/{}",
        BFBC1_MP_LEVELS.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Command 4: `bfbc1-mainmenu` (Native BFBC1 mainmenu-00.fbrb + PC 3D Gunrack)
// ---------------------------------------------------------------------------

fn rewrite_gunrack_shaders_xml(xml: &str) -> Option<String> {
    let part_guid = xml.split("partition guid=\"").nth(1)?.split('"').next()?;
    let primary = xml.split("primaryInstance=\"").nth(1)?.split('"').next()?;
    let inst_guid = xml.split("<instance guid=\"").nth(1)?.split('"').next()?;
    let name_val = xml
        .split("<field name=\"Name\">")
        .nth(1)?
        .split("</field>")
        .next()?;
    Some(format!(
        "<?xml version=\"1.0\"?>\n\
         <partition guid=\"{part_guid}\" primaryInstance=\"{primary}\" exportMode=\"All\">\n\
         \t<instance guid=\"{inst_guid}\" type=\"Render.ShaderDatabaseCollection\" id=\"Shaders\">\n\
         \t\t<field name=\"Name\">{name_val}</field>\n\
         \t\t<field name=\"MemoryReportGroup\">MemoryReportGroup_default</field>\n\
         \t\t<array name=\"Groups\">\n\
         \t\t\t<complex>\n\
         \t\t\t\t<field name=\"Name\">Scenes/UI/Gunrack/Shaders/Dx9_Single</field>\n\
         \t\t\t\t<field name=\"Path\">ShaderRenderPath_Dx9</field>\n\
         \t\t\t</complex>\n\
         \t\t\t<complex>\n\
         \t\t\t\t<field name=\"Name\">Scenes/UI/Gunrack/Shaders/Dx10_Single</field>\n\
         \t\t\t\t<field name=\"Path\">ShaderRenderPath_Dx10</field>\n\
         \t\t\t</complex>\n\
         \t\t\t<complex>\n\
         \t\t\t\t<field name=\"Name\">Scenes/UI/Gunrack/Shaders/Dx11_Single</field>\n\
         \t\t\t\t<field name=\"Path\">ShaderRenderPath_Dx11</field>\n\
         \t\t\t</complex>\n\
         \t\t</array>\n\
         \t</instance>\n\
         </partition>\n"
    ))
}

pub fn cmd_bfbc1_mainmenu(default_root: &Path, args: &[String]) -> Result<(), String> {
    let opfor = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let iso = flag_val(args, "--iso")
        .ok_or("usage: opfor-cli bfbc1-mainmenu --iso <bfbc1.iso> --bfbc2 <bfbc2_dir> [--opfor <dir>]")?;
    let bfbc2 = flag_val(args, "--bfbc2")
        .ok_or("usage: opfor-cli bfbc1-mainmenu --iso <bfbc1.iso> --bfbc2 <bfbc2_dir> [--opfor <dir>]")?;
    let xenolith = opfor.join("utility").join("xenolith.exe");
    if !xenolith.is_file() {
        return Err(format!("xenolith.exe not found at {}", xenolith.display()));
    }

    let idx = index_iso(&iso)?;
    let &(off, sz) = idx
        .files
        .get("dist/xenon/menu/main/mainmenu-00.fbrb")
        .ok_or("dist/xenon/menu/main/mainmenu-00.fbrb not found on BFBC1 ISO")?;

    let pid = std::process::id();
    let tmp_raw = std::env::temp_dir().join(format!("opfor_bc1_mm_raw_{pid}.fbrb"));
    let tmp_port = std::env::temp_dir().join(format!("opfor_bc1_mm_port_{pid}.fbrb"));

    {
        let mut fp = File::open(&iso).map_err(|e| format!("open ISO: {e}"))?;
        let mut out = File::create(&tmp_raw).map_err(|e| format!("create tmp: {e}"))?;
        copy_iso_range(&mut fp, off, sz, &mut out)?;
    }

    let out_port = Command::new(&xenolith)
        .args([
            "port-fbrb",
            "--bfbc1",
            &tmp_raw.display().to_string(),
            &tmp_port.display().to_string(),
        ])
        .output()
        .map_err(|e| format!("run xenolith port-fbrb: {e}"))?;
    let _ = fs::remove_file(&tmp_raw);
    if !out_port.status.success() {
        let _ = fs::remove_file(&tmp_port);
        return Err(format!(
            "xenolith port-fbrb mainmenu failed: {}",
            String::from_utf8_lossy(&out_port.stderr)
        ));
    }

    // Load PC Dx9/Dx10/Dx11 shader databases from BFBC2 donor level (mp_012gr)
    let donor_lvl = bfbc2
        .join("Dist")
        .join("win32")
        .join("levels")
        .join("mp_012gr")
        .join("level-00.fbrb");
    let (_, donor_entries) = read_fbrb(&donor_lvl)?;
    let mut pc_shaders: Vec<FbrbEntry> = Vec::new();
    for (ext, target_name) in [
        ("Dx9ShaderDatabase", "Scenes/UI/Gunrack/Shaders/Dx9_Single.res"),
        ("Dx10ShaderDatabase", "Scenes/UI/Gunrack/Shaders/Dx10_Single.res"),
        ("Dx11ShaderDatabase", "Scenes/UI/Gunrack/Shaders/Dx11_Single.res"),
    ] {
        if let Some(s) = donor_entries.iter().find(|e| e.ext == ext) {
            pc_shaders.push(FbrbEntry {
                name: target_name.to_owned(),
                ext: s.ext.clone(),
                flags: 0x0001_0000,
                data: s.data.clone(),
            });
        }
    }
    if pc_shaders.len() != 3 {
        let _ = fs::remove_file(&tmp_port);
        return Err(format!(
            "expected 3 PC shader databases in {}, found {}",
            donor_lvl.display(),
            pc_shaders.len()
        ));
    }

    let (ver, entries) = read_fbrb(&tmp_port)?;
    let _ = fs::remove_file(&tmp_port);

    let mut out_entries = Vec::with_capacity(entries.len() + 3);
    let mut stripped = 0usize;
    let mut gunrack_rewritten = 0usize;

    for e in entries {
        if matches!(
            e.ext.as_str(),
            "XenonVertexShader"
                | "XenonPixelShader"
                | "ShaderDatabase"
                | "Dx9ShaderDatabase"
                | "Dx10ShaderDatabase"
                | "Dx11ShaderDatabase"
        ) {
            stripped += 1;
            continue;
        }
        let nl = e.name.to_ascii_lowercase();
        if nl.starts_with("scenes/ui/gunrack/")
            && nl.ends_with("/shaders.dbx")
            && let Ok(parsed) = dbx::parse(&e.data)
            && let Some(new_xml) = rewrite_gunrack_shaders_xml(&parsed.to_xml())
            && let Ok(rebuilt) = dbx::from_xml(&new_xml)
        {
            out_entries.push(FbrbEntry {
                name: e.name,
                ext: e.ext,
                flags: e.flags,
                data: rebuilt.to_bytes(),
            });
            gunrack_rewritten += 1;
            continue;
        }
        out_entries.push(e);
    }

    out_entries.extend(pc_shaders);
    let dest = opfor
        .join("modules")
        .join("bfbc1")
        .join("menu")
        .join("main")
        .join("mainmenu-00.fbrb");
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    write_fbrb(&dest, ver, &out_entries, true)?;
    println!(
        "[mainmenu] Wrote {} ({} entries, {} Gunrack scenes -> PC Dx9/10/11, stripped {} Xenon shaders)",
        dest.display(),
        out_entries.len(),
        gunrack_rewritten,
        stripped
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Command 3: `bfbc1-navmeshes`
// ---------------------------------------------------------------------------

pub fn cmd_bfbc1_navmeshes(default_root: &Path, args: &[String]) -> Result<(), String> {
    let opfor = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let iso = flag_val(args, "--iso")
        .ok_or("usage: opfor-cli bfbc1-navmeshes --iso <bfbc1.iso> [--opfor <dir>]")?;
    let xenolith = opfor.join("utility").join("xenolith.exe");
    let out_base = opfor.join("modules").join("bfbc1");

    let idx = index_iso(&iso)?;
    let hpd_rels: Vec<(String, u64, u64)> = idx
        .files
        .iter()
        .filter(|(k, _)| k.starts_with("output/xenon/ai/") && k.ends_with(".hpd"))
        .map(|(k, &(off, sz))| (k.clone(), off, sz))
        .collect();
    println!("[hpd] Found {} BFBC1 navmesh files on ISO", hpd_rels.len());

    let mut fp = File::open(&iso).map_err(|e| format!("open ISO: {e}"))?;
    let mut converted = 0usize;
    let tmp_in = std::env::temp_dir().join(format!("opfor_hpd_in_{}.hpd", std::process::id()));

    for (rel, off, sz) in &hpd_rels {
        let sub = &rel["output/xenon/".len()..];
        let dest = out_base.join(sub.replace('/', "\\"));
        if dest.is_file()
            && let Ok(mut f) = File::open(&dest)
        {
            let mut magic = [0u8; 12];
            if f.read_exact(&mut magic).is_ok() && &magic == b"Kynogon HPD\0" {
                println!("  {sub}: SKIP (already HPD v2)");
                converted += 1;
                continue;
            }
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        {
            let mut out = File::create(&tmp_in).map_err(|e| e.to_string())?;
            copy_iso_range(&mut fp, *off, *sz, &mut out)?;
        }
        let out = Command::new(&xenolith)
            .args([
                "upgrade-hpd",
                &tmp_in.display().to_string(),
                &dest.display().to_string(),
            ])
            .output()
            .map_err(|e| format!("run xenolith upgrade-hpd: {e}"))?;
        let _ = fs::remove_file(&tmp_in);
        if !out.status.success() {
            return Err(format!(
                "xenolith upgrade-hpd failed on {sub}: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        println!("  {sub}: {}", String::from_utf8_lossy(&out.stdout).trim());
        converted += 1;
    }
    println!(
        "[hpd] Done: {converted}/{} BFBC1 navmeshes upgraded to HPD v2.",
        hpd_rels.len()
    );

    // Bridge each SP level's level-00.fbrb: replace legacy 8-byte FB 1.0 `PathDataDefinition.res`
    // with Frostbite 1.5 `Levels/<lvl>/Precomputed/{Infantry,Vehicle}/SectorOffsets.dbx`
    // (`GameSharedResources.PathdataSectorOffsets`) so `0x00F20A33` opens `/PathData.hpd`.
    let tmp_dbx = std::env::temp_dir().join(format!("opfor_sector_offsets_{}.dbx", std::process::id()));
    let mut patched_levels = 0usize;
    for lvl in BFBC1_SP_LEVELS {
        let level_fbrb = out_base.join("levels").join(lvl).join("level-00.fbrb");
        if !level_fbrb.is_file() {
            continue;
        }
        let mut layers_to_add: Vec<(&str, PathBuf)> = Vec::new();
        for layer in ["Infantry", "Vehicle"] {
            let hpd = out_base
                .join("ai")
                .join("levels")
                .join(lvl)
                .join(layer.to_ascii_lowercase())
                .join("pathdata.hpd");
            if hpd.is_file() {
                layers_to_add.push((layer, hpd));
            }
        }
        if layers_to_add.is_empty() {
            continue;
        }
        let (ver, mut entries) = read_fbrb(&level_fbrb)?;
        let before = entries.len();
        entries.retain(|e| {
            e.ext != "PathDataDefinition"
                && !e.name.to_ascii_lowercase().ends_with("/sectoroffsets.dbx")
        });
        let removed = before - entries.len();

        for (layer, hpd_path) in &layers_to_add {
            let (xoff, zoff) = compute_hpd_sector_offsets(hpd_path).unwrap_or((0, 0));
            let xoff_s = xoff.to_string();
            let zoff_s = zoff.to_string();
            let out = Command::new(&xenolith)
                .args([
                    "make-sector-offsets",
                    lvl,
                    layer,
                    &xoff_s,
                    &zoff_s,
                    &tmp_dbx.display().to_string(),
                ])
                .output()
                .map_err(|e| format!("run xenolith make-sector-offsets: {e}"))?;
            if !out.status.success() {
                let _ = fs::remove_file(&tmp_dbx);
                return Err(format!(
                    "xenolith make-sector-offsets failed for {lvl}/{layer}: {}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            let dbx_data = fs::read(&tmp_dbx).map_err(|e| format!("read tmp dbx: {e}"))?;
            let _ = fs::remove_file(&tmp_dbx);
            let entry_path = format!("Levels/{lvl}/Precomputed/{layer}/SectorOffsets.dbx");
            entries.push(FbrbEntry {
                name: entry_path,
                ext: "<non-resource>".into(),
                flags: 0x0001_0000,
                data: dbx_data,
            });
            println!("  [sectoroffsets] {lvl}/{layer}: XOffset={xoff}, ZOffset={zoff}");
        }
        write_fbrb(&level_fbrb, ver, &entries, true)?;
        patched_levels += 1;
        println!(
            "[hpd] {lvl}/level-00.fbrb: removed {removed} legacy PathDataDefinition, injected {} SectorOffsets.dbx",
            layers_to_add.len()
        );
    }
    println!(
        "[hpd] Patched {patched_levels}/{} BFBC1 SP level-00.fbrb bundles with PathdataSectorOffsets.",
        BFBC1_SP_LEVELS.len()
    );
    Ok(())
}

/// Computes `(XOffset, ZOffset)` on Frostbite's 100 m sector grid from a Kynogon HPD v2 file's
/// sector AABBs, ignoring sentinel/non-finite bounds (`|v| > 1e5`).
fn compute_hpd_sector_offsets(hpd_path: &Path) -> Result<(i32, i32), String> {
    let data = fs::read(hpd_path).map_err(|e| format!("read {}: {e}", hpd_path.display()))?;
    if data.len() < 0x9C || !data.starts_with(b"Kynogon HPD\0") {
        return Err(format!("not an HPD v2 file: {}", hpd_path.display()));
    }
    let cnt = u32::from_le_bytes(data[0x14..0x18].try_into().unwrap()) as usize;
    let mut min_x = i32::MAX;
    let mut min_z = i32::MAX;
    for i in 0..cnt {
        let ep = 0x9C + i * 48;
        if ep + 28 > data.len() {
            break;
        }
        let mut b = [0.0f32; 6];
        for (k, slot) in b.iter_mut().enumerate() {
            let o = ep + 4 + k * 4;
            *slot = f32::from_le_bytes(data[o..o + 4].try_into().unwrap());
        }
        if b.iter().any(|v| !v.is_finite() || v.abs() > 1e5) {
            continue;
        }
        let gx = ((b[0] + b[1]) / 200.0).floor() as i32;
        let gz = ((b[4] + b[5]) / 200.0).floor() as i32;
        if gx < min_x {
            min_x = gx;
        }
        if gz < min_z {
            min_z = gz;
        }
    }
    if min_x == i32::MAX || min_z == i32::MAX {
        Ok((0, 0))
    } else {
        Ok((min_x, min_z))
    }
}
