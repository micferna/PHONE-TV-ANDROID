use eframe::egui;
use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
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
    StorageInfo { device_id: String, total: String, used: String, avail: String, percent: f32 },
    Log(String),
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 800.0])
            .with_title("Phone-TV Controller"),
        ..Default::default()
    };

    eframe::run_native(
        "Phone-TV",
        options,
        Box::new(|cc| {
            let ctx = &cc.egui_ctx;
            let mut style = (*ctx.style()).clone();
            style.visuals.window_fill = egui::Color32::from_rgb(25, 25, 30);
            style.visuals.panel_fill = egui::Color32::from_rgb(25, 25, 30);
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 6.0);
            style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(4);
            style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
            style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
            style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
            ctx.set_style(style);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(PhoneTvApp::new()))
        }),
    )
}

#[derive(Clone, PartialEq)]
enum DeviceType {
    Phone,
    Tv,
    Unknown,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Devices,
    Tv,
    Phone,
    Video,
}

#[derive(Clone)]
struct Device {
    id: String,
    name: String,
    status: String,
    device_type: DeviceType,
}

#[derive(Clone)]
struct TvChannel {
    name: String,
    number: u32,
}

struct TvShell {
    device_id: String,
    child: Child,
    stdin: ChildStdin,
}

struct PhoneTvApp {
    devices: Vec<Device>,
    selected_device: Option<usize>,
    // UI tabs
    active_tab: Tab,
    logs_collapsed: bool,
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
    // Persistent ADB shell for TV (low-latency remote)
    tv_shell: Option<TvShell>,
    // TV storage
    tv_storage: Option<(String, String, String, f32)>,
    tv_storage_device: String,
    // TV channels
    tv_channels: Vec<TvChannel>,
    channel_edit_mode: bool,
    new_channel_name: String,
    new_channel_number: String,
    // Replay
    replay_custom_min: String,
    replay_ratio: f32, // minutes de programme par seconde de maintien
}

fn channels_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("phone-tv");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("channels.txt")
}

fn default_channels() -> Vec<TvChannel> {
    vec![
        TvChannel { name: "TF1".into(), number: 1 },
        TvChannel { name: "France 2".into(), number: 2 },
        TvChannel { name: "France 3".into(), number: 3 },
        TvChannel { name: "France 4".into(), number: 4 },
        TvChannel { name: "France 5".into(), number: 5 },
        TvChannel { name: "M6".into(), number: 6 },
        TvChannel { name: "Arte".into(), number: 7 },
        TvChannel { name: "LCP".into(), number: 8 },
        TvChannel { name: "W9".into(), number: 9 },
        TvChannel { name: "TMC".into(), number: 10 },
        TvChannel { name: "TFX".into(), number: 11 },
        TvChannel { name: "Gulli".into(), number: 12 },
        TvChannel { name: "BFMTV".into(), number: 13 },
        TvChannel { name: "CNEWS".into(), number: 14 },
        TvChannel { name: "LCI".into(), number: 15 },
        TvChannel { name: "FranceInfo".into(), number: 16 },
        TvChannel { name: "CSTAR".into(), number: 17 },
        TvChannel { name: "CMI TV".into(), number: 18 },
        TvChannel { name: "TF1 SF".into(), number: 20 },
        TvChannel { name: "L'Équipe".into(), number: 21 },
        TvChannel { name: "6ter".into(), number: 22 },
        TvChannel { name: "RMC Story".into(), number: 23 },
        TvChannel { name: "RMC Déc".into(), number: 24 },
        TvChannel { name: "Chérie 25".into(), number: 25 },
    ]
}

fn load_channels() -> Vec<TvChannel> {
    let path = channels_path();
    if let Ok(file) = std::fs::File::open(&path) {
        let reader = std::io::BufReader::new(file);
        let mut channels = Vec::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                let line = line.trim().to_string();
                if let Some((num_str, name)) = line.split_once(':') {
                    if let Ok(number) = num_str.parse::<u32>() {
                        channels.push(TvChannel { name: name.to_string(), number });
                    }
                }
            }
        }
        if !channels.is_empty() {
            return channels;
        }
    }
    // First run or empty file — use defaults and save them
    let channels = default_channels();
    save_channels(&channels);
    channels
}

fn save_channels(channels: &[TvChannel]) {
    let path = channels_path();
    if let Ok(mut file) = std::fs::File::create(&path) {
        for ch in channels {
            let _ = writeln!(file, "{}:{}", ch.number, ch.name);
        }
    }
}

fn replay_ratio_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("phone-tv");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("replay_ratio.txt")
}

fn load_replay_ratio() -> f32 {
    std::fs::read_to_string(replay_ratio_path())
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(12.0)
}

fn save_replay_ratio(ratio: f32) {
    let _ = std::fs::write(replay_ratio_path(), format!("{:.1}", ratio));
}


impl PhoneTvApp {
    fn new() -> Self {
        let devices = get_all_devices();
        let selected = if devices.is_empty() { None } else { Some(0) };
        let (bg_tx, bg_rx) = mpsc::channel();
        Self {
            devices,
            selected_device: selected,
            active_tab: Tab::Devices,
            logs_collapsed: false,
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
            tv_shell: None,
            tv_storage: None,
            tv_storage_device: String::new(),
            tv_channels: load_channels(),
            channel_edit_mode: false,
            new_channel_name: String::new(),
            new_channel_number: String::new(),
            replay_custom_min: String::new(),
            replay_ratio: load_replay_ratio(),
        }
    }

    fn log(&mut self, msg: &str) {
        self.logs.push_back(msg.to_string());
        if self.logs.len() > 15 {
            self.logs.pop_front();
        }
    }

    fn get_selected(&self) -> Option<&Device> {
        self.selected_device.and_then(|i| self.devices.get(i))
    }

    fn get_selected_id(&self) -> Option<String> {
        self.get_selected().map(|d| d.id.clone())
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
                BgEvent::StorageInfo { device_id, total, used, avail, percent } => {
                    self.tv_storage = Some((total, used, avail, percent));
                    self.tv_storage_device = device_id;
                }
                BgEvent::Log(msg) => {
                    self.log(&msg);
                }
            }
        }
    }

    fn check_children(&mut self) {
        if let Some(ref mut child) = self.webcam_child {
            if let Ok(Some(_status)) = child.try_wait() {
                self.webcam_child = None;
                if self.webcam_active {
                    self.webcam_active = false;
                    self.log("Webcam fermée");
                }
            }
        }
        if let Some(ref mut child) = self.mirror_child {
            if let Ok(Some(_status)) = child.try_wait() {
                self.mirror_child = None;
                if self.mirror_active {
                    self.mirror_active = false;
                    self.log("Mirroring fermé");
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
        self.kill_tv_shell();
        self.webcam_active = false;
        self.mirror_active = false;
        // Cleanup scrcpy-server on device
        let _ = Command::new("adb").args(["shell", "pkill", "-f", "scrcpy"]).spawn();
    }

    fn ensure_tv_shell(&mut self, device_id: &str) -> bool {
        // Check if existing shell is still alive and for the right device
        let need_new = match &mut self.tv_shell {
            Some(shell) => {
                if shell.device_id != device_id {
                    true
                } else if let Ok(Some(_)) = shell.child.try_wait() {
                    true // process died
                } else {
                    false
                }
            }
            None => true,
        };

        if need_new {
            self.kill_tv_shell();
            let mut child = match Command::new("adb")
                .args(["-s", device_id, "shell"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => return false,
            };
            let stdin = match child.stdin.take() {
                Some(s) => s,
                None => return false,
            };
            self.tv_shell = Some(TvShell {
                device_id: device_id.to_string(),
                child,
                stdin,
            });
        }
        true
    }

    fn tv_command(&mut self, device_id: &str, cmd: &str) {
        if !self.ensure_tv_shell(device_id) {
            // Fallback to fire-and-forget if shell fails
            adb_fire(device_id, &["shell", cmd]);
            return;
        }
        if let Some(ref mut shell) = self.tv_shell {
            let full_cmd = format!("{}\n", cmd);
            if shell.stdin.write_all(full_cmd.as_bytes()).is_err()
                || shell.stdin.flush().is_err()
            {
                // Shell broken, kill and fallback
                self.tv_shell = None;
                adb_fire(device_id, &["shell", cmd]);
            }
        }
    }

    fn kill_tv_shell(&mut self) {
        if let Some(mut shell) = self.tv_shell.take() {
            let _ = shell.child.kill();
            let _ = shell.child.wait();
        }
    }

    fn send_channel_number(&mut self, device_id: &str, number: u32) {
        let id = device_id.to_string();
        let bg_tx = self.bg_tx.clone();
        std::thread::spawn(move || {
            // Check current foreground app
            let focus_line = adb_device(&id, &["shell", "dumpsys", "window", "windows"])
                .map(|out| {
                    out.lines()
                        .find(|l| l.contains("mCurrentFocus"))
                        .unwrap_or("")
                        .trim()
                        .to_string()
                })
                .unwrap_or_default();
            let is_oqee_fg = focus_line.contains("net.oqee.androidtv");
            let _ = bg_tx.send(BgEvent::Log(format!("[1] Focus: {}", if focus_line.len() > 60 { &focus_line[focus_line.len()-60..] } else { &focus_line })));

            if !is_oqee_fg {
                let _ = bg_tx.send(BgEvent::Log("[2] HOME...".into()));
                adb_fire(&id, &["shell", "input", "keyevent", "KEYCODE_HOME"]);
                std::thread::sleep(std::time::Duration::from_millis(1000));

                // Force stop OQEE to get a clean launch
                let _ = bg_tx.send(BgEvent::Log("[3] Kill + lancement OQEE...".into()));
                adb_fire(&id, &["shell", "am", "force-stop", "net.oqee.androidtv.store"]);
                std::thread::sleep(std::time::Duration::from_millis(500));
                adb_fire(&id, &["shell", "am", "start", "-n",
                    "net.oqee.androidtv.store/net.oqee.androidtv.ui.main.RealMainActivity"]);

                // Poll: wait until OQEE is STABLE in foreground (2 consecutive checks)
                let mut wait_count = 0;
                let mut consecutive = 0;
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    wait_count += 1;
                    let ready = adb_device(&id, &["shell", "dumpsys", "window", "windows"])
                        .map(|out| {
                            out.lines()
                                .any(|l| l.contains("mCurrentFocus") && l.contains("net.oqee.androidtv"))
                        })
                        .unwrap_or(false);
                    if ready {
                        consecutive += 1;
                        if consecutive >= 2 { break; }
                    } else {
                        consecutive = 0;
                    }
                }
                let _ = bg_tx.send(BgEvent::Log(format!("[4] OQEE stable après {}x500ms", wait_count)));

                std::thread::sleep(std::time::Duration::from_millis(1500));

                // Check if we landed on live TV or menu
                let focus2 = adb_device(&id, &["shell", "dumpsys", "window", "windows"])
                    .map(|out| {
                        out.lines()
                            .find(|l| l.contains("mCurrentFocus"))
                            .unwrap_or("")
                            .trim()
                            .to_string()
                    })
                    .unwrap_or_default();
                let on_oqee = focus2.contains("net.oqee.androidtv");
                let on_live = focus2.contains("LivePlayer");
                let _ = bg_tx.send(BgEvent::Log(format!("[5] OQEE={} Live={} | {}", on_oqee, on_live, if focus2.len() > 40 { &focus2[focus2.len()-40..] } else { &focus2 })));

                if !on_oqee {
                    // OQEE didn't stay — retry
                    let _ = bg_tx.send(BgEvent::Log("[5b] Retry lancement...".into()));
                    adb_fire(&id, &["shell", "am", "start", "-n",
                        "net.oqee.androidtv.store/net.oqee.androidtv.ui.main.RealMainActivity"]);
                    std::thread::sleep(std::time::Duration::from_millis(3000));
                }

                if !on_live {
                    // Check again before pressing OK
                    let focus3 = adb_device(&id, &["shell", "dumpsys", "window", "windows"])
                        .map(|out| {
                            out.lines()
                                .find(|l| l.contains("mCurrentFocus"))
                                .unwrap_or("")
                                .trim()
                                .to_string()
                        })
                        .unwrap_or_default();
                    if focus3.contains("net.oqee.androidtv") && !focus3.contains("LivePlayer") {
                        let _ = bg_tx.send(BgEvent::Log("[6] Menu OQEE → OK...".into()));
                        adb_fire(&id, &["shell", "input", "keyevent", "KEYCODE_DPAD_CENTER"]);
                        std::thread::sleep(std::time::Duration::from_millis(2000));
                    }
                }
            } else {
                let _ = bg_tx.send(BgEvent::Log("[2] Déjà sur OQEE".into()));
            }

            let _ = bg_tx.send(BgEvent::Log(format!("[7] Envoi chiffres: {}", number)));
            std::thread::sleep(std::time::Duration::from_millis(1000));
            for digit in number.to_string().chars() {
                let cmd = format!("KEYCODE_{}", digit);
                adb_fire(&id, &["shell", "input", "keyevent", &cmd]);
                std::thread::sleep(std::time::Duration::from_millis(600));
            }
            let _ = bg_tx.send(BgEvent::Log(format!("→ Chaîne {} envoyée", number)));
        });
    }

    fn tab_enabled(&self, tab: Tab) -> bool {
        match tab {
            Tab::Devices => true,
            Tab::Tv => self.get_selected().map(|d| d.device_type == DeviceType::Tv).unwrap_or(false),
            Tab::Phone => self.get_selected().map(|d| d.device_type == DeviceType::Phone).unwrap_or(false),
            Tab::Video => self.get_selected_id().is_some(),
        }
    }
}

impl PhoneTvApp {
    // ===================== TAB: DEVICES =====================
    fn ui_tab_devices(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            let refresh_text = if self.refreshing { "⏳ Actualiser..." } else { "🔄 Actualiser" };
            if ui.add_enabled(!self.refreshing, egui::Button::new(refresh_text)).clicked() {
                self.refresh_async(ctx);
            }
            let scan_text = if self.scanning { "⏳ Scan réseau..." } else { "🔍 Scanner Réseau" };
            if ui.add_enabled(!self.scanning, egui::Button::new(scan_text)).clicked() {
                self.scan_network_async(ctx);
            }
        });

        // Network scan results
        if !self.network_devices.is_empty() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("📡 Appareils réseau détectés:").color(egui::Color32::LIGHT_BLUE));
            let mut to_connect: Option<String> = None;
            for ip in &self.network_devices {
                ui.horizontal(|ui| {
                    ui.label(format!("  {} (port 5555)", ip));
                    if ui.add_enabled(!self.connecting, egui::Button::new("Connecter")).clicked() {
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
        ui.add_space(4.0);
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

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        if self.devices.is_empty() {
            ui.label(egui::RichText::new("⚠ Aucun appareil détecté").color(egui::Color32::YELLOW));
            ui.label("• Connectez un téléphone/TV en USB");
            ui.label("• Ou scannez le réseau pour trouver les TV");
        } else {
            let mut new_selection: Option<(usize, DeviceType)> = None;

            for (i, device) in self.devices.iter().enumerate() {
                let is_selected = self.selected_device == Some(i);
                let is_connected = device.status == "device";

                let fill = if is_selected {
                    egui::Color32::from_rgb(30, 40, 80)
                } else if is_connected {
                    egui::Color32::from_rgb(25, 50, 30)
                } else {
                    egui::Color32::from_rgb(60, 55, 20)
                };

                let frame = egui::Frame::NONE
                    .corner_radius(6.0)
                    .inner_margin(8.0)
                    .fill(fill);

                frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (icon, type_str) = match device.device_type {
                            DeviceType::Phone => ("📱", "Phone"),
                            DeviceType::Tv => ("📺", "TV"),
                            DeviceType::Unknown => ("❓", "?"),
                        };

                        let btn_text = format!("{} {} {} [{}]",
                            if is_selected { "▶" } else { "○" },
                            icon, device.name, type_str);

                        let btn = egui::Button::new(
                            egui::RichText::new(btn_text).strong()
                        ).fill(egui::Color32::TRANSPARENT);

                        if ui.add(btn).clicked() {
                            new_selection = Some((i, device.device_type.clone()));
                        }

                        let status_color = if is_connected {
                            egui::Color32::GREEN
                        } else {
                            egui::Color32::YELLOW
                        };
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("●").color(status_color));
                            let status_text = if is_connected { "connecté" } else { "offline" };
                            ui.label(egui::RichText::new(status_text).small().color(status_color));
                        });
                    });
                });
                ui.add_space(2.0);
            }

            if let Some((idx, dtype)) = new_selection {
                self.selected_device = Some(idx);
                let name = self.devices[idx].name.clone();
                self.log(&format!("→ {}", name));
                // Auto-switch tab
                match dtype {
                    DeviceType::Tv => self.active_tab = Tab::Tv,
                    DeviceType::Phone => self.active_tab = Tab::Phone,
                    _ => {}
                }
            }
        }
    }

    fn refresh_tv_storage(&mut self, device_id: &str, ctx: &egui::Context) {
        let id = device_id.to_string();
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            if let Ok(output) = Command::new("adb")
                .args(["-s", &id, "shell", "df", "-h", "/sdcard"])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = text.lines().nth(1) {
                    let cols: Vec<&str> = line.split_whitespace().collect();
                    // Expected: Filesystem Size Used Avail Use% Mounted
                    if cols.len() >= 5 {
                        let total = cols[1].to_string();
                        let used = cols[2].to_string();
                        let avail = cols[3].to_string();
                        let percent = cols[4]
                            .trim_end_matches('%')
                            .parse::<f32>()
                            .unwrap_or(0.0) / 100.0;
                        let _ = tx.send(BgEvent::StorageInfo {
                            device_id: id,
                            total,
                            used,
                            avail,
                            percent,
                        });
                        ctx.request_repaint();
                    }
                }
            }
        });
    }

    // ===================== TAB: TV =====================
    fn ui_tab_tv(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let id = match self.get_selected_id() {
            Some(id) => id,
            None => return,
        };

        // Storage bar — auto-refresh if needed
        if self.tv_storage.is_none() || self.tv_storage_device != id {
            self.tv_storage = None;
            self.refresh_tv_storage(&id, ctx);
        }
        if let Some((ref total, ref used, ref avail, percent)) = self.tv_storage {
            let color = if percent < 0.7 {
                egui::Color32::from_rgb(40, 100, 50)
            } else if percent < 0.9 {
                egui::Color32::from_rgb(140, 110, 20)
            } else {
                egui::Color32::from_rgb(150, 40, 40)
            };
            let text = format!("{} / {} ({} libre)", used, total, avail);
            ui.horizontal(|ui| {
                ui.add(
                    egui::ProgressBar::new(percent)
                        .text(text)
                        .fill(color),
                );
                let id_clone = id.clone();
                if ui.small_button("\u{1f504}").clicked() {
                    self.tv_storage = None;
                    self.refresh_tv_storage(&id_clone, ctx);
                }
            });
            ui.add_space(6.0);
        }

        // D-Pad Navigation — 3x3 grid
        ui.label(egui::RichText::new("Navigation").strong());
        egui::Grid::new("dpad_grid")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                ui.label(""); // top-left empty
                if ui.add_sized([80.0, 50.0], egui::Button::new("▲")).clicked() {
                    self.tv_command(&id, "input keyevent KEYCODE_DPAD_UP");
                }
                ui.label(""); // top-right empty
                ui.end_row();

                if ui.add_sized([80.0, 50.0], egui::Button::new("◀")).clicked() {
                    self.tv_command(&id, "input keyevent KEYCODE_DPAD_LEFT");
                }
                if ui.add_sized([80.0, 50.0], egui::Button::new("OK").fill(egui::Color32::DARK_GREEN)).clicked() {
                    self.tv_command(&id, "input keyevent KEYCODE_DPAD_CENTER");
                }
                if ui.add_sized([80.0, 50.0], egui::Button::new("▶")).clicked() {
                    self.tv_command(&id, "input keyevent KEYCODE_DPAD_RIGHT");
                }
                ui.end_row();

                ui.label(""); // bottom-left empty
                if ui.add_sized([80.0, 50.0], egui::Button::new("▼")).clicked() {
                    self.tv_command(&id, "input keyevent KEYCODE_DPAD_DOWN");
                }
                ui.label(""); // bottom-right empty
                ui.end_row();
            });

        ui.add_space(8.0);

        // Navigation buttons
        ui.horizontal(|ui| {
            if ui.add_sized([100.0, 40.0], egui::Button::new("🏠 Home")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_HOME");
            }
            if ui.add_sized([100.0, 40.0], egui::Button::new("⬅ Back")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_BACK");
            }
            if ui.add_sized([100.0, 40.0], egui::Button::new("☰ Menu")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_MENU");
            }
        });

        ui.add_space(6.0);

        // Media controls
        ui.horizontal(|ui| {
            if ui.add_sized([70.0, 40.0], egui::Button::new("⏮")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_MEDIA_PREVIOUS");
            }
            if ui.add_sized([70.0, 40.0], egui::Button::new("⏪")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_MEDIA_REWIND");
            }
            if ui.add_sized([70.0, 40.0], egui::Button::new("⏯ Play")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_MEDIA_PLAY_PAUSE");
            }
            if ui.add_sized([70.0, 40.0], egui::Button::new("⏩")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_MEDIA_FAST_FORWARD");
            }
            if ui.add_sized([70.0, 40.0], egui::Button::new("⏭")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_MEDIA_NEXT");
            }
        });

        ui.add_space(6.0);

        // Volume & Power
        ui.horizontal(|ui| {
            if ui.add_sized([70.0, 40.0], egui::Button::new("🔊 Vol+")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_VOLUME_UP");
            }
            if ui.add_sized([70.0, 40.0], egui::Button::new("🔉 Vol-")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_VOLUME_DOWN");
            }
            if ui.add_sized([70.0, 40.0], egui::Button::new("🔇 Mute")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_VOLUME_MUTE");
            }
            if ui.add_sized([85.0, 40.0], egui::Button::new("💤 Veille")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_SLEEP");
                self.log("MiBox en veille");
            }
            if ui.add_sized([85.0, 40.0], egui::Button::new("☀ Réveil").fill(egui::Color32::from_rgb(40, 100, 50))).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_WAKEUP");
                self.log("MiBox réveillée");
            }
        });
        ui.horizontal(|ui| {
            if ui.add_sized([85.0, 34.0], egui::Button::new("⏻ Power")).clicked() {
                self.tv_command(&id, "input keyevent KEYCODE_POWER");
            }
            if ui.add_sized([85.0, 34.0], egui::Button::new("🔄 Reboot").fill(egui::Color32::from_rgb(120, 40, 40))).clicked() {
                let id_clone = id.clone();
                let tx = self.bg_tx.clone();
                std::thread::spawn(move || {
                    let _ = Command::new("adb")
                        .args(["-s", &id_clone, "reboot"])
                        .output();
                    let _ = tx.send(BgEvent::Log("MiBox redémarrage...".into()));
                });
                self.log("Reboot MiBox...");
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        // Apps — colored cards with logos
        ui.label(egui::RichText::new("Applications").strong());
        ui.horizontal_wrapped(|ui| {
            let apps: &[(&str, &str, [u8; 3], &str)] = &[
                ("youtube", "YouTube", [180, 20, 20], "am start -n com.google.android.youtube.tv/com.google.android.apps.youtube.tv.activity.ShellActivity"),
                ("netflix", "Netflix", [139, 0, 0], "am start -n com.netflix.ninja/.MainActivity"),
                ("plex", "Plex", [180, 160, 20], "am start -n com.plexapp.android/.activity.SplashActivity"),
                ("spotify", "Spotify", [30, 120, 40], "am start -n com.spotify.tv.android/.SpotifyTVActivity"),
                ("oqee", "Oqee", [40, 40, 120], "am start -a android.intent.action.MAIN -n net.oqee.androidtv.store/net.oqee.androidtv.ui.splash.SplashActivity"),
            ];

            for (icon_name, label, color, command) in apps {
                let frame = egui::Frame::NONE
                    .corner_radius(8.0)
                    .inner_margin(6.0)
                    .fill(egui::Color32::from_rgb(color[0], color[1], color[2]));
                frame.show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(56.0);
                        let img_source = match *icon_name {
                            "youtube" => egui::include_image!("../assets/youtube.png"),
                            "netflix" => egui::include_image!("../assets/netflix.png"),
                            "plex" => egui::include_image!("../assets/plex.png"),
                            "spotify" => egui::include_image!("../assets/spotify.png"),
                            _ => egui::include_image!("../assets/oqee.png"),
                        };
                        let img = egui::Image::new(img_source)
                            .fit_to_exact_size(egui::vec2(36.0, 36.0));
                        if ui.add(egui::Button::image(img)).clicked() {
                            self.tv_command(&id, command);
                            self.log(label);
                        }
                        ui.label(egui::RichText::new(*label).small().strong().color(egui::Color32::WHITE));
                    });
                });
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        // TV Channels
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Chaînes TV").strong());
            let edit_label = if self.channel_edit_mode { "✓ Terminé" } else { "✏ Éditer" };
            if ui.small_button(edit_label).clicked() {
                self.channel_edit_mode = !self.channel_edit_mode;
            }
        });

        // Channel grid — fixed 4 columns, uniform button size
        let mut channel_to_send: Option<u32> = None;
        let mut channel_to_delete: Option<usize> = None;
        let cols = 4;
        let btn_size = egui::vec2(125.0, 32.0);

        egui::Grid::new("channels_grid")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (i, ch) in self.tv_channels.iter().enumerate() {
                    let text = format!("{} · {}", ch.number, ch.name);
                    let btn = egui::Button::new(
                        egui::RichText::new(&text).size(12.0).strong()
                    )
                    .fill(egui::Color32::from_rgb(35, 45, 70))
                    .corner_radius(6.0);

                    if self.channel_edit_mode {
                        ui.horizontal(|ui| {
                            if ui.add_sized(btn_size, btn).clicked() {
                                channel_to_send = Some(ch.number);
                            }
                            if ui.small_button("✕").clicked() {
                                channel_to_delete = Some(i);
                            }
                        });
                    } else if ui.add_sized(btn_size, btn).clicked() {
                        channel_to_send = Some(ch.number);
                    }

                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });

        if let Some(number) = channel_to_send {
            self.send_channel_number(&id, number);
            self.log(&format!("Chaîne {}", number));
        }

        if let Some(idx) = channel_to_delete {
            let removed = self.tv_channels.remove(idx);
            self.log(&format!("Chaîne {} supprimée", removed.name));
            save_channels(&self.tv_channels);
        }

        // Replay OQEE — rewind timeline
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Replay OQEE").strong());

        let mut replay_mins: Option<u32> = None;

        ui.horizontal(|ui| {
            if ui.button("⏪ 30m").clicked() { replay_mins = Some(30); }
            if ui.button("⏪ 1h").clicked() { replay_mins = Some(60); }
            if ui.button("⏪ 1h30").clicked() { replay_mins = Some(90); }
            if ui.button("⏪ 2h").clicked() { replay_mins = Some(120); }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.replay_custom_min)
                .hint_text("min")
                .desired_width(40.0));
            let valid = self.replay_custom_min.parse::<u32>().is_ok();
            if ui.add_enabled(valid, egui::Button::new("⏪ Go")).clicked() {
                if let Ok(m) = self.replay_custom_min.parse::<u32>() {
                    replay_mins = Some(m);
                }
            }
            ui.separator();
            ui.label(egui::RichText::new(format!("1s={:.0}min", self.replay_ratio)).small());
            if ui.small_button("-").clicked() {
                self.replay_ratio = (self.replay_ratio - 1.0).max(1.0);
                save_replay_ratio(self.replay_ratio);
            }
            if ui.small_button("+").clicked() {
                self.replay_ratio += 1.0;
                save_replay_ratio(self.replay_ratio);
            }
        });

        if let Some(mins) = replay_mins {
            let hold_secs = (mins as f32) / self.replay_ratio;
            let hold_secs = hold_secs.max(0.5);
            self.log(&format!("Replay -{}min (maintien {:.1}s)", mins, hold_secs));
            let id_clone = id.clone();
            let tx = self.bg_tx.clone();
            std::thread::spawn(move || {
                let shell_cmd = format!(
                    concat!(
                        "DEV=$(getevent -pl 2>&1 | awk '/^add device/{{dev=$NF}} /KEY_LEFT/{{print dev; exit}}' | tr -d ':'); ",
                        "if [ -n \"$DEV\" ]; then ",
                        "sendevent $DEV 1 105 1; sendevent $DEV 0 0 0; ",
                        "sleep {:.1}; ",
                        "sendevent $DEV 1 105 0; sendevent $DEV 0 0 0; ",
                        "sleep 0.5; input keyevent KEYCODE_DPAD_CENTER; ",
                        "fi"
                    ),
                    hold_secs
                );
                let _ = Command::new("adb")
                    .args(["-s", &id_clone, "shell", &shell_cmd])
                    .output();
                let _ = tx.send(BgEvent::Log("Replay terminé".into()));
            });
        }

        // Add channel inline
        if self.channel_edit_mode {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("➕");
                ui.add(egui::TextEdit::singleline(&mut self.new_channel_number)
                    .hint_text("N°")
                    .desired_width(30.0));
                ui.add(egui::TextEdit::singleline(&mut self.new_channel_name)
                    .hint_text("Nom")
                    .desired_width(80.0));
                let can_add = !self.new_channel_name.is_empty()
                    && self.new_channel_number.parse::<u32>().is_ok();
                if ui.add_enabled(can_add, egui::Button::new("Ajouter")).clicked() {
                    if let Ok(num) = self.new_channel_number.parse::<u32>() {
                        let name = self.new_channel_name.clone();
                        self.log(&format!("Chaîne {} {} ajoutée", num, name));
                        self.tv_channels.push(TvChannel { name, number: num });
                        self.tv_channels.sort_by_key(|c| c.number);
                        save_channels(&self.tv_channels);
                        self.new_channel_name.clear();
                        self.new_channel_number.clear();
                    }
                }
            });
        }
    }

    // ===================== TAB: PHONE =====================
    fn ui_tab_phone(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Stay Awake toggle
        let prev = self.stay_awake;
        ui.checkbox(&mut self.stay_awake, "☀ Stay Awake (empêche la veille)");
        if prev != self.stay_awake {
            if let Some(ref id) = self.get_selected_id() {
                set_stay_awake_cmd(id, self.stay_awake);
                self.log(if self.stay_awake { "Stay Awake ON" } else { "Stay Awake OFF" });
            }
        }

        ui.add_space(8.0);

        // Webcam Section
        ui.group(|ui| {
            ui.label(egui::RichText::new("📷 Webcam Discord/OBS").strong());

            ui.horizontal(|ui| {
                ui.label("Caméra:");
                let switch_disabled = self.switching_cam;
                if ui.add_enabled(!switch_disabled, egui::Button::new("⬛ BACK").selected(!self.cam_front)).clicked() {
                    let was_front = self.cam_front;
                    self.cam_front = false;
                    if was_front && self.webcam_active {
                        if let Some(id) = self.get_selected_id() {
                            self.switch_camera_async(id, ctx);
                        }
                    }
                }
                if ui.add_enabled(!switch_disabled, egui::Button::new("🤳 FRONT").selected(self.cam_front)).clicked() {
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
                ui.checkbox(&mut self.audio_output, "🔊 Audio");
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

        ui.add_space(6.0);

        // Mirroring
        ui.group(|ui| {
            ui.label(egui::RichText::new("🖥 Mirroring Écran").strong());

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

        ui.add_space(8.0);

        // Quick phone actions
        ui.label(egui::RichText::new("Actions rapides").strong());
        if let Some(ref id) = self.get_selected_id() {
            ui.horizontal(|ui| {
                if ui.add_sized([90.0, 40.0], egui::Button::new("📸 Photo")).clicked() { open_camera(id); }
                if ui.add_sized([90.0, 40.0], egui::Button::new("🎥 Vidéo")).clicked() { open_video(id); }
                if ui.add_sized([90.0, 40.0], egui::Button::new("🎙 Micro")).clicked() { open_mic(id); }
                if ui.add_sized([90.0, 40.0], egui::Button::new("🏠 Home")).clicked() { press_key(id, "KEYCODE_HOME"); }
                if ui.add_sized([90.0, 40.0], egui::Button::new("⬅ Back")).clicked() { press_key(id, "KEYCODE_BACK"); }
            });
        }
    }

    // ===================== TAB: VIDEO =====================
    fn ui_tab_video(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Stream URL
        ui.group(|ui| {
            ui.label(egui::RichText::new("🔗 Lire une URL").strong());
            ui.horizontal(|ui| {
                let url_width = (ui.available_width() - 80.0).max(100.0);
                ui.add(egui::TextEdit::singleline(&mut self.video_url)
                    .hint_text("https://... ou chemin local")
                    .desired_width(url_width));

                if ui.button("▶ Lire").clicked() && !self.video_url.is_empty() {
                    if let Some(ref id) = self.get_selected_id() {
                        play_video_url(id, &self.video_url);
                        self.log(&format!("Lecture: {}", &self.video_url[..self.video_url.len().min(30)]));
                    }
                }
            });
        });

        ui.add_space(6.0);

        // File transfer
        ui.group(|ui| {
            ui.label(egui::RichText::new("📤 Transfert fichier").strong());
            ui.horizontal(|ui| {
                let path_width = (ui.available_width() - 80.0).max(100.0);
                ui.add(egui::TextEdit::singleline(&mut self.file_path)
                    .hint_text("/chemin/vers/video.mp4")
                    .desired_width(path_width));

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
                let progress = if transfer_state.total_bytes > 0 {
                    transfer_state.transferred_bytes as f32 / transfer_state.total_bytes as f32
                } else {
                    0.0
                };

                ui.label(format!("📤 {}", transfer_state.filename));
                ui.add(egui::ProgressBar::new(progress)
                    .text(format!("{:.0}% - {:.1} MB / {:.1} MB",
                        progress * 100.0,
                        transfer_state.transferred_bytes as f64 / 1_000_000.0,
                        transfer_state.total_bytes as f64 / 1_000_000.0))
                    .animate(true));

                if transfer_state.done {
                    ui.label(egui::RichText::new("✓ Terminé!").color(egui::Color32::GREEN));
                    if let Ok(mut t) = self.transfer.lock() {
                        t.active = false;
                        t.done = false;
                    }
                }

                ctx.request_repaint();
            } else {
                let file_ok = !self.file_path.is_empty() && Path::new(&self.file_path).exists();

                ui.horizontal(|ui| {
                    ui.add_enabled_ui(file_ok, |ui| {
                        if ui.add_sized([140.0, 36.0], egui::Button::new("📤 Envoyer")).clicked() {
                            if let Some(ref id) = self.get_selected_id() {
                                self.log("Transfert...");
                                let path = self.file_path.clone();
                                start_transfer(id, &path, Arc::clone(&self.transfer), false);
                            }
                        }

                        if ui.add_sized([140.0, 36.0], egui::Button::new("▶ Envoyer+Lire")
                            .fill(egui::Color32::from_rgb(20, 80, 30))
                        ).clicked() {
                            if let Some(ref id) = self.get_selected_id() {
                                self.log("Envoi + lecture...");
                                let path = self.file_path.clone();
                                start_transfer(id, &path, Arc::clone(&self.transfer), true);
                            }
                        }
                    });
                });

                if !file_ok && !self.file_path.is_empty() {
                    ui.label(egui::RichText::new("⚠ Fichier introuvable").color(egui::Color32::RED).small());
                }
            }
        });
    }
}

impl eframe::App for PhoneTvApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process background events and check child processes
        self.process_bg_events(ctx);
        self.check_children();

        // Guard: if active tab is disabled, fallback to Devices
        if !self.tab_enabled(self.active_tab) {
            self.active_tab = Tab::Devices;
        }

        // ===================== TOP PANEL: Tab bar + Status =====================
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let tabs = [
                    (Tab::Devices, "📱 Appareils"),
                    (Tab::Tv, "📺 TV"),
                    (Tab::Phone, "📷 Phone"),
                    (Tab::Video, "🎬 Vidéo"),
                ];
                for (tab, label) in tabs {
                    let enabled = self.tab_enabled(tab);
                    let selected = self.active_tab == tab;
                    if ui.add_enabled(enabled, egui::Button::new(label).selected(selected)).clicked() {
                        self.active_tab = tab;
                    }
                }
            });

            // Status bar — selected device info
            if let Some(device) = self.get_selected() {
                let (icon, _) = match device.device_type {
                    DeviceType::Phone => ("📱", "Phone"),
                    DeviceType::Tv => ("📺", "TV"),
                    DeviceType::Unknown => ("❓", "?"),
                };
                let status_text = if device.status == "device" { "connecté" } else { "offline" };
                let status_color = if device.status == "device" { egui::Color32::GREEN } else { egui::Color32::YELLOW };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{} {}", icon, device.name)).strong());
                    ui.label(egui::RichText::new(format!("[{}]", status_text)).color(status_color).small());
                });
            }
            ui.add_space(2.0);
        });

        // ===================== BOTTOM PANEL: STOP + Logs =====================
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_space(4.0);

            // STOP button — full width
            if ui.add_sized([ui.available_width(), 36.0],
                egui::Button::new(egui::RichText::new("🛑 STOP TOUT").strong())
                    .fill(egui::Color32::from_rgb(139, 0, 0))
            ).clicked() {
                self.stop_all();
                self.log("Tout stoppé");
            }

            ui.add_space(4.0);

            // Collapsible logs
            let log_count = self.logs.len();
            ui.horizontal(|ui| {
                let arrow = if self.logs_collapsed { "▶" } else { "▼" };
                if ui.button(format!("{} Logs ({})", arrow, log_count)).clicked() {
                    self.logs_collapsed = !self.logs_collapsed;
                }
                if ui.small_button("Clear").clicked() {
                    self.logs.clear();
                }
            });

            if !self.logs_collapsed {
                egui::ScrollArea::vertical()
                    .max_height(60.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for log in &self.logs {
                            ui.label(egui::RichText::new(log).small());
                        }
                    });
            }

            ui.add_space(4.0);
        });

        // ===================== CENTRAL PANEL: Tab content =====================
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.active_tab {
                    Tab::Devices => self.ui_tab_devices(ui, ctx),
                    Tab::Tv => self.ui_tab_tv(ui, ctx),
                    Tab::Phone => self.ui_tab_phone(ui, ctx),
                    Tab::Video => self.ui_tab_video(ui, ctx),
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
        "--turn-screen-off".to_string(),
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
        // Timeout 1s to catch slower devices (Android TV, MiBox, etc.)
        let script = format!(
            r#"for i in $(seq 1 254); do
                (timeout 1 bash -c "echo >/dev/tcp/{prefix}$i/5555" 2>/dev/null && echo "{prefix}$i") &
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
