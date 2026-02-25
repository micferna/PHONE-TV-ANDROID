use eframe::egui;
use std::collections::VecDeque;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct TransferState {
    active: bool,
    filename: String,
    total_bytes: u64,
    transferred_bytes: u64,
    done: bool,
    play_after: bool,
}

#[allow(dead_code)]
enum BgEvent {
    DevicesLoaded(Vec<Device>),
    NetworkScanDone(Vec<String>),
    WifiConnected { addr: String, success: bool },
    WebcamSwitched(Option<Child>),
    Log(String),
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([550.0, 750.0])
            .with_title("Phone-TV Controller"),
        ..Default::default()
    };

    eframe::run_native(
        "Phone-TV",
        options,
        Box::new(|_cc| Ok(Box::new(PhoneTvApp::new()))),
    )
}

#[derive(Clone, PartialEq)]
enum DeviceType {
    Phone,
    Tv,
    Unknown,
}

#[derive(Clone)]
struct Device {
    id: String,
    name: String,
    status: String,
    device_type: DeviceType,
}

struct PhoneTvApp {
    devices: Vec<Device>,
    selected_device: Option<usize>,
    // Phone options
    cam_front: bool,
    with_mic: bool,
    audio_output: bool,
    stay_awake: bool,
    webcam_active: bool,
    mirror_active: bool,
    // Process tracking
    webcam_child: Option<Child>,
    mirror_child: Option<Child>,
    // Transfer
    video_url: String,
    file_path: String,
    transfer: Arc<Mutex<TransferState>>,
    // Network scan
    network_devices: Vec<String>,
    scanning: bool,
    manual_ip: String,
    // Logs
    logs: VecDeque<String>,
    // Background events
    bg_rx: mpsc::Receiver<BgEvent>,
    bg_tx: mpsc::Sender<BgEvent>,
    // Async state flags
    refreshing: bool,
    connecting: bool,
    switching_cam: bool,
}

impl PhoneTvApp {
    fn new() -> Self {
        let devices = get_all_devices();
        let selected = if devices.is_empty() { None } else { Some(0) };
        let (bg_tx, bg_rx) = mpsc::channel();
        Self {
            devices,
            selected_device: selected,
            cam_front: true,
            with_mic: false,
            audio_output: false,
            stay_awake: true,
            webcam_active: false,
            mirror_active: false,
            webcam_child: None,
            mirror_child: None,
            video_url: String::new(),
            file_path: String::new(),
            transfer: Arc::new(Mutex::new(TransferState::default())),
            network_devices: Vec::new(),
            scanning: false,
            manual_ip: String::new(),
            logs: VecDeque::from(["Bienvenue! Connectez vos appareils Android.".to_string()]),
            bg_rx,
            bg_tx,
            refreshing: false,
            connecting: false,
            switching_cam: false,
        }
    }

    fn log(&mut self, msg: &str) {
        self.logs.push_back(msg.to_string());
        if self.logs.len() > 8 {
            self.logs.pop_front();
        }
    }

    fn get_selected(&self) -> Option<&Device> {
        self.selected_device.and_then(|i| self.devices.get(i))
    }

    fn get_selected_id(&self) -> Option<String> {
        self.get_selected().map(|d| d.id.clone())
    }

    fn is_tv_selected(&self) -> bool {
        self.get_selected().map(|d| d.device_type == DeviceType::Tv).unwrap_or(false)
    }

    fn refresh_async(&mut self, ctx: &egui::Context) {
        if self.refreshing { return; }
        self.refreshing = true;
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let devices = get_all_devices();
            let _ = tx.send(BgEvent::DevicesLoaded(devices));
            ctx.request_repaint();
        });
    }

    fn scan_network_async(&mut self, ctx: &egui::Context) {
        if self.scanning { return; }
        self.scanning = true;
        self.network_devices.clear();
        self.log("Scan réseau en cours...");
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let found = scan_network_for_adb();
            let _ = tx.send(BgEvent::NetworkScanDone(found));
            ctx.request_repaint();
        });
    }

    fn connect_wifi_async(&mut self, addr: String, ctx: &egui::Context) {
        if self.connecting { return; }
        self.connecting = true;
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let success = connect_adb_wifi(&addr);
            let _ = tx.send(BgEvent::WifiConnected { addr, success });
            ctx.request_repaint();
        });
    }

    fn switch_camera_async(&mut self, id: String, ctx: &egui::Context) {
        if self.switching_cam { return; }
        self.switching_cam = true;
        self.kill_webcam();
        let front = self.cam_front;
        let with_mic = self.with_mic;
        let audio_output = self.audio_output;
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let child = start_webcam_process(&id, front, with_mic, audio_output);
            let _ = tx.send(BgEvent::WebcamSwitched(child));
            ctx.request_repaint();
        });
    }

    fn process_bg_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.bg_rx.try_recv() {
            match event {
                BgEvent::DevicesLoaded(devices) => {
                    self.devices = devices;
                    if self.devices.is_empty() {
                        self.selected_device = None;
                    } else if self.selected_device.is_none() {
                        self.selected_device = Some(0);
                    }
                    let phones = self.devices.iter().filter(|d| d.device_type == DeviceType::Phone).count();
                    let tvs = self.devices.iter().filter(|d| d.device_type == DeviceType::Tv).count();
                    self.log(&format!("{} phone(s), {} TV(s)", phones, tvs));
                    self.refreshing = false;
                }
                BgEvent::NetworkScanDone(found) => {
                    self.log(&format!("{} appareil(s) trouvé(s)", found.len()));
                    self.network_devices = found;
                    self.scanning = false;
                }
                BgEvent::WifiConnected { addr, success } => {
                    if success {
                        self.log(&format!("Connecté à {}", addr));
                        self.refresh_async(ctx);
                    } else {
                        self.log(&format!("Échec connexion {}", addr));
                    }
                    self.connecting = false;
                }
                BgEvent::WebcamSwitched(child) => {
                    if !self.switching_cam {
                        // Cancelled (stop_all was pressed) — kill the new process
                        if let Some(mut c) = child {
                            kill_child_tree(&mut c);
                        }
                    } else {
                        self.webcam_child = child;
                        self.webcam_active = self.webcam_child.is_some();
                        self.switching_cam = false;
                        self.log(&format!("Switch → {}", if self.cam_front { "FRONT" } else { "BACK" }));
                    }
                }
                BgEvent::Log(msg) => {
                    self.log(&msg);
                }
            }
        }
    }

    fn kill_webcam(&mut self) {
        if let Some(mut child) = self.webcam_child.take() {
            kill_child_tree(&mut child);
        }
    }

    fn kill_mirror(&mut self) {
        if let Some(mut child) = self.mirror_child.take() {
            kill_child_tree(&mut child);
        }
    }

    fn stop_all(&mut self) {
        self.switching_cam = false;
        self.kill_webcam();
        self.kill_mirror();
        self.webcam_active = false;
        self.mirror_active = false;
        // Cleanup scrcpy-server on device
        let _ = Command::new("adb").args(["shell", "pkill", "-f", "scrcpy"]).spawn();
    }
}

impl eframe::App for PhoneTvApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process background events first
        self.process_bg_events(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("📱📺 Phone-TV Controller");
            ui.separator();

            // ===================== DEVICE SELECTION =====================
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Appareils connectés:").strong());
                    let refresh_text = if self.refreshing { "⏳ Actualiser..." } else { "🔄 Actualiser" };
                    if ui.add_enabled(!self.refreshing, egui::Button::new(refresh_text)).clicked() {
                        self.refresh_async(ctx);
                    }
                    let scan_text = if self.scanning { "⏳ Scan..." } else { "🔍 Scanner Réseau" };
                    if ui.add_enabled(!self.scanning, egui::Button::new(scan_text)).clicked() {
                        self.scan_network_async(ctx);
                    }
                });

                // Network scan results
                if !self.network_devices.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("📡 Appareils réseau détectés:").color(egui::Color32::LIGHT_BLUE));
                    let mut to_connect: Option<String> = None;
                    for ip in &self.network_devices {
                        ui.horizontal(|ui| {
                            ui.label(format!("  {} (port 5555)", ip));
                            let clicked = ui.add_enabled(!self.connecting, egui::Button::new("Connecter")).clicked();
                            if clicked {
                                to_connect = Some(ip.clone());
                            }
                        });
                    }
                    if let Some(ip) = to_connect {
                        let addr = format!("{}:5555", ip);
                        self.connect_wifi_async(addr, ctx);
                    }
                }

                // Manual IP connection
                ui.horizontal(|ui| {
                    ui.label("IP manuelle:");
                    ui.add(egui::TextEdit::singleline(&mut self.manual_ip)
                        .hint_text("192.168.1.x")
                        .desired_width(120.0));
                    let can_connect = !self.manual_ip.is_empty() && !self.connecting;
                    if ui.add_enabled(can_connect, egui::Button::new("➕ Connecter")).clicked() {
                        let addr = if self.manual_ip.contains(':') {
                            self.manual_ip.clone()
                        } else {
                            format!("{}:5555", self.manual_ip)
                        };
                        self.manual_ip.clear();
                        self.connect_wifi_async(addr, ctx);
                    }
                });

                ui.separator();

                if self.devices.is_empty() {
                    ui.label(egui::RichText::new("⚠ Aucun appareil détecté").color(egui::Color32::YELLOW));
                    ui.label("• Connectez un téléphone/TV en USB");
                    ui.label("• Ou scannez le réseau pour trouver les TV");
                } else {
                    let mut new_selection: Option<(usize, String)> = None;

                    for (i, device) in self.devices.iter().enumerate() {
                        let is_selected = self.selected_device == Some(i);
                        let (icon, type_str) = match device.device_type {
                            DeviceType::Phone => ("📱", "Phone"),
                            DeviceType::Tv => ("📺", "TV"),
                            DeviceType::Unknown => ("❓", "?"),
                        };
                        let status_color = if device.status == "device" {
                            egui::Color32::GREEN
                        } else {
                            egui::Color32::YELLOW
                        };

                        ui.horizontal(|ui| {
                            let btn_text = format!("{} {} {} [{}]",
                                if is_selected { "▶" } else { "○" },
                                icon, device.name, type_str);

                            let btn = if is_selected {
                                egui::Button::new(egui::RichText::new(btn_text).strong())
                                    .fill(egui::Color32::from_rgb(40, 40, 80))
                            } else {
                                egui::Button::new(btn_text)
                            };

                            if ui.add(btn).clicked() {
                                new_selection = Some((i, device.name.clone()));
                            }
                            ui.label(egui::RichText::new("●").color(status_color));
                        });
                    }

                    if let Some((idx, name)) = new_selection {
                        self.selected_device = Some(idx);
                        self.log(&format!("→ {}", name));
                    }
                }
            });

            ui.separator();

            // Get selected device type
            let is_tv = self.is_tv_selected();
            let has_device = self.get_selected_id().is_some();

            // ===================== TV REMOTE CONTROL =====================
            if is_tv {
                ui.group(|ui| {
                    ui.heading("📺 Télécommande TV");

                    if let Some(ref id) = self.get_selected_id() {
                        // D-Pad Navigation
                        ui.horizontal(|ui| {
                            ui.add_space(50.0);
                            if ui.add_sized([60.0, 40.0], egui::Button::new("▲")).clicked() {
                                press_key(id, "KEYCODE_DPAD_UP");
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.add_sized([60.0, 40.0], egui::Button::new("◀")).clicked() {
                                press_key(id, "KEYCODE_DPAD_LEFT");
                            }
                            if ui.add_sized([60.0, 40.0], egui::Button::new("OK").fill(egui::Color32::DARK_GREEN)).clicked() {
                                press_key(id, "KEYCODE_DPAD_CENTER");
                            }
                            if ui.add_sized([60.0, 40.0], egui::Button::new("▶")).clicked() {
                                press_key(id, "KEYCODE_DPAD_RIGHT");
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.add_space(50.0);
                            if ui.add_sized([60.0, 40.0], egui::Button::new("▼")).clicked() {
                                press_key(id, "KEYCODE_DPAD_DOWN");
                            }
                        });

                        ui.add_space(5.0);

                        // Control buttons
                        ui.horizontal(|ui| {
                            if ui.button("🏠 Home").clicked() { press_key(id, "KEYCODE_HOME"); }
                            if ui.button("⬅ Back").clicked() { press_key(id, "KEYCODE_BACK"); }
                            if ui.button("☰ Menu").clicked() { press_key(id, "KEYCODE_MENU"); }
                        });

                        ui.horizontal(|ui| {
                            if ui.button("⏮").clicked() { press_key(id, "KEYCODE_MEDIA_PREVIOUS"); }
                            if ui.button("⏪").clicked() { press_key(id, "KEYCODE_MEDIA_REWIND"); }
                            if ui.button("⏯ Play").clicked() { press_key(id, "KEYCODE_MEDIA_PLAY_PAUSE"); }
                            if ui.button("⏩").clicked() { press_key(id, "KEYCODE_MEDIA_FAST_FORWARD"); }
                            if ui.button("⏭").clicked() { press_key(id, "KEYCODE_MEDIA_NEXT"); }
                        });

                        ui.horizontal(|ui| {
                            if ui.button("🔊 Vol+").clicked() { press_key(id, "KEYCODE_VOLUME_UP"); }
                            if ui.button("🔉 Vol-").clicked() { press_key(id, "KEYCODE_VOLUME_DOWN"); }
                            if ui.button("🔇 Mute").clicked() { press_key(id, "KEYCODE_VOLUME_MUTE"); }
                            if ui.button("⏻ Power").clicked() { press_key(id, "KEYCODE_POWER"); }
                        });

                        // Apps
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            if ui.button("▶ YouTube").clicked() { open_youtube_tv(id); self.log("YouTube TV"); }
                            if ui.button("🎬 Netflix").clicked() { open_netflix(id); self.log("Netflix"); }
                            if ui.button("📺 Plex").clicked() { open_plex(id); self.log("Plex"); }
                            if ui.button("🎵 Spotify").clicked() { open_spotify(id); self.log("Spotify"); }
                        });
                    }
                });

                ui.add_space(5.0);
            }

            // ===================== PHONE CONTROLS =====================
            if !is_tv && has_device {
                // Stay Awake
                ui.horizontal(|ui| {
                    let prev = self.stay_awake;
                    ui.checkbox(&mut self.stay_awake, "☀ Stay Awake (empêche la veille)");
                    if prev != self.stay_awake {
                        if let Some(ref id) = self.get_selected_id() {
                            set_stay_awake_cmd(id, self.stay_awake);
                            self.log(if self.stay_awake { "Stay Awake ON" } else { "Stay Awake OFF" });
                        }
                    }
                });

                ui.separator();

                // Webcam Section
                ui.group(|ui| {
                    ui.heading("📷 Webcam Discord/OBS");

                    ui.horizontal(|ui| {
                        ui.label("Caméra:");
                        let switch_disabled = self.switching_cam;
                        if ui.add_enabled(!switch_disabled, egui::SelectableLabel::new(!self.cam_front, "⬛ BACK")).clicked() {
                            let was_front = self.cam_front;
                            self.cam_front = false;
                            if was_front && self.webcam_active {
                                if let Some(id) = self.get_selected_id() {
                                    self.switch_camera_async(id, ctx);
                                }
                            }
                        }
                        if ui.add_enabled(!switch_disabled, egui::SelectableLabel::new(self.cam_front, "🤳 FRONT")).clicked() {
                            let was_back = !self.cam_front;
                            self.cam_front = true;
                            if was_back && self.webcam_active {
                                if let Some(id) = self.get_selected_id() {
                                    self.switch_camera_async(id, ctx);
                                }
                            }
                        }
                        ui.separator();
                        ui.checkbox(&mut self.with_mic, "🎤 Micro");
                        ui.checkbox(&mut self.audio_output, "🔊 Audio Sortie");
                    });

                    if self.switching_cam {
                        ui.label(egui::RichText::new("⏳ Switch caméra...").color(egui::Color32::YELLOW));
                    }

                    ui.horizontal(|ui| {
                        let webcam_btn = if self.webcam_active {
                            egui::Button::new("⏹ Stop Webcam").fill(egui::Color32::DARK_RED)
                        } else {
                            egui::Button::new("▶ Démarrer Webcam").fill(egui::Color32::DARK_GREEN)
                        };

                        if ui.add_sized([150.0, 35.0], webcam_btn).clicked() {
                            if self.webcam_active {
                                self.kill_webcam();
                                self.webcam_active = false;
                                self.log("Webcam stoppée");
                            } else if let Some(ref id) = self.get_selected_id() {
                                let child = start_webcam_process(id, self.cam_front, self.with_mic, self.audio_output);
                                if child.is_some() {
                                    self.webcam_child = child;
                                    self.webcam_active = true;
                                    self.log(&format!("Webcam {} ON", if self.cam_front { "FRONT" } else { "BACK" }));
                                }
                            }
                        }

                        if self.webcam_active {
                            ui.label(egui::RichText::new("● LIVE").color(egui::Color32::RED).strong());
                        }
                    });

                    if !Path::new("/dev/video10").exists() {
                        ui.label(egui::RichText::new("⚠ v4l2loopback non configuré").color(egui::Color32::YELLOW).small());
                    }
                });

                ui.add_space(5.0);

                // Mirroring
                ui.group(|ui| {
                    ui.heading("🖥 Mirroring Écran");

                    ui.horizontal(|ui| {
                        let mirror_btn = if self.mirror_active {
                            egui::Button::new("⏹ Stop").fill(egui::Color32::DARK_RED)
                        } else {
                            egui::Button::new("▶ Démarrer").fill(egui::Color32::DARK_BLUE)
                        };

                        if ui.add_sized([120.0, 35.0], mirror_btn).clicked() {
                            if self.mirror_active {
                                self.kill_mirror();
                                self.mirror_active = false;
                                self.log("Mirroring stoppé");
                            } else if let Some(ref id) = self.get_selected_id() {
                                let child = start_mirror_process(id, self.stay_awake);
                                if child.is_some() {
                                    self.mirror_child = child;
                                    self.mirror_active = true;
                                    self.log("Mirroring actif");
                                }
                            }
                        }

                        if self.mirror_active {
                            ui.label(egui::RichText::new("● ACTIF").color(egui::Color32::GREEN).strong());
                        }
                    });
                });

                ui.add_space(5.0);

                // Quick phone actions
                if let Some(ref id) = self.get_selected_id() {
                    ui.horizontal(|ui| {
                        if ui.button("📸 Photo").clicked() { open_camera(id); }
                        if ui.button("🎥 Vidéo").clicked() { open_video(id); }
                        if ui.button("🎙 Micro").clicked() { open_mic(id); }
                        if ui.button("🏠").clicked() { press_key(id, "KEYCODE_HOME"); }
                        if ui.button("⬅").clicked() { press_key(id, "KEYCODE_BACK"); }
                    });
                }
            }

            ui.separator();

            // ===================== VIDEO TRANSFER / STREAM =====================
            if has_device {
                ui.group(|ui| {
                    ui.heading("🎬 Vidéo / Transfert");

                    // Stream URL
                    ui.horizontal(|ui| {
                        ui.label("URL:");
                        ui.add(egui::TextEdit::singleline(&mut self.video_url)
                            .hint_text("https://... ou chemin local")
                            .desired_width(250.0));

                        if ui.button("▶ Lire").clicked() && !self.video_url.is_empty() {
                            if let Some(ref id) = self.get_selected_id() {
                                play_video_url(id, &self.video_url);
                                self.log(&format!("Lecture: {}", &self.video_url[..self.video_url.len().min(30)]));
                            }
                        }
                    });

                    // File transfer
                    ui.horizontal(|ui| {
                        ui.label("Fichier:");
                        ui.add(egui::TextEdit::singleline(&mut self.file_path)
                            .hint_text("/chemin/vers/video.mp4")
                            .desired_width(200.0));

                        if ui.button("📂").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Vidéos", &["mp4", "mkv", "avi", "mov", "webm"])
                                .add_filter("Tous", &["*"])
                                .pick_file()
                            {
                                self.file_path = path.display().to_string();
                            }
                        }
                    });

                    // Check transfer state
                    let transfer_state = self.transfer.lock().unwrap().clone();

                    if transfer_state.active {
                        // Show progress bar
                        let progress = if transfer_state.total_bytes > 0 {
                            transfer_state.transferred_bytes as f32 / transfer_state.total_bytes as f32
                        } else {
                            0.0
                        };

                        ui.horizontal(|ui| {
                            ui.label(format!("📤 {}", transfer_state.filename));
                        });

                        ui.add(egui::ProgressBar::new(progress)
                            .text(format!("{:.0}% - {:.1} MB / {:.1} MB",
                                progress * 100.0,
                                transfer_state.transferred_bytes as f64 / 1_000_000.0,
                                transfer_state.total_bytes as f64 / 1_000_000.0))
                            .animate(true));

                        if transfer_state.done {
                            ui.label(egui::RichText::new("✓ Terminé!").color(egui::Color32::GREEN));
                            // Reset after showing done
                            if let Ok(mut t) = self.transfer.lock() {
                                t.active = false;
                                t.done = false;
                            }
                        }

                        // Request repaint to update progress
                        ctx.request_repaint();
                    } else {
                        ui.horizontal(|ui| {
                            let file_ok = !self.file_path.is_empty() && Path::new(&self.file_path).exists();

                            ui.add_enabled_ui(file_ok, |ui| {
                                if ui.button("📤 Envoyer").clicked() {
                                    if let Some(ref id) = self.get_selected_id() {
                                        self.log("Transfert...");
                                        let path = self.file_path.clone();
                                        start_transfer(id, &path, Arc::clone(&self.transfer), false);
                                    }
                                }

                                if ui.button("▶ Envoyer+Lire").clicked() {
                                    if let Some(ref id) = self.get_selected_id() {
                                        self.log("Envoi + lecture...");
                                        let path = self.file_path.clone();
                                        start_transfer(id, &path, Arc::clone(&self.transfer), true);
                                    }
                                }
                            });

                            if !file_ok && !self.file_path.is_empty() {
                                ui.label(egui::RichText::new("⚠ Fichier introuvable").color(egui::Color32::RED).small());
                            }
                        });
                    }
                });
            }

            ui.separator();

            // ===================== STOP ALL =====================
            if ui.add_sized([ui.available_width(), 30.0],
                egui::Button::new("🛑 STOP TOUT").fill(egui::Color32::from_rgb(139, 0, 0))
            ).clicked() {
                self.stop_all();
                self.log("Tout stoppé");
            }

            // ===================== LOGS =====================
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Logs:");
                if ui.small_button("Clear").clicked() { self.logs.clear(); }
            });
            egui::ScrollArea::vertical().max_height(50.0).show(ui, |ui| {
                for log in &self.logs {
                    ui.label(egui::RichText::new(log).small());
                }
            });
        });
    }
}

// ============================================================================
// Process management
// ============================================================================

fn kill_child_tree(child: &mut Child) {
    let pid = child.id();
    // Kill child processes first
    let _ = Command::new("pkill").args(["-P", &pid.to_string()]).output();
    let _ = child.kill();
    let _ = child.wait();
}

fn start_webcam_process(id: &str, front: bool, with_mic: bool, audio_output: bool) -> Option<Child> {
    let facing_arg = format!("--camera-facing={}", if front { "front" } else { "back" });
    let mut args = vec![
        "run".to_string(), "--command=scrcpy".to_string(),
        "io.github.IshuSinghSE.aurynk".to_string(),
        "-s".to_string(), id.to_string(),
        "--video-source=camera".to_string(), facing_arg,
        "--camera-size=1280x720".to_string(),
        "--v4l2-sink=/dev/video10".to_string(),
    ];

    // Audio:
    // - with_mic: envoie le micro du téléphone vers le PC (pour que les autres t'entendent sur Lovo/Azar)
    // - audio_output: envoie le son des apps du téléphone vers le PC
    if with_mic && audio_output {
        // Les deux: micro prioritaire, on ne peut pas avoir les deux sources en même temps dans scrcpy
        args.push("--audio-source=mic".to_string());
    } else if with_mic {
        args.push("--audio-source=mic".to_string());
    } else if audio_output {
        args.push("--audio-source=playback".to_string());
        args.push("--audio-dup".to_string());
    } else {
        args.push("--no-audio".to_string());
    }
    Command::new("flatpak").args(&args).spawn().ok()
}

fn start_mirror_process(id: &str, stay_awake: bool) -> Option<Child> {
    let mut args = vec![
        "run".to_string(), "--command=scrcpy".to_string(),
        "io.github.IshuSinghSE.aurynk".to_string(),
        "-s".to_string(), id.to_string(), "--no-audio".to_string(),
    ];
    if stay_awake { args.push("--stay-awake".to_string()); }
    Command::new("flatpak").args(&args).spawn().ok()
}

// ============================================================================
// ADB Functions
// ============================================================================

fn adb(args: &[&str]) -> Option<String> {
    Command::new("adb").args(args).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

fn adb_device(id: &str, args: &[&str]) -> Option<String> {
    let mut full_args = vec!["-s", id];
    full_args.extend(args);
    adb(&full_args)
}

fn adb_fire(id: &str, args: &[&str]) {
    let mut full_args = vec!["-s", id];
    full_args.extend(args);
    let _ = Command::new("adb").args(&full_args).spawn();
}

fn get_all_devices() -> Vec<Device> {
    let mut devices = Vec::new();

    if let Some(output) = adb(&["devices", "-l"]) {
        for line in output.lines().skip(1) {
            if line.trim().is_empty() { continue; }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let id = parts[0].to_string();
                let status = parts[1].to_string();

                let name = if status == "device" {
                    adb_device(&id, &["shell", "getprop", "ro.product.model"])
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| id.clone())
                } else {
                    parts.iter().find(|p| p.starts_with("model:"))
                        .map(|p| p.replace("model:", ""))
                        .unwrap_or_else(|| id.clone())
                };

                // Detect TV
                let device_type = if status == "device" {
                    let features = adb_device(&id, &["shell", "getprop", "ro.build.characteristics"])
                        .unwrap_or_default().to_lowercase();
                    let product = adb_device(&id, &["shell", "getprop", "ro.product.name"])
                        .unwrap_or_default().to_lowercase();

                    if features.contains("tv") || product.contains("tv") ||
                       name.to_lowercase().contains("tv") || name.to_lowercase().contains("shield") ||
                       name.to_lowercase().contains("chromecast") || name.to_lowercase().contains("mibox") {
                        DeviceType::Tv
                    } else {
                        DeviceType::Phone
                    }
                } else {
                    DeviceType::Unknown
                };

                devices.push(Device { id, name, status, device_type });
            }
        }
    }
    devices
}

fn set_stay_awake_cmd(id: &str, enabled: bool) {
    let value = if enabled { "true" } else { "false" };
    adb_fire(id, &["shell", "svc", "power", "stayon", value]);
}

fn press_key(id: &str, key: &str) {
    adb_fire(id, &["shell", "input", "keyevent", key]);
}

fn open_camera(id: &str) {
    adb_fire(id, &["shell", "am", "start", "-a", "android.media.action.IMAGE_CAPTURE"]);
}

fn open_video(id: &str) {
    adb_fire(id, &["shell", "am", "start", "-a", "android.media.action.VIDEO_CAPTURE"]);
}

fn open_mic(id: &str) {
    adb_fire(id, &["shell", "am", "start", "-a", "android.provider.MediaStore.RECORD_SOUND"]);
}

fn open_youtube_tv(id: &str) {
    adb_fire(id, &["shell", "am", "start", "-n",
        "com.google.android.youtube.tv/com.google.android.apps.youtube.tv.activity.ShellActivity"]);
}

fn open_netflix(id: &str) {
    adb_fire(id, &["shell", "am", "start", "-n", "com.netflix.ninja/.MainActivity"]);
}

fn open_plex(id: &str) {
    adb_fire(id, &["shell", "am", "start", "-n", "com.plexapp.android/.activity.SplashActivity"]);
}

fn open_spotify(id: &str) {
    adb_fire(id, &["shell", "am", "start", "-n", "com.spotify.tv.android/.SpotifyTVActivity"]);
}

fn play_video_url(id: &str, url: &str) {
    adb_fire(id, &["shell", "am", "start", "-a", "android.intent.action.VIEW", "-d", url, "-t", "video/*"]);
}

fn start_transfer(id: &str, local_path: &str, state: Arc<Mutex<TransferState>>, play_after: bool) {
    let path = Path::new(local_path);
    let filename = match path.file_name() {
        Some(n) => n.to_string_lossy().to_string(),
        None => return,
    };

    // Get file size
    let total_bytes = std::fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);

    let remote = format!("/sdcard/Movies/{}", filename);
    let id = id.to_string();
    let local = local_path.to_string();

    // Initialize state
    if let Ok(mut t) = state.lock() {
        t.active = true;
        t.filename = filename;
        t.total_bytes = total_bytes;
        t.transferred_bytes = 0;
        t.done = false;
        t.play_after = play_after;
    }

    // Monitor thread (uses clones)
    let monitor_state = Arc::clone(&state);
    let monitor_id = id.clone();
    let monitor_remote = remote.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Check remote file size
            if let Some(output) = adb_device(&monitor_id, &["shell", "stat", "-c", "%s", &monitor_remote]) {
                if let Ok(size) = output.trim().parse::<u64>() {
                    if let Ok(mut t) = monitor_state.lock() {
                        t.transferred_bytes = size;
                        if t.done || !t.active {
                            break;
                        }
                    }
                }
            }

            // Check if still active
            if let Ok(t) = monitor_state.lock() {
                if t.done || !t.active {
                    break;
                }
            }
        }
    });

    // Transfer thread (takes ownership of id, remote, state)
    std::thread::spawn(move || {
        let output = Command::new("adb")
            .args(["-s", &id, "push", &local, &remote])
            .output();

        let success = output.map(|o| o.status.success()).unwrap_or(false);

        if let Ok(mut t) = state.lock() {
            t.transferred_bytes = t.total_bytes;
            t.done = true;

            if success && t.play_after {
                // Play video on device (fire-and-forget)
                let _ = Command::new("adb")
                    .args(["-s", &id, "shell", "am", "start", "-a", "android.intent.action.VIEW",
                           "-d", &format!("file://{}", remote), "-t", "video/*"])
                    .spawn();
            }
        }
    });
}

fn connect_adb_wifi(addr: &str) -> bool {
    Command::new("adb")
        .args(["connect", addr])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout).to_lowercase();
            out.contains("connected") && !out.contains("cannot") && !out.contains("failed")
        })
        .unwrap_or(false)
}

fn get_local_ip_prefix() -> Option<String> {
    // Get local IP from `ip route` or `hostname -I`
    if let Ok(output) = Command::new("hostname").arg("-I").output() {
        let ips = String::from_utf8_lossy(&output.stdout);
        for ip in ips.split_whitespace() {
            if ip.starts_with("192.168.") || ip.starts_with("10.") || ip.starts_with("172.") {
                let parts: Vec<&str> = ip.split('.').collect();
                if parts.len() >= 3 {
                    return Some(format!("{}.{}.{}.", parts[0], parts[1], parts[2]));
                }
            }
        }
    }
    None
}

fn scan_network_for_adb() -> Vec<String> {
    let mut found = Vec::new();

    if let Some(prefix) = get_local_ip_prefix() {
        // Fast parallel scan using bash with timeout
        // Scan common IP range (1-254) for port 5555
        let script = format!(
            r#"for i in $(seq 1 254); do
                (timeout 0.3 bash -c "echo >/dev/tcp/{prefix}$i/5555" 2>/dev/null && echo "{prefix}$i") &
            done; wait"#,
            prefix = prefix
        );

        if let Ok(output) = Command::new("bash").args(["-c", &script]).output() {
            let result = String::from_utf8_lossy(&output.stdout);
            for line in result.lines() {
                let ip = line.trim();
                if !ip.is_empty() && ip.starts_with(&prefix[..prefix.len()-1]) {
                    found.push(ip.to_string());
                }
            }
        }
    }

    found
}
