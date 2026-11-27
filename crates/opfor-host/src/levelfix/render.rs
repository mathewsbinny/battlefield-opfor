//! `render` subsystem compatibility guards for `opfor-host`.
#![allow(dead_code, non_snake_case, unused_imports)]

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use opfor_vfs::ModeId;

use super::*;

// D3D11 SRV texture-binding guards (prevents d3d11.dll crash when a ported map binds a stale SRV)
const PS_BIND: usize = 0x009B_C68E;

const PS_BIND_PROLOGUE: [u8; 5] = [0x8B, 0x17, 0x8B, 0x52, 0x20];

const VS_BIND: usize = 0x009B_C661;

const VS_BIND_PROLOGUE: [u8; 5] = [0x8B, 0x17, 0x8B, 0x52, 0x64];

// MeshModel::init null-MeshAsset guard (`0x005D2070`: `__thiscall(asset, a1, a2)`)
const MESH_MODEL_INIT: usize = 0x005D_2070;

const MESH_MODEL_INIT_PROLOGUE: [u8; 7] = [0x83, 0x7C, 0x24, 0x0C, 0x01, 0x56, 0x57];

// MeshModel::init missing-resource guard (`0x005D20D4`: `mov eax, [esi]; mov ecx, [eax+0xc]`)
const MESH_MODEL_RES_NULL: usize = 0x005D_20D4;

const MESH_MODEL_RES_NULL_PROLOGUE: [u8; 5] = [0x8B, 0x06, 0x8B, 0x48, 0x0C];

const MESH_MODEL_RES_NULL_CONT: usize = 0x005D_20D9;

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

// Render bucket null-table guard (`0x00970D52`: `xor esi, esi; cmp [edx], esi; mov [esp+0x1c], esi`)
const RENDER_BUCKET_GUARD: usize = 0x0097_0D52;

const RENDER_BUCKET_PROLOGUE: [u8; 8] = [0x33, 0xF6, 0x39, 0x32, 0x89, 0x74, 0x24, 0x1C];

const RENDER_BUCKET_CONT: usize = 0x0097_0D5A;

const RENDER_BUCKET_SKIP: usize = 0x0097_0F66;

// MeshModel::setLod null-MeshAsset guard (`0x005CA970`: `mov eax, [ecx]; mov edx, [eax+0x44]`, `ret 4`)
const MESH_SET_LOD_GUARD: usize = 0x005C_A970;

const MESH_SET_LOD_PROLOGUE: [u8; 5] = [0x8B, 0x01, 0x8B, 0x50, 0x44];

const MESH_SET_LOD_CONT: usize = 0x005C_A975;

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
static mut G_PS_BIND_BACK: usize = 0;

#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
static mut G_VS_BIND_BACK: usize = 0;



#[cfg(target_arch = "x86")]
static ORIG_MESH_MODEL_INIT: OnceLock<LevelListWalkFn> = OnceLock::new();

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
            return installed;
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
                MESH_SET_LOD_GUARD,
                &MESH_SET_LOD_PROLOGUE,
                mesh_set_lod_guard_entry as *const c_void,
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
