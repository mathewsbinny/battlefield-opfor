//! Authentic Battlefield 1943 (Xbox 360) In-Game Menu Renderer.
//!
//! Faithfully matches the original Battlefield 1943 visual design language:
//! - Full-bleed Pacific sky & clouds with diving Zero (left) and banking Corsair (right)
//! - Top-center bold distressed `BATTLEFIELD` + `★ ★ ★  1943  ★ ★ ★` logo
//! - Centered crisp off-white paper menu box (`#F3F2EC`) with thin dark border, hairline
//!   row dividers, bold condensed black text, and military olive-green (`#5D8035`) active highlight
//! - Matching paper-and-olive-green modals & submenus for Quick Match, Offline Matches,
//!   Connect to a Match (Direct IP), Host Private Match, Stats & Awards, Help & Options,
//!   and Achievements (all 12 BF1943 XBLA achievements offline).

#![allow(non_snake_case, dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;

use crate::ui_state::{Bf1943Modal, Bf1943Screen, InGameUiState, InputDevice, PACIFIC_MAPS};
use crate::win32;

pub const CANVAS_W: u32 = 1280;
pub const CANVAS_H: u32 = 720;
/// Backing raster resolution (1920x1080 Full HD — 1.5x over the 1280x720 logical layout)
/// balancing razor-sharp vector text with fast frame times & low PCIe upload bandwidth.
pub const RENDER_SCALE_F: f32 = 1.5;
pub const RENDER_W: u32 = 1920;
pub const RENDER_H: u32 = 1080;

const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026_200A;
const FONT_STYLE_REGULAR: i32 = 0;
const FONT_STYLE_BOLD: i32 = 1;
const FONT_STYLE_BOLD_ITALIC: i32 = 3;
const UNIT_PIXEL: i32 = 2;
const STRING_ALIGN_NEAR: i32 = 0;
const STRING_ALIGN_CENTER: i32 = 1;
const STRING_ALIGN_FAR: i32 = 2;

#[repr(C)]
struct GdiplusStartupInput {
    version: u32,
    debug_cb: *mut c_void,
    suppress_bg: i32,
    suppress_codecs: i32,
}

#[repr(C)]
struct RectF {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[link(name = "gdiplus")]
unsafe extern "system" {
    fn GdiplusStartup(token: *mut usize, input: *const GdiplusStartupInput, out: *mut c_void) -> i32;
    fn GdipCreateBitmapFromScan0(
        width: i32,
        height: i32,
        stride: i32,
        format: i32,
        scan0: *mut u8,
        bitmap: *mut *mut c_void,
    ) -> i32;
    fn GdipGetImageGraphicsContext(image: *mut c_void, graphics: *mut *mut c_void) -> i32;
    fn GdipDeleteGraphics(graphics: *mut c_void) -> i32;
    fn GdipDisposeImage(image: *mut c_void) -> i32;
    fn GdipLoadImageFromFile(filename: *const u16, image: *mut *mut c_void) -> i32;
    fn GdipGetImageWidth(image: *mut c_void, width: *mut u32) -> i32;
    fn GdipGetImageHeight(image: *mut c_void, height: *mut u32) -> i32;
    fn GdipSetSmoothingMode(graphics: *mut c_void, mode: i32) -> i32;
    fn GdipSetInterpolationMode(graphics: *mut c_void, mode: i32) -> i32;
    fn GdipSetTextRenderingHint(graphics: *mut c_void, mode: i32) -> i32;
    fn GdipScaleWorldTransform(graphics: *mut c_void, sx: f32, sy: f32, order: i32) -> i32;
    fn GdipGraphicsClear(graphics: *mut c_void, color: u32) -> i32;
    fn GdipCreateSolidFill(color: u32, brush: *mut *mut c_void) -> i32;
    fn GdipDeleteBrush(brush: *mut c_void) -> i32;
    fn GdipCreatePen1(color: u32, width: f32, unit: i32, pen: *mut *mut c_void) -> i32;
    fn GdipDeletePen(pen: *mut c_void) -> i32;
    fn GdipFillRectangle(g: *mut c_void, brush: *mut c_void, x: f32, y: f32, w: f32, h: f32) -> i32;
    fn GdipDrawRectangle(g: *mut c_void, pen: *mut c_void, x: f32, y: f32, w: f32, h: f32) -> i32;
    fn GdipFillEllipse(g: *mut c_void, brush: *mut c_void, x: f32, y: f32, w: f32, h: f32) -> i32;
    fn GdipDrawImageRect(g: *mut c_void, image: *mut c_void, x: f32, y: f32, w: f32, h: f32) -> i32;
    fn GdipCreateFontFamilyFromName(
        name: *const u16,
        collection: *mut c_void,
        family: *mut *mut c_void,
    ) -> i32;
    fn GdipCreateFont(
        family: *mut c_void,
        em_size: f32,
        style: i32,
        unit: i32,
        font: *mut *mut c_void,
    ) -> i32;
    fn GdipDeleteFont(font: *mut c_void) -> i32;
    fn GdipCreateStringFormat(format_attrs: i32, lang: u16, format: *mut *mut c_void) -> i32;
    fn GdipSetStringFormatAlign(format: *mut c_void, align: i32) -> i32;
    fn GdipSetStringFormatLineAlign(format: *mut c_void, align: i32) -> i32;
    fn GdipDeleteStringFormat(format: *mut c_void) -> i32;
    fn GdipDrawString(
        g: *mut c_void,
        string: *const u16,
        length: i32,
        font: *const c_void,
        layout: *const RectF,
        format: *const c_void,
        brush: *const c_void,
    ) -> i32;
}

static GDIPLUS_TOKEN: OnceLock<usize> = OnceLock::new();

fn ensure_gdiplus() {
    GDIPLUS_TOKEN.get_or_init(|| {
        let mut token = 0usize;
        let inp = GdiplusStartupInput {
            version: 1,
            debug_cb: std::ptr::null_mut(),
            suppress_bg: 0,
            suppress_codecs: 0,
        };
        unsafe { GdiplusStartup(&mut token, &inp, std::ptr::null_mut()) };
        token
    });
}

const fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

// Exact Xbox 360 Battlefield 1943 Palette (extracted directly from `main_nav.geo`)
const SKY_FALLBACK: u32 = argb(255, 118, 146, 174);
const PAPER_BG: u32 = argb(218, 245, 245, 242);      // Translucent off-white paper (#BFFFFFFF / #EDEDED)
const PAPER_SOLID: u32 = argb(214, 245, 245, 242);   // Frosted translucent modal/card paper (#CCFFFFFF / #D6F5F5F2)
const PAPER_ALT: u32 = argb(226, 234, 234, 230);     // Subtle alternating row paper (#E8E8E8)
const PAPER_BORDER: u32 = argb(255, 18, 20, 18);     // Crisp 1px black border (#000000)
const ROW_DIVIDER: u32 = argb(220, 51, 51, 51);      // Crisp hairline divider (#333333)
const OLIVE_ACTIVE: u32 = argb(248, 112, 143, 82);   // Exact BF1943 main_nav.geo olive (#708F52)
const OLIVE_HI: u32 = argb(255, 140, 179, 103);      // Exact BF1943 main_nav.geo highlight (#8CB367)
const OLIVE_DARK: u32 = argb(255, 74, 98, 52);       // Deeper olive label accent
const USMC_SLATE: u32 = argb(255, 60, 80, 102);      // Exact BF1943 main_nav.geo USMC slate (#3C5066)
const IJN_CRIMSON: u32 = argb(255, 199, 56, 44);     // Exact BF1943 main_nav.geo IJN red (#C7382C)
const INK_BLACK: u32 = argb(255, 14, 15, 13);        // Bold condensed black menu text
const INK_MUTED: u32 = argb(255, 78, 82, 74);        // Secondary charcoal text
const LOGO_SILVER: u32 = argb(255, 226, 228, 224);   // Fallback logo fill
const LOGO_OUTLINE: u32 = argb(255, 14, 16, 14);     // Fallback logo outline
const WHITE: u32 = argb(255, 255, 255, 255);
const MODAL_SCRIM: u32 = argb(135, 12, 16, 18);
const XBOX_A_GREEN: u32 = argb(255, 112, 143, 82);
const XBOX_B_RED: u32 = argb(255, 199, 56, 44);
const XBOX_Y_YELLOW: u32 = argb(255, 214, 176, 56);

#[derive(Clone, Copy)]
pub struct HitRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub action: MenuClickAction,
}

#[derive(Clone, Copy, Debug)]
pub enum MenuClickAction {
    HoverAndActivate(usize),
    ModalRowActivate(usize),
    ModalCycle(usize, i32),
    GoBack,
    ToggleFaction,
}

impl HitRect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Clone, Copy)]
struct BitmapGlyph {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    min_x: i32,
    y_offset: i32,
    advance: i32,
}

/// Literal Battlefield 1943 Xbox 360 bitmap font (`EurostileLTStd-BoldCn`) exported from
/// `build_overlay-00.fbrb` (`.png` L8 alpha atlas + `.fnt` glyph metrics).
struct Bf1943BitmapFont {
    atlas_w: u32,
    atlas_h: u32,
    alpha: Vec<u8>,
    glyphs: HashMap<u32, BitmapGlyph>,
    kerning: HashMap<(u32, u32), i32>,
    line_h: f32,
    space_adv: f32,
}

impl Bf1943BitmapFont {
    fn load(fonts_dir: &Path, stem: &str) -> Option<Self> {
        let png_path = fonts_dir.join(format!("{stem}.png"));
        let fnt_path = fonts_dir.join(format!("{stem}.fnt"));
        let fnt_txt = std::fs::read_to_string(&fnt_path).ok()?;
        let img = load_gdip_image(&png_path);
        if img.is_null() {
            return None;
        }
        let mut w = 0u32;
        let mut h = 0u32;
        unsafe {
            GdipGetImageWidth(img, &mut w);
            GdipGetImageHeight(img, &mut h);
        }
        if w == 0 || h == 0 || w > 2048 || h > 2048 {
            unsafe { GdipDisposeImage(img) };
            return None;
        }
        let mut tmp = vec![0u8; (w * h * 4) as usize];
        let mut bmp = std::ptr::null_mut();
        let ok = unsafe {
            GdipCreateBitmapFromScan0(
                w as i32,
                h as i32,
                (w * 4) as i32,
                PIXEL_FORMAT_32BPP_ARGB,
                tmp.as_mut_ptr(),
                &mut bmp,
            )
        } == 0
            && !bmp.is_null();
        let mut alpha = Vec::new();
        if ok {
            let mut gg = std::ptr::null_mut();
            if unsafe { GdipGetImageGraphicsContext(bmp, &mut gg) } == 0 && !gg.is_null() {
                unsafe {
                    GdipDrawImageRect(gg, img, 0.0, 0.0, w as f32, h as f32);
                    GdipDeleteGraphics(gg);
                }
                // L8 font atlas was decoded as RGB=(v,v,v), A=255 -> green channel is exact glyph alpha
                alpha = tmp.chunks_exact(4).map(|px| px[1]).collect();
            }
            unsafe { GdipDisposeImage(bmp) };
        }
        unsafe { GdipDisposeImage(img) };
        if alpha.len() != (w * h) as usize {
            return None;
        }

        let mut glyphs = HashMap::new();
        let mut kerning = HashMap::new();
        for line in fnt_txt.lines() {
            if let Some(rest) = line.strip_prefix("K\t") {
                let mut it = rest.split('\t');
                if let (Some(a), Some(b), Some(k)) = (
                    it.next().and_then(|s| s.parse::<u32>().ok()),
                    it.next().and_then(|s| s.parse::<u32>().ok()),
                    it.next().and_then(|s| s.parse::<i32>().ok()),
                ) {
                    kerning.insert((a, b), k);
                }
                continue;
            }
            let mut it = line.split('\t');
            let (Some(c), Some(gx), Some(gy), Some(gw), Some(gh), Some(mx), Some(yo), Some(adv)) = (
                it.next().and_then(|s| s.parse::<u32>().ok()),
                it.next().and_then(|s| s.parse::<i32>().ok()),
                it.next().and_then(|s| s.parse::<i32>().ok()),
                it.next().and_then(|s| s.parse::<i32>().ok()),
                it.next().and_then(|s| s.parse::<i32>().ok()),
                it.next().and_then(|s| s.parse::<i32>().ok()),
                it.next().and_then(|s| s.parse::<i32>().ok()),
                it.next().and_then(|s| s.parse::<i32>().ok()),
            ) else {
                continue;
            };
            glyphs.insert(
                c,
                BitmapGlyph {
                    x: gx,
                    y: gy,
                    w: gw,
                    h: gh,
                    min_x: mx,
                    y_offset: yo,
                    advance: adv,
                },
            );
        }
        if glyphs.is_empty() {
            return None;
        }
        let ref_g = glyphs.get(&(b'H' as u32)).or_else(|| glyphs.get(&(b'W' as u32)));
        let line_h = ref_g.map_or(24.0, |g| (g.y_offset + g.h).max(12) as f32);
        let space_adv = glyphs
            .get(&32)
            .map(|g| g.advance as f32)
            .filter(|&a| a > 1.0)
            .unwrap_or(line_h * 0.30);
        Some(Self {
            atlas_w: w,
            atlas_h: h,
            alpha,
            glyphs,
            kerning,
            line_h,
            space_adv,
        })
    }

    fn supports(&self, text: &str) -> bool {
        text.chars().all(|c| {
            c == ' ' || self.glyphs.contains_key(&(c as u32))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        g: *mut c_void,
        text: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        target_size: f32,
        color: u32,
        h_align: i32,
        v_align: i32,
    ) -> bool {
        if text.is_empty() || !self.supports(text) {
            return false;
        }
        // Measure unscaled width & height (with exact BF1943 kerning pairs)
        let mut total_w = 0i32;
        let mut max_bottom = self.line_h as i32;
        let mut prev_code = 0u32;
        for ch in text.chars() {
            let code = ch as u32;
            if prev_code != 0 {
                if let Some(&k) = self.kerning.get(&(prev_code, code)) {
                    total_w += k;
                }
            }
            if ch == ' ' {
                total_w += self.space_adv.round() as i32;
            } else if let Some(gl) = self.glyphs.get(&code) {
                total_w += gl.advance.max(gl.w);
                max_bottom = max_bottom.max(gl.y_offset + gl.h);
            }
            prev_code = code;
        }
        if total_w <= 0 || max_bottom <= 0 {
            return false;
        }
        let buf_w = (total_w + 4) as u32;
        let buf_h = (max_bottom + 2) as u32;
        let mut argb_buf = vec![0u8; (buf_w * buf_h * 4) as usize];

        let ca = ((color >> 24) & 0xFF) as u32;
        let cr = ((color >> 16) & 0xFF) as u8;
        let cg = ((color >> 8) & 0xFF) as u8;
        let cb = (color & 0xFF) as u8;

        let mut pen_x = 1i32;
        let mut prev_c = 0u32;
        for ch in text.chars() {
            let code = ch as u32;
            if prev_c != 0 {
                if let Some(&k) = self.kerning.get(&(prev_c, code)) {
                    pen_x += k;
                }
            }
            prev_c = code;
            if ch == ' ' {
                pen_x += self.space_adv.round() as i32;
                continue;
            }
            let Some(gl) = self.glyphs.get(&code) else { continue };
            for gy in 0..gl.h {
                let sy = gl.y + gy;
                let dy = gl.y_offset + gy;
                if sy < 0 || sy >= self.atlas_h as i32 || dy < 0 || dy >= buf_h as i32 {
                    continue;
                }
                for gx in 0..gl.w {
                    let sx = gl.x + gx;
                    let dx = pen_x + gl.min_x + gx;
                    if sx < 0 || sx >= self.atlas_w as i32 || dx < 0 || dx >= buf_w as i32 {
                        continue;
                    }
                    let a_raw = self.alpha[(sy as u32 * self.atlas_w + sx as u32) as usize];
                    if a_raw == 0 {
                        continue;
                    }
                    let a = ((u32::from(a_raw) * ca) / 255) as u8;
                    let di = ((dy as u32 * buf_w + dx as u32) * 4) as usize;
                    if a > argb_buf[di + 3] {
                        argb_buf[di] = cb;
                        argb_buf[di + 1] = cg;
                        argb_buf[di + 2] = cr;
                        argb_buf[di + 3] = a;
                    }
                }
            }
            pen_x += gl.advance.max(gl.w);
        }

        let scale = (target_size / self.line_h).clamp(0.25, 3.0);
        let mut draw_w = (buf_w as f32) * scale;
        let mut draw_h = (buf_h as f32) * scale;
        if draw_w > w && w > 4.0 {
            let fit = w / draw_w;
            draw_w *= fit;
            draw_h *= fit;
        }
        let draw_x = match h_align {
            STRING_ALIGN_CENTER => x + (w - draw_w) * 0.5,
            STRING_ALIGN_FAR => x + (w - draw_w),
            _ => x,
        };
        let draw_y = match v_align {
            STRING_ALIGN_CENTER => y + (h - draw_h) * 0.5 - 1.0,
            STRING_ALIGN_FAR => y + (h - draw_h),
            _ => y,
        };

        let mut bmp = std::ptr::null_mut();
        if unsafe {
            GdipCreateBitmapFromScan0(
                buf_w as i32,
                buf_h as i32,
                (buf_w * 4) as i32,
                PIXEL_FORMAT_32BPP_ARGB,
                argb_buf.as_mut_ptr(),
                &mut bmp,
            )
        } == 0
            && !bmp.is_null()
        {
            unsafe {
                GdipDrawImageRect(g, bmp, draw_x, draw_y, draw_w, draw_h);
                GdipDisposeImage(bmp);
            }
            true
        } else {
            false
        }
    }
}

pub struct MenuCanvas {
    pub pixels: Vec<u8>,
    pub hit_rects: Vec<HitRect>,
    /// Pre-rasterized 1920x1080 BF1943 sky backdrop (with paper grain baked in once at startup).
    backdrop_pixels: Vec<u8>,
    /// Pre-rasterized 1920x1080 BFBC1 backdrop (with tactical vignette baked in once at startup).
    bc1_backdrop_pixels: Vec<u8>,
    backdrop_img: *mut c_void,
    bc1_backdrop_img: *mut c_void,
    logo_img: *mut c_void,
    zero_img: *mut c_void,
    corsair_img: *mut c_void,
    headline_bar_img: *mut c_void,
    subheadline_bar_img: *mut c_void,
    btn_a_img: *mut c_void,
    btn_b_img: *mut c_void,
    btn_y_img: *mut c_void,
    flag_us_img: *mut c_void,
    flag_jp_img: *mut c_void,
    controller_img: *mut c_void,
    rank_imgs: Vec<*mut c_void>,
    us_soldiers: [*mut c_void; 3],
    jp_soldiers: [*mut c_void; 3],
    map_imgs: [*mut c_void; 4],
    /// Pre-rasterized 1280x720 grayscale `mc_raster` paper grain from `shared_images_atlas0.png`.
    grain_luma: Vec<u8>,
    /// Literal BF1943 Xbox 360 `EurostileLTStd-BoldCn` bitmap fonts (`headline_medium` & `normal_medium`).
    bf_headline: Option<Bf1943BitmapFont>,
    bf_normal: Option<Bf1943BitmapFont>,
    display_font: *mut c_void, // Fallback geometric condensed display font
    body_font: *mut c_void,    // Fallback body/wrap font
    italic_font: *mut c_void,  // Century Schoolbook Bold Italic for WW2 postcard accents
    // Smooth animation & transition state (33fps matching BF1943 APT)
    first_render_time: Option<f32>,
    last_screen: Bf1943Screen,
    screen_change_time: f32,
    last_modal: Bf1943Modal,
    modal_change_time: f32,
    zero_pos: (f32, f32),
    corsair_pos: (f32, f32),
}

unsafe impl Send for MenuCanvas {}

fn load_gdip_image(path: &Path) -> *mut c_void {
    if !path.is_file() {
        return std::ptr::null_mut();
    }
    let mut img = std::ptr::null_mut();
    let w = win32::to_wide_nul(&path.to_string_lossy());
    if unsafe { GdipLoadImageFromFile(w.as_ptr(), &mut img) } == 0 {
        img
    } else {
        std::ptr::null_mut()
    }
}

impl MenuCanvas {
    pub fn new(opfor_root: &Path) -> Self {
        ensure_gdiplus();
        let art_dir = opfor_root.join("modules").join("bf1943").join("art");
        let x360_dir = art_dir.join("x360");

        let mut backdrop_img = std::ptr::null_mut();
        for cand in [
            x360_dir.join("sky_bg.png"),
            art_dir.join("bf1943_sky_bg.png"),
            opfor_root.join("modules").join("bf1943").join("blade.png"),
        ] {
            backdrop_img = load_gdip_image(&cand);
            if !backdrop_img.is_null() {
                break;
            }
        }

        let bc1_backdrop_img = load_gdip_image(&opfor_root.join("modules").join("bfbc1").join("blade.png"));
        let logo_img = load_gdip_image(&x360_dir.join("bf1943_logo.png"));
        let zero_img = load_gdip_image(&x360_dir.join("zero.png"));
        let corsair_img = load_gdip_image(&x360_dir.join("corsair.png"));
        let headline_bar_img = load_gdip_image(&x360_dir.join("headline_bar.png"));
        let subheadline_bar_img = load_gdip_image(&x360_dir.join("subheadline_bar.png"));
        let btn_a_img = load_gdip_image(&x360_dir.join("btn_xbox_a.png"));
        let btn_b_img = load_gdip_image(&x360_dir.join("btn_xbox_b.png"));
        let btn_y_img = load_gdip_image(&x360_dir.join("btn_xbox_y.png"));
        let flag_us_img = load_gdip_image(&x360_dir.join("flag_us.png"));
        let flag_jp_img = load_gdip_image(&x360_dir.join("flag_jp.png"));
        let mut controller_img = load_gdip_image(&x360_dir.join("controller_360_hd.png"));
        if controller_img.is_null() {
            controller_img = load_gdip_image(&x360_dir.join("controller_xbox.png"));
        }
        let mut rank_imgs = Vec::with_capacity(25);
        for r in 1..=25u32 {
            rank_imgs.push(load_gdip_image(&x360_dir.join(format!("rank_r{r:03}.png"))));
        }
        let us_soldiers = [
            load_gdip_image(&x360_dir.join("soldier_us_infantry.png")),
            load_gdip_image(&x360_dir.join("soldier_us_rifleman.png")),
            load_gdip_image(&x360_dir.join("soldier_us_scout.png")),
        ];
        let jp_soldiers = [
            load_gdip_image(&x360_dir.join("soldier_jp_infantry.png")),
            load_gdip_image(&x360_dir.join("soldier_jp_rifleman.png")),
            load_gdip_image(&x360_dir.join("soldier_jp_scout.png")),
        ];

        // Pre-rasterize `paper_grain.png` (`mc_raster` from `shared_images_atlas0.png`) once into
        // a 2560x1440 luma table for a subtle multiply pass across the menu canvas.
        let mut grain_luma = Vec::new();
        let grain_img = load_gdip_image(&x360_dir.join("paper_grain.png"));
        if !grain_img.is_null() {
            let mut tmp = vec![255u8; (RENDER_W * RENDER_H * 4) as usize];
            let mut bmp = std::ptr::null_mut();
            if unsafe {
                GdipCreateBitmapFromScan0(
                    RENDER_W as i32,
                    RENDER_H as i32,
                    (RENDER_W * 4) as i32,
                    PIXEL_FORMAT_32BPP_ARGB,
                    tmp.as_mut_ptr(),
                    &mut bmp,
                )
            } == 0
                && !bmp.is_null()
            {
                let mut gg = std::ptr::null_mut();
                if unsafe { GdipGetImageGraphicsContext(bmp, &mut gg) } == 0 && !gg.is_null() {
                    unsafe {
                        GdipSetInterpolationMode(gg, 7);
                        GdipDrawImageRect(gg, grain_img, 0.0, 0.0, RENDER_W as f32, RENDER_H as f32);
                        GdipDeleteGraphics(gg);
                    }
                    grain_luma = tmp.chunks_exact(4).map(|px| px[1]).collect();
                }
                unsafe { GdipDisposeImage(bmp) };
            }
            unsafe { GdipDisposeImage(grain_img) };
        }

        let mut map_imgs = [std::ptr::null_mut(); 4];
        let map_files = [
            "thumb_wake_island_s.png",
            "thumb_guadal_canal.png",
            "thumb_iwo_jima_s.png",
            "thumb_coral_sea.png",
        ];
        for (i, fname) in map_files.iter().enumerate() {
            map_imgs[i] = load_gdip_image(&art_dir.join(fname));
        }

        let fonts_dir = opfor_root.join("modules").join("bf1943").join("fonts");
        let bf_headline = Bf1943BitmapFont::load(&fonts_dir, "headline_medium")
            .or_else(|| Bf1943BitmapFont::load(&fonts_dir, "normal_large"));
        let bf_normal = Bf1943BitmapFont::load(&fonts_dir, "normal_medium")
            .or_else(|| Bf1943BitmapFont::load(&fonts_dir, "list_item"));

        let display_font = Self::load_font_family(&["Agency FB", "Bahnschrift", "Arial Narrow", "Impact"]);
        let body_font = Self::load_font_family(&["Bahnschrift", "Arial Narrow", "Segoe UI", "Arial"]);
        let italic_font = Self::load_font_family(&["Century Schoolbook", "Georgia", "Times New Roman"]);

        // Pre-rasterize BF1943 backdrop + `paper_grain.png` once at startup (avoids ~50ms per-frame
        // HighQualityBicubic scaling + per-pixel grain multiply on every Present()).
        let mut backdrop_pixels = vec![0u8; (RENDER_W * RENDER_H * 4) as usize];
        {
            let mut bmp = std::ptr::null_mut();
            if unsafe {
                GdipCreateBitmapFromScan0(
                    RENDER_W as i32,
                    RENDER_H as i32,
                    (RENDER_W * 4) as i32,
                    PIXEL_FORMAT_32BPP_ARGB,
                    backdrop_pixels.as_mut_ptr(),
                    &mut bmp,
                )
            } == 0
                && !bmp.is_null()
            {
                let mut gg = std::ptr::null_mut();
                if unsafe { GdipGetImageGraphicsContext(bmp, &mut gg) } == 0 && !gg.is_null() {
                    unsafe {
                        GdipSetInterpolationMode(gg, 7);
                        GdipGraphicsClear(gg, SKY_FALLBACK);
                        if !backdrop_img.is_null() {
                            GdipDrawImageRect(gg, backdrop_img, 0.0, 0.0, RENDER_W as f32, RENDER_H as f32);
                        }
                        GdipDeleteGraphics(gg);
                    }
                }
                unsafe { GdipDisposeImage(bmp) };
            }
            if grain_luma.len() == (RENDER_W * RENDER_H) as usize {
                for (px, &gl) in backdrop_pixels.chunks_exact_mut(4).zip(grain_luma.iter()) {
                    let m = 214u32 + ((u32::from(gl) * 42) >> 8);
                    px[0] = ((u32::from(px[0]) * m) >> 8) as u8;
                    px[1] = ((u32::from(px[1]) * m) >> 8) as u8;
                    px[2] = ((u32::from(px[2]) * m) >> 8) as u8;
                }
            }
        }

        // Pre-rasterize BFBC1 backdrop + dark tactical vignette once at startup.
        let mut bc1_backdrop_pixels = vec![0u8; (RENDER_W * RENDER_H * 4) as usize];
        {
            let mut bmp = std::ptr::null_mut();
            if unsafe {
                GdipCreateBitmapFromScan0(
                    RENDER_W as i32,
                    RENDER_H as i32,
                    (RENDER_W * 4) as i32,
                    PIXEL_FORMAT_32BPP_ARGB,
                    bc1_backdrop_pixels.as_mut_ptr(),
                    &mut bmp,
                )
            } == 0
                && !bmp.is_null()
            {
                let mut gg = std::ptr::null_mut();
                if unsafe { GdipGetImageGraphicsContext(bmp, &mut gg) } == 0 && !gg.is_null() {
                    unsafe {
                        GdipSetInterpolationMode(gg, 7);
                        GdipGraphicsClear(gg, 0xFF10_1310);
                        if !bc1_backdrop_img.is_null() {
                            GdipDrawImageRect(gg, bc1_backdrop_img, 0.0, 0.0, RENDER_W as f32, RENDER_H as f32);
                        }
                        GdipScaleWorldTransform(gg, RENDER_SCALE_F, RENDER_SCALE_F, 0);
                        Self::fill_rect(gg, argb(175, 14, 17, 14), 0.0, 0.0, CANVAS_W as f32, CANVAS_H as f32);
                        Self::fill_rect(gg, argb(210, 12, 14, 12), 0.0, 0.0, 510.0, CANVAS_H as f32);
                        Self::fill_rect(gg, 0xFFE0_A82E, 0.0, 0.0, 6.0, CANVAS_H as f32);
                        GdipDeleteGraphics(gg);
                    }
                }
                unsafe { GdipDisposeImage(bmp) };
            }
        }

        Self {
            pixels: vec![0u8; (RENDER_W * RENDER_H * 4) as usize],
            hit_rects: Vec::with_capacity(48),
            backdrop_pixels,
            bc1_backdrop_pixels,
            backdrop_img,
            bc1_backdrop_img,
            logo_img,
            zero_img,
            corsair_img,
            headline_bar_img,
            subheadline_bar_img,
            btn_a_img,
            btn_b_img,
            btn_y_img,
            flag_us_img,
            flag_jp_img,
            controller_img,
            rank_imgs,
            us_soldiers,
            jp_soldiers,
            map_imgs,
            grain_luma,
            bf_headline,
            bf_normal,
            display_font,
            body_font,
            italic_font,
            first_render_time: None,
            last_screen: Bf1943Screen::Main,
            screen_change_time: 0.0,
            last_modal: Bf1943Modal::None,
            modal_change_time: 0.0,
            zero_pos: (118.0, 224.0),
            corsair_pos: (718.0, 126.0),
        }
    }

    fn load_font_family(candidates: &[&str]) -> *mut c_void {
        for name in candidates {
            let mut fam = std::ptr::null_mut();
            let wn = win32::to_wide_nul(name);
            if unsafe { GdipCreateFontFamilyFromName(wn.as_ptr(), std::ptr::null_mut(), &mut fam) } == 0
                && !fam.is_null()
            {
                return fam;
            }
        }
        std::ptr::null_mut()
    }

    fn fill_rect(g: *mut c_void, color: u32, x: f32, y: f32, w: f32, h: f32) {
        let mut brush = std::ptr::null_mut();
        if unsafe { GdipCreateSolidFill(color, &mut brush) } == 0 && !brush.is_null() {
            unsafe {
                GdipFillRectangle(g, brush, x, y, w, h);
                GdipDeleteBrush(brush);
            }
        }
    }

    fn stroke_rect(g: *mut c_void, color: u32, width: f32, x: f32, y: f32, w: f32, h: f32) {
        let mut pen = std::ptr::null_mut();
        if unsafe { GdipCreatePen1(color, width, UNIT_PIXEL, &mut pen) } == 0 && !pen.is_null() {
            unsafe {
                GdipDrawRectangle(g, pen, x, y, w, h);
                GdipDeletePen(pen);
            }
        }
    }

    fn fill_circle(g: *mut c_void, color: u32, x: f32, y: f32, d: f32) {
        let mut brush = std::ptr::null_mut();
        if unsafe { GdipCreateSolidFill(color, &mut brush) } == 0 && !brush.is_null() {
            unsafe {
                GdipFillEllipse(g, brush, x, y, d, d);
                GdipDeleteBrush(brush);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text_ex(
        family: *mut c_void,
        g: *mut c_void,
        text: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        size: f32,
        bold: bool,
        color: u32,
        h_align: i32,
        v_align: i32,
    ) {
        if family.is_null() {
            return;
        }
        let mut font = std::ptr::null_mut();
        let style = if bold { FONT_STYLE_BOLD } else { FONT_STYLE_REGULAR };
        if unsafe { GdipCreateFont(family, size, style, UNIT_PIXEL, &mut font) } != 0
            || font.is_null()
        {
            return;
        }
        let mut fmt = std::ptr::null_mut();
        if unsafe { GdipCreateStringFormat(0, 0, &mut fmt) } == 0 && !fmt.is_null() {
            unsafe {
                GdipSetStringFormatAlign(fmt, h_align);
                GdipSetStringFormatLineAlign(fmt, v_align);
            }
        }
        let mut brush = std::ptr::null_mut();
        if unsafe { GdipCreateSolidFill(color, &mut brush) } == 0 && !brush.is_null() {
            let wt = win32::to_wide_nul(text);
            let rect = RectF { x, y, w, h };
            unsafe {
                GdipDrawString(
                    g,
                    wt.as_ptr(),
                    (wt.len() - 1) as i32,
                    font,
                    &rect,
                    fmt,
                    brush,
                );
                GdipDeleteBrush(brush);
            }
        }
        if !fmt.is_null() {
            unsafe { GdipDeleteStringFormat(fmt) };
        }
        unsafe { GdipDeleteFont(font) };
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_display(&self, g: *mut c_void, text: &str, x: f32, y: f32, w: f32, h: f32, size: f32, color: u32, align: i32) {
        // Use vector TrueType at 2560x1440 HiDPI for crisp legibility at all sizes;
        // only use the Xbox 360 bitmap atlas for large display headers (>= 26px).
        if size >= 26.0 {
            if let Some(bf) = self.bf_headline.as_ref() {
                if bf.draw(g, text, x, y, w, h, size * 1.15, color, align, STRING_ALIGN_CENTER) {
                    return;
                }
            }
        }
        Self::draw_text_ex(self.display_font, g, text, x, y, w, h, size, true, color, align, STRING_ALIGN_CENTER);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_body(&self, g: *mut c_void, text: &str, x: f32, y: f32, w: f32, h: f32, size: f32, bold: bool, color: u32) {
        // Always render body/detail text via HiDPI vector TrueType so small text is 100% legible.
        Self::draw_text_ex(self.body_font, g, text, x, y, w, h, size, bold, color, STRING_ALIGN_NEAR, STRING_ALIGN_NEAR);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_italic_accent(&self, g: *mut c_void, text: &str, x: f32, y: f32, w: f32, h: f32, size: f32, color: u32) {
        if self.italic_font.is_null() {
            self.draw_body(g, text, x, y, w, h, size, true, color);
            return;
        }
        let mut font = std::ptr::null_mut();
        if unsafe { GdipCreateFont(self.italic_font, size, FONT_STYLE_BOLD_ITALIC, UNIT_PIXEL, &mut font) } != 0
            || font.is_null()
        {
            return;
        }
        let mut fmt = std::ptr::null_mut();
        if unsafe { GdipCreateStringFormat(0, 0, &mut fmt) } == 0 && !fmt.is_null() {
            unsafe {
                GdipSetStringFormatAlign(fmt, STRING_ALIGN_NEAR);
                GdipSetStringFormatLineAlign(fmt, STRING_ALIGN_NEAR);
            }
        }
        let mut brush = std::ptr::null_mut();
        if unsafe { GdipCreateSolidFill(color, &mut brush) } == 0 && !brush.is_null() {
            let wt = win32::to_wide_nul(text);
            let rect = RectF { x, y, w, h };
            unsafe {
                GdipDrawString(g, wt.as_ptr(), (wt.len() - 1) as i32, font, &rect, fmt, brush);
                GdipDeleteBrush(brush);
            }
        }
        if !fmt.is_null() {
            unsafe { GdipDeleteStringFormat(fmt) };
        }
        unsafe { GdipDeleteFont(font) };
    }

    /// Draws the literal extracted Xbox 360 `BATTLEFIELD / ★ ★ ★ 1943 ★ ★ ★` logo sprite
    /// (falling back to crisp vector stencil text if the PNG is absent).
    fn draw_bf1943_logo(&self, g: *mut c_void, top_y: f32, scale: f32) {
        if !self.logo_img.is_null() {
            let lw = 608.0 * scale;
            let lh = 191.0 * scale;
            let lx = ((CANVAS_W as f32) - lw) * 0.5;
            unsafe { GdipDrawImageRect(g, self.logo_img, lx, top_y, lw, lh) };
            return;
        }
        let title_size = 120.0 * scale;
        let title_h = 132.0 * scale;
        let outline = (3.5 * scale).max(1.5);

        // Crisp thick black stencil outline around "BATTLEFIELD"
        let offsets = [
            (-outline, -outline),
            (0.0, -outline),
            (outline, -outline),
            (-outline, 0.0),
            (outline, 0.0),
            (-outline, outline),
            (0.0, outline),
            (outline, outline),
        ];
        for (ox, oy) in offsets {
            Self::draw_text_ex(
                self.display_font,
                g,
                "BATTLEFIELD",
                ox,
                top_y + oy,
                CANVAS_W as f32,
                title_h,
                title_size,
                false,
                LOGO_OUTLINE,
                STRING_ALIGN_CENTER,
                STRING_ALIGN_CENTER,
            );
        }
        // Subtle drop shadow + bright weathered silver-white fill
        Self::draw_text_ex(
            self.display_font,
            g,
            "BATTLEFIELD",
            0.0,
            top_y,
            CANVAS_W as f32,
            title_h,
            title_size,
            false,
            LOGO_SILVER,
            STRING_ALIGN_CENTER,
            STRING_ALIGN_CENTER,
        );

        // Horizontal rules + `★  ★  ★   1943   ★  ★  ★`
        let sub_y = top_y + 126.0 * scale;
        let bar_h = (3.2 * scale).max(1.8);
        let half_span = 296.0 * scale;
        let center_gap = 78.0 * scale;
        let cx = (CANVAS_W as f32) * 0.5;

        // Top & bottom black horizontal bars on left and right of "1943"
        let rule_top = sub_y + 4.0 * scale;
        let rule_bot = sub_y + 48.0 * scale;
        let left_w = half_span - center_gap;
        Self::fill_rect(g, LOGO_OUTLINE, cx - half_span, rule_top, left_w, bar_h);
        Self::fill_rect(g, LOGO_OUTLINE, cx - half_span, rule_bot, left_w, bar_h);
        Self::fill_rect(g, LOGO_OUTLINE, cx + center_gap, rule_top, left_w, bar_h);
        Self::fill_rect(g, LOGO_OUTLINE, cx + center_gap, rule_bot, left_w, bar_h);

        // 3 stars on the left, 3 stars on the right
        let star_size = 24.0 * scale;
        for s in 0..3 {
            let lx = cx - half_span + (22.0 + (s as f32) * 68.0) * scale;
            let rx = cx + center_gap + (22.0 + (s as f32) * 68.0) * scale;
            Self::draw_text_ex(
                self.body_font,
                g,
                "★",
                lx,
                sub_y + 8.0 * scale,
                36.0 * scale,
                36.0 * scale,
                star_size,
                true,
                LOGO_OUTLINE,
                STRING_ALIGN_CENTER,
                STRING_ALIGN_CENTER,
            );
            Self::draw_text_ex(
                self.body_font,
                g,
                "★",
                rx,
                sub_y + 8.0 * scale,
                36.0 * scale,
                36.0 * scale,
                star_size,
                true,
                LOGO_OUTLINE,
                STRING_ALIGN_CENTER,
                STRING_ALIGN_CENTER,
            );
        }

        // Bold "1943" centered between the stars
        Self::draw_text_ex(
            self.display_font,
            g,
            "1943",
            cx - 90.0 * scale,
            sub_y - 4.0 * scale,
            180.0 * scale,
            62.0 * scale,
            54.0 * scale,
            false,
            LOGO_OUTLINE,
            STRING_ALIGN_CENTER,
            STRING_ALIGN_CENTER,
        );
    }

    fn draw_xbox_badge(&self, g: *mut c_void, x: f32, y: f32, color: u32, letter: &str) {
        let sprite = match letter {
            "A" => self.btn_a_img,
            "B" => self.btn_b_img,
            "Y" => self.btn_y_img,
            _ => std::ptr::null_mut(),
        };
        if !sprite.is_null() {
            unsafe { GdipDrawImageRect(g, sprite, x - 2.0, y - 2.0, 26.0, 26.0) };
            return;
        }
        Self::fill_circle(g, color, x, y, 22.0);
        Self::draw_text_ex(
            self.body_font,
            g,
            letter,
            x,
            y,
            22.0,
            22.0,
            13.0,
            true,
            WHITE,
            STRING_ALIGN_CENTER,
            STRING_ALIGN_CENTER,
        );
    }

    fn draw_kbm_badge(&self, g: *mut c_void, x: f32, y: f32, key: &str, w: f32) {
        Self::fill_rect(g, PAPER_BG, x, y, w, 22.0);
        Self::stroke_rect(g, PAPER_BORDER, 1.2, x, y, w, 22.0);
        Self::draw_text_ex(
            self.body_font,
            g,
            key,
            x,
            y,
            w,
            22.0,
            11.5,
            true,
            INK_BLACK,
            STRING_ALIGN_CENTER,
            STRING_ALIGN_CENTER,
        );
    }

    fn draw_postcard(&self, g: *mut c_void, x: f32, y: f32, w: f32, h: f32, map_idx: usize) {
        let map = &PACIFIC_MAPS[map_idx.min(PACIFIC_MAPS.len() - 1)];
        Self::fill_rect(g, argb(75, 0, 0, 0), x + 5.0, y + 5.0, w, h);
        Self::fill_rect(g, PAPER_SOLID, x, y, w, h);
        Self::stroke_rect(g, PAPER_BORDER, 1.8, x, y, w, h);

        let img_h = (h * 0.54).min(230.0);
        let img = self.map_imgs[map_idx.min(self.map_imgs.len() - 1)];
        if !img.is_null() {
            unsafe { GdipDrawImageRect(g, img, x + 12.0, y + 12.0, w - 24.0, img_h) };
        } else {
            Self::fill_rect(g, SKY_FALLBACK, x + 12.0, y + 12.0, w - 24.0, img_h);
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.5, x + 12.0, y + 12.0, w - 24.0, img_h);

        let ty = y + 20.0 + img_h;
        self.draw_display(g, map.title, x + 14.0, ty, w - 28.0, 32.0, 26.0, INK_BLACK, STRING_ALIGN_NEAR);
        self.draw_italic_accent(
            g,
            &format!("{}  ·  {}", map.subtitle, map.mode_label),
            x + 14.0,
            ty + 34.0,
            w - 28.0,
            20.0,
            12.5,
            OLIVE_DARK,
        );
        self.draw_body(
            g,
            map.briefing,
            x + 14.0,
            ty + 58.0,
            w - 28.0,
            (h - (ty - y) - 66.0).max(50.0),
            13.5,
            false,
            INK_MUTED,
        );
    }

    pub fn render(&mut self, st: &InGameUiState, elapsed_secs: f32) {
        self.hit_rects.clear();

        // Update transition clocks (boot fade-in, screen crossfade, modal fade/settle)
        let boot_start = *self.first_render_time.get_or_insert(elapsed_secs);
        if st.screen != self.last_screen {
            self.last_screen = st.screen;
            self.screen_change_time = elapsed_secs;
        }
        if st.modal != self.last_modal {
            self.last_modal = st.modal;
            self.modal_change_time = elapsed_secs;
        }

        let ease_out = |t: f32| -> f32 {
            let c = t.clamp(0.0, 1.0);
            1.0 - (1.0 - c) * (1.0 - c) * (1.0 - c)
        };
        let boot_fade = ease_out((elapsed_secs - boot_start) / 0.45);
        let screen_fade = ease_out((elapsed_secs - self.screen_change_time) / 0.24);
        let modal_fade = ease_out((elapsed_secs - self.modal_change_time) / 0.18);

        // Smooth per-screen parallax targets for Zero & Corsair (matching `backgroundPlanes` in `main_nav.res`)
        let (target_zero, target_corsair) = match st.screen {
            Bf1943Screen::Main => ((118.0, 224.0), (718.0, 126.0)),
            Bf1943Screen::OfflineMatches => ((68.0, 256.0), (776.0, 102.0)),
            Bf1943Screen::StatsAwards => ((142.0, 240.0), (748.0, 148.0)),
            Bf1943Screen::HelpOptions => ((82.0, 198.0), (788.0, 152.0)),
            Bf1943Screen::Achievements => ((156.0, 252.0), (706.0, 112.0)),
        };
        self.zero_pos.0 += (target_zero.0 - self.zero_pos.0) * 0.14;
        self.zero_pos.1 += (target_zero.1 - self.zero_pos.1) * 0.14;
        self.corsair_pos.0 += (target_corsair.0 - self.corsair_pos.0) * 0.14;
        self.corsair_pos.1 += (target_corsair.1 - self.corsair_pos.1) * 0.14;

        // Gentle continuous sinusoidal Pacific air-drift
        let z_dx = (elapsed_secs * 0.55).sin() * 14.0 + (elapsed_secs * 1.15).cos() * 3.5;
        let z_dy = (elapsed_secs * 0.72).cos() * 8.0 + (elapsed_secs * 1.4).sin() * 2.0;
        let c_dx = (elapsed_secs * 0.48).cos() * 18.0 + (elapsed_secs * 0.95).sin() * 4.0;
        let c_dy = (elapsed_secs * 0.63).sin() * 10.0 + (elapsed_secs * 1.25).cos() * 2.5;

        // Fast memcpy of pre-baked 1080p backdrop (avoids full-screen GDI+ bicubic scaling every frame)
        if st.active_era == opfor_vfs::ModeId::Bfbc1 {
            self.pixels.copy_from_slice(&self.bc1_backdrop_pixels);
        } else {
            self.pixels.copy_from_slice(&self.backdrop_pixels);
        }

        let mut bmp = std::ptr::null_mut();
        let rc = unsafe {
            GdipCreateBitmapFromScan0(
                RENDER_W as i32,
                RENDER_H as i32,
                (RENDER_W * 4) as i32,
                PIXEL_FORMAT_32BPP_ARGB,
                self.pixels.as_mut_ptr(),
                &mut bmp,
            )
        };
        if rc != 0 || bmp.is_null() {
            return;
        }
        let mut g = std::ptr::null_mut();
        if unsafe { GdipGetImageGraphicsContext(bmp, &mut g) } != 0 || g.is_null() {
            unsafe { GdipDisposeImage(bmp) };
            return;
        }
        unsafe {
            GdipSetSmoothingMode(g, 4);
            // Bilinear (3) instead of HighQualityBicubic (7) for fast 60+ FPS live sprite draws
            GdipSetInterpolationMode(g, 3);
            GdipSetTextRenderingHint(g, 4);
            GdipScaleWorldTransform(g, RENDER_SCALE_F, RENDER_SCALE_F, 0);
        }

        // Dispatch Battlefield: Bad Company 1 native front-end menu when ModeId::Bfbc1 is active
        if st.active_era == opfor_vfs::ModeId::Bfbc1 {
            self.render_bfbc1(g, st, boot_fade, screen_fade);
            unsafe {
                GdipDeleteGraphics(g);
                GdipDisposeImage(bmp);
            }
            return;
        }

        // 1. Drifting A6M Zero + Banking F4U Corsair (sky + paper grain already blitted via backdrop_pixels)
        if !self.zero_img.is_null() {
            unsafe {
                GdipDrawImageRect(
                    g,
                    self.zero_img,
                    self.zero_pos.0 + z_dx,
                    self.zero_pos.1 + z_dy,
                    313.0,
                    331.0,
                );
            }
        }
        if !self.corsair_img.is_null() {
            unsafe {
                GdipDrawImageRect(
                    g,
                    self.corsair_img,
                    self.corsair_pos.0 + c_dx,
                    self.corsair_pos.1 + c_dy,
                    588.0,
                    498.0,
                );
            }
        }

        // 2. Render Active Screen
        match st.screen {
            Bf1943Screen::Main => self.render_main_screen(g, st, elapsed_secs),
            Bf1943Screen::OfflineMatches => self.render_offline_matches(g, st),
            Bf1943Screen::StatsAwards => self.render_stats_awards(g, st),
            Bf1943Screen::HelpOptions => self.render_help_options(g, st),
            Bf1943Screen::Achievements => self.render_achievements(g),
        }

        // 3. Modal Overlay (Quick Match / Host Private Match / Direct IP) with smooth fade + settle
        match st.modal {
            Bf1943Modal::None => {}
            Bf1943Modal::MatchSetup { is_private } => {
                self.hit_rects.clear();
                self.render_match_setup_modal(g, st, is_private, modal_fade);
            }
            Bf1943Modal::DirectIp => {
                self.hit_rects.clear();
                self.render_direct_ip_modal(g, st, modal_fade);
            }
        }

        // 3b. BF1943 Rotating Deploy / Wait Spinner when loading into a Pacific match
        if let Some(ref lvl) = st.pending_deploy_level {
            self.hit_rects.clear();
            self.render_deploy_spinner_modal(g, lvl, elapsed_secs);
        }

        // 4. Clean Floating Bottom Prompt Bar (Xbox 360 <-> Keyboard/Mouse)
        self.render_bottom_bar(g, st);

        // Subtle screen-change white/sky wash fade-in (smooth 0.24s transition)
        if screen_fade < 0.995 {
            let wash_a = ((1.0 - screen_fade) * 125.0) as u8;
            Self::fill_rect(g, argb(wash_a, 240, 242, 238), 0.0, 0.0, CANVAS_W as f32, CANVAS_H as f32);
        }

        unsafe {
            GdipDeleteGraphics(g);
            GdipDisposeImage(bmp);
        }

        // 5. Initial 0.45s boot fade-in only (paper grain is pre-baked into backdrop_pixels)
        let boot_scale = (boot_fade * 256.0).clamp(0.0, 256.0) as u32;
        if boot_scale < 256 {
            for px in self.pixels.chunks_exact_mut(4) {
                px[0] = ((u32::from(px[0]) * boot_scale) >> 8) as u8;
                px[1] = ((u32::from(px[1]) * boot_scale) >> 8) as u8;
                px[2] = ((u32::from(px[2]) * boot_scale) >> 8) as u8;
            }
        }
    }

    fn render_main_screen(&mut self, g: *mut c_void, st: &InGameUiState, elapsed_secs: f32) {
        // Exact Xbox 360 `main_nav.apt` coordinates: mc_logo at (335.3, 98.0)
        self.draw_bf1943_logo(g, 98.0, 1.0);

        // Exact Xbox 360 `startMc` centered paper menu stack (485.0, 313.0, 310.0 x 31.0/row)
        let items: &[&str] = &[
            "QUICK MATCH",
            "OFFLINE MATCHES",
            "CONNECT TO A MATCH",
            "HOST PRIVATE MATCH",
            "STATS & AWARDS",
            "HELP & OPTIONS",
            "ACHIEVEMENTS",
        ];

        let box_w = 310.0;
        let row_h = 31.0;
        let box_h = (items.len() as f32) * row_h;
        let box_x = 485.0;
        let box_y = 313.0;

        // Subtle drop shadow + translucent off-white paper fill (#BFFFFFFF)
        Self::fill_rect(g, argb(56, 0, 0, 0), box_x + 3.0, box_y + 3.0, box_w, box_h);
        Self::fill_rect(g, PAPER_BG, box_x, box_y, box_w, box_h);

        for (i, label) in items.iter().enumerate() {
            let y = box_y + (i as f32) * row_h;
            let selected = st.modal == Bf1943Modal::None && st.cursor_idx == i;
            if selected {
                Self::fill_rect(g, OLIVE_ACTIVE, box_x, y, box_w, row_h);
                Self::fill_rect(g, OLIVE_HI, box_x, y, box_w, 2.0);
                self.draw_display(
                    g,
                    label,
                    box_x + 13.0,
                    y + 1.5,
                    box_w - 24.0,
                    row_h,
                    18.5,
                    argb(190, 20, 30, 10),
                    STRING_ALIGN_NEAR,
                );
                self.draw_display(
                    g,
                    label,
                    box_x + 12.0,
                    y + 0.5,
                    box_w - 24.0,
                    row_h,
                    18.5,
                    WHITE,
                    STRING_ALIGN_NEAR,
                );
            } else {
                self.draw_display(
                    g,
                    label,
                    box_x + 12.0,
                    y + 0.5,
                    box_w - 24.0,
                    row_h,
                    18.5,
                    INK_BLACK,
                    STRING_ALIGN_NEAR,
                );
            }
            if i > 0 {
                Self::fill_rect(g, ROW_DIVIDER, box_x, y, box_w, 1.0);
            }
            self.hit_rects.push(HitRect {
                x: box_x,
                y,
                w: box_w,
                h: row_h,
                action: MenuClickAction::HoverAndActivate(i),
            });
        }

        // Crisp 1.5px black outer frame matching `main_nav.geo`
        Self::stroke_rect(g, PAPER_BORDER, 1.5, box_x, box_y, box_w, box_h);

        // Authentic `theticker` strip from `main_nav.res` Frame 1 (`d=118 'theticker' @ (0.0, 642.0)`)
        let ty = 642.0;
        Self::fill_rect(g, argb(176, 245, 245, 242), 0.0, ty, CANVAS_W as f32, 24.0);
        Self::fill_rect(g, argb(180, 18, 20, 18), 0.0, ty, CANVAS_W as f32, 1.0);
        Self::fill_rect(g, argb(180, 18, 20, 18), 0.0, ty + 23.0, CANVAS_W as f32, 1.0);
        let ticker_msg = "BATTLEFIELD 1943 PACIFIC THEATRE  ·  WAKE ISLAND  ·  GUADALCANAL  ·  IWO JIMA  ·  CORAL SEA AIR SUPERIORITY  ·  ALL 12 XBLA ACHIEVEMENTS TRACKED OFFLINE   ★   ";
        let span = 1280.0;
        let scroll = (elapsed_secs * 44.0) % span;
        self.draw_body(g, ticker_msg, -scroll, ty + 3.0, span + 200.0, 20.0, 12.5, true, INK_BLACK);
        self.draw_body(g, ticker_msg, span - scroll, ty + 3.0, span + 200.0, 20.0, 12.5, true, INK_BLACK);
    }

    fn render_subpage_header(&self, g: *mut c_void, title: &str, right_badge: Option<&str>) {
        // Authentic Xbox 360 BF1943 title-safe header: native unstretched `headline_bar.png` (509x64)
        // at x=112, y=136 leaving the top 0..136px of Pacific sky & aircraft completely open.
        if !self.headline_bar_img.is_null() {
            unsafe { GdipDrawImageRect(g, self.headline_bar_img, 112.0, 136.0, 509.0, 64.0) };
        } else {
            Self::fill_rect(g, OLIVE_ACTIVE, 112.0, 142.0, 509.0, 48.0);
            Self::fill_rect(g, OLIVE_HI, 112.0, 142.0, 509.0, 2.0);
            Self::stroke_rect(g, PAPER_BORDER, 1.5, 112.0, 142.0, 509.0, 48.0);
        }
        self.draw_display(g, title, 132.0, 141.0, 468.0, 50.0, 26.0, WHITE, STRING_ALIGN_NEAR);

        if let Some(badge) = right_badge {
            Self::fill_rect(g, PAPER_SOLID, 768.0, 162.0, 400.0, 32.0);
            Self::stroke_rect(g, PAPER_BORDER, 1.5, 768.0, 162.0, 400.0, 32.0);
            self.draw_display(g, badge, 778.0, 162.0, 380.0, 32.0, 16.5, OLIVE_DARK, STRING_ALIGN_FAR);
        }
    }

    fn render_offline_matches(&mut self, g: *mut c_void, st: &InGameUiState) {
        self.render_subpage_header(g, "OFFLINE MATCHES", Some("PACIFIC SERVER BROWSER  ·  LOCAL"));

        // Left Paper Table Panel aligned to BF1943 title-safe grid (x=112..752, y=213..615)
        let tx = 112.0;
        let ty = 213.0;
        let tw = 640.0;
        let th = 402.0;
        Self::fill_rect(g, argb(65, 0, 0, 0), tx + 4.0, ty + 4.0, tw, th);
        Self::fill_rect(g, PAPER_SOLID, tx, ty, tw, th);
        Self::stroke_rect(g, PAPER_BORDER, 1.5, tx, ty, tw, th);

        // Compact Column Header Bar (30px) + crisp 2px horizontal rule (matching `leaderboard`)
        Self::fill_rect(g, PAPER_ALT, tx + 1.0, ty + 1.0, tw - 2.0, 30.0);
        Self::fill_rect(g, PAPER_BORDER, tx, ty + 31.0, tw, 2.0);
        self.draw_body(g, "SERVER NAME", tx + 12.0, ty + 7.0, 250.0, 18.0, 12.0, true, INK_BLACK);
        self.draw_body(g, "MAP", tx + 276.0, ty + 7.0, 114.0, 18.0, 12.0, true, INK_BLACK);
        self.draw_body(g, "MODE", tx + 394.0, ty + 7.0, 118.0, 18.0, 12.0, true, INK_BLACK);
        self.draw_body(g, "PLY", tx + 516.0, ty + 7.0, 48.0, 18.0, 12.0, true, INK_BLACK);
        self.draw_body(g, "PING", tx + 568.0, ty + 7.0, 48.0, 18.0, 12.0, true, INK_BLACK);

        // Right scroll rail with ▲ / ▼ arrows (matching `friendsList` arrowUp/arrowDown)
        let rail_x = tx + tw - 22.0;
        Self::fill_rect(g, ROW_DIVIDER, rail_x, ty + 33.0, 1.0, th - 66.0);
        Self::draw_text_ex(self.body_font, g, "▲", rail_x + 1.0, ty + 36.0, 20.0, 18.0, 10.0, true, INK_MUTED, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
        Self::draw_text_ex(self.body_font, g, "▼", rail_x + 1.0, ty + th - 54.0, 20.0, 18.0, 10.0, true, INK_MUTED, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);

        // Compact 31px BF1943 data rows (matching `startMc`/`cm` 31px row pitch)
        let row_h = 31.0;
        let row_w = tw - 24.0;
        for (i, m) in PACIFIC_MAPS.iter().enumerate() {
            let y = ty + 33.0 + (i as f32) * row_h;
            let selected = st.cursor_idx == i;
            let bg = if selected {
                OLIVE_ACTIVE
            } else if i % 2 == 0 {
                PAPER_SOLID
            } else {
                PAPER_ALT
            };
            Self::fill_rect(g, bg, tx + 1.0, y, row_w, row_h);
            if selected {
                Self::fill_rect(g, OLIVE_HI, tx + 1.0, y, row_w, 2.0);
            }
            Self::fill_rect(g, ROW_DIVIDER, tx, y + row_h - 1.0, row_w + 1.0, 1.0);
            let c = if selected { WHITE } else { INK_BLACK };
            let c2 = if selected { WHITE } else { OLIVE_DARK };
            if !self.flag_us_img.is_null() {
                unsafe { GdipDrawImageRect(g, self.flag_us_img, tx + 10.0, y + 8.0, 20.0, 14.0) };
            }
            if !self.flag_jp_img.is_null() {
                unsafe { GdipDrawImageRect(g, self.flag_jp_img, tx + 33.0, y + 8.0, 20.0, 14.0) };
            }
            self.draw_body(g, m.server_name, tx + 58.0, y + 7.0, 214.0, 18.0, 12.0, true, c);
            self.draw_body(g, m.title, tx + 276.0, y + 7.0, 114.0, 18.0, 12.5, true, c);
            self.draw_body(g, m.mode_key, tx + 394.0, y + 7.0, 118.0, 18.0, 11.5, false, c);
            self.draw_body(g, "1/24", tx + 516.0, y + 7.0, 48.0, 18.0, 12.0, false, c);
            self.draw_body(g, "LOCAL", tx + 566.0, y + 7.0, 48.0, 18.0, 11.5, true, c2);

            self.hit_rects.push(HitRect {
                x: tx,
                y,
                w: row_w,
                h: row_h,
                action: MenuClickAction::HoverAndActivate(i),
            });
        }

        // Bottom summary strip inside table panel (matching `leaderboard` bottom divider)
        Self::fill_rect(g, PAPER_BORDER, tx, ty + th - 33.0, tw, 1.2);
        Self::fill_rect(g, PAPER_ALT, tx + 1.0, ty + th - 32.0, tw - 2.0, 31.0);
        self.draw_body(
            g,
            "4 PACIFIC THEATER ISLANDS AVAILABLE  ·  SELECT ANY SERVER TO DEPLOY IMMEDIATELY",
            tx + 14.0,
            ty + th - 24.0,
            tw - 28.0,
            18.0,
            11.5,
            true,
            INK_MUTED,
        );

        // Right Postcard Preview flush with table inside 112..1168 title-safe area
        self.draw_postcard(g, 768.0, 213.0, 400.0, 402.0, st.bf1943_selected_map);
    }

    fn render_match_setup_modal(&mut self, g: *mut c_void, st: &InGameUiState, is_private: bool, fade: f32) {
        let scrim_a = (127.0 * fade).clamp(0.0, 127.0) as u8;
        Self::fill_rect(g, argb(scrim_a, 0, 0, 0), 0.0, 0.0, CANVAS_W as f32, CANVAS_H as f32);

        let slide_y = (1.0 - fade) * 14.0;
        let mx = 266.0;
        let my = 128.0 + slide_y;
        let mw = 748.0;
        let mh = 468.0;

        Self::fill_rect(g, argb(110, 0, 0, 0), mx + 8.0, my + 8.0, mw, mh);
        Self::fill_rect(g, PAPER_SOLID, mx, my, mw, mh);
        Self::stroke_rect(g, PAPER_BORDER, 2.0, mx, my, mw, mh);

        // Olive Green Header Strip with highlight bevel
        Self::fill_rect(g, OLIVE_ACTIVE, mx, my, mw, 48.0);
        Self::fill_rect(g, OLIVE_HI, mx, my, mw, 2.5);
        Self::fill_rect(g, PAPER_BORDER, mx, my + 48.0, mw, 1.5);
        let title = if is_private {
            "HOST PRIVATE MATCH"
        } else {
            "QUICK MATCH"
        };
        self.draw_display(g, title, mx + 20.0, my, mw - 120.0, 48.0, 25.0, WHITE, STRING_ALIGN_NEAR);
        if !self.flag_us_img.is_null() {
            unsafe { GdipDrawImageRect(g, self.flag_us_img, mx + mw - 86.0, my + 12.0, 32.0, 24.0) };
        }
        if !self.flag_jp_img.is_null() {
            unsafe { GdipDrawImageRect(g, self.flag_jp_img, mx + mw - 48.0, my + 12.0, 32.0, 24.0) };
        }

        let map_idx = st.popup_selected_map_index();
        let map = &PACIFIC_MAPS[map_idx];

        // Right-hand Island Preview Card inside Modal
        let px = mx + 432.0;
        let py = my + 66.0;
        let pw = 294.0;
        Self::fill_rect(g, PAPER_ALT, px, py, pw, 316.0);
        Self::stroke_rect(g, PAPER_BORDER, 1.5, px, py, pw, 316.0);
        let img = self.map_imgs[map_idx.min(self.map_imgs.len() - 1)];
        if !img.is_null() {
            unsafe { GdipDrawImageRect(g, img, px + 10.0, py + 10.0, pw - 20.0, 154.0) };
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.2, px + 10.0, py + 10.0, pw - 20.0, 154.0);
        self.draw_display(g, map.title, px + 12.0, py + 170.0, pw - 24.0, 28.0, 21.0, INK_BLACK, STRING_ALIGN_NEAR);
        self.draw_body(g, map.mode_label, px + 12.0, py + 200.0, pw - 24.0, 20.0, 12.0, true, OLIVE_DARK);
        self.draw_body(g, map.briefing, px + 12.0, py + 222.0, pw - 24.0, 84.0, 12.0, false, INK_MUTED);

        // Left Selector Rows (BF1943 crisp 32px row pitch)
        self.draw_modal_selector(
            g,
            mx + 22.0,
            my + 72.0,
            390.0,
            0,
            st.modal_cursor == 0,
            "GAME MODE",
            st.popup_mode().label(),
        );
        self.draw_modal_selector(
            g,
            mx + 22.0,
            my + 146.0,
            390.0,
            1,
            st.modal_cursor == 1,
            "MAP",
            map.title,
        );

        // Row 2: ENTER GAME / START PRIVATE MATCH (32px BF1943 button)
        let btn_y = my + 244.0;
        let sel_enter = st.modal_cursor == 2;
        Self::fill_rect(
            g,
            if sel_enter { OLIVE_ACTIVE } else { PAPER_ALT },
            mx + 22.0,
            btn_y,
            390.0,
            32.0,
        );
        if sel_enter {
            Self::fill_rect(g, OLIVE_HI, mx + 22.0, btn_y, 390.0, 2.0);
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.5, mx + 22.0, btn_y, 390.0, 32.0);
        let enter_lbl = if is_private {
            "START PRIVATE MATCH"
        } else {
            "ENTER GAME"
        };
        self.draw_display(
            g,
            enter_lbl,
            mx + 34.0,
            btn_y,
            366.0,
            32.0,
            18.5,
            if sel_enter { WHITE } else { INK_BLACK },
            STRING_ALIGN_NEAR,
        );
        self.hit_rects.push(HitRect {
            x: mx + 22.0,
            y: btn_y,
            w: 390.0,
            h: 32.0,
            action: MenuClickAction::ModalRowActivate(2),
        });

        // Row 3: CANCEL (32px BF1943 button)
        let cancel_y = my + 284.0;
        let sel_cancel = st.modal_cursor == 3;
        Self::fill_rect(
            g,
            if sel_cancel { OLIVE_ACTIVE } else { PAPER_ALT },
            mx + 22.0,
            cancel_y,
            390.0,
            32.0,
        );
        if sel_cancel {
            Self::fill_rect(g, OLIVE_HI, mx + 22.0, cancel_y, 390.0, 2.0);
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.4, mx + 22.0, cancel_y, 390.0, 32.0);
        self.draw_display(
            g,
            "CANCEL",
            mx + 34.0,
            cancel_y,
            366.0,
            32.0,
            18.0,
            if sel_cancel { WHITE } else { INK_BLACK },
            STRING_ALIGN_NEAR,
        );
        self.hit_rects.push(HitRect {
            x: mx + 22.0,
            y: cancel_y,
            w: 390.0,
            h: 32.0,
            action: MenuClickAction::GoBack,
        });

        let note = if is_private {
            "Starts an offline locally hosted multiplayer match on this PC."
        } else {
            "Use ◀ / ▶ or Left / Right arrows to select Game Mode and Map."
        };
        self.draw_body(g, note, mx + 22.0, my + 412.0, mw - 44.0, 22.0, 13.0, false, INK_MUTED);
    }

    fn render_deploy_spinner_modal(&self, g: *mut c_void, level_path: &str, elapsed_secs: f32) {
        Self::fill_rect(g, argb(145, 0, 0, 0), 0.0, 0.0, CANVAS_W as f32, CANVAS_H as f32);
        let mw = 460.0;
        let mh = 164.0;
        let mx = ((CANVAS_W as f32) - mw) * 0.5;
        let my = ((CANVAS_H as f32) - mh) * 0.5;
        Self::fill_rect(g, argb(110, 0, 0, 0), mx + 6.0, my + 6.0, mw, mh);
        Self::fill_rect(g, PAPER_SOLID, mx, my, mw, mh);
        Self::fill_rect(g, OLIVE_ACTIVE, mx, my, mw, 36.0);
        Self::fill_rect(g, OLIVE_HI, mx, my, mw, 2.0);
        Self::stroke_rect(g, PAPER_BORDER, 1.8, mx, my, mw, mh);
        self.draw_display(g, "DEPLOYING TO PACIFIC THEATRE", mx + 16.0, my, mw - 32.0, 36.0, 19.0, WHITE, STRING_ALIGN_NEAR);

        // 12-dot BF1943 rotating wait indicator (matching `main_popups.res` ch=37 53-frame spinner)
        let cx = mx + 68.0;
        let cy = my + 100.0;
        let radius = 22.0;
        let head = ((elapsed_secs * 12.0) as i32).rem_euclid(12);
        for i in 0..12 {
            let ang = (i as f32) * std::f32::consts::TAU / 12.0;
            let dist = ((head - i).rem_euclid(12)) as f32;
            let alpha = ((1.0 - dist / 12.0).powf(1.6) * 235.0 + 20.0) as u8;
            let dot_d = if dist < 2.0 { 8.0 } else { 6.0 };
            let px = cx + ang.cos() * radius - dot_d * 0.5;
            let py = cy + ang.sin() * radius - dot_d * 0.5;
            Self::fill_circle(g, argb(alpha, 112, 143, 82), px, py, dot_d);
        }

        let map_title = PACIFIC_MAPS
            .iter()
            .find(|m| m.level_path.eq_ignore_ascii_case(level_path))
            .map_or(level_path, |m| m.title);
        self.draw_display(g, map_title, mx + 114.0, my + 74.0, mw - 130.0, 28.0, 21.0, INK_BLACK, STRING_ALIGN_NEAR);
        self.draw_body(g, "LOADING ISLAND ASSETS...", mx + 114.0, my + 106.0, mw - 130.0, 20.0, 12.5, true, OLIVE_DARK);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_modal_selector(
        &mut self,
        g: *mut c_void,
        x: f32,
        y: f32,
        w: f32,
        row: usize,
        selected: bool,
        label: &str,
        value: &str,
    ) {
        self.draw_body(g, label, x, y, w, 18.0, 12.0, true, INK_MUTED);
        let by = y + 20.0;
        let bh = 32.0;
        Self::fill_rect(g, if selected { OLIVE_ACTIVE } else { PAPER_ALT }, x, by, w, bh);
        if selected {
            Self::fill_rect(g, OLIVE_HI, x, by, w, 2.0);
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.5, x, by, w, bh);

        let fg = if selected { WHITE } else { INK_BLACK };

        // Left arrow
        Self::fill_rect(g, argb(40, 0, 0, 0), x + 1.0, by + 1.0, 34.0, bh - 2.0);
        Self::draw_text_ex(self.body_font, g, "◀", x + 1.0, by, 34.0, bh, 14.0, true, fg, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
        self.hit_rects.push(HitRect {
            x,
            y: by,
            w: 36.0,
            h: bh,
            action: MenuClickAction::ModalCycle(row, -1),
        });

        // Value
        self.draw_display(g, value, x + 40.0, by, w - 80.0, bh, 18.0, fg, STRING_ALIGN_CENTER);

        // Right arrow
        Self::fill_rect(g, argb(40, 0, 0, 0), x + w - 35.0, by + 1.0, 34.0, bh - 2.0);
        Self::draw_text_ex(self.body_font, g, "▶", x + w - 35.0, by, 34.0, bh, 14.0, true, fg, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
        self.hit_rects.push(HitRect {
            x: x + w - 36.0,
            y: by,
            w: 36.0,
            h: bh,
            action: MenuClickAction::ModalCycle(row, 1),
        });
    }

    fn render_direct_ip_modal(&mut self, g: *mut c_void, st: &InGameUiState, fade: f32) {
        let scrim_a = (127.0 * fade).clamp(0.0, 127.0) as u8;
        Self::fill_rect(g, argb(scrim_a, 0, 0, 0), 0.0, 0.0, CANVAS_W as f32, CANVAS_H as f32);

        let slide_y = (1.0 - fade) * 14.0;
        let mx = 310.0;
        let my = 176.0 + slide_y;
        let mw = 660.0;
        let mh = 344.0;

        Self::fill_rect(g, argb(110, 0, 0, 0), mx + 8.0, my + 8.0, mw, mh);
        Self::fill_rect(g, PAPER_SOLID, mx, my, mw, mh);
        Self::stroke_rect(g, PAPER_BORDER, 2.0, mx, my, mw, mh);

        Self::fill_rect(g, OLIVE_ACTIVE, mx, my, mw, 48.0);
        Self::fill_rect(g, OLIVE_HI, mx, my, mw, 2.5);
        Self::fill_rect(g, PAPER_BORDER, mx, my + 48.0, mw, 1.5);
        self.draw_display(g, "CONNECT TO A MATCH — DIRECT IP", mx + 20.0, my, mw - 40.0, 48.0, 23.0, WHITE, STRING_ALIGN_NEAR);

        self.draw_body(
            g,
            "ENTER HOST IP ADDRESS AND PORT (TYPE WITH KEYBOARD):",
            mx + 26.0,
            my + 68.0,
            mw - 52.0,
            20.0,
            12.5,
            true,
            INK_MUTED,
        );

        // Row 0: IP input
        let sel_ip = st.modal_cursor == 0;
        Self::fill_rect(g, WHITE, mx + 26.0, my + 96.0, mw - 52.0, 46.0);
        Self::stroke_rect(
            g,
            if sel_ip { OLIVE_ACTIVE } else { PAPER_BORDER },
            if sel_ip { 2.5 } else { 1.5 },
            mx + 26.0,
            my + 96.0,
            mw - 52.0,
            46.0,
        );
        let ip_disp = format!("{}_", st.direct_ip);
        self.draw_body(g, &ip_disp, mx + 40.0, my + 108.0, mw - 80.0, 24.0, 18.0, true, INK_BLACK);
        self.hit_rects.push(HitRect {
            x: mx + 26.0,
            y: my + 96.0,
            w: mw - 52.0,
            h: 46.0,
            action: MenuClickAction::ModalRowActivate(0),
        });

        // Status notice
        if let Some(status) = st.direct_ip_status {
            Self::fill_rect(g, argb(255, 245, 224, 218), mx + 26.0, my + 158.0, mw - 52.0, 44.0);
            Self::stroke_rect(g, argb(255, 170, 56, 44), 1.5, mx + 26.0, my + 158.0, mw - 52.0, 44.0);
            self.draw_body(g, status, mx + 38.0, my + 170.0, mw - 76.0, 24.0, 11.5, true, argb(255, 130, 30, 24));
        } else {
            self.draw_body(
                g,
                "Direct IP multiplayer transport is reserved for a future update.",
                mx + 26.0,
                my + 168.0,
                mw - 52.0,
                22.0,
                13.0,
                false,
                INK_MUTED,
            );
        }

        // Row 1: CONNECT (32px BF1943 button)
        let sel_conn = st.modal_cursor == 1;
        Self::fill_rect(
            g,
            if sel_conn { OLIVE_ACTIVE } else { PAPER_ALT },
            mx + 26.0,
            my + 234.0,
            290.0,
            32.0,
        );
        if sel_conn {
            Self::fill_rect(g, OLIVE_HI, mx + 26.0, my + 234.0, 290.0, 2.0);
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.5, mx + 26.0, my + 234.0, 290.0, 32.0);
        self.draw_display(
            g,
            "CONNECT",
            mx + 26.0,
            my + 234.0,
            290.0,
            32.0,
            18.5,
            if sel_conn { WHITE } else { INK_BLACK },
            STRING_ALIGN_CENTER,
        );
        self.hit_rects.push(HitRect {
            x: mx + 26.0,
            y: my + 234.0,
            w: 290.0,
            h: 32.0,
            action: MenuClickAction::ModalRowActivate(1),
        });

        // Row 2: CANCEL (32px BF1943 button)
        let sel_cancel = st.modal_cursor == 2;
        Self::fill_rect(
            g,
            if sel_cancel { OLIVE_ACTIVE } else { PAPER_ALT },
            mx + 344.0,
            my + 234.0,
            290.0,
            32.0,
        );
        if sel_cancel {
            Self::fill_rect(g, OLIVE_HI, mx + 344.0, my + 234.0, 290.0, 2.0);
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.5, mx + 344.0, my + 234.0, 290.0, 32.0);
        self.draw_display(
            g,
            "CANCEL",
            mx + 344.0,
            my + 234.0,
            290.0,
            32.0,
            18.5,
            if sel_cancel { WHITE } else { INK_BLACK },
            STRING_ALIGN_CENTER,
        );
        self.hit_rects.push(HitRect {
            x: mx + 344.0,
            y: my + 234.0,
            w: 290.0,
            h: 32.0,
            action: MenuClickAction::GoBack,
        });
    }

    fn stat_val(stats: &BTreeMap<String, f32>, keys: &[&str]) -> f32 {
        for k in keys {
            if let Some(&v) = stats.get(*k) {
                return v;
            }
        }
        0.0
    }

    fn render_stats_awards(&mut self, g: *mut c_void, st: &InGameUiState) {
        self.render_subpage_header(g, "STATS & AWARDS", Some("OFFLINE CAREER RECORD"));

        let stats = crate::persistence::snapshot_stats();
        let score = Self::stat_val(&stats, &["score", "sc_general", "sc_award"]) as u32;
        let kills = Self::stat_val(&stats, &["kills", "k_total"]) as u32;
        let deaths = Self::stat_val(&stats, &["deaths", "d_total"]) as u32;
        let wins = Self::stat_val(&stats, &["wins", "r_wins"]) as u32;
        let rounds = Self::stat_val(&stats, &["rounds", "r_played", "games"]) as u32;
        let headshots = Self::stat_val(&stats, &["headshots", "k_hs"]) as u32;
        let flags = Self::stat_val(&stats, &["flags", "flag_captures", "sc_objective"]) as u32;
        let kd = if deaths == 0 { kills as f32 } else { kills as f32 / deaths as f32 };
        let rank = (score / 1500 + 1).min(50);
        let rank_frac = ((score % 1500) as f32 / 1500.0).clamp(0.0, 1.0);

        // Left Career Stats Paper Card (x=112..528, y=213..615)
        Self::fill_rect(g, argb(65, 0, 0, 0), 116.0, 217.0, 416.0, 402.0);
        Self::fill_rect(g, PAPER_SOLID, 112.0, 213.0, 416.0, 402.0);
        Self::stroke_rect(g, PAPER_BORDER, 1.5, 112.0, 213.0, 416.0, 402.0);

        let rank_idx = (rank as usize).saturating_sub(1).min(self.rank_imgs.len().saturating_sub(1));
        if let Some(&r_img) = self.rank_imgs.get(rank_idx) {
            if !r_img.is_null() {
                unsafe { GdipDrawImageRect(g, r_img, 456.0, 222.0, 56.0, 56.0) };
            }
        }

        let callsign = crate::profile::current_name();
        self.draw_display(
            g,
            if callsign.is_empty() { "SOLDIER" } else { &callsign },
            128.0,
            222.0,
            320.0,
            30.0,
            24.0,
            INK_BLACK,
            STRING_ALIGN_NEAR,
        );
        self.draw_body(
            g,
            &format!("RANK {:02}  ·  SAVED IN user/stats.txt", rank),
            128.0,
            254.0,
            320.0,
            18.0,
            11.5,
            true,
            OLIVE_DARK,
        );
        // BF1943 `rankProgress` olive bar
        Self::fill_rect(g, PAPER_ALT, 128.0, 276.0, 316.0, 10.0);
        Self::fill_rect(g, OLIVE_ACTIVE, 128.0, 276.0, (316.0 * rank_frac).max(8.0), 10.0);
        Self::stroke_rect(g, PAPER_BORDER, 1.0, 128.0, 276.0, 316.0, 10.0);

        let rows = [
            ("TOTAL SCORE", format!("{score}")),
            ("KILLS", format!("{kills}")),
            ("DEATHS", format!("{deaths}")),
            ("K/D RATIO", format!("{kd:.2}")),
            ("HEADSHOTS", format!("{headshots}")),
            ("MATCHES PLAYED", format!("{rounds}")),
            ("MATCHES WON", format!("{wins}")),
            ("FLAGS CAPTURED", format!("{flags}")),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = 298.0 + (i as f32) * 37.0;
            if i % 2 == 0 {
                Self::fill_rect(g, PAPER_ALT, 124.0, y, 392.0, 36.0);
            }
            Self::fill_rect(g, ROW_DIVIDER, 124.0, y + 36.0, 392.0, 1.0);
            self.draw_body(g, k, 134.0, y + 9.0, 210.0, 18.0, 12.5, true, INK_MUTED);
            self.draw_display(g, v, 350.0, y, 154.0, 36.0, 17.5, INK_BLACK, STRING_ALIGN_FAR);
        }

        // Right BF1943 Postcard Stamps Card (x=544..1168, y=213..615)
        Self::fill_rect(g, argb(65, 0, 0, 0), 548.0, 217.0, 624.0, 402.0);
        Self::fill_rect(g, PAPER_SOLID, 544.0, 213.0, 624.0, 402.0);
        Self::stroke_rect(g, PAPER_BORDER, 1.5, 544.0, 213.0, 624.0, 402.0);
        if !self.subheadline_bar_img.is_null() {
            unsafe { GdipDrawImageRect(g, self.subheadline_bar_img, 558.0, 223.0, 257.0, 38.0) };
            self.draw_display(g, "POSTCARDS & AWARDS", 570.0, 223.0, 236.0, 36.0, 18.0, WHITE, STRING_ALIGN_NEAR);
        } else {
            self.draw_display(g, "POSTCARDS & AWARDS", 560.0, 225.0, 300.0, 28.0, 20.0, INK_BLACK, STRING_ALIGN_NEAR);
        }

        let stamps: &[(&str, &str, u32, u32, &str)] = &[
            ("RIFLEMAN BADGE", "Score 25+ kills in combat", kills, 25, "Awarded for sustained rifleman combat effectiveness across the Pacific theatre."),
            ("SHARPSHOOTER", "Score 10+ headshots", headshots, 10, "Awarded for precision marksmanship with infantry rifles and sniper optics."),
            ("ISLAND CONQUEROR", "Capture 5+ enemy flags", flags, 5, "Awarded for spearheading amphibious assaults and capturing strategic control points."),
            ("PACIFIC VETERAN", "Complete 10+ matches", rounds, 10, "Awarded to seasoned Pacific veterans who have served across ten full engagements."),
            ("VICTORY AT SEA", "Win 5+ Pacific matches", wins, 5, "Awarded for leading USMC or IJN forces to five decisive island victories."),
            ("HIGH COMMAND", "Earn 5,000+ career score", score, 5000, "Awarded for distinguished leadership and 5,000 cumulative career score."),
        ];
        for (i, &(title, desc, cur, goal, _flavor)) in stamps.iter().enumerate() {
            let unlocked = cur >= goal;
            let focused = st.cursor_idx == i;
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let sx = 560.0 + col * 302.0;
            let sy = 272.0 + row * 112.0;
            Self::fill_rect(
                g,
                if focused {
                    OLIVE_ACTIVE
                } else if unlocked {
                    WHITE
                } else {
                    PAPER_ALT
                },
                sx,
                sy,
                290.0,
                102.0,
            );
            if focused {
                Self::fill_rect(g, OLIVE_HI, sx, sy, 290.0, 2.5);
            }
            Self::stroke_rect(
                g,
                if focused {
                    PAPER_BORDER
                } else if unlocked {
                    OLIVE_ACTIVE
                } else {
                    ROW_DIVIDER
                },
                if focused || unlocked { 2.0 } else { 1.0 },
                sx,
                sy,
                290.0,
                102.0,
            );
            let fg_t = if focused { WHITE } else { INK_BLACK };
            let fg_d = if focused { argb(240, 240, 245, 232) } else { INK_MUTED };
            self.draw_display(g, title, sx + 12.0, sy + 8.0, 266.0, 24.0, 16.5, fg_t, STRING_ALIGN_NEAR);
            self.draw_body(g, desc, sx + 12.0, sy + 36.0, 266.0, 20.0, 12.0, false, fg_d);
            let badge_col = if focused {
                PAPER_BORDER
            } else if unlocked {
                OLIVE_ACTIVE
            } else {
                argb(255, 140, 142, 134)
            };
            let badge_txt = if unlocked { "AWARDED" } else { "LOCKED" };
            Self::fill_rect(g, badge_col, sx + 12.0, sy + 66.0, 88.0, 22.0);
            Self::draw_text_ex(self.body_font, g, badge_txt, sx + 12.0, sy + 66.0, 88.0, 22.0, 10.5, true, WHITE, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
            self.draw_body(
                g,
                &format!("{} / {}", cur.min(goal), goal),
                sx + 112.0,
                sy + 69.0,
                160.0,
                18.0,
                11.5,
                true,
                fg_d,
            );
            self.hit_rects.push(HitRect {
                x: sx,
                y: sy,
                w: 290.0,
                h: 102.0,
                action: MenuClickAction::HoverAndActivate(i),
            });
        }

        // Authentic BF1943 `zoomBg` postcard detail overlay when an award tile is clicked/activated
        if st.stats_zoom_open {
            self.hit_rects.clear();
            let idx = st.cursor_idx.min(stamps.len() - 1);
            let (title, desc, cur, goal, flavor) = stamps[idx];
            let unlocked = cur >= goal;
            let frac = (cur as f32 / goal.max(1) as f32).clamp(0.0, 1.0);

            Self::fill_rect(g, argb(145, 0, 0, 0), 0.0, 0.0, CANVAS_W as f32, CANVAS_H as f32);
            let zx = 340.0;
            let zy = 186.0;
            let zw = 600.0;
            let zh = 348.0;
            Self::fill_rect(g, argb(110, 0, 0, 0), zx + 8.0, zy + 8.0, zw, zh);
            Self::fill_rect(g, PAPER_SOLID, zx, zy, zw, zh);
            Self::fill_rect(g, OLIVE_ACTIVE, zx, zy, zw, 44.0);
            Self::fill_rect(g, OLIVE_HI, zx, zy, zw, 2.5);
            Self::stroke_rect(g, PAPER_BORDER, 2.0, zx, zy, zw, zh);
            self.draw_display(g, title, zx + 20.0, zy, zw - 40.0, 44.0, 23.0, WHITE, STRING_ALIGN_NEAR);

            let map_img = self.map_imgs[idx % self.map_imgs.len()];
            if !map_img.is_null() {
                unsafe { GdipDrawImageRect(g, map_img, zx + 20.0, zy + 58.0, 236.0, 146.0) };
            }
            Self::stroke_rect(g, PAPER_BORDER, 1.5, zx + 20.0, zy + 58.0, 236.0, 146.0);

            self.draw_italic_accent(
                g,
                if unlocked { "POSTCARD AWARDED" } else { "AWARD IN PROGRESS" },
                zx + 272.0,
                zy + 60.0,
                308.0,
                22.0,
                13.5,
                if unlocked { OLIVE_DARK } else { INK_MUTED },
            );
            self.draw_body(g, desc, zx + 272.0, zy + 88.0, 308.0, 24.0, 14.0, true, INK_BLACK);
            self.draw_body(g, flavor, zx + 272.0, zy + 118.0, 308.0, 86.0, 13.0, false, INK_MUTED);

            // Progress bar
            Self::fill_rect(g, PAPER_ALT, zx + 20.0, zy + 224.0, zw - 40.0, 18.0);
            Self::fill_rect(g, OLIVE_ACTIVE, zx + 20.0, zy + 224.0, (zw - 40.0) * frac, 18.0);
            Self::stroke_rect(g, PAPER_BORDER, 1.2, zx + 20.0, zy + 224.0, zw - 40.0, 18.0);
            self.draw_body(
                g,
                &format!("PROGRESS: {} / {} ({:.0}%)", cur.min(goal), goal, frac * 100.0),
                zx + 20.0,
                zy + 250.0,
                zw - 40.0,
                20.0,
                12.5,
                true,
                INK_BLACK,
            );

            // Close button (32px)
            Self::fill_rect(g, OLIVE_ACTIVE, zx + 20.0, zy + 294.0, 200.0, 32.0);
            Self::fill_rect(g, OLIVE_HI, zx + 20.0, zy + 294.0, 200.0, 2.0);
            Self::stroke_rect(g, PAPER_BORDER, 1.5, zx + 20.0, zy + 294.0, 200.0, 32.0);
            self.draw_display(g, "CLOSE POSTCARD", zx + 20.0, zy + 294.0, 200.0, 32.0, 17.5, WHITE, STRING_ALIGN_CENTER);
            self.hit_rects.push(HitRect {
                x: zx + 20.0,
                y: zy + 294.0,
                w: 200.0,
                h: 32.0,
                action: MenuClickAction::GoBack,
            });
        }
    }

    fn render_achievements(&mut self, g: *mut c_void) {
        let stats = crate::persistence::snapshot_stats();
        let kills = Self::stat_val(&stats, &["kills", "k_total"]) as u32;
        let wins = Self::stat_val(&stats, &["wins", "r_wins"]) as u32;
        let rounds = Self::stat_val(&stats, &["rounds", "r_played", "games"]) as u32;
        let flags = Self::stat_val(&stats, &["flags", "flag_captures"]) as u32;
        let defends = Self::stat_val(&stats, &["flag_defends", "defends"]) as u32;
        let melee = Self::stat_val(&stats, &["melee_kills", "k_knife"]) as u32;
        let veh_kills = Self::stat_val(&stats, &["vehicle_kills", "k_vehicle"]) as u32;
        let air_time = Self::stat_val(&stats, &["air_time", "t_plane"]) as u32;
        let chute = Self::stat_val(&stats, &["parachute_used", "chute"]) as u32;

        // All 12 original Battlefield 1943 Xbox 360 XBLA Achievements (200G total)
        let achievements: &[(&str, &str, u32, bool)] = &[
            ("ATTACKER", "Capture five flags.", 10, flags >= 5),
            ("BEST SQUAD", "Be in the best squad on all three maps.", 20, wins >= 3),
            ("DEFENDER", "Defend five flags.", 10, defends >= 5),
            ("MASTER OF YOUR DOMAIN", "Get one kill playing as each class.", 10, kills >= 3),
            ("MILKRUN", "Fly a plane for 10 minutes.", 10, air_time >= 600),
            ("MOTORMAN", "Get one kill with car, tank, and plane.", 15, veh_kills >= 3),
            ("PARACHUTIST", "Use the parachute once (2 seconds).", 10, chute >= 1),
            ("THE PACIFIC CAMPAIGN", "Win once on every map as any team.", 25, wins >= 4),
            ("TOUR OF DUTY I", "Play 30 matches.", 20, rounds >= 30),
            ("TOUR OF DUTY II", "Play 100 matches.", 30, rounds >= 100),
            ("VETERAN", "Cap 25 enemy flags.", 25, flags >= 25),
            ("MELEE MAN", "Melee kill one enemy player.", 15, melee >= 1),
        ];

        let earned_g: u32 = achievements.iter().filter(|a| a.3).map(|a| a.2).sum();
        let earned_cnt = achievements.iter().filter(|a| a.3).count();

        self.render_subpage_header(
            g,
            "ACHIEVEMENTS",
            Some(&format!("{earned_cnt}/12 UNLOCKED  ·  {earned_g}G / 200G")),
        );

        Self::fill_rect(g, argb(65, 0, 0, 0), 116.0, 217.0, 1056.0, 402.0);
        Self::fill_rect(g, PAPER_SOLID, 112.0, 213.0, 1056.0, 402.0);
        Self::stroke_rect(g, PAPER_BORDER, 1.5, 112.0, 213.0, 1056.0, 402.0);

        for (i, (title, desc, gs, unlocked)) in achievements.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let ax = 128.0 + col * 518.0;
            let ay = 227.0 + row * 62.0;
            let aw = 506.0;
            let ah = 54.0;

            Self::fill_rect(g, if *unlocked { WHITE } else { PAPER_ALT }, ax, ay, aw, ah);
            Self::stroke_rect(
                g,
                if *unlocked { OLIVE_ACTIVE } else { ROW_DIVIDER },
                if *unlocked { 1.8 } else { 1.0 },
                ax,
                ay,
                aw,
                ah,
            );

            Self::fill_circle(
                g,
                if *unlocked { OLIVE_ACTIVE } else { argb(255, 162, 164, 156) },
                ax + 12.0,
                ay + 11.0,
                32.0,
            );
            Self::draw_text_ex(
                self.body_font,
                g,
                if *unlocked { "★" } else { "○" },
                ax + 12.0,
                ay + 11.0,
                32.0,
                32.0,
                16.0,
                true,
                WHITE,
                STRING_ALIGN_CENTER,
                STRING_ALIGN_CENTER,
            );

            self.draw_display(g, title, ax + 54.0, ay + 4.0, 376.0, 24.0, 16.0, INK_BLACK, STRING_ALIGN_NEAR);
            self.draw_body(g, desc, ax + 54.0, ay + 29.0, 382.0, 18.0, 11.5, false, INK_MUTED);

            let gs_txt = format!("{gs}G");
            self.draw_display(
                g,
                &gs_txt,
                ax + aw - 66.0,
                ay,
                54.0,
                ah,
                17.5,
                if *unlocked { OLIVE_DARK } else { INK_MUTED },
                STRING_ALIGN_FAR,
            );
        }
    }

    fn render_help_options(&mut self, g: *mut c_void, st: &InGameUiState) {
        self.render_subpage_header(g, "HELP & OPTIONS", None);

        let faction = if st.bf1943_faction_ijn {
            "IMPERIAL JAPANESE NAVY (IJN)"
        } else {
            "UNITED STATES MARINE CORPS (USMC)"
        };
        let items: &[&str] = &[
            "TOGGLE KIT PREVIEW FACTION",
            "SWITCH TO BFBC2",
            "SWITCH TO VIETNAM",
            "EXIT GAME",
        ];

        // Exact BF1943 `common_screens.res` (`helpopt`) left stack: x=114, y=230, w=315, row_h=31
        let bx = 114.0;
        let by = 230.0;
        let bw = 315.0;
        let rh = 31.0;
        let bh = (items.len() as f32) * rh;
        Self::fill_rect(g, argb(56, 0, 0, 0), bx + 3.0, by + 3.0, bw, bh);
        Self::fill_rect(g, PAPER_BG, bx, by, bw, bh);

        for (i, label) in items.iter().enumerate() {
            let y = by + (i as f32) * rh;
            let selected = st.cursor_idx == i;
            if selected {
                Self::fill_rect(g, OLIVE_ACTIVE, bx, y, bw, rh);
                Self::fill_rect(g, OLIVE_HI, bx, y, bw, 2.0);
            }
            if i > 0 {
                Self::fill_rect(g, ROW_DIVIDER, bx, y, bw, 1.0);
            }
            self.draw_display(
                g,
                label,
                bx + 12.0,
                y,
                bw - 24.0,
                rh,
                18.0,
                if selected { WHITE } else { INK_BLACK },
                STRING_ALIGN_NEAR,
            );
            self.hit_rects.push(HitRect {
                x: bx,
                y,
                w: bw,
                h: rh,
                action: MenuClickAction::HoverAndActivate(i),
            });
        }
        Self::stroke_rect(g, PAPER_BORDER, 1.5, bx, by, bw, bh);

        // Right Controls + Kits Reference Paper Card (x=464..1168, y=213..615)
        let rx = 464.0;
        let ry = 213.0;
        let rw = 704.0;
        let rh_card = 402.0;
        Self::fill_rect(g, argb(65, 0, 0, 0), rx + 4.0, ry + 4.0, rw, rh_card);
        Self::fill_rect(g, PAPER_SOLID, rx, ry, rw, rh_card);
        Self::stroke_rect(g, PAPER_BORDER, 1.5, rx, ry, rw, rh_card);

        let dev_title = match st.input_device {
            InputDevice::GamepadXbox => "XBOX 360 CONTROLS",
            InputDevice::KeyboardMouse => "KEYBOARD & MOUSE",
        };
        if !self.subheadline_bar_img.is_null() {
            unsafe { GdipDrawImageRect(g, self.subheadline_bar_img, rx + 14.0, ry + 10.0, 257.0, 36.0) };
            self.draw_display(g, dev_title, rx + 26.0, ry + 10.0, 236.0, 34.0, 17.5, WHITE, STRING_ALIGN_NEAR);
        } else {
            self.draw_display(g, dev_title, rx + 18.0, ry + 12.0, 300.0, 26.0, 19.0, OLIVE_DARK, STRING_ALIGN_NEAR);
        }

        let lines: &[&str] = match st.input_device {
            InputDevice::GamepadXbox => &[
                "D-Pad / Left Stick  — Navigate Menus & Selectors",
                "(A) Button          — Confirm / Deploy",
                "(B) Button          — Back / Close Pop-Up",
                "(Y) Button          — Switch USMC / IJN Preview",
                "In-Match            — Full Xenon Prompt Mode Active",
            ],
            InputDevice::KeyboardMouse => &[
                "Up / Down / Mouse   — Navigate Menus",
                "Left / Right        — Cycle Mode & Map in Pop-Up",
                "Enter / Left Click  — Confirm / Deploy",
                "Esc / Backspace     — Back / Close Pop-Up",
                "In-Match Specials   — [H] Horn · [G] Gear · [E] Ramp",
            ],
        };
        for (i, ln) in lines.iter().enumerate() {
            self.draw_body(g, ln, rx + 18.0, ry + 52.0 + (i as f32) * 21.0, rw - 252.0, 19.0, 12.0, false, INK_BLACK);
        }
        if !self.controller_img.is_null() {
            unsafe { GdipDrawImageRect(g, self.controller_img, rx + rw - 230.0, ry + 42.0, 212.0, 112.0) };
        }

        Self::fill_rect(g, ROW_DIVIDER, rx + 18.0, ry + 164.0, rw - 36.0, 1.2);
        let faction_flag = if st.bf1943_faction_ijn {
            self.flag_jp_img
        } else {
            self.flag_us_img
        };
        let faction_col = if st.bf1943_faction_ijn {
            IJN_CRIMSON
        } else {
            USMC_SLATE
        };
        let mut kit_hdr_x = rx + 18.0;
        if !faction_flag.is_null() {
            unsafe { GdipDrawImageRect(g, faction_flag, rx + 18.0, ry + 174.0, 28.0, 20.0) };
            kit_hdr_x += 36.0;
        }
        self.draw_display(
            g,
            &format!("PACIFIC KITS — {faction}"),
            kit_hdr_x,
            ry + 171.0,
            rw - 72.0,
            26.0,
            17.5,
            faction_col,
            STRING_ALIGN_NEAR,
        );

        let soldiers = if st.bf1943_faction_ijn {
            &self.jp_soldiers
        } else {
            &self.us_soldiers
        };
        let kits = if st.bf1943_faction_ijn {
            [
                ("INFANTRY", "Type 100 SMG · Type 99 Rifle Grenade · Type 4 AT Rocket"),
                ("RIFLEMAN", "Type 5 Semi-Auto · Rifle Grenade Launcher · Katana"),
                ("SCOUT", "Type 99 Sniper · Type 14 Pistol · Demolition Satchel"),
            ]
        } else {
            [
                ("INFANTRY", "M1A1 Thompson · M1 Bazooka · Repair Wrench"),
                ("RIFLEMAN", "M1 Garand · M7 Rifle Grenade · Combat Knife"),
                ("SCOUT", "M1903 Springfield · M1911A1 · C4 Satchel Charges"),
            ]
        };
        for (i, (k, w)) in kits.iter().enumerate() {
            let ky = ry + 204.0 + (i as f32) * 63.0;
            Self::fill_rect(g, PAPER_ALT, rx + 18.0, ky, rw - 36.0, 56.0);
            Self::stroke_rect(g, ROW_DIVIDER, 1.0, rx + 18.0, ky, rw - 36.0, 56.0);
            let s_img = soldiers[i];
            if !s_img.is_null() {
                unsafe { GdipDrawImageRect(g, s_img, rx + 24.0, ky + 3.0, 36.0, 50.0) };
            }
            self.draw_display(g, k, rx + 70.0, ky + 4.0, 200.0, 22.0, 15.5, OLIVE_DARK, STRING_ALIGN_NEAR);
            self.draw_body(g, w, rx + 70.0, ky + 28.0, rw - 98.0, 20.0, 12.0, false, INK_BLACK);
        }
    }

    fn render_bottom_bar(&mut self, g: *mut c_void, st: &InGameUiState) {
        // Authentic Xbox 360 BF1943 floating bottom prompt badges (no heavy dark bar)
        let show_back = st.screen != Bf1943Screen::Main || st.modal != Bf1943Modal::None;
        let show_y = st.modal == Bf1943Modal::None && st.screen == Bf1943Screen::HelpOptions;

        let mut cx = 44.0;
        let cy = 678.0;
        let body_font = self.body_font;
        let draw_prompt_label = |g: *mut c_void, x: f32, y: f32, w: f32, text: &str| {
            Self::draw_text_ex(body_font, g, text, x + 1.2, y + 3.2, w, 20.0, 13.5, true, argb(210, 0, 0, 0), STRING_ALIGN_NEAR, STRING_ALIGN_NEAR);
            Self::draw_text_ex(body_font, g, text, x, y + 2.0, w, 20.0, 13.5, true, WHITE, STRING_ALIGN_NEAR, STRING_ALIGN_NEAR);
        };

        match st.input_device {
            InputDevice::GamepadXbox => {
                self.draw_xbox_badge(g, cx, cy, XBOX_A_GREEN, "A");
                draw_prompt_label(g, cx + 28.0, cy, 90.0, "SELECT");
                cx += 118.0;

                if show_back {
                    self.draw_xbox_badge(g, cx, cy, XBOX_B_RED, "B");
                    draw_prompt_label(g, cx + 28.0, cy, 80.0, "BACK");
                    self.hit_rects.push(HitRect {
                        x: cx,
                        y: cy - 4.0,
                        w: 100.0,
                        h: 30.0,
                        action: MenuClickAction::GoBack,
                    });
                    cx += 108.0;
                }
                if show_y {
                    self.draw_xbox_badge(g, cx, cy, XBOX_Y_YELLOW, "Y");
                    draw_prompt_label(g, cx + 28.0, cy, 150.0, "SWITCH FACTION");
                }
                draw_prompt_label(g, 1048.0, cy, 200.0, "XBOX 360 CONTROLLER");
            }
            InputDevice::KeyboardMouse => {
                self.draw_kbm_badge(g, cx, cy, "ENTER / CLICK", 108.0);
                draw_prompt_label(g, cx + 116.0, cy, 80.0, "SELECT");
                cx += 200.0;

                if show_back {
                    self.draw_kbm_badge(g, cx, cy, "ESC", 42.0);
                    draw_prompt_label(g, cx + 50.0, cy, 70.0, "BACK");
                    self.hit_rects.push(HitRect {
                        x: cx,
                        y: cy - 4.0,
                        w: 112.0,
                        h: 30.0,
                        action: MenuClickAction::GoBack,
                    });
                    cx += 120.0;
                }
                if matches!(st.modal, Bf1943Modal::MatchSetup { .. }) {
                    self.draw_kbm_badge(g, cx, cy, "◀ / ▶", 54.0);
                    draw_prompt_label(g, cx + 62.0, cy, 180.0, "CHANGE MODE / MAP");
                }
                draw_prompt_label(g, 1072.0, cy, 180.0, "KEYBOARD & MOUSE");
            }
        }
    }

    /// Native Battlefield: Bad Company 1 Front-End Menu (Gold Rush, 7-Mission Campaign, 5 Kits).
    fn render_bfbc1(&mut self, g: *mut c_void, st: &InGameUiState, _boot_fade: f32, _screen_fade: f32) {
        const BC1_BG: u32 = 0xFF14_1613;
        const BC1_GOLD: u32 = 0xFFE0_A82E;
        const BC1_OLIVE_PANEL: u32 = 0xD818_1C17;
        const BC1_CARD: u32 = 0xCC1E_231D;
        const BC1_SUBTEXT: u32 = 0xFFB8_C0B2;
        // Backdrop + dark tactical vignette + gold accent stripe are already pre-baked into bc1_backdrop_pixels.

        // Header brand bar
        Self::draw_text_ex(
            self.display_font,
            g,
            "BATTLEFIELD: BAD COMPANY",
            64.0,
            42.0,
            780.0,
            44.0,
            34.0,
            true,
            BC1_GOLD,
            STRING_ALIGN_NEAR,
            STRING_ALIGN_NEAR,
        );
        Self::draw_text_ex(
            self.body_font,
            g,
            "FROSTBITE 1.0 · GOLD RUSH · 5 KITS · LIFE-2 AUTO-INJECTOR · DESTRUCTION 1.0",
            66.0,
            88.0,
            780.0,
            24.0,
            12.5,
            true,
            BC1_SUBTEXT,
            STRING_ALIGN_NEAR,
            STRING_ALIGN_NEAR,
        );
        Self::fill_rect(g, argb(140, 224, 168, 46), 64.0, 118.0, 1152.0, 2.0);

        match st.screen {
            Bf1943Screen::Main => {
                let items = [
                    ("QUICK MATCH (OASIS)", "Deploy directly into MP_Beachhead_Day1 (Gold Rush)"),
                    ("OFFLINE MATCHES & CAMPAIGN", "Browse 8 Gold Rush maps & 7 Singleplayer missions"),
                    ("CONNECT TO A MATCH", "Direct IP multiplayer (offline ready)"),
                    ("HOST PRIVATE MATCH", "Launch local Gold Rush server on Oasis"),
                    ("UNLOCKS & STATS", "Offline B-Company rank, dogtags & collectibles"),
                    ("HELP & OPTIONS", "5-Kit loadout rules, Era Switch & Exit"),
                    ("ACHIEVEMENTS", "BFBC1 Xbox 360 awards & campaign progress"),
                ];
                for (i, (title, sub)) in items.iter().enumerate() {
                    let y = 154.0 + i as f32 * 64.0;
                    let sel = st.cursor_idx == i && st.modal == Bf1943Modal::None;
                    Self::fill_rect(
                        g,
                        if sel { argb(235, 224, 168, 46) } else { BC1_CARD },
                        64.0,
                        y,
                        412.0,
                        54.0,
                    );
                    if sel {
                        Self::stroke_rect(g, WHITE, 1.5, 64.0, y, 412.0, 54.0);
                    }
                    Self::draw_text_ex(
                        self.display_font,
                        g,
                        title,
                        82.0,
                        y + 7.0,
                        380.0,
                        24.0,
                        18.0,
                        true,
                        if sel { argb(255, 16, 18, 15) } else { WHITE },
                        STRING_ALIGN_NEAR,
                        STRING_ALIGN_NEAR,
                    );
                    Self::draw_text_ex(
                        self.body_font,
                        g,
                        sub,
                        82.0,
                        y + 31.0,
                        380.0,
                        18.0,
                        11.0,
                        false,
                        if sel { argb(230, 28, 30, 24) } else { BC1_SUBTEXT },
                        STRING_ALIGN_NEAR,
                        STRING_ALIGN_NEAR,
                    );
                    self.hit_rects.push(HitRect {
                        x: 64.0,
                        y,
                        w: 412.0,
                        h: 54.0,
                        action: MenuClickAction::HoverAndActivate(i),
                    });
                }

                // Right tactical briefing plate: 5-Kit & Rules Summary
                Self::fill_rect(g, BC1_OLIVE_PANEL, 530.0, 154.0, 686.0, 438.0);
                Self::stroke_rect(g, argb(160, 224, 168, 46), 1.5, 530.0, 154.0, 686.0, 438.0);
                Self::draw_text_ex(
                    self.display_font,
                    g,
                    "ACTIVE ERA: BAD COMPANY 1 (BFBC2BC1)",
                    556.0,
                    174.0,
                    630.0,
                    28.0,
                    20.0,
                    true,
                    BC1_GOLD,
                    STRING_ALIGN_NEAR,
                    STRING_ALIGN_NEAR,
                );
                let rules_lines = [
                    "NATIVE HUD VFS:    modules/bfbc1/menu/hud/hud_multiplayer-00.fbrb",
                    "DEFAULT MAP:       OASIS (Levels/MP_Beachhead_Day1 · Gold Rush)",
                    "5 PLAYABLE KITS:   ASSAULT · DEMOLITION · RECON · SPECIALIST · SUPPORT",
                    "HEALTH SYSTEM:     NO AUTO-REGEN · LIFE-2 AUTO-INJECTOR & MEDKIT",
                    "SUPPORT STRIKES:   RECON LASER DESIGNATOR · ARTILLERY · MORTAR",
                    "DESTRUCTION:       FROSTBITE 1.0 COMPOSITE WALLS & MATMASK SKELETON",
                ];
                for (ri, line) in rules_lines.iter().enumerate() {
                    Self::draw_text_ex(
                        self.body_font,
                        g,
                        line,
                        556.0,
                        220.0 + ri as f32 * 34.0,
                        636.0,
                        24.0,
                        13.0,
                        true,
                        WHITE,
                        STRING_ALIGN_NEAR,
                        STRING_ALIGN_NEAR,
                    );
                }

                // Quick-deploy Oasis CTA button
                Self::fill_rect(g, BC1_GOLD, 556.0, 514.0, 280.0, 48.0);
                Self::draw_text_ex(
                    self.display_font,
                    g,
                    "DEPLOY: OASIS (GOLD RUSH)",
                    556.0,
                    524.0,
                    280.0,
                    28.0,
                    17.0,
                    true,
                    argb(255, 16, 18, 15),
                    STRING_ALIGN_CENTER,
                    STRING_ALIGN_NEAR,
                );
                self.hit_rects.push(HitRect {
                    x: 556.0,
                    y: 514.0,
                    w: 280.0,
                    h: 48.0,
                    action: MenuClickAction::HoverAndActivate(0),
                });
            }
            Bf1943Screen::OfflineMatches => {
                let maps = crate::ui_state::BFBC1_MAPS;
                for (i, m) in maps.iter().enumerate() {
                    let y = 150.0 + i as f32 * 64.0;
                    let sel = st.cursor_idx == i;
                    Self::fill_rect(
                        g,
                        if sel { argb(235, 224, 168, 46) } else { BC1_CARD },
                        64.0,
                        y,
                        460.0,
                        54.0,
                    );
                    Self::draw_text_ex(
                        self.display_font,
                        g,
                        m.title,
                        82.0,
                        y + 6.0,
                        420.0,
                        24.0,
                        18.0,
                        true,
                        if sel { argb(255, 16, 18, 15) } else { WHITE },
                        STRING_ALIGN_NEAR,
                        STRING_ALIGN_NEAR,
                    );
                    Self::draw_text_ex(
                        self.body_font,
                        g,
                        m.mode_label,
                        82.0,
                        y + 30.0,
                        420.0,
                        18.0,
                        11.5,
                        false,
                        if sel { argb(230, 28, 30, 24) } else { BC1_SUBTEXT },
                        STRING_ALIGN_NEAR,
                        STRING_ALIGN_NEAR,
                    );
                    self.hit_rects.push(HitRect {
                        x: 64.0,
                        y,
                        w: 460.0,
                        h: 54.0,
                        action: MenuClickAction::HoverAndActivate(i),
                    });
                }

                let cur = &maps[st.cursor_idx.min(maps.len() - 1)];
                Self::fill_rect(g, BC1_OLIVE_PANEL, 560.0, 150.0, 656.0, 442.0);
                Self::stroke_rect(g, argb(160, 224, 168, 46), 1.5, 560.0, 150.0, 656.0, 442.0);
                Self::draw_text_ex(
                    self.display_font,
                    g,
                    cur.title,
                    586.0,
                    174.0,
                    600.0,
                    34.0,
                    26.0,
                    true,
                    BC1_GOLD,
                    STRING_ALIGN_NEAR,
                    STRING_ALIGN_NEAR,
                );
                Self::draw_text_ex(
                    self.body_font,
                    g,
                    cur.subtitle,
                    586.0,
                    214.0,
                    600.0,
                    22.0,
                    13.0,
                    true,
                    BC1_SUBTEXT,
                    STRING_ALIGN_NEAR,
                    STRING_ALIGN_NEAR,
                );
                Self::draw_text_ex(
                    self.body_font,
                    g,
                    cur.briefing,
                    586.0,
                    256.0,
                    600.0,
                    140.0,
                    14.5,
                    false,
                    WHITE,
                    STRING_ALIGN_NEAR,
                    STRING_ALIGN_NEAR,
                );
                Self::fill_rect(g, BC1_GOLD, 586.0, 514.0, 240.0, 48.0);
                Self::draw_text_ex(
                    self.display_font,
                    g,
                    "LAUNCH LEVEL",
                    586.0,
                    524.0,
                    240.0,
                    28.0,
                    18.0,
                    true,
                    argb(255, 16, 18, 15),
                    STRING_ALIGN_CENTER,
                    STRING_ALIGN_NEAR,
                );
                self.hit_rects.push(HitRect {
                    x: 586.0,
                    y: 514.0,
                    w: 240.0,
                    h: 48.0,
                    action: MenuClickAction::HoverAndActivate(st.cursor_idx.min(maps.len() - 1)),
                });
            }
            _ => {
                Self::fill_rect(g, BC1_OLIVE_PANEL, 64.0, 154.0, 1152.0, 438.0);
                Self::stroke_rect(g, argb(160, 224, 168, 46), 1.5, 64.0, 154.0, 1152.0, 438.0);
                Self::draw_text_ex(
                    self.display_font,
                    g,
                    "BAD COMPANY 1 — OPTIONS & ERA SWITCH",
                    96.0,
                    186.0,
                    900.0,
                    32.0,
                    24.0,
                    true,
                    BC1_GOLD,
                    STRING_ALIGN_NEAR,
                    STRING_ALIGN_NEAR,
                );
                let opts = [
                    "SWITCH TO BFBC2",
                    "SWITCH TO VIETNAM",
                    "RETURN TO MAIN MENU",
                    "EXIT GAME",
                ];
                for (i, label) in opts.iter().enumerate() {
                    let y = 248.0 + i as f32 * 58.0;
                    Self::fill_rect(g, BC1_CARD, 96.0, y, 360.0, 46.0);
                    Self::draw_text_ex(
                        self.display_font,
                        g,
                        label,
                        114.0,
                        y + 10.0,
                        320.0,
                        26.0,
                        17.0,
                        true,
                        WHITE,
                        STRING_ALIGN_NEAR,
                        STRING_ALIGN_NEAR,
                    );
                    self.hit_rects.push(HitRect {
                        x: 96.0,
                        y,
                        w: 360.0,
                        h: 46.0,
                        action: match i {
                            2 => MenuClickAction::GoBack,
                            _ => MenuClickAction::HoverAndActivate(i),
                        },
                    });
                }
            }
        }

        self.render_bottom_bar(g, st);
    }
}
