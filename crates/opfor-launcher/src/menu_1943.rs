//! Hand-crafted Battlefield 1943 (Pacific Theatre) Menu UI.
//!
//! Designed with authentic BF1943 design-language fidelity:
//! - Warm sun-bleached parchment cards (`#efe6cf`), weathered Pacific olive (`#2b3626`),
//!   deep naval slate (`#11181d`), coral-crimson stamps (`#c6472c`), and vintage brass (`#d8a648`).
//! - First-class PC Server Browser, Postcard Island Select, Expanded 4-Kit Arsenal (USMC / IJN),
//!   and offline Soldier Profile/Stats — launching directly into `OPFOREngine.exe` with zero
//!   Vietnam-menu grafting hacks.

use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, Vec2};
use opfor_vfs::ModeId;

use crate::input_monitor::InputDevice;
use crate::ui_state::{
    Bf1943Tab, BrowserModeFilter, GlobalUiState, PACIFIC_MAPS, PacificFaction,
};

// Battlefield 1943 Pacific Theatre Palette
const NAVY_BG: Color32 = Color32::from_rgb(17, 24, 29);
const OLIVE_PANEL: Color32 = Color32::from_rgb(33, 42, 32);
const OLIVE_DARK: Color32 = Color32::from_rgb(23, 30, 23);
const PARCHMENT: Color32 = Color32::from_rgb(238, 229, 206);
const PARCHMENT_DIM: Color32 = Color32::from_rgb(206, 195, 168);
const INK_DARK: Color32 = Color32::from_rgb(32, 30, 24);
const INK_LIGHT: Color32 = Color32::from_rgb(241, 236, 221);
const INK_MUTED: Color32 = Color32::from_rgb(176, 173, 158);
const CORAL_RED: Color32 = Color32::from_rgb(198, 71, 44);
const BRASS: Color32 = Color32::from_rgb(216, 166, 72);
const LINE_OLIVE: Color32 = Color32::from_rgb(66, 78, 58);

pub enum Bf1943Action {
    None,
    BackToHub,
    SwitchMode(ModeId),
    OpenSettings,
    DeployMap { level_path: String },
    SaveProfile,
}

struct KitSpec {
    name: &'static str,
    role: &'static str,
    usmc_primary: &'static str,
    ijn_primary: &'static str,
    sidearm: &'static str,
    gadgets: &'static str,
}

const PACIFIC_KITS: &[KitSpec] = &[
    KitSpec {
        name: "RIFLEMAN",
        role: "Medium-range semi-auto precision & rifle grenades",
        usmc_primary: "M1 Garand",
        ijn_primary: "Type 5 Semi-Auto",
        sidearm: "M1911A1 / Nambu",
        gadgets: "Rifle Grenade · Frag Grenades · Bayonet",
    },
    KitSpec {
        name: "INFANTRY",
        role: "Close-quarters SMG & anti-armor demolition",
        usmc_primary: "M1A1 Thompson",
        ijn_primary: "Type 100 SMG",
        sidearm: "M1911A1 / Nambu",
        gadgets: "M1A1 Bazooka · Repair Wrench · Frag Grenades",
    },
    KitSpec {
        name: "SCOUT",
        role: "Long-range recon, sniping & sabotage",
        usmc_primary: "M1903 Springfield Scoped",
        ijn_primary: "Type 99 Arisaka Scoped",
        sidearm: "M1911A1 / Nambu",
        gadgets: "TNT Detpack · Combat Knife / Katana · Binoculars",
    },
    KitSpec {
        name: "SUPPORT (OPFOR EXPANDED)",
        role: "Suppressive LMG fire, ammo supply & fortification",
        usmc_primary: "BAR M1918A2",
        ijn_primary: "Type 99 LMG",
        sidearm: "M1911A1 / Nambu",
        gadgets: "Ammo Box · Anti-Tank Mines · Smoke Grenades",
    },
];

pub fn render_bf1943_menu(
    ui: &mut egui::Ui,
    state: &mut GlobalUiState,
    backdrop: Option<&egui::TextureHandle>,
    map_thumbs: &[Option<egui::TextureHandle>; 4],
    player_name: &mut String,
    clan_tag: &mut String,
    status_line: &str,
) -> Bf1943Action {
    let mut action = Bf1943Action::None;

    // 1. Top BF1943 Header + Global Mode Switcher
    egui::Panel::top("bf1943_top_bar")
        .exact_size(64.0)
        .frame(
            egui::Frame::new()
                .fill(OLIVE_DARK)
                .inner_margin(Margin::symmetric(20, 10))
                .stroke(Stroke::new(1.5, BRASS)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                // Back to Global OPFOR Hub
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("◄ OPFOR HUB")
                                .font(FontId::proportional(13.0))
                                .strong()
                                .color(PARCHMENT),
                        )
                        .fill(OLIVE_PANEL)
                        .stroke(Stroke::new(1.0, BRASS))
                        .corner_radius(CornerRadius::same(3)),
                    )
                    .clicked()
                {
                    action = Bf1943Action::BackToHub;
                }

                ui.add_space(14.0);
                ui.label(
                    RichText::new("BATTLEFIELD 1943")
                        .font(FontId::proportional(24.0))
                        .strong()
                        .color(PARCHMENT),
                );
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(CORAL_RED)
                    .inner_margin(Margin::symmetric(8, 3))
                    .corner_radius(CornerRadius::same(2))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("PACIFIC THEATRE")
                                .font(FontId::proportional(11.0))
                                .strong()
                                .color(Color32::WHITE),
                        );
                    });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("SETTINGS")
                                    .font(FontId::proportional(12.0))
                                    .strong()
                                    .color(INK_LIGHT),
                            )
                            .fill(OLIVE_PANEL)
                            .stroke(Stroke::new(1.0, LINE_OLIVE)),
                        )
                        .clicked()
                    {
                        action = Bf1943Action::OpenSettings;
                    }
                    ui.add_space(6.0);
                    for (label, target) in [("VIETNAM", ModeId::Vietnam), ("BFBC2", ModeId::Bfbc2)] {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(label)
                                        .font(FontId::proportional(11.5))
                                        .color(INK_MUTED),
                                )
                                .fill(NAVY_BG)
                                .stroke(Stroke::new(1.0, LINE_OLIVE)),
                            )
                            .clicked()
                        {
                            action = Bf1943Action::SwitchMode(target);
                        }
                        ui.add_space(4.0);
                    }
                    ui.label(
                        RichText::new("SWITCH ERA:")
                            .font(FontId::proportional(11.0))
                            .color(INK_MUTED),
                    );
                });
            });
        });

    // 2. Sub-Navigation Bar (PLAY | SERVER BROWSER | KITS & ARSENAL | SOLDIER)
    egui::Panel::top("bf1943_nav_bar")
        .exact_size(44.0)
        .frame(
            egui::Frame::new()
                .fill(NAVY_BG)
                .inner_margin(Margin::symmetric(24, 6))
                .stroke(Stroke::new(1.0, LINE_OLIVE)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                for (tab, label) in [
                    (Bf1943Tab::Play, "ISLAND SELECT"),
                    (Bf1943Tab::ServerBrowser, "OFFLINE SERVER BROWSER"),
                    (Bf1943Tab::Kits, "KITS & ARSENAL"),
                    (Bf1943Tab::Soldier, "SOLDIER & STATS"),
                ] {
                    let active = state.bf1943_tab == tab;
                    let btn = egui::Button::new(
                        RichText::new(label)
                            .font(FontId::proportional(13.5))
                            .strong()
                            .color(if active { INK_DARK } else { PARCHMENT_DIM }),
                    )
                    .fill(if active { BRASS } else { Color32::TRANSPARENT })
                    .stroke(Stroke::new(1.0, if active { BRASS } else { LINE_OLIVE }))
                    .corner_radius(CornerRadius::same(2));
                    if ui.add(btn).clicked() {
                        state.bf1943_tab = tab;
                    }
                    ui.add_space(8.0);
                }
            });
        });

    // 3. Bottom Status Strip with Dynamic Input Device Prompts (KBM <-> Xbox Gamepad)
    egui::Panel::bottom("bf1943_bottom_bar")
        .exact_size(36.0)
        .frame(
            egui::Frame::new()
                .fill(OLIVE_DARK)
                .inner_margin(Margin::symmetric(20, 7))
                .stroke(Stroke::new(1.0, LINE_OLIVE)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let prompts = match state.input_device {
                    InputDevice::KeyboardMouse => {
                        "[← →] Select Island   [Click / Enter] Deploy   [Esc] Back to OPFOR Hub"
                    }
                    InputDevice::GamepadXbox => {
                        "[D-Pad / LS] Select   [LB / RB] Switch Tab   [A] Deploy   [B] OPFOR Hub"
                    }
                };
                ui.label(
                    RichText::new(prompts)
                        .font(FontId::proportional(12.0))
                        .strong()
                        .color(PARCHMENT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let badge = match state.input_device {
                        InputDevice::KeyboardMouse => "INPUT: KEYBOARD & MOUSE",
                        InputDevice::GamepadXbox => "INPUT: XBOX CONTROLLER",
                    };
                    ui.label(
                        RichText::new(format!("{badge}  ·  {status_line}"))
                            .font(FontId::proportional(11.0))
                            .color(BRASS),
                    );
                });
            });
        });

    // 4. Main Content Area with Subtle Pacific Backdrop
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(NAVY_BG).inner_margin(Margin::same(22)))
        .show(ui, |ui| {
            let full_rect = ui.max_rect();
            if let Some(tex) = backdrop {
                let ts = tex.size_vec2();
                let scale = (full_rect.width() / ts.x).max(full_rect.height() / ts.y);
                let uv_w = (full_rect.width() / (ts.x * scale)).min(1.0);
                let uv_h = (full_rect.height() / (ts.y * scale)).min(1.0);
                let uv = egui::Rect::from_min_max(
                    egui::pos2(0.5 - uv_w * 0.5, 0.5 - uv_h * 0.5),
                    egui::pos2(0.5 + uv_w * 0.5, 0.5 + uv_h * 0.5),
                );
                ui.painter().image(
                    tex.id(),
                    full_rect,
                    uv,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 34),
                );
            }

            match state.bf1943_tab {
                Bf1943Tab::Play => {
                    if let Some(lvl) = render_play_tab(ui, state, map_thumbs) {
                        action = Bf1943Action::DeployMap { level_path: lvl };
                    }
                }
                Bf1943Tab::ServerBrowser => {
                    if let Some(lvl) = render_browser_tab(ui, state) {
                        action = Bf1943Action::DeployMap { level_path: lvl };
                    }
                }
                Bf1943Tab::Kits => render_kits_tab(ui, state),
                Bf1943Tab::Soldier => {
                    if render_soldier_tab(ui, state, player_name, clan_tag) {
                        action = Bf1943Action::SaveProfile;
                    }
                }
            }
        });

    action
}

fn render_play_tab(
    ui: &mut egui::Ui,
    state: &mut GlobalUiState,
    map_thumbs: &[Option<egui::TextureHandle>; 4],
) -> Option<String> {
    let mut deploy_level = None;

    ui.label(
        RichText::new("SELECT PACIFIC OPERATION")
            .font(FontId::proportional(13.0))
            .strong()
            .color(BRASS),
    );
    ui.add_space(10.0);

    // 4 Vintage Pacific Postcard Cards across the top
    let avail_w = ui.available_width();
    let gap = 14.0;
    let card_w = ((avail_w - gap * 3.0) / 4.0).max(210.0);

    ui.horizontal(|ui| {
        for (i, map) in PACIFIC_MAPS.iter().enumerate() {
            let selected = state.bf1943_selected_map == i;
            let border = if selected {
                Stroke::new(3.0, CORAL_RED)
            } else {
                Stroke::new(1.5, PARCHMENT_DIM)
            };
            let frame = egui::Frame::new()
                .fill(PARCHMENT)
                .stroke(border)
                .corner_radius(CornerRadius::same(3))
                .inner_margin(Margin::same(10));

            let resp = frame.show(ui, |ui| {
                ui.set_width(card_w - 22.0);
                // Map preview image
                let img_h = 128.0;
                let (img_rect, img_resp) =
                    ui.allocate_exact_size(Vec2::new(card_w - 22.0, img_h), egui::Sense::click());
                if let Some(tex) = &map_thumbs[i] {
                    ui.painter().image(
                        tex.id(),
                        img_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    ui.painter().rect_filled(img_rect, 2.0, OLIVE_PANEL);
                }
                // Stamp badge in top-right of postcard photo
                let stamp_rect = egui::Rect::from_min_size(
                    egui::pos2(img_rect.right() - 108.0, img_rect.top() + 8.0),
                    Vec2::new(100.0, 20.0),
                );
                ui.painter().rect_filled(stamp_rect, 2.0, CORAL_RED);
                ui.painter().text(
                    stamp_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    map.mode_label,
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );

                ui.add_space(8.0);
                ui.label(
                    RichText::new(map.title)
                        .font(FontId::proportional(17.0))
                        .strong()
                        .color(INK_DARK),
                );
                ui.label(
                    RichText::new(map.subtitle)
                        .font(FontId::proportional(10.5))
                        .color(Color32::from_rgb(95, 86, 68)),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{} · {} PLAYERS", map.stamp, map.max_players))
                        .font(FontId::proportional(10.5))
                        .strong()
                        .color(CORAL_RED),
                );

                if img_resp.clicked() {
                    state.bf1943_selected_map = i;
                }
                if img_resp.double_clicked() {
                    deploy_level = Some(map.level_path.to_string());
                }
            });

            if resp.response.interact(egui::Sense::click()).clicked() {
                state.bf1943_selected_map = i;
            }
            if i < 3 {
                ui.add_space(gap);
            }
        }
    });

    ui.add_space(18.0);

    // Selected Operation Briefing & Deploy Banner
    let sel = &PACIFIC_MAPS[state.bf1943_selected_map.min(PACIFIC_MAPS.len() - 1)];
    egui::Frame::new()
        .fill(OLIVE_PANEL)
        .stroke(Stroke::new(1.5, BRASS))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 270.0);
                    ui.label(
                        RichText::new(format!("{} — {}", sel.title, sel.mode_label))
                            .font(FontId::proportional(20.0))
                            .strong()
                            .color(PARCHMENT),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(sel.briefing)
                            .font(FontId::proportional(13.5))
                            .color(INK_LIGHT),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "Engine Target: {} · Full WW2 Arsenal Unlocked · Repair Stations, Tank Horn [H] & LCVP Release Active",
                            sel.level_path
                        ))
                        .font(FontId::proportional(11.5))
                        .color(BRASS),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let deploy_label = match state.input_device {
                        InputDevice::KeyboardMouse => "DEPLOY TO ISLAND ▶",
                        InputDevice::GamepadXbox => "[A] DEPLOY TO ISLAND",
                    };
                    let deploy_btn = egui::Button::new(
                        RichText::new(deploy_label)
                            .font(FontId::proportional(16.0))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(CORAL_RED)
                    .min_size(Vec2::new(235.0, 52.0))
                    .corner_radius(CornerRadius::same(3));
                    if ui.add(deploy_btn).clicked() {
                        deploy_level = Some(sel.level_path.to_string());
                    }
                });
            });
        });

    deploy_level
}

fn render_browser_tab(ui: &mut egui::Ui, state: &mut GlobalUiState) -> Option<String> {
    let mut join_level = None;

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("MODE FILTER:")
                .font(FontId::proportional(12.0))
                .strong()
                .color(BRASS),
        );
        for (f, label) in [
            (BrowserModeFilter::All, "ALL"),
            (BrowserModeFilter::Conquest, "CONQUEST"),
            (BrowserModeFilter::AirSuperiority, "AIR SUPERIORITY"),
        ] {
            let active = state.bf1943_browser_filter == f;
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(label)
                            .font(FontId::proportional(12.0))
                            .strong()
                            .color(if active { INK_DARK } else { PARCHMENT_DIM }),
                    )
                    .fill(if active { BRASS } else { OLIVE_PANEL }),
                )
                .clicked()
            {
                state.bf1943_browser_filter = f;
            }
        }
        ui.add_space(16.0);
        ui.label(
            RichText::new("SEARCH:")
                .font(FontId::proportional(12.0))
                .color(INK_MUTED),
        );
        ui.text_edit_singleline(&mut state.bf1943_browser_search);
    });

    ui.add_space(12.0);

    egui::Frame::new()
        .fill(OLIVE_DARK)
        .stroke(Stroke::new(1.0, LINE_OLIVE))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            let q = state.bf1943_browser_search.to_ascii_lowercase();
            for map in PACIFIC_MAPS {
                let mode_ok = match state.bf1943_browser_filter {
                    BrowserModeFilter::All => true,
                    BrowserModeFilter::Conquest => map.mode_label == "CONQUEST",
                    BrowserModeFilter::AirSuperiority => map.mode_label == "AIR SUPERIORITY",
                };
                let text_ok = q.is_empty()
                    || map.title.to_ascii_lowercase().contains(&q)
                    || map.mode_label.to_ascii_lowercase().contains(&q);
                if !mode_ok || !text_ok {
                    continue;
                }

                egui::Frame::new()
                    .fill(OLIVE_PANEL)
                    .stroke(Stroke::new(1.0, LINE_OLIVE))
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(
                                RichText::new(format!("OPFOR PACIFIC — {}", map.title))
                                    .font(FontId::proportional(14.5))
                                    .strong()
                                    .color(PARCHMENT),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(map.mode_label)
                                    .font(FontId::proportional(12.0))
                                    .strong()
                                    .color(BRASS),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(format!("LOCAL · 0 ms · 0/{}", map.max_players))
                                    .font(FontId::proportional(12.0))
                                    .color(INK_MUTED),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("JOIN SERVER ▶")
                                                .font(FontId::proportional(12.5))
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .fill(CORAL_RED)
                                        .min_size(Vec2::new(130.0, 30.0)),
                                    )
                                    .clicked()
                                {
                                    join_level = Some(map.level_path.to_string());
                                }
                            });
                        });
                    });
                ui.add_space(6.0);
            }
        });

    join_level
}

fn render_kits_tab(ui: &mut egui::Ui, state: &mut GlobalUiState) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("FACTION:")
                .font(FontId::proportional(13.0))
                .strong()
                .color(BRASS),
        );
        for (fac, label) in [
            (PacificFaction::Usmc, "US MARINE CORPS (USMC)"),
            (PacificFaction::Ijn, "IMPERIAL JAPANESE NAVY (IJN)"),
        ] {
            let active = state.bf1943_faction == fac;
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(label)
                            .font(FontId::proportional(12.5))
                            .strong()
                            .color(if active { INK_DARK } else { PARCHMENT_DIM }),
                    )
                    .fill(if active { BRASS } else { OLIVE_PANEL }),
                )
                .clicked()
            {
                state.bf1943_faction = fac;
            }
        }
    });

    ui.add_space(14.0);

    for (i, kit) in PACIFIC_KITS.iter().enumerate() {
        let selected = state.bf1943_selected_kit == i;
        let primary = match state.bf1943_faction {
            PacificFaction::Usmc => kit.usmc_primary,
            PacificFaction::Ijn => kit.ijn_primary,
        };
        egui::Frame::new()
            .fill(if selected { PARCHMENT } else { OLIVE_PANEL })
            .stroke(Stroke::new(
                if selected { 2.0 } else { 1.0 },
                if selected { CORAL_RED } else { LINE_OLIVE },
            ))
            .inner_margin(Margin::symmetric(16, 12))
            .show(ui, |ui| {
                let title_col = if selected { INK_DARK } else { PARCHMENT };
                let body_col = if selected { Color32::from_rgb(64, 58, 45) } else { INK_LIGHT };
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(kit.name)
                                .font(FontId::proportional(16.0))
                                .strong()
                                .color(title_col),
                        );
                        ui.label(
                            RichText::new(kit.role)
                                .font(FontId::proportional(12.5))
                                .color(body_col),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!(
                                "Primary: {primary}  |  Sidearm: {}  |  Gear: {}",
                                kit.sidearm, kit.gadgets
                            ))
                            .font(FontId::proportional(12.0))
                            .strong()
                            .color(if selected { CORAL_RED } else { BRASS }),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(if selected { "SELECTED" } else { "INSPECT" })
                                    .fill(if selected { CORAL_RED } else { NAVY_BG }),
                            )
                            .clicked()
                        {
                            state.bf1943_selected_kit = i;
                        }
                    });
                });
            });
        ui.add_space(8.0);
    }
}

fn render_soldier_tab(
    ui: &mut egui::Ui,
    state: &GlobalUiState,
    player_name: &mut String,
    clan_tag: &mut String,
) -> bool {
    let mut saved = false;
    egui::Frame::new()
        .fill(OLIVE_PANEL)
        .stroke(Stroke::new(1.5, BRASS))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.label(
                RichText::new("PACIFIC SERVICE RECORD (OFFLINE)")
                    .font(FontId::proportional(17.0))
                    .strong()
                    .color(PARCHMENT),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Soldier Name:").color(INK_LIGHT));
                ui.text_edit_singleline(player_name);
                ui.add_space(12.0);
                ui.label(RichText::new("Clan Tag:").color(INK_LIGHT));
                ui.text_edit_singleline(clan_tag);
                ui.add_space(12.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("SAVE PROFILE")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(CORAL_RED),
                    )
                    .clicked()
                {
                    saved = true;
                }
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                for (label, val) in [
                    ("TOTAL SCORE", format!("{:.0}", state.stat("score"))),
                    ("RANK", format!("{:.0}", state.stat("rank"))),
                    ("KILLS", format!("{:.0}", state.stat("kills"))),
                    ("WINS", format!("{:.0}", state.stat("wins"))),
                ] {
                    egui::Frame::new()
                        .fill(PARCHMENT)
                        .stroke(Stroke::new(1.0, BRASS))
                        .inner_margin(Margin::symmetric(18, 12))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(val)
                                        .font(FontId::proportional(24.0))
                                        .strong()
                                        .color(INK_DARK),
                                );
                                ui.label(
                                    RichText::new(label)
                                        .font(FontId::proportional(11.0))
                                        .strong()
                                        .color(CORAL_RED),
                                );
                            });
                        });
                    ui.add_space(10.0);
                }
            });
        });
    saved
}
