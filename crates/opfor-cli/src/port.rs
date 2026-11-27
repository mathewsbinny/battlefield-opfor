//! Pure-Rust staging and batch porting drivers for Project OPFOR (`opfor-cli`):
//! - `setup-bfbc2`: Stage user's read-only PC BFBC2 install into `engine/` + extract Hub logos.
//! - `build-havok-template`: Synthesize unified multi-shape PC Havok 5.5.0 template from BFBC2.
//! - `port-bf1943`: Read-only Xbox 360 XBLA STFS extractor + batch `xenolith port-fbrb` + Hub logo.
//! - `port-bfbc1`: Read-only Xbox 360 ISO batch porting driver (MP/SP/Oasis/All) using `xenolith`.
#![deny(unsafe_code)]

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bfbc1::{FbrbEntry, copy_iso_range, flag_val, index_iso, read_fbrb, write_fbrb};

const HAVOK_MAGIC: [u8; 8] = [0x57, 0xE0, 0xE0, 0x57, 0x10, 0xC0, 0xC0, 0x10];
const STFS_BLOCK_SIZE: u64 = 0x1000;

const MP_LEVELS: &[&str] = &[
    "mp_beachhead_day1",
    "mp_beachhead_day2",
    "mp_harbour_day1",
    "mp_harbour_day2",
    "mp_ignition_day1",
    "mp_ignition_day2",
    "mp_monastery_day1",
    "mp_monastery_day2",
];

const SP_LEVELS: &[&str] = &[
    "sp_01_greenacres",
    "sp_02_traitor_of_zabograd",
    "sp_03_crossroads",
    "sp_05_par_for_the_course",
    "sp_06_hot_zone",
    "sp_07_pow",
    "sp_08_ghost_town",
];

const DONOR_LEVEL: &str = "mp_012gr";
const DONOR_PART_GUID: &str = "F717D0D2-159F-11DF-A23A-F187024E9658";
const DONOR_INST_GUID: &str = "F717D0D3-159F-11DF-A23A-F187024E9658";

fn flag_str(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn run_xenolith(xenolith: &Path, args: &[String]) -> Result<String, String> {
    let out = Command::new(xenolith)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {}: {e}", xenolith.display()))?;
    if !out.status.success() {
        return Err(format!(
            "xenolith {} failed (exit {:?}): {}",
            args.first().map_or("", String::as_str),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn copy_if_size_differs(src: &Path, dst: &Path) -> Result<bool, String> {
    let sm = fs::metadata(src).map_err(|e| format!("stat {}: {e}", src.display()))?;
    if let Ok(dm) = fs::metadata(dst)
        && dm.is_file()
        && dm.len() == sm.len()
    {
        return Ok(false);
    }
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p).map_err(|e| format!("mkdir {}: {e}", p.display()))?;
    }
    fs::copy(src, dst).map_err(|e| format!("copy {} -> {}: {e}", src.display(), dst.display()))?;
    Ok(true)
}

fn copy_dir_recursive(src_root: &Path, base: &Path, dst_base: &Path, copied: &mut usize, skipped: &mut usize) -> Result<(), String> {
    if !src_root.is_dir() {
        return Ok(());
    }
    for ent in fs::read_dir(src_root).map_err(|e| format!("readdir {}: {e}", src_root.display()))? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        let ft = ent.file_type().map_err(|e| e.to_string())?;
        if ft.is_dir() {
            copy_dir_recursive(&p, base, dst_base, copied, skipped)?;
        } else if ft.is_file() {
            let rel = p.strip_prefix(base).map_err(|e| e.to_string())?;
            if copy_if_size_differs(&p, &dst_base.join(rel))? {
                *copied += 1;
            } else {
                *skipped += 1;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 1. `setup-bfbc2` (replaces utility/setup_bfbc2.py)
// ---------------------------------------------------------------------------

pub fn cmd_setup_bfbc2(default_root: &Path, args: &[String]) -> Result<(), String> {
    let bfbc2_dir = flag_val(args, "--bfbc2")
        .ok_or("usage: opfor-cli setup-bfbc2 --bfbc2 <bfbc2_dir> --opfor <opfor_dir>")?;
    let opfor_dir = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());

    if !bfbc2_dir.join("BFBC2Game.exe").is_file() {
        return Err(format!("BFBC2Game.exe not found in {}", bfbc2_dir.display()));
    }

    let engine_dir = opfor_dir.join("engine");
    fs::create_dir_all(&engine_dir).map_err(|e| e.to_string())?;
    println!(
        "[BFBC2] Staging PC runtime from {} -> {} (original untouched)...",
        bfbc2_dir.display(),
        engine_dir.display()
    );

    let mut copied = 0usize;
    let mut skipped = 0usize;
    for ent in fs::read_dir(&bfbc2_dir).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        if !ent.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().to_string();
        let dst_name = if name.eq_ignore_ascii_case("bfbc2game.exe") {
            "OPFOREngine.exe".to_owned()
        } else {
            name
        };
        if copy_if_size_differs(&p, &engine_dir.join(dst_name))? {
            copied += 1;
        } else {
            skipped += 1;
        }
    }

    for sub in ["Dist", "Output", "Package"] {
        copy_dir_recursive(&bfbc2_dir.join(sub), &bfbc2_dir, &engine_dir, &mut copied, &mut skipped)?;
    }
    println!("[BFBC2] Done: {copied} files staged, {skipped} already up-to-date. Original BFBC2 untouched.");

    let xenolith = opfor_dir.join("utility").join("xenolith.exe");
    let pkg_menu = bfbc2_dir.join("Package").join("menu").join("main").join("mainmenu-00.fbrb");
    let hub_dir = opfor_dir.join("modules").join("hub");
    if xenolith.is_file() && pkg_menu.is_file() {
        fs::create_dir_all(&hub_dir).map_err(|e| e.to_string())?;
        for (sprite_id, out_name) in [("473", "logo_bfbc2.png"), ("475", "logo_vietnam.png")] {
            let dst_logo = hub_dir.join(out_name);
            match run_xenolith(
                &xenolith,
                &[
                    "export-atlas-sprite".into(),
                    pkg_menu.display().to_string(),
                    "menu/main_nav_textures_win32".into(),
                    sprite_id.into(),
                    dst_logo.display().to_string(),
                ],
            ) {
                Ok(msg) => println!("[Hub] {msg}"),
                Err(e) => println!("[Hub] warning: could not extract {out_name}: {e}"),
            }
        }
    }
    if let Err(e) = enrich_onslaught_loader_ingame(&bfbc2_dir, &opfor_dir) {
        println!("[Onslaught] warning: could not enrich loader/ingame bundles: {e}");
    }
    Ok(())
}

/// Ensure all pre-staged Onslaught levels (`modules/onslaught/levels/ons_mp_*`) have
/// native PC `Loading_Music_MP_0x_Wave.{res,dbx}` in `loader-00.fbrb` and
/// `Levels/MP_00x/UI/minimapTexture.res` in `ingame-00.fbrb` sourced from stock PC BFBC2.
fn enrich_onslaught_loader_ingame(bfbc2_dir: &Path, opfor_dir: &Path) -> Result<(), String> {
    let ons_root = opfor_dir.join("modules").join("onslaught").join("levels");
    if !ons_root.is_dir() {
        return Ok(());
    }
    let ons_pairs = [
        ("ons_mp_002", "mp_002", "loading_music_mp_02_wave"),
        ("ons_mp_004", "mp_004", "loading_music_mp_04_wave"),
        ("ons_mp_005", "mp_005", "loading_music_mp_05_wave"),
        ("ons_mp_005gr", "mp_005", "loading_music_mp_05_wave"),
        ("ons_mp_008", "mp_008", "loading_music_mp_08_wave"),
    ];
    let mut updated = 0usize;
    for (ons_lv, base_mp, music_stem) in ons_pairs {
        let ldir = ons_root.join(ons_lv);
        if !ldir.is_dir() {
            continue;
        }
        let dst_loader = ldir.join("loader-00.fbrb");
        if dst_loader.is_file() {
            let (ver, mut ents) = read_fbrb(&dst_loader)?;
            let mut seen: HashSet<String> = ents.iter().map(|e| e.name.to_ascii_lowercase()).collect();
            let mut changed = false;
            for src_dir in [
                bfbc2_dir.join("Dist").join("win32").join("levels").join(base_mp),
                bfbc2_dir.join("Package").join("levels").join(base_mp),
                bfbc2_dir.join("Package").join("levels").join(format!("{base_mp}cq")),
            ] {
                let src_loader = src_dir.join("loader-00.fbrb");
                if !src_loader.is_file() {
                    continue;
                }
                if let Ok((_, src_ents)) = read_fbrb(&src_loader) {
                    for e in src_ents {
                        let nl = e.name.to_ascii_lowercase();
                        if nl.contains(music_stem) && (nl.ends_with(".res") || nl.ends_with(".dbx")) && seen.insert(nl) {
                            ents.push(e);
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                write_fbrb(&dst_loader, ver, &ents, true)?;
                updated += 1;
            }
        }
        let dst_ingame = ldir.join("ingame-00.fbrb");
        if dst_ingame.is_file() {
            let (ver, mut ents) = read_fbrb(&dst_ingame)?;
            let mut seen: HashSet<String> = ents.iter().map(|e| e.name.to_ascii_lowercase()).collect();
            let mut changed = false;
            for src_dir in [
                bfbc2_dir.join("Dist").join("win32").join("levels").join(base_mp),
                bfbc2_dir.join("Package").join("levels").join(base_mp),
            ] {
                let src_ingame = src_dir.join("ingame-00.fbrb");
                if !src_ingame.is_file() {
                    continue;
                }
                if let Ok((_, src_ents)) = read_fbrb(&src_ingame) {
                    for e in src_ents {
                        let nl = e.name.to_ascii_lowercase();
                        if e.ext == "ITexture" && nl.ends_with("/ui/minimaptexture.res") && seen.insert(nl) {
                            ents.push(e);
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                write_fbrb(&dst_ingame, ver, &ents, true)?;
                updated += 1;
            }
        }
    }
    if updated > 0 {
        println!("[Onslaught] Enriched {updated} loader/ingame bundle(s) with native PC music & minimaps.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `build-havok-template` (replaces utility/build_havok_template.py)
// ---------------------------------------------------------------------------

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn extract_le_classnames(blob: &[u8]) -> Vec<(String, u32)> {
    let Some(m) = find_bytes(blob, &HAVOK_MAGIC) else {
        return Vec::new();
    };
    if m + 64 > blob.len() || blob[m + 0x11] != 1 {
        return Vec::new();
    }
    let num_secs = u32::from_le_bytes(blob[m + 20..m + 24].try_into().unwrap()) as usize;
    for si in 0..num_secs {
        let so = m + 64 + si * 48;
        if so + 48 > blob.len() {
            break;
        }
        let tag_raw = &blob[so..so + 19];
        let tag_end = tag_raw.iter().position(|&b| b == 0).unwrap_or(19);
        if &tag_raw[..tag_end] == b"__classnames__" {
            let abs_off = u32::from_le_bytes(blob[so + 20..so + 24].try_into().unwrap()) as usize;
            let loc_off = u32::from_le_bytes(blob[so + 24..so + 28].try_into().unwrap()) as usize;
            let mut o = m + abs_off;
            let end = blob.len().min(o + loc_off);
            let mut out = Vec::new();
            while o + 5 < end && blob[o + 4] == 0x09 {
                let sig = u32::from_le_bytes(blob[o..o + 4].try_into().unwrap());
                let Some(rel_z) = blob[o + 5..end].iter().position(|&b| b == 0) else {
                    break;
                };
                let z = o + 5 + rel_z;
                let name = String::from_utf8_lossy(&blob[o + 5..z]).into_owned();
                out.push((name, sig));
                o = z + 1;
            }
            return out;
        }
    }
    Vec::new()
}

pub fn build_havok_template(bfbc2_dir: &Path, out_path: &Path) -> Result<usize, String> {
    let mut candidates = Vec::new();
    for base in [
        bfbc2_dir.join("Dist").join("win32").join("levels"),
        bfbc2_dir.join("Package").join("levels"),
    ] {
        if let Ok(rd) = fs::read_dir(&base) {
            let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|x| x.path())).collect();
            dirs.sort();
            for lv in dirs {
                let f = lv.join("level-00.fbrb");
                if f.is_file() {
                    candidates.push(f);
                }
            }
        }
    }

    let mut merged: BTreeMap<String, u32> = BTreeMap::new();
    let mut scanned = 0usize;
    for fbrb in &candidates {
        if let Ok((_, entries)) = read_fbrb(fbrb) {
            scanned += 1;
            for e in entries {
                if (e.ext == "HavokPhysicsData" || e.ext == "RagdollResource")
                    && e.data.len() >= 64
                    && find_bytes(&e.data, b"Havok-5.5.0").is_some()
                {
                    for (name, sig) in extract_le_classnames(&e.data) {
                        merged.entry(name).or_insert(sig);
                    }
                }
            }
        }
    }
    if merged.is_empty() {
        return Err(format!("No PC Havok 5.5.0 packfiles found under {}", bfbc2_dir.display()));
    }
    for extra in [
        "hkpSphereShape",
        "hkpConvexTransformShape",
        "hkpCapsuleShape",
        "hkpCylinderShape",
        "hkpTriangleShape",
    ] {
        merged.entry(extra.to_owned()).or_insert(0);
    }

    let mut cn_buf = Vec::new();
    for (name, sig) in &merged {
        cn_buf.extend_from_slice(&sig.to_le_bytes());
        cn_buf.push(0x09);
        cn_buf.extend_from_slice(name.as_bytes());
        cn_buf.push(0x00);
    }
    while cn_buf.len() % 16 != 0 {
        cn_buf.push(0xFF);
    }

    let cn_abs_off: u32 = 128;
    let cn_len = cn_buf.len() as u32;
    let mut hdr = [0u8; 64];
    hdr[0..8].copy_from_slice(&HAVOK_MAGIC);
    hdr[12..16].copy_from_slice(&5u32.to_le_bytes());
    hdr[16] = 4;
    hdr[17] = 1;
    hdr[19] = 1;
    hdr[20..24].copy_from_slice(&1u32.to_le_bytes());
    hdr[40..54].copy_from_slice(b"Havok-5.5.0-r1");

    let mut sec = [0u8; 48];
    sec[0..14].copy_from_slice(b"__classnames__");
    sec[19] = 0xFF;
    sec[20..24].copy_from_slice(&cn_abs_off.to_le_bytes());
    for i in 0..6 {
        let o = 24 + i * 4;
        sec[o..o + 4].copy_from_slice(&cn_len.to_le_bytes());
    }

    let mut tpl = Vec::with_capacity(128 + cn_buf.len());
    tpl.extend_from_slice(&hdr);
    tpl.extend_from_slice(&sec);
    tpl.resize(128, 0);
    tpl.extend_from_slice(&cn_buf);

    if let Some(p) = out_path.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::write(out_path, &tpl).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    println!(
        "Built Havok 5.5.0 master template ({} unique classes from {scanned} BFBC2 bundles, {} bytes) -> {}",
        merged.len(),
        tpl.len(),
        out_path.display()
    );
    Ok(merged.len())
}

pub fn cmd_build_havok_template(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: opfor-cli build-havok-template <bfbc2_dir> <out_file>".into());
    }
    build_havok_template(Path::new(&args[0]), Path::new(&args[1]))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `port-bf1943` (replaces utility/port_bf1943.py)
// ---------------------------------------------------------------------------

fn stfs_block_to_offset(block: u32, first_hash_off: u64, female: bool, total_blocks: u32) -> u64 {
    let step: u64 = if female { 1 } else { 2 };
    let b = block as u64;
    let l0 = (b / 0xAA) + 1;
    let l1 = if total_blocks > 0xAA { (b / 0x70E4) + 1 } else { 0 };
    let l2 = if total_blocks > 0x70E4 { 1 } else { 0 };
    first_hash_off + (b + (l0 + l1 + l2) * step) * STFS_BLOCK_SIZE
}

fn stfs_next_block(fp: &mut File, block: u32, first_hash_off: u64, female: bool, total_blocks: u32) -> Result<u32, String> {
    let step: u64 = if female { 1 } else { 2 };
    let b = block as u64;
    let l0_idx = b / 0xAA;
    let entry_idx = b % 0xAA;
    let l0_before = l0_idx;
    let l1_before = if total_blocks > 0xAA { (b / 0x70E4) + 1 } else { 0 };
    let l2_before = if total_blocks > 0x70E4 { 1 } else { 0 };
    let hash_blk = (l0_before + l1_before + l2_before) * step + l0_idx * 0xAA;
    let hash_off = first_hash_off + hash_blk * STFS_BLOCK_SIZE + entry_idx * 24 + 21;
    fp.seek(SeekFrom::Start(hash_off)).map_err(|e| e.to_string())?;
    let mut nxt = [0u8; 3];
    if fp.read_exact(&mut nxt).is_err() {
        return Ok(0x00FF_FFFF);
    }
    Ok(((nxt[0] as u32) << 16) | ((nxt[1] as u32) << 8) | (nxt[2] as u32))
}

fn extract_stfs(pkg_path: &Path, out_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut fp = File::open(pkg_path).map_err(|e| format!("open STFS {}: {e}", pkg_path.display()))?;
    let mut magic = [0u8; 4];
    fp.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"LIVE" && &magic != b"PIRS" && &magic != b"CON " {
        return Err(format!("Not an Xbox 360 STFS package: {}", pkg_path.display()));
    }
    fp.seek(SeekFrom::Start(0x340)).map_err(|e| e.to_string())?;
    let mut hs_buf = [0u8; 4];
    fp.read_exact(&mut hs_buf).map_err(|e| e.to_string())?;
    let header_size = u32::from_be_bytes(hs_buf) as u64;
    let first_hash_off = (header_size + 0xFFF) & !0xFFF;

    fp.seek(SeekFrom::Start(0x379)).map_err(|e| e.to_string())?;
    let mut vd = [0u8; 36];
    fp.read_exact(&mut vd).map_err(|e| e.to_string())?;
    let female = (vd[2] & 1) == 1;
    let ft_blocks = u16::from_le_bytes([vd[3], vd[4]]) as usize;
    let ft_start = (vd[5] as u32) | ((vd[6] as u32) << 8) | ((vd[7] as u32) << 16);
    let total_blocks = u32::from_be_bytes([vd[28], vd[29], vd[30], vd[31]]);

    let mut ft_data = Vec::with_capacity(ft_blocks * STFS_BLOCK_SIZE as usize);
    let mut cur = ft_start;
    let mut blk_buf = [0u8; STFS_BLOCK_SIZE as usize];
    for _ in 0..ft_blocks {
        if cur >= 0x00FF_FFFF {
            break;
        }
        fp.seek(SeekFrom::Start(stfs_block_to_offset(cur, first_hash_off, female, total_blocks)))
            .map_err(|e| e.to_string())?;
        fp.read_exact(&mut blk_buf).map_err(|e| e.to_string())?;
        ft_data.extend_from_slice(&blk_buf);
        cur = stfs_next_block(&mut fp, cur, first_hash_off, female, total_blocks)?;
    }

    struct StfsRec {
        name: String,
        is_dir: bool,
        contiguous: bool,
        start_blk: u32,
        path_idx: i16,
        file_size: u64,
    }
    let mut entries = Vec::new();
    for chunk in ft_data.chunks_exact(64) {
        if chunk[0] == 0 {
            break;
        }
        let flags = chunk[40];
        let name_len = (flags & 0x3F) as usize;
        let is_dir = (flags & 0x80) != 0;
        let contiguous = (flags & 0x40) != 0;
        let name = String::from_utf8_lossy(&chunk[..name_len.min(40)]).into_owned();
        let start_blk = (chunk[47] as u32) | ((chunk[48] as u32) << 8) | ((chunk[49] as u32) << 16);
        let path_idx = i16::from_be_bytes([chunk[50], chunk[51]]);
        let file_size = u32::from_be_bytes([chunk[52], chunk[53], chunk[54], chunk[55]]) as u64;
        entries.push(StfsRec {
            name,
            is_dir,
            contiguous,
            start_blk,
            path_idx,
            file_size,
        });
    }

    let mut extracted = Vec::new();
    for (idx, rec) in entries.iter().enumerate() {
        if rec.is_dir {
            continue;
        }
        let mut parts = Vec::new();
        let mut seen = HashSet::new();
        let mut cur_i = idx as i32;
        while cur_i >= 0 && (cur_i as usize) < entries.len() && seen.insert(cur_i) {
            parts.push(entries[cur_i as usize].name.as_str());
            cur_i = entries[cur_i as usize].path_idx as i32;
        }
        parts.reverse();
        let mut dst = out_dir.to_path_buf();
        for p in parts {
            dst.push(p);
        }
        if let Ok(m) = fs::metadata(&dst)
            && m.is_file()
            && m.len() == rec.file_size
        {
            extracted.push(dst);
            continue;
        }
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut out = File::create(&dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
        let mut rem = rec.file_size;
        let mut blk = rec.start_blk;
        while rem > 0 && blk < 0x00FF_FFFF {
            fp.seek(SeekFrom::Start(stfs_block_to_offset(blk, first_hash_off, female, total_blocks)))
                .map_err(|e| e.to_string())?;
            let take = rem.min(STFS_BLOCK_SIZE) as usize;
            fp.read_exact(&mut blk_buf[..take]).map_err(|e| e.to_string())?;
            out.write_all(&blk_buf[..take]).map_err(|e| e.to_string())?;
            rem -= take as u64;
            blk = if rec.contiguous {
                blk + 1
            } else {
                stfs_next_block(&mut fp, blk, first_hash_off, female, total_blocks)?
            };
        }
        extracted.push(dst);
    }
    Ok(extracted)
}

pub fn cmd_port_bf1943(default_root: &Path, args: &[String]) -> Result<(), String> {
    let xbla = flag_val(args, "--xbla")
        .ok_or("usage: opfor-cli port-bf1943 --xbla <file> --bfbc2 <dir> --opfor <dir>")?;
    let bfbc2_dir = flag_val(args, "--bfbc2")
        .ok_or("usage: opfor-cli port-bf1943 --xbla <file> --bfbc2 <dir> --opfor <dir>")?;
    let opfor_dir = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());

    let xenolith = opfor_dir.join("utility").join("xenolith.exe");
    let bfbc2_exe = bfbc2_dir.join("BFBC2Game.exe");
    if !xenolith.is_file() {
        return Err(format!("Missing {}", xenolith.display()));
    }
    if !bfbc2_exe.is_file() {
        return Err(format!("BFBC2Game.exe not found in {}", bfbc2_dir.display()));
    }

    let work_dir = opfor_dir.join("work").join("bf1943");
    let raw_dir = work_dir.join("extracted");
    let mod_dir = opfor_dir.join("modules").join("bf1943");
    fs::create_dir_all(&raw_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&mod_dir).map_err(|e| e.to_string())?;

    println!("[Stage 1/3] Extracting BF1943 XBLA STFS package (read-only): {}", xbla.display());
    let files = extract_stfs(&xbla, &raw_dir)?;
    let mut fbrbs: Vec<PathBuf> = files
        .iter()
        .filter(|p| {
            p.extension().is_some_and(|e| e.eq_ignore_ascii_case("fbrb"))
                && !p
                    .file_name()
                    .map_or(false, |n| n.to_string_lossy().to_ascii_lowercase().contains("streaming_sounds"))
        })
        .cloned()
        .collect();
    fbrbs.sort();
    println!("  Extracted {} files ({} FbRB bundles).", files.len(), fbrbs.len());

    println!("[Stage 2/3] Synthesizing PC Havok 5.5.0 master template from BFBC2 (read-only)...");
    let havok_tpl = work_dir.join("havok_550_master_template.res");
    build_havok_template(&bfbc2_dir, &havok_tpl)?;

    println!("[Stage 2/3] Porting {} BF1943 FbRB bundles via xenolith.exe...", fbrbs.len());
    for (idx, src) in fbrbs.iter().enumerate() {
        let rel = src.strip_prefix(&raw_dir).map_err(|e| e.to_string())?;
        let dst = mod_dir.join("ported").join(rel);
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let out = run_xenolith(
            &xenolith,
            &[
                "port-fbrb".into(),
                "--havok-exe".into(),
                bfbc2_exe.display().to_string(),
                "--havok-template".into(),
                havok_tpl.display().to_string(),
                src.display().to_string(),
                dst.display().to_string(),
            ],
        )?;
        println!("  [{}/{}] {}: {out}", idx + 1, fbrbs.len(), rel.display());
    }

    // Stage ported BF1943 fonts & HUD bundles into modules/bf1943/menu/{shared,hud}/
    for (src_rel, dst_rel) in [
        ("dist/xenon/menu/shared/fonts_us-00.fbrb", "menu/shared/fonts_us-00.fbrb"),
        ("dist/xenon/menu/hud/hud_multiplayer_cq-00.fbrb", "menu/hud/hud_multiplayer_cq-00.fbrb"),
        ("dist/xenon/menu/hud/hud_multiplayer-00.fbrb", "menu/hud/hud_multiplayer-00.fbrb"),
        ("dist/xenon/menu/hud/hud_singleplayer-00.fbrb", "menu/hud/hud_singleplayer-00.fbrb"),
    ] {
        let s = mod_dir.join("ported").join(src_rel.replace('/', "\\"));
        let d = mod_dir.join(dst_rel.replace('/', "\\"));
        if s.is_file() {
            if let Some(p) = d.parent() {
                let _ = fs::create_dir_all(p);
            }
            let _ = fs::copy(&s, &d);
            println!("  [BF1943] Staged {} -> {}", src_rel, d.display());
        }
    }

    println!("[Stage 3/3] Exporting BF1943 Hub logo, menu art, fonts, and UI SFX...");
    let msg = run_xenolith(
        &xenolith,
        &[
            "export-bf1943-menu".into(),
            xbla.display().to_string(),
            mod_dir.display().to_string(),
        ],
    )?;
    println!("  {msg}");
    let logo_src = mod_dir.join("art").join("x360").join("bf1943_logo.png");
    if logo_src.is_file() {
        let hub_logo = opfor_dir.join("modules").join("hub").join("logo_bf1943.png");
        if let Some(p) = hub_logo.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        fs::copy(&logo_src, &hub_logo).map_err(|e| e.to_string())?;
        println!("[Hub] Installed BF1943 logo -> {}", hub_logo.display());
    }
    println!("[Stage 3/3] BF1943 installed cleanly into {}", mod_dir.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `port-bfbc1` (replaces utility/port_bfbc1.py + register_bfbc1_level.py + bind_pc_shaders.py)
// ---------------------------------------------------------------------------

fn parse_hex_byte(s: &str) -> Option<u8> {
    u8::from_str_radix(s, 16).ok()
}

fn guid_to_bytes_be(g: &str) -> Option<[u8; 16]> {
    let hex: String = g.chars().filter(|&c| c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = parse_hex_byte(&hex[i * 2..i * 2 + 2])?;
    }
    Some(out)
}

fn guid_to_bytes_le(g: &str) -> Option<[u8; 16]> {
    let mut b = guid_to_bytes_be(g)?;
    b[0..4].reverse();
    b[4..6].reverse();
    b[6..8].reverse();
    Some(b)
}

fn replace_bytes_all(data: &mut Vec<u8>, from: &[u8], to: &[u8]) {
    if from.is_empty() || from.len() != to.len() {
        return;
    }
    let mut i = 0;
    while i + from.len() <= data.len() {
        if &data[i..i + from.len()] == from {
            data[i..i + from.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
}

fn extract_two_guids_via_xenolith(xenolith: &Path, dbx_bytes: &[u8]) -> Option<(String, String)> {
    let pid = std::process::id();
    let p_in = std::env::temp_dir().join(format!("opfor_sh_in_{pid}.dbx"));
    let p_xml = std::env::temp_dir().join(format!("opfor_sh_out_{pid}.xml"));
    fs::write(&p_in, dbx_bytes).ok()?;
    let ok = run_xenolith(
        xenolith,
        &[
            "dbx-to-xml".into(),
            p_in.display().to_string(),
            p_xml.display().to_string(),
        ],
    )
    .is_ok();
    let _ = fs::remove_file(&p_in);
    if !ok {
        let _ = fs::remove_file(&p_xml);
        return None;
    }
    let xml = fs::read_to_string(&p_xml).ok()?;
    let _ = fs::remove_file(&p_xml);
    let mut found = Vec::new();
    let mut rest = xml.as_str();
    while let Some(pos) = rest.find("guid=\"") {
        rest = &rest[pos + 6..];
        if rest.len() >= 36 && rest.as_bytes()[36] == b'"' {
            found.push(rest[..36].to_owned());
            if found.len() >= 2 {
                return Some((found[0].clone(), found[1].clone()));
            }
        }
    }
    None
}

fn collect_referenced_sns_keys(raw_levels: &[PathBuf]) -> HashSet<String> {
    let mut keys = HashSet::new();
    for lp in raw_levels {
        let Ok((_, entries)) = read_fbrb(lp) else {
            continue;
        };
        for e in entries {
            if e.ext != "Wave" || e.data.len() < 40 {
                continue;
            }
            let nl = e.name.replace('\\', "/").to_ascii_lowercase();
            let Some(stem) = nl.strip_suffix("_wave.res") else {
                continue;
            };
            let mut o = 28usize;
            for seg_i in 0..3 {
                if o + 4 > e.data.len() {
                    break;
                }
                let n = u32::from_be_bytes(e.data[o..o + 4].try_into().unwrap()) as usize;
                o += 4;
                if n == 0 {
                    continue;
                }
                if n == 8 {
                    keys.insert(format!("{stem}_wavedatasns_{seg_i}.res"));
                    if seg_i == 1 {
                        keys.insert(format!("{stem}_wavedatasns.res"));
                    }
                }
                o += n + ((4 - n % 4) % 4);
                if o + 4 > e.data.len() {
                    break;
                }
                let m = u32::from_be_bytes(e.data[o..o + 4].try_into().unwrap()) as usize;
                o += 4 + m + ((4 - m % 4) % 4);
            }
        }
    }
    keys
}

pub fn cmd_port_bfbc1(default_root: &Path, args: &[String]) -> Result<(), String> {
    let iso_path = flag_val(args, "--iso")
        .ok_or("usage: opfor-cli port-bfbc1 --iso <bfbc1.iso> --bfbc2 <dir> --opfor <dir> [--mode oasis|mp|sp|all] [--levels a,b] [--skip-streaming]")?;
    let bfbc2_dir = flag_val(args, "--bfbc2")
        .ok_or("missing --bfbc2 <dir>")?;
    let opfor_dir = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let mode = flag_str(args, "--mode").unwrap_or_else(|| "oasis".into());
    let levels_arg = flag_str(args, "--levels").unwrap_or_default();
    let skip_streaming = has_flag(args, "--skip-streaming");

    let xenolith = opfor_dir.join("utility").join("xenolith.exe");
    let bfbc2_exe = bfbc2_dir.join("BFBC2Game.exe");
    let mp_common = bfbc2_dir.join("Dist").join("win32").join("levels").join("mp_common").join("level-00.fbrb");
    let donor_fbrb = bfbc2_dir.join("Package").join("levels").join(DONOR_LEVEL).join("level-00.fbrb");
    if !xenolith.is_file() {
        return Err(format!("Missing {}", xenolith.display()));
    }
    if !bfbc2_exe.is_file() {
        return Err(format!("BFBC2Game.exe not found in {}", bfbc2_dir.display()));
    }
    if !donor_fbrb.is_file() {
        return Err(format!("Missing donor PC level: {}", donor_fbrb.display()));
    }

    let (_, donor_entries) = read_fbrb(&donor_fbrb)?;
    let donor_shaders: Vec<FbrbEntry> = donor_entries
        .iter()
        .filter(|e| matches!(e.ext.as_str(), "Dx9ShaderDatabase" | "Dx10ShaderDatabase" | "Dx11ShaderDatabase"))
        .map(|e| FbrbEntry {
            name: e.name.clone(),
            ext: e.ext.clone(),
            flags: e.flags,
            data: e.data.clone(),
        })
        .collect();
    let donor_shaders_key = format!("levels/{DONOR_LEVEL}/shaders.dbx");
    let donor_shaders_dbx = donor_entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(&donor_shaders_key))
        .map(|e| e.data.clone())
        .ok_or("Donor PC shaders.dbx missing")?;
    if donor_shaders.len() != 3 {
        return Err(format!("Expected 3 donor PC ShaderDBs, got {}", donor_shaders.len()));
    }

    let work_dir = opfor_dir.join("work").join("bfbc1");
    let raw_dir = work_dir.join("extracted");
    let mod_dir = opfor_dir.join("modules").join("bfbc1");
    fs::create_dir_all(&raw_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&mod_dir).map_err(|e| e.to_string())?;

    println!("[Stage 1/3] Indexing BFBC1 ISO (read-only): {}", iso_path.display());
    let iso_idx = index_iso(&iso_path)?;
    println!("  Indexed {} files on Xbox 360 disc.", iso_idx.files.len());

    let levels: Vec<String> = if !levels_arg.trim().is_empty() {
        levels_arg
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        match mode.as_str() {
            "oasis" => vec!["mp_beachhead_day1".into()],
            "mp" => MP_LEVELS.iter().map(|&s| s.to_owned()).collect(),
            "sp" => SP_LEVELS.iter().map(|&s| s.to_owned()).collect(),
            _ => MP_LEVELS.iter().chain(SP_LEVELS.iter()).map(|&s| s.to_owned()).collect(),
        }
    };

    let streaming_rel = "dist/xenon/streaming_sounds-00.fbrb";
    let streaming_path = raw_dir.join(streaming_rel.replace('/', "\\"));
    {
        let mut fp = File::open(&iso_path).map_err(|e| format!("open ISO: {e}"))?;
        if !skip_streaming
            && let Some(&(off, sz)) = iso_idx.files.get(streaming_rel)
            && !streaming_path.is_file()
        {
            if let Some(p) = streaming_path.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut out = File::create(&streaming_path).map_err(|e| e.to_string())?;
            copy_iso_range(&mut fp, off, sz, &mut out)?;
        }
        for lv in &levels {
            for sub in [
                "level-00.fbrb",
                "terrain-00.fbrb",
                "vo/en-00.fbrb",
                "ui/singleplayer-00.fbrb",
                "ui/multiplayer-00.fbrb",
            ] {
                let rel = format!("dist/xenon/levels/{lv}/{sub}");
                if let Some(&(off, sz)) = iso_idx.files.get(&rel) {
                    let dst = raw_dir.join(rel.replace('/', "\\"));
                    if !dst.is_file() || fs::metadata(&dst).map_or(0, |m| m.len()) != sz {
                        if let Some(p) = dst.parent() {
                            fs::create_dir_all(p).map_err(|e| e.to_string())?;
                        }
                        let mut out = File::create(&dst).map_err(|e| e.to_string())?;
                        copy_iso_range(&mut fp, off, sz, &mut out)?;
                    }
                }
            }
        }
    }

    let havok_tpl = work_dir.join("havok_550_master_template.res");
    if !havok_tpl.is_file() {
        println!("[Stage 2/3] Synthesizing PC Havok 5.5.0 master template from BFBC2...");
        build_havok_template(&bfbc2_dir, &havok_tpl)?;
    }

    let startup_fbrb = opfor_dir.join("cache").join("startup-00.fbrb");
    let pid = std::process::id();
    let tmp_port = std::env::temp_dir().join(format!("opfor_bfbc1_port_{pid}.fbrb"));
    let tmp_dedup = std::env::temp_dir().join(format!("opfor_bfbc1_dedup_{pid}.fbrb"));

    println!("[Stage 2/3] Porting {} BFBC1 level(s) via xenolith.exe...", levels.len());
    for (idx, lv) in levels.iter().enumerate() {
        let lv_mod_dir = mod_dir.join("levels").join(lv);
        fs::create_dir_all(&lv_mod_dir).map_err(|e| e.to_string())?;
        let raw_lvl = raw_dir.join("dist").join("xenon").join("levels").join(lv).join("level-00.fbrb");
        let raw_ter = raw_dir.join("dist").join("xenon").join("levels").join(lv).join("terrain-00.fbrb");

        println!("\n=== [{}/{}] {lv} ===", idx + 1, levels.len());
        if raw_ter.is_file() {
            let out_ter = run_xenolith(
                &xenolith,
                &[
                    "port-fbrb".into(),
                    "--bfbc1".into(),
                    raw_ter.display().to_string(),
                    lv_mod_dir.join("terrain-00.fbrb").display().to_string(),
                ],
            )?;
            println!("  terrain-00: {out_ter}");
        }

        for sub in ["vo/en-00.fbrb", "ui/singleplayer-00.fbrb", "ui/multiplayer-00.fbrb"] {
            let raw_sub = raw_dir
                .join("dist")
                .join("xenon")
                .join("levels")
                .join(lv)
                .join(sub.replace('/', "\\"));
            if raw_sub.is_file() {
                let dst_sub = lv_mod_dir.join(sub.replace('/', "\\"));
                if let Some(p) = dst_sub.parent() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                let out_sub = run_xenolith(
                    &xenolith,
                    &[
                        "port-fbrb".into(),
                        "--bfbc1".into(),
                        raw_sub.display().to_string(),
                        dst_sub.display().to_string(),
                    ],
                )?;
                println!("  {sub}: {out_sub}");
            }
        }

        if raw_lvl.is_file() {
            let is_sp = lv.starts_with("sp_");
            let mut desc_dbx: Option<Vec<u8>> = None;
            if startup_fbrb.is_file() {
                let donor = if is_sp { "SP_02" } else { "MP_012GR" };
                let reg_out = run_xenolith(
                    &xenolith,
                    &[
                        "register-level".into(),
                        startup_fbrb.display().to_string(),
                        lv.clone(),
                        donor.into(),
                    ],
                )?;
                println!("  startup: {reg_out}");
                let (_, st_entries) = read_fbrb(&startup_fbrb)?;
                let want = format!("levels/{lv}_description.dbx");
                desc_dbx = st_entries
                    .into_iter()
                    .find(|e| e.name.eq_ignore_ascii_case(&want))
                    .map(|e| e.data);
            }

            let out_lvl = run_xenolith(
                &xenolith,
                &[
                    "port-fbrb".into(),
                    "--bfbc1".into(),
                    "--havok-exe".into(),
                    bfbc2_exe.display().to_string(),
                    "--havok-template".into(),
                    havok_tpl.display().to_string(),
                    raw_lvl.display().to_string(),
                    tmp_port.display().to_string(),
                ],
            )?;
            println!("  level-00 port: {out_lvl}");
            let out_dedup = run_xenolith(
                &xenolith,
                &[
                    "dedup-fbrb".into(),
                    tmp_port.display().to_string(),
                    mp_common.display().to_string(),
                    tmp_dedup.display().to_string(),
                ],
            )?;
            println!("  level-00 dedup: {out_dedup}");

            let (ver, entries) = read_fbrb(&tmp_dedup)?;
            let target_shaders_key = format!("levels/{lv}/shaders.dbx");
            let target_desc_key = format!("levels/{lv}_description.dbx");
            let mut out_entries = Vec::with_capacity(entries.len() + donor_shaders.len());
            let mut stripped = 0usize;
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
                if e.name.eq_ignore_ascii_case(&target_shaders_key)
                    && let Some((tp, ti)) = extract_two_guids_via_xenolith(&xenolith, &e.data)
                {
                    let mut patched = donor_shaders_dbx.clone();
                    replace_bytes_all(&mut patched, DONOR_PART_GUID.to_ascii_uppercase().as_bytes(), tp.to_ascii_uppercase().as_bytes());
                    replace_bytes_all(&mut patched, DONOR_PART_GUID.to_ascii_lowercase().as_bytes(), tp.to_ascii_lowercase().as_bytes());
                    replace_bytes_all(&mut patched, DONOR_INST_GUID.to_ascii_uppercase().as_bytes(), ti.to_ascii_uppercase().as_bytes());
                    replace_bytes_all(&mut patched, DONOR_INST_GUID.to_ascii_lowercase().as_bytes(), ti.to_ascii_lowercase().as_bytes());
                    if let (Some(dp), Some(tp_b)) = (guid_to_bytes_be(DONOR_PART_GUID), guid_to_bytes_be(&tp)) {
                        replace_bytes_all(&mut patched, &dp, &tp_b);
                    }
                    if let (Some(dp), Some(tp_b)) = (guid_to_bytes_le(DONOR_PART_GUID), guid_to_bytes_le(&tp)) {
                        replace_bytes_all(&mut patched, &dp, &tp_b);
                    }
                    if let (Some(di), Some(ti_b)) = (guid_to_bytes_be(DONOR_INST_GUID), guid_to_bytes_be(&ti)) {
                        replace_bytes_all(&mut patched, &di, &ti_b);
                    }
                    if let (Some(di), Some(ti_b)) = (guid_to_bytes_le(DONOR_INST_GUID), guid_to_bytes_le(&ti)) {
                        replace_bytes_all(&mut patched, &di, &ti_b);
                    }
                    out_entries.push(FbrbEntry {
                        name: e.name,
                        ext: e.ext,
                        flags: e.flags,
                        data: patched,
                    });
                    continue;
                }
                if e.name.eq_ignore_ascii_case(&target_desc_key)
                    && let Some(ref dbytes) = desc_dbx
                {
                    out_entries.push(FbrbEntry {
                        name: e.name,
                        ext: e.ext,
                        flags: e.flags,
                        data: dbytes.clone(),
                    });
                    continue;
                }
                out_entries.push(e);
            }
            for s in &donor_shaders {
                out_entries.push(FbrbEntry {
                    name: s.name.clone(),
                    ext: s.ext.clone(),
                    flags: s.flags,
                    data: s.data.clone(),
                });
            }
            write_fbrb(&lv_mod_dir.join("level-00.fbrb"), ver, &out_entries, true)?;
            let _ = fs::remove_file(&tmp_port);
            let _ = fs::remove_file(&tmp_dedup);
            println!(
                "  level-00 final: {} entries (stripped_shaders={stripped})",
                out_entries.len()
            );
        }

        // Author native PC loader-00.fbrb (BFBC1 ported loading music + PC Bink fallback)
        // and ingame-00.fbrb (BFBC1 ported minimap ITexture + .dbx) instead of stock placeholders.
        if let Err(e) = author_bfbc1_loader_ingame(lv, &lv_mod_dir, &bfbc2_dir, &startup_fbrb, &xenolith) {
            println!("  warning: native loader/ingame authoring failed ({e}), falling back to donor copy");
            let donor_sub = if lv.starts_with("sp_") { "sp_02" } else { "mp_001" };
            let standalone_donor = bfbc2_dir.join("Dist").join("win32").join("levels").join(donor_sub);
            for sub in ["loader-00.fbrb", "ingame-00.fbrb"] {
                let dst_sub = lv_mod_dir.join(sub);
                let src_sub = standalone_donor.join(sub);
                if !dst_sub.is_file() && src_sub.is_file() {
                    fs::copy(&src_sub, &dst_sub).map_err(|e| e.to_string())?;
                }
            }
        }

        let bm_path = opfor_dir.join("cache").join("bundleManifest");
        if bm_path.is_file() {
            let text = fs::read_to_string(&bm_path).map_err(|e| e.to_string())?;
            let mut bm_lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
            let mut seen: HashSet<String> = bm_lines
                .iter()
                .filter_map(|l| l.split('\t').next().map(|k| k.trim().to_ascii_lowercase()))
                .filter(|k| !k.is_empty())
                .collect();
            let mut added = false;
            for (sub_key, fbrb_rel) in [
                ("level", "level-00.fbrb"),
                ("terrain", "terrain-00.fbrb"),
                ("ingame", "ingame-00.fbrb"),
                ("loader", "loader-00.fbrb"),
                ("ui/multiplayer", "ui/multiplayer-00.fbrb"),
                ("ui/singleplayer", "ui/singleplayer-00.fbrb"),
                ("vo/en", "vo/en-00.fbrb"),
            ] {
                if lv_mod_dir.join(fbrb_rel.replace('/', "\\")).is_file() {
                    let key = format!("levels/{lv}/{sub_key}");
                    if seen.insert(key.clone()) {
                        bm_lines.push(format!("{key}\t00000000-0000-0000-0000-000000000000\t1"));
                        added = true;
                    }
                }
            }
            if added {
                fs::write(&bm_path, format!("{}\n", bm_lines.join("\n"))).map_err(|e| e.to_string())?;
            }
        }
    }

    if !skip_streaming && streaming_path.is_file() {
        let mut all_raw_lvls = Vec::new();
        let lvls_dir = raw_dir.join("dist").join("xenon").join("levels");
        if let Ok(rd) = fs::read_dir(&lvls_dir) {
            for ent in rd.flatten() {
                let p = ent.path().join("level-00.fbrb");
                if p.is_file() {
                    all_raw_lvls.push(p);
                }
            }
        }
        all_raw_lvls.sort();
        let wanted = collect_referenced_sns_keys(&all_raw_lvls);
        println!(
            "\n[Stage 2/3] Filtering {} referenced streaming_sounds entries across {} levels...",
            wanted.len(),
            all_raw_lvls.len()
        );
        let (ver_s, ss_entries) = read_fbrb(&streaming_path)?;
        let filtered: Vec<FbrbEntry> = ss_entries
            .into_iter()
            .filter(|e| wanted.contains(&e.name.replace('\\', "/").to_ascii_lowercase()))
            .collect();
        let sub_sns = std::env::temp_dir().join(format!("opfor_filtered_sns_{pid}.fbrb"));
        write_fbrb(&sub_sns, ver_s, &filtered, false)?;
        let mut cmd: Vec<String> = vec!["port-streaming-sounds".into()];
        for lp in &all_raw_lvls {
            cmd.push("--waves".into());
            cmd.push(lp.display().to_string());
        }
        cmd.push(sub_sns.display().to_string());
        cmd.push(mod_dir.join("streaming_sounds-00.fbrb").display().to_string());
        let out_ss = run_xenolith(&xenolith, &cmd)?;
        let _ = fs::remove_file(&sub_sns);
        println!("  streaming_sounds-00: {out_ss}");
    }

    println!("\n[Stage 3/3] BFBC1 ({mode}) installed cleanly into {}", mod_dir.display());
    Ok(())
}

fn extract_primary_guid_via_xenolith(xenolith: &Path, dbx_bytes: &[u8]) -> Option<String> {
    let pid = std::process::id();
    let p_in = std::env::temp_dir().join(format!("opfor_pg_in_{pid}.dbx"));
    let p_xml = std::env::temp_dir().join(format!("opfor_pg_out_{pid}.xml"));
    fs::write(&p_in, dbx_bytes).ok()?;
    let ok = run_xenolith(
        xenolith,
        &["dbx-to-xml".into(), p_in.display().to_string(), p_xml.display().to_string()],
    )
    .is_ok();
    let _ = fs::remove_file(&p_in);
    if !ok {
        let _ = fs::remove_file(&p_xml);
        return None;
    }
    let xml = fs::read_to_string(&p_xml).ok()?;
    let _ = fs::remove_file(&p_xml);
    let pos = xml.find("primaryInstance=\"")?;
    let rest = &xml[pos + 17..];
    if rest.len() >= 36 && rest.as_bytes()[36] == b'"' {
        return Some(rest[..36].to_ascii_lowercase());
    }
    None
}

fn replace_xml_field_ref(xml: &str, field_name: &str, new_ref: &str) -> String {
    let needle = format!("\" name=\"{field_name}\"");
    let Some(end_idx) = xml.find(&needle) else {
        return xml.to_owned();
    };
    let prefix = &xml[..end_idx];
    let Some(start_idx) = prefix.rfind("ref=\"") else {
        return xml.to_owned();
    };
    let mut out = String::with_capacity(xml.len() + new_ref.len());
    out.push_str(&xml[..start_idx + 5]);
    out.push_str(new_ref);
    out.push_str(&xml[end_idx..]);
    out
}

fn author_bfbc1_loader_ingame(
    lv: &str,
    lv_mod_dir: &Path,
    bfbc2_dir: &Path,
    startup_fbrb: &Path,
    xenolith: &Path,
) -> Result<(), String> {
    let is_sp = lv.starts_with("sp_");
    let ui_bundle = lv_mod_dir
        .join("ui")
        .join(if is_sp { "singleplayer-00.fbrb" } else { "multiplayer-00.fbrb" });
    let level_bundle = lv_mod_dir.join("level-00.fbrb");

    // 1. Build native PC loader-00.fbrb from BFBC1's ported loading music + native loading media
    //    - MP: embed BFBC1's static loading screen textures (`Levels/<lv>/UI/*`) from ui/multiplayer-00.fbrb
    //          (zero BFBC2 donor BinkMemory movies!)
    //    - SP: embed SP_02 BinkMemory slot placeholder (replaced by native BFBC1 SP movie in `bfbc1-movies`)
    let mut music_ref: Option<String> = None;
    let mut mp_screens_block: Option<String> = None;
    let mut loader_entries: Vec<FbrbEntry> = Vec::new();
    if ui_bundle.is_file() {
        let (_, ui_ents) = read_fbrb(&ui_bundle)?;
        let wave_opt = ui_ents
            .iter()
            .find(|e| e.ext == "Wave" && e.name.to_ascii_lowercase().contains("load"))
            .or_else(|| ui_ents.iter().find(|e| e.ext == "Wave" && e.name.to_ascii_lowercase().contains("music")));
        if let Some(w) = wave_opt {
            let stem = w.name[..w.name.len().saturating_sub(4)].to_ascii_lowercase();
            let dbx_want = format!("{stem}.dbx");
            loader_entries.push(FbrbEntry {
                name: w.name.clone(),
                ext: w.ext.clone(),
                flags: 0x0001_0000,
                data: w.data.clone(),
            });
            if let Some(wd) = ui_ents.iter().find(|e| e.name.eq_ignore_ascii_case(&dbx_want)) {
                if let Some(g) = extract_primary_guid_via_xenolith(xenolith, &wd.data) {
                    music_ref = Some(format!("{stem}/{g}"));
                }
                loader_entries.push(FbrbEntry {
                    name: wd.name.clone(),
                    ext: wd.ext.clone(),
                    flags: 0x0001_0000,
                    data: wd.data.clone(),
                });
            }
        }
        if !is_sp {
            let ui_prefix = format!("levels/{lv}/ui/");
            for e in &ui_ents {
                let nl = e.name.to_ascii_lowercase();
                if nl.starts_with(&ui_prefix) && (e.ext == "ITexture" || nl.ends_with(".dbx")) {
                    loader_entries.push(e.clone());
                }
            }
            let desc_want = format!("levels/{lv}_description.dbx");
            if let Some(d_ent) = ui_ents.iter().find(|e| e.name.eq_ignore_ascii_case(&desc_want)) {
                let pid = std::process::id();
                let p_in = std::env::temp_dir().join(format!("opfor_mpdesc_in_{pid}.dbx"));
                let p_xml = std::env::temp_dir().join(format!("opfor_mpdesc_out_{pid}.xml"));
                if fs::write(&p_in, &d_ent.data).is_ok()
                    && run_xenolith(
                        xenolith,
                        &["dbx-to-xml".into(), p_in.display().to_string(), p_xml.display().to_string()],
                    )
                    .is_ok()
                    && let Ok(xml) = fs::read_to_string(&p_xml)
                    && let Some(s) = xml.find("<array name=\"LoadingScreens\">")
                    && let Some(e_rel) = xml[s..].find("</array>")
                {
                    mp_screens_block = Some(xml[s..s + e_rel + "</array>".len()].to_owned());
                }
                let _ = fs::remove_file(&p_in);
                let _ = fs::remove_file(&p_xml);
            }
        }
    }
    if is_sp {
        let donor_loader = bfbc2_dir
            .join("Dist")
            .join("win32")
            .join("levels")
            .join("sp_02")
            .join("loader-00.fbrb");
        if donor_loader.is_file() {
            let (_, d_ents) = read_fbrb(&donor_loader)?;
            if let Some(bink) = d_ents.into_iter().find(|e| e.ext == "BinkMemory") {
                loader_entries.push(bink);
            }
        }
    }
    if !loader_entries.is_empty() {
        write_fbrb(&lv_mod_dir.join("loader-00.fbrb"), 2, &loader_entries, true)?;
    }

    // 2. Build native PC ingame-00.fbrb from BFBC1's ported minimap ITexture + .dbx
    let mut minimap_ref: Option<String> = None;
    let mut ingame_entries: Vec<FbrbEntry> = Vec::new();
    if level_bundle.is_file() {
        let (_, lv_ents) = read_fbrb(&level_bundle)?;
        if let Some(mm) = lv_ents.iter().find(|e| {
            let nl = e.name.to_ascii_lowercase();
            e.ext == "ITexture" && nl.contains("minimap") && !nl.contains("tile")
        }) {
            let stem = mm.name[..mm.name.len().saturating_sub(4)].to_ascii_lowercase();
            let dbx_want = format!("{stem}.dbx");
            ingame_entries.push(FbrbEntry {
                name: mm.name.clone(),
                ext: mm.ext.clone(),
                flags: 0x0001_0000,
                data: mm.data.clone(),
            });
            let mm_dbx = lv_ents.iter().find(|e| e.name.eq_ignore_ascii_case(&dbx_want)).or_else(|| {
                lv_ents.iter().find(|e| {
                    let nl = e.name.to_ascii_lowercase();
                    nl.ends_with(".dbx") && nl.contains("minimap") && !nl.contains("tile") && !nl.contains("mesh")
                })
            });
            if let Some(md) = mm_dbx {
                let mstem = md.name[..md.name.len().saturating_sub(4)].to_ascii_lowercase();
                if let Some(g) = extract_primary_guid_via_xenolith(xenolith, &md.data) {
                    minimap_ref = Some(format!("{mstem}/{g}"));
                }
                ingame_entries.push(FbrbEntry {
                    name: md.name.clone(),
                    ext: md.ext.clone(),
                    flags: 0x0001_0000,
                    data: md.data.clone(),
                });
            }
        }
    }
    if !ingame_entries.is_empty() {
        write_fbrb(&lv_mod_dir.join("ingame-00.fbrb"), 2, &ingame_entries, true)?;
    }

    // 3. Patch Music, MinimapTexture & MP LoadingScreens refs in startup-00.fbrb's levels/<lv>_description.dbx
    if startup_fbrb.is_file() && (music_ref.is_some() || minimap_ref.is_some() || mp_screens_block.is_some()) {
        let desc_key = format!("levels/{lv}_description.dbx");
        let (ver, mut st_ents) = read_fbrb(startup_fbrb)?;
        let mut patched = false;
        let pid = std::process::id();
        for e in &mut st_ents {
            if !e.name.eq_ignore_ascii_case(&desc_key) {
                continue;
            }
            let p_in = std::env::temp_dir().join(format!("opfor_desc_in_{pid}.dbx"));
            let p_xml = std::env::temp_dir().join(format!("opfor_desc_out_{pid}.xml"));
            let p_out = std::env::temp_dir().join(format!("opfor_desc_new_{pid}.dbx"));
            if fs::write(&p_in, &e.data).is_ok()
                && run_xenolith(
                    xenolith,
                    &["dbx-to-xml".into(), p_in.display().to_string(), p_xml.display().to_string()],
                )
                .is_ok()
                && let Ok(mut xml) = fs::read_to_string(&p_xml)
            {
                let orig = xml.clone();
                if let Some(ref mr) = music_ref {
                    xml = replace_xml_field_ref(&xml, "Music", mr);
                }
                if let Some(ref mmr) = minimap_ref {
                    xml = replace_xml_field_ref(&xml, "MinimapTexture", mmr);
                }
                if let Some(ref sb) = mp_screens_block {
                    if let Some(s) = xml.find("<array name=\"LoadingScreens\"") {
                        if let Some(rel_sc) = xml[s..].find("/>")
                            && xml[s..s + rel_sc].find('>').is_none()
                        {
                            xml.replace_range(s..s + rel_sc + 2, sb);
                        } else if let Some(rel_end) = xml[s..].find("</array>") {
                            xml.replace_range(s..s + rel_end + "</array>".len(), sb);
                        }
                    }
                    xml = replace_xml_field_ref(&xml, "LoadingMovie", "null");
                }
                if xml != orig
                    && fs::write(&p_xml, &xml).is_ok()
                    && run_xenolith(
                        xenolith,
                        &["xml-to-dbx".into(), p_xml.display().to_string(), p_out.display().to_string()],
                    )
                    .is_ok()
                    && let Ok(new_bytes) = fs::read(&p_out)
                {
                    e.data = new_bytes;
                    patched = true;
                }
            }
            let _ = fs::remove_file(&p_in);
            let _ = fs::remove_file(&p_xml);
            let _ = fs::remove_file(&p_out);
        }
        if patched {
            write_fbrb(startup_fbrb, ver, &st_ents, true)?;
        }
    }
    println!(
        "  loader/ingame: authored native BFBC1 PC bundles (loader={} entries, ingame={} entries)",
        loader_entries.len(),
        ingame_entries.len()
    );
    Ok(())
}
