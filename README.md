# Battlefield OPFOR (`battlefield-opfor/`)

Standalone, pure-Rust modular 4-era client (**Battlefield 1943**, **BFBC2: Vietnam**, **Battlefield: Bad Company 1**, **Battlefield: Bad Company 2**) organized under the **Option B Per-Era `modules/` VFS Architecture**, with a native 4-Blade OPFOR Hub (`OPFOR.exe`) and rehosted 32-bit Frostbite host (`engine/OPFOREngine.exe`).

## Directory Layout

```text
battlefield-opfor/
├── OPFOR.exe                 # 64-bit Rust 4-Blade OPFOR Hub launcher (original BF1943/Vietnam/BFBC1/BFBC2 logos)
├── Setup.exe                 # 64-bit pure-Rust Setup & Console Porting Wizard (`opfor-setup`)
├── OPFOREngine.exe           # 32-bit pure-Rust Frostbite host (`opfor-host`)
├── opfor-cli.exe             # 64-bit pure-Rust CLI (setup-bfbc2, port-bf1943, port-bfbc1, bfbc1-*, era-ui-input-parity, sync-cache, build-hub, apt-info)
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
│   ├── bf1943/               # Battlefield 1943 module (4 Pacific maps, native APT mainmenu/ingame, 52 menu PNGs, 4 UI WAVs, 8 Eurostile bitmap fonts)
│   ├── vietnam/              # BFBC2: Vietnam (`levels/nam_*` cleanly separated from engine/Package)
│   ├── bfbc1/                # Battlefield: Bad Company 1 module (8 Gold Rush MP + 7 SP missions, native APT mainmenu/Gunrack/ingame,
│   │                         # era-isolated MorganSnLining fonts, PC EA-XAS `streaming_sounds-00.fbrb`, 9 campaign Bink movies,
│   │                         # native SP loader movies & MP static loading screens, 13 HPD v2 navmeshes)
│   └── onslaught/            # Onslaught Co-op module
├── utility/                  # Pure-Rust porting & setup binaries (`opfor-cli.exe`, `xenolith.exe`)
└── crates/                   # Pure-Rust workspace
    ├── opfor-vfs/            # Option B VFS path router (era-isolated HUD/APT menus, `menu/shared/`, `streaming_sounds-00.fbrb`, `output/win32/`), 4-mode table, `package.mft` merger
    ├── opfor-apt/            # EA APT BIGF reader/writer (shared by `opfor-cli`), BE->LE `.const` converter, Hub generator
    ├── opfor-dll/            # 32-bit `i686-pc-windows-msvc` `dinput8.dll` proxy + Win32 IAT VFS + 4-mode core
    ├── opfor-host/           # 32-bit `OPFOREngine.exe` (native APT main menus + FSCommand/XBL blade shims, 3-way KBM/Xbox/PS3 prompts & HID bridge, BF1943 Air Raid, Type 3 SP launcher, per-era save isolation, modular era-gated `levelfix/` guards)
    ├── opfor-launcher/       # 64-bit `OPFOR.exe` 4-Blade Hub
    ├── opfor-setup/          # 64-bit `Setup.exe` GUI wizard (pure-Rust native Win32 dialogs; stages BFBC2 read-only, ports BFBC1 MP/SP/UI/Movies/Navmeshes & BF1943)
    └── opfor-cli/            # 64-bit `opfor-cli.exe` (pure-Rust staging, porting, UI/input parity, and cache/hub toolchain)
```

## Key Features
- **4-Blade OPFOR Hub (`OPFOR.exe`) & Pure-Rust Setup Wizard (`Setup.exe`):** Uses original extracted transparent logos across all four blades with standardized bottom metadata plates (`Battlefield 1943` / `4 Maps`, `Battlefield: Bad Company 2 Vietnam` / `5 Maps`, `Battlefield: Bad Company` / `7 Missions · 8 Maps`, `Battlefield: Bad Company 2` / `13 Missions · 14 Maps`). `Setup.exe` orchestrates `utility/opfor-cli.exe` and `utility/xenolith.exe` with zero Python or PowerShell dependencies, supporting `Oasis`, `MpAll`, `SpAll`, and `Full` BFBC1 porting modes (automatically running mainmenu/Gunrack staging, fonts/localization, SP loader movies & MP loading screens, campaign Bink movie extraction, Kynogon HPD v1 → v2 navmesh conversion, and era UI/input parity).
- **Per-Era Savegame Isolation (`opfor-host/src/imports.rs`):** Redirects `SHGetFolderPathA/W(CSIDL_PERSONAL)` under `Documents\Project OPFOR\` so **BFBC1** (`Project OPFOR\BFBC1\`), **BF1943** (`Project OPFOR\BF1943\`), and **BFBC2/Vietnam** (`Project OPFOR\BFBC2\`) never overwrite each other's campaign progress or settings.
- **Native APT Menus & 3-Way KBM / Xbox / PS3 Input Parity (`opfor-host` & `opfor-cli`):** Both **BFBC1** and **BF1943** run their native EA APT `mainmenu-00.fbrb` and era-isolated `ingame-00.fbrb` bundles with device-aware `WM_SETCURSOR` handling, main-menu `FSCommand` and Xbox Live blade shims, and live 3-way KBM / Xbox / PlayStation prompt switching backed by a Win32 HID bridge.
- **Battlefield 1943 Air Raid & Pacific Gameplay (`OPFOREngine.exe`):** Exports all 52 extracted BF1943 RGBA PNGs, 4 RAM-cached UI WAVs, and 8 literal `EurostileLTStd-BoldCn` bitmap fonts with kerning pairs; registers native `TeamVehicleProjectileEntityData`, `AirRaidEffectsData`, and `ScorePartComponentData` runtime DBX classes; implements Tank Horn `[H]`, Aircraft Landing Gear `[G]` with 60 m ground-height probe, LCVP cradle release, airfield repair stations, and bunker auto-fire using lock-free mode/probe atomics.
- **Battlefield: Bad Company 1 Campaign & Multiplayer (`modules/bfbc1/`):**
  - **All 7 SP campaign missions** (`sp_01_greenacres`, `sp_02_traitor_of_zabograd`, `sp_03_crossroads`, `sp_05_par_for_the_course`, `sp_06_hot_zone`, `sp_07_pow`, `sp_08_ghost_town`) boot into live gameplay under native `START_TYPE_SINGLEPLAYER = 3` (stripping the double `"Levels/"` prefix), with authentic no-health-regen soldier tuning, native per-level SP loader movies & `SP01`–`SP08` PC EA-XAS loading music, 305 MB `vo/en-00.fbrb` voiceover, 9 extracted campaign & intro/credits Bink movies (64B → 72B Frostbite 1.5 header), and all 13 Kynogon HPD v1 → v2 navmeshes staged under `modules/bfbc1/ai/levels/`.
  - **All 8 Gold Rush MP maps** (`mp_beachhead_day1` through `mp_monastery_day2`) ported with native BFBC1 MP static loading screens (`loader-00.fbrb` & `startup-00.fbrb`, 0 donor `BinkMemory`), PC `MeshData`, Havok `4.6.1 → 5.5.0`, `AnimTreeInfo` v6 BE→LE, PC EA-XAS `Wave`s + era-isolated `streaming_sounds-00.fbrb`, GUID-protected `mp_common` overrides, and PC DX9/10/11 ShaderDBs.
  - **BFBC1 UI & Menu Parity:** Native APT `mainmenu-00.fbrb` + Gunrack staged by `Setup.exe`, era-isolated `MorganSnLining` US fonts (`modules/bfbc1/menu/shared/fonts_us-00.fbrb`, 38 entries), and +6,320 additive BFBC1 English localization strings merged into `cache/overlay-00.fbrb` (13,670 total strings).
- **Era-Gated Runtime Compatibility Guards (`opfor-host/src/levelfix/{mod,trie,havok,net,ui,render,vo}.rs`):** Installs only the guards required for the active era (`2/2` on BFBC2 & Vietnam, `6/6` on BF1943, `272/272` on BFBC1 covering Frostbite 1.0 → 1.5 SP/MP runtime boundaries, Trie bounds/locking, `SvSoldier` ghost bitstream desync, `ResourceManager` spin-wait, and `UIIngame` Flash-ready fallback), avoiding unnecessary overhead on stock PC eras.
- **Controller Aim Assist, MultiKill VO & AI Runtime (`opfor-host`):** Includes console `HaloAimerData` aim-assist swapping (`padaimer.rs`, [docs/CONTROLLER_AIM_ASSIST.md](docs/CONTROLLER_AIM_ASSIST.md)), BF1943 MultiKill VO call-out carrier (`voprobe.rs`), offline promo-code unlocks ([docs/PROMO_UNLOCKS.md](docs/PROMO_UNLOCKS.md)), and `[dev]`/`[debug]` AI runtime hooks (`force_ai`, `sector_offsets`, `unit_counters`, `kill_probe`, `vo_probe`, live APT `menutree`).
- **Stock-MP Bots Layer (`opfor-cli/src/stockmp_ai.rs`):** Pure-Rust, in-memory `apply-ai-template`, `apply-sector-offsets`, and `author-stockmp-ai` commands that author non-destructive AI bot layers (`HasAI = true`, campaign grunt templates, `SSAiPlayer` spawners, `AreaOrder`s, and `SectorOffsets`) into `modules/stockmp/` without modifying `engine/` ([docs/STOCK_MP_AI.md](docs/STOCK_MP_AI.md)).
- **Asset Porting Stack (`../xenolith` → `utility/xenolith.exe`):** Pure-Rust Xbox 360 → PC porting toolkit supporting XEX2, STFS, XISO, `FbRB`, `{binary}` DBX (`dbx-to-xml`, `xml-to-dbx`, BFBC1 schema migration & `<item ref="null"/>` array pruning), `XenonTexture` (incl. `RES\x02` & 3D volumes), `*MeshSet`/`MeshData`, Terrain (incl. BFBC1 `256×256` & `512×512`), Havok `4.6.1 → 5.5.0`, `AnimTreeInfo` v6, Granny 2, `AimAnimation`, EA APT, XMA2 → EA-XAS (`Wave` + `streaming_sounds`), GUID-aware `dedup-fbrb`, `unwrap-bink`, `upgrade-hpd` (Kynogon HPD v1 → v2), `make-sector-offsets`, `register-level`, and Xenos → Dx9/Dx10/Dx11 shader compilation.

## Ecosystem & Standalone Packaging
- **End Users:** Only need a `battlefield-opfor` release (which bundles `utility/opfor-cli.exe` and `utility/xenolith.exe`) plus their own legal copies of BFBC2 PC, BFBC1 Xbox 360 ISO, and/or BF1943 XBLA. **No `bfbc2-tool-suite`, Python, or .NET/C# runtime is required.**
- **Ecosystem Architecture:** `battlefield-opfor` stays 100% MIT-licensed while keeping `utility/xenolith.exe` as the separate LGPL-2.1 process boundary for XMA2 audio decoding. See [`../xenolith/docs/ECOSYSTEM_ROADMAP.md`](../xenolith/docs/ECOSYSTEM_ROADMAP.md) for the shared `xenolith` MIT format foundation and the planned game-agnostic `frostbite-tool-suite` (supporting native BFBC2/OPFOR, BF1943, BFBC1, and MoH 2010 MP map & mod authoring).

## Build & Test Commands

```powershell
# Run all unit tests
cargo test

# Build 64-bit Hub launcher, Setup wizard, and CLI
cargo build --release -p opfor-launcher -p opfor-setup -p opfor-cli

# Build 32-bit engine host and VFS proxy DLL
cargo build --release --target i686-pc-windows-msvc -p opfor-host -p opfor-dll
```
