//! `net` subsystem compatibility guards for `opfor-host`.
#![allow(dead_code, non_snake_case, unused_imports)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use opfor_vfs::ModeId;

use super::*;

// Client entity-factory ReferenceObjectData null-blueprint guard (`0x0082E515`: `mov eax, [ebx+0x78]; mov ecx, [eax+0x50]`)
const CLIENT_REF_OBJ_FACT: usize = 0x0082_E515;

const CLIENT_REF_OBJ_FACT_PROLOGUE: [u8; 6] = [0x8B, 0x43, 0x78, 0x8B, 0x48, 0x50];

const CLIENT_REF_OBJ_FACT_CONT: usize = 0x0082_E51B;

const CLIENT_REF_OBJ_FACT_SKIP: usize = 0x0082_E53F;

// Client tick null-field10 guard (`0x007B44A7`: `mov ecx, [esi+0x10]; mov edx, [ecx]`)
const CLIENT_TICK_GUARD: usize = 0x007B_44A7;

const CLIENT_TICK_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x10, 0x8B, 0x11];

const CLIENT_TICK_CONT: usize = 0x007B_44AC;

const CLIENT_TICK_SKIP: usize = 0x007B_44B5;

// Client tick null-mesh guard (`0x007EA79E`: `mov ecx, [ebx]; cmp dword ptr [ecx+0xc], 1`)
const CLIENT_MESH_GUARD: usize = 0x007E_A79E;

const CLIENT_MESH_PROLOGUE: [u8; 6] = [0x8B, 0x0B, 0x83, 0x79, 0x0C, 0x01];

const CLIENT_MESH_CONT: usize = 0x007E_A7A4;

const CLIENT_MESH_SKIP: usize = 0x007E_A906;

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

// Entity update null-owner/data guard (`0x006E6D20`: `push ecx; push esi; mov esi, ecx`, `ret`)
const ENT_UPD_DATA_GUARD: usize = 0x006E_6D20;

const ENT_UPD_DATA_PROLOGUE: [u8; 8] = [0x51, 0x56, 0x8B, 0xF1, 0x83, 0x7E, 0x7C, 0x00];

const ENT_UPD_DATA_CONT: usize = 0x006E_6D28;

// Client entity init null-field10 guard (`0x00832938`: `mov edx, [esi+0x10]; mov ecx, [edx+0x40]`)
const CLIENT_ENT_F10_GUARD: usize = 0x0083_2938;

const CLIENT_ENT_F10_PROLOGUE: [u8; 6] = [0x8B, 0x56, 0x10, 0x8B, 0x4A, 0x40];

const CLIENT_ENT_F10_CONT: usize = 0x0083_293E;

const CLIENT_ENT_F10_SKIP: usize = 0x0083_294C;

// Entity tick null-field2c guard (`0x006B8BD0`: `push ebx; push esi; mov esi, ecx; mov ecx, [esi+0x2c]`, `ret`)
const ENT_TICK2C_NULL_GUARD: usize = 0x006B_8BD0;

const ENT_TICK2C_NULL_PROLOGUE: [u8; 7] = [0x53, 0x56, 0x8B, 0xF1, 0x8B, 0x4E, 0x2C];

const ENT_TICK2C_NULL_CONT: usize = 0x006B_8BD7;

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

// Entity component update2 null-field08 guard (`0x007993BA`: `mov ecx, [esi+8]; mov edx, [ecx]`)
const ENT_COMP08_B_GUARD: usize = 0x0079_93BA;

const ENT_COMP08_B_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x08, 0x8B, 0x11];

const ENT_COMP08_B_CONT: usize = 0x0079_93BF;

const ENT_COMP08_B_SKIP: usize = 0x0079_93C8;

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

// Entity component query2/3 null-field3c guards (`0x007BEF04` & `0x007E0255`)
const ENT_COMP3C_B_GUARD: usize = 0x007B_EF04;

const ENT_COMP3C_B_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x3C, 0x8B, 0x01];

const ENT_COMP3C_B_CONT: usize = 0x007B_EF09;

const ENT_COMP3C_B_SKIP: usize = 0x007B_EF19;

const ENT_COMP3C_C_GUARD: usize = 0x007E_0255;

const ENT_COMP3C_C_PROLOGUE: [u8; 5] = [0x8B, 0x4E, 0x3C, 0x8B, 0x01];

const ENT_COMP3C_C_CONT: usize = 0x007E_025A;

const ENT_COMP3C_C_SKIP: usize = 0x007E_02C1;

// SP null `edi` deref (`0x0072BA21`: `mov edi, eax; mov eax, [edi+0x3c]`)
const SP_ENT_EDI3C_GUARD: usize = 0x0072_BA21;

const SP_ENT_EDI3C_PROLOGUE: [u8; 5] = [0x8B, 0xF8, 0x8B, 0x47, 0x3C];

const SP_ENT_EDI3C_CONT: usize = 0x0072_BA26;

const SP_ENT_EDI3C_SKIP: usize = 0x0072_BAB4;

// SP slot-0 null fallback guard #2 (`0x0069F302`: `test esi, esi; jne +2; mov esi, [ecx]`)
const SP_69F302_GUARD: usize = 0x0069_F302;

const SP_69F302_PROLOGUE: [u8; 6] = [0x85, 0xF6, 0x75, 0x02, 0x8B, 0x31];

const SP_69F302_CONT: usize = 0x0069_F308;

const SP_69F302_EXIT: usize = 0x0069_F34A;

// SP slot-0 null fallback guard #3 (`0x0069F256`: `test edi, edi; jne +2; mov edi, [ecx]`)
const SP_69F256_GUARD: usize = 0x0069_F256;

const SP_69F256_PROLOGUE: [u8; 6] = [0x85, 0xFF, 0x75, 0x02, 0x8B, 0x39];

const SP_69F256_CONT: usize = 0x0069_F25C;

const SP_69F256_EXIT: usize = 0x0069_F2AF;

// SP local-loopback packet deserializer (`0x00BEDF50`): log & suppress protocol-error disconnect on ported BFBC1 ghosts
const SP_PACKET_PROC: usize = 0x00BE_DF50;

const SP_PACKET_PROC_PROLOGUE: [u8; 6] = [0x53, 0x8B, 0x5C, 0x24, 0x08, 0x55];

// Entity component sub-object null-field14 guards (`0x0078EDA0` & `0x0078FA0D`)
const ENT_COMP14_GUARD: usize = 0x0078_EDA0;

const ENT_COMP14_PROLOGUE: [u8; 5] = [0x8B, 0xC1, 0x8B, 0x48, 0x04];

const ENT_COMP14_CONT: usize = 0x0078_EDA5;

const ENT_COMP14_B_GUARD: usize = 0x0078_FA0D;

const ENT_COMP14_B_PROLOGUE: [u8; 5] = [0x8B, 0x46, 0x08, 0x8B, 0x11];

const ENT_COMP14_B_CONT: usize = 0x0078_FA12;

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
unsafe extern "C" fn sp_69f302_guard_entry() {
    std::arch::naked_asm!(
        "test esi, esi",
        "jnz 3f",
        "mov esi, dword ptr [ecx]",
        "3:",
        "cmp esi, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        // ClSoldier::readState still reads 1 bit (0x7F03D0) + 3 bits (0x50BF20);
        // write 4 zero bits to bs (edi) via WriteBitStream::writeBits (0x0050D5D0)
        // so the ghost bitstream stays synchronized.
        "push 4",
        "push 0",
        "mov ecx, edi",
        "mov eax, 0x0050D5D0",
        "call eax",
        "push {exit}",
        "ret",
        cont = const SP_69F302_CONT,
        exit = const SP_69F302_EXIT,
    );
}

#[cfg(target_arch = "x86")]
#[unsafe(naked)]
unsafe extern "C" fn sp_69f256_guard_entry() {
    std::arch::naked_asm!(
        "test edi, edi",
        "jnz 3f",
        "mov edi, dword ptr [ecx]",
        "3:",
        "cmp edi, 0x10000",
        "jb 2f",
        "push {cont}",
        "ret",
        "2:",
        // ClSoldier::readState still reads 5 bits (0x8026D0 when state==0) + 1 bit (0x50BE90);
        // write 6 zero bits to bs (esi) via WriteBitStream::writeBits (0x0050D5D0)
        // so the ghost bitstream stays synchronized.
        "push 6",
        "push 0",
        "mov ecx, esi",
        "mov eax, 0x0050D5D0",
        "call eax",
        "push {exit}",
        "ret",
        cont = const SP_69F256_CONT,
        exit = const SP_69F256_EXIT,
    );
}

#[cfg(target_arch = "x86")]
type PacketProcFn = unsafe extern "fastcall" fn(usize, *mut c_void, usize) -> u8;

#[cfg(target_arch = "x86")]
static ORIG_PACKET_PROC: OnceLock<PacketProcFn> = OnceLock::new();

#[cfg(target_arch = "x86")]
unsafe extern "fastcall" fn detour_packet_proc(this: usize, edx: *mut c_void, pkt: usize) -> u8 {
    let ok = if let Some(&orig) = ORIG_PACKET_PROC.get() {
        unsafe { orig(this, edx, pkt) }
    } else {
        1
    };
    if ok == 0 {
        static FAIL_LOGS: AtomicUsize = AtomicUsize::new(0);
        if FAIL_LOGS.fetch_add(1, Ordering::Relaxed) < 8 {
            let bits = if pkt >= 0x10000 {
                unsafe { *((pkt + 8) as *const u32) }
            } else {
                0
            };
            let stream = if this >= 0x10000 {
                unsafe { *((this + 0xc) as *const usize) }
            } else {
                0
            };
            let stream_vt = if stream >= 0x10000 {
                unsafe { *(stream as *const usize) }
            } else {
                0
            };
            crate::log_line(&format!(
                "levelfix: packet_proc failed (bits={bits} stream=0x{stream:x} vt=0x{stream_vt:x}) -> keeping loopback alive"
            ));
        }
        return 1;
    }
    ok
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
        if !is_bfbc1 {
            return installed;
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
                CLIENT_MESH_GUARD,
                &CLIENT_MESH_PROLOGUE,
                client_mesh_guard_entry as *const c_void,
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
                SP_69F302_GUARD,
                &SP_69F302_PROLOGUE,
                sp_69f302_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if unsafe {
            crate::hooks::install_detour(
                SP_69F256_GUARD,
                &SP_69F256_PROLOGUE,
                sp_69f256_guard_entry as *const c_void,
            )
        }
        .is_some()
        {
            installed += 1;
        }
        if let Some(tramp) = unsafe {
            crate::hooks::install_detour(
                SP_PACKET_PROC,
                &SP_PACKET_PROC_PROLOGUE,
                detour_packet_proc as *const c_void,
            )
        } {
            let _ = ORIG_PACKET_PROC.set(unsafe { std::mem::transmute(tramp) });
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
    let _ = (is_console_port, is_bfbc1);
    installed
}
