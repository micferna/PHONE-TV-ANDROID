use eframe::egui;
use std::process::Command;

use crate::adb;
use crate::app::PhoneTvApp;
use crate::config;
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

fn tv_btn(ui: &mut egui::Ui, label: &str, size: [f32; 2]) -> bool {
    ui.add_sized(
        size,
        egui::Button::new(egui::RichText::new(label).size(13.0)).corner_radius(8.0),
    )
    .clicked()
}

fn tv_btn_fill(ui: &mut egui::Ui, label: &str, size: [f32; 2], fill: egui::Color32) -> bool {
    ui.add_sized(
        size,
        egui::Button::new(egui::RichText::new(label).size(13.0).color(egui::Color32::WHITE))
            .fill(fill)
            .corner_radius(8.0),
    )
    .clicked()
}

pub fn draw_tv(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let id = match app.get_selected_id() {
        Some(id) => id,
        None => return,
    };

    ui.add_space(4.0);

    // ======== Stockage (compact en haut) ========
    if app.tv_storage.is_none() || app.tv_storage_device != id {
        app.tv_storage = None;
        app.refresh_tv_storage(&id, ctx);
    }
    if let Some((ref total, ref used, ref avail, percent)) = app.tv_storage {
        let color = if percent < 0.7 {
            theme::success_color()
        } else if percent < 0.9 {
            theme::warning_color()
        } else {
            theme::danger_color()
        };
        let text = format!("💾  {} / {} ({} libre)", used, total, avail);
        ui.horizontal(|ui| {
            ui.add(
                egui::ProgressBar::new(percent)
                    .text(egui::RichText::new(text).strong().size(12.0).color(egui::Color32::WHITE))
                    .fill(color)
                    .corner_radius(8.0),
            );
            let id_c = id.clone();
            if ui.small_button("🔄").clicked() {
                app.tv_storage = None;
                app.refresh_tv_storage(&id_c, ctx);
            }
        });
        ui.add_space(8.0);
    }

    // ======== Télécommande (Navigation + Média + Volume en une seule card) ========
    section(ui, app.dark_mode, |ui| {
        section_title(ui, "🎮 Télécommande");

        let dpad_size = egui::vec2(68.0, 44.0);
        let dpad_fill = theme::widget_bg(app.dark_mode);

        // Use columns: D-pad on left, nav/media/volume on right
        ui.columns(2, |cols| {
            // LEFT: D-Pad centered
            cols[0].vertical_centered(|ui| {
                egui::Grid::new("dpad_grid")
                    .spacing([3.0, 3.0])
                    .show(ui, |ui| {
                        ui.label("");
                        if ui.add_sized(dpad_size, egui::Button::new(egui::RichText::new("▲").size(18.0)).fill(dpad_fill).corner_radius(8.0)).clicked() {
                            app.tv_command(&id, "input keyevent KEYCODE_DPAD_UP");
                        }
                        ui.label("");
                        ui.end_row();
                        if ui.add_sized(dpad_size, egui::Button::new(egui::RichText::new("◀").size(18.0)).fill(dpad_fill).corner_radius(8.0)).clicked() {
                            app.tv_command(&id, "input keyevent KEYCODE_DPAD_LEFT");
                        }
                        if ui.add_sized(dpad_size, egui::Button::new(egui::RichText::new("OK").size(15.0).strong().color(egui::Color32::WHITE)).fill(theme::success_color()).corner_radius(8.0)).clicked() {
                            app.tv_command(&id, "input keyevent KEYCODE_DPAD_CENTER");
                        }
                        if ui.add_sized(dpad_size, egui::Button::new(egui::RichText::new("▶").size(18.0)).fill(dpad_fill).corner_radius(8.0)).clicked() {
                            app.tv_command(&id, "input keyevent KEYCODE_DPAD_RIGHT");
                        }
                        ui.end_row();
                        ui.label("");
                        if ui.add_sized(dpad_size, egui::Button::new(egui::RichText::new("▼").size(18.0)).fill(dpad_fill).corner_radius(8.0)).clicked() {
                            app.tv_command(&id, "input keyevent KEYCODE_DPAD_DOWN");
                        }
                        ui.label("");
                        ui.end_row();
                    });
            });

            // RIGHT: Nav + Media + Volume stacked
            cols[1].vertical(|ui| {
                // Nav
                ui.horizontal_wrapped(|ui| {
                    if tv_btn(ui, "🏠 Home", [82.0, 34.0]) { app.tv_command(&id, "input keyevent KEYCODE_HOME"); }
                    if tv_btn(ui, "⬅ Back", [82.0, 34.0]) { app.tv_command(&id, "input keyevent KEYCODE_BACK"); }
                    if tv_btn(ui, "☰ Menu", [82.0, 34.0]) { app.tv_command(&id, "input keyevent KEYCODE_MENU"); }
                });
                ui.add_space(6.0);
                // Media
                ui.horizontal_wrapped(|ui| {
                    for (l, c) in [("⏮", "KEYCODE_MEDIA_PREVIOUS"), ("⏪", "KEYCODE_MEDIA_REWIND"), ("⏯", "KEYCODE_MEDIA_PLAY_PAUSE"), ("⏩", "KEYCODE_MEDIA_FAST_FORWARD"), ("⏭", "KEYCODE_MEDIA_NEXT")] {
                        if tv_btn(ui, l, [48.0, 34.0]) { app.tv_command(&id, &format!("input keyevent {}", c)); }
                    }
                });
                ui.add_space(6.0);
                // Volume
                ui.horizontal_wrapped(|ui| {
                    if tv_btn(ui, "🔊+", [58.0, 32.0]) { app.tv_command(&id, "input keyevent KEYCODE_VOLUME_UP"); }
                    if tv_btn(ui, "🔉-", [58.0, 32.0]) { app.tv_command(&id, "input keyevent KEYCODE_VOLUME_DOWN"); }
                    if tv_btn(ui, "🔇", [50.0, 32.0]) { app.tv_command(&id, "input keyevent KEYCODE_VOLUME_MUTE"); }
                });
            });
        });
    });

    // ======== Saisie texte + Apps côte à côte ========
    ui.columns(2, |cols| {
        // LEFT: Text input
        section(&mut cols[0], app.dark_mode, |ui| {
            section_title(ui, "⌨ Saisie texte");
            ui.add(
                egui::TextEdit::singleline(&mut app.tv_text_input)
                    .hint_text("Texte à envoyer...")
                    .desired_width(ui.available_width()),
            );
            ui.add_space(4.0);
            if ui
                .add_enabled(
                    !app.tv_text_input.is_empty(),
                    egui::Button::new(egui::RichText::new("Envoyer").color(egui::Color32::WHITE))
                        .fill(theme::accent_color())
                        .corner_radius(8.0)
                        .min_size(egui::vec2(ui.available_width(), 30.0)),
                )
                .clicked()
            {
                let text = app.tv_text_input.clone();
                adb::send_text_to_device(&id, &text);
                app.log(&format!("Texte: {}", &text[..text.len().min(30)]));
                app.tv_text_input.clear();
            }
        });

        // RIGHT: Apps
        section(&mut cols[1], app.dark_mode, |ui| {
            section_title(ui, "📺 Applications");
            let apps: &[(&str, &str, [u8; 3], &str)] = &[
                ("youtube", "YouTube", [180, 20, 20], "am start -n com.google.android.youtube.tv/com.google.android.apps.youtube.tv.activity.ShellActivity"),
                ("netflix", "Netflix", [139, 0, 0], "am start -n com.netflix.ninja/.MainActivity"),
                ("plex", "Plex", [180, 160, 20], "am start -n com.plexapp.android/.activity.SplashActivity"),
                ("spotify", "Spotify", [30, 120, 40], "am start -n com.spotify.tv.android/.SpotifyTVActivity"),
                ("oqee", "Oqee", [40, 40, 120], "am start -a android.intent.action.MAIN -n net.oqee.androidtv.store/net.oqee.androidtv.ui.splash.SplashActivity"),
            ];

            ui.horizontal_wrapped(|ui| {
                for (icon_name, label, color, command) in apps {
                    let app_bg = egui::Color32::from_rgb(color[0], color[1], color[2]);
                    egui::Frame::NONE
                        .corner_radius(12.0)
                        .inner_margin(6.0)
                        .fill(app_bg)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(48.0);
                                let img_source = match *icon_name {
                                    "youtube" => egui::include_image!("../../assets/youtube.png"),
                                    "netflix" => egui::include_image!("../../assets/netflix.png"),
                                    "plex" => egui::include_image!("../../assets/plex.png"),
                                    "spotify" => egui::include_image!("../../assets/spotify.png"),
                                    _ => egui::include_image!("../../assets/oqee.png"),
                                };
                                let img = egui::Image::new(img_source).fit_to_exact_size(egui::vec2(36.0, 36.0));
                                if ui.add(egui::Button::image(img).corner_radius(8.0)).clicked() {
                                    app.tv_command(&id, command);
                                    app.log(label);
                                }
                                ui.label(egui::RichText::new(*label).size(10.0).strong().color(egui::Color32::WHITE));
                            });
                        });
                }
            });
        });
    });

    // ======== Chaînes TV ========
    section(ui, app.dark_mode, |ui| {
        ui.horizontal(|ui| {
            section_title(ui, "📡 Chaînes TV");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let edit_label = if app.channel_edit_mode { "✓ Terminé" } else { "✏ Éditer" };
                if ui.add(egui::Button::new(egui::RichText::new(edit_label).size(12.0)).corner_radius(6.0)).clicked() {
                    app.channel_edit_mode = !app.channel_edit_mode;
                }
            });
        });

        let mut channel_to_send: Option<u32> = None;
        let mut channel_to_delete: Option<usize> = None;
        let cols = 4;
        let btn_size = egui::vec2(120.0, 30.0);
        let ch_fill = theme::widget_bg(app.dark_mode);

        egui::Grid::new("channels_grid")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (i, ch) in app.tv_channels.iter().enumerate() {
                    let text = format!("{} · {}", ch.number, ch.name);
                    let btn = egui::Button::new(egui::RichText::new(&text).size(12.0).strong())
                        .fill(ch_fill)
                        .corner_radius(8.0);

                    if app.channel_edit_mode {
                        ui.horizontal(|ui| {
                            if ui.add_sized(btn_size, btn).clicked() { channel_to_send = Some(ch.number); }
                            if ui.add(egui::Button::new(egui::RichText::new("✕").color(theme::danger_color())).fill(egui::Color32::TRANSPARENT)).clicked() {
                                channel_to_delete = Some(i);
                            }
                        });
                    } else if ui.add_sized(btn_size, btn).clicked() {
                        channel_to_send = Some(ch.number);
                    }

                    if (i + 1) % cols == 0 { ui.end_row(); }
                }
            });

        if let Some(number) = channel_to_send {
            app.send_channel_number(&id, number);
            app.log(&format!("Chaîne {}", number));
        }
        if let Some(idx) = channel_to_delete {
            let removed = app.tv_channels.remove(idx);
            app.log(&format!("Chaîne {} supprimée", removed.name));
            config::save_channels(&app.tv_channels);
        }

        if app.channel_edit_mode {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("➕");
                ui.add(egui::TextEdit::singleline(&mut app.new_channel_number).hint_text("N°").desired_width(35.0));
                ui.add(egui::TextEdit::singleline(&mut app.new_channel_name).hint_text("Nom").desired_width(90.0));
                let can_add = !app.new_channel_name.is_empty() && app.new_channel_number.parse::<u32>().is_ok();
                if ui.add_enabled(can_add, egui::Button::new("Ajouter").fill(theme::success_color()).corner_radius(6.0)).clicked() {
                    if let Ok(num) = app.new_channel_number.parse::<u32>() {
                        let name = app.new_channel_name.clone();
                        app.log(&format!("Chaîne {} {} ajoutée", num, name));
                        app.tv_channels.push(crate::types::TvChannel { name, number: num });
                        app.tv_channels.sort_by_key(|c| c.number);
                        config::save_channels(&app.tv_channels);
                        app.new_channel_name.clear();
                        app.new_channel_number.clear();
                    }
                }
            });
        }
    });

    // ======== Replay + Power + Screenshot côte à côte ========
    ui.columns(2, |cols| {
        // LEFT: Replay OQEE
        section(&mut cols[0], app.dark_mode, |ui| {
            section_title(ui, "⏪ Replay OQEE");
            let mut replay_mins: Option<u32> = None;

            egui::Grid::new("replay_grid").spacing([4.0, 4.0]).show(ui, |ui| {
                for (label, mins) in [("30m", 30u32), ("1h", 60), ("1h30", 90), ("2h", 120)] {
                    if ui.add_sized([60.0, 30.0], egui::Button::new(format!("⏪ {}", label)).corner_radius(8.0)).clicked() {
                        replay_mins = Some(mins);
                    }
                }
                ui.end_row();
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.replay_custom_min).hint_text("min").desired_width(40.0));
                let valid = app.replay_custom_min.parse::<u32>().is_ok();
                if ui.add_enabled(valid, egui::Button::new("⏪ Go").fill(theme::accent_color()).corner_radius(6.0)).clicked() {
                    if let Ok(m) = app.replay_custom_min.parse::<u32>() { replay_mins = Some(m); }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("1s = {:.0} min", app.settings.replay_ratio)).size(11.0).color(egui::Color32::GRAY));
                if ui.small_button("-").clicked() { app.settings.replay_ratio = (app.settings.replay_ratio - 1.0).max(1.0); app.save_settings(); }
                if ui.small_button("+").clicked() { app.settings.replay_ratio += 1.0; app.save_settings(); }
            });

            if let Some(mins) = replay_mins {
                let hold_secs = ((mins as f32) / app.settings.replay_ratio).max(0.5);
                app.log(&format!("Replay -{}min ({:.1}s)", mins, hold_secs));
                let id_clone = id.clone();
                let tx = app.bg_tx.clone();
                std::thread::spawn(move || {
                    let shell_cmd = format!(
                        concat!(
                            "DEV=$(getevent -pl 2>&1 | awk '/^add device/{{dev=$NF}} /KEY_LEFT/{{print dev; exit}}' | tr -d ':'); ",
                            "if [ -n \"$DEV\" ]; then ",
                            "sendevent $DEV 1 105 1; sendevent $DEV 0 0 0; ",
                            "sleep {:.1}; ",
                            "sendevent $DEV 1 105 0; sendevent $DEV 0 0 0; ",
                            "sleep 0.5; input keyevent KEYCODE_DPAD_CENTER; fi"
                        ),
                        hold_secs
                    );
                    let _ = Command::new("adb").args(["-s", &id_clone, "shell", &shell_cmd]).output();
                    let _ = tx.send(BgEvent::Log("Replay terminé".into()));
                });
            }
        });

        // RIGHT: Power + Screenshot
        cols[1].vertical(|ui| {
            section(ui, app.dark_mode, |ui| {
                section_title(ui, "⚡ Alimentation");
                egui::Grid::new("power_grid").spacing([4.0, 4.0]).show(ui, |ui| {
                    if tv_btn(ui, "💤 Veille", [100.0, 34.0]) { app.tv_command(&id, "input keyevent KEYCODE_SLEEP"); app.log("MiBox en veille"); }
                    if tv_btn_fill(ui, "☀ Réveil", [100.0, 34.0], theme::success_color()) { app.tv_command(&id, "input keyevent KEYCODE_WAKEUP"); app.log("MiBox réveillée"); }
                    ui.end_row();
                    if tv_btn(ui, "⏻ Power", [100.0, 34.0]) { app.tv_command(&id, "input keyevent KEYCODE_POWER"); }
                    if tv_btn_fill(ui, "🔄 Reboot", [100.0, 34.0], theme::danger_color()) {
                        let id_clone = id.clone();
                        let tx = app.bg_tx.clone();
                        std::thread::spawn(move || {
                            let _ = Command::new("adb").args(["-s", &id_clone, "reboot"]).output();
                            let _ = tx.send(BgEvent::Log("MiBox redémarrage...".into()));
                        });
                        app.log("Reboot MiBox...");
                    }
                    ui.end_row();
                });
            });

            section(ui, app.dark_mode, |ui| {
                section_title(ui, "📸 Capture d'écran");
                let taking = app.tv_screenshot_loading;
                let btn_text = if taking { "⏳ Capture..." } else { "📸 Capturer" };
                if ui.add_enabled(!taking, egui::Button::new(btn_text).fill(theme::accent_color()).corner_radius(8.0).min_size(egui::vec2(ui.available_width(), 32.0))).clicked() {
                    app.tv_screenshot_loading = true;
                    let id_clone = id.clone();
                    let tx = app.bg_tx.clone();
                    std::thread::spawn(move || {
                        if let Some(data) = adb::take_screenshot(&id_clone) {
                            let _ = tx.send(BgEvent::ScreenshotReady { device_id: id_clone, data });
                        } else {
                            let _ = tx.send(BgEvent::Log("Échec capture".into()));
                        }
                    });
                }
                if app.tv_screenshot.is_some() {
                    ui.add_space(4.0);
                    let uri = "bytes://tv_screenshot.png";
                    ctx.include_bytes(uri, app.tv_screenshot.clone().unwrap());
                    ui.add(egui::Image::new(uri).max_width(ui.available_width()).corner_radius(6.0));
                }
            });
        });
    });
}
