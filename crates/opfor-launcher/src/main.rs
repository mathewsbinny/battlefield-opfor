//! Project OPFOR — Unified 4-Era Rust Frontend (`OPFOR.exe`).
//!
//! Boots directly into the 1280x720 Tool Suite Dark 4-Blade Hub
//! (Battlefield 1943 · BFBC2: Vietnam · Battlefield: Bad Company · BFBC2),
//! switches instantaneously between per-era menus in one process, and
//! direct-launches `engine/BFBC2Game.exe` for 3D matches via `config/launch.ini`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod input_monitor;
mod menu_1943;
mod ui_state;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Instant;

use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, Vec2};
use opfor_vfs::{ModeId, ModeTable, ModuleManifest, discover_modules, merge_package_mft};

use crate::input_monitor::{InputDevice, InputMonitor};
use crate::menu_1943::{Bf1943Action, render_bf1943_menu};
use crate::ui_state::{Bf1943Tab, GlobalScreen, GlobalUiState, PACIFIC_MAPS};

// Tool Suite Design Language Dark tokens (`xenolith/design/tokens.json`)
const GROUND: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
const SURFACE_1: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x1f);
const SURFACE_2: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b);
const SURFACE_3: Color32 = Color32::from_rgb(0x36, 0x36, 0x36);
const LINE: Color32 = Color32::from_rgb(0x0f, 0x0f, 0x0f);
const LINE_STRONG: Color32 = Color32::from_rgb(0x7a, 0x7a, 0x7a);
const INK: Color32 = Color32::from_rgb(0xe3, 0xe3, 0xe3);
const INK_MUTED: Color32 = Color32::from_rgb(0xa6, 0xa6, 0xa6);
const INK_FAINT: Color32 = Color32::from_rgb(0x6c, 0x6c, 0x6c);
const ACCENT: Color32 = Color32::from_rgb(0x34, 0x75, 0xd0);
const ACCENT_TEXT: Color32 = Color32::from_rgb(0x6e, 0xaa, 0xf0);

fn era_accent(mode: ModeId) -> Color32 {
    match mode {
        ModeId::Bf1943 => Color32::from_rgb(0x3b, 0xa6, 0xa8),
        ModeId::Vietnam => Color32::from_rgb(0xd9, 0x6b, 0x27),
        ModeId::Bfbc1 => Color32::from_rgb(0xe0, 0xa8, 0x2e),
        ModeId::Bfbc2 => Color32::from_rgb(0xf0, 0x7b, 0x19),
    }
}

fn era_subtitle(mode: ModeId) -> &'static str {
    match mode {
        ModeId::Bf1943 => "4 Maps",
        ModeId::Vietnam => "5 Maps",
        ModeId::Bfbc1 => "7 Missions · 8 Maps",
        ModeId::Bfbc2 => "13 Missions · 14 Maps",
    }
}

fn era_full_name(mode: ModeId) -> &'static str {
    match mode {
        ModeId::Bf1943 => "Battlefield 1943",
        ModeId::Vietnam => "Battlefield: Bad Company 2 Vietnam",
        ModeId::Bfbc1 => "Battlefield: Bad Company",
        ModeId::Bfbc2 => "Battlefield: Bad Company 2",
    }
}

const RESOLUTIONS: &[(u32, u32, &str)] = &[
    (1280, 720, "1280 × 720 (720p)"),
    (1366, 768, "1366 × 768"),
    (1600, 900, "1600 × 900 (900p)"),
    (1920, 1080, "1920 × 1080 (1080p)"),
    (2560, 1080, "2560 × 1080 (21:9)"),
    (2560, 1440, "2560 × 1440 (1440p)"),
    (3440, 1440, "3440 × 1440 (21:9)"),
    (3840, 2160, "3840 × 2160 (4K)"),
];

struct OpforApp {
    root: PathBuf,
    ui_state: GlobalUiState,
    input_monitor: InputMonitor,
    focused_blade: usize,
    modules: Vec<ModuleManifest>,
    table: ModeTable,
    #[allow(dead_code)]
    levels_cache: Vec<String>,
    selected_level: Option<String>,
    player_name: String,
    clan_tag: String,
    display_w: u32,
    display_h: u32,
    fullscreen: bool,
    blade_textures: [Option<egui::TextureHandle>; 4],
    blade_logos: [Option<egui::TextureHandle>; 4],
    bf1943_map_thumbs: [Option<egui::TextureHandle>; 4],
    splash_texture: Option<egui::TextureHandle>,
    splash_started: Instant,
    textures_loaded: bool,
    status_line: String,
    running_game: Option<Child>,
    launch_started: Option<Instant>,
    curtain_active: bool,
}

impl OpforApp {
    fn new(root: PathBuf, display_w: u32, display_h: u32, fullscreen: bool) -> Self {
        let modules = discover_modules(&root.join("modules")).unwrap_or_default();
        let mut mask = ModeId::Bfbc2.bit();
        for m in &modules {
            if m.enabled && m.has_content_bundles() {
                if let Some(mid) = m.mode_id() {
                    mask |= mid.bit();
                }
            }
        }
        let (player_name, clan_tag) = load_profile(&root.join("user").join("profile.ini"));
        let ui_state = GlobalUiState::new(&root);
        let mut app = Self {
            root,
            ui_state,
            input_monitor: InputMonitor::new(),
            focused_blade: 3, // default highlight on BFBC2
            modules,
            table: ModeTable::new(mask),
            levels_cache: Vec::new(),
            selected_level: None,
            player_name,
            clan_tag,
            display_w,
            display_h,
            fullscreen,
            blade_textures: [None, None, None, None],
            blade_logos: [None, None, None, None],
            bf1943_map_thumbs: [None, None, None, None],
            splash_texture: None,
            splash_started: Instant::now(),
            textures_loaded: false,
            status_line: "Ready — select an era to enter its menu.".to_string(),
            running_game: None,
            launch_started: None,
            curtain_active: false,
        };
        app.refresh_cache();
        app
    }

    fn refresh_cache(&mut self) {
        let stock_mft = self.root.join("engine").join("Dist").join("win32").join("package.mft");
        let cache_mft = self.root.join("cache").join("package.mft");
        if stock_mft.is_file() {
            if let Ok((base, added)) = merge_package_mft(&stock_mft, &self.modules, &cache_mft) {
                self.status_line = format!(
                    "VFS cache synced ({base} stock + {added} module bundles) · installed=0x{:02x}",
                    self.table.installed_mask()
                );
            }
        }
    }

    fn enter_era(&mut self, mode: ModeId) {
        if !self.table.is_installed(mode) {
            self.status_line = format!(
                "{} is not installed yet — drop converted bundles into modules/{}/",
                self.table.spec(mode).display_name,
                self.table.spec(mode).slug
            );
            return;
        }
        if let Some(direct_engine_mode) = self.ui_state.select_mode(mode) {
            self.selected_level = None;
            self.launch_match(direct_engine_mode);
        } else {
            self.ui_state.reload_stats(&self.root);
            self.status_line = format!(
                "{} — select an island or join from the offline server browser.",
                self.table.spec(mode).display_name
            );
        }
    }

    fn launch_match(&mut self, mode: ModeId) {
        if self.running_game.is_some() {
            self.status_line = "OPFOREngine.exe is already running.".to_string();
            return;
        }
        let engine_dir = self.root.join("engine");
        let exe = if engine_dir.join("OPFOREngine.exe").is_file() {
            engine_dir.join("OPFOREngine.exe")
        } else {
            engine_dir.join("BFBC2Game.exe")
        };
        if !exe.is_file() {
            self.status_line = format!("Missing {} — run opfor-cli setup-engine first.", exe.display());
            return;
        }
        self.refresh_cache();

        let level = self.selected_level.clone().unwrap_or_default();
        let launch_ini = self.root.join("config").join("launch.ini");
        let _ = fs::create_dir_all(self.root.join("config"));
        let payload = format!(
            "mode={}\nlevel={}\nprofile_name={}\nclan_tag={}\nwidth={}\nheight={}\nfullscreen={}\n",
            self.table.spec(mode).slug,
            level,
            self.player_name,
            self.clan_tag,
            self.display_w,
            self.display_h,
            i32::from(self.fullscreen),
        );
        let _ = fs::write(&launch_ini, payload);

        let ready_flag = self.root.join("cache").join("engine_ready.flag");
        let _ = fs::remove_file(&ready_flag);

        match Command::new(&exe).current_dir(&engine_dir).spawn() {
            Ok(child) => {
                self.running_game = Some(child);
                self.launch_started = Some(Instant::now());
                // Only show the OPFOR splash curtain over the stock BFBC2 title screen for WW2 & BFBC1
                self.curtain_active = matches!(mode, ModeId::Bf1943 | ModeId::Bfbc1);
                self.status_line = format!(
                    "Launched {} ({}) — waiting for match exit...",
                    self.table.spec(mode).display_name,
                    if level.is_empty() { "menu" } else { &level }
                );
            }
            Err(e) => {
                self.status_line = format!("Failed to launch {}: {e}", exe.display());
            }
        }
    }

    fn poll_running_game(&mut self, ctx: &egui::Context) {
        if let Some(child) = self.running_game.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                self.running_game = None;
                self.launch_started = None;
                if self.curtain_active {
                    self.curtain_active = false;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.status_line = format!("Match process exited ({status}). Returned to OPFOR Hub.");
                return;
            }
            if self.curtain_active {
                let ready_flag = self.root.join("cache").join("engine_ready.flag");
                let timed_out = self
                    .launch_started
                    .is_some_and(|t| t.elapsed().as_secs_f32() >= 15.0);
                if ready_flag.is_file() || timed_out {
                    let _ = fs::remove_file(&ready_flag);
                    self.curtain_active = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        egui::WindowLevel::AlwaysOnTop,
                    ));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                }
            }
        }
    }
}

impl eframe::App for OpforApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_running_game(&ctx);

        // Dynamic input-device monitor: switches cleanly between Keyboard/Mouse & Xbox Gamepad
        let pad = self.input_monitor.poll_gamepad();
        let kbm_active = ctx.input(|i| {
            i.pointer.any_click()
                || i.pointer.delta().length_sq() > 4.0
                || !i.keys_down.is_empty()
        });
        if pad.any_activity {
            self.ui_state.input_device = InputDevice::GamepadXbox;
            ctx.request_repaint();
        } else if kbm_active {
            self.ui_state.input_device = InputDevice::KeyboardMouse;
        }
        // Keep polling gamepad at ~30 Hz when idle so connecting/pressing a button responds immediately
        ctx.request_repaint_after(std::time::Duration::from_millis(33));

        apply_tool_suite_dark_theme(&ctx);
        if !self.textures_loaded {
            self.textures_loaded = true;
            self.splash_texture = load_image_texture(
                &ctx,
                "opfor_splash",
                &[
                    self.root.join("modules").join("hub").join("splash_opfor.png"),
                    self.root.join("modules").join("hub").join("splash_opfor.jpg"),
                ],
            );
            for (idx, mode) in ModeId::HUB_ORDER.iter().copied().enumerate() {
                if let Some(tex) = load_blade_texture(&ctx, &self.root, mode) {
                    self.blade_textures[idx] = Some(tex);
                }
                if let Some(logo) = load_logo_texture(&ctx, &self.root, mode) {
                    self.blade_logos[idx] = Some(logo);
                }
            }
            for (idx, map) in PACIFIC_MAPS.iter().enumerate() {
                let p = self.root.join("modules").join("bf1943").join("art").join(map.thumb_file);
                self.bf1943_map_thumbs[idx] =
                    load_image_texture(&ctx, &format!("bf1943_thumb_{}", map.id), &[p]);
            }
        }

        // Full-window BATTLEFIELD OPFOR splash on boot (1.8s, click/key skips) or while launching engine
        if self.ui_state.screen == GlobalScreen::Splash || self.running_game.is_some() {
            if self.ui_state.screen == GlobalScreen::Splash {
                let elapsed = self.splash_started.elapsed().as_secs_f32();
                let skip = ctx.input(|i| i.pointer.any_click() || !i.keys_down.is_empty());
                if elapsed >= 1.8 || skip {
                    self.ui_state.screen = GlobalScreen::Hub;
                } else {
                    ctx.request_repaint();
                }
            } else {
                ctx.request_repaint();
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(Color32::BLACK).inner_margin(Margin::ZERO))
                .show(ui, |ui| {
                    let rect = ui.available_rect_before_wrap();
                    if let Some(tex) = &self.splash_texture {
                        let ts = tex.size_vec2();
                        let scale = (rect.width() / ts.x).max(rect.height() / ts.y);
                        let uv_w = (rect.width() / (ts.x * scale)).min(1.0);
                        let uv_h = (rect.height() / (ts.y * scale)).min(1.0);
                        let uv = egui::Rect::from_min_max(
                            egui::pos2(0.5 - uv_w * 0.5, 0.5 - uv_h * 0.5),
                            egui::pos2(0.5 + uv_w * 0.5, 0.5 + uv_h * 0.5),
                        );
                        ui.painter().image(tex.id(), rect, uv, Color32::WHITE);
                    } else {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "BATTLEFIELD OPFOR",
                            FontId::proportional(54.0),
                            Color32::WHITE,
                        );
                    }
                });
            return;
        }

        // Hand-crafted Battlefield 1943 Pacific Theatre UI (full-window era shell)
        if self.ui_state.screen == GlobalScreen::Bf1943 {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || pad.back {
                self.ui_state.return_to_hub();
                return;
            }
            if pad.lb {
                self.ui_state.cycle_bf1943_tab(-1);
            }
            if pad.rb {
                self.ui_state.cycle_bf1943_tab(1);
            }
            if self.ui_state.bf1943_tab == Bf1943Tab::Play {
                if pad.left || ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    self.ui_state.bf1943_selected_map =
                        (self.ui_state.bf1943_selected_map + PACIFIC_MAPS.len() - 1)
                            % PACIFIC_MAPS.len();
                }
                if pad.right || ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    self.ui_state.bf1943_selected_map =
                        (self.ui_state.bf1943_selected_map + 1) % PACIFIC_MAPS.len();
                }
                if pad.confirm || ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let lvl = PACIFIC_MAPS[self.ui_state.bf1943_selected_map].level_path.to_string();
                    self.selected_level = Some(lvl);
                    self.launch_match(ModeId::Bf1943);
                    return;
                }
            }
            let act = render_bf1943_menu(
                ui,
                &mut self.ui_state,
                self.blade_textures[0].as_ref(),
                &self.bf1943_map_thumbs,
                &mut self.player_name,
                &mut self.clan_tag,
                &self.status_line,
            );
            match act {
                Bf1943Action::None => {}
                Bf1943Action::BackToHub => self.ui_state.return_to_hub(),
                Bf1943Action::SwitchMode(m) => self.enter_era(m),
                Bf1943Action::OpenSettings => self.ui_state.screen = GlobalScreen::Settings,
                Bf1943Action::DeployMap { level_path } => {
                    self.selected_level = Some(level_path);
                    self.launch_match(ModeId::Bf1943);
                }
                Bf1943Action::SaveProfile => {
                    save_profile(
                        &self.root.join("user").join("profile.ini"),
                        &self.player_name,
                        &self.clan_tag,
                    );
                    self.status_line = format!(
                        "Saved Pacific soldier profile: [{}] {}",
                        self.clan_tag, self.player_name
                    );
                }
            }
            return;
        }

        // Keyboard & Gamepad navigation in Hub
        if self.ui_state.screen == GlobalScreen::Hub {
            let mut move_left = pad.left;
            let mut move_right = pad.right;
            let mut confirm = pad.confirm;
            ctx.input(|i| {
                move_left |= i.key_pressed(egui::Key::ArrowLeft);
                move_right |= i.key_pressed(egui::Key::ArrowRight);
                confirm |= i.key_pressed(egui::Key::Enter);
            });
            if move_left {
                self.focused_blade = (self.focused_blade + 3) % 4;
            }
            if move_right {
                self.focused_blade = (self.focused_blade + 1) % 4;
            }
            if confirm {
                let mode = ModeId::HUB_ORDER[self.focused_blade];
                self.enter_era(mode);
            }
        } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || pad.back {
            self.ui_state.return_to_hub();
        }

        // Top Bar (56px, surface-1)
        egui::Panel::top("opfor_top_bar")
            .exact_size(56.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .inner_margin(Margin::symmetric(20, 12))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    // Accent bar + Title
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(5.0, 24.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 1.0, ACCENT);
                    ui.add_space(10.0);

                    match self.ui_state.screen {
                        GlobalScreen::Splash | GlobalScreen::Hub | GlobalScreen::Bf1943 => {
                            ui.label(
                                RichText::new("PROJECT OPFOR")
                                    .font(FontId::proportional(20.0))
                                    .strong()
                                    .color(INK),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("FROSTBITE 1.x UNIFIED CLIENT")
                                    .font(FontId::proportional(11.0))
                                    .color(INK_MUTED),
                            );
                        }
                        GlobalScreen::Settings => {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("◄ OPFOR HUB").color(INK).strong(),
                                    )
                                    .fill(SURFACE_2)
                                    .stroke(Stroke::new(1.0, LINE_STRONG)),
                                )
                                .clicked()
                            {
                                self.ui_state.return_to_hub();
                            }
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("GLOBAL SETTINGS & MODULES")
                                    .font(FontId::proportional(18.0))
                                    .strong()
                                    .color(INK),
                            );
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Quit").color(INK))
                                    .fill(SURFACE_2)
                                    .stroke(Stroke::new(1.0, LINE_STRONG)),
                            )
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        ui.add_space(8.0);
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Settings & Modules").color(INK))
                                    .fill(if self.ui_state.screen == GlobalScreen::Settings {
                                        SURFACE_3
                                    } else {
                                        SURFACE_2
                                    })
                                    .stroke(Stroke::new(1.0, LINE_STRONG)),
                            )
                            .clicked()
                        {
                            self.ui_state.screen = if self.ui_state.screen == GlobalScreen::Settings {
                                GlobalScreen::Hub
                            } else {
                                GlobalScreen::Settings
                            };
                        }
                        ui.add_space(12.0);
                        let tag = if self.clan_tag.is_empty() {
                            self.player_name.clone()
                        } else {
                            format!("[{}] {}", self.clan_tag, self.player_name)
                        };
                        ui.label(RichText::new(tag).color(ACCENT_TEXT).strong());
                    });
                });
            });

        // Footer Bar (44px, surface-1)
        egui::Panel::bottom("opfor_footer")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .inner_margin(Margin::symmetric(20, 10))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let hint = match (self.ui_state.screen, self.ui_state.input_device) {
                        (GlobalScreen::Splash | GlobalScreen::Hub | GlobalScreen::Bf1943, InputDevice::KeyboardMouse) => {
                            "[← →] Select Era   [Enter / Click] Open Era   [KBM]"
                        }
                        (GlobalScreen::Splash | GlobalScreen::Hub | GlobalScreen::Bf1943, InputDevice::GamepadXbox) => {
                            "[D-Pad / LS] Select Era   [A] Open Era   [XBOX PAD]"
                        }
                        (GlobalScreen::Settings, InputDevice::KeyboardMouse) => "[Esc] Back to Hub",
                        (GlobalScreen::Settings, InputDevice::GamepadXbox) => "[B] Back to Hub",
                    };
                    ui.label(RichText::new(hint).font(FontId::proportional(12.0)).color(INK_MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(&self.status_line)
                                .font(FontId::proportional(12.0))
                                .color(INK_MUTED),
                        );
                    });
                });
            });

        // Central Panel
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(GROUND).inner_margin(Margin::ZERO))
            .show(ui, |ui| match self.ui_state.screen {
                GlobalScreen::Splash | GlobalScreen::Bf1943 => {}
                GlobalScreen::Hub => self.ui_hub(ui),
                GlobalScreen::Settings => self.ui_settings(ui),
            });
    }
}

impl OpforApp {
    fn ui_hub(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_size();
        let blade_w = (avail.x / 4.0).floor();
        let blade_h = avail.y;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for (idx, mode) in ModeId::HUB_ORDER.iter().copied().enumerate() {
                let installed = self.table.is_installed(mode);
                let focused = self.focused_blade == idx;
                let accent = era_accent(mode);

                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(blade_w, blade_h), egui::Sense::click());
                if resp.hovered() {
                    self.focused_blade = idx;
                }
                if resp.clicked() {
                    self.enter_era(mode);
                }

                let p = ui.painter();
                // Blade body fill
                let body_fill = if !installed {
                    Color32::from_rgb(0x16, 0x16, 0x16)
                } else if focused {
                    Color32::from_rgb(0x23, 0x25, 0x29)
                } else {
                    Color32::from_rgb(0x1a, 0x1a, 0x1a)
                };
                p.rect_filled(rect, CornerRadius::ZERO, body_fill);

                // Full-bleed aspect-fill blade key art (if modules/<era>/blade.png exists)
                let art_rect = egui::Rect::from_min_max(
                    rect.min,
                    egui::pos2(rect.max.x, rect.max.y - 156.0),
                );
                if let Some(tex) = &self.blade_textures[idx] {
                    let ts = tex.size_vec2();
                    let scale = (art_rect.width() / ts.x).max(art_rect.height() / ts.y);
                    let uv_w = (art_rect.width() / (ts.x * scale)).min(1.0);
                    let uv_h = (art_rect.height() / (ts.y * scale)).min(1.0);
                    let uv = egui::Rect::from_min_max(
                        egui::pos2(0.5 - uv_w * 0.5, 0.5 - uv_h * 0.5),
                        egui::pos2(0.5 + uv_w * 0.5, 0.5 + uv_h * 0.5),
                    );
                    let tint = if !installed {
                        Color32::from_rgba_unmultiplied(90, 90, 90, 140)
                    } else if focused {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(145, 145, 150)
                    };
                    p.image(tex.id(), art_rect, uv, tint);
                    // Subtle top/bottom dark vignette for clean contrast
                    let scrim = egui::Rect::from_min_max(
                        egui::pos2(art_rect.min.x, art_rect.max.y - 64.0),
                        art_rect.max,
                    );
                    p.rect_filled(scrim, 0.0, Color32::from_rgba_unmultiplied(0x14, 0x14, 0x14, 150));
                }

                // Subtle top era gradient bar when focused
                if focused && installed {
                    let top_bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), 4.0));
                    p.rect_filled(top_bar, 0.0, accent);
                }

                // Bottom metadata plate (`surface-2` when focused, `surface-1` when idle)
                let plate_h = 156.0;
                let plate_rect = egui::Rect::from_min_max(
                    egui::pos2(rect.min.x, rect.max.y - plate_h),
                    rect.max,
                );
                p.rect_filled(plate_rect, 0.0, if focused { SURFACE_2 } else { SURFACE_1 });

                // 3px era accent strip above metadata plate
                let strip = egui::Rect::from_min_size(
                    plate_rect.min,
                    Vec2::new(plate_rect.width(), if focused { 3.0 } else { 1.0 }),
                );
                p.rect_filled(strip, 0.0, if installed { accent } else { INK_FAINT });

                // 1px vertical hairline divider
                if idx > 0 {
                    p.line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0, LINE));
                }

                // Center original mode logo (clean, no subtitle text over key art)
                let center_y = rect.min.y + (blade_h - plate_h) * 0.50;
                let max_logo_w = (blade_w - 36.0).max(80.0);
                let max_logo_h = if focused { 116.0 } else { 100.0 };
                if let Some(logo) = &self.blade_logos[idx] {
                    let ls = logo.size_vec2();
                    let scale = (max_logo_w / ls.x.max(1.0)).min(max_logo_h / ls.y.max(1.0));
                    let dw = ls.x * scale;
                    let dh = ls.y * scale;
                    let logo_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, center_y),
                        Vec2::new(dw, dh),
                    );
                    // Subtle dark halo behind logo for crisp readability over bright art
                    p.rect_filled(
                        logo_rect.expand2(Vec2::new(14.0, 10.0)),
                        CornerRadius::same(4),
                        Color32::from_rgba_unmultiplied(0x10, 0x10, 0x10, if focused { 135 } else { 110 }),
                    );
                    let logo_tint = if !installed {
                        Color32::from_rgba_unmultiplied(150, 150, 150, 150)
                    } else if focused {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(210, 210, 215)
                    };
                    p.image(
                        logo.id(),
                        logo_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        logo_tint,
                    );
                } else {
                    // Clean fallback emblem if a mode logo PNG is not yet present
                    let fallback_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, center_y),
                        Vec2::new(max_logo_w, 64.0),
                    );
                    p.rect_filled(
                        fallback_rect,
                        CornerRadius::same(4),
                        Color32::from_rgba_unmultiplied(0x10, 0x10, 0x10, 130),
                    );
                    p.text(
                        fallback_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "BATTLEFIELD\nBAD COMPANY",
                        FontId::proportional(if focused { 24.0 } else { 21.0 }),
                        if !installed {
                            INK_FAINT
                        } else if focused {
                            Color32::WHITE
                        } else {
                            INK_MUTED
                        },
                    );
                }

                // Standardized bottom metadata plate (full official title + uniform subtitle + status)
                let tx = plate_rect.min.x + 18.0;
                let mut ty = plate_rect.min.y + 22.0;
                p.text(
                    egui::pos2(tx, ty),
                    egui::Align2::LEFT_TOP,
                    era_full_name(mode),
                    FontId::proportional(16.0),
                    if installed { INK } else { INK_FAINT },
                );
                ty += 28.0;
                p.text(
                    egui::pos2(tx, ty),
                    egui::Align2::LEFT_TOP,
                    era_subtitle(mode),
                    FontId::proportional(12.0),
                    if installed { INK_MUTED } else { INK_FAINT },
                );
                ty += 34.0;
                let status_badge = if installed {
                    if focused {
                        "● READY — CLICK TO ENTER"
                    } else {
                        "● INSTALLED"
                    }
                } else {
                    "○ NOT INSTALLED"
                };
                p.text(
                    egui::pos2(tx, ty),
                    egui::Align2::LEFT_TOP,
                    status_badge,
                    FontId::proportional(11.0),
                    if !installed {
                        INK_FAINT
                    } else if focused {
                        accent
                    } else {
                        ACCENT_TEXT
                    },
                );
            }
        });
    }

    #[allow(dead_code)]
    fn ui_era_menu(&mut self, ui: &mut egui::Ui, mode: ModeId) {
        let accent = era_accent(mode);
        let spec = self.table.spec(mode).clone();

        egui::Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
            ui.columns(2, |cols| {
                // Left column: Level / Map Selector
                cols[0].label(
                    RichText::new("AVAILABLE MAPS & MISSIONS")
                        .font(FontId::proportional(13.0))
                        .color(accent)
                        .strong(),
                );
                cols[0].add_space(8.0);

                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(Stroke::new(1.0, LINE))
                    .inner_margin(Margin::same(10))
                    .show(&mut cols[0], |ui| {
                        egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                            if self.levels_cache.is_empty() {
                                ui.label(
                                    RichText::new("No level folders discovered in this module yet.")
                                        .color(INK_MUTED),
                                );
                            }
                            for lv in &self.levels_cache {
                                let selected = self.selected_level.as_deref() == Some(lv.as_str());
                                let resp = ui.selectable_label(
                                    selected,
                                    RichText::new(lv)
                                        .font(FontId::monospace(13.0))
                                        .color(if selected { INK } else { INK_MUTED }),
                                );
                                if resp.clicked() {
                                    self.selected_level = Some(lv.clone());
                                }
                            }
                        });
                    });

                // Right column: Era Info & Deploy Action
                cols[1].label(
                    RichText::new("DEPLOYMENT BRIEFING")
                        .font(FontId::proportional(13.0))
                        .color(accent)
                        .strong(),
                );
                cols[1].add_space(8.0);

                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(Stroke::new(1.0, LINE))
                    .inner_margin(Margin::same(18))
                    .show(&mut cols[1], |ui| {
                        ui.label(
                            RichText::new(spec.display_name)
                                .font(FontId::proportional(22.0))
                                .strong()
                                .color(INK),
                        );
                        ui.label(RichText::new(era_subtitle(mode)).color(INK_MUTED));
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new(format!("MatchmakingModID: {}", spec.matchmaking_mod_id))
                                .font(FontId::monospace(12.0))
                                .color(INK_MUTED),
                        );
                        ui.label(
                            RichText::new(format!("Kit Classes: {}", spec.kit_count))
                                .font(FontId::monospace(12.0))
                                .color(INK_MUTED),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Selected Level: {}",
                                self.selected_level.as_deref().unwrap_or("(Main Menu)")
                            ))
                            .font(FontId::monospace(12.0))
                            .color(INK),
                        );
                        ui.add_space(22.0);

                        let busy = self.running_game.is_some();
                        let btn = egui::Button::new(
                            RichText::new(if busy {
                                "MATCH RUNNING..."
                            } else {
                                "DEPLOY INTO ENGINE"
                            })
                            .font(FontId::proportional(16.0))
                            .strong()
                            .color(Color32::WHITE),
                        )
                        .fill(if busy { SURFACE_3 } else { accent })
                        .min_size(Vec2::new(240.0, 44.0));

                        if ui.add_enabled(!busy, btn).clicked() {
                            self.launch_match(mode);
                        }
                    });
            });
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
            ui.columns(2, |cols| {
                // Profile editor
                cols[0].label(RichText::new("PLAYER PROFILE").color(ACCENT_TEXT).strong());
                cols[0].add_space(8.0);
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(Stroke::new(1.0, LINE))
                    .inner_margin(Margin::same(16))
                    .show(&mut cols[0], |ui| {
                        ui.label(RichText::new("Player Name").color(INK_MUTED));
                        ui.text_edit_singleline(&mut self.player_name);
                        ui.add_space(10.0);
                        ui.label(RichText::new("Clan Tag").color(INK_MUTED));
                        ui.text_edit_singleline(&mut self.clan_tag);
                        ui.add_space(14.0);
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Save Profile").color(Color32::WHITE))
                                    .fill(ACCENT),
                            )
                            .clicked()
                        {
                            let p = self.root.join("user").join("profile.ini");
                            let _ = fs::create_dir_all(self.root.join("user"));
                            let body = format!(
                                "[profile]\nname = \"{}\"\nclan_tag = \"{}\"\n",
                                self.player_name, self.clan_tag
                            );
                            if fs::write(&p, body).is_ok() {
                                self.status_line = "Saved user/profile.ini.".to_string();
                            }
                        }

                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.label(RichText::new("DISPLAY RESOLUTION (BFBC2-SYNCED)").color(ACCENT_TEXT).strong());
                        ui.add_space(6.0);
                        egui::ComboBox::from_id_salt("opfor_res")
                            .selected_text(format!("{} × {}", self.display_w, self.display_h))
                            .show_ui(ui, |ui| {
                                for &(w, h, label) in RESOLUTIONS {
                                    if ui
                                        .selectable_label(self.display_w == w && self.display_h == h, label)
                                        .clicked()
                                    {
                                        self.display_w = w;
                                        self.display_h = h;
                                    }
                                }
                            });
                        ui.add_space(6.0);
                        ui.checkbox(&mut self.fullscreen, "Fullscreen / Borderless");
                        ui.add_space(8.0);
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Apply Display").color(Color32::WHITE))
                                    .fill(ACCENT),
                            )
                            .clicked()
                        {
                            save_display(
                                &self.root.join("config").join("display.ini"),
                                self.display_w,
                                self.display_h,
                                self.fullscreen,
                            );
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                            if !self.fullscreen {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                                    self.display_w as f32,
                                    self.display_h as f32,
                                )));
                            }
                            self.status_line = format!(
                                "Display set to {}×{} ({})",
                                self.display_w,
                                self.display_h,
                                if self.fullscreen { "fullscreen" } else { "windowed" }
                            );
                        }
                    });

                // Discovered Modules
                cols[1].label(RichText::new("OPTION B MODULES").color(ACCENT_TEXT).strong());
                cols[1].add_space(8.0);
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(Stroke::new(1.0, LINE))
                    .inner_margin(Margin::same(16))
                    .show(&mut cols[1], |ui| {
                        for m in &self.modules {
                            ui.horizontal(|ui| {
                                let has = m.has_content_bundles();
                                ui.label(
                                    RichText::new(if has { "●" } else { "○" })
                                        .color(if has { ACCENT_TEXT } else { INK_FAINT }),
                                );
                                ui.label(RichText::new(&m.name).color(INK).strong());
                                ui.label(
                                    RichText::new(format!("({})", m.id))
                                        .font(FontId::monospace(11.0))
                                        .color(INK_MUTED),
                                );
                            });
                        }
                        ui.add_space(12.0);
                        if ui.button("Re-sync cache/package.mft").clicked() {
                            self.refresh_cache();
                        }
                    });
            });
        });
    }
}

fn apply_tool_suite_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = SURFACE_1;
    visuals.panel_fill = GROUND;
    visuals.faint_bg_color = SURFACE_2;
    visuals.extreme_bg_color = GROUND;
    visuals.widgets.noninteractive.bg_fill = SURFACE_1;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    visuals.widgets.inactive.bg_fill = SURFACE_2;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, INK);
    visuals.widgets.hovered.bg_fill = SURFACE_3;
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.selection.bg_fill = Color32::from_rgb(0x24, 0x40, 0x5f);
    ctx.set_visuals(visuals);
}

fn load_profile(path: &Path) -> (String, String) {
    let mut name = "Player".to_string();
    let mut tag = "OPFOR".to_string();
    if let Ok(txt) = fs::read_to_string(path) {
        for raw in txt.lines() {
            let line = raw.trim();
            if let Some((k, v)) = line.split_once('=') {
                let val = v.trim().trim_matches('"').to_string();
                match k.trim() {
                    "name" => name = val,
                    "clan_tag" | "clantag" => tag = val,
                    _ => {}
                }
            }
        }
    }
    (name, tag)
}

fn save_profile(path: &Path, name: &str, tag: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = format!("[profile]\nname={name}\nclantag={tag}\n");
    let _ = fs::write(path, content);
}

#[allow(dead_code)]
fn discover_levels_for_mode(root: &Path, mode: ModeId) -> Vec<String> {
    let mut dirs = Vec::new();
    match mode {
        ModeId::Bfbc2 => {
            dirs.push(root.join("engine").join("Package").join("levels"));
            dirs.push(root.join("modules").join("onslaught").join("levels"));
        }
        ModeId::Vietnam => dirs.push(root.join("modules").join("vietnam").join("levels")),
        ModeId::Bf1943 => dirs.push(root.join("modules").join("bf1943").join("levels")),
        ModeId::Bfbc1 => dirs.push(root.join("modules").join("bfbc1").join("levels")),
    }
    let mut out = Vec::new();
    for d in dirs {
        if let Ok(rd) = fs::read_dir(&d) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    let nm = e.file_name().to_string_lossy().into_owned();
                    if !nm.ends_with("_common") {
                        out.push(nm);
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn load_image_texture(
    ctx: &egui::Context,
    name: &str,
    candidates: &[PathBuf],
) -> Option<egui::TextureHandle> {
    for p in candidates {
        if let Ok(bytes) = fs::read(p) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_img = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                return Some(ctx.load_texture(name, color_img, egui::TextureOptions::LINEAR));
            }
        }
    }
    None
}

fn load_logo_texture(
    ctx: &egui::Context,
    root: &Path,
    mode: ModeId,
) -> Option<egui::TextureHandle> {
    let candidates = match mode {
        ModeId::Bf1943 => vec![
            root.join("modules").join("bf1943").join("art").join("x360").join("bf1943_logo.png"),
            root.join("modules").join("hub").join("logo_bf1943.png"),
        ],
        ModeId::Vietnam => vec![
            root.join("modules").join("hub").join("logo_vietnam.png"),
            root.join("modules").join("vietnam").join("logo.png"),
        ],
        ModeId::Bfbc1 => vec![
            root.join("modules").join("hub").join("logo_bfbc1.png"),
            root.join("modules").join("bfbc1").join("logo.png"),
        ],
        ModeId::Bfbc2 => vec![
            root.join("modules").join("hub").join("logo_bfbc2.png"),
            root.join("modules").join("bfbc2").join("logo.png"),
        ],
    };
    load_image_texture(ctx, &format!("logo_{mode:?}"), &candidates)
}

fn load_blade_texture(
    ctx: &egui::Context,
    root: &Path,
    mode: ModeId,
) -> Option<egui::TextureHandle> {
    let candidates = match mode {
        ModeId::Bfbc2 => vec![
            root.join("modules").join("hub").join("blade_bfbc2.png"),
            root.join("modules").join("hub").join("blade_bfbc2.jpg"),
        ],
        _ => {
            let slug = match mode {
                ModeId::Bf1943 => "bf1943",
                ModeId::Vietnam => "vietnam",
                ModeId::Bfbc1 => "bfbc1",
                ModeId::Bfbc2 => "bfbc2",
            };
            vec![
                root.join("modules").join(slug).join("blade.png"),
                root.join("modules").join(slug).join("blade.jpg"),
            ]
        }
    };
    load_image_texture(ctx, &format!("blade_{mode:?}"), &candidates)
}

fn load_display(path: &Path) -> (u32, u32, bool) {
    let mut w = 1280u32;
    let mut h = 720u32;
    let mut fs_flag = false;
    if let Ok(txt) = fs::read_to_string(path) {
        for raw in txt.lines() {
            if let Some((k, v)) = raw.trim().split_once('=') {
                let val = v.trim();
                match k.trim() {
                    "width" => w = val.parse().unwrap_or(1280),
                    "height" => h = val.parse().unwrap_or(720),
                    "fullscreen" => fs_flag = matches!(val, "1" | "true" | "yes"),
                    _ => {}
                }
            }
        }
    }
    (w.max(960), h.max(540), fs_flag)
}

fn save_display(path: &Path, w: u32, h: u32, fullscreen: bool) {
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let _ = fs::write(
        path,
        format!("[display]\nwidth = {w}\nheight = {h}\nfullscreen = {}\n", i32::from(fullscreen)),
    );
}

fn load_viewport_icon(path: &Path) -> Option<egui::IconData> {
    let bytes = fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    let root = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let (w, h, fullscreen) = load_display(&root.join("config").join("display.ini"));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Project OPFOR")
        .with_inner_size([w as f32, h as f32])
        .with_min_inner_size([960.0, 540.0])
        .with_fullscreen(fullscreen);
    if let Some(icon) = load_viewport_icon(&root.join("config").join("opfor.ico")) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Project OPFOR",
        options,
        Box::new(move |_cc| Ok(Box::new(OpforApp::new(root, w, h, fullscreen)))),
    )
}
