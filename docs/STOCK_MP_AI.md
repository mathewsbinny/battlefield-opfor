# Stock-MP Bots Layer & Onslaught AI Guide

Pure-Rust authoring and runtime support for enabling AI bots on **Onslaught** and **stock BFBC2 multiplayer** maps under Project OPFOR's non-destructive Option B `modules/` VFS architecture.

## 1. What a Working AI Level Needs

Frostbite 1.5 multiplayer maps ship without an AI layer (`HasAI = false`, no `AreaOrder` / `PatrolOrder`, no `PathData.hpd`, no `SectorOffsets`). A functioning bot level requires four components:

1. **`LevelData.HasAI = true`**: When `false`, Frostbite's server level loader (`0x00575046`) skips AI init, `0x005958F0`, and Kynapse pathdata setup.
   - *Authoring*: `opfor-cli author-stockmp-ai` sets `<field name="HasAI">true</field>` in the level DBX.
   - *Runtime override*: `[dev] force_ai = true` in `config/opfor.ini` (`opfor-host/src/ai_runtime.rs`) forces `*(level + 0x3AE) = 1` at `0x00575046`.
2. **Real AI Template (`SpawnTemplate_Enemy_Grunt`)**: `DefaultSoldier` has a placeholder sensing profile (`CombatFOV = 0`) causing bots to aim without firing. Swapping to DICE's campaign grunt template (`objects/soldiers/templates/spawntemplate_enemy_grunt/2eedbef1-52de-4b20-819c-e71097c15cbe` + `Enemy_Grunt_Sense` + `Enemy_Grunt_Bhv` from `sp_common`) enables full combat firing.
3. **`AreaOrderEntityData` at Objectives / Capture Points**: Gives bots destinations at Conquest flags and Rush M-COM stations.
4. **Kynogon HPD v2 Navmesh + `SectorOffsets`**: Without `Levels/<LEVEL>/Precomputed/<Layer>/SectorOffsets.dbx`, Kynapse assumes `(0, 0)` sector offsets and route requests outside sector `(0, 0)` fail.
   - *Authoring*: `opfor-cli apply-sector-offsets` computes minimum `(sector_x, sector_z)` directly from `PathData.hpd` and injects `SectorOffsets.dbx`.
   - *Runtime override*: `[dev] sector_offsets = X,Z` in `config/opfor.ini` overrides zero offsets at `0x00F20B48` / `0x00F20BB0`.

## 2. Pure-Rust CLI Commands (`opfor-cli`)

All commands operate in-memory via `x360-frostbite` (zero Python or temp files) and never mutate stock `engine/` files when authoring into `modules/stockmp/`:

```powershell
# Author a non-destructive Stock-MP Bots Layer overlay into modules/stockmp/levels/<lv>/level-00.fbrb
.\utility\opfor-cli.exe author-stockmp-ai --level mp_002 --bots-per-team 8

# Inject SectorOffsets.dbx computed from a Kynogon HPD v2 navmesh
.\utility\opfor-cli.exe apply-sector-offsets --bundle modules\stockmp\levels\mp_002\level-00.fbrb --level MP_002 --hpd modules\stockmp\ai\levels\mp_002\Infantry\PathData.hpd --layer Infantry

# Swap DefaultSoldier templates -> DICE campaign grunt template on an Onslaught or MP bundle
.\utility\opfor-cli.exe apply-ai-template --bundle modules\onslaught\levels\ons_mp_002\level-00.fbrb --pack-grunt

# Refresh cache/package.mft so VFS routes modules/stockmp/
.\utility\opfor-cli.exe sync-cache
```

## 3. Runtime AI Telemetry (`config/opfor.ini`)

Enable `[dev] unit_counters_dump = true` (and optional `unit_counters_interval_s = 60`) to log cumulative per-pathfinder request, success, failure, resolve, stage-2, state-7, and distance-walked metrics to `user/logs/opfor_host.log`.
