//! In-Game DirectX 11 `IDXGISwapChain::Present` Vtable Hook & `WndProc` Subclass.
//!
//! Blits our 1280x720 GDI+ `MenuCanvas` (`menu_canvas.rs`) onto Frostbite's live DX11
//! backbuffer whenever `ui_state::is_custom_menu_active()` is true, and captures mouse/
//! keyboard input cleanly via `WndProc` while passing 100% of input through during gameplay.

#![allow(non_snake_case, dead_code)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::menu_canvas::{CANVAS_H, CANVAS_W, HitRect, MenuCanvas, MenuClickAction, RENDER_H, RENDER_W};
use crate::ui_state::{Bf1943Modal, Bf1943Screen, PACIFIC_MAPS, UI_STATE};
use crate::win32::{self, HWND, LPARAM, WPARAM};

const GWL_WNDPROC: i32 = -4;
#[repr(C)]
struct BitmapInfoHeader {
    biSize: u32,
    biWidth: i32,
    biHeight: i32,
    biPlanes: u16,
    biBitCount: u16,
    biCompression: u32,
    biSizeImage: u32,
    biXPelsPerMeter: i32,
    biYPelsPerMeter: i32,
    biClrUsed: u32,
    biClrImportant: u32,
}

#[repr(C)]
struct IconInfo {
    fIcon: i32,
    xHotspot: u32,
    yHotspot: u32,
    hbmMask: *mut c_void,
    hbmColor: *mut c_void,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn LoadCursorW(hInstance: *mut c_void, lpCursorName: *const u16) -> *mut c_void;
    fn SetCursor(hCursor: *mut c_void) -> *mut c_void;
    fn CreateIconIndirect(piconinfo: *const IconInfo) -> *mut c_void;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateDIBSection(
        hdc: *mut c_void,
        pbmi: *const BitmapInfoHeader,
        usage: u32,
        ppvBits: *mut *mut c_void,
        hSection: *mut c_void,
        offset: u32,
    ) -> *mut c_void;
    fn CreateBitmap(
        nWidth: i32,
        nHeight: i32,
        nPlanes: u32,
        nBitCount: u32,
        lpBits: *const c_void,
    ) -> *mut c_void;
    fn DeleteObject(ho: *mut c_void) -> i32;
}

/// Build a crisp 32x32 hardware Win32 `HCURSOR` reskinned after BFBC2's angular tactical cursor
/// (sharp beveled chevron arrow with dark border, light upper facet, era body fill, and tail notch).
unsafe fn build_tactical_cursor(border: u32, fill: u32, bevel: u32, accent: u32) -> *mut c_void {
    const W: i32 = 32;
    const H: i32 = 32;
    let bmi = BitmapInfoHeader {
        biSize: std::mem::size_of::<BitmapInfoHeader>() as u32,
        biWidth: W,
        biHeight: -H, // top-down DIB
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0, // BI_RGB
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let hbm_color = unsafe {
        CreateDIBSection(
            std::ptr::null_mut(),
            &bmi,
            0,
            &mut bits,
            std::ptr::null_mut(),
            0,
        )
    };
    if hbm_color.is_null() || bits.is_null() {
        return unsafe { LoadCursorW(std::ptr::null_mut(), 32512usize as *const u16) };
    }

    // Angular BFBC2-style tactical pointer polygon (hotspot at (1, 1)):
    // Tip (1,1) -> right wing (17,15) -> inner notch (10,15) -> stem right (14,23)
    // -> stem bottom (11,24) -> stem left (7,16) -> left wing (1,21) -> Tip (1,1).
    let poly: &[(f32, f32)] = &[
        (1.0, 1.0),
        (17.0, 15.0),
        (10.5, 15.0),
        (14.0, 23.0),
        (11.0, 24.5),
        (7.2, 16.5),
        (1.0, 21.0),
    ];
    let inside = |px: f32, py: f32| -> bool {
        let mut c = false;
        let mut j = poly.len() - 1;
        for i in 0..poly.len() {
            let (xi, yi) = poly[i];
            let (xj, yj) = poly[j];
            if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
                c = !c;
            }
            j = i;
        }
        c
    };

    let pixels = unsafe { std::slice::from_raw_parts_mut(bits as *mut u32, (W * H) as usize) };
    pixels.fill(0);
    for y in 0..H {
        for x in 0..W {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            if inside(fx, fy) {
                // Dark 1px outline if any cardinal neighbor is outside
                let on_edge = !inside(fx - 1.1, fy)
                    || !inside(fx + 1.1, fy)
                    || !inside(fx, fy - 1.1)
                    || !inside(fx, fy + 1.1);
                let col = if on_edge {
                    border
                } else if y >= 17 && y <= 23 && x >= 8 && x <= 13 {
                    // Signature tactical accent stripe on the stem
                    accent
                } else if (x as f32) < (y as f32 * 0.45 + 3.0) {
                    // Upper-left crisp metallic bevel
                    bevel
                } else {
                    fill
                };
                pixels[(y * W + x) as usize] = col;
            }
        }
    }

    let mask_bits = [0u8; (W * H / 8) as usize];
    let hbm_mask = unsafe { CreateBitmap(W, H, 1, 1, mask_bits.as_ptr().cast()) };
    let info = IconInfo {
        fIcon: 0, // cursor
        xHotspot: 1,
        yHotspot: 1,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };
    let hcur = unsafe { CreateIconIndirect(&info) };
    unsafe {
        if !hbm_mask.is_null() {
            DeleteObject(hbm_mask);
        }
        DeleteObject(hbm_color);
    }
    if hcur.is_null() {
        unsafe { LoadCursorW(std::ptr::null_mut(), 32512usize as *const u16) }
    } else {
        hcur
    }
}

/// Cached era-reskinned tactical cursor (`BFBC1` gold / `BF1943` Pacific cream).
fn era_tactical_cursor(mode: opfor_vfs::ModeId) -> *mut c_void {
    static BFBC1_CUR: OnceLock<usize> = OnceLock::new();
    static BF1943_CUR: OnceLock<usize> = OnceLock::new();
    let slot = match mode {
        opfor_vfs::ModeId::Bfbc1 => &BFBC1_CUR,
        _ => &BF1943_CUR,
    };
    *slot.get_or_init(|| unsafe {
        match mode {
            // BFBC1: dark olive outline, Bad Company gold fill, pale gold bevel, amber-orange notch
            opfor_vfs::ModeId::Bfbc1 => {
                build_tactical_cursor(0xFF14_1510, 0xFFE8_C868, 0xFFFF_F0B8, 0xFFF5_9B1A) as usize
            }
            // BF1943: dark ink outline, Pacific parchment cream fill, warm ivory bevel, brass notch
            _ => build_tactical_cursor(0xFF1A_1812, 0xFFF2_E6C8, 0xFFFF_F8E6, 0xFFC8_9B3C) as usize,
        }
    }) as *mut c_void
}

const WM_SETCURSOR: u32 = 0x0020;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_INPUT: u32 = 0x00FF;
const WM_KEYDOWN: u32 = 0x0100;
const WM_CHAR: u32 = 0x0102;
const VK_LEFT: usize = 0x25;
const VK_UP: usize = 0x26;
const VK_RIGHT: usize = 0x27;
const VK_DOWN: usize = 0x28;
const VK_RETURN: usize = 0x0D;
const VK_ESCAPE: usize = 0x1B;
const VK_BACK: usize = 0x08;

const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;
const DXGI_USAGE_RENDER_TARGET_OUTPUT: u32 = 0x20;
const D3D_DRIVER_TYPE_HARDWARE: u32 = 1;
const D3D11_SDK_VERSION: u32 = 7;
const D3D11_BIND_SHADER_RESOURCE: u32 = 0x8;
const D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST: u32 = 4;

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct Guid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

const IID_ID3D11_DEVICE: Guid = Guid {
    d1: 0xDB6F_6DDB,
    d2: 0xAC77,
    d3: 0x4E88,
    d4: [0x82, 0x53, 0x81, 0x9D, 0xF9, 0xBB, 0xF1, 0x40],
};
const IID_ID3D11_TEXTURE2D: Guid = Guid {
    d1: 0x6F15_AAF2,
    d2: 0xD208,
    d3: 0x4E89,
    d4: [0x9A, 0xB4, 0x48, 0x95, 0x35, 0xD3, 0x4F, 0x9C],
};

#[repr(C)]
#[derive(Default)]
struct DxgiModeDesc {
    width: u32,
    height: u32,
    refresh_num: u32,
    refresh_den: u32,
    format: u32,
    scanline: u32,
    scaling: u32,
}

#[repr(C)]
#[derive(Default)]
struct DxgiSampleDesc {
    count: u32,
    quality: u32,
}

#[repr(C)]
struct DxgiSwapChainDesc {
    buffer_desc: DxgiModeDesc,
    sample_desc: DxgiSampleDesc,
    buffer_usage: u32,
    buffer_count: u32,
    output_window: HWND,
    windowed: i32,
    swap_effect: u32,
    flags: u32,
}

#[repr(C)]
struct D3d11Tex2dDesc {
    width: u32,
    height: u32,
    mip_levels: u32,
    array_size: u32,
    format: u32,
    sample_desc: DxgiSampleDesc,
    usage: u32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
}

#[repr(C)]
struct D3d11Viewport {
    top_left_x: f32,
    top_left_y: f32,
    width: f32,
    height: f32,
    min_depth: f32,
    max_depth: f32,
}

#[repr(C)]
struct D3d11SamplerDesc {
    filter: u32,
    addr_u: u32,
    addr_v: u32,
    addr_w: u32,
    mip_lod_bias: f32,
    max_aniso: u32,
    comp_func: u32,
    border_color: [f32; 4],
    min_lod: f32,
    max_lod: f32,
}

type WndProcFn = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> isize;
type PresentFn = unsafe extern "system" fn(*mut c_void, u32, u32) -> i32;

#[link(name = "user32")]
unsafe extern "system" {
    fn SetWindowLongW(hwnd: HWND, idx: i32, new_val: isize) -> isize;
    fn CallWindowProcW(prev: WndProcFn, hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> isize;
    fn GetClientRect(hwnd: HWND, rect: *mut Rect) -> i32;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: *mut c_void,
        inst: *mut c_void,
        param: *mut c_void,
    ) -> HWND;
    fn DestroyWindow(hwnd: HWND) -> i32;
}

static OPFOR_ROOT: OnceLock<PathBuf> = OnceLock::new();
static GAME_HWND: AtomicUsize = AtomicUsize::new(0);
static ORIG_WNDPROC: OnceLock<WndProcFn> = OnceLock::new();
static ORIG_PRESENT: OnceLock<PresentFn> = OnceLock::new();
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static CANVAS_DIRTY: AtomicBool = AtomicBool::new(true);

static CANVAS: Mutex<Option<MenuCanvas>> = Mutex::new(None);
static HIT_RECTS: Mutex<Vec<HitRect>> = Mutex::new(Vec::new());
static GPU_STATE: Mutex<Option<Dx11Blitter>> = Mutex::new(None);

struct Dx11Blitter {
    device: *mut c_void,
    context: *mut c_void,
    texture: *mut c_void,
    srv: *mut c_void,
    vs: *mut c_void,
    ps: *mut c_void,
    sampler: *mut c_void,
}

unsafe impl Send for Dx11Blitter {}

const HLSL_SHADER: &[u8] = br#"
Texture2D tex0 : register(t0);
SamplerState samp0 : register(s0);
struct VSOut { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };
VSOut VSMain(uint id : SV_VertexID) {
    VSOut o;
    float2 p = float2((id == 1) ? 3.0 : -1.0, (id == 2) ? -3.0 : 1.0);
    o.pos = float4(p, 0.0, 1.0);
    o.uv = float2((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return o;
}
float4 PSMain(VSOut i) : SV_TARGET {
    return tex0.Sample(samp0, i.uv);
}
"#;

static SFX_CACHE: OnceLock<HashMap<&'static str, Vec<u8>>> = OnceLock::new();
static ANIM_CLOCK: OnceLock<Instant> = OnceLock::new();
static LAST_RENDER: Mutex<Option<Instant>> = Mutex::new(None);

#[link(name = "winmm")]
unsafe extern "system" {
    fn PlaySoundW(pszSound: *const u16, hmod: *mut c_void, fdwSound: u32) -> i32;
}

const SND_ASYNC: u32 = 0x0001;
const SND_NODEFAULT: u32 = 0x0002;
const SND_MEMORY: u32 = 0x0004;

pub fn play_ui_sfx(name: &str) {
    if let Some(cache) = SFX_CACHE.get() {
        if let Some(bytes) = cache.get(name) {
            unsafe {
                PlaySoundW(
                    bytes.as_ptr() as *const u16,
                    std::ptr::null_mut(),
                    SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
                );
            }
        }
    }
}

pub fn init(opfor_root: &PathBuf) {
    let _ = OPFOR_ROOT.set(opfor_root.clone());
    let _ = ANIM_CLOCK.get_or_init(Instant::now);
    let sfx_dir = opfor_root.join("modules").join("bf1943").join("sfx");
    let mut map = HashMap::new();
    for fname in [
        "ui_scroll.wav",
        "ui_forward.wav",
        "ui_backwards.wav",
        "ui_tick.wav",
    ] {
        if let Ok(bytes) = std::fs::read(sfx_dir.join(fname)) {
            map.insert(fname, bytes);
        }
    }
    let _ = SFX_CACHE.set(map);
    if let Ok(mut g) = CANVAS.lock() {
        *g = Some(MenuCanvas::new(opfor_root));
    }
}

pub fn mark_dirty() {
    CANVAS_DIRTY.store(true, Ordering::Release);
}

/// Called from `imports.rs` when Frostbite creates a top-level game window.
pub unsafe fn on_game_window_created(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    if GAME_HWND
        .compare_exchange(0, hwnd as usize, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let prev = unsafe { SetWindowLongW(hwnd, GWL_WNDPROC, hook_wndproc as *const () as isize) };
        if prev != 0 {
            let _ = ORIG_WNDPROC.set(unsafe { std::mem::transmute::<isize, WndProcFn>(prev) });
            crate::controller::register_raw_input(hwnd);
            crate::log_line(&format!("dx_overlay: subclassed game WndProc on hwnd={hwnd:p}"));
        }
        unsafe { install_dx11_present_hook(hwnd) };
    }
}

unsafe fn com_vtable(obj: *mut c_void) -> *const usize {
    unsafe { *(obj as *const *const usize) }
}

unsafe fn com_release(obj: *mut c_void) {
    if !obj.is_null() {
        type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
        let f: ReleaseFn = unsafe { std::mem::transmute(*com_vtable(obj).add(2)) };
        unsafe { f(obj) };
    }
}

/// Discover the `dxgi.dll` `IDXGISwapChain::Present` COM vtable slot using a temporary
/// D3D11 swapchain and patch slot `8` directly (zero fragile disassembly).
unsafe fn install_dx11_present_hook(_game_hwnd: HWND) {
    if HOOK_INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    let d3d11 = unsafe { win32::LoadLibraryA(b"d3d11.dll\0".as_ptr()) };
    if d3d11.is_null() {
        crate::log_line("dx_overlay: d3d11.dll unavailable");
        return;
    }
    let proc = unsafe { win32::GetProcAddress(d3d11, b"D3D11CreateDeviceAndSwapChain\0".as_ptr()) };
    if proc.is_null() {
        return;
    }
    // Use an isolated hidden dummy window so DXGI never resizes or interferes with the game's real HWND!
    let cls = win32::to_wide_nul("STATIC");
    let title = win32::to_wide_nul("OPFOR_DXGI_PROBE");
    let dummy_hwnd = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            16,
            16,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if dummy_hwnd.is_null() {
        return;
    }
    type CreateDevScFn = unsafe extern "system" fn(
        *mut c_void,
        u32,
        *mut c_void,
        u32,
        *const u32,
        u32,
        u32,
        *const DxgiSwapChainDesc,
        *mut *mut c_void,
        *mut *mut c_void,
        *mut u32,
        *mut *mut c_void,
    ) -> i32;
    let create: CreateDevScFn = unsafe { std::mem::transmute(proc) };

    let desc = DxgiSwapChainDesc {
        buffer_desc: DxgiModeDesc {
            width: 16,
            height: 16,
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ..Default::default()
        },
        sample_desc: DxgiSampleDesc { count: 1, quality: 0 },
        buffer_usage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        buffer_count: 1,
        output_window: dummy_hwnd,
        windowed: 1,
        swap_effect: 0,
        flags: 0,
    };
    let mut sc = std::ptr::null_mut();
    let mut dev = std::ptr::null_mut();
    let mut ctx = std::ptr::null_mut();
    let hr = unsafe {
        create(
            std::ptr::null_mut(),
            D3D_DRIVER_TYPE_HARDWARE,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            0,
            D3D11_SDK_VERSION,
            &desc,
            &mut sc,
            &mut dev,
            std::ptr::null_mut(),
            &mut ctx,
        )
    };
    if hr < 0 || sc.is_null() {
        unsafe { DestroyWindow(dummy_hwnd) };
        crate::log_line(&format!(
            "dx_overlay: dummy D3D11CreateDeviceAndSwapChain failed (hr=0x{hr:08X})"
        ));
        return;
    }

    let vtbl = unsafe { com_vtable(sc) };
    let present_slot = unsafe { vtbl.add(8) } as usize;
    let orig_addr = unsafe { *(present_slot as *const usize) };
    let _ = ORIG_PRESENT.set(unsafe { std::mem::transmute::<usize, PresentFn>(orig_addr) });
    let detour_bytes = (detour_present as *const () as usize).to_le_bytes();
    if unsafe { crate::hooks::write_code(present_slot, &detour_bytes) } {
        crate::log_line(&format!(
            "dx_overlay: hooked IDXGISwapChain::Present vtable[8] @ 0x{present_slot:08X} (orig=0x{orig_addr:08X})"
        ));
    }

    unsafe {
        com_release(ctx);
        com_release(dev);
        com_release(sc);
        DestroyWindow(dummy_hwnd);
    }
}

unsafe fn compile_hlsl(entry: &[u8], target: &[u8]) -> Option<*mut c_void> {
    let comp_dll = unsafe { win32::LoadLibraryA(b"D3DCompiler_42.dll\0".as_ptr()) };
    let comp_dll = if comp_dll.is_null() {
        unsafe { win32::LoadLibraryA(b"d3dcompiler_47.dll\0".as_ptr()) }
    } else {
        comp_dll
    };
    if comp_dll.is_null() {
        return None;
    }
    let proc = unsafe { win32::GetProcAddress(comp_dll, b"D3DCompile\0".as_ptr()) };
    if proc.is_null() {
        return None;
    }
    type D3dCompileFn = unsafe extern "system" fn(
        *const u8,
        usize,
        *const u8,
        *const c_void,
        *const c_void,
        *const u8,
        *const u8,
        u32,
        u32,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> i32;
    let compile: D3dCompileFn = unsafe { std::mem::transmute(proc) };
    let mut blob = std::ptr::null_mut();
    let mut err = std::ptr::null_mut();
    let hr = unsafe {
        compile(
            HLSL_SHADER.as_ptr(),
            HLSL_SHADER.len(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            entry.as_ptr(),
            target.as_ptr(),
            0,
            0,
            &mut blob,
            &mut err,
        )
    };
    if !err.is_null() {
        unsafe { com_release(err) };
    }
    if hr < 0 || blob.is_null() {
        return None;
    }
    Some(blob)
}

unsafe fn blob_ptr_len(blob: *mut c_void) -> (*const u8, usize) {
    type GetPtrFn = unsafe extern "system" fn(*mut c_void) -> *const u8;
    type GetSizeFn = unsafe extern "system" fn(*mut c_void) -> usize;
    let vtbl = unsafe { com_vtable(blob) };
    let get_ptr: GetPtrFn = unsafe { std::mem::transmute(*vtbl.add(3)) };
    let get_sz: GetSizeFn = unsafe { std::mem::transmute(*vtbl.add(4)) };
    unsafe { (get_ptr(blob), get_sz(blob)) }
}

unsafe fn ensure_blitter(swapchain: *mut c_void) -> bool {
    let Ok(mut guard) = GPU_STATE.lock() else {
        return false;
    };
    if guard.is_some() {
        return true;
    }
    // IDXGISwapChain::GetDevice = vtable[7]
    type GetDevFn = unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32;
    let get_dev: GetDevFn = unsafe { std::mem::transmute(*com_vtable(swapchain).add(7)) };
    let mut dev = std::ptr::null_mut();
    if unsafe { get_dev(swapchain, &IID_ID3D11_DEVICE, &mut dev) } < 0 || dev.is_null() {
        return false;
    }

    // ID3D11Device::GetImmediateContext = vtable[40]
    type GetCtxFn = unsafe extern "system" fn(*mut c_void, *mut *mut c_void);
    let dvtbl = unsafe { com_vtable(dev) };
    let get_ctx: GetCtxFn = unsafe { std::mem::transmute(*dvtbl.add(40)) };
    let mut ctx = std::ptr::null_mut();
    unsafe { get_ctx(dev, &mut ctx) };
    if ctx.is_null() {
        unsafe { com_release(dev) };
        return false;
    }

    // CreateTexture2D = vtable[5]
    type CreateTexFn = unsafe extern "system" fn(*mut c_void, *const D3d11Tex2dDesc, *const c_void, *mut *mut c_void) -> i32;
    let create_tex: CreateTexFn = unsafe { std::mem::transmute(*dvtbl.add(5)) };
    let tdesc = D3d11Tex2dDesc {
        width: RENDER_W,
        height: RENDER_H,
        mip_levels: 1,
        array_size: 1,
        format: DXGI_FORMAT_B8G8R8A8_UNORM,
        sample_desc: DxgiSampleDesc { count: 1, quality: 0 },
        usage: 0, // DEFAULT
        bind_flags: D3D11_BIND_SHADER_RESOURCE,
        cpu_access_flags: 0,
        misc_flags: 0,
    };
    let mut texture = std::ptr::null_mut();
    if unsafe { create_tex(dev, &tdesc, std::ptr::null(), &mut texture) } < 0 || texture.is_null() {
        return false;
    }

    // CreateShaderResourceView = vtable[7]
    type CreateSrvFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_void, *mut *mut c_void) -> i32;
    let create_srv: CreateSrvFn = unsafe { std::mem::transmute(*dvtbl.add(7)) };
    let mut srv = std::ptr::null_mut();
    if unsafe { create_srv(dev, texture, std::ptr::null(), &mut srv) } < 0 || srv.is_null() {
        return false;
    }

    // Compile VS + PS
    let Some(vs_blob) = (unsafe { compile_hlsl(b"VSMain\0", b"vs_4_0\0") }) else {
        crate::log_line("dx_overlay: VSMain compile failed");
        return false;
    };
    let Some(ps_blob) = (unsafe { compile_hlsl(b"PSMain\0", b"ps_4_0\0") }) else {
        unsafe { com_release(vs_blob) };
        crate::log_line("dx_overlay: PSMain compile failed");
        return false;
    };
    let (vs_p, vs_l) = unsafe { blob_ptr_len(vs_blob) };
    let (ps_p, ps_l) = unsafe { blob_ptr_len(ps_blob) };

    // CreateVertexShader = vtable[12], CreatePixelShader = vtable[15]
    type CreateShaderFn = unsafe extern "system" fn(*mut c_void, *const u8, usize, *mut c_void, *mut *mut c_void) -> i32;
    let create_vs: CreateShaderFn = unsafe { std::mem::transmute(*dvtbl.add(12)) };
    let create_ps: CreateShaderFn = unsafe { std::mem::transmute(*dvtbl.add(15)) };
    let mut vs = std::ptr::null_mut();
    let mut ps = std::ptr::null_mut();
    let ok_vs = unsafe { create_vs(dev, vs_p, vs_l, std::ptr::null_mut(), &mut vs) } >= 0;
    let ok_ps = unsafe { create_ps(dev, ps_p, ps_l, std::ptr::null_mut(), &mut ps) } >= 0;
    unsafe {
        com_release(vs_blob);
        com_release(ps_blob);
    }
    if !ok_vs || !ok_ps || vs.is_null() || ps.is_null() {
        return false;
    }

    // CreateSamplerState = vtable[23]
    type CreateSampFn = unsafe extern "system" fn(*mut c_void, *const D3d11SamplerDesc, *mut *mut c_void) -> i32;
    let create_samp: CreateSampFn = unsafe { std::mem::transmute(*dvtbl.add(23)) };
    let sdesc = D3d11SamplerDesc {
        filter: 0x15, // MIN_MAG_MIP_LINEAR
        addr_u: 3,    // CLAMP
        addr_v: 3,
        addr_w: 3,
        mip_lod_bias: 0.0,
        max_aniso: 1,
        comp_func: 1,
        border_color: [0.0; 4],
        min_lod: 0.0,
        max_lod: f32::MAX,
    };
    let mut sampler = std::ptr::null_mut();
    if unsafe { create_samp(dev, &sdesc, &mut sampler) } < 0 || sampler.is_null() {
        return false;
    }

    crate::log_line(&format!(
        "dx_overlay: D3D11 {RENDER_W}x{RENDER_H} (HiDPI) menu blitter initialized cleanly"
    ));
    *guard = Some(Dx11Blitter {
        device: dev,
        context: ctx,
        texture,
        srv,
        vs,
        ps,
        sampler,
    });
    true
}

unsafe extern "system" fn detour_present(swapchain: *mut c_void, sync: u32, flags: u32) -> i32 {
    if !swapchain.is_null() && crate::ui_state::is_custom_menu_active() {
        if unsafe { ensure_blitter(swapchain) } {
            unsafe { draw_overlay(swapchain) };
        }
    }
    if let Some(&orig) = ORIG_PRESENT.get() {
        unsafe { orig(swapchain, sync, flags) }
    } else {
        0
    }
}

unsafe fn draw_overlay(swapchain: *mut c_void) {
    let Ok(gpu_guard) = GPU_STATE.lock() else { return };
    let Some(b) = gpu_guard.as_ref() else { return };

    // BFBC1 menu is static (event-driven: 0% CPU when idle); BF1943 animates planes/spinner at ~60fps.
    let needs_anim = UI_STATE
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref().map(|st| {
                st.active_era != opfor_vfs::ModeId::Bfbc1 || st.pending_deploy_level.is_some()
            })
        })
        .unwrap_or(false);
    let now = Instant::now();
    let anim_due = if needs_anim {
        LAST_RENDER
            .lock()
            .ok()
            .map(|mut last| match *last {
                Some(prev) if now.duration_since(prev).as_millis() < 16 => false,
                _ => {
                    *last = Some(now);
                    true
                }
            })
            .unwrap_or(true)
    } else {
        false
    };
    let dirty = CANVAS_DIRTY.swap(false, Ordering::AcqRel);
    if dirty || anim_due {
        let elapsed_secs = ANIM_CLOCK
            .get_or_init(Instant::now)
            .elapsed()
            .as_secs_f32();
        if let (Ok(mut c_guard), Ok(st_guard)) = (CANVAS.lock(), UI_STATE.lock()) {
            if let (Some(canvas), Some(st)) = (c_guard.as_mut(), st_guard.as_ref()) {
                canvas.render(st, elapsed_secs);
                if let Ok(mut hr) = HIT_RECTS.lock() {
                    hr.clear();
                    hr.extend_from_slice(&canvas.hit_rects);
                }
                // ID3D11DeviceContext::UpdateSubresource = vtable[48]
                type UpdateSubFn = unsafe extern "system" fn(
                    *mut c_void,
                    *mut c_void,
                    u32,
                    *const c_void,
                    *const u8,
                    u32,
                    u32,
                );
                let upd: UpdateSubFn = unsafe { std::mem::transmute(*com_vtable(b.context).add(48)) };
                unsafe {
                    upd(
                        b.context,
                        b.texture,
                        0,
                        std::ptr::null(),
                        canvas.pixels.as_ptr(),
                        RENDER_W * 4,
                        0,
                    );
                }
            }
        }
    }

    // IDXGISwapChain::GetBuffer(0, IID_ID3D11Texture2D) = vtable[9]
    type GetBufFn = unsafe extern "system" fn(*mut c_void, u32, *const Guid, *mut *mut c_void) -> i32;
    let get_buf: GetBufFn = unsafe { std::mem::transmute(*com_vtable(swapchain).add(9)) };
    let mut backbuf = std::ptr::null_mut();
    if unsafe { get_buf(swapchain, 0, &IID_ID3D11_TEXTURE2D, &mut backbuf) } < 0 || backbuf.is_null() {
        return;
    }

    // Read the actual swapchain backbuffer dimensions via ID3D11Texture2D::GetDesc (vtable[10])
    // so fullscreen & high-DPI modes always fill 100% of the screen.
    type GetDescFn = unsafe extern "system" fn(*mut c_void, *mut D3d11Tex2dDesc);
    let get_desc: GetDescFn = unsafe { std::mem::transmute(*com_vtable(backbuf).add(10)) };
    let mut bb_desc = D3d11Tex2dDesc {
        width: 1280,
        height: 720,
        mip_levels: 1,
        array_size: 1,
        format: 0,
        sample_desc: DxgiSampleDesc { count: 1, quality: 0 },
        usage: 0,
        bind_flags: 0,
        cpu_access_flags: 0,
        misc_flags: 0,
    };
    unsafe { get_desc(backbuf, &mut bb_desc) };

    // ID3D11Device::CreateRenderTargetView = vtable[9]
    type CreateRtvFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_void, *mut *mut c_void) -> i32;
    let create_rtv: CreateRtvFn = unsafe { std::mem::transmute(*com_vtable(b.device).add(9)) };
    let mut rtv = std::ptr::null_mut();
    let rtv_ok = unsafe { create_rtv(b.device, backbuf, std::ptr::null(), &mut rtv) } >= 0 && !rtv.is_null();
    unsafe { com_release(backbuf) };
    if !rtv_ok {
        return;
    }

    let vp = D3d11Viewport {
        top_left_x: 0.0,
        top_left_y: 0.0,
        width: bb_desc.width.max(1) as f32,
        height: bb_desc.height.max(1) as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    };

    let cvtbl = unsafe { com_vtable(b.context) };
    type OmSetRtvFn = unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void, *mut c_void);
    type RsSetVpFn = unsafe extern "system" fn(*mut c_void, u32, *const D3d11Viewport);
    type IaSetTopoFn = unsafe extern "system" fn(*mut c_void, u32);
    type SetShaderFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *const *mut c_void, u32);
    type PsSetSrvFn = unsafe extern "system" fn(*mut c_void, u32, u32, *const *mut c_void);
    type PsSetSampFn = unsafe extern "system" fn(*mut c_void, u32, u32, *const *mut c_void);
    type OmSetBlendFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *const f32, u32);
    type OmSetDepthFn = unsafe extern "system" fn(*mut c_void, *mut c_void, u32);
    type RsSetStateFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
    type IaSetLayoutFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
    type DrawFn = unsafe extern "system" fn(*mut c_void, u32, u32);

    unsafe {
        let om_set_rtv: OmSetRtvFn = std::mem::transmute(*cvtbl.add(33));
        let rs_set_vp: RsSetVpFn = std::mem::transmute(*cvtbl.add(44));
        let ia_set_topo: IaSetTopoFn = std::mem::transmute(*cvtbl.add(24));
        let ia_set_layout: IaSetLayoutFn = std::mem::transmute(*cvtbl.add(17));
        let vs_set: SetShaderFn = std::mem::transmute(*cvtbl.add(11));
        let ps_set: SetShaderFn = std::mem::transmute(*cvtbl.add(9));
        let ps_set_srv: PsSetSrvFn = std::mem::transmute(*cvtbl.add(8));
        let ps_set_samp: PsSetSampFn = std::mem::transmute(*cvtbl.add(10));
        let om_set_blend: OmSetBlendFn = std::mem::transmute(*cvtbl.add(35));
        let om_set_depth: OmSetDepthFn = std::mem::transmute(*cvtbl.add(36));
        let rs_set_state: RsSetStateFn = std::mem::transmute(*cvtbl.add(43));
        let draw: DrawFn = std::mem::transmute(*cvtbl.add(13));

        om_set_rtv(b.context, 1, &rtv, std::ptr::null_mut());
        rs_set_vp(b.context, 1, &vp);
        om_set_blend(b.context, std::ptr::null_mut(), std::ptr::null(), 0xFFFF_FFFF);
        om_set_depth(b.context, std::ptr::null_mut(), 0);
        rs_set_state(b.context, std::ptr::null_mut());
        ia_set_layout(b.context, std::ptr::null_mut());
        ia_set_topo(b.context, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        vs_set(b.context, b.vs, std::ptr::null(), 0);
        ps_set(b.context, b.ps, std::ptr::null(), 0);
        ps_set_srv(b.context, 0, 1, &b.srv);
        ps_set_samp(b.context, 0, 1, &b.sampler);
        draw(b.context, 3, 0);

        let null_srv: *mut c_void = std::ptr::null_mut();
        ps_set_srv(b.context, 0, 1, &null_srv);
        com_release(rtv);
    }
}

unsafe extern "system" fn hook_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    if msg == WM_SETCURSOR {
        let active_mode = crate::hooks::STATE
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.active))
            .unwrap_or(opfor_vfs::ModeId::Bfbc2);
        let is_era_port = matches!(active_mode, opfor_vfs::ModeId::Bfbc1 | opfor_vfs::ModeId::Bf1943);
        let in_menu = crate::ui_state::current_scene() == crate::ui_state::GameScene::InMenu
            || crate::era_ui::is_ingame_menu_open();
        if in_menu && (is_era_port || crate::ui_state::is_custom_menu_active()) {
            if crate::ui_state::current_input_device() == crate::ui_state::InputDevice::KeyboardMouse {
                unsafe {
                    SetCursor(era_tactical_cursor(active_mode));
                }
            } else {
                unsafe { SetCursor(std::ptr::null_mut()) };
            }
            return 1;
        }
    }
    if msg == WM_INPUT {
        crate::controller::handle_wm_input(lparam);
    } else if matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MOUSEWHEEL | WM_KEYDOWN) {
        crate::ui_state::note_kbm_input();
    } else if msg == WM_MOUSEMOVE {
        static LAST_MX: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);
        static LAST_MY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);
        let mx = (lparam & 0xFFFF) as i16 as i32;
        let my = ((lparam >> 16) & 0xFFFF) as i16 as i32;
        let px = LAST_MX.swap(mx, std::sync::atomic::Ordering::Relaxed);
        let py = LAST_MY.swap(my, std::sync::atomic::Ordering::Relaxed);
        if px != i32::MIN && ((mx - px).abs() > 6 || (my - py).abs() > 6) {
            crate::ui_state::note_kbm_input();
        }
    }

    if crate::ui_state::is_custom_menu_active() {
        match msg {
            WM_MOUSEMOVE => {
                let mut rc = Rect { left: 0, top: 0, right: 1280, bottom: 720 };
                unsafe { GetClientRect(hwnd, &mut rc) };
                let cw = (rc.right - rc.left).max(1) as f32;
                let ch = (rc.bottom - rc.top).max(1) as f32;
                let lx = (lparam & 0xFFFF) as i16 as f32;
                let ly = ((lparam >> 16) & 0xFFFF) as i16 as f32;
                let mx = (lx / cw) * (CANVAS_W as f32);
                let my = (ly / ch) * (CANVAS_H as f32);
                let hovered = HIT_RECTS
                    .lock()
                    .ok()
                    .and_then(|hr| hr.iter().find(|r| r.contains(mx, my)).map(|r| r.action));
                let mut changed = false;
                if let Ok(mut g) = UI_STATE.lock() {
                    if let Some(st) = g.as_mut() {
                        st.mouse_x = mx;
                        st.mouse_y = my;
                        match hovered {
                            Some(MenuClickAction::HoverAndActivate(i)) => {
                                if st.cursor_idx != i {
                                    st.cursor_idx = i;
                                    play_ui_sfx("ui_scroll.wav");
                                    changed = true;
                                }
                                if st.screen == Bf1943Screen::OfflineMatches {
                                    let clamped = i.min(PACIFIC_MAPS.len() - 1);
                                    if st.bf1943_selected_map != clamped {
                                        st.bf1943_selected_map = clamped;
                                        changed = true;
                                    }
                                }
                            }
                            Some(MenuClickAction::ModalRowActivate(r))
                            | Some(MenuClickAction::ModalCycle(r, _)) => {
                                if st.modal_cursor != r {
                                    st.modal_cursor = r;
                                    play_ui_sfx("ui_scroll.wav");
                                    changed = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if changed {
                    mark_dirty();
                }
                return 0;
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                let (mx, my) = UI_STATE
                    .lock()
                    .ok()
                    .and_then(|g| g.as_ref().map(|s| (s.mouse_x, s.mouse_y)))
                    .unwrap_or((0.0, 0.0));
                let clicked = HIT_RECTS
                    .lock()
                    .ok()
                    .and_then(|hr| hr.iter().find(|r| r.contains(mx, my)).map(|r| r.action));
                if let Some(act) = clicked {
                    if let Ok(mut g) = UI_STATE.lock() {
                        if let Some(st) = g.as_mut() {
                            match act {
                                MenuClickAction::HoverAndActivate(i) => {
                                    st.cursor_idx = i;
                                    st.activate_cursor();
                                    play_ui_sfx("ui_forward.wav");
                                }
                                MenuClickAction::ModalRowActivate(r) => {
                                    st.modal_cursor = r;
                                    st.activate_cursor();
                                    play_ui_sfx("ui_forward.wav");
                                }
                                MenuClickAction::ModalCycle(r, delta) => {
                                    st.modal_cursor = r;
                                    st.cycle_horizontal(delta);
                                    play_ui_sfx("ui_tick.wav");
                                }
                                MenuClickAction::GoBack => {
                                    st.go_back();
                                    play_ui_sfx("ui_backwards.wav");
                                }
                                MenuClickAction::ToggleFaction => {
                                    st.bf1943_faction_ijn = !st.bf1943_faction_ijn;
                                    play_ui_sfx("ui_tick.wav");
                                }
                            }
                        }
                    }
                    mark_dirty();
                }
                return 0;
            }
            WM_CHAR => {
                if let Some(ch) = char::from_u32(wparam as u32) {
                    if let Ok(mut g) = UI_STATE.lock() {
                        if let Some(st) = g.as_mut() {
                            if st.modal == Bf1943Modal::DirectIp {
                                st.push_ip_char(ch);
                                play_ui_sfx("ui_tick.wav");
                                mark_dirty();
                                return 0;
                            }
                        }
                    }
                }
            }
            WM_KEYDOWN => {
                if let Ok(mut g) = UI_STATE.lock() {
                    if let Some(st) = g.as_mut() {
                        match wparam {
                            VK_UP => {
                                st.move_cursor(-1);
                                play_ui_sfx("ui_scroll.wav");
                                mark_dirty();
                                return 0;
                            }
                            VK_DOWN => {
                                st.move_cursor(1);
                                play_ui_sfx("ui_scroll.wav");
                                mark_dirty();
                                return 0;
                            }
                            VK_LEFT => {
                                st.cycle_horizontal(-1);
                                play_ui_sfx("ui_tick.wav");
                                mark_dirty();
                                return 0;
                            }
                            VK_RIGHT => {
                                st.cycle_horizontal(1);
                                play_ui_sfx("ui_tick.wav");
                                mark_dirty();
                                return 0;
                            }
                            VK_RETURN => {
                                st.activate_cursor();
                                play_ui_sfx("ui_forward.wav");
                                mark_dirty();
                                return 0;
                            }
                            VK_ESCAPE => {
                                if st.modal != Bf1943Modal::None || st.screen != Bf1943Screen::Main {
                                    st.go_back();
                                    play_ui_sfx("ui_backwards.wav");
                                    mark_dirty();
                                }
                                return 0;
                            }
                            VK_BACK => {
                                if st.modal != Bf1943Modal::DirectIp {
                                    if st.modal != Bf1943Modal::None || st.screen != Bf1943Screen::Main {
                                        st.go_back();
                                        play_ui_sfx("ui_backwards.wav");
                                        mark_dirty();
                                    }
                                    return 0;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(&orig) = ORIG_WNDPROC.get() {
        unsafe { CallWindowProcW(orig, hwnd, msg, wparam, lparam) }
    } else {
        0
    }
}
