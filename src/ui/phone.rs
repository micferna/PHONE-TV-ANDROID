use eframe::egui;
#[cfg(target_os = "linux")]
use std::path::Path;

use crate::adb;
use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::BgEvent;

fn section(ui: &mut egui::Ui, dark_mode: bool, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(12.0)
        .fill(theme::card_bg(dark_mode))
        .stroke(egui::Stroke::new(0.5, theme::card_border(dark_mode)))
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
                    if ui
                        .add_enabled(
                            !sd,
                            egui::Button::new(if !app.cam_front {
                                egui::RichText::new("⬛ BACK").strong()
                            } else {
                                egui::RichText::new("⬛ BACK")
                            })
                            .selected(!app.cam_front)
                            .corner_radius(6.0),
                        )
                        .clicked()
                    {
                        let was_front = app.cam_front;
                        app.cam_front = false;
                        if was_front && app.webcam_active {
                            if let Some(id) = app.get_selected_id() {
                                app.switch_camera_async(id, ctx);
                            }
                        }
                    }
                    if ui
                        .add_enabled(
                            !sd,
                            egui::Button::new(if app.cam_front {
                                egui::RichText::new("🤳 FRONT").strong()
                            } else {
                                egui::RichText::new("🤳 FRONT")
                            })
                            .selected(app.cam_front)
                            .corner_radius(6.0),
                        )
                        .clicked()
                    {
                        let was_back = !app.cam_front;
                        app.cam_front = true;
                        if was_back && app.webcam_active {
                            if let Some(id) = app.get_selected_id() {
                                app.switch_camera_async(id, ctx);
                            }
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
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(btn_label).color(egui::Color32::WHITE),
                        )
                        .fill(btn_color)
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                    )
                    .clicked()
                {
                    if app.webcam_active {
                        app.kill_webcam();
                        app.webcam_active = false;
                        app.log("Webcam stoppée");
                    } else if let Some(ref id) = app.get_selected_id() {
                        let child = adb::start_webcam_process(
                            id,
                            app.cam_front,
                            app.with_mic,
                            app.audio_output,
                        );
                        if child.is_some() {
                            app.webcam_child = child;
                            app.webcam_active = true;
                            app.log(&format!(
                                "Webcam {} ON",
                                if app.cam_front { "FRONT" } else { "BACK" }
                            ));
                        }
                    }
                }

                if app.webcam_active {
                    ui.add_space(2.0);
                    let t = ui.ctx().input(|i| i.time);
                    let alpha = ((t * 3.0).sin() * 0.5 + 0.5) * 255.0;
                    let live_color =
                        egui::Color32::from_rgba_unmultiplied(248, 81, 73, alpha as u8);
                    ui.label(egui::RichText::new("● LIVE").color(live_color).strong());
                    ui.ctx().request_repaint();
                }

                #[cfg(target_os = "linux")]
                {
                    if !Path::new("/dev/video10").exists() {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new("⚠ v4l2loopback manquant")
                                .color(theme::warning_color())
                                .size(11.0),
                        );
                    }
                }

                if !adb::webcam_direct_supported() {
                    ui.add_space(4.0);
                    ui.collapsing("ℹ Setup webcam virtuelle", |ui| {
                        ui.label(
                            egui::RichText::new(
                                "scrcpy ouvre une fenêtre avec la caméra du téléphone. \
                                 Pour la voir comme webcam dans Discord/Teams/Zoom :",
                            )
                            .size(11.0),
                        );
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("1. Installer OBS Studio (gratuit)").size(11.0));
                        ui.hyperlink_to("→ obsproject.com", "https://obsproject.com/download");
                        ui.label(
                            egui::RichText::new(
                                "2. Dans OBS : Sources → + → Capture de fenêtre → choisir \"scrcpy\"\n\
                                 3. Cliquer \"Démarrer la caméra virtuelle\" (en bas à droite)\n\
                                 4. Dans Discord : choisir \"OBS Virtual Camera\"",
                            )
                            .size(11.0),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Alternative open source : Unity Capture")
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                        ui.hyperlink_to(
                            "→ github.com/schellingb/UnityCapture",
                            "https://github.com/schellingb/UnityCapture",
                        );
                    });
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
                        app.log(if app.stay_awake {
                            "Stay Awake ON"
                        } else {
                            "Stay Awake OFF"
                        });
                    }
                }

                ui.add_space(12.0);
                let (btn_label, btn_color) = if app.mirror_active {
                    ("⏹ Stop Mirror", theme::danger_color())
                } else {
                    ("▶ Démarrer Mirror", egui::Color32::from_rgb(30, 50, 130))
                };
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(btn_label).color(egui::Color32::WHITE),
                        )
                        .fill(btn_color)
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                    )
                    .clicked()
                {
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
                    ui.label(
                        egui::RichText::new("● ACTIF")
                            .color(theme::success_color())
                            .strong(),
                    );
                }
            });
        });
    });

    // ======== Actions rapides + Batterie côte à côte ========
    ui.columns(2, |cols| {
        // LEFT: Actions rapides — dashboard tiles
        section(&mut cols[0], app.dark_mode, |ui| {
            section_title(ui, "⚡ Actions rapides");
            if let Some(ref id) = app.get_selected_id() {
                let tile_size = egui::vec2(90.0, 70.0);
                let dark = app.dark_mode;
                egui::Grid::new("phone_actions")
                    .spacing([6.0, 6.0])
                    .show(ui, |ui| {
                        for (label, action) in [
                            ("📸\nPhoto", "camera"),
                            ("🎥\nVidéo", "video"),
                            ("🎙\nMicro", "mic"),
                        ] {
                            if ui
                                .add_sized(
                                    tile_size,
                                    egui::Button::new(
                                        egui::RichText::new(label).size(16.0).strong(),
                                    )
                                    .corner_radius(8.0)
                                    .fill(theme::widget_bg(dark)),
                                )
                                .clicked()
                            {
                                match action {
                                    "camera" => adb::open_camera(id),
                                    "video" => adb::open_video(id),
                                    _ => adb::open_mic(id),
                                }
                            }
                        }
                        ui.end_row();
                        for (label, key) in [
                            ("🏠\nHome", "KEYCODE_HOME"),
                            ("⬅\nBack", "KEYCODE_BACK"),
                            ("📱\nRecent", "KEYCODE_APP_SWITCH"),
                        ] {
                            if ui
                                .add_sized(
                                    tile_size,
                                    egui::Button::new(
                                        egui::RichText::new(label).size(16.0).strong(),
                                    )
                                    .corner_radius(8.0)
                                    .fill(theme::widget_bg(dark)),
                                )
                                .clicked()
                            {
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
                                    let _ = tx.send(BgEvent::BatteryInfo {
                                        device_id: id,
                                        level,
                                        status,
                                    });
                                }
                            });
                        }
                    }
                });
            });

            if let Some((level, ref status)) = app.phone_battery {
                let color = if level > 50 {
                    theme::success_color()
                } else if level > 20 {
                    theme::warning_color()
                } else {
                    theme::danger_color()
                };

                // Big level display
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}%", level))
                            .size(32.0)
                            .strong()
                            .color(color),
                    );
                    ui.label(
                        egui::RichText::new(status.as_str())
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
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

    // ======== Sonnerie (full width) ========
    section(ui, app.dark_mode, |ui| {
        section_title(ui, "🔔 Retrouver mon tel");
        ui.label(
            egui::RichText::new("Fait sonner le téléphone au volume max")
                .size(11.0)
                .color(egui::Color32::GRAY),
        );
        ui.add_space(6.0);
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("🔊 Faire sonner")
                        .size(14.0)
                        .color(egui::Color32::WHITE),
                )
                .fill(theme::warning_color())
                .corner_radius(8.0)
                .min_size(egui::vec2(ui.available_width(), 38.0)),
            )
            .clicked()
        {
            if let Some(ref id) = app.get_selected_id() {
                adb::ring_phone(id);
                app.log("Sonnerie activée");
            }
        }
        ui.add_space(4.0);
        if ui
            .add(
                egui::Button::new(egui::RichText::new("🔇 Arrêter").size(13.0))
                    .corner_radius(8.0)
                    .min_size(egui::vec2(ui.available_width(), 32.0)),
            )
            .clicked()
        {
            if let Some(ref id) = app.get_selected_id() {
                adb::stop_ring(id);
                app.log("Sonnerie arrêtée");
            }
        }
    });

    // ======== Screenshot ========
    if let Some(id) = app.get_selected_id() {
        section(ui, app.dark_mode, |ui| {
            section_title(ui, "📸 Capture d'écran");
            crate::ui::screenshot_panel(app, ui, ctx, &id);
        });
    }

    // ======== Screen recording ========
    if let Some(id) = app.get_selected_id() {
        section(ui, app.dark_mode, |ui| {
            section_title(ui, "🎥 Enregistrement écran");
            let recording = app.screenrecord_child.is_some();
            ui.label(
                egui::RichText::new(if recording {
                    "● Enregistrement en cours..."
                } else {
                    "Limite Android : ~3 min par capture"
                })
                .size(11.0)
                .color(if recording {
                    theme::danger_color()
                } else {
                    egui::Color32::GRAY
                }),
            );
            ui.add_space(6.0);

            if !recording {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("⏺ Démarrer").color(egui::Color32::WHITE),
                        )
                        .fill(theme::danger_color())
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                    )
                    .clicked()
                {
                    if let Some((child, remote)) = adb::start_screenrecord(&id) {
                        app.screenrecord_child = Some(child);
                        app.screenrecord_remote = remote;
                        app.screenrecord_device_id = id.clone();
                        app.log("Enregistrement démarré");
                    } else {
                        app.log("Échec démarrage screenrecord");
                    }
                }
            } else if ui
                .add(
                    egui::Button::new("⏹ Arrêter et sauvegarder")
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                )
                .clicked()
            {
                let default_name = format!(
                    "phone-tv-rec_{}.mp4",
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                );
                let path = rfd::FileDialog::new()
                    .add_filter("MP4", &["mp4"])
                    .set_file_name(&default_name)
                    .save_file();
                if let (Some(mut child), Some(dest)) = (app.screenrecord_child.take(), path) {
                    let remote = app.screenrecord_remote.clone();
                    let id = app.screenrecord_device_id.clone();
                    let dest_clone = dest.clone();
                    let tx = app.bg_tx.clone();
                    std::thread::spawn(move || {
                        let ok =
                            adb::stop_screenrecord_and_pull(&id, &mut child, &remote, &dest_clone);
                        let msg = if ok {
                            format!("Enregistrement sauvegardé: {}", dest_clone.display())
                        } else {
                            "Échec sauvegarde enregistrement".to_string()
                        };
                        let _ = tx.send(BgEvent::Log(msg));
                    });
                    app.screenrecord_remote.clear();
                    app.screenrecord_device_id.clear();
                } else {
                    // User cancelled save dialog: still stop the recording
                    if let Some(mut child) = app.screenrecord_child.take() {
                        adb::kill_child_tree(&mut child);
                        app.log("Enregistrement annulé");
                    }
                    app.screenrecord_remote.clear();
                    app.screenrecord_device_id.clear();
                }
            }
        });
    }

    // ======== Clipboard PC → phone ========
    if let Some(id) = app.get_selected_id() {
        section(ui, app.dark_mode, |ui| {
            section_title(ui, "📋 Presse-papier");
            ui.label(
                egui::RichText::new(
                    "Colle le texte du presse-papier PC dans le champ actif du téléphone",
                )
                .size(11.0)
                .color(egui::Color32::GRAY),
            );
            ui.add_space(6.0);
            if ui
                .add(
                    egui::Button::new("📋 Coller depuis le PC")
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 32.0)),
                )
                .clicked()
            {
                match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                    Ok(text) if !text.is_empty() => {
                        adb::send_text_to_device(&id, &text);
                        let preview: String = text.chars().take(40).collect();
                        app.log(&format!("Collé: {}", preview));
                    }
                    Ok(_) => app.log("Presse-papier vide"),
                    Err(e) => app.log(&format!("Erreur presse-papier: {}", e)),
                }
            }
        });
    }

    // ======== File push / pull ========
    if let Some(id) = app.get_selected_id() {
        section(ui, app.dark_mode, |ui| {
            section_title(ui, "📁 Transfert de fichier");
            let busy = app.file_transferring;

            // Push
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new("📤 Envoyer un fichier vers /sdcard/Download/")
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 32.0)),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let local = path.to_string_lossy().to_string();
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".into());
                    let remote = format!("/sdcard/Download/{}", filename);
                    app.file_transferring = true;
                    app.log(&format!("Envoi de {} -> {}", filename, remote));
                    let id_clone = id.clone();
                    let tx = app.bg_tx.clone();
                    std::thread::spawn(move || {
                        let (success, message) = adb::push_file(&id_clone, &local, &remote);
                        let _ = tx.send(BgEvent::FileTransferDone { success, message });
                    });
                }
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Récupérer depuis le téléphone:").size(11.0));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.pull_remote_path)
                        .hint_text("/sdcard/Download/file.ext")
                        .desired_width(ui.available_width() - 110.0),
                );
                if ui
                    .add_enabled(
                        !busy && !app.pull_remote_path.is_empty(),
                        egui::Button::new("📥 Récupérer"),
                    )
                    .clicked()
                {
                    let remote = app.pull_remote_path.clone();
                    let default_name = std::path::Path::new(&remote)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "pulled-file".into());
                    if let Some(dest) = rfd::FileDialog::new()
                        .set_file_name(&default_name)
                        .save_file()
                    {
                        let local = dest.to_string_lossy().to_string();
                        app.file_transferring = true;
                        app.log(&format!("Téléchargement {} -> {}", remote, local));
                        let id_clone = id.clone();
                        let tx = app.bg_tx.clone();
                        std::thread::spawn(move || {
                            let (success, message) = adb::pull_file(&id_clone, &remote, &local);
                            let _ = tx.send(BgEvent::FileTransferDone { success, message });
                        });
                    }
                }
            });
        });
    }

    // ======== Install APK ========
    if let Some(id) = app.get_selected_id() {
        section(ui, app.dark_mode, |ui| {
            section_title(ui, "📦 Installer APK");
            ui.label(
                egui::RichText::new(
                    "Sélectionne un fichier .apk pour l'installer sur le téléphone",
                )
                .size(11.0)
                .color(egui::Color32::GRAY),
            );
            ui.add_space(6.0);

            let installing = app.apk_installing;
            let btn_text = if installing {
                "⏳ Installation..."
            } else {
                "📥 Choisir un APK"
            };

            if ui
                .add_enabled(
                    !installing,
                    egui::Button::new(egui::RichText::new(btn_text).color(egui::Color32::WHITE))
                        .fill(theme::accent_color())
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 38.0)),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Android Package", &["apk"])
                    .pick_file()
                {
                    let apk = path.to_string_lossy().to_string();
                    app.apk_installing = true;
                    app.log(&format!(
                        "Installation de {}",
                        path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    ));
                    let id_clone = id.clone();
                    let tx = app.bg_tx.clone();
                    std::thread::spawn(move || {
                        let (success, message) = adb::install_apk(&id_clone, &apk);
                        let _ = tx.send(BgEvent::ApkInstalled { success, message });
                    });
                }
            }
        });
    }
}
