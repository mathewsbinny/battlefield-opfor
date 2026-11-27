//! Pure-Rust extractor for `config/opfor_console_aimer.ini` from user-owned PC & Xbox 360
//! `DefaultHaloAimer.dbx` (or directly from `mp_common/level-00.fbrb` or decoded `.xml`).
//!
//! Each `[aimerN]` section records the PC sentinel values (`pc_*`) checked by `opfor-host/src/padaimer.rs`
//! before patching live `HaloAimerData` heap objects, followed by the console aim-assist and
//! stick-response tuning values.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use x360_frostbite::dbx;

use crate::bfbc1::read_fbrb;

const HALO_AIMER_DBX_SUFFIX: &str = "objects/soldiers/common/defaulthaloaimer.dbx";

const PC_CHECK_KEYS: &[&str] = &["YawSpeed", "PitchSpeed", "InputPolynomial"];

const CONSOLE_KEYS: &[&str] = &[
    // Floats
    "AccelerationInputThreshold",
    "AccelerationMultiplier",
    "AccelerationDamping",
    "YawSpeed",
    "PitchSpeed",
    "SuckSoftZone",
    "SuckUserInputMultiplier",
    "SuckOwnSpeedInfluence",
    "SuckTargetSpeedInfluence",
    "SuckOwnRequiredMovementForMaximumSuck",
    "SuckStartInputThreshold",
    "SuckZoomingMultiplier",
    "SuckZoomingPostTime",
    "SuckYaw",
    "SuckPitch",
    // Arrays
    "InputPolynomial",
    "ZoomedInputPolynomial",
    "SuckDistanceFallOff",
    // Bools
    "UseYawAcceleration",
    "UsePitchAcceleration",
];

#[derive(Debug, Clone)]
struct HaloAimerInstance {
    guid: String,
    fields: HashMap<String, String>,
}

/// `opfor-cli extract-console-aimer --pc <file> --xbox <file> [--out <config/opfor_console_aimer.ini>]`
///
/// Accepts `.fbrb` (extracts `Objects/Soldiers/Common/DefaultHaloAimer.dbx` in memory),
/// binary `.dbx`, or decoded `.xml` for both `--pc` and `--xbox`.
pub fn cmd_extract_console_aimer(root: &Path, args: &[String]) -> Result<(), String> {
    let mut pc_path: Option<PathBuf> = None;
    let mut xbox_path: Option<PathBuf> = None;
    let mut out_path = root.join("config").join("opfor_console_aimer.ini");

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--pc" if i + 1 < args.len() => {
                pc_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--xbox" if i + 1 < args.len() => {
                xbox_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--out" if i + 1 < args.len() => {
                out_path = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            other => return Err(format!("unknown extract-console-aimer arg: {other}")),
        }
    }

    let pc_path = pc_path.ok_or(
        "usage: opfor-cli extract-console-aimer --pc <level-00.fbrb|DefaultHaloAimer.dbx|.xml> --xbox <level-00.fbrb|DefaultHaloAimer.dbx|.xml> [--out <path>]",
    )?;
    let xbox_path = xbox_path.ok_or("missing --xbox <path>")?;

    let pc_xml = load_halo_aimer_xml(&pc_path)?;
    let xbox_xml = load_halo_aimer_xml(&xbox_path)?;
    let ini = build_console_aimer_ini(&pc_xml, &xbox_xml)?;

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    fs::write(&out_path, ini.as_bytes())
        .map_err(|e| format!("write {}: {e}", out_path.display()))?;

    let count = parse_halo_aimer_instances(&pc_xml).len();
    println!("Wrote {count} console HaloAimerData profiles -> {}", out_path.display());
    Ok(())
}

fn load_halo_aimer_xml(path: &Path) -> Result<String, String> {
    let raw = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "xml" => Ok(String::from_utf8_lossy(&raw).into_owned()),
        "dbx" => {
            let doc = dbx::parse(&raw).map_err(|e| format!("dbx parse {}: {e}", path.display()))?;
            Ok(doc.to_xml())
        }
        "fbrb" => load_halo_aimer_from_fbrb(path),
        _ => {
            // Auto-detect: XML starts with '<', FbRB starts with "FbRB", otherwise try DBX.
            if raw.starts_with(b"<") || raw.starts_with(b"\xEF\xBB\xBF<") {
                Ok(String::from_utf8_lossy(&raw).into_owned())
            } else if raw.starts_with(b"FbRB") {
                load_halo_aimer_from_fbrb(path)
            } else {
                let doc =
                    dbx::parse(&raw).map_err(|e| format!("dbx parse {}: {e}", path.display()))?;
                Ok(doc.to_xml())
            }
        }
    }
}

fn load_halo_aimer_from_fbrb(path: &Path) -> Result<String, String> {
    let (_, entries) = read_fbrb(path)?;
    let entry = entries
        .iter()
        .find(|e| {
            e.name
                .replace('\\', "/")
                .to_ascii_lowercase()
                .ends_with(HALO_AIMER_DBX_SUFFIX)
        })
        .ok_or_else(|| format!("{HALO_AIMER_DBX_SUFFIX} not found inside {}", path.display()))?;
    let doc = dbx::parse(&entry.data).map_err(|e| format!("dbx parse {}: {e}", entry.name))?;
    Ok(doc.to_xml())
}

fn parse_halo_aimer_instances(xml: &str) -> Vec<HaloAimerInstance> {
    let mut out = Vec::new();
    let mut cursor = xml;
    while let Some(start) = cursor.find("<instance ") {
        let rest = &cursor[start..];
        let Some(tag_end) = rest.find('>') else {
            break;
        };
        let open_tag = &rest[..tag_end];
        let after_tag = &rest[tag_end + 1..];
        let Some(close_idx) = after_tag.find("</instance>") else {
            break;
        };
        let body = &after_tag[..close_idx];
        if open_tag.contains("type=\"GameSharedResources.HaloAimerData\"")
            && let Some(guid) = extract_attr(open_tag, "guid")
        {
            let mut fields = HashMap::new();
            parse_fields(body, &mut fields);
            out.push(HaloAimerInstance { guid, fields });
        }
        cursor = &after_tag[close_idx + "</instance>".len()..];
    }
    out
}

fn parse_fields(body: &str, fields: &mut HashMap<String, String>) {
    let mut cursor = body;
    while let Some(name_pos) = cursor.find("name=\"") {
        let after_name = &cursor[name_pos + 6..];
        let Some(name_end) = after_name.find('"') else {
            break;
        };
        let key = &after_name[..name_end];
        let after_key = &after_name[name_end + 1..];
        if let Some(gt) = after_key.find('>') {
            let val_rest = &after_key[gt + 1..];
            if let Some(lt) = val_rest.find('<') {
                let val = val_rest[..lt].trim();
                if !val.is_empty() || !fields.contains_key(key) {
                    fields.insert(key.to_string(), val.to_string());
                }
                cursor = &val_rest[lt..];
                continue;
            }
        }
        cursor = after_key;
    }
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
}

fn build_console_aimer_ini(pc_xml: &str, xbox_xml: &str) -> Result<String, String> {
    let pc = parse_halo_aimer_instances(pc_xml);
    let xb = parse_halo_aimer_instances(xbox_xml);
    if pc.is_empty() {
        return Err("no GameSharedResources.HaloAimerData instances found in PC input".to_string());
    }
    if pc.len() != xb.len()
        || pc
            .iter()
            .zip(&xb)
            .any(|(p, c)| !p.guid.eq_ignore_ascii_case(&c.guid))
    {
        return Err(format!(
            "PC ({}) and Xbox ({}) HaloAimerData instance GUIDs differ",
            pc.len(),
            xb.len()
        ));
    }

    let mut lines = vec![
        "; OPFOR import: console aimer tuning from your Xbox 360 data (DefaultHaloAimer). Generated - do not edit.".to_string(),
        "[aimers]".to_string(),
        format!("count={}", pc.len()),
    ];

    for (idx, (p, c)) in pc.iter().zip(&xb).enumerate() {
        lines.push(String::new());
        lines.push(format!("[aimer{idx}]"));
        lines.push(format!("; {}", p.guid));
        for key in PC_CHECK_KEYS {
            let val = p
                .fields
                .get(*key)
                .ok_or_else(|| format!("PC aimer{idx} missing field {key}"))?;
            lines.push(format!("pc_{key}={val}"));
        }
        for key in CONSOLE_KEYS {
            let val = c
                .fields
                .get(*key)
                .ok_or_else(|| format!("Xbox aimer{idx} missing field {key}"))?;
            lines.push(format!("{key}={val}"));
        }
    }

    Ok(lines.join("\r\n") + "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_console_aimer_ini_from_pc_and_xbox_xml() {
        let pc = r#"
<partition>
  <instance guid="467D18BB-3BC0-457E-8190-CD5A025A7F6B" type="GameSharedResources.HaloAimerData">
    <field name="YawSpeed">50</field>
    <field name="PitchSpeed">50</field>
    <field name="InputPolynomial">0.3/0/0/0.4/0/0/0/0.3</field>
  </instance>
</partition>"#;
        let xb = r#"
<partition>
  <instance guid="467D18BB-3BC0-457E-8190-CD5A025A7F6B" type="GameSharedResources.HaloAimerData">
    <field name="AccelerationInputThreshold">0.9</field>
    <field name="AccelerationMultiplier">1.5</field>
    <field name="AccelerationDamping">1</field>
    <field name="YawSpeed">3.5</field>
    <field name="PitchSpeed">2.5</field>
    <field name="SuckSoftZone">1</field>
    <field name="SuckUserInputMultiplier">0.8</field>
    <field name="SuckOwnSpeedInfluence">0.3</field>
    <field name="SuckTargetSpeedInfluence">0.3</field>
    <field name="SuckOwnRequiredMovementForMaximumSuck">0</field>
    <field name="SuckStartInputThreshold">0.1</field>
    <field name="SuckZoomingMultiplier">8</field>
    <field name="SuckZoomingPostTime">0</field>
    <field name="SuckYaw">1</field>
    <field name="SuckPitch">1</field>
    <field name="InputPolynomial">0.1/0.4/0/0/0/0/0/0</field>
    <field name="ZoomedInputPolynomial">0.1/1.2/0/0/0/0/0/0</field>
    <field name="SuckDistanceFallOff">0.5/0.8</field>
    <field name="UseYawAcceleration">true</field>
    <field name="UsePitchAcceleration">true</field>
  </instance>
</partition>"#;

        let ini = build_console_aimer_ini(pc, xb).unwrap();
        assert!(ini.contains("[aimers]\r\ncount=1\r\n"));
        assert!(ini.contains("pc_YawSpeed=50\r\n"));
        assert!(ini.contains("YawSpeed=3.5\r\n"));
        assert!(ini.contains("UsePitchAcceleration=true\r\n"));
    }
}
