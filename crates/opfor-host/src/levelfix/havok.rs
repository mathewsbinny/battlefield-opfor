//! `havok` subsystem compatibility guards for `opfor-host`.
#![allow(dead_code, non_snake_case, unused_imports)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use opfor_vfs::ModeId;

use super::*;

const PART_SETUP: usize = 0x008F_4720;

const PART_SETUP_PROLOGUE: [u8; 6] = [0x83, 0xEC, 0x08, 0x57, 0x8B, 0xF9];

// StaticModelGroup null-member-mesh guard (`0x008F9357`: `mov ecx, [esi+0xec]; mov eax, [edx+0x18]`)
const SMG_MESH_GUARD: usize = 0x008F_9357;

const SMG_MESH_GUARD_PROLOGUE: [u8; 9] = [0x8B, 0x8E, 0xEC, 0x00, 0x00, 0x00, 0x8B, 0x42, 0x18];

const SMG_MESH_GUARD_CONT: usize = 0x008F_9360;

// StaticModelGroup bbox null-mesh guard (`0x008F7207`: `mov ecx, [ebx]; mov eax, [ecx+0x44]`)
const SMG_BBOX_GUARD: usize = 0x008F_7207;

const SMG_BBOX_GUARD_PROLOGUE: [u8; 5] = [0x8B, 0x0B, 0x8B, 0x41, 0x44];

const SMG_BBOX_GUARD_CONT: usize = 0x008F_720C;

const SMG_BBOX_GUARD_SKIP: usize = 0x008F_728D;

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

// PhysicsBody::getTransform null-this guard (`0x00AA72E0`: `mov eax, [ecx+0x8c]; add eax, 0xe0; ret`)
const PHYS_GET_XFORM: usize = 0x00AA_72E0;

const PHYS_GET_XFORM_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0x8C, 0x00, 0x00, 0x00];

// StaticModelGroup primary-LOD null-mesh guard (`0x008F7174`: `mov eax, [edx]; mov eax, [eax+0x44]`)
const SMG_BBOX2_GUARD: usize = 0x008F_7174;

const SMG_BBOX2_PROLOGUE: [u8; 5] = [0x8B, 0x02, 0x8B, 0x40, 0x44];

const SMG_BBOX2_CONT: usize = 0x008F_7179;

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

// PhysicsBody::getField150 null-this guard (`0x00AA75B0`: `mov eax, [ecx+0x8c]; add eax, 0x150; ret`)
const PHYS_GET_VEL_GUARD: usize = 0x00AA_75B0;

const PHYS_GET_VEL_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0x8C, 0x00, 0x00, 0x00];

// Physics/collision slot null-entry guard (`0x005F6354`: `mov [esp+0x10], eax; mov eax, [eax+0xc]`)
const PHYS_SLOT_NULL_GUARD: usize = 0x005F_6354;

const PHYS_SLOT_NULL_PROLOGUE: [u8; 7] = [0x89, 0x44, 0x24, 0x10, 0x8B, 0x40, 0x0C];

const PHYS_SLOT_NULL_CONT: usize = 0x005F_635B;

const PHYS_SLOT_NULL_SKIP: usize = 0x005F_6DEA;

// Entity bbox merge null-MeshAsset guard (`0x008946CA`: `mov eax, [eax]; movups xmm1, [esi]; mov eax, [eax+0x44]`)
const ENT_BBOX_MERGE_GUARD: usize = 0x0089_46CA;

const ENT_BBOX_MERGE_PROLOGUE: [u8; 8] = [0x8B, 0x00, 0x0F, 0x10, 0x0E, 0x8B, 0x40, 0x44];

const ENT_BBOX_MERGE_CONT: usize = 0x0089_46D2;

const ENT_BBOX_MERGE_SKIP: usize = 0x0089_4790;

// Client physics impulse null-body guard (`0x0087E048`: `mov edx, [esi]; mov eax, [edx+0x84]`)
const CLIENT_IMPULSE_GUARD: usize = 0x0087_E048;

const CLIENT_IMPULSE_PROLOGUE: [u8; 8] = [0x8B, 0x16, 0x8B, 0x82, 0x84, 0x00, 0x00, 0x00];

const CLIENT_IMPULSE_CONT: usize = 0x0087_E050;

const CLIENT_IMPULSE_SKIP: usize = 0x0087_E250;

// StaticModel/Destruction getBoundingRadius null-MeshAsset guard 4 (`0x00894360`: `mov eax, [ecx+0x1cc]`)
const DESTR_RADIUS_GUARD4: usize = 0x0089_4360;

const DESTR_RADIUS_GUARD4_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0xCC, 0x01, 0x00, 0x00];

// Physics/collision slot null-esi write guard (`0x005F6CDE`: `test eax, eax; mov [esi+0x90], eax`)
const PHYS_SLOT_ESI_GUARD: usize = 0x005F_6CDE;

const PHYS_SLOT_ESI_PROLOGUE: [u8; 8] = [0x85, 0xC0, 0x89, 0x86, 0x90, 0x00, 0x00, 0x00];

const PHYS_SLOT_ESI_CONT: usize = 0x005F_6CE6;

const PHYS_SLOT_ESI_SKIP: usize = 0x005F_6DEA;

// Client physics callback null-arg0 guard (`0x00873EC9`: `mov ecx, [ebp+8]; mov edx, [ecx]`)
const CLIENT_PHYS_CB_GUARD: usize = 0x0087_3EC9;

const CLIENT_PHYS_CB_PROLOGUE: [u8; 5] = [0x8B, 0x4D, 0x08, 0x8B, 0x11];

const CLIENT_PHYS_CB_CONT: usize = 0x0087_3ECE;

const CLIENT_PHYS_CB_SKIP: usize = 0x0087_3EDB;

// Physics body update null-fielda0 guard (`0x00ACDE20`: `mov ecx, [esi+0xa0]; mov eax, [ecx]`)
const PHYS_BODY_A0_GUARD: usize = 0x00AC_DE20;

const PHYS_BODY_A0_PROLOGUE: [u8; 8] = [0x8B, 0x8E, 0xA0, 0x00, 0x00, 0x00, 0x8B, 0x01];

const PHYS_BODY_A0_CONT: usize = 0x00AC_DE28;

// SP physics float `+0x8c` null guard (`0x00B11175`: `fld dword ptr [eax+0x8c]`)
const SP_PHYS_F8C_GUARD: usize = 0x00B1_1175;

const SP_PHYS_F8C_PROLOGUE: [u8; 6] = [0xD9, 0x80, 0x8C, 0x00, 0x00, 0x00];

const SP_PHYS_F8C_CONT: usize = 0x00B1_117B;

// SP physics add null-arg guard (`0x00AF7140`: `mov eax, [esp+4]; mov edx, [eax+0x2c]`)
const SP_PHYS_ADD_GUARD: usize = 0x00AF_7140;

const SP_PHYS_ADD_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x8B, 0x50, 0x2C];

const SP_PHYS_ADD_CONT: usize = 0x00AF_7147;

// SP physics `[esi+0x9c]` null guard (`0x00B090A5`)
const SP_PHYS_9C_GUARD: usize = 0x00B0_90A5;

const SP_PHYS_9C_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0x9C, 0x00, 0x00, 0x00];

const SP_PHYS_9C_CONT: usize = 0x00B0_90AB;

const SP_PHYS_9C_EXIT: usize = 0x00B0_90E9;

// SP physics null arg1 guard (`0x00ABECF0`: `sub esp, 8; mov eax, [esp+0x10]`)
const SP_PHYS_80_GUARD: usize = 0x00AB_ECF0;

const SP_PHYS_80_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x08, 0x8B, 0x44, 0x24, 0x10];

const SP_PHYS_80_CONT: usize = 0x00AB_ECF7;

// SP physics `[eax+0x9c]/[eax+0xa0]` null guard (`0x00B03F20`)
const SP_PHYS_4C_GUARD: usize = 0x00B0_3F20;

const SP_PHYS_4C_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];

const SP_PHYS_4C_CONT: usize = 0x00B0_3F26;

// SP physics null `this` guard (`0x00A9E7F0`: `mov eax, [ecx+0x10]; and eax, 0x40000000`)
const SP_PHYS_10_GUARD: usize = 0x00A9_E7F0;

const SP_PHYS_10_PROLOGUE: [u8; 8] = [0x8B, 0x41, 0x10, 0x25, 0x00, 0x00, 0x00, 0x40];

const SP_PHYS_10_CONT: usize = 0x00A9_E7F8;

// SP physics `[ecx+0x9c]` null guard #2 (`0x00B08CE0`)
const SP_PHYS_9C_B_GUARD: usize = 0x00B0_8CE0;

const SP_PHYS_9C_B_PROLOGUE: [u8; 5] = [0x83, 0xEC, 0x14, 0x53, 0x55];

const SP_PHYS_9C_B_CONT: usize = 0x00B0_8CE5;

// SP physics `[esi+0x44]` null guard (`0x00B0C25A`: `mov ecx, [esi+0x44]; mov byte [esi+0x5c], 1`)
const SP_PHYS_44_GUARD: usize = 0x00B0_C25A;

const SP_PHYS_44_PROLOGUE: [u8; 7] = [0x8B, 0x4E, 0x44, 0xC6, 0x46, 0x5C, 0x01];

const SP_PHYS_44_CONT: usize = 0x00B0_C261;

const SP_PHYS_44_SKIP: usize = 0x00B0_C27E;

// SP Havok body null shape `[ecx+0x10]` guard (`0x00CCC290`)
const SP_PHYS_C290_GUARD: usize = 0x00CC_C290;

const SP_PHYS_C290_PROLOGUE: [u8; 5] = [0x56, 0x8B, 0x74, 0x24, 0x08];

const SP_PHYS_C290_CONT: usize = 0x00CC_C295;

// SP physics float `+0x80` null guard (`0x00B115A3`: `fld dword ptr [ecx+0x80]`)
const SP_PHYS_F80_GUARD: usize = 0x00B1_15A3;

const SP_PHYS_F80_PROLOGUE: [u8; 6] = [0xD9, 0x81, 0x80, 0x00, 0x00, 0x00];

const SP_PHYS_F80_CONT: usize = 0x00B1_15A9;

// SP Havok body null shape `[esi+0x10]` #2 guard (`0x00CCC247`)
const SP_PHYS_C250_GUARD: usize = 0x00CC_C247;

const SP_PHYS_C250_PROLOGUE: [u8; 9] = [0x8B, 0x4E, 0x10, 0xDC, 0x0D, 0x88, 0x94, 0x4D, 0x01];

const SP_PHYS_C250_CONT: usize = 0x00CC_C250;

const SP_PHYS_C250_EXIT: usize = 0x00CC_C284;

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

// SP physics `[esi+0xa0]` null-entry guard (`0x00B02255`: `mov ecx, [esi+0xa0]`)
const SP_PHYS_A0_GUARD: usize = 0x00B0_2255;

const SP_PHYS_A0_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0xA0, 0x00, 0x00, 0x00];

const SP_PHYS_A0_CONT: usize = 0x00B0_225E;

const SP_PHYS_A0_EXIT: usize = 0x00B0_2273;

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

// SP physics invalid `[esp+0x18]`/`esi` guard (`0x00B2BD0A`: `mov ecx, [esp+0x18]; mov [esi], eax`)
const SP_PHYS_BD0A_GUARD: usize = 0x00B2_BD0A;

const SP_PHYS_BD0A_PROLOGUE: [u8; 6] = [0x8B, 0x4C, 0x24, 0x18, 0x89, 0x06];

const SP_PHYS_BD0A_CONT: usize = 0x00B2_BD10;

const SP_PHYS_BD0A_SKIP: usize = 0x00B2_BD1F;

// SP entity `[esi+0x190]` null guard (`0x007192EC`: `mov ecx, [esi+0x190]`)
const SP_PHYS_190_GUARD: usize = 0x0071_92EC;

const SP_PHYS_190_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0x90, 0x01, 0x00, 0x00];

const SP_PHYS_190_CONT: usize = 0x0071_92F2;

const SP_PHYS_190_SKIP: usize = 0x0071_9306;

// SP Havok raycast null `this` guard (`0x00ABC970`: `push ebx; mov ebx, esp; sub esp, 8`, `ret 0xc`)
const SP_PHYS_C970_GUARD: usize = 0x00AB_C970;

const SP_PHYS_C970_PROLOGUE: [u8; 6] = [0x53, 0x8B, 0xDC, 0x83, 0xEC, 0x08];

const SP_PHYS_C970_CONT: usize = 0x00AB_C976;

// SP Havok `hkpWorld::removeEntity` unmapped/freed `this` guard (`0x00CB22B0`: `sub esp, 8; push esi; mov esi, ecx`, `ret 4`)
const SP_CB22B0_GUARD: usize = 0x00CB_22B0;

const SP_CB22B0_PROLOGUE: [u8; 6] = [0x83, 0xEC, 0x08, 0x56, 0x8B, 0xF1];

const SP_CB22B0_CONT: usize = 0x00CB_22B6;

// Entity getBounds null-MeshAsset guard (`0x007D49FF`: `mov ecx, [ecx]; mov eax, [ecx+0x44]`)
const ENT_BBOX_MESH_GUARD: usize = 0x007D_49FF;

const ENT_BBOX_MESH_PROLOGUE: [u8; 5] = [0x8B, 0x09, 0x8B, 0x41, 0x44];

const ENT_BBOX_MESH_CONT: usize = 0x007D_4A04;

#[cfg(target_arch = "x86")]
type PartSetupFn = unsafe extern "fastcall" fn(usize, *mut c_void);

#[cfg(target_arch = "x86")]
type PartLookupFn = unsafe extern "fastcall" fn(usize, *mut c_void, u32) -> usize;

#[cfg(target_arch = "x86")]
static ORIG_PART_SETUP: OnceLock<PartSetupFn> = OnceLock::new();

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
unsafe extern "C" fn sp_cb22b0_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "push eax",
        "push ecx",
        "push edx",
        "lea eax, [ecx + 0x84]",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "sub esp, 8",
        "push esi",
        "mov esi, ecx",
        "push {cont}",
        "ret",
        "2:",
        "ret 4",
        bad = sym opfor_is_bad_write_ptr,
        cont = const SP_CB22B0_CONT,
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

pub(super) unsafe fn install(mode: ModeId) -> usize {
    let mut installed = 0usize;
    let is_console_port = matches!(mode, ModeId::Bf1943 | ModeId::Bfbc1);
    let is_bfbc1 = mode == ModeId::Bfbc1;
    #[cfg(target_arch = "x86")]
    {
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
        if !is_console_port {
            return installed;
        }
        if !is_bfbc1 {
            return installed;
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
                SMG_BBOX2_GUARD,
                &SMG_BBOX2_PROLOGUE,
                smg_bbox2_guard_entry as *const c_void,
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
                SP_CB22B0_GUARD,
                &SP_CB22B0_PROLOGUE,
                sp_cb22b0_guard_entry as *const c_void,
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
    }
    let _ = (is_console_port, is_bfbc1);
    installed
}
