//! Embeds `../../config/opfor.ico` into `OPFOR.exe` PE resources when `rc.exe` is available.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc_exe() -> Option<PathBuf> {
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc.exe"));
    }
    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(rd) = fs::read_dir(kits) {
        let mut vers: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        vers.sort();
        vers.reverse();
        for v in vers {
            let cand = v.join("x64").join("rc.exe");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico = manifest_dir.join("..").join("..").join("config").join("opfor.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    if !ico.is_file() {
        return;
    }
    let Some(rc_exe) = find_rc_exe() else {
        return;
    };
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rc_file = out_dir.join("opfor.rc");
    let res_file = out_dir.join("opfor.res");
    let ico_escaped = ico.to_string_lossy().replace('\\', "\\\\");
    let _ = fs::write(&rc_file, format!("1 ICON \"{ico_escaped}\"\n"));
    if let Ok(st) = Command::new(rc_exe)
        .args(["/nologo", "/fo"])
        .arg(&res_file)
        .arg(&rc_file)
        .status()
        && st.success()
        && res_file.is_file()
    {
        println!("cargo:rustc-link-arg-bin=OPFOR={}", res_file.display());
    }
}
