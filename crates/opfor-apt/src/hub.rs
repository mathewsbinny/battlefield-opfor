//! Tool Suite Design Language (`xenolith/design/tokens.json`, Dark theme)
//! 4-Blade Chronological OPFOR Hub (`1280x720`) layout and RGBA/TGA chrome generator.

use std::fs;
use std::io;
use std::path::Path;

/// RGB color parsed from `tokens.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    #[must_use]
    pub fn from_hex(s: &str) -> Option<Self> {
        let hex = s.trim().trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self(r, g, b))
    }
}

/// Tool Suite Design Language Dark palette (`xenolith/design/tokens.json`).
#[derive(Debug, Clone, Copy)]
pub struct HubPalette {
    pub ground: Rgb,
    pub surface_1: Rgb,
    pub surface_2: Rgb,
    pub surface_3: Rgb,
    pub line: Rgb,
    pub line_strong: Rgb,
    pub ink: Rgb,
    pub ink_muted: Rgb,
    pub ink_faint: Rgb,
    pub accent: Rgb,
    pub era_accents: [Rgb; 4], // [BF1943, Vietnam, BFBC1, BFBC2]
}

impl Default for HubPalette {
    fn default() -> Self {
        Self {
            ground: Rgb(0x14, 0x14, 0x14),
            surface_1: Rgb(0x1f, 0x1f, 0x1f),
            surface_2: Rgb(0x2b, 0x2b, 0x2b),
            surface_3: Rgb(0x36, 0x36, 0x36),
            line: Rgb(0x0f, 0x0f, 0x0f),
            line_strong: Rgb(0x7a, 0x7a, 0x7a),
            ink: Rgb(0xe3, 0xe3, 0xe3),
            ink_muted: Rgb(0xa6, 0xa6, 0xa6),
            ink_faint: Rgb(0x6c, 0x6c, 0x6c),
            accent: Rgb(0x34, 0x75, 0xd0),
            era_accents: [
                Rgb(0x3b, 0xa6, 0xa8), // BF1943 Pacific teal
                Rgb(0xd9, 0x6b, 0x27), // Vietnam napalm amber
                Rgb(0xe0, 0xa8, 0x2e), // BFBC1 tactical gold
                Rgb(0xf0, 0x7b, 0x19), // BFBC2 ember orange
            ],
        }
    }
}

/// Geometry of the 4-Blade OPFOR Hub stage (`1280x720`).
pub const STAGE_W: usize = 1280;
pub const STAGE_H: usize = 720;
pub const TOP_BAR_H: usize = 56;
pub const FOOTER_BAR_H: usize = 48;
pub const BLADE_W: usize = STAGE_W / 4; // 320px per blade

/// Render the neutral Tool Suite Dark 4-Blade Hub chrome to an uncompressed 32-bit TGA
/// (`modules/hub/hub_chrome.tga`) for immediate visual review and APT atlas compositing.
pub fn write_hub_chrome_tga(path: &Path, pal: &HubPalette) -> io::Result<()> {
    let mut rgba = vec![0u8; STAGE_W * STAGE_H * 4];
    let mut put = |x: usize, y: usize, c: Rgb, a: u8| {
        let i = (y * STAGE_W + x) * 4;
        rgba[i] = c.2; // TGA stores BGRA
        rgba[i + 1] = c.1;
        rgba[i + 2] = c.0;
        rgba[i + 3] = a;
    };

    // 1. Fill ground
    for y in 0..STAGE_H {
        for x in 0..STAGE_W {
            put(x, y, pal.ground, 255);
        }
    }
    // 2. Top bar & footer bar (`surface-1`) + 1px `line` dividers
    for y in 0..TOP_BAR_H {
        for x in 0..STAGE_W {
            put(x, y, pal.surface_1, 255);
        }
    }
    for x in 0..STAGE_W {
        put(x, TOP_BAR_H, pal.line, 255);
        put(x, STAGE_H - FOOTER_BAR_H - 1, pal.line, 255);
    }
    for y in (STAGE_H - FOOTER_BAR_H)..STAGE_H {
        for x in 0..STAGE_W {
            put(x, y, pal.surface_1, 255);
        }
    }
    // 3. 4 Blade panels + bottom metadata plates (`surface-2`) + 3px era accent bars + 1px `line` dividers
    let blade_top = TOP_BAR_H + 1;
    let blade_bot = STAGE_H - FOOTER_BAR_H - 1;
    let plate_h = 112;
    for b in 0..4 {
        let x0 = b * BLADE_W;
        let x1 = x0 + BLADE_W;
        for y in (blade_bot - plate_h)..blade_bot {
            for x in x0..x1 {
                put(x, y, pal.surface_2, 230);
            }
        }
        // 3px era accent strip at top of blade plate
        let era_c = pal.era_accents[b];
        for y in (blade_bot - plate_h)..(blade_bot - plate_h + 3) {
            for x in x0..x1 {
                put(x, y, era_c, 255);
            }
        }
        // Vertical 1px `line` divider between blades
        if b > 0 {
            for y in blade_top..blade_bot {
                put(x0, y, pal.line, 255);
            }
        }
    }
    // 4. OPFOR accent pill in top-left header (`accent` #3475d0)
    for y in 16..40 {
        for x in 20..26 {
            put(x, y, pal.accent, 255);
        }
    }

    // Write 18-byte uncompressed 32-bit top-down TGA
    let mut tga = Vec::with_capacity(18 + rgba.len());
    tga.extend_from_slice(&[
        0,
        0,
        2, // uncompressed true-color
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        (STAGE_W & 0xFF) as u8,
        (STAGE_W >> 8) as u8,
        (STAGE_H & 0xFF) as u8,
        (STAGE_H >> 8) as u8,
        32,
        0x28, // 32-bit BGRA, top-left origin, 8-bit alpha
    ]);
    tga.extend_from_slice(&rgba);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, tga)
}

/// Write the machine-readable `hub_layout.toml` describing the 4 blades for APT authoring.
pub fn write_hub_layout_toml(path: &Path) -> io::Result<()> {
    let content = r##"# Project OPFOR — 4-Blade Hub APT Layout (1280x720, Tool Suite Dark Theme)
stage_width = 1280
stage_height = 720
top_bar_height = 56
footer_bar_height = 48
theme_source = "xenolith/design/tokens.json"

[[blade]]
slot = 0
id = "bf1943"
sprite = "blade_1943"
x = 0
width = 320
accent = "#3ba6a8"
fscommand = "opforEnterMode"
arg = "2"

[[blade]]
slot = 1
id = "vietnam"
sprite = "blade_nam"
x = 320
width = 320
accent = "#d96b27"
fscommand = "opforEnterMode"
arg = "1"

[[blade]]
slot = 2
id = "bfbc1"
sprite = "blade_bc1"
x = 640
width = 320
accent = "#e0a82e"
fscommand = "opforEnterMode"
arg = "4"

[[blade]]
slot = 3
id = "bfbc2"
sprite = "blade_bc2"
x = 960
width = 320
accent = "#f07b19"
fscommand = "opforEnterMode"
arg = "0"
"##;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}
