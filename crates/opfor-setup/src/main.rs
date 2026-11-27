//! Battlefield OPFOR Setup Wizard (`Setup.exe`).
//!
//! Guides the end user through staging their PC BFBC2 runtime (strictly read-only on the
//! original BFBC2 installation) and porting their Xbox 360 BFBC1 ISO and/or BF1943 XBLA
//! package via pure-Rust `utility/` binaries (`opfor-cli.exe` and `xenolith.exe`),
//! then launches `OPFOR.exe` directly into the Hub.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(unsafe_code)]

mod win_dialog;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bfbc1Mode {
    Oasis,
    MpAll,
    SpAll,
    Full,
}

impl Bfbc1Mode {
    const fn as_arg(self) -> &'static str {
        match self {
            Self::Oasis => "oasis",
            Self::MpAll => "mp",
            Self::SpAll => "sp",
            Self::Full => "all",
        }
    }

    const fn includes_sp(self) -> bool {
        matches!(self, Self::SpAll | Self::Full)
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Oasis => "Vertical Slice (Oasis + Menus + Fonts/Loc)",
            Self::MpAll => "All 8 Gold Rush MP Maps + Menus + Fonts/Loc",
            Self::SpAll => "All 7 SP Campaign Missions + UI/Movies/Navmeshes",
            Self::Full => "Full Port (8 MP + 7 SP + Fonts/Loc + Movies + Navmeshes)",
        }
    }
}

#[derive(Default)]
struct SharedProgress {
    running: bool,
    done: bool,
    succeeded: bool,
    fraction: f32,
    status: String,
    logs: Vec<String>,
}

struct SetupApp {
    opfor_dir: String,
    bfbc2_dir: String,
    bfbc1_iso: String,
    bf1943_xbla: String,
    stage_bfbc2: bool,
    port_bfbc1: bool,
    port_bf1943: bool,
    bfbc1_mode: Bfbc1Mode,
    launch_after: bool,
    auto_launched: bool,
    progress: Arc<Mutex<SharedProgress>>,
}

impl Default for SetupApp {
    fn default() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));

        let bfbc2_default = [
            r"C:\Games\Battlefield Bad Company 2",
            r"C:\Program Files (x86)\Steam\steamapps\common\Battlefield Bad Company 2",
            r"C:\Program Files\EA Games\Battlefield Bad Company 2",
        ]
        .iter()
        .find(|p| Path::new(p).join("BFBC2Game.exe").is_file())
        .map(|s| (*s).to_owned())
        .unwrap_or_default();

        let bf1943_default = r"C:\Games\Battlefield 1943\945CD965D6B2B2584A9C06CE16F38A075A6D390558";
        let bf1943_xbla = if Path::new(bf1943_default).is_file() {
            bf1943_default.to_owned()
        } else {
            String::new()
        };

        Self {
            opfor_dir: exe_dir.display().to_string(),
            bfbc2_dir: bfbc2_default,
            bfbc1_iso: String::new(),
            bf1943_xbla: bf1943_xbla.clone(),
            stage_bfbc2: true,
            port_bfbc1: false,
            port_bf1943: !bf1943_xbla.is_empty(),
            bfbc1_mode: Bfbc1Mode::Full,
            launch_after: true,
            auto_launched: false,
            progress: Arc::new(Mutex::new(SharedProgress {
                status: "Select your source game files and click Install & Port.".into(),
                ..Default::default()
            })),
        }
    }
}

fn pick_folder(title: &str) -> Option<String> {
    win_dialog::pick_folder(title)
}

fn pick_file(title: &str, filter: &str) -> Option<String> {
    win_dialog::pick_file(title, filter)
}

fn run_step(
    progress: &Arc<Mutex<SharedProgress>>,
    label: &str,
    program: &str,
    args: &[String],
) -> Result<(), String> {
    if let Ok(mut p) = progress.lock() {
        p.status = label.to_owned();
        p.logs.push(format!("=== {label} ==="));
    }
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start {program}: {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(mut p) = progress.lock() {
                p.logs.push(line);
            }
        }
    }
    let status = child.wait().map_err(|e| format!("wait error: {e}"))?;
    if !status.success() {
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(mut p) = progress.lock() {
                    p.logs.push(format!("stderr: {line}"));
                }
            }
        }
        return Err(format!("{label} failed (exit {:?})", status.code()));
    }
    Ok(())
}

impl SetupApp {
    fn start_install(&mut self) {
        let opfor_dir = self.opfor_dir.trim().to_owned();
        let bfbc2_dir = self.bfbc2_dir.trim().to_owned();
        let bfbc1_iso = self.bfbc1_iso.trim().to_owned();
        let bf1943_xbla = self.bf1943_xbla.trim().to_owned();
        let stage_bfbc2 = self.stage_bfbc2;
        let port_bfbc1 = self.port_bfbc1 && !bfbc1_iso.is_empty();
        let port_bf1943 = self.port_bf1943 && !bf1943_xbla.is_empty();
        let bfbc1_mode = self.bfbc1_mode;
        let progress = Arc::clone(&self.progress);
        self.auto_launched = false;

        if let Ok(mut p) = progress.lock() {
            p.running = true;
            p.done = false;
            p.succeeded = false;
            p.fraction = 0.02;
            p.logs.clear();
            p.status = "Starting Battlefield OPFOR Setup...".into();
        }

        thread::spawn(move || {
            let util = PathBuf::from(&opfor_dir).join("utility");
            let opfor_cli = util.join("opfor-cli.exe").display().to_string();
            let mut steps: Vec<(String, String, Vec<String>)> = Vec::new();

            if stage_bfbc2 {
                steps.push((
                    "Staging PC BFBC2 Runtime (original untouched)".into(),
                    opfor_cli.clone(),
                    vec![
                        "setup-bfbc2".into(),
                        "--bfbc2".into(),
                        bfbc2_dir.clone(),
                        "--opfor".into(),
                        opfor_dir.clone(),
                    ],
                ));
            }
            if port_bfbc1 {
                steps.push((
                    format!("Porting BFBC1 ({})", bfbc1_mode.label()),
                    opfor_cli.clone(),
                    vec![
                        "port-bfbc1".into(),
                        "--iso".into(),
                        bfbc1_iso.clone(),
                        "--bfbc2".into(),
                        bfbc2_dir.clone(),
                        "--opfor".into(),
                        opfor_dir.clone(),
                        "--mode".into(),
                        bfbc1_mode.as_arg().into(),
                    ],
                ));
                steps.push((
                    "Building BFBC1 Fonts, English Localization & SP Loading Music".into(),
                    opfor_cli.clone(),
                    vec![
                        "bfbc1-ui-parity".into(),
                        "--opfor".into(),
                        opfor_dir.clone(),
                        "--bfbc2".into(),
                        bfbc2_dir.clone(),
                        "--iso".into(),
                        bfbc1_iso.clone(),
                    ],
                ));
                steps.push((
                    "Porting BFBC1 Native Main Menu & 3D Gunrack (PC Shaders)".into(),
                    opfor_cli.clone(),
                    vec![
                        "bfbc1-mainmenu".into(),
                        "--iso".into(),
                        bfbc1_iso.clone(),
                        "--bfbc2".into(),
                        bfbc2_dir.clone(),
                        "--opfor".into(),
                        opfor_dir.clone(),
                    ],
                ));
                steps.push((
                    "Applying BFBC1 Ingame Menus, Minimap, Icons & Dual KBM/Gamepad Parity".into(),
                    opfor_cli.clone(),
                    vec![
                        "era-ui-input-parity".into(),
                        "--era".into(),
                        "bfbc1".into(),
                        "--opfor".into(),
                        opfor_dir.clone(),
                    ],
                ));
                if bfbc1_mode.includes_sp() {
                    steps.push((
                        "Extracting BFBC1 Campaign Bink Movies (64B -> 72B FB1.5)".into(),
                        opfor_cli.clone(),
                        vec![
                            "bfbc1-movies".into(),
                            "--iso".into(),
                            bfbc1_iso.clone(),
                            "--opfor".into(),
                            opfor_dir.clone(),
                        ],
                    ));
                    steps.push((
                        "Upgrading BFBC1 Kynogon HPD v1 -> v2 AI Navmeshes".into(),
                        opfor_cli.clone(),
                        vec![
                            "bfbc1-navmeshes".into(),
                            "--iso".into(),
                            bfbc1_iso,
                            "--opfor".into(),
                            opfor_dir.clone(),
                        ],
                    ));
                }
            }
            if port_bf1943 {
                steps.push((
                    "Porting Battlefield 1943 (XBLA STFS)".into(),
                    opfor_cli.clone(),
                    vec![
                        "port-bf1943".into(),
                        "--xbla".into(),
                        bf1943_xbla,
                        "--bfbc2".into(),
                        bfbc2_dir,
                        "--opfor".into(),
                        opfor_dir.clone(),
                    ],
                ));
                steps.push((
                    "Applying BF1943 Ingame Menus, Icons & Mouse Navigation Parity".into(),
                    opfor_cli,
                    vec![
                        "era-ui-input-parity".into(),
                        "--era".into(),
                        "bf1943".into(),
                        "--opfor".into(),
                        opfor_dir,
                    ],
                ));
            }

            if steps.is_empty() {
                if let Ok(mut p) = progress.lock() {
                    p.running = false;
                    p.done = true;
                    p.succeeded = false;
                    p.status = "No tasks selected (enable at least one option).".into();
                }
                return;
            }

            let total = steps.len() as f32;
            let mut ok = true;
            for (idx, (label, program, args)) in steps.into_iter().enumerate() {
                if let Ok(mut p) = progress.lock() {
                    p.fraction = idx as f32 / total;
                }
                if let Err(err) = run_step(&progress, &label, &program, &args) {
                    if let Ok(mut p) = progress.lock() {
                        p.logs.push(format!("ERROR: {err}"));
                        p.status = format!("Failed: {err}");
                    }
                    ok = false;
                    break;
                }
            }

            if let Ok(mut p) = progress.lock() {
                p.running = false;
                p.done = true;
                p.succeeded = ok;
                if ok {
                    p.fraction = 1.0;
                    p.status = "Setup complete! Original BFBC2 installation untouched.".into();
                }
            }
        });
    }

    fn launch_hub(&self) {
        let exe = PathBuf::from(self.opfor_dir.trim()).join("OPFOR.exe");
        if exe.is_file() {
            let _ = Command::new(&exe).current_dir(self.opfor_dir.trim()).spawn();
        }
    }
}

impl eframe::App for SetupApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(15, 18, 22);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(26, 31, 38);
        ui.ctx().set_visuals(visuals);

        let (running, done, succeeded, fraction, status, logs) = self
            .progress
            .lock()
            .map(|p| (p.running, p.done, p.succeeded, p.fraction, p.status.clone(), p.logs.clone()))
            .unwrap_or_default();

        if running {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        if done && succeeded && self.launch_after && !self.auto_launched {
            self.auto_launched = true;
            self.launch_hub();
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("BATTLEFIELD OPFOR")
                        .size(22.0)
                        .strong()
                        .color(egui::Color32::from_rgb(224, 168, 46)),
                );
                ui.label(
                    egui::RichText::new("— Client Setup & Console Porting Utility")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(185, 195, 208)),
                );
            });
            ui.label(
                egui::RichText::new(
                    "Points to your own BFBC2 PC install, BFBC1 Xbox 360 ISO, and BF1943 XBLA package. \
                     Your original BFBC2 folder and console images are strictly read-only; saves are isolated per game under Documents\\Project OPFOR\\.",
                )
                .size(12.0)
                .color(egui::Color32::from_rgb(140, 152, 168)),
            );
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            ui.add_enabled_ui(!running, |ui| {
                egui::Grid::new("paths_grid")
                    .num_columns(3)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("OPFOR Folder:");
                        ui.add(egui::TextEdit::singleline(&mut self.opfor_dir).desired_width(480.0));
                        if ui.button("Browse...").clicked()
                            && let Some(p) = pick_folder("Select Battlefield OPFOR folder")
                        {
                            self.opfor_dir = p;
                        }
                        ui.end_row();

                        ui.label("BFBC2 PC Folder (read-only):");
                        ui.add(egui::TextEdit::singleline(&mut self.bfbc2_dir).desired_width(480.0));
                        if ui.button("Browse...").clicked()
                            && let Some(p) = pick_folder("Select Battlefield: Bad Company 2 installation")
                        {
                            self.bfbc2_dir = p;
                        }
                        ui.end_row();

                        ui.label("BFBC1 Xbox 360 ISO (optional):");
                        ui.add(egui::TextEdit::singleline(&mut self.bfbc1_iso).desired_width(480.0));
                        if ui.button("Browse...").clicked()
                            && let Some(p) = pick_file("Select BFBC1 Xbox 360 ISO", "ISO Images (*.iso)|*.iso|All Files (*.*)|*.*")
                        {
                            self.bfbc1_iso = p;
                            self.port_bfbc1 = true;
                        }
                        ui.end_row();

                        ui.label("BF1943 XBLA File (optional):");
                        ui.add(egui::TextEdit::singleline(&mut self.bf1943_xbla).desired_width(480.0));
                        if ui.button("Browse...").clicked()
                            && let Some(p) = pick_file("Select BF1943 XBLA STFS Package", "All Files (*.*)|*.*")
                        {
                            self.bf1943_xbla = p;
                            self.port_bf1943 = true;
                        }
                        ui.end_row();
                    });

                ui.add_space(10.0);
                ui.checkbox(&mut self.stage_bfbc2, "Stage PC BFBC2 runtime into OPFOR/engine (original BFBC2 untouched)");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.port_bfbc1, "Port Battlefield: Bad Company 1 (ISO)");
                    egui::ComboBox::from_id_salt("bfbc1_mode")
                        .selected_text(self.bfbc1_mode.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.bfbc1_mode, Bfbc1Mode::Oasis, Bfbc1Mode::Oasis.label());
                            ui.selectable_value(&mut self.bfbc1_mode, Bfbc1Mode::MpAll, Bfbc1Mode::MpAll.label());
                            ui.selectable_value(&mut self.bfbc1_mode, Bfbc1Mode::SpAll, Bfbc1Mode::SpAll.label());
                            ui.selectable_value(&mut self.bfbc1_mode, Bfbc1Mode::Full, Bfbc1Mode::Full.label());
                        });
                });
                ui.checkbox(&mut self.port_bf1943, "Port Battlefield 1943 (XBLA STFS)");
                ui.checkbox(&mut self.launch_after, "Launch OPFOR.exe (Hub) automatically when finished");
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let can_start = !running && !self.bfbc2_dir.trim().is_empty() && !self.opfor_dir.trim().is_empty();
                if ui
                    .add_enabled(
                        can_start,
                        egui::Button::new(egui::RichText::new("Install & Port").strong().size(15.0))
                            .fill(egui::Color32::from_rgb(224, 168, 46))
                            .min_size(egui::vec2(160.0, 34.0)),
                    )
                    .clicked()
                {
                    self.start_install();
                }

                if ui
                    .add_enabled(
                        !running && Path::new(self.opfor_dir.trim()).join("OPFOR.exe").is_file(),
                        egui::Button::new(egui::RichText::new("Launch OPFOR Hub").size(14.0))
                            .min_size(egui::vec2(150.0, 34.0)),
                    )
                    .clicked()
                {
                    self.launch_hub();
                }
            });

            ui.add_space(10.0);
            ui.add(egui::ProgressBar::new(fraction).text(&status).animate(running));
            ui.add_space(8.0);

            egui::Frame::new()
                .fill(egui::Color32::from_rgb(10, 12, 16))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &logs {
                                ui.label(egui::RichText::new(line).monospace().size(11.5));
                            }
                        });
                });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Battlefield OPFOR — Setup & Porting Utility")
            .with_inner_size([780.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Battlefield OPFOR Setup",
        options,
        Box::new(|_cc| Ok(Box::new(SetupApp::default()))),
    )
}
