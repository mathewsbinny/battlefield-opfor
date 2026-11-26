//! Build script for `OPFOREngine.exe` (re-hosted 32-bit x86 Frostbite 1.5 host).
//!
//! - Links at fixed high base `0x10000000` so `0x00400000..0x01D13000` is completely
//!   free for mapping `BFBC2Game.exe` (build 795745) at its native un-relocated address.
//! - Enables `/LARGEADDRESSAWARE` (4 GB on 64-bit Windows) and `/SAFESEH:NO` (so
//!   Frostbite's un-registered x86 SEH frames execute cleanly).
//! - Embeds `../../config/opfor.ico` directly into the host PE resources via `rc.exe`.

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
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "windows" && target_arch == "x86" {
        // Link at 0x00300000 so host .text/.rdata/.data sit in 0x00301000..0x00335000,
        // and our zero-disk-size BSS arena spans 0x00335000..0x01D30000, guaranteeing
        // the Windows kernel reserves 100% of 0x00400000..0x01D13000 before any heap/stack.
        println!("cargo:rustc-link-arg-bin=OPFOREngine=/BASE:0x00300000");
        println!("cargo:rustc-link-arg-bin=OPFOREngine=/DYNAMICBASE:NO");
        println!("cargo:rustc-link-arg-bin=OPFOREngine=/FIXED");
        println!("cargo:rustc-link-arg-bin=OPFOREngine=/LARGEADDRESSAWARE");
        println!("cargo:rustc-link-arg-bin=OPFOREngine=/SAFESEH:NO");
        println!("cargo:rustc-link-arg-bin=OPFOREngine=/MANIFEST:NO");
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico = manifest_dir.join("..").join("..").join("config").join("opfor.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    let Some(rc_exe) = find_rc_exe() else {
        return;
    };
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rc_file = out_dir.join("opfor_engine.rc");
    let res_file = out_dir.join("opfor_engine.res");
    let manifest_file = out_dir.join("opfor_engine.manifest");

    // WinSxS dependency manifest for Microsoft.VC80.CRT (MSVCR80.dll / MSVCP80.dll)
    let _ = fs::write(
        &manifest_file,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.VC80.CRT" version="8.0.50727.4053" processorArchitecture="x86" publicKeyToken="1fc8b3b9a1e18e3b"/>
    </dependentAssembly>
  </dependency>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.VC80.CRT" version="8.0.50727.762" processorArchitecture="x86" publicKeyToken="1fc8b3b9a1e18e3b"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    );

    let mut rc = String::new();
    if ico.is_file() {
        let ico_escaped = ico.to_string_lossy().replace('\\', "\\\\");
        rc.push_str(&format!("1 ICON \"{ico_escaped}\"\n"));
    }
    let man_escaped = manifest_file.to_string_lossy().replace('\\', "\\\\");
    rc.push_str(&format!("1 24 \"{man_escaped}\"\n"));
    let _ = fs::write(&rc_file, rc);
    if let Ok(st) = Command::new(rc_exe)
        .args(["/nologo", "/fo"])
        .arg(&res_file)
        .arg(&rc_file)
        .status()
    {
        if st.success() && res_file.is_file() {
            println!("cargo:rustc-link-arg-bin=OPFOREngine={}", res_file.display());
        }
    }
}
