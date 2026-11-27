//! Pure-Rust Stock-MP Bots Layer & Onslaught AI Fix Pipeline for `opfor-cli`.
//!
//! Eliminates all Python / external tool dependencies by operating directly in memory on
//! Frostbite `FbRB` bundles, binary `DBX` partitions, and Kynogon `HPD` v2 navmeshes via
//! `x360-frostbite`:
//! - `apply-ai-template`: Packs DICE's campaign grunt template chain from `sp_common` (when needed)
//!   and replaces broken `DefaultSoldier` templates on `SSAiPlayer` spawners.
//! - `apply-sector-offsets`: Computes minimum `(sector_x, sector_z)` from a Kynogon HPD v2
//!   navmesh and injects `Levels/<LEVEL>/Precomputed/<Layer>/SectorOffsets.dbx`.
//! - `author-stockmp-ai`: Non-destructively authors a Stock-MP Bots Layer overlay into
//!   `modules/stockmp/levels/<lv>/level-00.fbrb` (Option B VFS; stock `engine/` untouched):
//!   sets `HasAI = true`, packs the grunt template chain, places `SSAiPlayer` spawners for
//!   Team 1 & Team 2 near existing spawns, and creates `AreaOrderEntityData` at objectives/flags.
#![deny(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use x360_frostbite::bfbc1::build_sector_offsets_dbx;
use x360_frostbite::dbx;

use crate::bfbc1::{FbrbEntry, flag_val, read_fbrb, write_fbrb};
use crate::era_parity::deterministic_guid;

const GRUNT_TEMPLATE_REF: &str =
    "objects/soldiers/templates/spawntemplate_enemy_grunt/2eedbef1-52de-4b20-819c-e71097c15cbe";
const DEFAULT_SOLDIER_PREFIX: &str = "objects/soldiers/templates/defaultsoldier/";

const GRUNT_CHAIN_PATHS: &[&str] = &[
    "Objects/Soldiers/Templates/SpawnTemplate_Enemy_Grunt.dbx",
    "Objects/Soldiers/Templates/AI/Enemy_Grunt_Sense.dbx",
    "Objects/Soldiers/Templates/AI/Enemy_Grunt_Bhv.dbx",
];

fn flag_str(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}

/// Read minimum `(sector_x, sector_z)` from a Frostbite 1.5 Kynogon HPD v2 navmesh header.
pub fn compute_hpd_v2_sector_offsets(hpd_bytes: &[u8]) -> Result<(i32, i32), String> {
    const HPD_V2_HDR_LEN: usize = 0x9C;
    const SECTOR_ENTRY_LEN: usize = 48;

    if hpd_bytes.len() < HPD_V2_HDR_LEN || !hpd_bytes.starts_with(b"Kynogon HPD\0") {
        return Err("not a Frostbite 1.5 Kynogon HPD v2 navmesh".into());
    }
    let sector_count = u32::from_le_bytes(hpd_bytes[0x14..0x18].try_into().unwrap()) as usize;
    if sector_count == 0 {
        return Ok((0, 0));
    }
    let tbl_end = HPD_V2_HDR_LEN + sector_count * SECTOR_ENTRY_LEN;
    if tbl_end > hpd_bytes.len() {
        return Err(format!("truncated HPD v2 sector table ({sector_count} sectors)"));
    }
    let mut min_x = i32::MAX;
    let mut min_z = i32::MAX;
    for i in 0..sector_count {
        let off = HPD_V2_HDR_LEN + i * SECTOR_ENTRY_LEN;
        let sx = i32::from_le_bytes(hpd_bytes[off..off + 4].try_into().unwrap());
        let sz = i32::from_le_bytes(hpd_bytes[off + 4..off + 8].try_into().unwrap());
        min_x = min_x.min(sx);
        min_z = min_z.min(sz);
    }
    Ok((min_x, min_z))
}

fn load_grunt_chain(opfor: &Path) -> Result<Vec<FbrbEntry>, String> {
    let sp_common = opfor
        .join("engine")
        .join("Dist")
        .join("win32")
        .join("levels")
        .join("sp_common")
        .join("level-00.fbrb");
    if !sp_common.is_file() {
        return Err(format!("sp_common not found at {}", sp_common.display()));
    }
    let (_, entries) = read_fbrb(&sp_common)?;
    let mut out = Vec::with_capacity(GRUNT_CHAIN_PATHS.len());
    for &want in GRUNT_CHAIN_PATHS {
        let e = entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(want))
            .ok_or_else(|| format!("missing {want} in {}", sp_common.display()))?;
        out.push(e.clone());
    }
    Ok(out)
}

fn upsert_entry(entries: &mut Vec<FbrbEntry>, item: FbrbEntry) {
    if let Some(existing) = entries.iter_mut().find(|e| e.name.eq_ignore_ascii_case(&item.name)) {
        *existing = item;
    } else {
        entries.push(item);
    }
}

/// Swap `objects/soldiers/templates/defaultsoldier/...` -> DICE campaign grunt template on all
/// `SSAiPlayer` `SoldierSpawnEntityData` instances in `xml`. Returns `(new_xml, swapped_count)`.
pub fn swap_default_soldier_templates_xml(xml: &str) -> (String, usize) {
    if !xml.to_ascii_lowercase().contains(DEFAULT_SOLDIER_PREFIX) {
        return (xml.to_owned(), 0);
    }
    let mut out = String::with_capacity(xml.len() + 256);
    let mut swapped = 0usize;
    let mut in_spawner = false;
    let mut spawner_buf = String::new();

    for line in xml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<instance ") && trimmed.contains("SoldierSpawnEntityData") {
            in_spawner = true;
            spawner_buf.clear();
            spawner_buf.push_str(line);
            spawner_buf.push('\n');
            if trimmed.trim_end().ends_with("</instance>") {
                in_spawner = false;
                let (patched, n) = patch_spawner_instance(&spawner_buf);
                swapped += n;
                out.push_str(&patched);
            }
            continue;
        }
        if in_spawner {
            spawner_buf.push_str(line);
            spawner_buf.push('\n');
            if trimmed.trim_end().ends_with("</instance>") {
                in_spawner = false;
                let (patched, n) = patch_spawner_instance(&spawner_buf);
                swapped += n;
                out.push_str(&patched);
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, swapped)
}

fn patch_spawner_instance(inst: &str) -> (String, usize) {
    if !inst.contains("SSAiPlayer") {
        return (inst.to_owned(), 0);
    }
    let mut out = String::with_capacity(inst.len() + 64);
    let mut swapped = 0usize;
    for line in inst.lines() {
        let low = line.to_ascii_lowercase();
        if low.contains("name=\"template\"")
            && let Some(pos) = low.find(DEFAULT_SOLDIER_PREFIX)
            && let Some(end_rel) = line[pos..].find('"')
        {
            out.push_str(&line[..pos]);
            out.push_str(GRUNT_TEMPLATE_REF);
            out.push_str(&line[pos + end_rel..]);
            out.push('\n');
            swapped += 1;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, swapped)
}

/// Set `<field name="HasAI">true</field>` on `LevelData` inside a level partition XML.
pub fn enable_level_has_ai_xml(xml: &str) -> (String, bool) {
    if xml.contains("<field name=\"HasAI\">false</field>") {
        return (
            xml.replacen("<field name=\"HasAI\">false</field>", "<field name=\"HasAI\">true</field>", 1),
            true,
        );
    }
    if xml.contains("<field name=\"HasAI\">0</field>") {
        return (
            xml.replacen("<field name=\"HasAI\">0</field>", "<field name=\"HasAI\">true</field>", 1),
            true,
        );
    }
    (xml.to_owned(), false)
}

/// Extract `(team_id, transform_xml_block)` from existing `SoldierSpawnEntityData` instances,
/// and `transform_xml_block` from `CapturePointEntityData` / `MissionObjectiveEntityData`.
fn extract_transforms(xml: &str, type_substr: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut cur: Option<String> = None;
    for line in xml.lines() {
        let t = line.trim_start();
        if t.starts_with("<instance ") && t.contains(type_substr) {
            cur = Some(format!("{line}\n"));
            if t.trim_end().ends_with("</instance>")
                && let Some(inst) = cur.take()
                && let Some(pair) = parse_team_and_transform(&inst)
            {
                results.push(pair);
            }
            continue;
        }
        if let Some(ref mut buf) = cur {
            buf.push_str(line);
            buf.push('\n');
            if t.trim_end().ends_with("</instance>") {
                let inst = cur.take().unwrap_or_default();
                if let Some(pair) = parse_team_and_transform(&inst) {
                    results.push(pair);
                }
            }
        }
    }
    results
}

fn parse_team_and_transform(inst: &str) -> Option<(String, String)> {
    let team = inst
        .lines()
        .find(|l| l.contains("name=\"Team\""))
        .and_then(|l| l.split('>').nth(1))
        .and_then(|s| s.split('<').next())
        .unwrap_or("Team1")
        .trim()
        .to_owned();
    let start = inst.find("<complex name=\"Transform\">")?;
    let rest = &inst[start..];
    let end = rest.find("</complex>")? + "</complex>".len();
    Some((team, rest[..end].to_owned()))
}

fn build_ai_spawner_instance(guid: &str, team: &str, transform_xml: &str) -> String {
    format!(
        "\t<instance guid=\"{guid}\" type=\"GameSharedResources.SoldierSpawnEntityData\">\n\
         \t\t<field name=\"Enabled\">true</field>\n\
         \t\t<field name=\"Team\">{team}</field>\n\
         \t\t<field name=\"PlayerType\">SSAiPlayer</field>\n\
         \t\t<field name=\"SpawnDelay\">10</field>\n\
         \t\t<field name=\"MaxCount\">1</field>\n\
         \t\t<field name=\"MaxCountSimultaneously\">1</field>\n\
         \t\t<field ref=\"{GRUNT_TEMPLATE_REF}\" name=\"Template\" />\n\
         \t\t{transform_xml}\n\
         \t</instance>\n"
    )
}

fn build_area_order_instance(guid: &str, team: &str, transform_xml: &str) -> String {
    format!(
        "\t<instance guid=\"{guid}\" type=\"GameSharedResources.AreaOrderEntityData\">\n\
         \t\t<field name=\"Enabled\">true</field>\n\
         \t\t<field name=\"Team\">{team}</field>\n\
         \t\t<field name=\"Radius\">25</field>\n\
         \t\t{transform_xml}\n\
         \t</instance>\n"
    )
}

/// Inject `new_instances` before `</partition>` and append their `<item ref="..." />` entries
/// to `LevelData`'s `<array name="Objects">` container.
pub fn inject_level_ai_entities_xml(xml: &str, level_name: &str, bots_per_team: usize) -> (String, usize, usize) {
    let (mut out, _) = enable_level_has_ai_xml(xml);
    let (swapped_xml, _) = swap_default_soldier_templates_xml(&out);
    out = swapped_xml;

    let spawns = extract_transforms(&out, "SoldierSpawnEntityData");
    let objectives = extract_transforms(&out, "CapturePointEntityData");
    let mcoms = extract_transforms(&out, "MissionObjectiveEntityData");
    let all_objs: Vec<(String, String)> = objectives.into_iter().chain(mcoms).collect();

    let mut extra_instances = String::new();
    let mut new_refs: Vec<String> = Vec::new();
    let mut spawner_count = 0usize;
    let mut order_count = 0usize;

    for team in ["Team1", "Team2"] {
        let team_spawns: Vec<&str> = spawns
            .iter()
            .filter(|(t, _)| t.eq_ignore_ascii_case(team))
            .map(|(_, tr)| tr.as_str())
            .collect();
        if team_spawns.is_empty() {
            continue;
        }
        for i in 0..bots_per_team {
            let tr = team_spawns[i % team_spawns.len()];
            let g = deterministic_guid(&format!("stockmp:{level_name}:spawner:{team}:{i}"));
            if !out.contains(&g) {
                extra_instances.push_str(&build_ai_spawner_instance(&g, team, tr));
                new_refs.push(g);
                spawner_count += 1;
            }
        }
        for (oi, (_, tr)) in all_objs.iter().enumerate() {
            let g = deterministic_guid(&format!("stockmp:{level_name}:order:{team}:{oi}"));
            if !out.contains(&g) {
                extra_instances.push_str(&build_area_order_instance(&g, team, tr));
                new_refs.push(g);
                order_count += 1;
            }
        }
    }

    if new_refs.is_empty() {
        return (out, 0, 0);
    }

    if let Some(arr_pos) = out.find("<array name=\"Objects\">")
        && let Some(end_rel) = out[arr_pos..].find("</array>")
    {
        let insert_at = arr_pos + end_rel;
        let mut ref_block = String::with_capacity(new_refs.len() * 56);
        for g in &new_refs {
            ref_block.push_str(&format!("\t\t\t<item ref=\"{g}\" />\n"));
        }
        out.insert_str(insert_at, &ref_block);
    }
    if let Some(part_end) = out.rfind("</partition>") {
        out.insert_str(part_end, &extra_instances);
    }
    (out, spawner_count, order_count)
}

/// `opfor-cli apply-ai-template --opfor <dir> --bundle <level-00.fbrb> [--pack-grunt]`
pub fn cmd_apply_ai_template(default_root: &Path, args: &[String]) -> Result<(), String> {
    let opfor = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let bundle = flag_val(args, "--bundle")
        .ok_or("usage: opfor-cli apply-ai-template --bundle <level-00.fbrb> [--opfor <dir>] [--pack-grunt]")?;
    let pack_grunt = args.iter().any(|a| a == "--pack-grunt");

    let (ver, mut entries) = read_fbrb(&bundle)?;
    if pack_grunt {
        let chain = load_grunt_chain(&opfor)?;
        for item in chain {
            upsert_entry(&mut entries, item);
        }
    }

    let mut total_swapped = 0usize;
    for e in &mut entries {
        if !e.name.to_ascii_lowercase().ends_with(".dbx") || e.data.is_empty() {
            continue;
        }
        if let Ok(parsed) = dbx::parse(&e.data) {
            let xml = parsed.to_xml();
            let (patched, n) = swap_default_soldier_templates_xml(&xml);
            if n > 0 {
                e.data = dbx::from_xml(&patched).map_err(|err| err.to_string())?.to_bytes();
                total_swapped += n;
            }
        }
    }
    write_fbrb(&bundle, ver, &entries, true)?;
    println!(
        "[stockmp-ai] {}: swapped {total_swapped} DefaultSoldier spawner(s) -> campaign grunt (pack_grunt={pack_grunt})",
        bundle.display()
    );
    Ok(())
}

/// `opfor-cli apply-sector-offsets --bundle <level-00.fbrb> --level <LEVEL> --hpd <PathData.hpd> [--layer Infantry|Vehicle]`
pub fn cmd_apply_sector_offsets(args: &[String]) -> Result<(), String> {
    let bundle = flag_val(args, "--bundle").ok_or(
        "usage: opfor-cli apply-sector-offsets --bundle <level-00.fbrb> --level <LEVEL> --hpd <PathData.hpd> [--layer Infantry|Vehicle]",
    )?;
    let level = flag_str(args, "--level").ok_or("missing --level <LEVEL>")?;
    let hpd_path = flag_val(args, "--hpd").ok_or("missing --hpd <PathData.hpd>")?;
    let layer = flag_str(args, "--layer").unwrap_or_else(|| "Infantry".into());

    let hpd_bytes = fs::read(&hpd_path).map_err(|e| format!("read {}: {e}", hpd_path.display()))?;
    let (sx, sz) = compute_hpd_v2_sector_offsets(&hpd_bytes)?;
    let dbx_bytes = build_sector_offsets_dbx(&level, &layer, sx, sz);

    let (ver, mut entries) = read_fbrb(&bundle)?;
    let entry_name = format!("Levels/{level}/Precomputed/{layer}/SectorOffsets.dbx");
    upsert_entry(
        &mut entries,
        FbrbEntry {
            name: entry_name.clone(),
            ext: "<non-resource>".into(),
            flags: 0x0001_0000,
            data: dbx_bytes,
        },
    );
    write_fbrb(&bundle, ver, &entries, true)?;
    println!(
        "[stockmp-ai] Injected {entry_name} (offset=({sx},{sz})) into {}",
        bundle.display()
    );
    Ok(())
}

/// `opfor-cli author-stockmp-ai --level <mp_002> [--opfor <dir>] [--bots-per-team 8]`
pub fn cmd_author_stockmp_ai(default_root: &Path, args: &[String]) -> Result<(), String> {
    let opfor = flag_val(args, "--opfor").unwrap_or_else(|| default_root.to_path_buf());
    let level = flag_str(args, "--level")
        .ok_or("usage: opfor-cli author-stockmp-ai --level <mp_002> [--opfor <dir>] [--bots-per-team 8]")?;
    let bots_per_team = flag_str(args, "--bots-per-team")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(1, 32);

    let lv_lower = level.to_ascii_lowercase();
    let src_bundle = [
        opfor.join("engine").join("Package").join("levels").join(&lv_lower).join("level-00.fbrb"),
        opfor.join("engine").join("Dist").join("win32").join("levels").join(&lv_lower).join("level-00.fbrb"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .ok_or_else(|| format!("stock level-00.fbrb not found for `{level}` under engine/"))?;

    let (ver, mut entries) = read_fbrb(&src_bundle)?;
    for item in load_grunt_chain(&opfor)? {
        upsert_entry(&mut entries, item);
    }

    let lv_dbx_want = format!("levels/{lv_lower}.dbx");
    let mut spawners_added = 0usize;
    let mut orders_added = 0usize;
    for e in &mut entries {
        if !e.name.eq_ignore_ascii_case(&lv_dbx_want) {
            continue;
        }
        let xml = dbx::parse(&e.data).map_err(|err| err.to_string())?.to_xml();
        let (patched, sp, ord) = inject_level_ai_entities_xml(&xml, &lv_lower, bots_per_team);
        e.data = dbx::from_xml(&patched).map_err(|err| err.to_string())?.to_bytes();
        spawners_added = sp;
        orders_added = ord;
    }

    let mod_dir = opfor.join("modules").join("stockmp");
    let mod_toml = mod_dir.join("module.toml");
    if !mod_toml.is_file() {
        fs::create_dir_all(&mod_dir).map_err(|e| e.to_string())?;
        fs::write(
            &mod_toml,
            "id = \"stockmp\"\nname = \"Stock MP Bots Layer\"\nenabled = true\nera_order = 90\nmatchmaking_mod_id = \"\"\n",
        )
        .map_err(|e| e.to_string())?;
    }
    let dst_bundle: PathBuf = mod_dir.join("levels").join(&lv_lower).join("level-00.fbrb");
    write_fbrb(&dst_bundle, ver, &entries, true)?;
    println!(
        "[stockmp-ai] Authored {} (HasAI=true, +{spawners_added} AI spawners, +{orders_added} AreaOrders)",
        dst_bundle.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_default_soldier_template_on_ai_spawners() {
        let xml = "\
<partition guid=\"1\">\n\
\t<instance guid=\"2\" type=\"GameSharedResources.SoldierSpawnEntityData\">\n\
\t\t<field name=\"PlayerType\">SSAiPlayer</field>\n\
\t\t<field ref=\"objects/soldiers/templates/defaultsoldier/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee\" name=\"Template\" />\n\
\t</instance>\n\
</partition>\n";
        let (out, n) = swap_default_soldier_templates_xml(xml);
        assert_eq!(n, 1);
        assert!(out.contains(GRUNT_TEMPLATE_REF));
    }

    #[test]
    fn authors_stockmp_ai_entities_and_sets_has_ai() {
        let xml = "\
<partition guid=\"1\">\n\
\t<instance guid=\"10\" type=\"GameSharedResources.LevelData\">\n\
\t\t<field name=\"HasAI\">false</field>\n\
\t\t<array name=\"Objects\">\n\
\t\t\t<item ref=\"20\" />\n\
\t\t</array>\n\
\t</instance>\n\
\t<instance guid=\"20\" type=\"GameSharedResources.SoldierSpawnEntityData\">\n\
\t\t<field name=\"Team\">Team1</field>\n\
\t\t<complex name=\"Transform\"><field name=\"trans\">1/2/3</field></complex>\n\
\t</instance>\n\
\t<instance guid=\"21\" type=\"GameSharedResources.SoldierSpawnEntityData\">\n\
\t\t<field name=\"Team\">Team2</field>\n\
\t\t<complex name=\"Transform\"><field name=\"trans\">4/5/6</field></complex>\n\
\t</instance>\n\
\t<instance guid=\"30\" type=\"GameSharedResources.CapturePointEntityData\">\n\
\t\t<field name=\"Team\">TeamNeutral</field>\n\
\t\t<complex name=\"Transform\"><field name=\"trans\">7/8/9</field></complex>\n\
\t</instance>\n\
</partition>\n";
        let (out, sp, ord) = inject_level_ai_entities_xml(xml, "mp_002", 2);
        assert!(out.contains("<field name=\"HasAI\">true</field>"));
        assert_eq!(sp, 4);
        assert_eq!(ord, 2);
        assert!(out.contains("GameSharedResources.AreaOrderEntityData"));
        assert!(out.contains(GRUNT_TEMPLATE_REF));
    }

    #[test]
    fn computes_hpd_v2_min_sector_offsets() {
        let mut hpd = vec![0u8; 0x9C + 2 * 48];
        hpd[..12].copy_from_slice(b"Kynogon HPD\0");
        hpd[0x14..0x18].copy_from_slice(&2u32.to_le_bytes());
        hpd[0x9C..0xA0].copy_from_slice(&(-3i32).to_le_bytes());
        hpd[0xA0..0xA4].copy_from_slice(&(-7i32).to_le_bytes());
        hpd[0x9C + 48..0xA0 + 48].copy_from_slice(&(-1i32).to_le_bytes());
        hpd[0xA0 + 48..0xA4 + 48].copy_from_slice(&(-2i32).to_le_bytes());
        assert_eq!(compute_hpd_v2_sector_offsets(&hpd).unwrap(), (-3, -7));
    }
}
