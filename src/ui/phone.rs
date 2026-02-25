use eframe::egui;
use std::path::Path;

use crate::adb;
use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::BgEvent;

fn section(ui: &mut egui::Ui, dark_mode: bool, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .corner_radius(10.0)
        .inner_margin(14.0)
        .fill(theme::card_bg(dark_mode))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
    ui.add_space(8.0);
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).strong().size(15.0));
    ui.add_space(6.0);
}

pub fn draw_phone(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.add_space(4.0);

    // ======== Streaming (Webcam + Mirror côte à côte) ========
    section(ui, app.dark_mode, |ui| {
        section_title(ui, "📡 Streaming");

        ui.columns(2, |cols| {
            // LEFT: Webcam
            cols[0].vertical(|ui| {
                ui.label(egui::RichText::new("📷 Webcam").strong().size(13.0));
                ui.add_space(4.0);

                // Camera selector
                ui.horizontal(|ui| {
                    let sd = app.switching_cam;
                    if ui.add_enabled(!sd, egui::Button::new(
                        if !app.cam_front { egui::RichText::new("⬛ BACK").strong() } else { egui::RichText::new("⬛ BACK") }
                    ).selected(!app.cam_front).corner_radius(6.0)).clicked() {
                        let was_front = app.cam_front;
                        app.cam_front = false;
                        if was_front && app.webcam_active {
                            if let Some(id) = app.get_selected_id() { app.switch_camera_async(id, ctx); }
                        }
                    }
                    if ui.add_enabled(!sd, egui::Button::new(
                        if app.cam_front { egui::RichText::new("🤳 FRONT").strong() } else { egui::RichText::new("🤳 FRONT") }
                    ).selected(app.cam_front).corner_radius(6.0)).clicked() {
                        let was_back = !app.cam_front;
                        app.cam_front = true;
                        if was_back && app.webcam_active {
                            if let Some(id) = app.get_selected_id() { app.switch_camera_async(id, ctx); }
                        }
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.with_mic, "🎤 Micro");
                    ui.checkbox(&mut app.audio_output, "🔊 Audio");
                });

                if app.switching_cam {
                    ui.label(egui::RichText::new("⏳ Switch...").color(theme::warning_color()));
                }

                ui.add_space(6.0);
                let (btn_label, btn_color) = if app.webcam_active {
                    ("⏹ Stop Webcam", theme::danger_color())
                } else {
                    ("▶ Démarrer Webcam", theme::success_color())
                };
                if ui.add(
                    egui::Button::new(egui::RichText::new(btn_label).color(egui::Color32::WHITE))
                        .fill(btn_color)
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                ).clicked() {
                    if app.webcam_active {
                        app.kill_webcam();
                        app.webcam_active = false;
                        app.log("Webcam stoppée");
                    } else if let Some(ref id) = app.get_selected_id() {
                        let child = adb::start_webcam_process(id, app.cam_front, app.with_mic, app.audio_output);
                        if child.is_some() {
                            app.webcam_child = child;
                            app.webcam_active = true;
                            app.log(&format!("Webcam {} ON", if app.cam_front { "FRONT" } else { "BACK" }));
                        }
                    }
                }

                if app.webcam_active {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("● LIVE").color(egui::Color32::RED).strong());
                }

                if !Path::new("/dev/video10").exists() {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("⚠ v4l2loopback manquant").color(theme::warning_color()).size(11.0));
                }
            });

            // RIGHT: Mirror + Options
            cols[1].vertical(|ui| {
                ui.label(egui::RichText::new("🖥 Mirroring").strong().size(13.0));
                ui.add_space(4.0);

                let prev = app.stay_awake;
                ui.checkbox(&mut app.stay_awake, "☀ Stay Awake");
                if prev != app.stay_awake {
                    if let Some(ref id) = app.get_selected_id() {
                        adb::set_stay_awake_cmd(id, app.stay_awake);
                        app.log(if app.stay_awake { "Stay Awake ON" } else { "Stay Awake OFF" });
                    }
                }

                ui.add_space(12.0);
                let (btn_label, btn_color) = if app.mirror_active {
                    ("⏹ Stop Mirror", theme::danger_color())
                } else {
                    ("▶ Démarrer Mirror", egui::Color32::from_rgb(30, 50, 130))
                };
                if ui.add(
                    egui::Button::new(egui::RichText::new(btn_label).color(egui::Color32::WHITE))
                        .fill(btn_color)
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                ).clicked() {
                    if app.mirror_active {
                        app.kill_mirror();
                        app.mirror_active = false;
                        app.log("Mirroring stoppé");
                    } else if let Some(ref id) = app.get_selected_id() {
                        let child = adb::start_mirror_process(id, app.stay_awake);
                        if child.is_some() {
                            app.mirror_child = child;
                            app.mirror_active = true;
                            app.log("Mirroring actif");
                        }
                    }
                }

                if app.mirror_active {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("● ACTIF").color(theme::success_color()).strong());
                }
            });
        });
    });

    // ======== Actions rapides + Batterie côte à côte ========
    ui.columns(2, |cols| {
        // LEFT: Actions rapides
        section(&mut cols[0], app.dark_mode, |ui| {
            section_title(ui, "⚡ Actions rapides");
            if let Some(ref id) = app.get_selected_id() {
                egui::Grid::new("phone_actions").spacing([4.0, 4.0]).show(ui, |ui| {
                    for (label, action) in [
                        ("📸 Photo", "camera"),
                        ("🎥 Vidéo", "video"),
                        ("🎙 Micro", "mic"),
                    ] {
                        if ui.add_sized([90.0, 36.0], egui::Button::new(label).corner_radius(8.0)).clicked() {
                            match action {
                                "camera" => adb::open_camera(id),
                                "video" => adb::open_video(id),
                                _ => adb::open_mic(id),
                            }
                        }
                    }
                    ui.end_row();
                    for (label, key) in [
                        ("🏠 Home", "KEYCODE_HOME"),
                        ("⬅ Back", "KEYCODE_BACK"),
                        ("📱 Recent", "KEYCODE_APP_SWITCH"),
                    ] {
                        if ui.add_sized([90.0, 36.0], egui::Button::new(label).corner_radius(8.0)).clicked() {
                            adb::press_key(id, key);
                        }
                    }
                    ui.end_row();
                });
            }
        });

        // RIGHT: Battery
        section(&mut cols[1], app.dark_mode, |ui| {
            ui.horizontal(|ui| {
                section_title(ui, "🔋 Batterie");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("🔄").corner_radius(6.0)).clicked() {
                        if let Some(ref id) = app.get_selected_id() {
                            let id = id.clone();
                            let tx = app.bg_tx.clone();
                            std::thread::spawn(move || {
                                if let Some((level, status)) = adb::get_battery_info(&id) {
                                    let _ = tx.send(BgEvent::BatteryInfo { device_id: id, level, status });
                                }
                            });
                        }
                    }
                });
            });

            if let Some((level, ref status)) = app.phone_battery {
                let color = if level > 50 { theme::success_color() } else if level > 20 { theme::warning_color() } else { theme::danger_color() };

                // Big level display
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(format!("{}%", level)).size(32.0).strong().color(color));
                    ui.label(egui::RichText::new(status.as_str()).size(12.0).color(egui::Color32::GRAY));
                });
                ui.add_space(6.0);
                ui.add(
                    egui::ProgressBar::new(level as f32 / 100.0)
                        .fill(color)
                        .corner_radius(6.0),
                );
            } else {
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Appuyez 🔄").color(egui::Color32::GRAY));
                });
            }
        });
    });

    // ======== Sonnerie + Apps côte à côte ========
    ui.columns(2, |cols| {
        // LEFT: Sonnerie
        section(&mut cols[0], app.dark_mode, |ui| {
            section_title(ui, "🔔 Retrouver mon tel");
            ui.label(egui::RichText::new("Fait sonner le téléphone au volume max").size(11.0).color(egui::Color32::GRAY));
            ui.add_space(6.0);
            if ui.add(
                egui::Button::new(egui::RichText::new("🔊 Faire sonner").size(14.0).color(egui::Color32::WHITE))
                    .fill(theme::warning_color())
                    .corner_radius(8.0)
                    .min_size(egui::vec2(ui.available_width(), 38.0)),
            ).clicked() {
                if let Some(ref id) = app.get_selected_id() {
                    adb::ring_phone(id);
                    app.log("Sonnerie activée");
                }
            }
            ui.add_space(4.0);
            if ui.add(
                egui::Button::new(egui::RichText::new("🔇 Arrêter").size(13.0))
                    .corner_radius(8.0)
                    .min_size(egui::vec2(ui.available_width(), 32.0)),
            ).clicked() {
                if let Some(ref id) = app.get_selected_id() {
                    adb::stop_ring(id);
                    app.log("Sonnerie arrêtée");
                }
            }
        });

        // RIGHT: Apps
        section(&mut cols[1], app.dark_mode, |ui| {
            ui.horizontal(|ui| {
                section_title(ui, "📦 Apps tierces");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let loading = app.phone_apps_loading;
                    let btn_text = if loading { "⏳" } else { "🔄 Charger" };
                    if ui.add_enabled(!loading, egui::Button::new(btn_text).corner_radius(6.0)).clicked() {
                        app.phone_apps_loading = true;
                        if let Some(ref id) = app.get_selected_id() {
                            let id = id.clone();
                            let tx = app.bg_tx.clone();
                            std::thread::spawn(move || {
                                let apps = adb::get_third_party_apps(&id);
                                let _ = tx.send(BgEvent::PhoneApps { device_id: id, apps });
                            });
                        }
                    }
                });
            });

            if !app.phone_apps.is_empty() {
                ui.label(egui::RichText::new(format!("{} apps", app.phone_apps.len())).size(11.0).color(egui::Color32::GRAY));
                ui.add_space(2.0);
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    let mut to_uninstall: Option<String> = None;
                    for pkg in &app.phone_apps {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(pkg).size(11.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button(egui::RichText::new("🗑").color(theme::danger_color())).clicked() {
                                    to_uninstall = Some(pkg.clone());
                                }
                            });
                        });
                    }
                    if let Some(pkg) = to_uninstall {
                        if let Some(ref id) = app.get_selected_id() {
                            let id = id.clone();
                            let pkg_clone = pkg.clone();
                            let tx = app.bg_tx.clone();
                            std::thread::spawn(move || {
                                let ok = adb::uninstall_app(&id, &pkg_clone);
                                let msg = if ok { format!("{} désinstallé", pkg_clone) } else { format!("Échec {}", pkg_clone) };
                                let _ = tx.send(BgEvent::Log(msg));
                                let apps = adb::get_third_party_apps(&id);
                                let _ = tx.send(BgEvent::PhoneApps { device_id: id, apps });
                            });
                        }
                        app.log(&format!("Désinstallation {}...", pkg));
                    }
                });
            } else {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Appuyez 🔄 Charger").size(11.0).color(egui::Color32::GRAY));
                });
            }
        });
    });
}
