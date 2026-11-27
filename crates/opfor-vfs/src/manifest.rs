//! Module manifest parser (`modules/<id>/manifest.toml`) and `package.mft` cache merger.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::modes::ModeId;

/// Parsed `modules/<id>/manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleManifest {
    pub id: String,
    pub name: String,
    pub era_order: u32,
    pub enabled: bool,
    pub matchmaking_mod_id: String,
    pub label: String,
    pub accent_hex: String,
    pub subtitle: String,
    pub menu_config: String,
    pub hud_config: String,
    pub game_tips: String,
    pub kit_count: u8,
    pub root_dir: PathBuf,
}

impl ModuleManifest {
    /// Parse a simple flat `key = value` TOML manifest without external deps.
    pub fn parse_str(text: &str, root_dir: PathBuf) -> Result<Self, String> {
        let mut m = Self {
            id: String::new(),
            name: String::new(),
            era_order: 99,
            enabled: true,
            matchmaking_mod_id: String::new(),
            label: String::new(),
            accent_hex: "#3475d0".to_string(),
            subtitle: String::new(),
            menu_config: "menuConfig".to_string(),
            hud_config: "hudConfig".to_string(),
            game_tips: "Menu/GameTips/GameTips".to_string(),
            kit_count: 4,
            root_dir,
        };

        for raw_line in text.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let val = unquote(v.trim());
            match key {
                "id" => m.id = val,
                "name" => m.name = val,
                "era_order" => m.era_order = val.parse().unwrap_or(99),
                "enabled" => m.enabled = matches!(val.as_str(), "true" | "1" | "yes"),
                "matchmaking_mod_id" => m.matchmaking_mod_id = val,
                "label" => m.label = val,
                "accent_hex" => m.accent_hex = val,
                "subtitle" => m.subtitle = val,
                "menu_config" => m.menu_config = val,
                "hud_config" => m.hud_config = val,
                "game_tips" => m.game_tips = val,
                "kit_count" => m.kit_count = val.parse().unwrap_or(4),
                _ => {}
            }
        }

        if m.id.is_empty() {
            return Err("manifest missing required `id` field".to_string());
        }
        Ok(m)
    }

    #[must_use]
    pub fn mode_id(&self) -> Option<ModeId> {
        ModeId::from_id(&self.id)
    }

    /// True if this module has at least one `.fbrb` bundle in its directory tree.
    #[must_use]
    pub fn has_content_bundles(&self) -> bool {
        let mut out = Vec::new();
        collect_fbrb_relpaths(&self.root_dir, &self.root_dir, &mut out);
        !out.is_empty()
    }
}

fn unquote(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Discover all `modules/*/manifest.toml` files, sorted by `era_order` then `id`.
pub fn discover_modules(modules_root: &Path) -> io::Result<Vec<ModuleManifest>> {
    let mut out = Vec::new();
    if !modules_root.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(modules_root)? {
        let entry = entry?;
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let mf_path = dir.join("manifest.toml");
        if !mf_path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&mf_path)?;
        if let Ok(mf) = ModuleManifest::parse_str(&text, dir) {
            out.push(mf);
        }
    }
    out.sort_by(|a, b| a.era_order.cmp(&b.era_order).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

/// Recursively collect relative forward-slash paths of all `.fbrb` bundles under `dir`.
pub fn collect_fbrb_relpaths(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_fbrb_relpaths(base, &p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("fbrb"))
            && let Ok(rel) = p.strip_prefix(base)
        {
            let s = rel.to_string_lossy().replace('\\', "/");
            out.push(s);
        }
    }
}

/// Merge stock `engine/Dist/win32/package.mft` with all enabled modules' `.fbrb`
/// bundles into `cache/package.mft`, without mutating the stock file.
///
/// Returns `(total_existing_sources, added_module_sources)`.
pub fn merge_package_mft(
    stock_mft: &Path,
    modules: &[ModuleManifest],
    out_cache_mft: &Path,
) -> io::Result<(usize, usize)> {
    let stock_text = fs::read_to_string(stock_mft)?;
    let mut existing_sources: BTreeSet<String> = BTreeSet::new();
    for line in stock_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("BundleSource ") {
            existing_sources.insert(rest.trim().to_ascii_lowercase());
        }
    }

    let mut module_bundles: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for mf in modules.iter().filter(|m| m.enabled) {
        let mut found = Vec::new();
        collect_fbrb_relpaths(&mf.root_dir, &mf.root_dir, &mut found);
        for rel in found {
            module_bundles.entry(rel).or_insert_with(|| mf.id.clone());
        }
    }

    let mut merged = stock_text;
    if !merged.ends_with('\n') {
        merged.push('\n');
    }

    let mut added = 0usize;
    for (rel, mod_id) in module_bundles {
        let rel_lower = rel.to_ascii_lowercase();
        let source_key = format!("game:{rel_lower}");
        if existing_sources.contains(&source_key) {
            continue;
        }
        merged.push('\n');
        merged.push_str(&format!("BundleSource game:{rel}\n"));

        // WW2 (BF1943) maps require ww2_common + mp_common mounted alongside each level bundle,
        // and mp1_common VO target for vo/en-00.fbrb.
        if mod_id == "bf1943" && rel_lower.starts_with("levels/") && !rel_lower.starts_with("levels/ww2_common/") {
            if rel_lower.ends_with("/vo/en-00.fbrb") {
                merged.push_str("BundleTarget package:levels/mp1_common/vo/en-00.fbrb\n");
            } else if rel_lower.ends_with("/level-00.fbrb") {
                merged.push_str(&format!("BundleTarget package:{rel}\n"));
                merged.push_str("BundleTarget package:levels/ww2_common/level-00.fbrb\n");
                merged.push_str("BundleTarget game:levels/mp_common/level-00.fbrb\n");
            } else if rel_lower.ends_with("/ingame-00.fbrb") {
                merged.push_str(&format!("BundleTarget package:{rel}\n"));
                merged.push_str("BundleTarget package:levels/ww2_common/ingame-00.fbrb\n");
                merged.push_str("BundleTarget game:levels/mp_common/ingame-00.fbrb\n");
            } else if rel_lower.ends_with("/loader-00.fbrb") {
                merged.push_str(&format!("BundleTarget package:{rel}\n"));
                merged.push_str("BundleTarget package:levels/ww2_common/loader-00.fbrb\n");
                merged.push_str("BundleTarget game:levels/mp_common/loader-00.fbrb\n");
            } else {
                merged.push_str(&format!("BundleTarget package:{rel}\n"));
            }
        } else if mod_id == "onslaught" && rel_lower.starts_with("levels/") {
            merged.push_str(&format!("BundleTarget package:{rel}\n"));
            if rel_lower.ends_with("/level-00.fbrb") {
                merged.push_str("BundleTarget game:levels/mp_common/level-00.fbrb\n");
            } else if rel_lower.ends_with("/ingame-00.fbrb") {
                merged.push_str("BundleTarget game:levels/mp_common/ingame-00.fbrb\n");
            } else if rel_lower.ends_with("/loader-00.fbrb") {
                merged.push_str("BundleTarget game:levels/mp_common/loader-00.fbrb\n");
            }
        } else {
            merged.push_str(&format!("BundleTarget package:{rel}\n"));
        }
        added += 1;
    }

    if let Some(parent) = out_cache_mft.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out_cache_mft, merged)?;
    Ok((existing_sources.len(), added))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_fields() {
        let txt = r#"
id = "bfbc1"
name = "Battlefield: Bad Company"
era_order = 3
enabled = true
kit_count = 5
label = "BFBC2BC1"
"#;
        let mf = ModuleManifest::parse_str(txt, PathBuf::from("modules/bfbc1")).unwrap();
        assert_eq!(mf.id, "bfbc1");
        assert_eq!(mf.era_order, 3);
        assert_eq!(mf.kit_count, 5);
        assert_eq!(mf.mode_id(), Some(ModeId::Bfbc1));
    }
}
