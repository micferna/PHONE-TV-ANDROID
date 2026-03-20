use eframe::egui;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::adb;
use crate::config::{self, Settings};
use crate::theme;
use crate::types::*;
use crate::ui;

pub struct PhoneTvApp {
    pub devices: Vec<Device>,
    pub selected_device: Option<usize>,
    pub active_tab: Tab,
    pub dark_mode: bool,
    pub settings: Settings,
    // Logs
    pub logs: VecDeque<String>,
    pub logs_collapsed: bool,
    // Phone options
    pub cam_front: bool,
    pub with_mic: bool,
    pub audio_output: bool,
    pub stay_awake: bool,
    pub webcam_active: bool,
    pub mirror_active: bool,
    // Process tracking
    pub webcam_child: Option<Child>,
    pub mirror_child: Option<Child>,
    // Transfer
    pub video_url: String,
    pub file_path: String,
    pub transfer: Arc<Mutex<TransferState>>,
    // Network scan
    pub network_devices: Vec<String>,
    pub scanning: bool,
    pub manual_ip: String,
    // Background events
    pub bg_rx: mpsc::Receiver<BgEvent>,
    pub bg_tx: mpsc::Sender<BgEvent>,
    // Async state flags
    pub refreshing: bool,
    pub connecting: bool,
    pub switching_cam: bool,
    // TV shell
    pub tv_shell: Option<TvShell>,
    // TV storage
    pub tv_storage: Option<(String, String, String, f32)>,
    pub tv_storage_device: String,
    // TV channels
    pub tv_channels: Vec<TvChannel>,
    pub channel_edit_mode: bool,
    pub new_channel_name: String,
    pub new_channel_number: String,
    // TV text input (NEW)
    pub tv_text_input: String,
    // TV screenshot (NEW)
    pub tv_screenshot: Option<Vec<u8>>,
    pub tv_screenshot_loading: bool,
    // Replay
    pub replay_custom_min: String,
    // Phone battery (NEW)
    pub phone_battery: Option<(u8, String)>,
    // Phone apps (NEW)
    pub phone_apps: Vec<String>,
    pub phone_apps_loading: bool,
    // Security
    pub security_view: SecurityView,
    pub security_score: Option<(u8, Vec<SecurityIssue>)>,
    pub security_score_loading: bool,
    pub security_apps: Vec<AppInfo>,
    pub security_apps_filter: AppFilter,
    pub security_apps_sort: AppSort,
    pub security_apps_search: String,
    pub security_apps_loading: bool,
    pub security_loading_cancel: Arc<AtomicBool>,
    pub security_permission_view: PermissionView,
    pub security_permission_cache: HashMap<String, Vec<PermissionInfo>>,
    pub security_selected_app: Option<String>,
    pub security_monitoring_view: MonitoringView,
    pub security_processes: Vec<ProcessInfo>,
    pub security_processes_loading: bool,
    pub security_processes_auto_refresh: bool,
    pub security_processes_last_refresh: f64,
    pub security_data_usage: Vec<DataUsage>,
    pub security_data_usage_loading: bool,
    pub security_wakelocks: Vec<WakelockInfo>,
    pub security_wakelocks_loading: bool,
    pub security_posture: Vec<DevicePosture>,
    pub security_posture_loading: bool,
    pub security_permissions_loading: bool,
    pub blacklist: Vec<String>,
    pub blacklist_alerts: Vec<String>,
    pub blacklist_new_entry: String,
    pub confirm_clear_data: Option<String>,
    pub confirm_uninstall: Option<String>,
    pub security_apps_loaded_count: usize,
    pub security_auto_loaded_device: Option<String>,
}

impl PhoneTvApp {
    pub fn new(settings: Settings) -> Self {
        let devices = adb::get_all_devices();
        let selected = if devices.is_empty() { None } else { Some(0) };
        let (bg_tx, bg_rx) = mpsc::channel();
        let dark_mode = settings.dark_mode;
        Self {
            devices,
            selected_device: selected,
            active_tab: Tab::Devices,
            dark_mode,
            settings,
            logs: VecDeque::from(["Bienvenue! Connectez vos appareils Android.".to_string()]),
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
            bg_rx,
            bg_tx,
            refreshing: false,
            connecting: false,
            switching_cam: false,
            tv_shell: None,
            tv_storage: None,
            tv_storage_device: String::new(),
            tv_channels: config::load_channels(),
            channel_edit_mode: false,
            new_channel_name: String::new(),
            new_channel_number: String::new(),
            tv_text_input: String::new(),
            tv_screenshot: None,
            tv_screenshot_loading: false,
            replay_custom_min: String::new(),
            phone_battery: None,
            phone_apps: Vec::new(),
            phone_apps_loading: false,
            // Security
            security_view: SecurityView::Score,
            security_score: None,
            security_score_loading: false,
            security_apps: Vec::new(),
            security_apps_filter: AppFilter::ThirdParty,
            security_apps_sort: AppSort::Danger,
            security_apps_search: String::new(),
            security_apps_loading: false,
            security_loading_cancel: Arc::new(AtomicBool::new(false)),
            security_permission_view: PermissionView::ByPermission,
            security_permission_cache: HashMap::new(),
            security_selected_app: None,
            security_monitoring_view: MonitoringView::Processes,
            security_processes: Vec::new(),
            security_processes_loading: false,
            security_processes_auto_refresh: false,
            security_processes_last_refresh: 0.0,
            security_data_usage: Vec::new(),
            security_data_usage_loading: false,
            security_wakelocks: Vec::new(),
            security_wakelocks_loading: false,
            security_posture: Vec::new(),
            security_posture_loading: false,
            security_permissions_loading: false,
            blacklist: config::load_blacklist(),
            blacklist_alerts: Vec::new(),
            blacklist_new_entry: String::new(),
            confirm_clear_data: None,
            confirm_uninstall: None,
            security_apps_loaded_count: 0,
            security_auto_loaded_device: None,
        }
    }

    pub fn log(&mut self, msg: &str) {
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push_back(format!("[{}] {}", now, msg));
        if self.logs.len() > 15 {
            self.logs.pop_front();
        }
    }

    pub fn get_selected(&self) -> Option<&Device> {
        self.selected_device.and_then(|i| self.devices.get(i))
    }

    pub fn get_selected_id(&self) -> Option<String> {
        self.get_selected().map(|d| d.id.clone())
    }

    pub fn save_settings(&self) {
        let s = Settings {
            dark_mode: self.dark_mode,
            replay_ratio: self.settings.replay_ratio,
            window_size: self.settings.window_size,
        };
        config::save_settings(&s);
    }

    pub fn tab_enabled(&self, tab: Tab) -> bool {
        match tab {
            Tab::Devices => true,
            Tab::Tv => self
                .get_selected()
                .map(|d| d.device_type == DeviceType::Tv)
                .unwrap_or(false),
            Tab::Phone => self
                .get_selected()
                .map(|d| d.device_type == DeviceType::Phone)
                .unwrap_or(false),
            Tab::Video => self.get_selected_id().is_some(),
            Tab::Security => self.get_selected_id().is_some(),
        }
    }

    pub fn refresh_async(&mut self, ctx: &egui::Context) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let devices = adb::get_all_devices();
            let _ = tx.send(BgEvent::DevicesLoaded(devices));
            ctx.request_repaint();
        });
    }

    pub fn scan_network_async(&mut self, ctx: &egui::Context) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        self.network_devices.clear();
        self.log("Scan réseau en cours...");
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let found = adb::scan_network_for_adb();
            let _ = tx.send(BgEvent::NetworkScanDone(found));
            ctx.request_repaint();
        });
    }

    pub fn connect_wifi_async(&mut self, addr: String, ctx: &egui::Context) {
        if self.connecting {
            return;
        }
        self.connecting = true;
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let success = adb::connect_adb_wifi(&addr);
            let _ = tx.send(BgEvent::WifiConnected { addr, success });
            ctx.request_repaint();
        });
    }

    pub fn switch_camera_async(&mut self, id: String, ctx: &egui::Context) {
        if self.switching_cam {
            return;
        }
        self.switching_cam = true;
        self.kill_webcam();
        let front = self.cam_front;
        let with_mic = self.with_mic;
        let audio_output = self.audio_output;
        let tx = self.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let child = adb::start_webcam_process(&id, front, with_mic, audio_output);
            let _ = tx.send(BgEvent::WebcamSwitched(child));
            ctx.request_repaint();
        });
    }

    pub fn kill_webcam(&mut self) {
        if let Some(mut child) = self.webcam_child.take() {
            adb::kill_child_tree(&mut child);
        }
    }

    pub fn kill_mirror(&mut self) {
        if let Some(mut child) = self.mirror_child.take() {
            adb::kill_child_tree(&mut child);
        }
    }

    pub fn stop_all(&mut self) {
        self.switching_cam = false;
        self.kill_webcam();
        self.kill_mirror();
        self.kill_tv_shell();
        self.webcam_active = false;
        self.mirror_active = false;
        let _ = Command::new("adb")
            .args(["shell", "pkill", "-f", "scrcpy"])
            .spawn();
    }

    pub fn ensure_tv_shell(&mut self, device_id: &str) -> bool {
        let need_new = match &mut self.tv_shell {
            Some(shell) => {
                if shell.device_id != device_id {
                    true
                } else if let Ok(Some(_)) = shell.child.try_wait() {
                    true
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

    pub fn tv_command(&mut self, device_id: &str, cmd: &str) {
        if !self.ensure_tv_shell(device_id) {
            adb::adb_fire(device_id, &["shell", cmd]);
            return;
        }
        if let Some(ref mut shell) = self.tv_shell {
            let full_cmd = format!("{}\n", cmd);
            if shell.stdin.write_all(full_cmd.as_bytes()).is_err()
                || shell.stdin.flush().is_err()
            {
                self.tv_shell = None;
                adb::adb_fire(device_id, &["shell", cmd]);
            }
        }
    }

    pub fn kill_tv_shell(&mut self) {
        if let Some(mut shell) = self.tv_shell.take() {
            let _ = shell.child.kill();
            let _ = shell.child.wait();
        }
    }

    pub fn send_channel_number(&mut self, device_id: &str, number: u32) {
        let id = device_id.to_string();
        let bg_tx = self.bg_tx.clone();
        std::thread::spawn(move || {
            let focus_line = adb::adb_device(&id, &["shell", "dumpsys", "window", "windows"])
                .map(|out| {
                    out.lines()
                        .find(|l| l.contains("mCurrentFocus"))
                        .unwrap_or("")
                        .trim()
                        .to_string()
                })
                .unwrap_or_default();
            let is_oqee_fg = focus_line.contains("net.oqee.androidtv");
            let _ = bg_tx.send(BgEvent::Log(format!(
                "[1] Focus: {}",
                if focus_line.len() > 60 {
                    &focus_line[focus_line.len() - 60..]
                } else {
                    &focus_line
                }
            )));

            if !is_oqee_fg {
                let _ = bg_tx.send(BgEvent::Log("[2] HOME...".into()));
                adb::adb_fire(&id, &["shell", "input", "keyevent", "KEYCODE_HOME"]);
                std::thread::sleep(std::time::Duration::from_millis(1000));

                let _ = bg_tx.send(BgEvent::Log("[3] Kill + lancement OQEE...".into()));
                adb::adb_fire(
                    &id,
                    &["shell", "am", "force-stop", "net.oqee.androidtv.store"],
                );
                std::thread::sleep(std::time::Duration::from_millis(500));
                adb::adb_fire(
                    &id,
                    &[
                        "shell",
                        "am",
                        "start",
                        "-n",
                        "net.oqee.androidtv.store/net.oqee.androidtv.ui.main.RealMainActivity",
                    ],
                );

                let mut wait_count = 0;
                let mut consecutive = 0;
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    wait_count += 1;
                    let ready =
                        adb::adb_device(&id, &["shell", "dumpsys", "window", "windows"])
                            .map(|out| {
                                out.lines().any(|l| {
                                    l.contains("mCurrentFocus")
                                        && l.contains("net.oqee.androidtv")
                                })
                            })
                            .unwrap_or(false);
                    if ready {
                        consecutive += 1;
                        if consecutive >= 2 {
                            break;
                        }
                    } else {
                        consecutive = 0;
                    }
                }
                let _ = bg_tx.send(BgEvent::Log(format!(
                    "[4] OQEE stable après {}x500ms",
                    wait_count
                )));

                std::thread::sleep(std::time::Duration::from_millis(1500));

                let focus2 =
                    adb::adb_device(&id, &["shell", "dumpsys", "window", "windows"])
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
                let _ = bg_tx.send(BgEvent::Log(format!(
                    "[5] OQEE={} Live={} | {}",
                    on_oqee,
                    on_live,
                    if focus2.len() > 40 {
                        &focus2[focus2.len() - 40..]
                    } else {
                        &focus2
                    }
                )));

                if !on_oqee {
                    let _ = bg_tx.send(BgEvent::Log("[5b] Retry lancement...".into()));
                    adb::adb_fire(
                        &id,
                        &[
                            "shell",
                            "am",
                            "start",
                            "-n",
                            "net.oqee.androidtv.store/net.oqee.androidtv.ui.main.RealMainActivity",
                        ],
                    );
                    std::thread::sleep(std::time::Duration::from_millis(3000));
                }

                if !on_live {
                    let focus3 =
                        adb::adb_device(&id, &["shell", "dumpsys", "window", "windows"])
                            .map(|out| {
                                out.lines()
                                    .find(|l| l.contains("mCurrentFocus"))
                                    .unwrap_or("")
                                    .trim()
                                    .to_string()
                            })
                            .unwrap_or_default();
                    if focus3.contains("net.oqee.androidtv")
                        && !focus3.contains("LivePlayer")
                    {
                        let _ = bg_tx.send(BgEvent::Log("[6] Menu OQEE → OK...".into()));
                        adb::adb_fire(
                            &id,
                            &["shell", "input", "keyevent", "KEYCODE_DPAD_CENTER"],
                        );
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
                adb::adb_fire(&id, &["shell", "input", "keyevent", &cmd]);
                std::thread::sleep(std::time::Duration::from_millis(600));
            }
            let _ = bg_tx.send(BgEvent::Log(format!("→ Chaîne {} envoyée", number)));
        });
    }

    pub fn refresh_tv_storage(&mut self, device_id: &str, ctx: &egui::Context) {
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
                    if cols.len() >= 5 {
                        let total = cols[1].to_string();
                        let used = cols[2].to_string();
                        let avail = cols[3].to_string();
                        let percent = cols[4]
                            .trim_end_matches('%')
                            .parse::<f32>()
                            .unwrap_or(0.0)
                            / 100.0;
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
                    let phones = self
                        .devices
                        .iter()
                        .filter(|d| d.device_type == DeviceType::Phone)
                        .count();
                    let tvs = self
                        .devices
                        .iter()
                        .filter(|d| d.device_type == DeviceType::Tv)
                        .count();
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
                        if let Some(mut c) = child {
                            adb::kill_child_tree(&mut c);
                        }
                    } else {
                        self.webcam_child = child;
                        self.webcam_active = self.webcam_child.is_some();
                        self.switching_cam = false;
                        self.log(&format!(
                            "Switch → {}",
                            if self.cam_front { "FRONT" } else { "BACK" }
                        ));
                    }
                }
                BgEvent::StorageInfo {
                    device_id,
                    total,
                    used,
                    avail,
                    percent,
                } => {
                    self.tv_storage = Some((total, used, avail, percent));
                    self.tv_storage_device = device_id;
                }
                BgEvent::BatteryInfo {
                    device_id: _,
                    level,
                    status,
                } => {
                    self.phone_battery = Some((level, status));
                }
                BgEvent::PhoneApps {
                    device_id: _,
                    apps,
                } => {
                    self.phone_apps = apps;
                    self.phone_apps_loading = false;
                }
                BgEvent::ScreenshotReady {
                    device_id: _,
                    data,
                } => {
                    // Forget old screenshot texture if any
                    ctx.forget_image("bytes://tv_screenshot.png");
                    self.tv_screenshot = Some(data);
                    self.tv_screenshot_loading = false;
                    self.log("Capture d'écran reçue");
                }
                BgEvent::Log(msg) => {
                    self.log(&msg);
                }
                BgEvent::SecurityScore { score, issues } => {
                    self.security_score = Some((score, issues));
                    self.security_score_loading = false;
                }
                BgEvent::SecurityAppsList { packages } => {
                    self.security_apps_loaded_count = 0;
                    self.security_apps = packages
                        .into_iter()
                        .map(|p| AppInfo { package: p, ..Default::default() })
                        .collect();
                }
                BgEvent::SecurityAppDetail { package, info } => {
                    if let Some(app) = self.security_apps.iter_mut().find(|a| a.package == package) {
                        *app = info;
                    }
                    self.security_apps_loaded_count += 1;
                }
                BgEvent::SecurityProcesses { processes } => {
                    self.security_processes = processes;
                    self.security_processes_loading = false;
                }
                BgEvent::SecurityDataUsage { usage } => {
                    self.security_data_usage = usage;
                    self.security_data_usage_loading = false;
                }
                BgEvent::SecurityWakelocks { wakelocks } => {
                    self.security_wakelocks = wakelocks;
                    self.security_wakelocks_loading = false;
                }
                BgEvent::SecurityPosture { checks } => {
                    self.security_posture = checks;
                    self.security_posture_loading = false;
                }
                BgEvent::SecurityPermissions { package, permissions } => {
                    self.security_permission_cache.insert(package, permissions);
                    // Clear loading when we have permissions for all known apps
                    if !self.security_apps.is_empty() && self.security_permission_cache.len() >= self.security_apps.len() {
                        self.security_permissions_loading = false;
                    }
                }
                BgEvent::BlacklistAlert { found } => {
                    self.blacklist_alerts = found;
                }
                BgEvent::AppActionResult { package, action, success, message } => {
                    let status = if success { "✓" } else { "✗" };
                    self.log(&format!("{} {} {} : {}", status, action, package, message));
                    if success {
                        match action.as_str() {
                            "uninstall" => {
                                self.security_apps.retain(|a| a.package != package);
                                // Force score refresh
                                self.security_score = None;
                                self.security_score_loading = false;
                            }
                            "disable" => {
                                if let Some(a) = self.security_apps.iter_mut().find(|a| a.package == package) {
                                    a.enabled = false;
                                }
                            }
                            "enable" => {
                                if let Some(a) = self.security_apps.iter_mut().find(|a| a.package == package) {
                                    a.enabled = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                BgEvent::SecurityAppsLoadingDone => {
                    self.security_apps_loading = false;
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
}

impl eframe::App for PhoneTvApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_bg_events(ctx);
        self.check_children();

        // Guard: if active tab is disabled, fallback to Devices
        if !self.tab_enabled(self.active_tab) {
            self.active_tab = Tab::Devices;
        }

        // Sidebar
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(180.0)
            .frame(
                egui::Frame::NONE
                    .inner_margin(10.0)
                    .fill(theme::sidebar_fill(self.dark_mode)),
            )
            .show(ctx, |ui| {
                ui::draw_sidebar(self, ui, ctx);
            });

        // Bottom panel: logs
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_space(4.0);

            let log_count = self.logs.len();
            ui.horizontal(|ui| {
                let arrow = if self.logs_collapsed { "▶" } else { "▼" };
                if ui
                    .button(format!("{} Logs ({})", arrow, log_count))
                    .clicked()
                {
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
                            ui.label(egui::RichText::new(log).small().family(egui::FontFamily::Monospace));
                        }
                    });
            }

            ui.add_space(4.0);
        });

        // Central panel: tab content
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.active_tab {
                    Tab::Devices => ui::draw_devices(self, ui, ctx),
                    Tab::Tv => ui::draw_tv(self, ui, ctx),
                    Tab::Phone => ui::draw_phone(self, ui, ctx),
                    Tab::Video => ui::draw_video(self, ui, ctx),
                    Tab::Security => ui::draw_security(self, ui, ctx),
                }
            });
        });
    }
}
