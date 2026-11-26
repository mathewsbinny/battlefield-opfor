//! Essential offline/AI/WW2 level-compatibility guards for `opfor-host`.
//!
//! Ported from `project-opfor-bfbc2/src/dll/levelfix.cpp`:
//! - `PartSetup` (`0x008F4720`): skips static-model-group destructible members that lack a
//!   physics part on MP/AI maps (prevents null-deref crashes like Nelson Bay's Antenna_Generator).
//! - `StrEq` (`0x004304D0`): treats `"onslaught"` and `"airsuperiority"` (Coral Sea) as
//!   `"conquest"` (`0x0142ABD8`) so their flag HUD and loading screens render cleanly.

#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use opfor_vfs::ModeId;

const PART_SETUP: usize = 0x008F_4720;
const PART_SETUP_PROLOGUE: [u8; 6] = [0x83, 0xEC, 0x08, 0x57, 0x8B, 0xF9];

const STR_EQ: usize = 0x0043_04D0;
const STR_EQ_PROLOGUE: [u8; 6] = [0x8B, 0x4C, 0x24, 0x08, 0x8B, 0xC1];
const CONQUEST_LITERAL: usize = 0x0142_ABD8;

// Voice-over walk null-object / null-blueprint-object guard (prevents 0x006EA3F0 crash on WW2 kits)
const VO_WALK: usize = 0x006E_A340;
const VO_WALK_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x24, 0xC6, 0x04, 0x24, 0x00];
const VO_WALK_LIST: usize = 0x006D_BFE0;
const VO_WALK_LIST_PROLOGUE: [u8; 5] = [0x51, 0x8B, 0x4C, 0x24, 0x08];
const IS_KIND_OF: usize = 0x004E_6100;
const REF_OBJECT_DATA_TYPE: usize = 0x01C1_1E64;

// D3D11 SRV texture-binding guards (prevents d3d11.dll crash when a ported map binds a stale SRV)
const PS_BIND: usize = 0x009B_C68E;
const PS_BIND_PROLOGUE: [u8; 5] = [0x8B, 0x17, 0x8B, 0x52, 0x20];
const VS_BIND: usize = 0x009B_C661;
const VS_BIND_PROLOGUE: [u8; 5] = [0x8B, 0x17, 0x8B, 0x52, 0x64];

// Level-object ReferenceObjectData null-blueprint guards:
// - inner loop (`0x006EFD23`: `mov eax, [edi+0x78]; mov edi, [eax+0x50]`)
// - outer loop (`0x006EFCA1`: `mov eax, [esi+0x78]; mov ecx, [eax+0x50]`)
const REF_OBJ_BP: usize = 0x006E_FD23;
const REF_OBJ_BP_PROLOGUE: [u8; 6] = [0x8B, 0x47, 0x78, 0x8B, 0x78, 0x50];
const REF_OBJ_BP_CONT: usize = 0x006E_FD29;
const REF_OBJ_BP_SKIP: usize = 0x006E_FDA4;

const REF_OBJ_BP2: usize = 0x006E_FCA1;
const REF_OBJ_BP2_PROLOGUE: [u8; 6] = [0x8B, 0x46, 0x78, 0x8B, 0x48, 0x50];
const REF_OBJ_BP2_CONT: usize = 0x006E_FCA7;
const REF_OBJ_BP2_SKIP: usize = 0x006F_010D;

// VO_WALK child null-ptr guard (`0x006EAC60`: `mov ecx, [esi+0xd8]` -> deref `[ecx+0x40]`)
const VO_WALK_CHILD: usize = 0x006E_AC60;
const VO_WALK_CHILD_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0xD8, 0x00, 0x00, 0x00];
const VO_WALK_CHILD_CONT: usize = 0x006E_AC66;
const VO_WALK_CHILD_SKIP: usize = 0x006E_AC8A;

// VO_WALK sub-walk null-ptr guard (`0x0062AEE0`: `mov ecx, [eax+ebx*4]; mov ecx, [ecx+0x2c]`)
const VO_WALK_SUB: usize = 0x0062_AEE0;
const VO_WALK_SUB_PROLOGUE: [u8; 6] = [0x8B, 0x0C, 0x98, 0x8B, 0x49, 0x2C];
const VO_WALK_SUB_CONT: usize = 0x0062_AEE6;
const VO_WALK_SUB_SKIP: usize = 0x0062_AF1A;

// Entity-factory ReferenceObjectData null-blueprint guard (`0x005711EF`: `mov eax, [esi+0x78]; mov ebx, [eax+0x50]`)
const REF_OBJ_FACT: usize = 0x0057_11EF;
const REF_OBJ_FACT_PROLOGUE: [u8; 6] = [0x8B, 0x46, 0x78, 0x8B, 0x58, 0x50];
const REF_OBJ_FACT_CONT: usize = 0x0057_11F5;
const REF_OBJ_FACT_SKIP: usize = 0x0057_1218;

// Level entity-list null-arg2 guard (`0x006EBFF0`: `__thiscall(a0, a1, a2)` -> `a2->0x13fc1f0`)
const LEVEL_LIST_WALK: usize = 0x006E_BFF0;
const LEVEL_LIST_WALK_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x18, 0x53, 0x55, 0x56, 0x57];

// Universal null-this guard on `getArray_0x18` (`0x013FC1F0`: `out = this + 0x18`)
const GET_ARRAY_18: usize = 0x013F_C1F0;
const GET_ARRAY_18_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x83, 0xC1, 0x18];

// World-init null-entry guard (`0x00561709`: `mov eax, [eax+0x88]` -> deref `[eax+0x18]`)
const WORLD_ENTRY_GUARD: usize = 0x0056_1709;
const WORLD_ENTRY_GUARD_PROLOGUE: [u8; 6] = [0x8B, 0x80, 0x88, 0x00, 0x00, 0x00];
const WORLD_ENTRY_GUARD_CONT: usize = 0x0056_170F;
const WORLD_ENTRY_GUARD_SKIP: usize = 0x0056_174F;

// ResourceProxy::load null-string guard (`0x00459530`: `mov eax, [esp+4]; mov edx, [eax]`)
const RES_PROXY_LOAD: usize = 0x0045_9530;
const RES_PROXY_LOAD_PROLOGUE: [u8; 6] = [0x8B, 0x44, 0x24, 0x04, 0x8B, 0x10];

// MeshModel::init null-MeshAsset guard (`0x005D2070`: `__thiscall(asset, a1, a2)`)
const MESH_MODEL_INIT: usize = 0x005D_2070;
const MESH_MODEL_INIT_PROLOGUE: [u8; 7] = [0x83, 0x7C, 0x24, 0x0C, 0x01, 0x56, 0x57];

// Client/Server blueprint-lookup null guard (`0x00717035`: `mov ecx, [eax+0xd8]` -> deref `[ecx+0x50]`)
const BP_LOOKUP_GUARD: usize = 0x0071_7035;
const BP_LOOKUP_GUARD_PROLOGUE: [u8; 6] = [0x8B, 0x88, 0xD8, 0x00, 0x00, 0x00];
const BP_LOOKUP_GUARD_CONT: usize = 0x0071_703B;
const BP_LOOKUP_GUARD_SKIP: usize = 0x0071_705B;

// StaticModelGroup null-member-mesh guard (`0x008F9357`: `mov ecx, [esi+0xec]; mov eax, [edx+0x18]`)
const SMG_MESH_GUARD: usize = 0x008F_9357;
const SMG_MESH_GUARD_PROLOGUE: [u8; 9] = [0x8B, 0x8E, 0xEC, 0x00, 0x00, 0x00, 0x8B, 0x42, 0x18];
const SMG_MESH_GUARD_CONT: usize = 0x008F_9360;

// StaticModelGroup bbox null-mesh guard (`0x008F7207`: `mov ecx, [ebx]; mov eax, [ecx+0x44]`)
const SMG_BBOX_GUARD: usize = 0x008F_7207;
const SMG_BBOX_GUARD_PROLOGUE: [u8; 5] = [0x8B, 0x0B, 0x8B, 0x41, 0x44];
const SMG_BBOX_GUARD_CONT: usize = 0x008F_720C;
const SMG_BBOX_GUARD_SKIP: usize = 0x008F_728D;

// Client entity-factory ReferenceObjectData null-blueprint guard (`0x0082E515`: `mov eax, [ebx+0x78]; mov ecx, [eax+0x50]`)
const CLIENT_REF_OBJ_FACT: usize = 0x0082_E515;
const CLIENT_REF_OBJ_FACT_PROLOGUE: [u8; 6] = [0x8B, 0x43, 0x78, 0x8B, 0x48, 0x50];
const CLIENT_REF_OBJ_FACT_CONT: usize = 0x0082_E51B;
const CLIENT_REF_OBJ_FACT_SKIP: usize = 0x0082_E53F;

// Physics body null-this guard (`0x00AAEE20`: `mov edx, [ecx+0x88]`, `ret 4`)
const PHYS_SET_PART_GUARD: usize = 0x00AA_EE20;
const PHYS_SET_PART_PROLOGUE: [u8; 6] = [0x8B, 0x91, 0x88, 0x00, 0x00, 0x00];
const PHYS_SET_PART_CONT: usize = 0x00AA_EE26;
const PHYS_SET_PART_SKIP: usize = 0x00AA_EE4E;

// Physics body creation null-result guard (`0x0061084A`: `mov [esi+4], eax; mov edx, [eax]`)
const PHYS_CREATE_GUARD: usize = 0x0061_084A;
const PHYS_CREATE_PROLOGUE: [u8; 5] = [0x89, 0x46, 0x04, 0x8B, 0x10];
const PHYS_CREATE_CONT: usize = 0x0061_084F;
const PHYS_CREATE_SKIP: usize = 0x0061_0882;

// PhysicsBody::setUserData null-this guard (`0x00A9F0D0`: `mov eax, [esp+4]; mov [ecx+0x68], eax; ret 4`)
const PHYS_USERDATA_GUARD: usize = 0x00A9_F0D0;
const PHYS_USERDATA_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x89, 0x41, 0x68];

// StaticModelGroup client lightmap/mesh null guard (`0x0080065C`: `mov ecx, [eax]; mov edi, [ecx+0x44]; mov edx, [esp+0x10]; mov ebx, [edx+0x44]`)
const SMG_CLIENT_GUARD: usize = 0x0080_065C;
const SMG_CLIENT_PROLOGUE: [u8; 12] = [0x8B, 0x08, 0x8B, 0x79, 0x44, 0x8B, 0x54, 0x24, 0x10, 0x8B, 0x5A, 0x44];
const SMG_CLIENT_CONT: usize = 0x0080_0668;
const SMG_CLIENT_SKIP: usize = 0x0080_0681;

// Post-ingame physics-part query null guard (`0x006A0A09`: `mov ecx, [esi+0xb8]` -> deref `[ecx]`)
const PHYS_QUERY_GUARD: usize = 0x006A_0A09;
const PHYS_QUERY_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0xB8, 0x00, 0x00, 0x00];
const PHYS_QUERY_CONT: usize = 0x006A_0A0F;
const PHYS_QUERY_SKIP: usize = 0x006A_0B9B;

// Post-ingame component init null-arg0 guard (`0x0078F520`: `__thiscall(arg0)`)
const INGAME_COMP_INIT: usize = 0x0078_F520;
const INGAME_COMP_INIT_PROLOGUE: [u8; 7] = [0x56, 0x8B, 0xF1, 0x8B, 0x4C, 0x24, 0x08];

// Post-ingame component init2 null-arg1 guard (`0x0079E840`: `__thiscall(arg0, arg1)`)
const INGAME_COMP_INIT2: usize = 0x0079_E840;
const INGAME_COMP_INIT2_PROLOGUE: [u8; 5] = [0x53, 0x55, 0x56, 0x8B, 0xF1];

// Post-ingame component update null-field10 guard (`0x0078F5C0`: `__thiscall(arg0)`, `[esi+0x10]`)
const INGAME_COMP_UPDATE: usize = 0x0078_F5C0;
const INGAME_COMP_UPDATE_PROLOGUE: [u8; 6] = [0x56, 0x8B, 0xF1, 0x8B, 0x4E, 0x10];

// PhysicsBody::getTransform null-this guard (`0x00AA72E0`: `mov eax, [ecx+0x8c]; add eax, 0xe0; ret`)
const PHYS_GET_XFORM: usize = 0x00AA_72E0;
const PHYS_GET_XFORM_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0x8C, 0x00, 0x00, 0x00];

// Post-ingame component bool query null-field10 guard (`0x0078F620`: `mov ecx, [ecx+0x10]; mov eax, [ecx]`)
const INGAME_COMP_QUERY: usize = 0x0078_F620;
const INGAME_COMP_QUERY_PROLOGUE: [u8; 5] = [0x8B, 0x49, 0x10, 0x8B, 0x01];
const INGAME_COMP_QUERY_CONT: usize = 0x0078_F625;

// Trie parent-walk self-cycle guard (`0x004BCBE7`: `mov ecx, [esi+0xc]; test ecx, ecx`)
const TRIE_PARENT_CYCLE: usize = 0x004B_CBE7;
const TRIE_PARENT_CYCLE_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x0C, 0x85, 0xC9];
const TRIE_PARENT_CYCLE_CONT: usize = 0x004B_CBEC;

// MeshModel::init missing-resource guard (`0x005D20D4`: `mov eax, [esi]; mov ecx, [eax+0xc]`)
const MESH_MODEL_RES_NULL: usize = 0x005D_20D4;
const MESH_MODEL_RES_NULL_PROLOGUE: [u8; 5] = [0x8B, 0x06, 0x8B, 0x48, 0x0C];
const MESH_MODEL_RES_NULL_CONT: usize = 0x005D_20D9;

// Client tick null-field10 guard (`0x007B44A7`: `mov ecx, [esi+0x10]; mov edx, [ecx]`)
const CLIENT_TICK_GUARD: usize = 0x007B_44A7;
const CLIENT_TICK_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x10, 0x8B, 0x11];
const CLIENT_TICK_CONT: usize = 0x007B_44AC;
const CLIENT_TICK_SKIP: usize = 0x007B_44B5;

// StaticModelGroup primary-LOD null-mesh guard (`0x008F7174`: `mov eax, [edx]; mov eax, [eax+0x44]`)
const SMG_BBOX2_GUARD: usize = 0x008F_7174;
const SMG_BBOX2_PROLOGUE: [u8; 5] = [0x8B, 0x02, 0x8B, 0x40, 0x44];
const SMG_BBOX2_CONT: usize = 0x008F_7179;

// Client tick null-mesh guard (`0x007EA79E`: `mov ecx, [ebx]; cmp dword ptr [ecx+0xc], 1`)
const CLIENT_MESH_GUARD: usize = 0x007E_A79E;
const CLIENT_MESH_PROLOGUE: [u8; 6] = [0x8B, 0x0B, 0x83, 0x79, 0x0C, 0x01];
const CLIENT_MESH_CONT: usize = 0x007E_A7A4;
const CLIENT_MESH_SKIP: usize = 0x007E_A906;

// PhysicsBody::getPartBounds null-this guard (`0x00AA84D0`: `__thiscall(out_vec, idx)`, `ret 8`)
const PHYS_PART_BOUNDS: usize = 0x00AA_84D0;
const PHYS_PART_BOUNDS_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];

// StaticModelGroup client render-member null-MeshAsset guard (`0x008FC381`: `mov ecx, [ebx+0x4c]; xor ebx, ebx`)
// Skips creating/registering broken 0x60-byte StaticModel render objects when `[ecx+0x70]` (MeshAsset) is null,
// preventing downstream crashes across 0x00C2xxxx/0x00C3xxxx render/soot passes.
const SMG_RENDER_MEMBER_GUARD: usize = 0x008F_C381;
const SMG_RENDER_MEMBER_PROLOGUE: [u8; 5] = [0x8B, 0x4B, 0x4C, 0x33, 0xDB];
const SMG_RENDER_MEMBER_CONT: usize = 0x008F_C386;
const SMG_RENDER_MEMBER_SKIP: usize = 0x008F_C438;

// VO_WALK array-item null-blueprint guard (`0x006E05B5`: `mov ecx, [esi]; mov edx, [ecx+0x50]`)
const VO_WALK_ARR_GUARD: usize = 0x006E_05B5;
const VO_WALK_ARR_PROLOGUE: [u8; 5] = [0x8B, 0x0E, 0x8B, 0x51, 0x50];
const VO_WALK_ARR_CONT: usize = 0x006E_05BA;
const VO_WALK_ARR_SKIP: usize = 0x006E_05C5;

// Physics entity init null-body guard (`0x008EA0D2`: `mov ecx, [esi+0x5c]; mov edx, [ecx]`)
const PHYS_ENT_INIT_GUARD: usize = 0x008E_A0D2;
const PHYS_ENT_INIT_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x5C, 0x8B, 0x11];
const PHYS_ENT_INIT_CONT: usize = 0x008E_A0D7;
const PHYS_ENT_INIT_SKIP: usize = 0x008E_A0E2;

// Physics entity init2 null-body guard (`0x008E210A`: `mov ecx, [edi+0x5c]; mov edx, [ecx]`)
const PHYS_ENT_INIT2_GUARD: usize = 0x008E_210A;
const PHYS_ENT_INIT2_PROLOGUE: [u8; 5] = [0x8B, 0x4F, 0x5C, 0x8B, 0x11];
const PHYS_ENT_INIT2_CONT: usize = 0x008E_210F;
const PHYS_ENT_INIT2_SKIP: usize = 0x008E_214E;

// Entity init null-parent guard (`0x006F1894`: `mov ecx, [ecx+0x40]; add ecx, 0x170`)
const ENT_PARENT_GUARD: usize = 0x006F_1894;
const ENT_PARENT_PROLOGUE: [u8; 9] = [0x8B, 0x49, 0x40, 0x81, 0xC1, 0x70, 0x01, 0x00, 0x00];
const ENT_PARENT_CONT: usize = 0x006F_189D;
const ENT_PARENT_SKIP: usize = 0x006F_1919;

// Post-ingame entity/VO walk null-field10 guard (`0x006EF52D`: `mov ecx, [esi+0x10]; mov eax, [ecx]`)
const ENT_WALK_GUARD: usize = 0x006E_F52D;
const ENT_WALK_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x10, 0x8B, 0x01];
const ENT_WALK_CONT: usize = 0x006E_F532;
const ENT_WALK_SKIP: usize = 0x006E_F581;

// Entity update null-owner guard (`0x006DD5A9`: `cmp [eax+0x40], edx; je 0x6dd685`)
const ENT_OWNER_GUARD: usize = 0x006D_D5A9;
const ENT_OWNER_PROLOGUE: [u8; 9] = [0x39, 0x50, 0x40, 0x0F, 0x84, 0xD3, 0x00, 0x00, 0x00];
const ENT_OWNER_CONT: usize = 0x006D_D5B2;
const ENT_OWNER_SKIP: usize = 0x006D_D685;

// Entity float getter null-owner guard (`0x006CDD80`: `mov eax, [ecx+8]; mov ecx, [eax+0x40]; fld [ecx+0x90]; ret`)
const ENT_GET_FLOAT90_GUARD: usize = 0x006C_DD80;
const ENT_GET_FLOAT90_PROLOGUE: [u8; 6] = [0x8B, 0x41, 0x08, 0x8B, 0x48, 0x40];
const ENT_GET_FLOAT90_CONT: usize = 0x006C_DD86;

// Entity u32 getter null-owner guard (`0x006CDD50`: `mov eax, [ecx+8]; mov ecx, [eax+0x40]; mov eax, [ecx+0x4c]; ret`)
const ENT_GET_U32_4C_GUARD: usize = 0x006C_DD50;
const ENT_GET_U32_4C_PROLOGUE: [u8; 6] = [0x8B, 0x41, 0x08, 0x8B, 0x48, 0x40];
const ENT_GET_U32_4C_CONT: usize = 0x006C_DD56;

// StaticModelGroup occluder/sub-object null-data guard (`0x008FC0CC`: `mov esi, [ebx+0x138]`)
const SMG_OCCLUDER_GUARD: usize = 0x008F_C0CC;
const SMG_OCCLUDER_PROLOGUE: [u8; 6] = [0x8B, 0xB3, 0x38, 0x01, 0x00, 0x00];
const SMG_OCCLUDER_CONT: usize = 0x008F_C0D2;

// StaticModelGroup render-member2 null-MeshAsset guard (`0x008EDBAA`: `mov eax, [ebx+0x40]; mov [esp+0x10], eax`)
const SMG_RENDER_MEMBER2_GUARD: usize = 0x008E_DBAA;
const SMG_RENDER_MEMBER2_PROLOGUE: [u8; 7] = [0x8B, 0x43, 0x40, 0x89, 0x44, 0x24, 0x10];
const SMG_RENDER_MEMBER2_CONT: usize = 0x008E_DBB1;
const SMG_RENDER_MEMBER2_SKIP: usize = 0x008E_DC62;

// Destruction mesh init null-MeshAsset guard 1 (`0x00B9BF6C`: `mov ecx, [eax+0x78]; add esp, 8`)
const DESTR_MESH_GUARD1: usize = 0x00B9_BF6C;
const DESTR_MESH_GUARD1_PROLOGUE: [u8; 6] = [0x8B, 0x48, 0x78, 0x83, 0xC4, 0x08];
const DESTR_MESH_GUARD1_CONT: usize = 0x00B9_BF72;
const DESTR_MESH_GUARD1_SKIP: usize = 0x00B9_C06D;

// Destruction mesh init null-MeshAsset guard 2 (`0x00B9B9D8`: `mov ebx, [edx+0x78]; cmp dword ptr [ebx+0x44], 0`)
const DESTR_MESH_GUARD2: usize = 0x00B9_B9D8;
const DESTR_MESH_GUARD2_PROLOGUE: [u8; 7] = [0x8B, 0x5A, 0x78, 0x83, 0x7B, 0x44, 0x00];
const DESTR_MESH_GUARD2_CONT: usize = 0x00B9_B9DF;
const DESTR_MESH_GUARD2_SKIP: usize = 0x00B9_BB25;

// StaticModel/Destruction getBoundingRadius null-MeshAsset guards (`0x00B8BE50`, `0x00B8B6E0`, `0x008D59F0`)
const DESTR_RADIUS_GUARD1: usize = 0x00B8_BE50;
const DESTR_RADIUS_GUARD2: usize = 0x00B8_B6E0;
const DESTR_RADIUS_PROLOGUE: [u8; 6] = [0x8B, 0x41, 0x68, 0x8B, 0x48, 0x78];
const DESTR_RADIUS_CONT1: usize = 0x00B8_BE56;
const DESTR_RADIUS_CONT2: usize = 0x00B8_B6E6;
const DESTR_RADIUS_GUARD3: usize = 0x008D_59F0;
const DESTR_RADIUS_GUARD3_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0xF4, 0x00, 0x00, 0x00];

// StaticModel/Destruction getBoundingBox null-MeshModel guards (`0x00B93510` & `0x00B92B00`)
const DESTR_BBOX_GUARD1: usize = 0x00B9_3510;
const DESTR_BBOX_GUARD2: usize = 0x00B9_2B00;
const DESTR_BBOX_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];

// Audio/effect bank item null-entry guard (`0x00FF2ABF`: `mov ebp, [eax+0x14]; mov eax, [ebp+0xc]`)
const SOUND_BANK_ITEM_GUARD: usize = 0x00FF_2ABF;
const SOUND_BANK_ITEM_PROLOGUE: [u8; 6] = [0x8B, 0x68, 0x14, 0x8B, 0x45, 0x0C];
const SOUND_BANK_ITEM_CONT: usize = 0x00FF_2AC5;
const SOUND_BANK_ITEM_SKIP: usize = 0x00FF_2B11;

// Audio/effect bank wave-selector null-entry guard (`0x00FF27D8`: `mov edx, [eax+ecx]; mov ecx, [0x19a7cd8]`)
const SOUND_BANK_SELECT_GUARD: usize = 0x00FF_27D8;
const SOUND_BANK_SELECT_PROLOGUE: [u8; 9] = [0x8B, 0x14, 0x08, 0x8B, 0x0D, 0xD8, 0x7C, 0x9A, 0x01];
const SOUND_BANK_SELECT_CONT: usize = 0x00FF_27E1;

// Audio mix-item loop null-entry guard (`0x01015450`: `mov eax, [esp+0x1c]; fld qword ptr [0x14d9640]`)
const AUDIO_MIX_ITEM_GUARD: usize = 0x0101_5450;
const AUDIO_MIX_ITEM_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x1C, 0xDD, 0x05, 0x40, 0x96, 0x4D, 0x01];
const AUDIO_MIX_ITEM_CONT: usize = 0x0101_545A;
const AUDIO_MIX_ITEM_SKIP: usize = 0x0101_55D2;

// Sound node update null-arg0 guard (`0x00FD9E70`: `__thiscall(arg0) -> u8`, `ret 4`)
const SOUND_NODE_UPDATE: usize = 0x00FD_9E70;
const SOUND_NODE_UPDATE_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];

// Renderer shader/env param null-ptr guard (`0x0098BD66`: `mov eax, [esp+0x18]; movups xmm1, [eax+0x10]; movups xmm0, [edx+8]`)
const RENDER_PARAM_GUARD: usize = 0x0098_BD66;
const RENDER_PARAM_PROLOGUE: [u8; 12] = [0x8B, 0x44, 0x24, 0x18, 0x0F, 0x10, 0x48, 0x10, 0x0F, 0x10, 0x42, 0x08];
const RENDER_PARAM_CONT: usize = 0x0098_BD72;

// ShaderDatabase lookup null-table guard (`0x009C28A9`: `mov ecx, [esi+0xc]; or edx, eax`)
const SHADER_DB_LOOKUP_GUARD: usize = 0x009C_28A9;
const SHADER_DB_LOOKUP_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x0C, 0x0B, 0xD0];
const SHADER_DB_LOOKUP_CONT: usize = 0x009C_28AE;
const SHADER_DB_LOOKUP_SKIP: usize = 0x009C_28CE;

// PostFX/UI texture inv-size null-ITexture guard (`0x00BDFA30`: `mov edx, [ecx]; mov eax, [edx+0x20]`)
const POSTFX_TEX_SIZE_GUARD: usize = 0x00BD_FA30;
const POSTFX_TEX_SIZE_PROLOGUE: [u8; 5] = [0x8B, 0x11, 0x8B, 0x42, 0x20];
const POSTFX_TEX_SIZE_CONT: usize = 0x00BD_FA35;
const POSTFX_TEX_SIZE_SKIP: usize = 0x00BD_FA88;

// Mesh LOD selector null-Mesh/MeshSet guard (`0x00975DF0`: `mov eax, [esp+4]; mov ecx, [eax+0x40]`)
const MESH_LOD_SELECT_GUARD: usize = 0x0097_5DF0;
const MESH_LOD_SELECT_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x8B, 0x48, 0x40];
const MESH_LOD_SELECT_CONT: usize = 0x0097_5DF7;

// PhysicsBody::getField150 null-this guard (`0x00AA75B0`: `mov eax, [ecx+0x8c]; add eax, 0x150; ret`)
const PHYS_GET_VEL_GUARD: usize = 0x00AA_75B0;
const PHYS_GET_VEL_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0x8C, 0x00, 0x00, 0x00];

// Entity factory lookup null-result guard (`0x0071DEC0`: `mov edx, [esi]; mov eax, [edx+8]`)
const SPAWN_ENT_NULL_GUARD: usize = 0x0071_DEC0;
const SPAWN_ENT_NULL_PROLOGUE: [u8; 5] = [0x8B, 0x16, 0x8B, 0x42, 0x08];
const SPAWN_ENT_NULL_CONT: usize = 0x0071_DEC5;
const SPAWN_ENT_NULL_SKIP: usize = 0x0071_DEFB;

// Entity flags update null-ptr guard (`0x008BC1AA`: `mov eax, [ebp+0x124]`)
const ENT_FLAGS_NULL_GUARD: usize = 0x008B_C1AA;
const ENT_FLAGS_NULL_PROLOGUE: [u8; 6] = [0x8B, 0x85, 0x24, 0x01, 0x00, 0x00];
const ENT_FLAGS_NULL_CONT: usize = 0x008B_C1B0;
const ENT_FLAGS_NULL_SKIP: usize = 0x008B_C1C3;

// Entity flags read null-ptr guard (`0x008AC1B4`: `mov ebx, [eax+0x10]; mov ecx, [edi+4]`)
const ENT_FLAGS2_NULL_GUARD: usize = 0x008A_C1B4;
const ENT_FLAGS2_NULL_PROLOGUE: [u8; 6] = [0x8B, 0x58, 0x10, 0x8B, 0x4F, 0x04];
const ENT_FLAGS2_NULL_CONT: usize = 0x008A_C1BA;

// Entity data null-ptr guard (`0x0082CE96`: `mov edx, [esi+4]; mov eax, [edx+0xc]`)
const ENT_DATA_NULL_GUARD: usize = 0x0082_CE96;
const ENT_DATA_NULL_PROLOGUE: [u8; 6] = [0x8B, 0x56, 0x04, 0x8B, 0x42, 0x0C];
const ENT_DATA_NULL_CONT: usize = 0x0082_CE9C;
const ENT_DATA_NULL_SKIP: usize = 0x0082_CF12;

// Entity component query null-field2c guard (`0x0068D2E0`: `mov ecx, [ecx+0x2c]; mov eax, [ecx]`)
const ENT_COMP2C_NULL_GUARD: usize = 0x0068_D2E0;
const ENT_COMP2C_NULL_PROLOGUE: [u8; 5] = [0x8B, 0x49, 0x2C, 0x8B, 0x01];
const ENT_COMP2C_NULL_CONT: usize = 0x0068_D2E5;

// Entity component query null-field3c guard (`0x007BEE91`: `mov ecx, [esi+0x3c]; mov edx, [ecx]`)
const ENT_COMP3C_NULL_GUARD: usize = 0x007B_EE91;
const ENT_COMP3C_NULL_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x3C, 0x8B, 0x11];
const ENT_COMP3C_NULL_CONT: usize = 0x007B_EE96;
const ENT_COMP3C_NULL_SKIP: usize = 0x007B_EEAB;

// Trie::insert thread-safety lock (`0x004C36D0`)
const TRIE_INSERT_LOCK: usize = 0x004C_36D0;
const TRIE_INSERT_LOCK_PROLOGUE: [u8; 8] = [0x83, 0xEC, 0x10, 0x53, 0x8B, 0x5C, 0x24, 0x20];

// Small-block pool slab recovery (`0x0041A1FC`, `0x0041A306`, `0x00427406`)
// Recovers cleanly with a fresh 4 KB slab if a corrupted free-list pointer is encountered.
const POOL_POP1_GUARD: usize = 0x0041_A1FC;
const POOL_POP1_PROLOGUE: [u8; 5] = [0x8B, 0x7E, 0x20, 0x8B, 0x07];
const POOL_POP1_CONT: usize = 0x0041_A201;
const POOL_POP1_REFILL: usize = 0x0041_A1E3;

const POOL_POP2_GUARD: usize = 0x0041_A306;
const POOL_POP2_PROLOGUE: [u8; 5] = [0x8B, 0x46, 0x20, 0x8B, 0x08];
const POOL_POP2_CONT: usize = 0x0041_A30B;
const POOL_POP2_REFILL: usize = 0x0041_A2ED;

const POOL_COUNT_GUARD: usize = 0x0042_7406;
const POOL_COUNT_PROLOGUE: [u8; 5] = [0x8B, 0x00, 0x83, 0xC5, 0x01];
const POOL_COUNT_CONT: usize = 0x0042_740B;
const POOL_COUNT_DONE: usize = 0x0042_740F;

const POOL_COLLECT_GUARD: usize = 0x0042_7510;
const POOL_COLLECT_PROLOGUE: [u8; 7] = [0x89, 0x01, 0x8B, 0x00, 0x83, 0xC1, 0x04];
const POOL_COLLECT_CONT: usize = 0x0042_7517;
const POOL_COLLECT_DONE: usize = 0x0042_751B;

// Physics/collision slot null-entry guard (`0x005F6354`: `mov [esp+0x10], eax; mov eax, [eax+0xc]`)
const PHYS_SLOT_NULL_GUARD: usize = 0x005F_6354;
const PHYS_SLOT_NULL_PROLOGUE: [u8; 7] = [0x89, 0x44, 0x24, 0x10, 0x8B, 0x40, 0x0C];
const PHYS_SLOT_NULL_CONT: usize = 0x005F_635B;
const PHYS_SLOT_NULL_SKIP: usize = 0x005F_6DEA;

// Entity update null-owner/data guard (`0x006E6D20`: `push ecx; push esi; mov esi, ecx`, `ret`)
const ENT_UPD_DATA_GUARD: usize = 0x006E_6D20;
const ENT_UPD_DATA_PROLOGUE: [u8; 8] = [0x51, 0x56, 0x8B, 0xF1, 0x83, 0x7E, 0x7C, 0x00];
const ENT_UPD_DATA_CONT: usize = 0x006E_6D28;

// Entity bbox merge null-MeshAsset guard (`0x008946CA`: `mov eax, [eax]; movups xmm1, [esi]; mov eax, [eax+0x44]`)
const ENT_BBOX_MERGE_GUARD: usize = 0x0089_46CA;
const ENT_BBOX_MERGE_PROLOGUE: [u8; 8] = [0x8B, 0x00, 0x0F, 0x10, 0x0E, 0x8B, 0x40, 0x44];
const ENT_BBOX_MERGE_CONT: usize = 0x0089_46D2;
const ENT_BBOX_MERGE_SKIP: usize = 0x0089_4790;

// Render bucket null-table guard (`0x00970D52`: `xor esi, esi; cmp [edx], esi; mov [esp+0x1c], esi`)
const RENDER_BUCKET_GUARD: usize = 0x0097_0D52;
const RENDER_BUCKET_PROLOGUE: [u8; 8] = [0x33, 0xF6, 0x39, 0x32, 0x89, 0x74, 0x24, 0x1C];
const RENDER_BUCKET_CONT: usize = 0x0097_0D5A;
const RENDER_BUCKET_SKIP: usize = 0x0097_0F66;

// Client entity init null-field10 guard (`0x00832938`: `mov edx, [esi+0x10]; mov ecx, [edx+0x40]`)
const CLIENT_ENT_F10_GUARD: usize = 0x0083_2938;
const CLIENT_ENT_F10_PROLOGUE: [u8; 6] = [0x8B, 0x56, 0x10, 0x8B, 0x4A, 0x40];
const CLIENT_ENT_F10_CONT: usize = 0x0083_293E;
const CLIENT_ENT_F10_SKIP: usize = 0x0083_294C;

// Component attach null-arg0 guard (`0x007991C0`: `push ebx; push esi; mov esi, ecx; mov ecx, [esp+0xc]`, `ret 0x10`)
const COMP_ATTACH_NULL_GUARD: usize = 0x0079_91C0;
const COMP_ATTACH_NULL_PROLOGUE: [u8; 8] = [0x53, 0x56, 0x8B, 0xF1, 0x8B, 0x4C, 0x24, 0x0C];
const COMP_ATTACH_NULL_CONT: usize = 0x0079_91C8;

// Entity tick null-field2c guard (`0x006B8BD0`: `push ebx; push esi; mov esi, ecx; mov ecx, [esi+0x2c]`, `ret`)
const ENT_TICK2C_NULL_GUARD: usize = 0x006B_8BD0;
const ENT_TICK2C_NULL_PROLOGUE: [u8; 7] = [0x53, 0x56, 0x8B, 0xF1, 0x8B, 0x4E, 0x2C];
const ENT_TICK2C_NULL_CONT: usize = 0x006B_8BD7;

// Client physics impulse null-body guard (`0x0087E048`: `mov edx, [esi]; mov eax, [edx+0x84]`)
const CLIENT_IMPULSE_GUARD: usize = 0x0087_E048;
const CLIENT_IMPULSE_PROLOGUE: [u8; 8] = [0x8B, 0x16, 0x8B, 0x82, 0x84, 0x00, 0x00, 0x00];
const CLIENT_IMPULSE_CONT: usize = 0x0087_E050;
const CLIENT_IMPULSE_SKIP: usize = 0x0087_E250;

// Client entity init2 null-field10 guard (`0x008329D6`: `mov eax, [esi+0x10]; fstp [esi+0x670]`)
const CLIENT_ENT_F10B_GUARD: usize = 0x0083_29D6;
const CLIENT_ENT_F10B_PROLOGUE: [u8; 9] = [0x8B, 0x46, 0x10, 0xD9, 0x9E, 0x70, 0x06, 0x00, 0x00];
const CLIENT_ENT_F10B_CONT: usize = 0x0083_29DF;
const CLIENT_ENT_F10B_SKIP: usize = 0x0083_29F6;

// Entity component update null-field08 guard (`0x0078F3C1`: `mov ecx, [esi+8]; fstp [esp+4]`)
const ENT_COMP08_GUARD: usize = 0x0078_F3C1;
const ENT_COMP08_PROLOGUE: [u8; 7] = [0x8B, 0x4E, 0x08, 0xD9, 0x5C, 0x24, 0x04];
const ENT_COMP08_CONT: usize = 0x0078_F3C8;
const ENT_COMP08_SKIP: usize = 0x0078_F3E7;

// StaticModel/Destruction getBoundingRadius null-MeshAsset guard 4 (`0x00894360`: `mov eax, [ecx+0x1cc]`)
const DESTR_RADIUS_GUARD4: usize = 0x0089_4360;
const DESTR_RADIUS_GUARD4_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0xCC, 0x01, 0x00, 0x00];

// Physics/collision slot null-esi write guard (`0x005F6CDE`: `test eax, eax; mov [esi+0x90], eax`)
const PHYS_SLOT_ESI_GUARD: usize = 0x005F_6CDE;
const PHYS_SLOT_ESI_PROLOGUE: [u8; 8] = [0x85, 0xC0, 0x89, 0x86, 0x90, 0x00, 0x00, 0x00];
const PHYS_SLOT_ESI_CONT: usize = 0x005F_6CE6;
const PHYS_SLOT_ESI_SKIP: usize = 0x005F_6DEA;

// MeshModel::setLod null-MeshAsset guard (`0x005CA970`: `mov eax, [ecx]; mov edx, [eax+0x44]`, `ret 4`)
const MESH_SET_LOD_GUARD: usize = 0x005C_A970;
const MESH_SET_LOD_PROLOGUE: [u8; 5] = [0x8B, 0x01, 0x8B, 0x50, 0x44];
const MESH_SET_LOD_CONT: usize = 0x005C_A975;

// Entity component update2 null-field08 guard (`0x007993BA`: `mov ecx, [esi+8]; mov edx, [ecx]`)
const ENT_COMP08_B_GUARD: usize = 0x0079_93BA;
const ENT_COMP08_B_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x08, 0x8B, 0x11];
const ENT_COMP08_B_CONT: usize = 0x0079_93BF;
const ENT_COMP08_B_SKIP: usize = 0x0079_93C8;

// Client physics callback null-arg0 guard (`0x00873EC9`: `mov ecx, [ebp+8]; mov edx, [ecx]`)
const CLIENT_PHYS_CB_GUARD: usize = 0x0087_3EC9;
const CLIENT_PHYS_CB_PROLOGUE: [u8; 5] = [0x8B, 0x4D, 0x08, 0x8B, 0x11];
const CLIENT_PHYS_CB_CONT: usize = 0x0087_3ECE;
const CLIENT_PHYS_CB_SKIP: usize = 0x0087_3EDB;

// Entity component update3 null-field08 guard (`0x007A088F`: `mov ecx, [esi+8]; fstp [esp+8]`)
const ENT_COMP08_C_GUARD: usize = 0x007A_088F;
const ENT_COMP08_C_PROLOGUE: [u8; 7] = [0x8B, 0x4E, 0x08, 0xD9, 0x5C, 0x24, 0x08];
const ENT_COMP08_C_CONT: usize = 0x007A_0896;
const ENT_COMP08_C_SKIP: usize = 0x007A_089D;

// Client entity tick null-field3c guard (`0x007D0952`: `mov ecx, [esi+0x3c]; mov edx, [esp+0x18]`)
const CLIENT_ENT3C_GUARD: usize = 0x007D_0952;
const CLIENT_ENT3C_PROLOGUE: [u8; 7] = [0x8B, 0x4E, 0x3C, 0x8B, 0x54, 0x24, 0x18];
const CLIENT_ENT3C_CONT: usize = 0x007D_0959;
const CLIENT_ENT3C_SKIP: usize = 0x007D_097A;

// Client entity switch null-field04 guard (`0x0081D056`: `mov ecx, [esi+4]; mov eax, [ecx+0x40]`)
const CLIENT_ENT04_GUARD: usize = 0x0081_D056;
const CLIENT_ENT04_PROLOGUE: [u8; 6] = [0x8B, 0x4E, 0x04, 0x8B, 0x41, 0x40];
const CLIENT_ENT04_CONT: usize = 0x0081_D05C;
const CLIENT_ENT04_SKIP: usize = 0x0081_DAFA;

// Client entity tick2 null-field3c guard (`0x007D4A6C`: `mov ecx, [esi+0x3c]; mov eax, [esp+0x1c]`)
const CLIENT_ENT3C_B_GUARD: usize = 0x007D_4A6C;
const CLIENT_ENT3C_B_PROLOGUE: [u8; 7] = [0x8B, 0x4E, 0x3C, 0x8B, 0x44, 0x24, 0x1C];
const CLIENT_ENT3C_B_CONT: usize = 0x007D_4A73;
const CLIENT_ENT3C_B_SKIP: usize = 0x007D_4A8C;

// Physics body update null-fielda0 guard (`0x00ACDE20`: `mov ecx, [esi+0xa0]; mov eax, [ecx]`)
const PHYS_BODY_A0_GUARD: usize = 0x00AC_DE20;
const PHYS_BODY_A0_PROLOGUE: [u8; 8] = [0x8B, 0x8E, 0xA0, 0x00, 0x00, 0x00, 0x8B, 0x01];
const PHYS_BODY_A0_CONT: usize = 0x00AC_DE28;

// Entity component query2/3 null-field3c guards (`0x007BEF04` & `0x007E0255`)
const ENT_COMP3C_B_GUARD: usize = 0x007B_EF04;
const ENT_COMP3C_B_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x3C, 0x8B, 0x01];
const ENT_COMP3C_B_CONT: usize = 0x007B_EF09;
const ENT_COMP3C_B_SKIP: usize = 0x007B_EF19;

const ENT_COMP3C_C_GUARD: usize = 0x007E_0255;
const ENT_COMP3C_C_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x3C, 0x8B, 0x01];
const ENT_COMP3C_C_CONT: usize = 0x007E_025A;
const ENT_COMP3C_C_SKIP: usize = 0x007E_02C1;

// Trie sibling-unlink null-end guard (`0x004C36A1`: `mov eax, [eax+4]; cmp [eax+4], esi`)
const TRIE_UNLINK_GUARD: usize = 0x004C_36A1;
const TRIE_UNLINK_PROLOGUE: [u8; 6] = [0x8B, 0x40, 0x04, 0x39, 0x70, 0x04];
const TRIE_UNLINK_CONT: usize = 0x004C_36A7;
const TRIE_UNLINK_SKIP: usize = 0x004C_36AC;

// DBX attribute null-string guard (`0x004EB52A`: `mov edi, ebx; mov esi, 0x0140b95c`)
const DBX_NULL_ATTR_GUARD: usize = 0x004E_B52A;
const DBX_NULL_ATTR_PROLOGUE: [u8; 7] = [0x8B, 0xFB, 0xBE, 0x5C, 0xB9, 0x40, 0x01];
const DBX_NULL_ATTR_CONT: usize = 0x004E_B531;
const DBX_NULL_ATTR_NULL: usize = 0x004E_B53C;

// SP blueprint array null-entry guards (`0x005AC6D6` & `0x005AC704`)
const SP_BP_ENTRY_GUARD: usize = 0x005A_C6D6;
const SP_BP_ENTRY_PROLOGUE: [u8; 5] = [0x8B, 0x0B, 0x8B, 0x71, 0x50];
const SP_BP_ENTRY_CONT: usize = 0x005A_C6DB;
const SP_BP_ENTRY_SKIP: usize = 0x005A_C759;

const SP_BP_ESI_GUARD: usize = 0x005A_C704;
const SP_BP_ESI_PROLOGUE: [u8; 7] = [0x83, 0xBE, 0xF0, 0x00, 0x00, 0x00, 0x00];
const SP_BP_ESI_CONT: usize = 0x005A_C70B;

// SP AI manager null-field6c guards (`0x00E9BA00` & `0x00F2F03B`)
const SP_AI_MGR_GUARD: usize = 0x00E9_BA00;
const SP_AI_MGR_PROLOGUE: [u8; 8] = [0xA1, 0xE8, 0x03, 0x89, 0x01, 0x8B, 0x50, 0x6C];
const SP_AI_MGR_CONT: usize = 0x00E9_BA08;

const SP_AI_MGR2_GUARD: usize = 0x00F2_F03B;
const SP_AI_MGR2_PROLOGUE: [u8; 6] = [0x8B, 0x43, 0x6C, 0x8B, 0x70, 0x74];
const SP_AI_MGR2_CONT: usize = 0x00F2_F041;
const SP_AI_MGR2_EXIT: usize = 0x00F2_F4F9;

const SP_AI_MGR3_GUARD: usize = 0x00EB_6800;
const SP_AI_MGR3_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];
const SP_AI_MGR3_CONT: usize = 0x00EB_6806;

const SP_AI_MGR4_GUARD: usize = 0x00EB_C3E7;
const SP_AI_MGR4_PROLOGUE: [u8; 6] = [0x8B, 0x15, 0xE8, 0x03, 0x89, 0x01];
const SP_AI_MGR4_CONT: usize = 0x00EB_C3F6;

const SP_AI_MGR5_GUARD: usize = 0x00F2_7F1E;
const SP_AI_MGR5_PROLOGUE: [u8; 6] = [0x8B, 0x8B, 0xE4, 0x03, 0x00, 0x00];
const SP_AI_MGR5_CONT: usize = 0x00F2_7F24;

// SP object list #2 null-entry guard (`0x005A8B3C`: `mov edx, [esp+0x1c]; mov esi, [edx]`)
const SP_LIST2_NULL_GUARD: usize = 0x005A_8B3C;
const SP_LIST2_NULL_PROLOGUE: [u8; 6] = [0x8B, 0x54, 0x24, 0x1C, 0x8B, 0x32];
const SP_LIST2_NULL_CONT: usize = 0x005A_8B42;
const SP_LIST2_NULL_SKIP: usize = 0x005A_8D6E;

// SP missing resource proxy null guard (`0x00F6808C`: `mov ecx, [edi]; call 0xf36b80`)
const SP_ANIM_RES_GUARD: usize = 0x00F6_808C;
const SP_ANIM_RES_PROLOGUE: [u8; 7] = [0x8B, 0x0F, 0xE8, 0xED, 0xEA, 0xFC, 0xFF];
const SP_ANIM_RES_CONT: usize = 0x00F6_8093;
const SP_ANIM_RES_EXIT: usize = 0x00F6_817C;

// SP anim ctor null-arg guard (`0x00F36BC0`: `mov [eax+0x18], edx; mov ecx, [ecx+4]`)
const SP_ANIM_CTOR_GUARD: usize = 0x00F3_6BC0;
const SP_ANIM_CTOR_PROLOGUE: [u8; 6] = [0x89, 0x50, 0x18, 0x8B, 0x49, 0x04];
const SP_ANIM_CTOR_CONT: usize = 0x00F3_6BC6;

// SP object list #3 null-entry guard (`0x00571DD0`: `mov eax, [esp+0x18]; mov esi, [eax+edi*4]`)
const SP_LIST3_NULL_GUARD: usize = 0x0057_1DD0;
const SP_LIST3_NULL_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x18, 0x8B, 0x34, 0xB8];
const SP_LIST3_NULL_CONT: usize = 0x0057_1DD7;
const SP_LIST3_NULL_SKIP: usize = 0x0057_1E30;

// SP animatable data null guard (`0x0062A7BE`: `mov ecx, [edi+0xc8]; mov ecx, [ecx+0x3c]`)
const SP_ANIM_DATA_GUARD: usize = 0x0062_A7BE;
const SP_ANIM_DATA_PROLOGUE: [u8; 6] = [0x8B, 0x8F, 0xC8, 0x00, 0x00, 0x00];
const SP_ANIM_DATA_CONT: usize = 0x0062_A7C7;
const SP_ANIM_DATA_EXIT: usize = 0x0062_A878;

// SP anim init null `[ebx]` guard (`0x00F6C112`: `mov edx, [ebx]; cmp [edx+0x5c], edi`)
const SP_ANIM_INIT1_GUARD: usize = 0x00F6_C112;
const SP_ANIM_INIT1_PROLOGUE: [u8; 5] = [0x8B, 0x13, 0x39, 0x7A, 0x5C];
const SP_ANIM_INIT1_CONT: usize = 0x00F6_C117;
const SP_ANIM_INIT1_SKIP: usize = 0x00F6_C146;

// SP anim init null `[ebx+4]` guard (`0x00F6C17A`: `mov ecx, [ebx+4]; call 0xf36b80`)
const SP_ANIM_INIT2_GUARD: usize = 0x00F6_C17A;
const SP_ANIM_INIT2_PROLOGUE: [u8; 8] = [0x8B, 0x4B, 0x04, 0xE8, 0xFE, 0xA9, 0xFC, 0xFF];
const SP_ANIM_INIT2_CONT: usize = 0x00F6_C182;

// SP AI manager #6 guard (`0x00EFD2F2`: `mov ecx, [eax+0x18]; fld [ecx+0x2c]`)
const SP_AI_MGR6_GUARD: usize = 0x00EF_D2F2;
const SP_AI_MGR6_PROLOGUE: [u8; 6] = [0x8B, 0x48, 0x18, 0xD9, 0x41, 0x2C];
const SP_AI_MGR6_CONT: usize = 0x00EF_D2F8;

// SP physics float `+0x8c` null guard (`0x00B11175`: `fld dword ptr [eax+0x8c]`)
const SP_PHYS_F8C_GUARD: usize = 0x00B1_1175;
const SP_PHYS_F8C_PROLOGUE: [u8; 6] = [0xD9, 0x80, 0x8C, 0x00, 0x00, 0x00];
const SP_PHYS_F8C_CONT: usize = 0x00B1_117B;

// SP `[ebx+0x14]` null vtable guard (`0x006EC021`: `mov ecx, [ebx+0x14]; mov edx, [ecx]`)
const SP_TERR_B14_GUARD: usize = 0x006E_C021;
const SP_TERR_B14_PROLOGUE: [u8; 5] = [0x8B, 0x4B, 0x14, 0x8B, 0x11];
const SP_TERR_B14_CONT: usize = 0x006E_C026;
const SP_TERR_B14_SKIP: usize = 0x006E_C0E9;

// SP physics add null-arg guard (`0x00AF7140`: `mov eax, [esp+4]; mov edx, [eax+0x2c]`)
const SP_PHYS_ADD_GUARD: usize = 0x00AF_7140;
const SP_PHYS_ADD_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x8B, 0x50, 0x2C];
const SP_PHYS_ADD_CONT: usize = 0x00AF_7147;

// SP Trie::find null string guard (`0x004C3ABC`: `mov edx, edi; mov eax, ebp; sub edx, ebp`)
const SP_TRIE_FIND_GUARD: usize = 0x004C_3ABC;
const SP_TRIE_FIND_PROLOGUE: [u8; 6] = [0x8B, 0xD7, 0x8B, 0xC5, 0x2B, 0xD5];
const SP_TRIE_FIND_CONT: usize = 0x004C_3AC2;
const SP_TRIE_FIND_SKIP: usize = 0x004C_3AD8;

// SP entity `[esi+0x44]` null flag-write guard (`0x00554A85`)
const SP_ENT44_FLAG_GUARD: usize = 0x0055_4A85;
const SP_ENT44_FLAG_PROLOGUE: [u8; 10] = [0x8B, 0x46, 0x44, 0x81, 0x48, 0x10, 0x00, 0x00, 0x00, 0x02];
const SP_ENT44_FLAG_CONT: usize = 0x0055_4A8F;
const SP_ENT44_FLAG_SKIP: usize = 0x0055_4A92;

// SP physics `[esi+0x9c]` null guard (`0x00B090A5`)
const SP_PHYS_9C_GUARD: usize = 0x00B0_90A5;
const SP_PHYS_9C_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0x9C, 0x00, 0x00, 0x00];
const SP_PHYS_9C_CONT: usize = 0x00B0_90AB;
const SP_PHYS_9C_EXIT: usize = 0x00B0_90E9;

// SP AI manager #7 guard (`0x00E94AAD`: `mov eax, [eax+0x18]; movss xmm0, [eax+0x38]`)
const SP_AI_MGR7_GUARD: usize = 0x00E9_4AAD;
const SP_AI_MGR7_PROLOGUE: [u8; 8] = [0x8B, 0x40, 0x18, 0xF3, 0x0F, 0x10, 0x40, 0x38];
const SP_AI_MGR7_CONT: usize = 0x00E9_4AB5;

// SP Trie::insert null string guard (`0x004C37BB`: `mov ecx, ebx; mov [esp+0x10], ebx`)
const SP_TRIE_INS_GUARD: usize = 0x004C_37BB;
const SP_TRIE_INS_PROLOGUE: [u8; 6] = [0x8B, 0xCB, 0x89, 0x5C, 0x24, 0x10];
const SP_TRIE_INS_CONT: usize = 0x004C_37C1;
const SP_TRIE_INS_SKIP: usize = 0x004C_37F1;

// SP entity `[esi+0x44]` null vtable+0xc0 guard (`0x0053E37C`)
const SP_ENT44_C0_GUARD: usize = 0x0053_E37C;
const SP_ENT44_C0_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x44, 0x8B, 0x01];
const SP_ENT44_C0_CONT: usize = 0x0053_E381;
const SP_ENT44_C0_SKIP: usize = 0x0053_E3C6;

// SP physics null arg1 guard (`0x00ABECF0`: `sub esp, 8; mov eax, [esp+0x10]`)
const SP_PHYS_80_GUARD: usize = 0x00AB_ECF0;
const SP_PHYS_80_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x08, 0x8B, 0x44, 0x24, 0x10];
const SP_PHYS_80_CONT: usize = 0x00AB_ECF7;

// SP AI manager #8 guard (`0x00E52D05`: `mov edi, [eax+0x18]; mov eax, [ebx+0xb00]`)
const SP_AI_MGR8_GUARD: usize = 0x00E5_2D05;
const SP_AI_MGR8_PROLOGUE: [u8; 9] = [0x8B, 0x78, 0x18, 0x8B, 0x83, 0x00, 0x0B, 0x00, 0x00];
const SP_AI_MGR8_CONT: usize = 0x00E5_2D0E;

// SP entity `[esi+0x44]` null flag-write #2 guard (`0x0053E5F7`)
const SP_ENT44_FLAG2_GUARD: usize = 0x0053_E5F7;
const SP_ENT44_FLAG2_PROLOGUE: [u8; 10] = [0x8B, 0x46, 0x44, 0x81, 0x48, 0x10, 0x00, 0x00, 0x00, 0x02];
const SP_ENT44_FLAG2_CONT: usize = 0x0053_E601;
const SP_ENT44_FLAG2_SKIP: usize = 0x0053_E604;

// SP physics `[eax+0x9c]/[eax+0xa0]` null guard (`0x00B03F20`)
const SP_PHYS_4C_GUARD: usize = 0x00B0_3F20;
const SP_PHYS_4C_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];
const SP_PHYS_4C_CONT: usize = 0x00B0_3F26;

// SP AI manager #9/#10/#11 guards (`0x00E6D673`, `0x00E6D8D0`, `0x00E6DA89`)
const SP_AI_MGR9_GUARD: usize = 0x00E6_D673;
const SP_AI_MGR9_PROLOGUE: [u8; 6] = [0x8B, 0x48, 0x18, 0xD9, 0x41, 0x40];
const SP_AI_MGR9_CONT: usize = 0x00E6_D679;

const SP_AI_MGR10_GUARD: usize = 0x00E6_D8D0;
const SP_AI_MGR10_PROLOGUE: [u8; 6] = [0x8B, 0x48, 0x18, 0xD9, 0x41, 0x28];
const SP_AI_MGR10_CONT: usize = 0x00E6_D8D6;

const SP_AI_MGR11_GUARD: usize = 0x00E6_DA89;
const SP_AI_MGR11_PROLOGUE: [u8; 6] = [0x8B, 0x48, 0x18, 0xD9, 0x41, 0x30];
const SP_AI_MGR11_CONT: usize = 0x00E6_DA8F;

// SP physics null `this` guard (`0x00A9E7F0`: `mov eax, [ecx+0x10]; and eax, 0x40000000`)
const SP_PHYS_10_GUARD: usize = 0x00A9_E7F0;
const SP_PHYS_10_PROLOGUE: [u8; 8] = [0x8B, 0x41, 0x10, 0x25, 0x00, 0x00, 0x00, 0x40];
const SP_PHYS_10_CONT: usize = 0x00A9_E7F8;

// SP entity `[esi+0x44]` null vtable+0xc0 #2 guard (`0x0053E66C`)
const SP_ENT44_C0_B_GUARD: usize = 0x0053_E66C;
const SP_ENT44_C0_B_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x44, 0x8B, 0x01];
const SP_ENT44_C0_B_CONT: usize = 0x0053_E671;
const SP_ENT44_C0_B_SKIP: usize = 0x0053_E6B6;

// SP physics `[ecx+0x9c]` null guard #2 (`0x00B08CE0`)
const SP_PHYS_9C_B_GUARD: usize = 0x00B0_8CE0;
const SP_PHYS_9C_B_PROLOGUE: [u8; 5] = [0x83, 0xEC, 0x14, 0x53, 0x55];
const SP_PHYS_9C_B_CONT: usize = 0x00B0_8CE5;

// SP AI manager #12 guard (`0x00EBC4F0`: `mov eax, [0x018903e8]`)
const SP_AI_MGR12_GUARD: usize = 0x00EB_C4F0;
const SP_AI_MGR12_PROLOGUE: [u8; 5] = [0xA1, 0xE8, 0x03, 0x89, 0x01];
const SP_AI_MGR12_CONT: usize = 0x00EB_C4F5;

// SP empty-vector last-element guard (`0x0062805C`: `mov ecx, [edi+eax*4-4]; mov edx, [ecx+0x68]`)
const SP_VEC_LAST_GUARD: usize = 0x0062_805C;
const SP_VEC_LAST_PROLOGUE: [u8; 7] = [0x8B, 0x4C, 0x87, 0xFC, 0x8B, 0x51, 0x68];
const SP_VEC_LAST_CONT: usize = 0x0062_8063;
const SP_VEC_LAST_ZERO: usize = 0x0062_8067;

// SP entity `[esi+8]` null vtable+0xe0 guard (`0x0071C3DB`)
const SP_ENT08_E0_GUARD: usize = 0x0071_C3DB;
const SP_ENT08_E0_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x08, 0x8B, 0x11];
const SP_ENT08_E0_CONT: usize = 0x0071_C3E0;
const SP_ENT08_E0_SKIP: usize = 0x0071_C40A;

// SP physics `[esi+0x44]` null guard (`0x00B0C25A`: `mov ecx, [esi+0x44]; mov byte [esi+0x5c], 1`)
const SP_PHYS_44_GUARD: usize = 0x00B0_C25A;
const SP_PHYS_44_PROLOGUE: [u8; 7] = [0x8B, 0x4E, 0x44, 0xC6, 0x46, 0x5C, 0x01];
const SP_PHYS_44_CONT: usize = 0x00B0_C261;
const SP_PHYS_44_SKIP: usize = 0x00B0_C27E;

// SP AI manager #13 guard (`0x00F2CDA5`: `mov esi, [ebp+0x38]; push edi; mov edi, [esi+8]`)
const SP_AI_MGR13_GUARD: usize = 0x00F2_CDA5;
const SP_AI_MGR13_PROLOGUE: [u8; 7] = [0x8B, 0x75, 0x38, 0x57, 0x8B, 0x7E, 0x08];
const SP_AI_MGR13_CONT: usize = 0x00F2_CDAC;
const SP_AI_MGR13_SKIP: usize = 0x00F2_CDE4;

// SP Havok body null shape `[ecx+0x10]` guard (`0x00CCC290`)
const SP_PHYS_C290_GUARD: usize = 0x00CC_C290;
const SP_PHYS_C290_PROLOGUE: [u8; 5] = [0x56, 0x8B, 0x74, 0x24, 0x08];
const SP_PHYS_C290_CONT: usize = 0x00CC_C295;

// SP entity `[ecx+8]` null vtable+0xc4/c8 guard (`0x00611E2F`)
const SP_ENT08_C4_GUARD: usize = 0x0061_1E2F;
const SP_ENT08_C4_PROLOGUE: [u8; 5] = [0x8B, 0x49, 0x08, 0x8B, 0x11];
const SP_ENT08_C4_CONT: usize = 0x0061_1E34;

// SP entity `[ebp+0x80]` null float+0x68 guard (`0x006DC736`)
const SP_ENT80_68_GUARD: usize = 0x006D_C736;
const SP_ENT80_68_PROLOGUE: [u8; 6] = [0x8B, 0x85, 0x80, 0x00, 0x00, 0x00];
const SP_ENT80_68_CONT: usize = 0x006D_C73C;
const SP_ENT80_68_EXIT: usize = 0x006D_C72F;

// SP AI manager #14/#15 guards (`0x00F2CE18` `[ebp+0x3e4]`, `0x00F2CE25` `[ebp+0x3dc]`)
const SP_AI_MGR14_GUARD: usize = 0x00F2_CE18;
const SP_AI_MGR14_PROLOGUE: [u8; 6] = [0x8B, 0x8D, 0xE4, 0x03, 0x00, 0x00];
const SP_AI_MGR14_CONT: usize = 0x00F2_CE1E;
const SP_AI_MGR14_SKIP: usize = 0x00F2_CE25;

const SP_AI_MGR15_GUARD: usize = 0x00F2_CE25;
const SP_AI_MGR15_PROLOGUE: [u8; 6] = [0x8B, 0x8D, 0xDC, 0x03, 0x00, 0x00];
const SP_AI_MGR15_CONT: usize = 0x00F2_CE2B;
const SP_AI_MGR15_SKIP: usize = 0x00F2_CE31;

// SP physics float `+0x80` null guard (`0x00B115A3`: `fld dword ptr [ecx+0x80]`)
const SP_PHYS_F80_GUARD: usize = 0x00B1_15A3;
const SP_PHYS_F80_PROLOGUE: [u8; 6] = [0xD9, 0x81, 0x80, 0x00, 0x00, 0x00];
const SP_PHYS_F80_CONT: usize = 0x00B1_15A9;

// SP AI manager #16 guard (`0x00F2A320`: `movzx eax, byte ptr [ecx+0x66c]`, `ret 4`)
const SP_AI_MGR16_GUARD: usize = 0x00F2_A320;
const SP_AI_MGR16_PROLOGUE: [u8; 7] = [0x0F, 0xB6, 0x81, 0x6C, 0x06, 0x00, 0x00];
const SP_AI_MGR16_CONT: usize = 0x00F2_A327;

// SP AI manager #17 guard (`0x00EB7290`: `mov ecx, [eax+0x7c]; fsub [ecx+0x10]`)
const SP_AI_MGR17_GUARD: usize = 0x00EB_7290;
const SP_AI_MGR17_PROLOGUE: [u8; 6] = [0x8B, 0x48, 0x7C, 0xD8, 0x61, 0x10];
const SP_AI_MGR17_CONT: usize = 0x00EB_7296;

// SP type index null-arg guard (`0x00FE2A20`: `push ebx; mov ebx, [esp+8]`, `ret 4`)
const SP_TYPE_IDX_GUARD: usize = 0x00FE_2A20;
const SP_TYPE_IDX_PROLOGUE: [u8; 5] = [0x53, 0x8B, 0x5C, 0x24, 0x08];
const SP_TYPE_IDX_CONT: usize = 0x00FE_2A25;

// SP Havok body null shape `[esi+0x10]` #2 guard (`0x00CCC247`)
const SP_PHYS_C250_GUARD: usize = 0x00CC_C247;
const SP_PHYS_C250_PROLOGUE: [u8; 9] = [0x8B, 0x4E, 0x10, 0xDC, 0x0D, 0x88, 0x94, 0x4D, 0x01];
const SP_PHYS_C250_CONT: usize = 0x00CC_C250;
const SP_PHYS_C250_EXIT: usize = 0x00CC_C284;

// SP AI manager #18 guard (`0x00EB9EB5`: `fld dword ptr [ecx+0xc0]`)
const SP_AI_MGR18_GUARD: usize = 0x00EB_9EB5;
const SP_AI_MGR18_PROLOGUE: [u8; 6] = [0xD9, 0x81, 0xC0, 0x00, 0x00, 0x00];
const SP_AI_MGR18_CONT: usize = 0x00EB_9EBB;

// SP physics `[esi+0x9c]` null guard #3 (`0x00B11628`: `mov edx, [esi+0x9c]`)
const SP_PHYS_9C_C_GUARD: usize = 0x00B1_1628;
const SP_PHYS_9C_C_PROLOGUE: [u8; 6] = [0x8B, 0x96, 0x9C, 0x00, 0x00, 0x00];
const SP_PHYS_9C_C_CONT: usize = 0x00B1_162E;
const SP_PHYS_9C_C_SKIP: usize = 0x00B1_16C6;

// SP physics `[esi+0x44]` null guard #2 (`0x00B0F57B`: `mov eax, [esi+0x44]; mov [esi+0x5c], bl`)
const SP_PHYS_44_B_GUARD: usize = 0x00B0_F57B;
const SP_PHYS_44_B_PROLOGUE: [u8; 6] = [0x8B, 0x46, 0x44, 0x88, 0x5E, 0x5C];
const SP_PHYS_44_B_CONT: usize = 0x00B0_F581;
const SP_PHYS_44_B_SKIP: usize = 0x00B0_F5A3;

// SP physics callback setter null `[ecx+0x44]` guard (`0x00AF82A0`: `mov eax, [ecx+0x44]; mov ecx, [esp+4]`)
const SP_PHYS_CB24_GUARD: usize = 0x00AF_82A0;
const SP_PHYS_CB24_PROLOGUE: [u8; 7] = [0x8B, 0x41, 0x44, 0x8B, 0x4C, 0x24, 0x04];

// SP null data-array (`ecx == 0x10`) guard (`0x006EC8B1`: `mov ecx, [esp+0x18]; mov edx, [ecx]`)
const SP_ARR10_GUARD: usize = 0x006E_C8B1;
const SP_ARR10_PROLOGUE: [u8; 6] = [0x8B, 0x4C, 0x24, 0x18, 0x8B, 0x11];
const SP_ARR10_CONT: usize = 0x006E_C8B7;
const SP_ARR10_EXIT: usize = 0x006E_C987;

// SP vehicle slot table null-entry guard (`0x006BD434`: `mov ecx, [esi+0x2b8]`)
const SP_VEH_SLOT_GUARD: usize = 0x006B_D434;
const SP_VEH_SLOT_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0xB8, 0x02, 0x00, 0x00];
const SP_VEH_SLOT_CONT: usize = 0x006B_D443;
const SP_VEH_SLOT_EXIT: usize = 0x006B_D478;

// SP physics `[esi+0xa0]` null-entry guard (`0x00B02255`: `mov ecx, [esi+0xa0]`)
const SP_PHYS_A0_GUARD: usize = 0x00B0_2255;
const SP_PHYS_A0_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0xA0, 0x00, 0x00, 0x00];
const SP_PHYS_A0_CONT: usize = 0x00B0_225E;
const SP_PHYS_A0_EXIT: usize = 0x00B0_2273;

// SP vehicle slot table #2 null-entry guard (`0x0069F52E`: `mov esi, [esi+0x1f8]`)
const SP_VEH_SLOT2_GUARD: usize = 0x0069_F52E;
const SP_VEH_SLOT2_PROLOGUE: [u8; 6] = [0x8B, 0xB6, 0xF8, 0x01, 0x00, 0x00];
const SP_VEH_SLOT2_CONT: usize = 0x0069_F53D;

// SP null `[eax+0x20]` byte check guard (`0x00594EDA`: `cmp byte [eax+0x20], 0; jne 0x59504e`)
const SP_AI_OBJ20_GUARD: usize = 0x0059_4EDA;
const SP_AI_OBJ20_PROLOGUE: [u8; 10] = [0x80, 0x78, 0x20, 0x00, 0x0F, 0x85, 0x6A, 0x01, 0x00, 0x00];
const SP_AI_OBJ20_CONT: usize = 0x0059_4EE4;
const SP_AI_OBJ20_JNE: usize = 0x0059_504E;
const SP_AI_OBJ20_RET: usize = 0x0059_4ECC;

// SP null `esi` deref (`0x005ACE2A: je 0x5ace57` falls into `mov ecx, [esi+0x14]`!)
const SP_AI_ESI14_GUARD: usize = 0x005A_CE57;
const SP_AI_ESI14_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x14, 0x85, 0xC9];
const SP_AI_ESI14_CONT: usize = 0x005A_CE5C;
const SP_AI_ESI14_SKIP: usize = 0x005A_CE9B;

// SP null `edi` deref (`0x0072BA21`: `mov edi, eax; mov eax, [edi+0x3c]`)
const SP_ENT_EDI3C_GUARD: usize = 0x0072_BA21;
const SP_ENT_EDI3C_PROLOGUE: [u8; 5] = [0x8B, 0xF8, 0x8B, 0x47, 0x3C];
const SP_ENT_EDI3C_CONT: usize = 0x0072_BA26;
const SP_ENT_EDI3C_SKIP: usize = 0x0072_BAB4;

// SP AI transform copy null `esi`/`eax` guard (`0x00E93ED2`: `mov ecx, 0x10; rep movsd`)
const SP_AI_MATCPY_GUARD: usize = 0x00E9_3ED2;
const SP_AI_MATCPY_PROLOGUE: [u8; 7] = [0xB9, 0x10, 0x00, 0x00, 0x00, 0xF3, 0xA5];
const SP_AI_MATCPY_CONT: usize = 0x00E9_3ED9;
const SP_AI_MATCPY_EXIT: usize = 0x00E9_3EEF;

// SP AI getter null `this` guard (`0x00E93A10`: `mov eax, [ecx+0xac]; ret`)
const SP_AI_GETAC_GUARD: usize = 0x00E9_3A10;
const SP_AI_GETAC_PROLOGUE: [u8; 7] = [0x8B, 0x81, 0xAC, 0x00, 0x00, 0x00, 0xC3];

// SP AI array getter null `[eax+0x1c]` guard (`0x0058F830`: `mov eax, [ecx+4]; mov ecx, [eax+0x1c]`)
const SP_AI_ARR1C_GUARD: usize = 0x0058_F830;
const SP_AI_ARR1C_PROLOGUE: [u8; 6] = [0x8B, 0x41, 0x04, 0x8B, 0x48, 0x1C];

// SP AI navmesh check null `esi` guard (`0x00F10960`: `mov edx, [esi]; test al, al; mov eax, [edx+0x18]`)
const SP_AI_NAV_GUARD: usize = 0x00F1_0960;
const SP_AI_NAV_PROLOGUE: [u8; 7] = [0x8B, 0x16, 0x84, 0xC0, 0x8B, 0x42, 0x18];
const SP_AI_NAV_CONT: usize = 0x00F1_0967;

// SP AI sub-data null `eax` guard (`0x005AF9F8`: `cmp dword [eax+0x14], 0; push esi`)
const SP_AI_SUB14_GUARD: usize = 0x005A_F9F8;
const SP_AI_SUB14_PROLOGUE: [u8; 5] = [0x83, 0x78, 0x14, 0x00, 0x56];
const SP_AI_SUB14_CONT: usize = 0x005A_F9FD;
const SP_AI_SUB14_EXIT: usize = 0x005A_FA04;

// Recursive node-chain cycle/ptr guard (`0x004BCBE0`: `mov eax, [esp+4]; push esi; mov esi, ecx`)
const SP_NODE_WALK_GUARD: usize = 0x004B_CBE0;
const SP_NODE_WALK_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x56, 0x8B, 0xF1];
const SP_NODE_WALK_CONT: usize = 0x004B_CBE7;

// SP physics `[esi+0x44]` null guard #3 (`0x00B0F698`: `mov ecx, [esi+0x44]; call 0xd40b70`)
const SP_PHYS_44_C_GUARD: usize = 0x00B0_F698;
const SP_PHYS_44_C_PROLOGUE: [u8; 8] = [0x8B, 0x4E, 0x44, 0xE8, 0xD0, 0x14, 0x23, 0x00];
const SP_PHYS_44_C_CONT: usize = 0x00B0_F6A0;
const SP_PHYS_44_C_EXIT: usize = 0x00B0_F752;

// SP physics `[esi]` null guard (`0x00B2BD80`: `mov ecx, [esi]; mov edx, [ecx]; mov edx, [edx+0x48]`)
const SP_PHYS_BD80_GUARD: usize = 0x00B2_BD80;
const SP_PHYS_BD80_PROLOGUE: [u8; 7] = [0x8B, 0x0E, 0x8B, 0x11, 0x8B, 0x52, 0x48];
const SP_PHYS_BD80_CONT: usize = 0x00B2_BD87;
const SP_PHYS_BD80_SKIP: usize = 0x00B2_BD95;

// SP AI navmesh query null `[esp+0x24]` guard (`0x00E6B2FB`: `mov ebx, [esp+0x24]; mov edx, [ebx]`)
const SP_AI_EBX24_GUARD: usize = 0x00E6_B2FB;
const SP_AI_EBX24_PROLOGUE: [u8; 6] = [0x8B, 0x5C, 0x24, 0x24, 0x8B, 0x13];
const SP_AI_EBX24_CONT: usize = 0x00E6_B301;
const SP_AI_EBX24_SKIP: usize = 0x00E6_B430;

// SP Frostbite 1.0 PathDataDefinition (.hpd) incompatible navmesh registration skip (`0x00EFB240`, `ret 0x10`)
const SP_AI_PATHDATA_GUARD: usize = 0x00EF_B240;
const SP_AI_PATHDATA_PROLOGUE: [u8; 6] = [0x83, 0xEC, 0x14, 0x56, 0x8B, 0xF1];

// SP physics invalid `[esp+0x18]`/`esi` guard (`0x00B2BD0A`: `mov ecx, [esp+0x18]; mov [esi], eax`)
const SP_PHYS_BD0A_GUARD: usize = 0x00B2_BD0A;
const SP_PHYS_BD0A_PROLOGUE: [u8; 6] = [0x8B, 0x4C, 0x24, 0x18, 0x89, 0x06];
const SP_PHYS_BD0A_CONT: usize = 0x00B2_BD10;
const SP_PHYS_BD0A_SKIP: usize = 0x00B2_BD1F;

// SP in-game AI query null `ecx` guard (`0x00EB8660`: `mov edx, [ecx]; mov eax, [edx+4]`)
const SP_AI_EB86_GUARD: usize = 0x00EB_8660;
const SP_AI_EB86_PROLOGUE: [u8; 5] = [0x8B, 0x11, 0x8B, 0x42, 0x04];
const SP_AI_EB86_CONT: usize = 0x00EB_8665;
const SP_AI_EB86_EXIT: usize = 0x00EB_8794;

// SP entity `[esi+0x190]` null guard (`0x007192EC`: `mov ecx, [esi+0x190]`)
const SP_PHYS_190_GUARD: usize = 0x0071_92EC;
const SP_PHYS_190_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0x90, 0x01, 0x00, 0x00];
const SP_PHYS_190_CONT: usize = 0x0071_92F2;
const SP_PHYS_190_SKIP: usize = 0x0071_9306;

// SP Havok raycast null `this` guard (`0x00ABC970`: `push ebx; mov ebx, esp; sub esp, 8`, `ret 0xc`)
const SP_PHYS_C970_GUARD: usize = 0x00AB_C970;
const SP_PHYS_C970_PROLOGUE: [u8; 6] = [0x53, 0x8B, 0xDC, 0x83, 0xEC, 0x08];
const SP_PHYS_C970_CONT: usize = 0x00AB_C976;

// SP in-game AI tick null `[esi+4]` guard (`0x00EBD20A`: `mov ecx, [esi+4]; mov edx, [ecx]`)
const SP_AI_EBD2_GUARD: usize = 0x00EB_D20A;
const SP_AI_EBD2_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x04, 0x8B, 0x11];
const SP_AI_EBD2_CONT: usize = 0x00EB_D20F;
const SP_AI_EBD2_SKIP: usize = 0x00EB_D28C;

// SP audio/emitter invalid `[eax]` ptr guard (`0x00FDA3B8`: `mov eax, [eax]; mov ecx, [eax+0xc]; mov edx, [eax+0x10]`)
const SP_FDA3_GUARD: usize = 0x00FD_A3B8;
const SP_FDA3_PROLOGUE: [u8; 8] = [0x8B, 0x00, 0x8B, 0x48, 0x0C, 0x8B, 0x50, 0x10];
const SP_FDA3_CONT: usize = 0x00FD_A3C0;

// Trie sibling-unlink corrupted-ptr guard (`0x004C3678`: `mov [eax+0xc], edi; mov eax, [eax+4]`)
const TRIE_UNLINK2_GUARD: usize = 0x004C_3678;
const TRIE_UNLINK2_PROLOGUE: [u8; 6] = [0x89, 0x78, 0x0C, 0x8B, 0x40, 0x04];
const TRIE_UNLINK2_CONT: usize = 0x004C_367E;
const TRIE_UNLINK2_SKIP: usize = 0x004C_36AC;

// Invalid allocator `this` fallback (`0x0040A410`: `mov eax, [ecx]; mov eax, [eax]; jmp eax`)
const SP_ALLOC_GUARD: usize = 0x0040_A410;
const SP_ALLOC_PROLOGUE: [u8; 6] = [0x8B, 0x01, 0x8B, 0x00, 0xFF, 0xE0];

// Trie::find corrupted node pointer guard (`0x004C3AA0`: `cmp esi, ebx; je 0x4c3afa; test byte [esi+8], 1`)
const TRIE_FIND_PTR_GUARD: usize = 0x004C_3AA0;
const TRIE_FIND_PTR_PROLOGUE: [u8; 8] = [0x3B, 0xF3, 0x74, 0x56, 0xF6, 0x46, 0x08, 0x01];
const TRIE_FIND_PTR_CONT: usize = 0x004C_3AA8;
const TRIE_FIND_PTR_NOTFOUND: usize = 0x004C_3AFA;

// Trie::firstLeaf corrupted node pointer guard (`0x004BCBC2`: `mov eax, ecx; and eax, -4; mov ecx, [eax+8]`)
const TRIE_LEAF_GUARD: usize = 0x004B_CBC2;
const TRIE_LEAF_PROLOGUE: [u8; 8] = [0x8B, 0xC1, 0x83, 0xE0, 0xFC, 0x8B, 0x48, 0x08];
const TRIE_LEAF_CONT: usize = 0x004B_CBCA;

// Resource ref-list corrupted `esi` guard (`0x004968C0`: `mov eax, [esi+0xc]; test eax, eax`)
const SP_LIST_4968_GUARD: usize = 0x0049_68C0;
const SP_LIST_4968_PROLOGUE: [u8; 5] = [0x8B, 0x46, 0x0C, 0x85, 0xC0];
const SP_LIST_4968_CONT: usize = 0x0049_68C5;
const SP_LIST_4968_EXIT: usize = 0x0049_69A2;

// Null arg1 guard (`0x004495E0`: `mov eax, [esp+4]; mov eax, [eax+0x10]`)
const SP_4495E0_GUARD: usize = 0x0044_95E0;
const SP_4495E0_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x8B, 0x40, 0x10];
const SP_4495E0_CONT: usize = 0x0044_95E7;

// Null ebx lookup guard (`0x009C4058`: `mov edi, [ebx+0x10]; cmp [ecx-8], edi`)
const SP_9C4055_GUARD: usize = 0x009C_4058;
const SP_9C4055_PROLOGUE: [u8; 6] = [0x8B, 0x7B, 0x10, 0x39, 0x79, 0xF8];
const SP_9C4055_CONT: usize = 0x009C_405E;
const SP_9C4055_EXIT: usize = 0x009C_40C8;

// Null esi guard (`0x004493A3`: `cmp dword ptr [esi+0x10], 0; mov ecx, esi`)
const SP_4493A3_GUARD: usize = 0x0044_93A3;
const SP_4493A3_PROLOGUE: [u8; 6] = [0x83, 0x7E, 0x10, 0x00, 0x8B, 0xCE];
const SP_4493A3_CONT: usize = 0x0044_93A9;
const SP_4493A3_SKIP: usize = 0x0044_93D6;

// Small-block pool ASCII-pointer write guard (`0x004CF9A2`: `test eax, eax; jz +0xb; mov ecx, [eax-8]`)
const SP_POOL_4CF9_GUARD: usize = 0x004C_F9A2;
const SP_POOL_4CF9_PROLOGUE: [u8; 7] = [0x85, 0xC0, 0x74, 0x0B, 0x8B, 0x48, 0xF8];
const SP_POOL_4CF9_AFTER: usize = 0x004C_F9B1;

// Trie::find null-parent `this=0x38` guard (`0x004C8950`: `mov eax, [esp+8]; sub esp, 0x400`)
const SP_TRIE_4C8950_GUARD: usize = 0x004C_8950;
const SP_TRIE_4C8950_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x08, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];
const SP_TRIE_4C8950_CONT: usize = 0x004C_895A;

// Sibling Trie null-parent `this` guards (`0x004C89B0`, `0x004C8A10`, `0x004C8A70`, `0x004C8AD0`, `0x004C8C80`)
const SP_TRIE_4C89B0_GUARD: usize = 0x004C_89B0;
const SP_TRIE_4C89B0_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x04, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];
const SP_TRIE_4C89B0_CONT: usize = 0x004C_89BA;

const SP_TRIE_4C8A10_GUARD: usize = 0x004C_8A10;
const SP_TRIE_4C8A10_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x04, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];
const SP_TRIE_4C8A10_CONT: usize = 0x004C_8A1A;

const SP_TRIE_4C8A70_GUARD: usize = 0x004C_8A70;
const SP_TRIE_4C8A70_PROLOGUE: [u8; 10] = [0x8B, 0x44, 0x24, 0x04, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00];
const SP_TRIE_4C8A70_CONT: usize = 0x004C_8A7A;

const SP_TRIE_4C8AD0_GUARD: usize = 0x004C_8AD0;
const SP_TRIE_4C8AD0_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x0C, 0x53, 0x56, 0x8B, 0xF1];
const SP_TRIE_4C8AD0_CONT: usize = 0x004C_8AD7;

const SP_TRIE_4C8C80_GUARD: usize = 0x004C_8C80;
const SP_TRIE_4C8C80_PROLOGUE: [u8; 6] = [0x81, 0xEC, 0xF4, 0x00, 0x00, 0x00];
const SP_TRIE_4C8C80_CONT: usize = 0x004C_8C86;

// Trie/allocator ASCII pointer guard (`0x004CF9A6`: `mov ecx, [eax-8]; mov [eax], ecx`)
const SP_4CF9A6_GUARD: usize = 0x004C_F9A6;
const SP_4CF9A6_PROLOGUE: [u8; 5] = [0x8B, 0x48, 0xF8, 0x89, 0x08];
const SP_4CF9A6_CONT: usize = 0x004C_F9AB;
const SP_4CF9A6_SKIP: usize = 0x004C_F9B1;

// SP object list null-entry guard (`0x005A8A34`: `mov edx, [esi]; mov eax, [esp+0x14]`)
const SP_LIST_NULL_GUARD: usize = 0x005A_8A34;
const SP_LIST_NULL_PROLOGUE: [u8; 6] = [0x8B, 0x16, 0x8B, 0x44, 0x24, 0x14];
const SP_LIST_NULL_CONT: usize = 0x005A_8A3A;
const SP_LIST_NULL_SKIP: usize = 0x005A_8A67;

// SP level-walk null-entry guard (`0x006EFF8A`: `mov eax, [esi+0x10]; sub eax, [esi+0xc]`)
const SP_LEVEL_WALK_GUARD: usize = 0x006E_FF8A;
const SP_LEVEL_WALK_PROLOGUE: [u8; 6] = [0x8B, 0x46, 0x10, 0x2B, 0x46, 0x0C];
const SP_LEVEL_WALK_CONT: usize = 0x006E_FF90;
const SP_LEVEL_WALK_SKIP: usize = 0x006F_010D;

// Entity getBounds null-MeshAsset guard (`0x007D49FF`: `mov ecx, [ecx]; mov eax, [ecx+0x44]`)
const ENT_BBOX_MESH_GUARD: usize = 0x007D_49FF;
const ENT_BBOX_MESH_PROLOGUE: [u8; 5] = [0x8B, 0x09, 0x8B, 0x41, 0x44];
const ENT_BBOX_MESH_CONT: usize = 0x007D_4A04;

// Entity component sub-object null-field14 guards (`0x0078EDA0` & `0x0078FA0D`)
const ENT_COMP14_GUARD: usize = 0x0078_EDA0;
const ENT_COMP14_PROLOGUE: [u8; 5] = [0x8B, 0xC1, 0x8B, 0x48, 0x04];
const ENT_COMP14_CONT: usize = 0x0078_EDA5;

const ENT_COMP14_B_GUARD: usize = 0x0078_FA0D;
const ENT_COMP14_B_PROLOGUE: [u8; 5] = [0x8B, 0x46, 0x08, 0x8B, 0x11];
const ENT_COMP14_B_CONT: usize = 0x0078_FA12;

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
static mut G_PS_BIND_BACK: usize = 0;
#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
static mut G_VS_BIND_BACK: usize = 0;

static D3D_LO: AtomicUsize = AtomicUsize::new(0);
static D3D_HI: AtomicUsize = AtomicUsize::new(0);

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleExA(flags: u32, name: *const c_void, out: *mut *mut c_void) -> i32;
    fn GetModuleFileNameA(module: *mut c_void, buf: *mut u8, size: u32) -> u32;
    fn IsBadReadPtr(ptr: *const c_void, len: usize) -> i32;
}

#[repr(C)]
struct EaString {
    begin: *const u8,
    end: *const u8,
}

#[cfg(target_arch = "x86")]
type PartSetupFn = unsafe extern "fastcall" fn(usize, *mut c_void);
#[cfg(target_arch = "x86")]
type PartLookupFn = unsafe extern "fastcall" fn(usize, *mut c_void, u32) -> usize;
#[cfg(target_arch = "x86")]
type StrEqFn = unsafe extern "cdecl" fn(*const EaString, *const u8) -> bool;
#[cfg(target_arch = "x86")]
type VoWalkFn = unsafe extern "cdecl" fn(*mut c_void, *mut c_void, *mut c_void);
#[cfg(target_arch = "x86")]
type LevelListWalkFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void);
#[cfg(target_arch = "x86")]
type ResProxyLoadFn = unsafe extern "fastcall" fn(
    *mut c_void,
    *mut c_void,
    *const *const u8,
    usize,
    usize,
    usize,
    usize,
);
#[cfg(target_arch = "x86")]
type GetTypeFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> *mut c_void;
#[cfg(target_arch = "x86")]
type IsKindOfFn = unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void) -> bool;

#[cfg(target_arch = "x86")]
static ORIG_PART_SETUP: OnceLock<PartSetupFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_STR_EQ: OnceLock<StrEqFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_VO_WALK: OnceLock<VoWalkFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_VO_WALK_LIST: OnceLock<VoWalkFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_LEVEL_LIST_WALK: OnceLock<LevelListWalkFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_RES_PROXY_LOAD: OnceLock<ResProxyLoadFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_MESH_MODEL_INIT: OnceLock<LevelListWalkFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_INGAME_COMP_INIT: OnceLock<unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void)> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_INGAME_COMP_INIT2: OnceLock<
    unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void),
> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_INGAME_COMP_UPDATE: OnceLock<
    unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void),
> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_PHYS_PART_BOUNDS: OnceLock<
    unsafe extern "fastcall" fn(usize, *mut c_void, *mut [f32; 4], usize) -> *mut [f32; 4],
> = OnceLock::new();
#[cfg(target_arch = "x86")]
type DestrBboxFn = unsafe extern "fastcall" fn(usize, *mut c_void, *mut [f32; 8]) -> *mut [f32; 8];
#[cfg(target_arch = "x86")]
static ORIG_DESTR_BBOX1: OnceLock<DestrBboxFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
static ORIG_DESTR_BBOX2: OnceLock<DestrBboxFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
type SoundNodeUpdateFn = unsafe extern "fastcall" fn(usize, *mut c_void, *const c_void) -> u8;
#[cfg(target_arch = "x86")]
static ORIG_SOUND_NODE_UPDATE: OnceLock<SoundNodeUpdateFn> = OnceLock::new();
#[cfg(target_arch = "x86")]
type TrieInsertFn = unsafe extern "fastcall" fn(usize, *mut c_void, *mut c_void, *mut c_void, *const u8) -> *mut c_void;
#[cfg(target_arch = "x86")]
static ORIG_TRIE_INSERT_LOCK: OnceLock<TrieInsertFn> = OnceLock::new();

pub unsafe fn install(mode: ModeId) -> (usize, usize) {
    let mut installed = 0usize;
    let is_console_port = matches!(mode, ModeId::Bf1943 | ModeId::Bfbc1);
    let is_bfbc1 = mode == ModeId::Bfbc1;
    let expected = if is_bfbc1 {
        194
    } else if is_console_port {
        6
    } else {
        2
    };
    #[cfg(target_arch = "x86")]
    {
        // Universal guards (all eras: BFBC2, Vietnam, BF1943, BFBC1)
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                PART_SETUP,
                &PART_SETUP_PROLOGUE,
                detour_part_setup as *const c_void,
            )
        } {
            let _ = ORIG_PART_SETUP
                .set(unsafe { std::mem::transmute::<*const c_void, PartSetupFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                STR_EQ,
                &STR_EQ_PROLOGUE,
                detour_str_eq as *const c_void,
            )
        } {
            let _ = ORIG_STR_EQ.set(unsafe { std::mem::transmute::<*const c_void, StrEqFn>(tramp) });
            installed += 1;
        }
        if !is_console_port {
            return (installed, expected);
        }
        // Console-port guards (BF1943 + BFBC1): VO null-walk & D3D11 stale SRV
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                VO_WALK,
                &VO_WALK_PROLOGUE,
                detour_vo_walk as *const c_void,
            )
        } {
            let _ = ORIG_VO_WALK.set(unsafe { std::mem::transmute::<*const c_void, VoWalkFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                VO_WALK_LIST,
                &VO_WALK_LIST_PROLOGUE,
                detour_vo_walk_list as *const c_void,
            )
        } {
            let _ = ORIG_VO_WALK_LIST
                .set(unsafe { std::mem::transmute::<*const c_void, VoWalkFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                PS_BIND,
                &PS_BIND_PROLOGUE,
                ps_bind_entry as *const c_void,
            )
        } {
            unsafe { G_PS_BIND_BACK = tramp as usize };
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                VS_BIND,
                &VS_BIND_PROLOGUE,
                vs_bind_entry as *const c_void,
            )
        } {
            unsafe { G_VS_BIND_BACK = tramp as usize };
            installed += 1;
        }
        if !is_bfbc1 {
            return (installed, expected);
        }
        // BFBC1 (Frostbite 1.0 -> 1.5) compatibility guards
        if unsafe {
            crate::hooks::install_detour(
                REF_OBJ_BP,
                &REF_OBJ_BP_PROLOGUE,
                ref_obj_bp_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                REF_OBJ_BP2,
                &REF_OBJ_BP2_PROLOGUE,
                ref_obj_bp2_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                VO_WALK_CHILD,
                &VO_WALK_CHILD_PROLOGUE,
                vo_walk_child_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                VO_WALK_SUB,
                &VO_WALK_SUB_PROLOGUE,
                vo_walk_sub_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                REF_OBJ_FACT,
                &REF_OBJ_FACT_PROLOGUE,
                ref_obj_fact_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                LEVEL_LIST_WALK,
                &LEVEL_LIST_WALK_PROLOGUE,
                detour_level_list_walk as *const c_void,
            )
        } {
            let _ = ORIG_LEVEL_LIST_WALK
                .set(unsafe { std::mem::transmute::<*const c_void, LevelListWalkFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                GET_ARRAY_18,
                &GET_ARRAY_18_PROLOGUE,
                detour_get_array_18 as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                WORLD_ENTRY_GUARD,
                &WORLD_ENTRY_GUARD_PROLOGUE,
                world_entry_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                RES_PROXY_LOAD,
                &RES_PROXY_LOAD_PROLOGUE,
                detour_res_proxy_load as *const c_void,
            )
        } {
            let _ = ORIG_RES_PROXY_LOAD
                .set(unsafe { std::mem::transmute::<*const c_void, ResProxyLoadFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                MESH_MODEL_INIT,
                &MESH_MODEL_INIT_PROLOGUE,
                detour_mesh_model_init as *const c_void,
            )
        } {
            let _ = ORIG_MESH_MODEL_INIT
                .set(unsafe { std::mem::transmute::<*const c_void, LevelListWalkFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                BP_LOOKUP_GUARD,
                &BP_LOOKUP_GUARD_PROLOGUE,
                bp_lookup_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SMG_MESH_GUARD,
                &SMG_MESH_GUARD_PROLOGUE,
                smg_mesh_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SMG_BBOX_GUARD,
                &SMG_BBOX_GUARD_PROLOGUE,
                smg_bbox_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_REF_OBJ_FACT,
                &CLIENT_REF_OBJ_FACT_PROLOGUE,
                client_ref_obj_fact_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_SET_PART_GUARD,
                &PHYS_SET_PART_PROLOGUE,
                phys_set_part_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_CREATE_GUARD,
                &PHYS_CREATE_PROLOGUE,
                phys_create_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_USERDATA_GUARD,
                &PHYS_USERDATA_PROLOGUE,
                detour_phys_userdata as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SMG_CLIENT_GUARD,
                &SMG_CLIENT_PROLOGUE,
                smg_client_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_QUERY_GUARD,
                &PHYS_QUERY_PROLOGUE,
                phys_query_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                INGAME_COMP_INIT,
                &INGAME_COMP_INIT_PROLOGUE,
                detour_ingame_comp_init as *const c_void,
            )
        } {
            let _ = ORIG_INGAME_COMP_INIT.set(unsafe {
                std::mem::transmute::<
                    *const c_void,
                    unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void),
                >(tramp)
            });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                INGAME_COMP_INIT2,
                &INGAME_COMP_INIT2_PROLOGUE,
                detour_ingame_comp_init2 as *const c_void,
            )
        } {
            let _ = ORIG_INGAME_COMP_INIT2.set(unsafe {
                std::mem::transmute::<
                    *const c_void,
                    unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void),
                >(tramp)
            });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                INGAME_COMP_UPDATE,
                &INGAME_COMP_UPDATE_PROLOGUE,
                detour_ingame_comp_update as *const c_void,
            )
        } {
            let _ = ORIG_INGAME_COMP_UPDATE.set(unsafe {
                std::mem::transmute::<
                    *const c_void,
                    unsafe extern "fastcall" fn(*mut c_void, *mut c_void, *mut c_void),
                >(tramp)
            });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_GET_XFORM,
                &PHYS_GET_XFORM_PROLOGUE,
                detour_phys_get_xform as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                INGAME_COMP_QUERY,
                &INGAME_COMP_QUERY_PROLOGUE,
                ingame_comp_query_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_PARENT_CYCLE,
                &TRIE_PARENT_CYCLE_PROLOGUE,
                trie_parent_cycle_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                MESH_MODEL_RES_NULL,
                &MESH_MODEL_RES_NULL_PROLOGUE,
                mesh_model_res_null_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_TICK_GUARD,
                &CLIENT_TICK_PROLOGUE,
                client_tick_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SMG_BBOX2_GUARD,
                &SMG_BBOX2_PROLOGUE,
                smg_bbox2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_MESH_GUARD,
                &CLIENT_MESH_PROLOGUE,
                client_mesh_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                PHYS_PART_BOUNDS,
                &PHYS_PART_BOUNDS_PROLOGUE,
                detour_phys_part_bounds as *const c_void,
            )
        } {
            let _ = ORIG_PHYS_PART_BOUNDS.set(unsafe {
                std::mem::transmute::<
                    *const c_void,
                    unsafe extern "fastcall" fn(usize, *mut c_void, *mut [f32; 4], usize) -> *mut [f32; 4],
                >(tramp)
            });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SMG_RENDER_MEMBER_GUARD,
                &SMG_RENDER_MEMBER_PROLOGUE,
                smg_render_member_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                VO_WALK_ARR_GUARD,
                &VO_WALK_ARR_PROLOGUE,
                vo_walk_arr_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_ENT_INIT_GUARD,
                &PHYS_ENT_INIT_PROLOGUE,
                phys_ent_init_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_ENT_INIT2_GUARD,
                &PHYS_ENT_INIT2_PROLOGUE,
                phys_ent_init2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_PARENT_GUARD,
                &ENT_PARENT_PROLOGUE,
                ent_parent_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_WALK_GUARD,
                &ENT_WALK_PROLOGUE,
                ent_walk_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_OWNER_GUARD,
                &ENT_OWNER_PROLOGUE,
                ent_owner_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_GET_FLOAT90_GUARD,
                &ENT_GET_FLOAT90_PROLOGUE,
                ent_get_float90_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_GET_U32_4C_GUARD,
                &ENT_GET_U32_4C_PROLOGUE,
                ent_get_u32_4c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SMG_OCCLUDER_GUARD,
                &SMG_OCCLUDER_PROLOGUE,
                smg_occluder_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SMG_RENDER_MEMBER2_GUARD,
                &SMG_RENDER_MEMBER2_PROLOGUE,
                smg_render_member2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                DESTR_MESH_GUARD1,
                &DESTR_MESH_GUARD1_PROLOGUE,
                destr_mesh_guard1_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                DESTR_MESH_GUARD2,
                &DESTR_MESH_GUARD2_PROLOGUE,
                destr_mesh_guard2_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                DESTR_RADIUS_GUARD1,
                &DESTR_RADIUS_PROLOGUE,
                destr_radius_guard1_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                DESTR_RADIUS_GUARD2,
                &DESTR_RADIUS_PROLOGUE,
                destr_radius_guard2_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                DESTR_RADIUS_GUARD3,
                &DESTR_RADIUS_GUARD3_PROLOGUE,
                destr_radius_guard3_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                DESTR_BBOX_GUARD1,
                &DESTR_BBOX_PROLOGUE,
                detour_destr_bbox1 as *const c_void,
            )
        } {
            let _ = ORIG_DESTR_BBOX1
                .set(unsafe { std::mem::transmute::<*const c_void, DestrBboxFn>(tramp) });
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                DESTR_BBOX_GUARD2,
                &DESTR_BBOX_PROLOGUE,
                detour_destr_bbox2 as *const c_void,
            )
        } {
            let _ = ORIG_DESTR_BBOX2
                .set(unsafe { std::mem::transmute::<*const c_void, DestrBboxFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SOUND_BANK_ITEM_GUARD,
                &SOUND_BANK_ITEM_PROLOGUE,
                sound_bank_item_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SOUND_BANK_SELECT_GUARD,
                &SOUND_BANK_SELECT_PROLOGUE,
                sound_bank_select_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                AUDIO_MIX_ITEM_GUARD,
                &AUDIO_MIX_ITEM_PROLOGUE,
                audio_mix_item_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                SOUND_NODE_UPDATE,
                &SOUND_NODE_UPDATE_PROLOGUE,
                detour_sound_node_update as *const c_void,
            )
        } {
            let _ = ORIG_SOUND_NODE_UPDATE
                .set(unsafe { std::mem::transmute::<*const c_void, SoundNodeUpdateFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                RENDER_PARAM_GUARD,
                &RENDER_PARAM_PROLOGUE,
                render_param_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SHADER_DB_LOOKUP_GUARD,
                &SHADER_DB_LOOKUP_PROLOGUE,
                shader_db_lookup_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POSTFX_TEX_SIZE_GUARD,
                &POSTFX_TEX_SIZE_PROLOGUE,
                postfx_tex_size_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                MESH_LOD_SELECT_GUARD,
                &MESH_LOD_SELECT_PROLOGUE,
                mesh_lod_select_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_GET_VEL_GUARD,
                &PHYS_GET_VEL_PROLOGUE,
                phys_get_vel_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SPAWN_ENT_NULL_GUARD,
                &SPAWN_ENT_NULL_PROLOGUE,
                spawn_ent_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_FLAGS_NULL_GUARD,
                &ENT_FLAGS_NULL_PROLOGUE,
                ent_flags_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_FLAGS2_NULL_GUARD,
                &ENT_FLAGS2_NULL_PROLOGUE,
                ent_flags2_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_DATA_NULL_GUARD,
                &ENT_DATA_NULL_PROLOGUE,
                ent_data_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP2C_NULL_GUARD,
                &ENT_COMP2C_NULL_PROLOGUE,
                ent_comp2c_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP3C_NULL_GUARD,
                &ENT_COMP3C_NULL_PROLOGUE,
                ent_comp3c_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                TRIE_INSERT_LOCK,
                &TRIE_INSERT_LOCK_PROLOGUE,
                detour_trie_insert_lock as *const c_void,
            )
        } {
            let _ = ORIG_TRIE_INSERT_LOCK
                .set(unsafe { std::mem::transmute::<*const c_void, TrieInsertFn>(tramp) });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_POP1_GUARD,
                &POOL_POP1_PROLOGUE,
                pool_pop1_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_POP2_GUARD,
                &POOL_POP2_PROLOGUE,
                pool_pop2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_COUNT_GUARD,
                &POOL_COUNT_PROLOGUE,
                pool_count_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                POOL_COLLECT_GUARD,
                &POOL_COLLECT_PROLOGUE,
                pool_collect_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_SLOT_NULL_GUARD,
                &PHYS_SLOT_NULL_PROLOGUE,
                phys_slot_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_UPD_DATA_GUARD,
                &ENT_UPD_DATA_PROLOGUE,
                ent_upd_data_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_BBOX_MERGE_GUARD,
                &ENT_BBOX_MERGE_PROLOGUE,
                ent_bbox_merge_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                RENDER_BUCKET_GUARD,
                &RENDER_BUCKET_PROLOGUE,
                render_bucket_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_ENT_F10_GUARD,
                &CLIENT_ENT_F10_PROLOGUE,
                client_ent_f10_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                COMP_ATTACH_NULL_GUARD,
                &COMP_ATTACH_NULL_PROLOGUE,
                comp_attach_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_TICK2C_NULL_GUARD,
                &ENT_TICK2C_NULL_PROLOGUE,
                ent_tick2c_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_IMPULSE_GUARD,
                &CLIENT_IMPULSE_PROLOGUE,
                client_impulse_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_ENT_F10B_GUARD,
                &CLIENT_ENT_F10B_PROLOGUE,
                client_ent_f10b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP08_GUARD,
                &ENT_COMP08_PROLOGUE,
                ent_comp08_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                DESTR_RADIUS_GUARD4,
                &DESTR_RADIUS_GUARD4_PROLOGUE,
                destr_radius_guard4_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_SLOT_ESI_GUARD,
                &PHYS_SLOT_ESI_PROLOGUE,
                phys_slot_esi_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                MESH_SET_LOD_GUARD,
                &MESH_SET_LOD_PROLOGUE,
                mesh_set_lod_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP08_B_GUARD,
                &ENT_COMP08_B_PROLOGUE,
                ent_comp08_b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_PHYS_CB_GUARD,
                &CLIENT_PHYS_CB_PROLOGUE,
                client_phys_cb_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP08_C_GUARD,
                &ENT_COMP08_C_PROLOGUE,
                ent_comp08_c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_ENT3C_GUARD,
                &CLIENT_ENT3C_PROLOGUE,
                client_ent3c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_ENT04_GUARD,
                &CLIENT_ENT04_PROLOGUE,
                client_ent04_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                CLIENT_ENT3C_B_GUARD,
                &CLIENT_ENT3C_B_PROLOGUE,
                client_ent3c_b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                PHYS_BODY_A0_GUARD,
                &PHYS_BODY_A0_PROLOGUE,
                phys_body_a0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP3C_B_GUARD,
                &ENT_COMP3C_B_PROLOGUE,
                ent_comp3c_b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP3C_C_GUARD,
                &ENT_COMP3C_C_PROLOGUE,
                ent_comp3c_c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_UNLINK_GUARD,
                &TRIE_UNLINK_PROLOGUE,
                trie_unlink_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                DBX_NULL_ATTR_GUARD,
                &DBX_NULL_ATTR_PROLOGUE,
                dbx_null_attr_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_BP_ENTRY_GUARD,
                &SP_BP_ENTRY_PROLOGUE,
                sp_bp_entry_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_BP_ESI_GUARD,
                &SP_BP_ESI_PROLOGUE,
                sp_bp_esi_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR_GUARD,
                &SP_AI_MGR_PROLOGUE,
                sp_ai_mgr_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR2_GUARD,
                &SP_AI_MGR2_PROLOGUE,
                sp_ai_mgr2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR3_GUARD,
                &SP_AI_MGR3_PROLOGUE,
                sp_ai_mgr3_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR4_GUARD,
                &SP_AI_MGR4_PROLOGUE,
                sp_ai_mgr4_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR5_GUARD,
                &SP_AI_MGR5_PROLOGUE,
                sp_ai_mgr5_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_LIST2_NULL_GUARD,
                &SP_LIST2_NULL_PROLOGUE,
                sp_list2_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ANIM_RES_GUARD,
                &SP_ANIM_RES_PROLOGUE,
                sp_anim_res_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ANIM_CTOR_GUARD,
                &SP_ANIM_CTOR_PROLOGUE,
                sp_anim_ctor_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_LIST3_NULL_GUARD,
                &SP_LIST3_NULL_PROLOGUE,
                sp_list3_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ANIM_DATA_GUARD,
                &SP_ANIM_DATA_PROLOGUE,
                sp_anim_data_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ANIM_INIT1_GUARD,
                &SP_ANIM_INIT1_PROLOGUE,
                sp_anim_init1_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ANIM_INIT2_GUARD,
                &SP_ANIM_INIT2_PROLOGUE,
                sp_anim_init2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR6_GUARD,
                &SP_AI_MGR6_PROLOGUE,
                sp_ai_mgr6_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_F8C_GUARD,
                &SP_PHYS_F8C_PROLOGUE,
                sp_phys_f8c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TERR_B14_GUARD,
                &SP_TERR_B14_PROLOGUE,
                sp_terr_b14_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_ADD_GUARD,
                &SP_PHYS_ADD_PROLOGUE,
                sp_phys_add_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_FIND_GUARD,
                &SP_TRIE_FIND_PROLOGUE,
                sp_trie_find_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT44_FLAG_GUARD,
                &SP_ENT44_FLAG_PROLOGUE,
                sp_ent44_flag_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_9C_GUARD,
                &SP_PHYS_9C_PROLOGUE,
                sp_phys_9c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR7_GUARD,
                &SP_AI_MGR7_PROLOGUE,
                sp_ai_mgr7_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_INS_GUARD,
                &SP_TRIE_INS_PROLOGUE,
                sp_trie_ins_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT44_C0_GUARD,
                &SP_ENT44_C0_PROLOGUE,
                sp_ent44_c0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_80_GUARD,
                &SP_PHYS_80_PROLOGUE,
                sp_phys_80_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR8_GUARD,
                &SP_AI_MGR8_PROLOGUE,
                sp_ai_mgr8_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT44_FLAG2_GUARD,
                &SP_ENT44_FLAG2_PROLOGUE,
                sp_ent44_flag2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_4C_GUARD,
                &SP_PHYS_4C_PROLOGUE,
                sp_phys_4c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR9_GUARD,
                &SP_AI_MGR9_PROLOGUE,
                sp_ai_mgr9_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR10_GUARD,
                &SP_AI_MGR10_PROLOGUE,
                sp_ai_mgr10_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR11_GUARD,
                &SP_AI_MGR11_PROLOGUE,
                sp_ai_mgr11_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_10_GUARD,
                &SP_PHYS_10_PROLOGUE,
                sp_phys_10_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT44_C0_B_GUARD,
                &SP_ENT44_C0_B_PROLOGUE,
                sp_ent44_c0_b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_9C_B_GUARD,
                &SP_PHYS_9C_B_PROLOGUE,
                sp_phys_9c_b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR12_GUARD,
                &SP_AI_MGR12_PROLOGUE,
                sp_ai_mgr12_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_VEC_LAST_GUARD,
                &SP_VEC_LAST_PROLOGUE,
                sp_vec_last_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT08_E0_GUARD,
                &SP_ENT08_E0_PROLOGUE,
                sp_ent08_e0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_44_GUARD,
                &SP_PHYS_44_PROLOGUE,
                sp_phys_44_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR13_GUARD,
                &SP_AI_MGR13_PROLOGUE,
                sp_ai_mgr13_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_C290_GUARD,
                &SP_PHYS_C290_PROLOGUE,
                sp_phys_c290_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT08_C4_GUARD,
                &SP_ENT08_C4_PROLOGUE,
                sp_ent08_c4_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT80_68_GUARD,
                &SP_ENT80_68_PROLOGUE,
                sp_ent80_68_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR14_GUARD,
                &SP_AI_MGR14_PROLOGUE,
                sp_ai_mgr14_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR15_GUARD,
                &SP_AI_MGR15_PROLOGUE,
                sp_ai_mgr15_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_F80_GUARD,
                &SP_PHYS_F80_PROLOGUE,
                sp_phys_f80_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR16_GUARD,
                &SP_AI_MGR16_PROLOGUE,
                sp_ai_mgr16_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR17_GUARD,
                &SP_AI_MGR17_PROLOGUE,
                sp_ai_mgr17_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TYPE_IDX_GUARD,
                &SP_TYPE_IDX_PROLOGUE,
                sp_type_idx_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_C250_GUARD,
                &SP_PHYS_C250_PROLOGUE,
                sp_phys_c250_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MGR18_GUARD,
                &SP_AI_MGR18_PROLOGUE,
                sp_ai_mgr18_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_9C_C_GUARD,
                &SP_PHYS_9C_C_PROLOGUE,
                sp_phys_9c_c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_44_B_GUARD,
                &SP_PHYS_44_B_PROLOGUE,
                sp_phys_44_b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_CB24_GUARD,
                &SP_PHYS_CB24_PROLOGUE,
                sp_phys_cb24_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ARR10_GUARD,
                &SP_ARR10_PROLOGUE,
                sp_arr10_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_VEH_SLOT_GUARD,
                &SP_VEH_SLOT_PROLOGUE,
                sp_veh_slot_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_A0_GUARD,
                &SP_PHYS_A0_PROLOGUE,
                sp_phys_a0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_VEH_SLOT2_GUARD,
                &SP_VEH_SLOT2_PROLOGUE,
                sp_veh_slot2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_OBJ20_GUARD,
                &SP_AI_OBJ20_PROLOGUE,
                sp_ai_obj20_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_ESI14_GUARD,
                &SP_AI_ESI14_PROLOGUE,
                sp_ai_esi14_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ENT_EDI3C_GUARD,
                &SP_ENT_EDI3C_PROLOGUE,
                sp_ent_edi3c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_MATCPY_GUARD,
                &SP_AI_MATCPY_PROLOGUE,
                sp_ai_matcpy_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_GETAC_GUARD,
                &SP_AI_GETAC_PROLOGUE,
                sp_ai_getac_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_ARR1C_GUARD,
                &SP_AI_ARR1C_PROLOGUE,
                sp_ai_arr1c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_NAV_GUARD,
                &SP_AI_NAV_PROLOGUE,
                sp_ai_nav_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_SUB14_GUARD,
                &SP_AI_SUB14_PROLOGUE,
                sp_ai_sub14_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                SP_NODE_WALK_GUARD,
                &SP_NODE_WALK_PROLOGUE,
                detour_node_walk as *const c_void,
            )
        } {
            let _ = ORIG_NODE_WALK.set(unsafe {
                std::mem::transmute::<
                    *const c_void,
                    unsafe extern "fastcall" fn(usize, *mut c_void, usize, usize) -> usize,
                >(tramp)
            });
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_44_C_GUARD,
                &SP_PHYS_44_C_PROLOGUE,
                sp_phys_44_c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_BD80_GUARD,
                &SP_PHYS_BD80_PROLOGUE,
                sp_phys_bd80_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_EBX24_GUARD,
                &SP_AI_EBX24_PROLOGUE,
                sp_ai_ebx24_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_PATHDATA_GUARD,
                &SP_AI_PATHDATA_PROLOGUE,
                sp_ai_pathdata_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_BD0A_GUARD,
                &SP_PHYS_BD0A_PROLOGUE,
                sp_phys_bd0a_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_EB86_GUARD,
                &SP_AI_EB86_PROLOGUE,
                sp_ai_eb86_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_190_GUARD,
                &SP_PHYS_190_PROLOGUE,
                sp_phys_190_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_PHYS_C970_GUARD,
                &SP_PHYS_C970_PROLOGUE,
                sp_phys_c970_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AI_EBD2_GUARD,
                &SP_AI_EBD2_PROLOGUE,
                sp_ai_ebd2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_FDA3_GUARD,
                &SP_FDA3_PROLOGUE,
                sp_fda3_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_UNLINK2_GUARD,
                &TRIE_UNLINK2_PROLOGUE,
                trie_unlink2_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ALLOC_GUARD,
                &SP_ALLOC_PROLOGUE,
                sp_alloc_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_FIND_PTR_GUARD,
                &TRIE_FIND_PTR_PROLOGUE,
                trie_find_ptr_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                TRIE_LEAF_GUARD,
                &TRIE_LEAF_PROLOGUE,
                trie_leaf_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_LIST_4968_GUARD,
                &SP_LIST_4968_PROLOGUE,
                sp_list_4968_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4495E0_GUARD,
                &SP_4495E0_PROLOGUE,
                sp_4495e0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_9C4055_GUARD,
                &SP_9C4055_PROLOGUE,
                sp_9c4055_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4493A3_GUARD,
                &SP_4493A3_PROLOGUE,
                sp_4493a3_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_POOL_4CF9_GUARD,
                &SP_POOL_4CF9_PROLOGUE,
                sp_pool_4cf9_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8950_GUARD,
                &SP_TRIE_4C8950_PROLOGUE,
                sp_trie_4c8950_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C89B0_GUARD,
                &SP_TRIE_4C89B0_PROLOGUE,
                sp_trie_4c89b0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8A10_GUARD,
                &SP_TRIE_4C8A10_PROLOGUE,
                sp_trie_4c8a10_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8A70_GUARD,
                &SP_TRIE_4C8A70_PROLOGUE,
                sp_trie_4c8a70_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8AD0_GUARD,
                &SP_TRIE_4C8AD0_PROLOGUE,
                sp_trie_4c8ad0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_TRIE_4C8C80_GUARD,
                &SP_TRIE_4C8C80_PROLOGUE,
                sp_trie_4c8c80_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4CF9A6_GUARD,
                &SP_4CF9A6_PROLOGUE,
                sp_4cf9a6_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_LIST_NULL_GUARD,
                &SP_LIST_NULL_PROLOGUE,
                sp_list_null_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_LEVEL_WALK_GUARD,
                &SP_LEVEL_WALK_PROLOGUE,
                sp_level_walk_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_BBOX_MESH_GUARD,
                &ENT_BBOX_MESH_PROLOGUE,
                ent_bbox_mesh_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP14_GUARD,
                &ENT_COMP14_PROLOGUE,
                ent_comp14_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                ENT_COMP14_B_GUARD,
                &ENT_COMP14_B_PROLOGUE,
                ent_comp14_b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
    }
    (installed, expected)
}

#[cfg(target_arch = "x86")]
unsafe fn view_looks_live(view: usize) -> bool {
    if view < 0x10000 || (view & 3) != 0 {
        return false;
    }
    if unsafe { IsBadReadPtr(view as *const c_void, 4) } != 0 {
        return false;
    }
    let vt = unsafe { *(view as *const usize) };
    if vt < 0x10000 || (vt & 3) != 0 {
        return false;
    }
    let lo = D3D_LO.load(Ordering::Relaxed);
    let hi = D3D_HI.load(Ordering::Relaxed);
    if lo != 0 && vt >= lo && vt < hi {
        return true;
    }
    let mut m: *mut c_void = std::ptr::null_mut();
    // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS (4) | UNCHANGED_REFCOUNT (2)
    if unsafe { GetModuleHandleExA(6, vt as *const c_void, &mut m) } == 0 || m.is_null() {
        return false;
    }
    let mut path = [0u8; 260];
    let n = unsafe { GetModuleFileNameA(m, path.as_mut_ptr(), path.len() as u32) } as usize;
    if n == 0 {
        return false;
    }
    let s = String::from_utf8_lossy(&path[..n]);
    let base = s.rsplit('\\').next().unwrap_or(&s).to_ascii_lowercase();
    if !base.starts_with("d3d1") && base != "renderdoc.dll" {
        return false;
    }
    let m_base = m as usize;
    let lfanew = unsafe { *((m_base + 0x3C) as *const u32) } as usize;
    let size_of_image = unsafe { *((m_base + lfanew + 0x50) as *const u32) } as usize;
    if lo == 0 && size_of_image > 0 {
        D3D_LO.store(m_base, Ordering::Relaxed);
        D3D_HI.store(m_base + size_of_image, Ordering::Relaxed);
        crate::log_line(&format!(
            "levelfix: D3D SRV guard locked to {base} (0x{m_base:08X}..0x{:08X})",
            m_base + size_of_image
        ));
    }
    true
}

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
unsafe extern "cdecl" fn opfor_check_views(cache: usize, views_off: u32, first_off: u32, last_off: u32) {
    if cache < 0x10000 {
        return;
    }
    let a = unsafe { *((cache + first_off as usize) as *const u32) };
    let b = unsafe { *((cache + last_off as usize) as *const u32) };
    if a > b || b >= 128 {
        return;
    }
    let slots = (cache + views_off as usize) as *mut usize;
    for i in a..=b {
        let p = unsafe { *slots.add(i as usize) };
        if p != 0 && !unsafe { view_looks_live(p) } {
            unsafe { *slots.add(i as usize) = 0 };
        }
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ps_bind_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0x21C",
        "push 0x214",
        "push 0x144",
        "push ebx",
        "call {check}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{back}]",
        check = sym opfor_check_views,
        back = sym G_PS_BIND_BACK,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn vs_bind_entry() {
    std::arch::naked_asm!(
        "pushad",
        "pushfd",
        "push 0x218",
        "push 0x210",
        "push 0xC4",
        "push ebx",
        "call {check}",
        "add esp, 16",
        "popfd",
        "popad",
        "jmp dword ptr [{back}]",
        check = sym opfor_check_views,
        back = sym G_VS_BIND_BACK,
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_destr_bbox1(
    this: usize,
    edx: *mut c_void,
    out: *mut [f32; 8],
) -> *mut [f32; 8] {
    if this < 0x10000 || unsafe { *((this + 0x50) as *const usize) } < 0x10000 {
        if !out.is_null() {
            unsafe { *out = [0.0; 8] };
        }
        return out;
    }
    if let Some(&orig) = ORIG_DESTR_BBOX1.get() {
        return unsafe { orig(this, edx, out) };
    }
    out
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_destr_bbox2(
    this: usize,
    edx: *mut c_void,
    out: *mut [f32; 8],
) -> *mut [f32; 8] {
    if this < 0x10000 || unsafe { *((this + 0x50) as *const usize) } < 0x10000 {
        if !out.is_null() {
            unsafe { *out = [0.0; 8] };
        }
        return out;
    }
    if let Some(&orig) = ORIG_DESTR_BBOX2.get() {
        return unsafe { orig(this, edx, out) };
    }
    out
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn destr_radius_guard1_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 0x68]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x78]",
        "test ecx, ecx",
        "jz 2f",
        "cmp dword ptr [ecx + 0x30], 0",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "ret",
        cont = const DESTR_RADIUS_CONT1,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn destr_radius_guard2_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 0x68]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x78]",
        "test ecx, ecx",
        "jz 2f",
        "cmp dword ptr [ecx + 0x30], 0",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "ret",
        cont = const DESTR_RADIUS_CONT2,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn destr_radius_guard3_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 0xf4]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx + 0x58]",
        "test edx, edx",
        "jz 2f",
        "fld dword ptr [edx + 0x20]",
        "ret",
        "2:",
        "fldz",
        "ret",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sound_bank_item_guard_entry() {
    std::arch::naked_asm!(
        "mov ebp, dword ptr [eax + 0x14]",
        "test ebp, ebp",
        "jz 2f",
        "mov eax, dword ptr [ebp + 0xc]",
        "push {cont}",
        "ret",
        "2:",
        "mov edx, dword ptr [esp + 0x14]",
        "mov ecx, dword ptr [esi]",
        "xor eax, eax",
        "mov dword ptr [edx + ecx], eax",
        "mov dword ptr [edx + ecx + 4], eax",
        "push {skip}",
        "ret",
        cont = const SOUND_BANK_ITEM_CONT,
        skip = const SOUND_BANK_ITEM_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sound_bank_select_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [eax + ecx]",
        "test edx, edx",
        "jz 2f",
        "mov ecx, dword ptr [0x019a7cd8]",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop esi",
        "pop ebp",
        "pop ebx",
        "pop ecx",
        "ret",
        cont = const SOUND_BANK_SELECT_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn audio_mix_item_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 0x1c]",
        "mov eax, dword ptr [eax]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x60], 0",
        "jz 2f",
        "mov eax, dword ptr [esp + 0x1c]",
        "fld qword ptr [0x014d9640]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const AUDIO_MIX_ITEM_CONT,
        skip = const AUDIO_MIX_ITEM_SKIP,
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_sound_node_update(
    this: usize,
    edx: *mut c_void,
    arg0: *const c_void,
) -> u8 {
    if (arg0 as usize) < 0x10000 {
        return 1;
    }
    if let Some(&orig) = ORIG_SOUND_NODE_UPDATE.get() {
        return unsafe { orig(this, edx, arg0) };
    }
    1
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn render_param_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 0x18]",
        "xorps xmm1, xmm1",
        "test eax, eax",
        "jz 2f",
        "movups xmm1, xmmword ptr [eax + 0x10]",
        "2:",
        "xorps xmm0, xmm0",
        "test edx, edx",
        "jz 3f",
        "movups xmm0, xmmword ptr [edx + 8]",
        "3:",
        "push {cont}",
        "ret",
        cont = const RENDER_PARAM_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn shader_db_lookup_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "mov ecx, dword ptr [esi + 0xc]",
        "or edx, eax",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SHADER_DB_LOOKUP_CONT,
        skip = const SHADER_DB_LOOKUP_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn postfx_tex_size_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "mov eax, dword ptr [edx + 0x20]",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esp + 0x10], 0",
        "fldz",
        "lea ecx, [esp + 0x4f0]",
        "push {skip}",
        "ret",
        cont = const POSTFX_TEX_SIZE_CONT,
        skip = const POSTFX_TEX_SIZE_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn mesh_lod_select_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 4]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x58], 0",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x40]",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, 0xff",
        "ret",
        cont = const MESH_LOD_SELECT_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_get_vel_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x8c]",
        "test eax, eax",
        "jz 2f",
        "add eax, 0x150",
        "ret",
        "2:",
        "lea eax, [{ident}]",
        "ret",
        ident = sym IDENTITY_MAT4,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn spawn_ent_null_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "mov edx, dword ptr [esi]",
        "mov eax, dword ptr [edx + 8]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SPAWN_ENT_NULL_CONT,
        skip = const SPAWN_ENT_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_flags_null_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ebp + 0x124]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_FLAGS_NULL_CONT,
        skip = const ENT_FLAGS_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_flags2_null_guard_entry() {
    std::arch::naked_asm!(
        "xor ebx, ebx",
        "test eax, eax",
        "jz 2f",
        "mov ebx, dword ptr [eax + 0x10]",
        "2:",
        "mov ecx, dword ptr [edi + 4]",
        "push {cont}",
        "ret",
        cont = const ENT_FLAGS2_NULL_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_data_null_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [esi + 4]",
        "test edx, edx",
        "jz 2f",
        "mov eax, dword ptr [edx + 0xc]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_DATA_NULL_CONT,
        skip = const ENT_DATA_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp2c_null_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov ecx, dword ptr [ecx + 0x2c]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, 1",
        "ret",
        cont = const ENT_COMP2C_NULL_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp3c_null_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x3c]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_COMP3C_NULL_CONT,
        skip = const ENT_COMP3C_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
static TRIE_LOCK_OWNER: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_arch = "x86")]
static TRIE_LOCK_DEPTH: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_arch = "x86")]
#[inline]
fn trie_lock_acquire() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    let tid = unsafe { GetCurrentThreadId() } as usize;
    if TRIE_LOCK_OWNER.load(Ordering::Acquire) != tid {
        while TRIE_LOCK_OWNER
            .compare_exchange_weak(0, tid, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
    }
    TRIE_LOCK_DEPTH.fetch_add(1, Ordering::Relaxed);
}

#[cfg(target_arch = "x86")]
#[inline]
fn trie_lock_release() {
    if TRIE_LOCK_DEPTH.fetch_sub(1, Ordering::Relaxed) == 1 {
        TRIE_LOCK_OWNER.store(0, Ordering::Release);
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_trie_insert_lock(
    this: usize,
    edx: *mut c_void,
    out: *mut c_void,
    root: *mut c_void,
    name: *const u8,
) -> *mut c_void {
    trie_lock_acquire();
    let ret = if let Some(&orig) = ORIG_TRIE_INSERT_LOCK.get() {
        unsafe { orig(this, edx, out, root, name) }
    } else {
        out
    };
    trie_lock_release();
    ret
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_pop1_guard_entry() {
    std::arch::naked_asm!(
        "pushfd",
        "mov edi, dword ptr [esi + 0x20]",
        "cmp edi, 0x02000000",
        "jb 4f",
        "cmp edi, 0x7fff0000",
        "ja 4f",
        "test edi, 3",
        "jnz 4f",
        "cmp edi, 0x60000000",
        "jb 5f",
        "push eax",
        "push ecx",
        "push edx",
        "push edi",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 4f",
        "5:",
        "mov eax, dword ptr [edi]",
        "test eax, eax",
        "jz 2f",
        "cmp eax, 0x02000000",
        "jb 3f",
        "cmp eax, 0x7fff0000",
        "ja 3f",
        "test eax, 3",
        "jnz 3f",
        "cmp eax, 0x60000000",
        "jb 2f",
        "push eax",
        "push ecx",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 3f",
        "2:",
        "popfd",
        "push {cont}",
        "ret",
        "3:",
        "xor eax, eax",
        "popfd",
        "push {cont}",
        "ret",
        "4:",
        "mov dword ptr [esi + 0x20], 0",
        "popfd",
        "push {refill}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const POOL_POP1_CONT,
        refill = const POOL_POP1_REFILL,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_pop2_guard_entry() {
    std::arch::naked_asm!(
        "pushfd",
        "mov eax, dword ptr [esi + 0x20]",
        "cmp eax, 0x02000000",
        "jb 4f",
        "cmp eax, 0x7fff0000",
        "ja 4f",
        "test eax, 3",
        "jnz 4f",
        "cmp eax, 0x60000000",
        "jb 6f",
        "push eax",
        "push ecx",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 4f",
        "6:",
        "mov ecx, dword ptr [eax]",
        "test ecx, ecx",
        "jz 2f",
        "cmp ecx, 0x02000000",
        "jb 3f",
        "cmp ecx, 0x7fff0000",
        "ja 3f",
        "test ecx, 3",
        "jnz 3f",
        "cmp ecx, 0x60000000",
        "jb 7f",
        "push eax",
        "push ecx",
        "push edx",
        "push ecx",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 3f",
        "7:",
        "push edx",
        "mov edx, ecx",
        "xor edx, eax",
        "cmp edx, 0x00100000",
        "jbe 5f",
        "mov edx, ecx",
        "shr edx, 8",
        "and edx, 0xffff",
        "cmp edx, 0x0101",
        "pop edx",
        "je 3f",
        "jmp 2f",
        "5:",
        "pop edx",
        "2:",
        "popfd",
        "push {cont}",
        "ret",
        "3:",
        "xor ecx, ecx",
        "popfd",
        "push {cont}",
        "ret",
        "4:",
        "mov dword ptr [esi + 0x20], 0",
        "popfd",
        "push {refill}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const POOL_POP2_CONT,
        refill = const POOL_POP2_REFILL,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
unsafe extern "cdecl" fn opfor_is_bad_ptr(p: usize) -> u32 {
    if unsafe { IsBadReadPtr(p as *const c_void, 4) } != 0 {
        1
    } else {
        0
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_count_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x02000000",
        "jb 2f",
        "cmp eax, 0x7fff0000",
        "ja 2f",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x656a626f",
        "je 2f",
        "cmp eax, 0x61746164",
        "je 2f",
        "cmp eax, 0x60000000",
        "jae 4f",
        "push edx",
        "mov edx, eax",
        "shr edx, 8",
        "and edx, 0xffff",
        "cmp edx, 0x0101",
        "pop edx",
        "jne 3f",
        "4:",
        "push ecx",
        "push edx",
        "push eax",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop eax",
        "pop edx",
        "pop ecx",
        "jnz 2f",
        "3:",
        "mov eax, dword ptr [eax]",
        "add ebp, 1",
        "push {cont}",
        "ret",
        "2:",
        "push {done}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const POOL_COUNT_CONT,
        done = const POOL_COUNT_DONE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn pool_collect_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x02000000",
        "jb 2f",
        "cmp eax, 0x7fff0000",
        "ja 2f",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x656a626f",
        "je 2f",
        "cmp eax, 0x61746164",
        "je 2f",
        "cmp eax, 0x60000000",
        "jae 4f",
        "push edx",
        "mov edx, eax",
        "shr edx, 8",
        "and edx, 0xffff",
        "cmp edx, 0x0101",
        "pop edx",
        "jne 3f",
        "4:",
        "push ecx",
        "push edx",
        "push eax",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop eax",
        "pop edx",
        "pop ecx",
        "jnz 2f",
        "3:",
        "mov dword ptr [ecx], eax",
        "mov eax, dword ptr [eax]",
        "add ecx, 4",
        "push {cont}",
        "ret",
        "2:",
        "push {done}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const POOL_COLLECT_CONT,
        done = const POOL_COLLECT_DONE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_slot_null_guard_entry() {
    std::arch::naked_asm!(
        "mov dword ptr [esp + 0x10], eax",
        "cmp dword ptr [esp + 0x28], 0xff",
        "je 2f",
        "cmp eax, 0x02000000",
        "jb 2f",
        "cmp eax, 0x7fff0000",
        "ja 2f",
        "mov eax, dword ptr [eax + 0xc]",
        "push {cont}",
        "ret",
        "2:",
        "fstp dword ptr [esp + 0x18]",
        "mov byte ptr [esp + 0x1f], 0",
        "xor esi, esi",
        "push {skip}",
        "ret",
        cont = const PHYS_SLOT_NULL_CONT,
        skip = const PHYS_SLOT_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_upd_data_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 8]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x40], 0",
        "jz 2f",
        "push ecx",
        "push esi",
        "mov esi, ecx",
        "cmp dword ptr [esi + 0x7c], 0",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const ENT_UPD_DATA_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_bbox_merge_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [eax]",
        "test eax, eax",
        "jz 2f",
        "movups xmm1, xmmword ptr [esi]",
        "mov eax, dword ptr [eax + 0x44]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_BBOX_MERGE_CONT,
        skip = const ENT_BBOX_MERGE_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn render_bucket_guard_entry() {
    std::arch::naked_asm!(
        "xor esi, esi",
        "mov dword ptr [esp + 0x1c], esi",
        "cmp edx, 0x10000",
        "jb 2f",
        "cmp dword ptr [edx + 0x10], 0",
        "jz 2f",
        "cmp dword ptr [edx], esi",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const RENDER_BUCKET_CONT,
        skip = const RENDER_BUCKET_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_ent_f10_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [esi + 0x10]",
        "test edx, edx",
        "jz 2f",
        "mov ecx, dword ptr [edx + 0x40]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_ENT_F10_CONT,
        skip = const CLIENT_ENT_F10_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn comp_attach_null_guard_entry() {
    std::arch::naked_asm!(
        "cmp dword ptr [esp + 4], 0",
        "jz 2f",
        "push ebx",
        "push esi",
        "mov esi, ecx",
        "mov ecx, dword ptr [esp + 0xc]",
        "push {cont}",
        "ret",
        "2:",
        "ret 0x10",
        cont = const COMP_ATTACH_NULL_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_tick2c_null_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "cmp dword ptr [ecx + 0x2c], 0",
        "jz 2f",
        "push ebx",
        "push esi",
        "mov esi, ecx",
        "mov ecx, dword ptr [esi + 0x2c]",
        "push {cont}",
        "ret",
        "2:",
        "mov esp, ebp",
        "pop ebp",
        "ret",
        cont = const ENT_TICK2C_NULL_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_impulse_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "mov edx, dword ptr [esi]",
        "mov eax, dword ptr [edx + 0x84]",
        "push {cont}",
        "ret",
        "2:",
        "fstp st(0)",
        "push {skip}",
        "ret",
        cont = const CLIENT_IMPULSE_CONT,
        skip = const CLIENT_IMPULSE_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_ent_f10b_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esi + 0x10]",
        "fstp dword ptr [esi + 0x670]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_ENT_F10B_CONT,
        skip = const CLIENT_ENT_F10B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp08_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 8]",
        "fstp dword ptr [esp + 4]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_COMP08_CONT,
        skip = const ENT_COMP08_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn destr_radius_guard4_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 0x1cc]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx + 0x58]",
        "test edx, edx",
        "jz 2f",
        "fld dword ptr [edx + 0x20]",
        "ret",
        "2:",
        "fldz",
        "ret",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_slot_esi_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "test eax, eax",
        "mov dword ptr [esi + 0x90], eax",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const PHYS_SLOT_ESI_CONT,
        skip = const PHYS_SLOT_ESI_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn mesh_set_lod_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "test eax, eax",
        "jz 2f",
        "mov edx, dword ptr [eax + 0x44]",
        "test edx, edx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "ret 4",
        cont = const MESH_SET_LOD_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp08_b_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 8]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esp + 8], 0",
        "push {skip}",
        "ret",
        cont = const ENT_COMP08_B_CONT,
        skip = const ENT_COMP08_B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_phys_cb_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebp + 8]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_PHYS_CB_CONT,
        skip = const CLIENT_PHYS_CB_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp08_c_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 8]",
        "fstp dword ptr [esp + 8]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {skip}",
        "ret",
        cont = const ENT_COMP08_C_CONT,
        skip = const ENT_COMP08_C_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_ent3c_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x3c]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [esp + 0x18]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_ENT3C_CONT,
        skip = const CLIENT_ENT3C_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_ent04_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 4]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x40]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_ENT04_CONT,
        skip = const CLIENT_ENT04_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_ent3c_b_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x3c]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [esp + 0x1c]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_ENT3C_B_CONT,
        skip = const CLIENT_ENT3C_B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_body_a0_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0xa0]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop esi",
        "mov esp, ebp",
        "pop ebp",
        "ret 4",
        cont = const PHYS_BODY_A0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp3c_b_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x3c]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_COMP3C_B_CONT,
        skip = const ENT_COMP3C_B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp3c_c_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x3c]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_COMP3C_C_CONT,
        skip = const ENT_COMP3C_C_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_unlink_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [eax + 4]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 4], esi",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const TRIE_UNLINK_CONT,
        skip = const TRIE_UNLINK_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn dbx_null_attr_guard_entry() {
    std::arch::naked_asm!(
        "mov edi, ebx",
        "test edi, edi",
        "jz 2f",
        "mov esi, 0x0140b95c",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "push {null_ret}",
        "ret",
        cont = const DBX_NULL_ATTR_CONT,
        null_ret = const DBX_NULL_ATTR_NULL,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_bp_entry_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebx]",
        "test ecx, ecx",
        "jz 2f",
        "mov esi, dword ptr [ecx + 0x50]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_BP_ENTRY_CONT,
        skip = const SP_BP_ENTRY_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_bp_esi_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "cmp dword ptr [esi + 0xf0], 0",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_BP_ESI_CONT,
        skip = const SP_BP_ENTRY_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [0x018903e8]",
        "test eax, eax",
        "jz 2f",
        "mov edx, dword ptr [eax + 0x6c]",
        "test edx, edx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [ecx + 0x640], 0",
        "ret",
        cont = const SP_AI_MGR_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr2_guard_entry() {
    std::arch::naked_asm!(
        "test ebx, ebx",
        "jz 2f",
        "mov eax, dword ptr [ebx + 0x3c]",
        "test eax, eax",
        "jz 2f",
        "mov eax, dword ptr [ebx + 0x6c]",
        "test eax, eax",
        "jz 2f",
        "mov esi, dword ptr [eax + 0x74]",
        "push {cont}",
        "ret",
        "2:",
        "add esp, 4",
        "push {exit}",
        "ret",
        cont = const SP_AI_MGR2_CONT,
        exit = const SP_AI_MGR2_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr3_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [0x018903e8]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x80], 0",
        "jz 2f",
        "push ebp",
        "mov ebp, esp",
        "and esp, -16",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_AI_MGR3_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr4_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [0x018903e8]",
        "test edx, edx",
        "jz 2f",
        "mov eax, dword ptr [edx + 0x80]",
        "test eax, eax",
        "jz 2f",
        "fld dword ptr [eax + 0x18]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_AI_MGR4_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr5_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebx + 0x3e4]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop esi",
        "pop ebp",
        "pop ebx",
        "add esp, 4",
        "ret 4",
        cont = const SP_AI_MGR5_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_list2_null_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [esp + 0x1c]",
        "mov esi, dword ptr [edx]",
        "test esi, esi",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_LIST2_NULL_CONT,
        skip = const SP_LIST2_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_anim_res_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [edi]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x10]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi + 0x0c], 0",
        "mov dword ptr [esi + 0x10], 0",
        "mov dword ptr [esi + 0x14], 0",
        "mov dword ptr [esi + 0x28], 0",
        "push {exit}",
        "ret",
        cont = const SP_ANIM_RES_CONT,
        exit = const SP_ANIM_RES_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_anim_ctor_guard_entry() {
    std::arch::naked_asm!(
        "mov dword ptr [eax + 0x18], edx",
        "test ecx, ecx",
        "jz 2f",
        "mov ecx, dword ptr [ecx + 4]",
        "2:",
        "push {cont}",
        "ret",
        cont = const SP_ANIM_CTOR_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_list3_null_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 0x18]",
        "mov esi, dword ptr [eax + edi*4]",
        "test esi, esi",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_LIST3_NULL_CONT,
        skip = const SP_LIST3_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_anim_data_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [edi + 0xc8]",
        "test ecx, ecx",
        "jz 2f",
        "mov ecx, dword ptr [ecx + 0x3c]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_ANIM_DATA_CONT,
        exit = const SP_ANIM_DATA_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_anim_init1_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [ebx]",
        "test edx, edx",
        "jz 2f",
        "cmp dword ptr [edx + 0x5c], edi",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi + 0xe4], 0",
        "push {skip}",
        "ret",
        cont = const SP_ANIM_INIT1_CONT,
        skip = const SP_ANIM_INIT1_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_anim_init2_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebx + 4]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x10]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "push {cont}",
        "ret",
        cont = const SP_ANIM_INIT2_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr6_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x18]",
        "test ecx, ecx",
        "jz 2f",
        "fld dword ptr [ecx + 0x2c]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_AI_MGR6_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_f8c_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "fld dword ptr [eax + 0x8c]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_PHYS_F8C_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_terr_b14_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebx + 0x14]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_TERR_B14_CONT,
        skip = const SP_TERR_B14_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_add_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 4]",
        "test eax, eax",
        "jz 2f",
        "mov edx, dword ptr [eax + 0x2c]",
        "push {cont}",
        "ret",
        "2:",
        "ret 4",
        cont = const SP_PHYS_ADD_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_find_guard_entry() {
    std::arch::naked_asm!(
        "cmp edi, 0x10000",
        "jb 2f",
        "mov edx, edi",
        "mov eax, ebp",
        "sub edx, ebp",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_TRIE_FIND_CONT,
        skip = const SP_TRIE_FIND_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent44_flag_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esi + 0x44]",
        "test eax, eax",
        "jz 2f",
        "or dword ptr [eax + 0x10], 0x2000000",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_ENT44_FLAG_CONT,
        skip = const SP_ENT44_FLAG_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_9c_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x9c]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_PHYS_9C_CONT,
        exit = const SP_PHYS_9C_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr7_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov eax, dword ptr [eax + 0x18]",
        "test eax, eax",
        "jz 2f",
        "movss xmm0, dword ptr [eax + 0x38]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "pop esi",
        "add esp, 0x28",
        "ret 4",
        cont = const SP_AI_MGR7_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_ins_guard_entry() {
    std::arch::naked_asm!(
        "cmp ebx, 0x10000",
        "jb 2f",
        "cmp ebx, 0x7fff0000",
        "jae 2f",
        "mov ecx, ebx",
        "mov dword ptr [esp + 0x10], ebx",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_TRIE_INS_CONT,
        skip = const SP_TRIE_INS_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent44_c0_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x44]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_ENT44_C0_CONT,
        skip = const SP_ENT44_C0_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_80_guard_entry() {
    std::arch::naked_asm!(
        "sub esp, 8",
        "mov eax, dword ptr [esp + 0x10]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "add esp, 8",
        "ret 8",
        cont = const SP_PHYS_80_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr8_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov edi, dword ptr [eax + 0x18]",
        "test edi, edi",
        "jz 2f",
        "mov eax, dword ptr [ebx + 0xb00]",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop esi",
        "pop ebx",
        "add esp, 0x10",
        "ret",
        cont = const SP_AI_MGR8_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent44_flag2_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esi + 0x44]",
        "test eax, eax",
        "jz 2f",
        "or dword ptr [eax + 0x10], 0x2000000",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_ENT44_FLAG2_CONT,
        skip = const SP_ENT44_FLAG2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_4c_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x9c], 0",
        "jz 2f",
        "mov edx, dword ptr [eax + 0xa0]",
        "test edx, edx",
        "jz 2f",
        "cmp dword ptr [edx], 0",
        "jz 2f",
        "push ebp",
        "mov ebp, esp",
        "and esp, -16",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_PHYS_4C_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr9_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x18]",
        "test ecx, ecx",
        "jz 2f",
        "fld dword ptr [ecx + 0x40]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_AI_MGR9_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr10_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x18]",
        "test ecx, ecx",
        "jz 2f",
        "fld dword ptr [ecx + 0x28]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_AI_MGR10_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr11_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x18]",
        "test ecx, ecx",
        "jz 2f",
        "fld dword ptr [ecx + 0x30]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_AI_MGR11_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_10_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x10]",
        "and eax, 0x40000000",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 8",
        cont = const SP_PHYS_10_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent44_c0_b_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x44]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_ENT44_C0_B_CONT,
        skip = const SP_ENT44_C0_B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_9c_b_guard_entry() {
    std::arch::naked_asm!(
        "cmp dword ptr [ecx + 0x9c], 0",
        "jz 2f",
        "sub esp, 0x14",
        "push ebx",
        "push ebp",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_PHYS_9C_B_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr12_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [0x018903e8]",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x80], 0",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_AI_MGR12_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_vec_last_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "test edi, edi",
        "jz 2f",
        "mov ecx, dword ptr [edi + eax*4 - 4]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx + 0x68]",
        "push {cont}",
        "ret",
        "2:",
        "xor edx, edx",
        "push {zero}",
        "ret",
        cont = const SP_VEC_LAST_CONT,
        zero = const SP_VEC_LAST_ZERO,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent08_e0_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 8]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_ENT08_E0_CONT,
        skip = const SP_ENT08_E0_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_44_guard_entry() {
    std::arch::naked_asm!(
        "mov byte ptr [esi + 0x5c], 1",
        "mov ecx, dword ptr [esi + 0x44]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_PHYS_44_CONT,
        skip = const SP_PHYS_44_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr13_guard_entry() {
    std::arch::naked_asm!(
        "mov esi, dword ptr [ebp + 0x38]",
        "push edi",
        "test esi, esi",
        "jz 2f",
        "mov edi, dword ptr [esi + 8]",
        "push {cont}",
        "ret",
        "2:",
        "xor edi, edi",
        "push {skip}",
        "ret",
        cont = const SP_AI_MGR13_CONT,
        skip = const SP_AI_MGR13_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_c290_guard_entry() {
    std::arch::naked_asm!(
        "cmp dword ptr [ecx + 0x10], 0",
        "jz 2f",
        "push esi",
        "mov esi, dword ptr [esp + 8]",
        "push {cont}",
        "ret",
        "2:",
        "ret 4",
        cont = const SP_PHYS_C290_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent08_c4_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ecx + 8]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "cmp byte ptr [esp + 8], 0",
        "push {cont}",
        "ret",
        "2:",
        "ret 8",
        cont = const SP_ENT08_C4_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent80_68_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ebp + 0x80]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_ENT80_68_CONT,
        exit = const SP_ENT80_68_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr14_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebp + 0x3e4]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_AI_MGR14_CONT,
        skip = const SP_AI_MGR14_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr15_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebp + 0x3dc]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_AI_MGR15_CONT,
        skip = const SP_AI_MGR15_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_f80_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "fld dword ptr [ecx + 0x80]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_PHYS_F80_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr16_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "movzx eax, byte ptr [ecx + 0x66c]",
        "push {cont}",
        "ret",
        "2:",
        "ret 4",
        cont = const SP_AI_MGR16_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr17_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x7c]",
        "test ecx, ecx",
        "jz 2f",
        "fsub dword ptr [ecx + 0x10]",
        "2:",
        "push {cont}",
        "ret",
        cont = const SP_AI_MGR17_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_type_idx_guard_entry() {
    std::arch::naked_asm!(
        "push ebx",
        "mov ebx, dword ptr [esp + 8]",
        "test ebx, ebx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "xor al, al",
        "pop ebx",
        "ret 4",
        cont = const SP_TYPE_IDX_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_c250_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x10]",
        "test ecx, ecx",
        "jz 2f",
        "fmul qword ptr [0x014d9488]",
        "push {cont}",
        "ret",
        "2:",
        "fstp st(0)",
        "push {exit}",
        "ret",
        cont = const SP_PHYS_C250_CONT,
        exit = const SP_PHYS_C250_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_mgr18_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "fld dword ptr [ecx + 0xc0]",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "push {cont}",
        "ret",
        cont = const SP_AI_MGR18_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_9c_c_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [esi + 0x9c]",
        "test edx, edx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi + 0xcc], 0",
        "push {skip}",
        "ret",
        cont = const SP_PHYS_9C_C_CONT,
        skip = const SP_PHYS_9C_C_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_44_b_guard_entry() {
    std::arch::naked_asm!(
        "mov byte ptr [esi + 0x5c], bl",
        "mov eax, dword ptr [esi + 0x44]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_PHYS_44_B_CONT,
        skip = const SP_PHYS_44_B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_list_null_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [esi]",
        "test edx, edx",
        "jz 2f",
        "mov eax, dword ptr [esp + 0x14]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_LIST_NULL_CONT,
        skip = const SP_LIST_NULL_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_level_walk_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "mov eax, dword ptr [esi + 0x10]",
        "sub eax, dword ptr [esi + 0xc]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_LEVEL_WALK_CONT,
        skip = const SP_LEVEL_WALK_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_bbox_mesh_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ecx]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x44]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "xor al, al",
        "pop edi",
        "pop esi",
        "mov esp, ebp",
        "pop ebp",
        "ret 4",
        cont = const ENT_BBOX_MESH_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp14_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, ecx",
        "mov ecx, dword ptr [eax + 4]",
        "test ecx, ecx",
        "jz 2f",
        "cmp dword ptr [ecx + 0x14], 0",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const ENT_COMP14_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_comp14_b_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esi + 8]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop esi",
        "xor eax, eax",
        "ret 4",
        cont = const ENT_COMP14_B_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn smg_render_member2_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ebx + 0x40]",
        "mov dword ptr [esp + 0x10], eax",
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x70], 0",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SMG_RENDER_MEMBER2_CONT,
        skip = const SMG_RENDER_MEMBER2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn destr_mesh_guard1_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [eax + 0x78]",
        "add esp, 8",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const DESTR_MESH_GUARD1_CONT,
        skip = const DESTR_MESH_GUARD1_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn destr_mesh_guard2_entry() {
    std::arch::naked_asm!(
        "mov ebx, dword ptr [edx + 0x78]",
        "test ebx, ebx",
        "jz 2f",
        "cmp dword ptr [ebx + 0x44], 0",
        "push {cont}",
        "ret",
        "2:",
        "pop ebx",
        "push {skip}",
        "ret",
        cont = const DESTR_MESH_GUARD2_CONT,
        skip = const DESTR_MESH_GUARD2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_get_u32_4c_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 8]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x40]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
        cont = const ENT_GET_U32_4C_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn smg_occluder_guard_entry() {
    std::arch::naked_asm!(
        "xor esi, esi",
        "test ebx, ebx",
        "jz 2f",
        "mov esi, dword ptr [ebx + 0x138]",
        "2:",
        "push {cont}",
        "ret",
        cont = const SMG_OCCLUDER_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn smg_render_member_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebx + 0x4c]",
        "xor ebx, ebx",
        "test ecx, ecx",
        "jz 2f",
        "cmp dword ptr [ecx + 0x70], 0",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SMG_RENDER_MEMBER_CONT,
        skip = const SMG_RENDER_MEMBER_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_get_float90_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 8]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x40]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "ret",
        cont = const ENT_GET_FLOAT90_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_owner_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x40], edx",
        "je 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_OWNER_CONT,
        skip = const ENT_OWNER_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_walk_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x10]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const ENT_WALK_CONT,
        skip = const ENT_WALK_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_ent_init_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x5c]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const PHYS_ENT_INIT_CONT,
        skip = const PHYS_ENT_INIT_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_ent_init2_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [edi + 0x5c]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, dword ptr [0x014d976c]",
        "mov dword ptr [esp + 0x18], eax",
        "xor eax, eax",
        "push {skip}",
        "ret",
        cont = const PHYS_ENT_INIT2_CONT,
        skip = const PHYS_ENT_INIT2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ent_parent_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov ecx, dword ptr [ecx + 0x40]",
        "add ecx, 0x170",
        "test edx, edx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi + 0xc], ecx",
        "push {skip}",
        "ret",
        cont = const ENT_PARENT_CONT,
        skip = const ENT_PARENT_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn vo_walk_arr_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx + 0x50]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const VO_WALK_ARR_CONT,
        skip = const VO_WALK_ARR_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn bp_lookup_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [eax + 0xd8]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const BP_LOOKUP_GUARD_CONT,
        skip = const BP_LOOKUP_GUARD_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[repr(align(16))]
struct IdentityMat4([f32; 16]);

#[cfg(target_arch = "x86")]
static IDENTITY_MAT4: IdentityMat4 = IdentityMat4([
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
]);

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_phys_get_xform(this: usize, _edx: *mut c_void) -> usize {
    if this < 0x10000 {
        return (&raw const IDENTITY_MAT4) as usize;
    }
    let inner = unsafe { *((this + 0x8C) as *const usize) };
    if inner < 0x10000 {
        return (&raw const IDENTITY_MAT4) as usize;
    }
    inner + 0xE0
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_phys_part_bounds(
    this: usize,
    edx: *mut c_void,
    out: *mut [f32; 4],
    idx: usize,
) -> *mut [f32; 4] {
    if this < 0x10000 {
        if !out.is_null() {
            unsafe { *out = [0.0; 4] };
        }
        return out;
    }
    if let Some(&orig) = ORIG_PHYS_PART_BOUNDS.get() {
        return unsafe { orig(this, edx, out, idx) };
    }
    out
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_mesh_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebx]",
        "test ecx, ecx",
        "jz 2f",
        "cmp dword ptr [ecx + 0xc], 1",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "push {skip}",
        "ret",
        cont = const CLIENT_MESH_CONT,
        skip = const CLIENT_MESH_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
static ZERO_BBOX_BUF: [u32; 16] = [0; 16];

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn smg_bbox2_guard_entry() {
    std::arch::naked_asm!(
        "test edx, edx",
        "jz 2f",
        "mov eax, dword ptr [edx]",
        "test eax, eax",
        "jz 2f",
        "mov eax, dword ptr [eax + 0x44]",
        "test eax, eax",
        "jnz 3f",
        "2:",
        "lea eax, [{zero}]",
        "3:",
        "push {cont}",
        "ret",
        zero = sym ZERO_BBOX_BUF,
        cont = const SMG_BBOX2_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_tick_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x10]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_TICK_CONT,
        skip = const CLIENT_TICK_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn mesh_model_res_null_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esi]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0xc]",
        "push {cont}",
        "ret",
        "2:",
        "add esp, 8",
        "pop edi",
        "pop esi",
        "ret 0xc",
        cont = const MESH_MODEL_RES_NULL_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_parent_cycle_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0xc]",
        "cmp ecx, esi",
        "jne 2f",
        "xor ecx, ecx",
        "2:",
        "test ecx, ecx",
        "push {cont}",
        "ret",
        cont = const TRIE_PARENT_CYCLE_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ingame_comp_query_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ecx + 0x10]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
        cont = const INGAME_COMP_QUERY_CONT,
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_ingame_comp_update(
    this: *mut c_void,
    edx: *mut c_void,
    arg0: *mut c_void,
) {
    if (this as usize) < 0x10000 || unsafe { *((this as usize + 0x10) as *const usize) } < 0x10000 {
        return;
    }
    if let Some(&orig) = ORIG_INGAME_COMP_UPDATE.get() {
        unsafe { orig(this, edx, arg0) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_ingame_comp_init2(
    this: *mut c_void,
    edx: *mut c_void,
    arg0: *mut c_void,
    arg1: *mut c_void,
) {
    if (arg1 as usize) < 0x10000 {
        return;
    }
    if let Some(&orig) = ORIG_INGAME_COMP_INIT2.get() {
        unsafe { orig(this, edx, arg0, arg1) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_ingame_comp_init(
    this: *mut c_void,
    edx: *mut c_void,
    arg0: *mut c_void,
) {
    if (arg0 as usize) < 0x10000 {
        return;
    }
    if let Some(&orig) = ORIG_INGAME_COMP_INIT.get() {
        unsafe { orig(this, edx, arg0) };
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_query_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0xb8]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const PHYS_QUERY_CONT,
        skip = const PHYS_QUERY_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn smg_client_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [eax]",
        "test ecx, ecx",
        "jz 2f",
        "mov edi, dword ptr [ecx + 0x44]",
        "mov edx, dword ptr [esp + 0x10]",
        "test edx, edx",
        "jz 2f",
        "mov ebx, dword ptr [edx + 0x44]",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop ebx",
        "xor esi, esi",
        "push {skip}",
        "ret",
        cont = const SMG_CLIENT_CONT,
        skip = const SMG_CLIENT_SKIP,
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_phys_userdata(this: usize, _edx: *mut c_void, val: usize) {
    if this >= 0x10000 {
        unsafe { *((this + 0x68) as *mut usize) = val };
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_set_part_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx + 0x88]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 4",
        cont = const PHYS_SET_PART_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn phys_create_guard_entry() {
    std::arch::naked_asm!(
        "mov dword ptr [esi + 4], eax",
        "test eax, eax",
        "jz 2f",
        "mov edx, dword ptr [eax]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const PHYS_CREATE_CONT,
        skip = const PHYS_CREATE_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn client_ref_obj_fact_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ebx + 0x78]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x50]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const CLIENT_REF_OBJ_FACT_CONT,
        skip = const CLIENT_REF_OBJ_FACT_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn smg_bbox_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ebx]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x44]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SMG_BBOX_GUARD_CONT,
        skip = const SMG_BBOX_GUARD_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn smg_mesh_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0xec]",
        "xor eax, eax",
        "test edx, edx",
        "jz 2f",
        "mov eax, dword ptr [edx + 0x18]",
        "2:",
        "push {cont}",
        "ret",
        cont = const SMG_MESH_GUARD_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn world_entry_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "mov eax, dword ptr [eax + 0x88]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const WORLD_ENTRY_GUARD_CONT,
        skip = const WORLD_ENTRY_GUARD_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ref_obj_fact_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esi + 0x78]",
        "test eax, eax",
        "jz 2f",
        "mov ebx, dword ptr [eax + 0x50]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const REF_OBJ_FACT_CONT,
        skip = const REF_OBJ_FACT_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn vo_walk_sub_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [eax + ebx*4]",
        "test ecx, ecx",
        "jz 2f",
        "mov ecx, dword ptr [ecx + 0x2c]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "add esp, 4",
        "push {skip}",
        "ret",
        cont = const VO_WALK_SUB_CONT,
        skip = const VO_WALK_SUB_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn vo_walk_child_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0xd8]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const VO_WALK_CHILD_CONT,
        skip = const VO_WALK_CHILD_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ref_obj_bp2_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esi + 0x78]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x50]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const REF_OBJ_BP2_CONT,
        skip = const REF_OBJ_BP2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn ref_obj_bp_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [edi + 0x78]",
        "test eax, eax",
        "jz 2f",
        "mov edi, dword ptr [eax + 0x50]",
        "test edi, edi",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const REF_OBJ_BP_CONT,
        skip = const REF_OBJ_BP_SKIP,
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_mesh_model_init(
    this: *mut c_void,
    edx: *mut c_void,
    asset: *mut c_void,
    a1: *mut c_void,
    a2: *mut c_void,
) {
    if (asset as usize) < 0x10000 {
        if (this as usize) >= 0x10000 {
            let p = this as *mut u32;
            unsafe {
                *p.add(2) = 0xFFFF_FFFF;
                *p.add(3) = 0xFFFF_FFFF;
                *p.add(4) = 0xFFFF_FFFF;
                *p.add(5) = 0xFFFF_FFFF;
                *p.add(6) = 0xFFFF_FFFF;
                *p.add(7) = 0xFFFF_FFFF;
            }
        }
        return;
    }
    if let Some(&orig) = ORIG_MESH_MODEL_INIT.get() {
        unsafe { orig(this, edx, asset, a1, a2) };
    }
}

#[cfg(target_arch = "x86")]
static EMPTY_CSTR_BYTES: [u8; 4] = [0; 4];

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_res_proxy_load(
    this: *mut c_void,
    edx: *mut c_void,
    name_ptr: *const *const u8,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
) {
    let empty_ptr: *const u8 = EMPTY_CSTR_BYTES.as_ptr();
    let safe_name = if (name_ptr as usize) < 0x10000 {
        &raw const empty_ptr
    } else {
        name_ptr
    };
    if let Some(&orig) = ORIG_RES_PROXY_LOAD.get() {
        unsafe { orig(this, edx, safe_name, a1, a2, a3, a4) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn empty_array_size(_this: *mut c_void, _edx: *mut c_void) -> u32 {
    0
}

#[cfg(target_arch = "x86")]
type EmptyArrayMethod = unsafe extern "fastcall" fn(*mut c_void, *mut c_void) -> u32;

#[cfg(target_arch = "x86")]
#[repr(C)]
struct EmptyRefArray {
    vtable: *const EmptyArrayMethod,
    pad: [usize; 6],
}

#[cfg(target_arch = "x86")]
static EMPTY_REF_ARRAY_VTBL: [EmptyArrayMethod; 8] = [empty_array_size; 8];

#[cfg(target_arch = "x86")]
static mut EMPTY_REF_ARRAY: EmptyRefArray = EmptyRefArray {
    vtable: (&raw const EMPTY_REF_ARRAY_VTBL) as *const EmptyArrayMethod,
    pad: [0; 6],
};

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_get_array_18(
    this: usize,
    _edx: *mut c_void,
    out: *mut usize,
) -> *mut usize {
    if !out.is_null() {
        if this < 0x10000 {
            unsafe { *out = (&raw const EMPTY_REF_ARRAY) as usize };
        } else {
            unsafe { *out = this + 0x18 };
        }
    }
    out
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_level_list_walk(
    this: *mut c_void,
    edx: *mut c_void,
    a0: *mut c_void,
    a1: *mut c_void,
    a2: *mut c_void,
) {
    if (a2 as usize) < 0x10000 {
        return;
    }
    if let Some(&orig) = ORIG_LEVEL_LIST_WALK.get() {
        unsafe { orig(this, edx, a0, a1, a2) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn detour_vo_walk_list(owner: *mut c_void, a: *mut c_void, b: *mut c_void) {
    if (owner as usize) < 0x10000 {
        return;
    }
    if let Some(&orig) = ORIG_VO_WALK_LIST.get() {
        unsafe { orig(owner, a, b) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn detour_vo_walk(object: *mut c_void, a: *mut c_void, b: *mut c_void) {
    if object.is_null() {
        return;
    }
    let vtbl = unsafe { *(object as *const *const usize) };
    if !vtbl.is_null() {
        let get_type: GetTypeFn = unsafe { std::mem::transmute(*vtbl.add(2)) };
        let ty = unsafe { get_type(object, std::ptr::null_mut()) };
        if !ty.is_null() {
            let is_kind: IsKindOfFn = unsafe { std::mem::transmute(IS_KIND_OF) };
            if unsafe { is_kind(ty, std::ptr::null_mut(), REF_OBJECT_DATA_TYPE as *mut c_void) } {
                let bp = unsafe { *((object as usize + 0x78) as *const usize) };
                if bp == 0 || unsafe { *((bp + 0x50) as *const usize) } == 0 {
                    return;
                }
            }
        }
    }
    if let Some(&orig) = ORIG_VO_WALK.get() {
        unsafe { orig(object, a, b) };
    }
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_cb24_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x44]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [esp + 4]",
        "mov dword ptr [eax + 0x24], ecx",
        "2:",
        "ret 4",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_arr10_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esp + 0x18]",
        "cmp ecx, 0x1000",
        "jbe 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_ARR10_CONT,
        exit = const SP_ARR10_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_veh_slot_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x2b8]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + eax * 4]",
        "test eax, eax",
        "jnz 3f",
        "mov eax, dword ptr [ecx]",
        "test eax, eax",
        "jz 2f",
        "3:",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_VEH_SLOT_CONT,
        exit = const SP_VEH_SLOT_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_a0_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0xa0]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + eax * 4]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_PHYS_A0_CONT,
        exit = const SP_PHYS_A0_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_veh_slot2_guard_entry() {
    std::arch::naked_asm!(
        "mov esi, dword ptr [esi + 0x1f8]",
        "test esi, esi",
        "jz 2f",
        "mov eax, dword ptr [esi + eax * 4]",
        "test eax, eax",
        "jnz 3f",
        "mov eax, dword ptr [esi]",
        "test eax, eax",
        "jz 2f",
        "3:",
        "push {cont}",
        "ret",
        "2:",
        "pop esi",
        "xor eax, eax",
        "ret",
        cont = const SP_VEH_SLOT2_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_obj20_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 3f",
        "cmp byte ptr [eax + 0x20], 0",
        "jne 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {jne_dst}",
        "ret",
        "3:",
        "push {ret_dst}",
        "ret",
        cont = const SP_AI_OBJ20_CONT,
        jne_dst = const SP_AI_OBJ20_JNE,
        ret_dst = const SP_AI_OBJ20_RET,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_esi14_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "mov ecx, dword ptr [esi + 0x14]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_AI_ESI14_CONT,
        skip = const SP_AI_ESI14_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ent_edi3c_guard_entry() {
    std::arch::naked_asm!(
        "mov edi, eax",
        "test edi, edi",
        "jz 2f",
        "mov eax, dword ptr [edi + 0x3c]",
        "test eax, eax",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_ENT_EDI3C_CONT,
        skip = const SP_ENT_EDI3C_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_matcpy_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, 0x10",
        "test esi, esi",
        "jz 2f",
        "rep movsd",
        "jmp 3f",
        "2:",
        "push eax",
        "xor eax, eax",
        "rep stosd",
        "pop eax",
        "3:",
        "test eax, eax",
        "jz 4f",
        "push {cont}",
        "ret",
        "4:",
        "mov ecx, dword ptr [esp + 0x18]",
        "mov dword ptr [ebx + 0x44], ecx",
        "mov dword ptr [ebx + 0x48], 0",
        "push {exit}",
        "ret",
        cont = const SP_AI_MATCPY_CONT,
        exit = const SP_AI_MATCPY_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_getac_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0xac]",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_arr1c_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 4]",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [eax + 0x1c]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [esp + 4]",
        "mov eax, dword ptr [ecx + edx * 4]",
        "ret 4",
        "2:",
        "xor eax, eax",
        "ret 4",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_nav_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jz 2f",
        "mov edx, dword ptr [esi]",
        "test edx, edx",
        "jz 2f",
        "test al, al",
        "mov eax, dword ptr [edx + 0x18]",
        "push {cont}",
        "ret",
        "2:",
        "mov al, 1",
        "pop edi",
        "pop esi",
        "ret 8",
        cont = const SP_AI_NAV_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_sub14_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jz 2f",
        "cmp dword ptr [eax + 0x14], 0",
        "push esi",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_AI_SUB14_CONT,
        exit = const SP_AI_SUB14_EXIT,
    );
}

#[cfg(target_arch = "x86")]
static ORIG_NODE_WALK: OnceLock<
    unsafe extern "fastcall" fn(usize, *mut c_void, usize, usize) -> usize,
> = OnceLock::new();
#[cfg(target_arch = "x86")]
static NODE_WALK_LOCK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_node_walk(
    this: usize,
    edx: *mut c_void,
    a: usize,
    b: usize,
) -> usize {
    thread_local! {
        static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    if (this & 3) != 0
        || this < 0x0100_0000
        || this >= 0x7FFF_0000
        || a < 0x0001_0000
        || a >= 0x7FFF_0000
        || (b >= a && b - a <= 8)
        || unsafe { IsBadReadPtr(this as *const c_void, 16) } != 0
    {
        return a;
    }
    let d = DEPTH.with(|c| c.get());
    if d >= 64 {
        return a;
    }
    if d == 0 {
        while NODE_WALK_LOCK
            .compare_exchange_weak(
                false,
                true,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {
            std::hint::spin_loop();
        }
    }
    DEPTH.with(|c| c.set(d + 1));
    let ret = if let Some(&orig) = ORIG_NODE_WALK.get() {
        unsafe { orig(this, edx, a, b) }
    } else {
        a
    };
    DEPTH.with(|c| c.set(d));
    if d == 0 {
        NODE_WALK_LOCK.store(false, std::sync::atomic::Ordering::Release);
    }
    ret
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_44_c_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x44]",
        "test ecx, ecx",
        "jz 2f",
        "mov eax, dword ptr [ecx + 0x10]",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_PHYS_44_C_CONT,
        exit = const SP_PHYS_44_C_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_bd80_guard_entry() {
    std::arch::naked_asm!(
        "cmp esi, 0x01000000",
        "jb 2f",
        "cmp esi, 0x7fff0000",
        "jae 2f",
        "mov ecx, dword ptr [esi]",
        "cmp ecx, 0x01000000",
        "jb 2f",
        "cmp ecx, 0x7fff0000",
        "jae 2f",
        "mov edx, dword ptr [ecx]",
        "mov edx, dword ptr [edx + 0x48]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_PHYS_BD80_CONT,
        skip = const SP_PHYS_BD80_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_ebx24_guard_entry() {
    std::arch::naked_asm!(
        "mov ebx, dword ptr [esp + 0x24]",
        "test ebx, ebx",
        "jz 2f",
        "mov edx, dword ptr [ebx]",
        "push {cont}",
        "ret",
        "2:",
        "pop ebp",
        "push {skip}",
        "ret",
        cont = const SP_AI_EBX24_CONT,
        skip = const SP_AI_EBX24_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_pathdata_guard_entry() {
    std::arch::naked_asm!("ret 0x10");
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_bd0a_guard_entry() {
    std::arch::naked_asm!(
        "cmp esi, 0x01000000",
        "jb 2f",
        "cmp esi, 0x7fff0000",
        "jae 2f",
        "mov dword ptr [esi], eax",
        "test eax, eax",
        "jz 2f",
        "mov ecx, dword ptr [esp + 0x18]",
        "cmp ecx, 0x01000000",
        "jb 2f",
        "cmp ecx, 0x7fff0000",
        "jae 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_PHYS_BD0A_CONT,
        skip = const SP_PHYS_BD0A_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_eb86_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "mov eax, dword ptr [edx + 4]",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_AI_EB86_CONT,
        exit = const SP_AI_EB86_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_190_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x190]",
        "test ecx, ecx",
        "jz 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_PHYS_190_CONT,
        skip = const SP_PHYS_190_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_phys_c970_guard_entry() {
    std::arch::naked_asm!(
        "test ecx, ecx",
        "jz 2f",
        "push ebx",
        "mov ebx, esp",
        "sub esp, 8",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 0xc",
        cont = const SP_PHYS_C970_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ai_ebd2_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 4]",
        "test ecx, ecx",
        "jz 2f",
        "mov edx, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_AI_EBD2_CONT,
        skip = const SP_AI_EBD2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_fda3_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x01000000",
        "jb 2f",
        "mov eax, dword ptr [eax]",
        "cmp eax, 0x01000000",
        "jb 2f",
        "mov ecx, dword ptr [eax + 0x0c]",
        "mov edx, dword ptr [eax + 0x10]",
        "push {cont}",
        "ret",
        "2:",
        "xor ecx, ecx",
        "xor edx, edx",
        "push {cont}",
        "ret",
        cont = const SP_FDA3_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_unlink2_guard_entry() {
    std::arch::naked_asm!(
        "test al, 3",
        "jnz 2f",
        "cmp eax, 0x02000000",
        "jb 2f",
        "cmp eax, 0x50000000",
        "jae 2f",
        "mov dword ptr [eax + 0x0c], edi",
        "mov eax, dword ptr [eax + 4]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const TRIE_UNLINK2_CONT,
        skip = const TRIE_UNLINK2_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_alloc_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x01000000",
        "jb 2f",
        "cmp ecx, 0x7fff0000",
        "jae 2f",
        "mov eax, dword ptr [ecx]",
        "mov eax, dword ptr [eax]",
        "jmp eax",
        "2:",
        "mov eax, dword ptr [esp + 4]",
        "push eax",
        "mov eax, 0x00420db0",
        "call eax",
        "add esp, 4",
        "ret 8",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_find_ptr_guard_entry() {
    std::arch::naked_asm!(
        "cmp esi, ebx",
        "je 2f",
        "test esi, 3",
        "jnz 2f",
        "cmp esi, 0x01000000",
        "jb 2f",
        "cmp esi, 0x60000000",
        "jae 2f",
        "test byte ptr [esi + 8], 1",
        "push {cont}",
        "ret",
        "2:",
        "push {notfound}",
        "ret",
        cont = const TRIE_FIND_PTR_CONT,
        notfound = const TRIE_FIND_PTR_NOTFOUND,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn trie_leaf_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, ecx",
        "and eax, -4",
        "cmp eax, 0x01000000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jae 2f",
        "mov ecx, dword ptr [eax + 8]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
        cont = const TRIE_LEAF_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_list_4968_guard_entry() {
    std::arch::naked_asm!(
        "test esi, 3",
        "jnz 2f",
        "cmp esi, 0x01000000",
        "jb 2f",
        "cmp esi, 0x60000000",
        "jae 2f",
        "mov eax, dword ptr [esi + 0x08]",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x01000000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jae 2f",
        "mov eax, dword ptr [eax + 0x1c]",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x01000000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jae 2f",
        "mov eax, dword ptr [esi + 0x0c]",
        "test eax, eax",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_LIST_4968_CONT,
        exit = const SP_LIST_4968_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4495e0_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 4]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [eax + 0x10]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
        cont = const SP_4495E0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_9c4055_guard_entry() {
    std::arch::naked_asm!(
        "cmp ebx, 0x10000",
        "jb 2f",
        "mov edi, dword ptr [ebx + 0x10]",
        "cmp dword ptr [ecx - 8], edi",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_9C4055_CONT,
        exit = const SP_9C4055_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4493a3_guard_entry() {
    std::arch::naked_asm!(
        "cmp esi, 0x10000",
        "jb 2f",
        "mov ecx, esi",
        "cmp dword ptr [esi + 0x10], 0",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_4493A3_CONT,
        skip = const SP_4493A3_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_pool_4cf9_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x01000000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jae 2f",
        "mov ecx, dword ptr [eax - 8]",
        "mov dword ptr [eax], ecx",
        "mov ecx, dword ptr [eax - 4]",
        "mov dword ptr [eax + 4], ecx",
        "2:",
        "push {after}",
        "ret",
        after = const SP_POOL_4CF9_AFTER,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8950_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 8]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, dword ptr [esp + 4]",
        "cmp eax, 0x10000",
        "jb 3f",
        "mov dword ptr [eax], 0",
        "3:",
        "ret 8",
        cont = const SP_TRIE_4C8950_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c89b0_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 4]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 4",
        cont = const SP_TRIE_4C89B0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8a10_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 4]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 4",
        cont = const SP_TRIE_4C8A10_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8a70_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [esp + 4]",
        "sub esp, 0x400",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret 4",
        cont = const SP_TRIE_4C8A70_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8ad0_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "sub esp, 0xc",
        "push ebx",
        "push esi",
        "mov esi, ecx",
        "push {cont}",
        "ret",
        "2:",
        "ret 8",
        cont = const SP_TRIE_4C8AD0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_trie_4c8c80_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "sub esp, 0xf4",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_TRIE_4C8C80_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4cf9a6_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "lea ecx, [eax - 8]",
        "push ecx",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "mov ecx, dword ptr [eax - 8]",
        "mov dword ptr [eax], ecx",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_4CF9A6_CONT,
        skip = const SP_4CF9A6_SKIP,
    );
}

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_part_setup(part: usize, edx: *mut c_void) {
    if part != 0 {
        let data = unsafe { *((part + 4) as *const usize) };
        let owner = unsafe { *((part + 8) as *const usize) };
        if data != 0 && owner != 0 && unsafe { *((data + 0x60) as *const usize) } != 0 {
            let vtable = unsafe { *(owner as *const usize) };
            if vtable != 0 {
                let lookup: PartLookupFn = unsafe { *((vtable + 0xD4) as *const PartLookupFn) };
                let idx = unsafe { *((data + 0x18) as *const u32) };
                let part_obj = unsafe { lookup(owner, std::ptr::null_mut(), idx) };
                if part_obj == 0 || unsafe { *((part_obj + 0x50) as *const usize) } == 0 {
                    return;
                }
            }
        }
    }
    if let Some(&orig) = ORIG_PART_SETUP.get() {
        unsafe { orig(part, edx) };
    }
}

#[cfg(target_arch = "x86")]
unsafe extern "cdecl" fn detour_str_eq(s: *const EaString, text: *const u8) -> bool {
    let eq = if let Some(&orig) = ORIG_STR_EQ.get() {
        unsafe { orig(s, text) }
    } else {
        false
    };
    if eq || text as usize != CONQUEST_LITERAL || s.is_null() {
        return eq;
    }
    let (b, e) = unsafe { ((*s).begin, (*s).end) };
    if b.is_null() || e <= b {
        return false;
    }
    let n = e as usize - b as usize;
    if n == 9 {
        unsafe { std::slice::from_raw_parts(b, 9) == b"onslaught" }
    } else if n == 14 {
        unsafe { std::slice::from_raw_parts(b, 14) == b"airsuperiority" }
    } else {
        false
    }
}
