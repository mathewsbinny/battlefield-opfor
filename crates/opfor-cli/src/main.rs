//! Pure-Rust CLI for Project OPFOR (`battlefield-opfor/`):
//! - `setup-engine <src_bfbc2>` : Copies stock BFBC2 -> `engine/` cleanly (skipping Rome/Nexus/Support)
//!                                and routes Vietnam (`nam_*`) into `modules/vietnam/levels/`
//! - `sync-cache`               : Merges `cache/package.mft` and writes `ledger/state.toml`
//! - `build-hub`                : Builds Tool Suite Dark 4-Blade Hub chrome (`modules/hub/`)
//! - `apt-info <file.res>`      : Inspects an EA APT `BIGF` movie archive

mod bfbc1;
mod era_parity;
mod port;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use opfor_apt::hub::{HubPalette, write_hub_chrome_tga, write_hub_layout_toml};
use opfor_vfs::{discover_modules, merge_package_mft};

const ROOT_STOCK_FILES: &[&str] = &[
    "BFBC2Game.exe",
    "binkw32.dll",
    "D3DCompiler_42.dll",
    "D3DX9_42.dll",
    "d3dx10_42.dll",
    "d3dx11_42.dll",
    "database.dbmanifest",
    "dfe",
    "dialoglogo128x128.jpg",
    "InstallationData.xml",
    "winui.dll",
];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return ExitCode::from(1);
    }

    let root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let res = match args[0].as_str() {
        "setup-engine" => {
            let src = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Games\Battlefield Bad Company 2"));
            cmd_setup_engine(&root, &src)
        }
        "import-modules" => {
            let src = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Programming\Projects\project-opfor-bfbc2\game"));
            cmd_import_modules(&root, &src)
        }
        "sync-cache" => cmd_sync_cache(&root),
        "build-hub" => cmd_build_hub(&root),
        "apt-info" => match args.get(1) {
            Some(p) => cmd_apt_info(Path::new(p)),
            None => Err("usage: opfor-cli apt-info <file.res>".to_string()),
        },
        "setup-bfbc2" => port::cmd_setup_bfbc2(&root, &args[1..]),
        "build-havok-template" => port::cmd_build_havok_template(&args[1..]),
        "port-bf1943" => port::cmd_port_bf1943(&root, &args[1..]),
        "port-bfbc1" => port::cmd_port_bfbc1(&root, &args[1..]),
        "bfbc1-ui-parity" => bfbc1::cmd_bfbc1_ui_parity(&root, &args[1..]),
        "bfbc1-mainmenu" => bfbc1::cmd_bfbc1_mainmenu(&root, &args[1..]),
        "bfbc1-movies" => bfbc1::cmd_bfbc1_movies(&root, &args[1..]),
        "bfbc1-sp-loaders" => bfbc1::wire_bfbc1_sp_loader_movies(&root),
        "bfbc1-navmeshes" => bfbc1::cmd_bfbc1_navmeshes(&root, &args[1..]),
        "era-ui-input-parity" => era_parity::cmd_era_ui_input_parity(&root, &args[1..]),
        other => Err(format!("unknown command `{other}`")),
    };

    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    eprintln!(
        "opfor-cli — Project OPFOR pure-Rust toolchain\n\
         usage:\n\
           opfor-cli setup-engine [\"C:\\Games\\Battlefield Bad Company 2\"]\n\
           opfor-cli sync-cache\n\
           opfor-cli build-hub\n\
           opfor-cli apt-info <SwfMovie.res>\n\
           opfor-cli bfbc1-ui-parity --opfor <dir> --bfbc2 <dir> [--iso <bfbc1.iso>]\n\
           opfor-cli bfbc1-movies --iso <bfbc1.iso> [--opfor <dir>]\n\
           opfor-cli bfbc1-navmeshes --iso <bfbc1.iso> [--opfor <dir>]\n\
           opfor-cli era-ui-input-parity --era <bfbc1|bf1943> --opfor <dir>"
    );
}

fn cmd_setup_engine(root: &Path, src: &Path) -> Result<(), String> {
    if !src.join("BFBC2Game.exe").is_file() {
        return Err(format!("BFBC2Game.exe not found in {}", src.display()));
    }
    let engine = root.join("engine");
    let vietnam_levels = root.join("modules").join("vietnam").join("levels");
    fs::create_dir_all(&engine).map_err(|e| e.to_string())?;
    fs::create_dir_all(&vietnam_levels).map_err(|e| e.to_string())?;

    println!("Copying stock root binaries from {} -> {} ...", src.display(), engine.display());
    for name in ROOT_STOCK_FILES {
        let s = src.join(name);
        if s.is_file() {
            let dst_name = if *name == "BFBC2Game.exe" { "OPFOREngine.exe" } else { *name };
            copy_if_changed(&s, &engine.join(dst_name)).map_err(|e| format!("{name}: {e}"))?;
        }
    }

    // Copy Dist/ and Output/ cleanly into engine/
    for sub in ["Dist", "Output"] {
        let s = src.join(sub);
        if s.is_dir() {
            println!("Copying {sub}/ -> engine/{sub}/ ...");
            let count = copy_tree(&s, &engine.join(sub)).map_err(|e| format!("{sub}: {e}"))?;
            println!("  {count} files synced.");
        }
    }

    // Copy Package/ into engine/Package/, EXCEPT Vietnam levels (`nam_*`) which go into modules/vietnam/levels/
    let src_pkg = src.join("Package");
    if src_pkg.is_dir() {
        println!("Organizing Package/ (base -> engine/Package/, Vietnam nam_* -> modules/vietnam/levels/) ...");
        let (base_cnt, nam_cnt) =
            copy_package_split_vietnam(&src_pkg, &engine.join("Package"), &vietnam_levels)
                .map_err(|e| e.to_string())?;
        println!("  {base_cnt} base Package files -> engine/Package/");
        println!("  {nam_cnt} Vietnam files -> modules/vietnam/levels/");
    }

    cmd_sync_cache(root)?;
    Ok(())
}

fn copy_package_split_vietnam(
    src_pkg: &Path,
    dst_pkg: &Path,
    dst_vietnam_levels: &Path,
) -> io::Result<(usize, usize)> {
    let mut base_cnt = 0usize;
    let mut nam_cnt = 0usize;
    fs::create_dir_all(dst_pkg)?;
    for entry in fs::read_dir(src_pkg)? {
        let entry = entry?;
        let name = entry.file_name();
        let src_p = entry.path();
        if name.eq_ignore_ascii_case("levels") && src_p.is_dir() {
            let dst_levels = dst_pkg.join("levels");
            fs::create_dir_all(&dst_levels)?;
            for lv in fs::read_dir(&src_p)? {
                let lv = lv?;
                let lv_name = lv.file_name();
                let lv_str = lv_name.to_string_lossy().to_ascii_lowercase();
                if lv_str.starts_with("nam_") {
                    nam_cnt += copy_tree(&lv.path(), &dst_vietnam_levels.join(&lv_name))?;
                } else {
                    base_cnt += copy_tree(&lv.path(), &dst_levels.join(&lv_name))?;
                }
            }
        } else if src_p.is_dir() {
            base_cnt += copy_tree(&src_p, &dst_pkg.join(&name))?;
        } else {
            copy_if_changed(&src_p, &dst_pkg.join(&name))?;
            base_cnt += 1;
        }
    }
    Ok((base_cnt, nam_cnt))
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<usize> {
    if src.is_file() {
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p)?;
        }
        copy_if_changed(src, dst)?;
        return Ok(1);
    }
    fs::create_dir_all(dst)?;
    let mut count = 0usize;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let sp = entry.path();
        let dp = dst.join(entry.file_name());
        if sp.is_dir() {
            count += copy_tree(&sp, &dp)?;
        } else {
            copy_if_changed(&sp, &dp)?;
            count += 1;
        }
    }
    Ok(count)
}

fn copy_if_changed(src: &Path, dst: &Path) -> io::Result<()> {
    if let (Ok(sm), Ok(dm)) = (fs::metadata(src), fs::metadata(dst)) {
        if sm.len() == dm.len() {
            return Ok(());
        }
    }
    fs::copy(src, dst)?;
    Ok(())
}

fn copy_tree_clean(src: &Path, dst: &Path) -> io::Result<usize> {
    if src.is_file() {
        let name = src.file_name().unwrap_or_default().to_string_lossy();
        if name.contains(".opfor-") || name.ends_with("-applied") {
            return Ok(0);
        }
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p)?;
        }
        copy_if_changed(src, dst)?;
        return Ok(1);
    }
    fs::create_dir_all(dst)?;
    let mut count = 0usize;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let sp = entry.path();
        let dp = dst.join(entry.file_name());
        if sp.is_dir() {
            count += copy_tree_clean(&sp, &dp)?;
        } else {
            count += copy_tree_clean(&sp, &dp)?;
        }
    }
    Ok(count)
}

fn cmd_import_modules(root: &Path, src_game: &Path) -> Result<(), String> {
    let pkg_levels = src_game.join("Package").join("levels");
    if !pkg_levels.is_dir() {
        return Err(format!("Package/levels not found in {}", src_game.display()));
    }

    let bf1943_dirs = ["ww2_common", "wake_island_s", "guadal_canal", "iwo_jima_s", "coral_sea"];
    let mut c43 = 0usize;
    for d in bf1943_dirs {
        let s = pkg_levels.join(d);
        if s.is_dir() {
            let dst = root.join("modules").join("bf1943").join("levels").join(d);
            c43 += copy_tree_clean(&s, &dst).map_err(|e| format!("{d}: {e}"))?;
        }
    }
    println!("Imported {c43} BF1943 files -> modules/bf1943/levels/");

    let mut c_ons = 0usize;
    for entry in fs::read_dir(&pkg_levels).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let lower = name.to_string_lossy().to_ascii_lowercase();
        if lower.starts_with("ons_") && entry.path().is_dir() {
            let dst = root.join("modules").join("onslaught").join("levels").join(&name);
            c_ons += copy_tree_clean(&entry.path(), &dst).map_err(|e| format!("{lower}: {e}"))?;
        }
    }
    let ai_levels = src_game.join("Output").join("win32").join("ai").join("levels");
    if ai_levels.is_dir() {
        for entry in fs::read_dir(&ai_levels).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let lower = name.to_string_lossy().to_ascii_lowercase();
            if lower.starts_with("ons_") && entry.path().is_dir() {
                let dst = root
                    .join("modules")
                    .join("onslaught")
                    .join("ai")
                    .join("levels")
                    .join(&name);
                c_ons += copy_tree_clean(&entry.path(), &dst).map_err(|e| format!("ai/{lower}: {e}"))?;
            }
        }
    }
    println!("Imported {c_ons} Onslaught files (levels + AI .hpd) -> modules/onslaught/");

    cmd_sync_cache(root)?;
    Ok(())
}

fn cmd_sync_cache(root: &Path) -> Result<(), String> {
    let modules = discover_modules(&root.join("modules")).map_err(|e| e.to_string())?;
    println!("Discovered {} modules:", modules.len());
    let mut ledger = String::from("# Project OPFOR — Module Ledger State\n\n");
    for m in &modules {
        let has_bundles = m.has_content_bundles();
        println!(
            "  - {:10} | enabled={:<5} | bundles={:<5} | {}",
            m.id, m.enabled, has_bundles, m.name
        );
        ledger.push_str(&format!(
            "[[module]]\nid = \"{}\"\nenabled = {}\nhas_bundles = {}\nmatchmaking_mod_id = \"{}\"\n\n",
            m.id, m.enabled, has_bundles, m.matchmaking_mod_id
        ));
    }

    let ledger_path = root.join("ledger").join("state.toml");
    if let Some(p) = ledger_path.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::write(&ledger_path, ledger).map_err(|e| e.to_string())?;

    let stock_mft = root.join("engine").join("Dist").join("win32").join("package.mft");
    let cache_mft = root.join("cache").join("package.mft");
    if stock_mft.is_file() {
        let (base, added) = merge_package_mft(&stock_mft, &modules, &cache_mft)
            .map_err(|e| e.to_string())?;
        println!("Merged cache/package.mft: {base} stock sources + {added} module sources.");
    } else {
        println!("Skipped cache/package.mft (engine/Dist/win32/package.mft not yet installed).");
    }
    Ok(())
}

fn cmd_build_hub(root: &Path) -> Result<(), String> {
    let hub_dir = root.join("modules").join("hub");
    let pal = HubPalette::default();
    let tga = hub_dir.join("hub_chrome.tga");
    let layout = hub_dir.join("hub_layout.toml");
    write_hub_chrome_tga(&tga, &pal).map_err(|e| e.to_string())?;
    write_hub_layout_toml(&layout).map_err(|e| e.to_string())?;
    println!("Built 4-Blade Hub assets:");
    println!("  {}", tga.display());
    println!("  {}", layout.display());
    Ok(())
}

fn cmd_apt_info(path: &Path) -> Result<(), String> {
    let raw = fs::read(path).map_err(|e| e.to_string())?;
    let entries = opfor_apt::bigf::parse(&raw)?;
    println!("{}: BIGF archive ({} entries)", path.display(), entries.len());
    let apt_len = entries
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".apt"))
        .map(|e| e.data.len())
        .unwrap_or(0);
    for e in &entries {
        println!("  {:32} {:8} bytes", e.name, e.data.len());
        if e.name.to_ascii_lowercase().ends_with(".const") && apt_len > 0 {
            if let Ok(ci) = opfor_apt::swap::inspect_const(&e.data, apt_len) {
                println!(
                    "    -> endian={}, movie_offset=0x{:x}, constants={}",
                    if ci.big_endian { "BE (Xbox 360)" } else { "LE (PC)" },
                    ci.movie_offset,
                    ci.count
                );
            }
        }
    }
    Ok(())
}
