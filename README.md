# Battlefield OPFOR (`battlefield-opfor/`)

Standalone, pure-Rust modular 4-era client (**Battlefield 1943**, **BFBC2: Vietnam**, **Battlefield: Bad Company 1**, **Battlefield: Bad Company 2**) organized under the **Option B Per-Era `modules/` VFS Architecture**, with a native 4-Blade OPFOR Hub (`OPFOR.exe`) and rehosted 32-bit Frostbite host (`engine/OPFOREngine.exe`).

## Directory Layout

```text
battlefield-opfor/
├── OPFOR.exe                 # 64-bit Rust 4-Blade OPFOR Hub launcher (original BF1943/Vietnam/BFBC1/BFBC2 logos)
├── Setup.exe                 # 64-bit Rust Setup & Console Porting Wizard (`opfor-setup`)
├── OPFOREngine.exe           # 32-bit pure-Rust Frostbite host (`opfor-host`)
├── opfor-cli.exe             # 64-bit Rust CLI (setup-engine, sync-cache, build-hub, apt-info)
├── config/                   # opfor.ini, display.ini, launch.ini, maps.ini, opfor.ico
├── user/                     # profile.ini, launch.ini, stats.txt, logs/
├── cache/                    # Auto-merged package.mft, bundleManifest, startup-00.fbrb, overlay-00.fbrb (+6,320 BFBC1 EN strings)
├── ledger/                   # state.toml (module install & enable state)
├── engine/                   # Rehosted 32-bit engine runtime:
│   ├── OPFOREngine.exe       # Mirror of 32-bit pure-Rust host (`opfor-host`)
│   ├── frostbite.bin         # Stock Frostbite 1.5 engine payload (clean, unmodified)
│   ├── dinput8.dll           # 32-bit Rust proxy & Win32 IAT VFS (`opfor-dll`)
│   └── Dist/ Output/ Package/# Clean stock BFBC2 PC data (untouched)
├── modules/                  # Self-contained per-era modules routed via Win32 IAT VFS
│   ├── hub/                  # 4-Blade Hub art & extracted transparent logos (`logo_bfbc2.png`, `logo_vietnam.png`)
│   ├── bf1943/               # Battlefield 1943 module (4 Pacific maps, 52 menu PNGs, 4 UI WAVs, 8 Eurostile bitmap fonts)
│   ├── vietnam/              # BFBC2: Vietnam (`levels/nam_*` cleanly separated from engine/Package)
│   ├── bfbc1/                # Battlefield: Bad Company 1 module (8 Gold Rush MP + 7 SP missions, era-isolated MorganSnLining
│   │                         # fonts, PC EA-XAS `streaming_sounds-00.fbrb`, 9 campaign Bink movies, 13 HPD v2 navmeshes)
│   └── onslaught/            # Onslaught Co-op module
├── utility/                  # Porting & setup helpers (`xenolith.exe`, `setup_bfbc2.py`, `port_bfbc1.py`, `port_bf1943.py`,
│                             # `build_bfbc1_ui_parity.py`, `extract_bfbc1_movies.py`, `port_bfbc1_navmeshes.py`, `build_havok_template.py`)
└── crates/                   # Pure-Rust workspace
    ├── opfor-vfs/            # Option B VFS path router (era-isolated HUD, `menu/shared/`, `streaming_sounds-00.fbrb`, `output/win32/`), 4-mode table, `package.mft` merger
    ├── opfor-apt/            # EA APT BIGF reader/writer, BE->LE `.const` converter, Hub generator
    ├── opfor-dll/            # 32-bit `i686-pc-windows-msvc` `dinput8.dll` proxy + Win32 IAT VFS + 4-mode core
    ├── opfor-host/           # 32-bit `OPFOREngine.exe` (BF1943 1:1 Menu, Air Raid, Type 3 SP launcher, per-era save isolation, era-gated `levelfix` guards)
    ├── opfor-launcher/       # 64-bit `OPFOR.exe` 4-Blade Hub
    ├── opfor-setup/          # 64-bit `Setup.exe` GUI wizard (stages BFBC2 read-only, ports BFBC1 MP/SP/UI/Movies/Navmeshes & BF1943)
    └── opfor-cli/            # 64-bit `opfor-cli.exe`
```

## Key Features
- **4-Blade OPFOR Hub (`OPFOR.exe`) & Setup Wizard (`Setup.exe`):** Uses original extracted transparent logos across all four blades with standardized bottom metadata plates (`Battlefield 1943` / `4 Maps`, `Battlefield: Bad Company 2 Vietnam` / `5 Maps`, `Battlefield: Bad Company` / `7 Missions · 8 Maps`, `Battlefield: Bad Company 2` / `13 Missions · 14 Maps`). `Setup.exe` supports `Oasis`, `MpAll`, `SpAll`, and `Full` BFBC1 porting modes (automatically running fonts/localization/SP loading music, campaign Bink movie extraction, and Kynogon HPD v1 → v2 navmesh conversion).
- **Per-Era Savegame Isolation (`opfor-host/src/imports.rs`):** Redirects `SHGetFolderPathA/W(CSIDL_PERSONAL)` under `Documents\Project OPFOR\` so **BFBC1** (`Project OPFOR\BFBC1\`), **BF1943** (`Project OPFOR\BF1943\`), and **BFBC2/Vietnam** (`Project OPFOR\BFBC2\`) never overwrite each other's campaign progress or settings.
- **Battlefield 1943 1:1 Menu, Air Raid & Deploy (`OPFOREngine.exe`):** Renders all 52 extracted BF1943 RGBA PNGs, 4 RAM-cached UI WAVs, and 8 literal `EurostileLTStd-BoldCn` bitmap fonts with kerning pairs; registers native `TeamVehicleProjectileEntityData`, `AirRaidEffectsData`, and `ScorePartComponentData` runtime DBX classes; implements Tank Horn `[H]`, Aircraft Landing Gear `[G]` with 60 m ground-height probe, LCVP cradle release, airfield repair stations, and bunker auto-fire using lock-free mode/probe atomics.
- **Battlefield: Bad Company 1 Campaign & Multiplayer (`modules/bfbc1/`):**
  - **All 7 SP campaign missions** (`sp_01_greenacres`, `sp_02_traitor_of_zabograd`, `sp_03_crossroads`, `sp_05_par_for_the_course`, `sp_06_hot_zone`, `sp_07_pow`, `sp_08_ghost_town`) boot under native `START_TYPE_SINGLEPLAYER = 3` (stripping the double `"Levels/"` prefix), with `SP_02`-matched `loader-00.fbrb`, authentic per-level `SP01`–`SP08` PC EA-XAS loading music, 305 MB `vo/en-00.fbrb` voiceover, 9 extracted campaign & intro/credits Bink movies (64B → 72B Frostbite 1.5 header), and all 13 Kynogon HPD v1 → v2 navmeshes staged under `modules/bfbc1/ai/levels/`.
  - **All 8 Gold Rush MP maps** (`mp_beachhead_day1` through `mp_monastery_day2`) ported with PC `MeshData`, Havok `4.6.1 → 5.5.0`, `AnimTreeInfo` v6 BE→LE, PC EA-XAS `Wave`s + era-isolated `streaming_sounds-00.fbrb`, GUID-protected `mp_common` overrides, and PC DX9/10/11 ShaderDBs.
  - **BFBC1 UI Parity:** Era-isolated `MorganSnLining` US fonts (`modules/bfbc1/menu/shared/fonts_us-00.fbrb`, 38 entries) and +6,320 additive BFBC1 English localization strings merged into `cache/overlay-00.fbrb` (13,670 total strings).
- **Era-Gated Runtime Compatibility Guards (`opfor-host/src/levelfix.rs`):** Installs only the guards required for the active era (`2/2` on BFBC2 & Vietnam, `6/6` on BF1943, `187/187` on BFBC1 covering Frostbite 1.0 → 1.5 SP/MP runtime boundaries), avoiding unnecessary overhead on stock PC eras.
- **Asset Porting Stack (`../xenolith` → `utility/xenolith.exe`):** Pure-Rust Xbox 360 → PC porting toolkit supporting XEX2, STFS, XISO, `FbRB`, `{binary}` DBX (including BFBC1 schema migration & `<item ref="null"/>` array pruning), `XenonTexture` (incl. `RES\x02` & 3D volumes), `*MeshSet`/`MeshData`, Terrain (incl. BFBC1 `256×256` & `512×512`), Havok `4.6.1 → 5.5.0`, `AnimTreeInfo` v6, Granny 2, `AimAnimation`, EA APT, XMA2 → EA-XAS (`Wave` + `streaming_sounds`), GUID-aware `dedup-fbrb`, `unwrap-bink`, `upgrade-hpd` (Kynogon HPD v1 → v2), and Xenos → Dx9/Dx10/Dx11 shader compilation.

## Build & Test Commands

```powershell
# Run all unit tests
cargo test

# Build 64-bit Hub launcher, Setup wizard, and CLI
cargo build --release -p opfor-launcher -p opfor-setup -p opfor-cli

# Build 32-bit engine host and VFS proxy DLL
cargo build --release --target i686-pc-windows-msvc -p opfor-host -p opfor-dll
```
