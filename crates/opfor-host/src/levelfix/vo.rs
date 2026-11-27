//! `vo` subsystem compatibility guards for `opfor-host`.
#![allow(dead_code, non_snake_case, unused_imports)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use opfor_vfs::ModeId;

use super::*;

// Voice-over walk null-object / null-blueprint-object guard (prevents 0x006EA3F0 crash on WW2 kits)
const VO_WALK: usize = 0x006E_A340;

const VO_WALK_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x24, 0xC6, 0x04, 0x24, 0x00];

const VO_WALK_LIST: usize = 0x006D_BFE0;

const VO_WALK_LIST_PROLOGUE: [u8; 5] = [0x51, 0x8B, 0x4C, 0x24, 0x08];

const IS_KIND_OF: usize = 0x004E_6100;

const REF_OBJECT_DATA_TYPE: usize = 0x01C1_1E64;

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

// Client/Server blueprint-lookup null guard (`0x00717035`: `mov ecx, [eax+0xd8]` -> deref `[ecx+0x50]`)
const BP_LOOKUP_GUARD: usize = 0x0071_7035;

const BP_LOOKUP_GUARD_PROLOGUE: [u8; 6] = [0x8B, 0x88, 0xD8, 0x00, 0x00, 0x00];

const BP_LOOKUP_GUARD_CONT: usize = 0x0071_703B;

const BP_LOOKUP_GUARD_SKIP: usize = 0x0071_705B;

// Post-ingame component init null-arg0 guard (`0x0078F520`: `__thiscall(arg0)`)
const INGAME_COMP_INIT: usize = 0x0078_F520;

const INGAME_COMP_INIT_PROLOGUE: [u8; 7] = [0x56, 0x8B, 0xF1, 0x8B, 0x4C, 0x24, 0x08];

// Post-ingame component init2 null-arg1 guard (`0x0079E840`: `__thiscall(arg0, arg1)`)
const INGAME_COMP_INIT2: usize = 0x0079_E840;

const INGAME_COMP_INIT2_PROLOGUE: [u8; 5] = [0x53, 0x55, 0x56, 0x8B, 0xF1];

// Post-ingame component update null-field10 guard (`0x0078F5C0`: `__thiscall(arg0)`, `[esi+0x10]`)
const INGAME_COMP_UPDATE: usize = 0x0078_F5C0;

const INGAME_COMP_UPDATE_PROLOGUE: [u8; 6] = [0x56, 0x8B, 0xF1, 0x8B, 0x4E, 0x10];

// Post-ingame component bool query null-field10 guard (`0x0078F620`: `mov ecx, [ecx+0x10]; mov eax, [ecx]`)
const INGAME_COMP_QUERY: usize = 0x0078_F620;

const INGAME_COMP_QUERY_PROLOGUE: [u8; 5] = [0x8B, 0x49, 0x10, 0x8B, 0x01];

const INGAME_COMP_QUERY_CONT: usize = 0x0078_F625;

// VO_WALK array-item null-blueprint guard (`0x006E05B5`: `mov ecx, [esi]; mov edx, [ecx+0x50]`)
const VO_WALK_ARR_GUARD: usize = 0x006E_05B5;

const VO_WALK_ARR_PROLOGUE: [u8; 5] = [0x8B, 0x0E, 0x8B, 0x51, 0x50];

const VO_WALK_ARR_CONT: usize = 0x006E_05BA;

const VO_WALK_ARR_SKIP: usize = 0x006E_05C5;

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

// Component attach null-arg0 guard (`0x007991C0`: `push ebx; push esi; mov esi, ecx; mov ecx, [esp+0xc]`, `ret 0x10`)
const COMP_ATTACH_NULL_GUARD: usize = 0x0079_91C0;

const COMP_ATTACH_NULL_PROLOGUE: [u8; 8] = [0x53, 0x56, 0x8B, 0xF1, 0x8B, 0x4C, 0x24, 0x0C];

const COMP_ATTACH_NULL_CONT: usize = 0x0079_91C8;

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

// SP `[ebx+0x14]` null vtable guard (`0x006EC021`: `mov ecx, [ebx+0x14]; mov edx, [ecx]`)
const SP_TERR_B14_GUARD: usize = 0x006E_C021;

const SP_TERR_B14_PROLOGUE: [u8; 5] = [0x8B, 0x4B, 0x14, 0x8B, 0x11];

const SP_TERR_B14_CONT: usize = 0x006E_C026;

const SP_TERR_B14_SKIP: usize = 0x006E_C0E9;

// SP entity `[esi+0x44]` null flag-write guard (`0x00554A85`)
const SP_ENT44_FLAG_GUARD: usize = 0x0055_4A85;

const SP_ENT44_FLAG_PROLOGUE: [u8; 10] = [0x8B, 0x46, 0x44, 0x81, 0x48, 0x10, 0x00, 0x00, 0x00, 0x02];

const SP_ENT44_FLAG_CONT: usize = 0x0055_4A8F;

const SP_ENT44_FLAG_SKIP: usize = 0x0055_4A92;

// SP AI manager #7 guard (`0x00E94AAD`: `mov eax, [eax+0x18]; movss xmm0, [eax+0x38]`)
const SP_AI_MGR7_GUARD: usize = 0x00E9_4AAD;

const SP_AI_MGR7_PROLOGUE: [u8; 8] = [0x8B, 0x40, 0x18, 0xF3, 0x0F, 0x10, 0x40, 0x38];

const SP_AI_MGR7_CONT: usize = 0x00E9_4AB5;

// SP entity `[esi+0x44]` null vtable+0xc0 guard (`0x0053E37C`)
const SP_ENT44_C0_GUARD: usize = 0x0053_E37C;

const SP_ENT44_C0_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x44, 0x8B, 0x01];

const SP_ENT44_C0_CONT: usize = 0x0053_E381;

const SP_ENT44_C0_SKIP: usize = 0x0053_E3C6;

// SP AI manager #8 guard (`0x00E52D05`: `mov edi, [eax+0x18]; mov eax, [ebx+0xb00]`)
const SP_AI_MGR8_GUARD: usize = 0x00E5_2D05;

const SP_AI_MGR8_PROLOGUE: [u8; 9] = [0x8B, 0x78, 0x18, 0x8B, 0x83, 0x00, 0x0B, 0x00, 0x00];

const SP_AI_MGR8_CONT: usize = 0x00E5_2D0E;

// SP entity `[esi+0x44]` null flag-write #2 guard (`0x0053E5F7`)
const SP_ENT44_FLAG2_GUARD: usize = 0x0053_E5F7;

const SP_ENT44_FLAG2_PROLOGUE: [u8; 10] = [0x8B, 0x46, 0x44, 0x81, 0x48, 0x10, 0x00, 0x00, 0x00, 0x02];

const SP_ENT44_FLAG2_CONT: usize = 0x0053_E601;

const SP_ENT44_FLAG2_SKIP: usize = 0x0053_E604;

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

// SP entity `[esi+0x44]` null vtable+0xc0 #2 guard (`0x0053E66C`)
const SP_ENT44_C0_B_GUARD: usize = 0x0053_E66C;

const SP_ENT44_C0_B_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x44, 0x8B, 0x01];

const SP_ENT44_C0_B_CONT: usize = 0x0053_E671;

const SP_ENT44_C0_B_SKIP: usize = 0x0053_E6B6;

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

// SP AI manager #13 guard (`0x00F2CDA5`: `mov esi, [ebp+0x38]; push edi; mov edi, [esi+8]`)
const SP_AI_MGR13_GUARD: usize = 0x00F2_CDA5;

const SP_AI_MGR13_PROLOGUE: [u8; 7] = [0x8B, 0x75, 0x38, 0x57, 0x8B, 0x7E, 0x08];

const SP_AI_MGR13_CONT: usize = 0x00F2_CDAC;

const SP_AI_MGR13_SKIP: usize = 0x00F2_CDE4;

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

// SP AI manager #18 guard (`0x00EB9EB5`: `fld dword ptr [ecx+0xc0]`)
const SP_AI_MGR18_GUARD: usize = 0x00EB_9EB5;

const SP_AI_MGR18_PROLOGUE: [u8; 6] = [0xD9, 0x81, 0xC0, 0x00, 0x00, 0x00];

const SP_AI_MGR18_CONT: usize = 0x00EB_9EBB;

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

const SP_NODE_WALK_CONT: usize = 0x004B_CBE7;

// SP AI navmesh query null `[esp+0x24]` guard (`0x00E6B2FB`: `mov ebx, [esp+0x24]; mov edx, [ebx]`)
const SP_AI_EBX24_GUARD: usize = 0x00E6_B2FB;

const SP_AI_EBX24_PROLOGUE: [u8; 6] = [0x8B, 0x5C, 0x24, 0x24, 0x8B, 0x13];

const SP_AI_EBX24_CONT: usize = 0x00E6_B301;

const SP_AI_EBX24_SKIP: usize = 0x00E6_B430;

// SP Frostbite 1.0 PathDataDefinition (.hpd) incompatible navmesh registration skip (`0x00EFB240`, `ret 0x10`)
const SP_AI_PATHDATA_GUARD: usize = 0x00EF_B240;

const SP_AI_PATHDATA_PROLOGUE: [u8; 6] = [0x83, 0xEC, 0x14, 0x56, 0x8B, 0xF1];

// SP in-game AI query null `ecx` guard (`0x00EB8660`: `mov edx, [ecx]; mov eax, [edx+4]`)
const SP_AI_EB86_GUARD: usize = 0x00EB_8660;

const SP_AI_EB86_PROLOGUE: [u8; 5] = [0x8B, 0x11, 0x8B, 0x42, 0x04];

const SP_AI_EB86_CONT: usize = 0x00EB_8665;

const SP_AI_EB86_EXIT: usize = 0x00EB_8794;

// SP in-game AI tick null `[esi+4]` guard (`0x00EBD20A`: `mov ecx, [esi+4]; mov edx, [ecx]`)
const SP_AI_EBD2_GUARD: usize = 0x00EB_D20A;

const SP_AI_EBD2_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x04, 0x8B, 0x11];

const SP_AI_EBD2_CONT: usize = 0x00EB_D20F;

const SP_AI_EBD2_SKIP: usize = 0x00EB_D28C;

// SP audio/emitter invalid `[eax]` ptr guard (`0x00FDA3B8`: `mov eax, [eax]; mov ecx, [eax+0xc]; mov edx, [eax+0x10]`)
const SP_FDA3_GUARD: usize = 0x00FD_A3B8;

const SP_FDA3_PROLOGUE: [u8; 8] = [0x8B, 0x00, 0x8B, 0x48, 0x0C, 0x8B, 0x50, 0x10];

const SP_FDA3_CONT: usize = 0x00FD_A3C0;

// SP slot-0 null fallback guard (`0x006B697B`: `mov eax, [eax+0x14]; cmp eax, ebp`)
const SP_6B697B_GUARD: usize = 0x006B_697B;

const SP_6B697B_PROLOGUE: [u8; 5] = [0x8B, 0x40, 0x14, 0x3B, 0xC5];

const SP_6B697B_CONT: usize = 0x006B_6980;

const SP_6B697B_SKIP: usize = 0x006B_69F4;

// SP physics `[edi+0x44]` null guards (`0x00B025D1` & `0x00B025EE`)
const SP_B025D1_GUARD: usize = 0x00B0_25D1;

const SP_B025D1_PROLOGUE: [u8; 8] = [0x8B, 0x4F, 0x44, 0xE8, 0x97, 0xE5, 0x23, 0x00];

const SP_B025D1_CONT: usize = 0x00B0_25D9;

const SP_B025EE_GUARD: usize = 0x00B0_25EE;

const SP_B025EE_PROLOGUE: [u8; 6] = [0x8B, 0x57, 0x44, 0xD9, 0x42, 0x2C];

const SP_B025EE_CONT: usize = 0x00B0_25F4;

const SP_B025EE_SKIP: usize = 0x00B0_2633;

const SP_B02651_GUARD: usize = 0x00B0_2651;

const SP_B02651_PROLOGUE: [u8; 6] = [0x8B, 0x4F, 0x44, 0xD9, 0x41, 0x50];

const SP_B02651_CONT: usize = 0x00B0_2657;

// SP anim init null `[ebx]` guard #0 (`0x00F6C0D1`: `mov eax, [ebx]; cmp [eax+0x54], edi`)
const SP_ANIM_INIT0_GUARD: usize = 0x00F6_C0D1;

const SP_ANIM_INIT0_PROLOGUE: [u8; 5] = [0x8B, 0x03, 0x39, 0x78, 0x54];

const SP_ANIM_INIT0_CONT: usize = 0x00F6_C0D6;

const SP_ANIM_INIT0_SKIP: usize = 0x00F6_C112;

// SP anim null-arg guard (`0x00A4F800`: `push esi; mov esi, [esp+8]; cmp [esi+4], 0`)
const SP_A4F800_GUARD: usize = 0x00A4_F800;

const SP_A4F800_PROLOGUE: [u8; 5] = [0x56, 0x8B, 0x74, 0x24, 0x08];

const SP_A4F800_CONT: usize = 0x00A4_F805;

// Frostbite free() corrupt/unaligned pointer guard (`0x0041B8B0`: `push esi; mov esi, [esp+8]`)
const SP_FREE_41B8B0_GUARD: usize = 0x0041_B8B0;

const SP_FREE_41B8B0_PROLOGUE: [u8; 5] = [0x56, 0x8B, 0x74, 0x24, 0x08];

const SP_FREE_41B8B0_CONT: usize = 0x0041_B8B5;

// Anim resource getter null-this guard (`0x00F36B80`: `mov eax, [ecx+0x10]; ret; int3`)
const SP_F36B80_GUARD: usize = 0x00F3_6B80;

const SP_F36B80_PROLOGUE: [u8; 5] = [0x8B, 0x41, 0x10, 0xC3, 0xCC];

// Sibling anim null-arg guards (`0x00A4EF90` & `0x00A4EFB0`)
const SP_A4EF90_GUARD: usize = 0x00A4_EF90;

const SP_A4EF90_PROLOGUE: [u8; 8] = [0x8B, 0x44, 0x24, 0x04, 0x8B, 0x40, 0x04, 0xC3];

const SP_A4EFB0_GUARD: usize = 0x00A4_EFB0;

const SP_A4EFB0_PROLOGUE: [u8; 7] = [0x8B, 0x54, 0x24, 0x04, 0x8B, 0x4A, 0x24];

const SP_A4EFB0_CONT: usize = 0x00A4_EFB7;

// SP anim init null `eax` guard (`0x00F6C449`: `mov edx, [esi+0x18]; mov ecx, [eax+4]`)
const SP_F6C449_GUARD: usize = 0x00F6_C449;

const SP_F6C449_PROLOGUE: [u8; 6] = [0x8B, 0x56, 0x18, 0x8B, 0x48, 0x04];

const SP_F6C449_CONT: usize = 0x00F6_C44F;

const SP_F6C449_SKIP: usize = 0x00F6_C458;

// StringBuilder bounds-check guards (`0x00507F80`, `0x00507FB0`, `0x00507FD0`) — stops stack buffer overflow
const SP_STRBUF_7F80_GUARD: usize = 0x0050_7F80;

const SP_STRBUF_7F80_PROLOGUE: [u8; 7] = [0x8B, 0x41, 0x04, 0x8A, 0x54, 0x24, 0x04];

const SP_STRBUF_7FB0_GUARD: usize = 0x0050_7FB0;

const SP_STRBUF_7FB0_PROLOGUE: [u8; 6] = [0x8A, 0x54, 0x24, 0x04, 0x8B, 0xC1];

const SP_STRBUF_7FD0_GUARD: usize = 0x0050_7FD0;

const SP_STRBUF_7FD0_PROLOGUE: [u8; 6] = [0x8B, 0xC1, 0x8B, 0x4C, 0x24, 0x04];

// SP anim update null `[ecx]` guard (`0x00F5CC10`: `mov edx, [ecx]; mov ecx, [edx+0x3c]`)
const SP_F5CC10_GUARD: usize = 0x00F5_CC10;

const SP_F5CC10_PROLOGUE: [u8; 5] = [0x8B, 0x11, 0x8B, 0x4A, 0x3C];

const SP_F5CC10_CONT: usize = 0x00F5_CC15;

// SP anim update null `[eax]` guard (`0x00F62BA1`: `mov edx, [eax]; mov eax, [edx+0x38]`)
const SP_F62BA1_GUARD: usize = 0x00F6_2BA1;

const SP_F62BA1_PROLOGUE: [u8; 5] = [0x8B, 0x10, 0x8B, 0x42, 0x38];

const SP_F62BA1_CONT: usize = 0x00F6_2BA6;

const SP_F62BA1_SKIP: usize = 0x00F6_2C5B;

// SP anim update null `[eax]` guard #2 (`0x00F6B52B`: `mov ecx, [eax]; cmp byte [ecx+0x7c], 0`)
const SP_F6B52B_GUARD: usize = 0x00F6_B52B;

const SP_F6B52B_PROLOGUE: [u8; 6] = [0x8B, 0x08, 0x80, 0x79, 0x7C, 0x00];

const SP_F6B52B_CONT: usize = 0x00F6_B531;

const SP_F6B52B_SKIP: usize = 0x00F6_B57F;

// SP anim stat-counter null `eax` guard (`0x00F689F9`: `mov eax, [eax+4]; add [0x18a4bb0], eax`)
const SP_F689F9_GUARD: usize = 0x00F6_89F9;

const SP_F689F9_PROLOGUE: [u8; 9] = [0x8B, 0x40, 0x04, 0x01, 0x05, 0xB0, 0x4B, 0x8A, 0x01];

const SP_F689F9_CONT: usize = 0x00F6_8A02;

const SP_F689F9_SKIP: usize = 0x00F6_8A15;

// SP anim null-arg guard (`0x00A4F020`: `push esi; push edi; mov edi, [esp+0xc]`)
const SP_A4F020_GUARD: usize = 0x00A4_F020;

const SP_A4F020_PROLOGUE: [u8; 6] = [0x56, 0x57, 0x8B, 0x7C, 0x24, 0x0C];

const SP_A4F020_CONT: usize = 0x00A4_F026;

// SP anim null `eax` guard #2 (`0x00F68CC4`: `mov edx, [esp+0x20]; mov ecx, [eax+4]`)
const SP_F68CC4_GUARD: usize = 0x00F6_8CC4;

const SP_F68CC4_PROLOGUE: [u8; 7] = [0x8B, 0x54, 0x24, 0x20, 0x8B, 0x48, 0x04];

const SP_F68CC4_CONT: usize = 0x00F6_8CCB;

const SP_F68CC4_SKIP: usize = 0x00F6_8CD7;

// SP anim resource lookup invalid `[eax]` guard (`0x00F42EEA`: `mov ecx, [eax]; mov edx, [ecx]; mov eax, [edx]`)
const SP_F42EEA_GUARD: usize = 0x00F4_2EEA;

const SP_F42EEA_PROLOGUE: [u8; 6] = [0x8B, 0x08, 0x8B, 0x11, 0x8B, 0x02];

const SP_F42EEA_CONT: usize = 0x00F4_2EF0;

const SP_F42EEA_SKIP: usize = 0x00F4_2F58;

// SP resource lookup corrupt `[esp+0x850]` guard (`0x004B2ED3`: `mov ecx, [esp+0x850]; mov eax, [ecx]`)
const SP_4B2ED3_GUARD: usize = 0x004B_2ED3;

const SP_4B2ED3_PROLOGUE: [u8; 9] = [0x8B, 0x8C, 0x24, 0x50, 0x08, 0x00, 0x00, 0x8B, 0x01];

const SP_4B2ED3_CONT: usize = 0x004B_2EDC;

const SP_4B2ED3_SKIP: usize = 0x004B_2F2A;

// SP anim entry ctor invalid `[eax]` guard (`0x00F68D8C`: `mov ecx, [eax]; mov edx, [ecx]; mov eax, [edx]`)
const SP_F68D8C_GUARD: usize = 0x00F6_8D8C;

const SP_F68D8C_PROLOGUE: [u8; 6] = [0x8B, 0x08, 0x8B, 0x11, 0x8B, 0x02];

const SP_F68D8C_CONT: usize = 0x00F6_8D92;

const SP_F68D8C_SKIP: usize = 0x00F6_8DCE;

// SP anim entry loop null `[ebp]` guard (`0x00F68E90`: `mov edx, [ebp]; mov ecx, [edx+0x14]`)
const SP_F68E90_GUARD: usize = 0x00F6_8E90;

const SP_F68E90_PROLOGUE: [u8; 6] = [0x8B, 0x55, 0x00, 0x8B, 0x4A, 0x14];

const SP_F68E90_CONT: usize = 0x00F6_8E96;

const SP_F68E90_SKIP: usize = 0x00F6_8EB2;

// EaString::append corrupt length guard (`0x00401340`: `mov eax, [esp+4]; sub esp, 8`, `ret 8`)
const SP_EASTR_401340_GUARD: usize = 0x0040_1340;

const SP_EASTR_401340_PROLOGUE: [u8; 7] = [0x8B, 0x44, 0x24, 0x04, 0x83, 0xEC, 0x08];

const SP_EASTR_401340_CONT: usize = 0x0040_1347;

// SP anim entry search loop null `[edx]` guard (`0x00F68D40`: `mov edi, [edx]; cmp [edi+0x14], ebp`)
const SP_F68D40_GUARD: usize = 0x00F6_8D40;

const SP_F68D40_PROLOGUE: [u8; 5] = [0x8B, 0x3A, 0x39, 0x6F, 0x14];

const SP_F68D40_CONT: usize = 0x00F6_8D45;

const SP_F68D40_SKIP: usize = 0x00F6_8D4B;

// SP anim buffer null `[ebx+0x38]` guard (`0x00F691CC`: `mov eax, [ebx+0x38]; mov ecx, [eax+0xc]; mov edx, [eax+0x10]`)
const SP_F691CC_GUARD: usize = 0x00F6_91CC;

const SP_F691CC_PROLOGUE: [u8; 9] = [0x8B, 0x43, 0x38, 0x8B, 0x48, 0x0C, 0x8B, 0x50, 0x10];

const SP_F691CC_CONT: usize = 0x00F6_91D5;

const SP_F691CC_SKIP: usize = 0x00F6_9731;

// SP anim null `[[esi]+0x38]` copy guard (`0x00F5D230`: `mov ecx, [esi]; mov ecx, [ecx+0x38]`)
const SP_F5D230_GUARD: usize = 0x00F5_D230;

const SP_F5D230_PROLOGUE: [u8; 5] = [0x8B, 0x0E, 0x8B, 0x49, 0x38];

const SP_F5D230_CONT: usize = 0x00F5_D235;

const SP_F5D230_SKIP: usize = 0x00F5_D251;

// Frostbite sibling free() corrupt/unaligned pointer guard (`0x0041B7F0`: `push esi; mov esi, [esp+8]`)
const SP_FREE_41B7F0_GUARD: usize = 0x0041_B7F0;

const SP_FREE_41B7F0_PROLOGUE: [u8; 5] = [0x56, 0x8B, 0x74, 0x24, 0x08];

const SP_FREE_41B7F0_CONT: usize = 0x0041_B7F5;

// SP audio/client null `[edi]` vtable guard (`0x01016679`: `mov ecx, [edi]; mov edx, [ecx]; mov edx, [edx+0xc]`)
const SP_1016679_GUARD: usize = 0x0101_6679;

const SP_1016679_PROLOGUE: [u8; 7] = [0x8B, 0x0F, 0x8B, 0x11, 0x8B, 0x52, 0x0C];

const SP_1016679_CONT: usize = 0x0101_6680;

const SP_1016679_EXIT: usize = 0x0101_668B;

// EaString concat corrupt `edi` guard (`0x0045AE5D`: `push edi; mov edi, [esp+0x14]`)
const SP_45AE5D_GUARD: usize = 0x0045_AE5D;

const SP_45AE5D_PROLOGUE: [u8; 5] = [0x57, 0x8B, 0x7C, 0x24, 0x14];

const SP_45AE5D_CONT: usize = 0x0045_AE62;

// Linked-list walk corrupt `edi` guard (`0x00465311`: `cmp [edi+8], ebx; je +6`)
const SP_465311_GUARD: usize = 0x0046_5311;

const SP_465311_PROLOGUE: [u8; 5] = [0x39, 0x5F, 0x08, 0x74, 0x06];

const SP_465311_CONT: usize = 0x0046_5316;

const SP_465311_DONE: usize = 0x0046_531C;

// Client entity init null `edi` guard (`0x008745D6`: `cmp [edi+0x68], 0; je +0x20`)
const SP_8745D6_GUARD: usize = 0x0087_45D6;

const SP_8745D6_PROLOGUE: [u8; 6] = [0x83, 0x7F, 0x68, 0x00, 0x74, 0x20];

const SP_8745D6_CONT: usize = 0x0087_45DC;

const SP_8745D6_ZERO: usize = 0x0087_45FC;

const SP_8745D6_SKIP: usize = 0x0087_462A;

// Client entity null `[ecx+0x80]` guard (`0x006C9E40`: `mov eax, [ecx+0x80]; mov edx, [eax+0x6c]`)
const SP_6C9E40_GUARD: usize = 0x006C_9E40;

const SP_6C9E40_PROLOGUE: [u8; 6] = [0x8B, 0x81, 0x80, 0x00, 0x00, 0x00];

const SP_6C9E40_CONT: usize = 0x006C_9E46;

// Resource lookup corrupt `esi` guard (`0x004B8341`: `mov eax, [esi+8]; push eax; mov ecx, ebp`)
const SP_4B8341_GUARD: usize = 0x004B_8341;

const SP_4B8341_PROLOGUE: [u8; 6] = [0x8B, 0x46, 0x08, 0x50, 0x8B, 0xCD];

const SP_4B8341_CONT: usize = 0x004B_8347;

const SP_4B8341_SKIP: usize = 0x004B_841E;

// Invalid strcat source pointer guard (`0x00490AB0`: `mov eax, [esp+0x630]; mov ecx, eax`)
const SP_490AB0_GUARD: usize = 0x0049_0AB0;

const SP_490AB0_PROLOGUE: [u8; 9] = [0x8B, 0x84, 0x24, 0x30, 0x06, 0x00, 0x00, 0x8B, 0xC8];

const SP_490AB0_CONT: usize = 0x0049_0AB9;

const SP_490AB0_SKIP: usize = 0x0049_0AEA;

// Client entity null `eax` guard (`0x008BE75D`: `mov ecx, [eax+0x9c]`)
const SP_8BE75D_GUARD: usize = 0x008B_E75D;

const SP_8BE75D_PROLOGUE: [u8; 6] = [0x8B, 0x88, 0x9C, 0x00, 0x00, 0x00];

const SP_8BE75D_CONT: usize = 0x008B_E763;

const SP_8BE75D_SKIP: usize = 0x008B_E848;

// SP AI null `ecx` guard (`0x00E6A710`: `mov edx, [ecx+0x18]; mov ecx, [0x1894260]`)
const SP_E6A710_GUARD: usize = 0x00E6_A710;

const SP_E6A710_PROLOGUE: [u8; 9] = [0x8B, 0x51, 0x18, 0x8B, 0x0D, 0x60, 0x42, 0x89, 0x01];

const SP_E6A710_CONT: usize = 0x00E6_A719;

const SP_E6A710_SKIP: usize = 0x00E6_A721;

// SP slot-0 null fallback guard #4 (`0x006C370A`: `test eax, eax; jne +2; mov eax, [ecx]`)
const SP_6C370A_GUARD: usize = 0x006C_370A;

const SP_6C370A_PROLOGUE: [u8; 6] = [0x85, 0xC0, 0x75, 0x02, 0x8B, 0x01];

const SP_6C370A_CONT: usize = 0x006C_3710;

const SP_6C370A_SKIP: usize = 0x006C_378D;

// SP physics getter null `[ecx+0x9c]` guard (`0x00B0380D`: `mov ecx, [ecx+0x9c]`)
const SP_B0380D_GUARD: usize = 0x00B0_380D;

const SP_B0380D_PROLOGUE: [u8; 6] = [0x8B, 0x89, 0x9C, 0x00, 0x00, 0x00];

const SP_B0380D_CONT: usize = 0x00B0_3813;

// SP physics set-pos null `this` guard (`0x00CCC220`: `push ebp; mov ebp, esp; and esp, -16`, `ret 8`)
const SP_CCC220_GUARD: usize = 0x00CC_C220;

const SP_CCC220_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];

const SP_CCC220_CONT: usize = 0x00CC_C226;

// SP physics raycast/probe null `[esi+8]` guard (`0x00B040F8`: `mov eax, [ecx]; mov eax, [eax+0x38]`)
const SP_B040F8_GUARD: usize = 0x00B0_40F8;

const SP_B040F8_PROLOGUE: [u8; 5] = [0x8B, 0x01, 0x8B, 0x40, 0x38];

const SP_B040F8_CONT: usize = 0x00B0_40FD;

const SP_B040F8_SKIP: usize = 0x00B0_4126;

// SP HUD/entity event null `0x6de2a0` return guard (`0x005AFA4A`: `cmp dword ptr [eax+0x14], 0; je 0x5afa77`)
const SP_5AFA4A_GUARD: usize = 0x005A_FA4A;

const SP_5AFA4A_PROLOGUE: [u8; 6] = [0x83, 0x78, 0x14, 0x00, 0x74, 0x27];

const SP_5AFA4A_CONT: usize = 0x005A_FA50;

const SP_5AFA4A_ZERO: usize = 0x005A_FA77;

const SP_5AFA4A_EXIT: usize = 0x005A_FA6E;

// SP component cleanup null `[ecx+4]->[+0x14]` guard (`0x0078FA37`: `mov ecx, [ecx+4]; mov ecx, [ecx+0x14]`)
const SP_78FA37_GUARD: usize = 0x0078_FA37;

const SP_78FA37_PROLOGUE: [u8; 6] = [0x8B, 0x49, 0x04, 0x8B, 0x49, 0x14];

const SP_78FA37_CONT: usize = 0x0078_FA3D;

// SP component cleanup null `[eax+4]->[+0x14]` guard (`0x0078EDC0`: `mov eax, ecx; mov ecx, [eax+4]`)
const SP_78EDC0_GUARD: usize = 0x0078_EDC0;

const SP_78EDC0_PROLOGUE: [u8; 5] = [0x8B, 0xC1, 0x8B, 0x48, 0x04];

const SP_78EDC0_CONT: usize = 0x0078_EDC8;

// SP physics body clearer null `[ecx+0x44]` guard (`0x00AF82B0`)
const SP_AF82B0_GUARD: usize = 0x00AF_82B0;

const SP_AF82B0_PROLOGUE: [u8; 10] = [0x8B, 0x41, 0x44, 0xC7, 0x40, 0x24, 0x00, 0x00, 0x00, 0x00];

// SP physics body destroy null `[edi+8]` guard (`0x00B11870`: `mov eax, [edi+8]; mov ecx, [eax+8]`)
const SP_B11870_GUARD: usize = 0x00B1_1870;

const SP_B11870_PROLOGUE: [u8; 6] = [0x8B, 0x47, 0x08, 0x8B, 0x48, 0x08];

const SP_B11870_CONT: usize = 0x00B1_1876;

const SP_B11870_SKIP: usize = 0x00B1_18A3;

// SP physics body dtor null `[esi+0x40]` / `[esi+0x48]` Havok ref release guards (`0x00B11951` / `0x00B11970`)
const SP_B11951_GUARD: usize = 0x00B1_1951;

const SP_B11951_PROLOGUE: [u8; 5] = [0x66, 0x83, 0x79, 0x04, 0x00];

const SP_B11951_CONT: usize = 0x00B1_1958;

const SP_B11951_SKIP: usize = 0x00B1_196D;

const SP_B11970_GUARD: usize = 0x00B1_1970;

const SP_B11970_PROLOGUE: [u8; 5] = [0x66, 0x83, 0x79, 0x04, 0x00];

const SP_B11970_CONT: usize = 0x00B1_1977;

const SP_B11970_SKIP: usize = 0x00B1_198C;

// SP audio/render teardown null `[esi+0x3e4]` guard (`0x00F2CC2D`: `mov ecx, [esi+0x3e4]`)
const SP_F2CC2D_GUARD: usize = 0x00F2_CC2D;

const SP_F2CC2D_PROLOGUE: [u8; 6] = [0x8B, 0x8E, 0xE4, 0x03, 0x00, 0x00];

const SP_F2CC2D_CONT: usize = 0x00F2_CC33;

const SP_F2CC2D_SKIP: usize = 0x00F2_CC41;

// SP render/audio unregister null `this` guard (`0x00EAE270`: `push ebx; mov ebx, ecx; push edi; mov edi, [ebx+0x64c]`, `ret 4`)
const SP_EAE270_GUARD: usize = 0x00EA_E270;

const SP_EAE270_PROLOGUE: [u8; 10] = [0x53, 0x8B, 0xD9, 0x57, 0x8B, 0xBB, 0x4C, 0x06, 0x00, 0x00];

const SP_EAE270_CONT: usize = 0x00EA_E27A;

// SP UI string getter null `[eax+0x20]` guard (`0x0074D885`: `mov ebx, [eax+4]; mov ebp, [eax]`)
const SP_74D885_GUARD: usize = 0x0074_D885;

const SP_74D885_PROLOGUE: [u8; 5] = [0x8B, 0x58, 0x04, 0x8B, 0x28];

const SP_74D885_CONT: usize = 0x0074_D88A;

const SP_74D885_EXIT: usize = 0x0074_D8B7;

// SP UI Flash HUD null `[eax]->[+0x7c]` guard (`0x00FA8A4E`: `mov eax, [eax]; mov eax, [eax+0x7c]`)
const SP_FA8A4E_GUARD: usize = 0x00FA_8A4E;

const SP_FA8A4E_PROLOGUE: [u8; 5] = [0x8B, 0x00, 0x8B, 0x40, 0x7C];

const SP_FA8A4E_CONT: usize = 0x00FA_8A53;

const SP_FA8A4E_SKIP: usize = 0x00FA_8AFA;

// SP resource/asset stale-vtable virtual call guard (`0x00448FB3`: `mov edx, [esi]; mov eax, [edx+0xc]`)
const SP_448FB3_GUARD: usize = 0x0044_8FB3;

const SP_448FB3_PROLOGUE: [u8; 5] = [0x8B, 0x16, 0x8B, 0x42, 0x0C];

const SP_448FB3_CONT: usize = 0x0044_8FB8;

const SP_448FB3_EXIT: usize = 0x0044_9005;

// SP resource manager null `esi` guard (`0x004B869C`: `mov al, [esi+0x24]; movzx ecx, al`)
const SP_4B869C_GUARD: usize = 0x004B_869C;

const SP_4B869C_PROLOGUE: [u8; 6] = [0x8A, 0x46, 0x24, 0x0F, 0xB6, 0xC8];

const SP_4B869C_CONT: usize = 0x004B_86A2;

const SP_4B869C_SKIP: usize = 0x004B_885B;

// SP ResourceProxy null lookup result guard (`0x00449871`: `mov [esi+4], eax; mov edx, [eax]`)
const SP_449871_GUARD: usize = 0x0044_9871;

const SP_449871_PROLOGUE: [u8; 5] = [0x89, 0x46, 0x04, 0x8B, 0x10];

const SP_449871_CONT: usize = 0x0044_9876;

// SP event-tree unsubscribe null `[ebp+0x1c]` guard (`0x00487BC0`: `mov edi, [ebp+0x1c]; lea edx, [edi+0x10]`)
const SP_487BC0_GUARD: usize = 0x0048_7BC0;

const SP_487BC0_PROLOGUE: [u8; 6] = [0x8B, 0x7D, 0x1C, 0x8D, 0x57, 0x10];

const SP_487BC0_CONT: usize = 0x0048_7BC6;

const SP_487BC0_SKIP: usize = 0x0048_7CA3;

// SP ResourceProxy::release null `[esi+4]` guard (`0x0043D5C5`: `mov ecx, [esi+4]; mov eax, [ecx]`)
const SP_43D5C5_GUARD: usize = 0x0043_D5C5;

const SP_43D5C5_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x04, 0x8B, 0x01];

const SP_43D5C5_CONT: usize = 0x0043_D5CA;

const SP_43D5C5_SKIP: usize = 0x0043_D5CF;

// SP case-insensitive `stricmp` bad-pointer guard (`0x00508220`: `mov edx, [esp+8]; push esi`)
const SP_508220_GUARD: usize = 0x0050_8220;

const SP_508220_PROLOGUE: [u8; 5] = [0x8B, 0x54, 0x24, 0x08, 0x56];

const SP_508220_CONT: usize = 0x0050_8225;

// SP live-sim null `[esi+0x80]` guard (`0x006CA0B0`: `cmp dword ptr [eax+0x1c], 0; je 0x6ca0ea`)
const SP_6CA0B0_GUARD: usize = 0x006C_A0B0;

const SP_6CA0B0_PROLOGUE: [u8; 6] = [0x83, 0x78, 0x1C, 0x00, 0x74, 0x34];

const SP_6CA0B0_CONT: usize = 0x006C_A0B6;

const SP_6CA0B0_SKIP: usize = 0x006C_A0EA;

// SP physics float getter null `[ecx+0x44]`/`[ecx+0x48]` guard (`0x00AF8560`: `sub esp, 8; push ebx; push esi; mov esi, ecx`, returns `float` in `st(0)`)
const SP_AF8560_GUARD: usize = 0x00AF_8560;

const SP_AF8560_PROLOGUE: [u8; 7] = [0x83, 0xEC, 0x08, 0x53, 0x56, 0x8B, 0xF1];

const SP_AF8560_CONT: usize = 0x00AF_8567;

// SP pool_free (`0x0040A3C0`) bad-pointer guard (rejects ASCII/unmapped pointers >= 0x60000000)
const SP_40A3C0_GUARD: usize = 0x0040_A3C0;

const SP_40A3C0_PROLOGUE: [u8; 6] = [0x8B, 0x51, 0x04, 0xF6, 0xC2, 0x02];

const SP_40A3C0_CONT: usize = 0x0040_A3C6;

// SP animation graph node loop null-entry guard (`0x00F67AD6`: `mov ecx, [edx+esi*4]; mov eax, [ecx]`)
const SP_F67AD6_GUARD: usize = 0x00F6_7AD6;

const SP_F67AD6_PROLOGUE: [u8; 5] = [0x8B, 0x0C, 0xB2, 0x8B, 0x01];

const SP_F67AD6_CONT: usize = 0x00F6_7ADB;

const SP_F67AD6_SKIP: usize = 0x00F6_7B25;

// SP physics query null arg2 / `[arg2+0xc8]` guard (`0x00AEC8C0`: `push ebp; mov ebp, esp; and esp, -16`, cdecl bool)
const SP_AEC8C0_GUARD: usize = 0x00AE_C8C0;

const SP_AEC8C0_PROLOGUE: [u8; 6] = [0x55, 0x8B, 0xEC, 0x83, 0xE4, 0xF0];

const SP_AEC8C0_CONT: usize = 0x00AE_C8C6;

// SP ResourceManager sync-wait spin-loop timeout (`0x004B86B3`: `cmp byte [esp+0x15], 0; je 0x4b86c7`)
// Prevents permanent main-thread deadlock when a missing/failed BFBC1 resource never sets `[esi+0x25] & 4`.
const SP_4B86B3_GUARD: usize = 0x004B_86B3;

const SP_4B86B3_PROLOGUE: [u8; 7] = [0x80, 0x7C, 0x24, 0x15, 0x00, 0x74, 0x0D];

const SP_4B86B3_DONE: usize = 0x004B_86C7;

// SP ANT animation asset null-resource guard (`0x00F632DB`: `mov ecx, [eax]; mov ecx, [ecx+8]`)
const SP_F632DB_GUARD: usize = 0x00F6_32DB;

const SP_F632DB_PROLOGUE: [u8; 5] = [0x8B, 0x08, 0x8B, 0x49, 0x08];

const SP_F632DB_CONT: usize = 0x00F6_32E0;

// SP ANT animation loop null-entry guard (`0x00F69B84`: `mov eax, [edi+eax*4]; mov ecx, [eax+8]`)
const SP_F69B84_GUARD: usize = 0x00F6_9B84;

const SP_F69B84_PROLOGUE: [u8; 6] = [0x8B, 0x04, 0x87, 0x8B, 0x48, 0x08];

const SP_F69B84_CONT: usize = 0x00F6_9B8A;

const SP_F69B84_SKIP: usize = 0x00F6_9BD9;

// SP UIIngame Flash-ready fallback (`0x0095666E`: `cmp byte [eax+0x272bc], 0`)
// If a ported BFBC1 HUD SWF never fires BFBC2's ready fscommand within 120 frames (~2s),
// mark UIIngame ready so UIManager stops waiting 1,000,000 frames and dismisses the loading screen.
const SP_95666E_GUARD: usize = 0x0095_666E;

const SP_95666E_PROLOGUE: [u8; 7] = [0x80, 0xB8, 0xBC, 0x72, 0x02, 0x00, 0x00];

const SP_95666E_READY: usize = 0x0095_6677;

const SP_95666E_WAIT: usize = 0x0095_668C;

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

#[cfg(target_arch = "x86")]
type VoWalkFn = unsafe extern "cdecl" fn(*mut c_void, *mut c_void, *mut c_void);

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
static ORIG_VO_WALK: OnceLock<VoWalkFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
static ORIG_VO_WALK_LIST: OnceLock<VoWalkFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
static ORIG_LEVEL_LIST_WALK: OnceLock<LevelListWalkFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
static ORIG_RES_PROXY_LOAD: OnceLock<ResProxyLoadFn> = OnceLock::new();

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
type SoundNodeUpdateFn = unsafe extern "fastcall" fn(usize, *mut c_void, *const c_void) -> u8;

#[cfg(target_arch = "x86")]
static ORIG_SOUND_NODE_UPDATE: OnceLock<SoundNodeUpdateFn> = OnceLock::new();

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
unsafe extern "C" fn sp_6b697b_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [eax + 0x14]",
        "cmp eax, ebp",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_6B697B_CONT,
        skip = const SP_6B697B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b025d1_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [edi + 0x44]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [ecx + 0x10]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "push {cont}",
        "ret",
        cont = const SP_B025D1_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b025ee_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [edi + 0x44]",
        "cmp edx, 0x10000",
        "jb 2f",
        "fld dword ptr [edx + 0x2c]",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi + 0x70], 0",
        "mov dword ptr [esi + 0x74], 0",
        "mov byte ptr [esi + 0x8c], 0",
        "mov byte ptr [esi + 0x8e], 0",
        "mov byte ptr [esi + 0x8f], 0",
        "mov byte ptr [esi + 0x90], 0",
        "push {skip}",
        "ret",
        cont = const SP_B025EE_CONT,
        skip = const SP_B025EE_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b02651_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [edi + 0x44]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "fld dword ptr [ecx + 0x50]",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi + 0x84], 0",
        "mov dword ptr [esi + 0x88], 0",
        "mov byte ptr [esi + 0x8d], 0",
        "pop edi",
        "pop esi",
        "mov esp, ebp",
        "pop ebp",
        "ret 4",
        cont = const SP_B02651_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_anim_init0_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ebx]",
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp dword ptr [eax + 0x54], edi",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_ANIM_INIT0_CONT,
        skip = const SP_ANIM_INIT0_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_a4f800_guard_entry() {
    std::arch::naked_asm!(
        "push esi",
        "mov esi, dword ptr [esp + 8]",
        "cmp esi, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "pop esi",
        "ret",
        cont = const SP_A4F800_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_free_41b8b0_guard_entry() {
    std::arch::naked_asm!(
        "push esi",
        "mov esi, dword ptr [esp + 8]",
        "test esi, esi",
        "jz 2f",
        "test esi, 3",
        "jnz 2f",
        "cmp esi, 0x00400000",
        "jb 2f",
        "cmp esi, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "push esi",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "push {cont}",
        "ret",
        "2:",
        "pop esi",
        "ret",
        bad = sym opfor_is_bad_write_ptr,
        cont = const SP_FREE_41B8B0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f36b80_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [ecx + 0x10]",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_a4ef90_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 4]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [eax + 4]",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_a4efb0_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [esp + 4]",
        "cmp edx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [edx + 0x24]",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
        cont = const SP_A4EFB0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f6c449_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [esi + 0x18]",
        "mov ecx, dword ptr [eax + 4]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F6C449_CONT,
        skip = const SP_F6C449_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_strbuf_7f80_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 4]",
        "mov edx, dword ptr [ecx + 8]",
        "dec edx",
        "cmp eax, edx",
        "jae 2f",
        "mov dl, byte ptr [esp + 4]",
        "mov byte ptr [eax], dl",
        "add dword ptr [ecx + 4], 1",
        "2:",
        "ret 4",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_strbuf_7fb0_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, ecx",
        "mov ecx, dword ptr [eax + 4]",
        "mov edx, dword ptr [eax + 8]",
        "dec edx",
        "cmp ecx, edx",
        "jae 2f",
        "mov dl, byte ptr [esp + 4]",
        "mov byte ptr [ecx], dl",
        "add ecx, 1",
        "mov dword ptr [eax + 4], ecx",
        "mov byte ptr [ecx], 0",
        "2:",
        "ret 4",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_strbuf_7fd0_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, ecx",
        "mov ecx, dword ptr [esp + 4]",
        "cmp ecx, 0x10000",
        "jb 3f",
        "push edi",
        "push ebx",
        "mov edi, dword ptr [eax + 4]",
        "mov ebx, dword ptr [eax + 8]",
        "dec ebx",
        "1:",
        "mov dl, byte ptr [ecx]",
        "test dl, dl",
        "jz 2f",
        "cmp edi, ebx",
        "jae 2f",
        "mov byte ptr [edi], dl",
        "add edi, 1",
        "add ecx, 1",
        "jmp 1b",
        "2:",
        "mov dword ptr [eax + 4], edi",
        "cmp edi, dword ptr [eax + 8]",
        "jae 4f",
        "mov byte ptr [edi], 0",
        "4:",
        "pop ebx",
        "pop edi",
        "3:",
        "ret 4",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f5cc10_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [ecx]",
        "cmp edx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [edx + 0x3c]",
        "push {cont}",
        "ret",
        "2:",
        "pop esi",
        "ret 4",
        cont = const SP_F5CC10_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f62ba1_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [eax]",
        "cmp edx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [edx + 0x38]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F62BA1_CONT,
        skip = const SP_F62BA1_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f6b52b_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [eax]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "cmp byte ptr [ecx + 0x7c], 0",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F6B52B_CONT,
        skip = const SP_F6B52B_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f689f9_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [eax + 4]",
        "add dword ptr [0x018a4bb0], eax",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F689F9_CONT,
        skip = const SP_F689F9_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_a4f020_guard_entry() {
    std::arch::naked_asm!(
        "push esi",
        "push edi",
        "mov edi, dword ptr [esp + 0xc]",
        "cmp edi, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop esi",
        "ret",
        cont = const SP_A4F020_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f68cc4_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [esp + 0x20]",
        "mov ecx, dword ptr [eax + 4]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F68CC4_CONT,
        skip = const SP_F68CC4_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f42eea_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [eax]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [ecx]",
        "cmp edx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [edx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F42EEA_CONT,
        skip = const SP_F42EEA_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4b2ed3_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esp + 0x850]",
        "test ecx, 3",
        "jnz 2f",
        "cmp ecx, 0x10000",
        "jb 2f",
        "cmp ecx, 0x7fff0000",
        "jae 2f",
        "cmp ecx, 0x60000000",
        "jb 3f",
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
        "jnz 2f",
        "3:",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_4B2ED3_CONT,
        skip = const SP_4B2ED3_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f68d8c_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [eax]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [ecx]",
        "cmp edx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [edx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F68D8C_CONT,
        skip = const SP_F68D8C_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f68e90_guard_entry() {
    std::arch::naked_asm!(
        "mov edx, dword ptr [ebp]",
        "cmp edx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [edx + 0x14]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F68E90_CONT,
        skip = const SP_F68E90_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_eastr_401340_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 4]",
        "mov edx, dword ptr [esp + 8]",
        "cmp eax, 0x10000",
        "jb 2f",
        "sub edx, eax",
        "cmp edx, 0x00100000",
        "ja 2f",
        "sub esp, 8",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, ecx",
        "ret 8",
        cont = const SP_EASTR_401340_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f68d40_guard_entry() {
    std::arch::naked_asm!(
        "mov edi, dword ptr [edx]",
        "cmp edi, 0x10000",
        "jb 2f",
        "cmp dword ptr [edi + 0x14], ebp",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F68D40_CONT,
        skip = const SP_F68D40_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f691cc_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ebx + 0x38]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [eax + 0xc]",
        "mov edx, dword ptr [eax + 0x10]",
        "push {cont}",
        "ret",
        "2:",
        "xor ebx, ebx",
        "mov dword ptr [ebp + 0x4c], ebx",
        "mov dword ptr [ebp + 0x50], ebx",
        "push {skip}",
        "ret",
        cont = const SP_F691CC_CONT,
        skip = const SP_F691CC_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f5d230_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [ecx + 0x38]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F5D230_CONT,
        skip = const SP_F5D230_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_free_41b7f0_guard_entry() {
    std::arch::naked_asm!(
        "push esi",
        "mov esi, dword ptr [esp + 8]",
        "test esi, esi",
        "jz 2f",
        "test esi, 3",
        "jnz 2f",
        "cmp esi, 0x00400000",
        "jb 2f",
        "cmp esi, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "push esi",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "push {cont}",
        "ret",
        "2:",
        "pop esi",
        "ret",
        bad = sym opfor_is_bad_write_ptr,
        cont = const SP_FREE_41B7F0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_1016679_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [edi]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [ecx]",
        "cmp edx, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [edx + 0xc]",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_1016679_CONT,
        exit = const SP_1016679_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_45ae5d_guard_entry() {
    std::arch::naked_asm!(
        "push edi",
        "mov edi, dword ptr [esp + 0x14]",
        "test edi, 3",
        "jnz 2f",
        "cmp edi, 0x10000",
        "jb 2f",
        "cmp edi, 0x7fff0000",
        "jae 2f",
        "cmp edi, 0x60000000",
        "jb 3f",
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
        "jnz 2f",
        "3:",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi], 0",
        "mov dword ptr [esi + 4], 0",
        "mov dword ptr [esi + 8], 0",
        "pop edi",
        "mov eax, esi",
        "pop esi",
        "pop ebp",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_45AE5D_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_465311_guard_entry() {
    std::arch::naked_asm!(
        "test edi, 3",
        "jnz 2f",
        "cmp edi, 0x10000",
        "jb 2f",
        "cmp edi, 0x7fff0000",
        "jae 2f",
        "cmp edi, 0x60000000",
        "jb 3f",
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
        "jnz 2f",
        "3:",
        "cmp dword ptr [edi + 8], ebx",
        "je 4f",
        "push {cont}",
        "ret",
        "2:",
        "mov edi, esi",
        "4:",
        "push {done}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_465311_CONT,
        done = const SP_465311_DONE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_8745d6_guard_entry() {
    std::arch::naked_asm!(
        "cmp edi, 0x10000",
        "jb 2f",
        "cmp dword ptr [edi + 0x68], 0",
        "je 3f",
        "push {cont}",
        "ret",
        "3:",
        "push {zero}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_8745D6_CONT,
        zero = const SP_8745D6_ZERO,
        skip = const SP_8745D6_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_6c9e40_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [ecx + 0x80]",
        "cmp eax, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [ecx + 0x88], 0",
        "ret",
        cont = const SP_6C9E40_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4b8341_guard_entry() {
    std::arch::naked_asm!(
        "test esi, 3",
        "jnz 2f",
        "cmp esi, 0x10000",
        "jb 2f",
        "cmp esi, 0x7fff0000",
        "jae 2f",
        "cmp esi, 0x60000000",
        "jb 3f",
        "push eax",
        "push ecx",
        "push edx",
        "push esi",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "pop ecx",
        "pop eax",
        "jnz 2f",
        "3:",
        "mov eax, dword ptr [esi + 8]",
        "push eax",
        "mov ecx, ebp",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_4B8341_CONT,
        skip = const SP_4B8341_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_490ab0_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 0x630]",
        "cmp eax, 0x00400000",
        "jb 2f",
        "cmp eax, 0x7fff0000",
        "jae 2f",
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
        "jnz 2f",
        "mov ecx, eax",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_490AB0_CONT,
        skip = const SP_490AB0_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_8be75d_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [eax + 0x9c]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_8BE75D_CONT,
        skip = const SP_8BE75D_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_e6a710_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [ecx + 0x18]",
        "mov ecx, dword ptr [0x01894260]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_E6A710_CONT,
        skip = const SP_E6A710_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_6c370a_guard_entry() {
    std::arch::naked_asm!(
        "test eax, eax",
        "jnz 3f",
        "mov eax, dword ptr [ecx]",
        "3:",
        "cmp eax, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_6C370A_CONT,
        skip = const SP_6C370A_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b0380d_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [ecx + 0x9c]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "cmp eax, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "mov edx, dword ptr [esp + 8]",
        "mov dword ptr [edx], 0",
        "mov edx, dword ptr [esp + 0xc]",
        "mov dword ptr [edx], 0",
        "mov edx, dword ptr [esp + 0x10]",
        "mov dword ptr [edx], 0",
        "mov edx, dword ptr [esp + 0x14]",
        "mov dword ptr [edx], 0",
        "ret 0x14",
        cont = const SP_B0380D_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_ccc220_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "push ebp",
        "mov ebp, esp",
        "and esp, -16",
        "push {cont}",
        "ret",
        "2:",
        "ret 8",
        cont = const SP_CCC220_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b040f8_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [ecx]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [eax + 0x38]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_B040F8_CONT,
        skip = const SP_B040F8_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_5afa4a_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp dword ptr [eax + 0x14], 0",
        "je 3f",
        "push {cont}",
        "ret",
        "3:",
        "push {zero}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_5AFA4A_CONT,
        zero = const SP_5AFA4A_ZERO,
        exit = const SP_5AFA4A_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_78fa37_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [ecx + 4]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [ecx + 0x14]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_78FA37_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_78edc0_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, ecx",
        "mov ecx, dword ptr [eax + 4]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [ecx + 0x14]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "ret",
        cont = const SP_78EDC0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_af82b0_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [ecx + 0x44]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov dword ptr [eax + 0x24], 0",
        "2:",
        "ret 4",
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b11870_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [edi + 8]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [eax + 8]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_B11870_CONT,
        skip = const SP_B11870_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b11951_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "cmp word ptr [ecx + 4], 0",
        "je 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_B11951_CONT,
        skip = const SP_B11951_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_b11970_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "cmp word ptr [ecx + 4], 0",
        "je 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_B11970_CONT,
        skip = const SP_B11970_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f2cc2d_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 0x3e4]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F2CC2D_CONT,
        skip = const SP_F2CC2D_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_eae270_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "push ebx",
        "mov ebx, ecx",
        "push edi",
        "mov edi, dword ptr [ebx + 0x64c]",
        "push {cont}",
        "ret",
        "2:",
        "ret 4",
        cont = const SP_EAE270_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_74d885_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ebx, dword ptr [eax + 4]",
        "mov ebp, dword ptr [eax]",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_74D885_CONT,
        exit = const SP_74D885_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_fa8a4e_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [eax]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [eax + 0x7c]",
        "cmp eax, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_FA8A4E_CONT,
        skip = const SP_FA8A4E_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_448fb3_guard_entry() {
    std::arch::naked_asm!(
        "cmp esi, 0x10000",
        "jb 2f",
        "mov edx, dword ptr [esi]",
        "cmp edx, 0x01400000",
        "jb 2f",
        "cmp edx, 0x01800000",
        "jae 2f",
        "mov eax, dword ptr [edx + 0xc]",
        "cmp eax, 0x00401000",
        "jb 2f",
        "cmp eax, 0x01400000",
        "jae 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {exit}",
        "ret",
        cont = const SP_448FB3_CONT,
        exit = const SP_448FB3_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4b869c_guard_entry() {
    std::arch::naked_asm!(
        "cmp esi, 0x10000",
        "jb 2f",
        "mov al, byte ptr [esi + 0x24]",
        "movzx ecx, al",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_4B869C_CONT,
        skip = const SP_4B869C_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_449871_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "mov dword ptr [esi + 4], eax",
        "mov edx, dword ptr [eax]",
        "push {cont}",
        "ret",
        "2:",
        "mov dword ptr [esi], 0",
        "mov dword ptr [esi + 4], 0x014F267C",
        "pop edi",
        "pop esi",
        "pop ebx",
        "ret 0x14",
        cont = const SP_449871_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_487bc0_guard_entry() {
    std::arch::naked_asm!(
        "mov edi, dword ptr [ebp + 0x1c]",
        "cmp edi, 0x10000",
        "jb 2f",
        "lea edx, [edi + 0x10]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_487BC0_CONT,
        skip = const SP_487BC0_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_43d5c5_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [esi + 4]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_43D5C5_CONT,
        skip = const SP_43D5C5_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_508220_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 4]",
        "mov edx, dword ptr [esp + 8]",
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jb 3f",
        "push edx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "jnz 2f",
        "3:",
        "cmp edx, 0x10000",
        "jb 4f",
        "cmp edx, 0x60000000",
        "jb 5f",
        "push edx",
        "push edx",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop edx",
        "jnz 4f",
        "5:",
        "push esi",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, -1",
        "ret",
        "4:",
        "mov eax, 1",
        "ret",
        bad = sym opfor_is_bad_ptr,
        cont = const SP_508220_CONT,
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
        "test eax, eax",
        "jz 2f",
        "test eax, 3",
        "jnz 2f",
        "cmp eax, 0x00400000",
        "jb 2f",
        "cmp eax, 0x7fff0000",
        "jae 2f",
        "cmp eax, 0x60000000",
        "jb 3f",
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
        "jnz 2f",
        "3:",
        "push {cont}",
        "ret",
        "2:",
        "mov eax, 1",
        "ret",
        bad = sym opfor_is_bad_ptr,
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
        "mov esi, dword ptr [esp + 0x10]",
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
unsafe extern "C" fn sp_6ca0b0_guard_entry() {
    std::arch::naked_asm!(
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp dword ptr [eax + 0x1c], 0",
        "je 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_6CA0B0_CONT,
        skip = const SP_6CA0B0_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_af8560_guard_entry() {
    std::arch::naked_asm!(
        "cmp ecx, 0x10000",
        "jb 2f",
        "cmp dword ptr [ecx + 0x44], 0x10000",
        "jb 2f",
        "cmp dword ptr [ecx + 0x48], 0x10000",
        "jb 2f",
        "sub esp, 8",
        "push ebx",
        "push esi",
        "mov esi, ecx",
        "push {cont}",
        "ret",
        "2:",
        "fldz",
        "ret",
        cont = const SP_AF8560_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_40a3c0_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 4]",
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp eax, 0x60000000",
        "jb 3f",
        "push ecx",
        "push eax",
        "call {bad}",
        "add esp, 4",
        "test eax, eax",
        "pop ecx",
        "jnz 2f",
        "3:",
        "mov edx, dword ptr [ecx + 4]",
        "test dl, 2",
        "push {cont}",
        "ret",
        "2:",
        "ret 4",
        bad = sym opfor_is_bad_write_ptr,
        cont = const SP_40A3C0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f67ad6_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [edx + esi*4]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov eax, dword ptr [ecx]",
        "push {cont}",
        "ret",
        "2:",
        "mov ecx, dword ptr [edi]",
        "mov dword ptr [ecx + esi*4], 0",
        "sub esp, 0xc",
        "push {skip}",
        "ret",
        cont = const SP_F67AD6_CONT,
        skip = const SP_F67AD6_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_aec8c0_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [esp + 8]",
        "cmp eax, 0x10000",
        "jb 2f",
        "cmp dword ptr [eax + 0xc8], 0x10000",
        "jb 2f",
        "push ebp",
        "mov ebp, esp",
        "and esp, 0xfffffff0",
        "push {cont}",
        "ret",
        "2:",
        "xor eax, eax",
        "ret",
        cont = const SP_AEC8C0_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_4b86b3_guard_entry() {
    std::arch::naked_asm!(
        "cmp byte ptr [esp + 0x15], 0",
        "je 3f",
        "push ebx",
        "mov ebx, 15",
        "2:",
        "mov eax, 0x00509140",
        "call eax",
        "test byte ptr [esi + 0x25], 4",
        "jnz 4f",
        "dec ebx",
        "jnz 2b",
        "4:",
        "pop ebx",
        "3:",
        "push {done}",
        "ret",
        done = const SP_4B86B3_DONE,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f632db_guard_entry() {
    std::arch::naked_asm!(
        "mov ecx, dword ptr [eax]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [ecx + 8]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "pop edi",
        "pop esi",
        "xor eax, eax",
        "add esp, 0x1c",
        "ret",
        cont = const SP_F632DB_CONT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_f69b84_guard_entry() {
    std::arch::naked_asm!(
        "mov eax, dword ptr [edi + eax*4]",
        "cmp eax, 0x10000",
        "jb 2f",
        "mov ecx, dword ptr [eax + 8]",
        "cmp ecx, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        "push {skip}",
        "ret",
        cont = const SP_F69B84_CONT,
        skip = const SP_F69B84_SKIP,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_95666e_guard_entry() {
    std::arch::naked_asm!(
        "cmp byte ptr [eax + 0x272bc], 0",
        "jne 3f",
        "cmp dword ptr [esi + 0xc3eb4], 0xF41C8",
        "ja 2f",
        "mov byte ptr [eax + 0x272bc], 1",
        "3:",
        "push {ready}",
        "ret",
        "2:",
        "push {wait}",
        "ret",
        ready = const SP_95666E_READY,
        wait = const SP_95666E_WAIT,
    );
}

pub(super) unsafe fn install(mode: ModeId) -> usize {
    let mut installed = 0usize;
    let is_console_port = matches!(mode, ModeId::Bf1943 | ModeId::Bfbc1);
    let is_bfbc1 = mode == ModeId::Bfbc1;
    #[cfg(target_arch = "x86")]
    {
        if !is_console_port {
            return installed;
        }
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
        if !is_bfbc1 {
            return installed;
        }
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
                SP_6B697B_GUARD,
                &SP_6B697B_PROLOGUE,
                sp_6b697b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B025D1_GUARD,
                &SP_B025D1_PROLOGUE,
                sp_b025d1_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B025EE_GUARD,
                &SP_B025EE_PROLOGUE,
                sp_b025ee_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B02651_GUARD,
                &SP_B02651_PROLOGUE,
                sp_b02651_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_ANIM_INIT0_GUARD,
                &SP_ANIM_INIT0_PROLOGUE,
                sp_anim_init0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_A4F800_GUARD,
                &SP_A4F800_PROLOGUE,
                sp_a4f800_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_FREE_41B8B0_GUARD,
                &SP_FREE_41B8B0_PROLOGUE,
                sp_free_41b8b0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F36B80_GUARD,
                &SP_F36B80_PROLOGUE,
                sp_f36b80_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_A4EF90_GUARD,
                &SP_A4EF90_PROLOGUE,
                sp_a4ef90_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_A4EFB0_GUARD,
                &SP_A4EFB0_PROLOGUE,
                sp_a4efb0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F6C449_GUARD,
                &SP_F6C449_PROLOGUE,
                sp_f6c449_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_STRBUF_7F80_GUARD,
                &SP_STRBUF_7F80_PROLOGUE,
                sp_strbuf_7f80_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_STRBUF_7FB0_GUARD,
                &SP_STRBUF_7FB0_PROLOGUE,
                sp_strbuf_7fb0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_STRBUF_7FD0_GUARD,
                &SP_STRBUF_7FD0_PROLOGUE,
                sp_strbuf_7fd0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F5CC10_GUARD,
                &SP_F5CC10_PROLOGUE,
                sp_f5cc10_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F62BA1_GUARD,
                &SP_F62BA1_PROLOGUE,
                sp_f62ba1_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F6B52B_GUARD,
                &SP_F6B52B_PROLOGUE,
                sp_f6b52b_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F689F9_GUARD,
                &SP_F689F9_PROLOGUE,
                sp_f689f9_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_A4F020_GUARD,
                &SP_A4F020_PROLOGUE,
                sp_a4f020_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F68CC4_GUARD,
                &SP_F68CC4_PROLOGUE,
                sp_f68cc4_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F42EEA_GUARD,
                &SP_F42EEA_PROLOGUE,
                sp_f42eea_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4B2ED3_GUARD,
                &SP_4B2ED3_PROLOGUE,
                sp_4b2ed3_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F68D8C_GUARD,
                &SP_F68D8C_PROLOGUE,
                sp_f68d8c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F68E90_GUARD,
                &SP_F68E90_PROLOGUE,
                sp_f68e90_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_EASTR_401340_GUARD,
                &SP_EASTR_401340_PROLOGUE,
                sp_eastr_401340_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F68D40_GUARD,
                &SP_F68D40_PROLOGUE,
                sp_f68d40_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F691CC_GUARD,
                &SP_F691CC_PROLOGUE,
                sp_f691cc_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F5D230_GUARD,
                &SP_F5D230_PROLOGUE,
                sp_f5d230_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_FREE_41B7F0_GUARD,
                &SP_FREE_41B7F0_PROLOGUE,
                sp_free_41b7f0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_1016679_GUARD,
                &SP_1016679_PROLOGUE,
                sp_1016679_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_45AE5D_GUARD,
                &SP_45AE5D_PROLOGUE,
                sp_45ae5d_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_465311_GUARD,
                &SP_465311_PROLOGUE,
                sp_465311_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_8745D6_GUARD,
                &SP_8745D6_PROLOGUE,
                sp_8745d6_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_6C9E40_GUARD,
                &SP_6C9E40_PROLOGUE,
                sp_6c9e40_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4B8341_GUARD,
                &SP_4B8341_PROLOGUE,
                sp_4b8341_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_490AB0_GUARD,
                &SP_490AB0_PROLOGUE,
                sp_490ab0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_8BE75D_GUARD,
                &SP_8BE75D_PROLOGUE,
                sp_8be75d_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_6C370A_GUARD,
                &SP_6C370A_PROLOGUE,
                sp_6c370a_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B0380D_GUARD,
                &SP_B0380D_PROLOGUE,
                sp_b0380d_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_CCC220_GUARD,
                &SP_CCC220_PROLOGUE,
                sp_ccc220_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B040F8_GUARD,
                &SP_B040F8_PROLOGUE,
                sp_b040f8_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_5AFA4A_GUARD,
                &SP_5AFA4A_PROLOGUE,
                sp_5afa4a_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_78FA37_GUARD,
                &SP_78FA37_PROLOGUE,
                sp_78fa37_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_78EDC0_GUARD,
                &SP_78EDC0_PROLOGUE,
                sp_78edc0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AF82B0_GUARD,
                &SP_AF82B0_PROLOGUE,
                sp_af82b0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B11870_GUARD,
                &SP_B11870_PROLOGUE,
                sp_b11870_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B11951_GUARD,
                &SP_B11951_PROLOGUE,
                sp_b11951_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_B11970_GUARD,
                &SP_B11970_PROLOGUE,
                sp_b11970_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F2CC2D_GUARD,
                &SP_F2CC2D_PROLOGUE,
                sp_f2cc2d_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_EAE270_GUARD,
                &SP_EAE270_PROLOGUE,
                sp_eae270_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_74D885_GUARD,
                &SP_74D885_PROLOGUE,
                sp_74d885_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_FA8A4E_GUARD,
                &SP_FA8A4E_PROLOGUE,
                sp_fa8a4e_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_448FB3_GUARD,
                &SP_448FB3_PROLOGUE,
                sp_448fb3_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4B869C_GUARD,
                &SP_4B869C_PROLOGUE,
                sp_4b869c_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_449871_GUARD,
                &SP_449871_PROLOGUE,
                sp_449871_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_487BC0_GUARD,
                &SP_487BC0_PROLOGUE,
                sp_487bc0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_43D5C5_GUARD,
                &SP_43D5C5_PROLOGUE,
                sp_43d5c5_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_508220_GUARD,
                &SP_508220_PROLOGUE,
                sp_508220_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_6CA0B0_GUARD,
                &SP_6CA0B0_PROLOGUE,
                sp_6ca0b0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AF8560_GUARD,
                &SP_AF8560_PROLOGUE,
                sp_af8560_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_40A3C0_GUARD,
                &SP_40A3C0_PROLOGUE,
                sp_40a3c0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F67AD6_GUARD,
                &SP_F67AD6_PROLOGUE,
                sp_f67ad6_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_AEC8C0_GUARD,
                &SP_AEC8C0_PROLOGUE,
                sp_aec8c0_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_4B86B3_GUARD,
                &SP_4B86B3_PROLOGUE,
                sp_4b86b3_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F632DB_GUARD,
                &SP_F632DB_PROLOGUE,
                sp_f632db_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_F69B84_GUARD,
                &SP_F69B84_PROLOGUE,
                sp_f69b84_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_95666E_GUARD,
                &SP_95666E_PROLOGUE,
                sp_95666e_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_E6A710_GUARD,
                &SP_E6A710_PROLOGUE,
                sp_e6a710_guard_entry as *const c_void,
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
    }
    let _ = (is_console_port, is_bfbc1);
    installed
}
