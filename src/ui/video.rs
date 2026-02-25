use eframe::egui;
use std::path::Path;
use std::sync::Arc;

use crate::adb;
use crate::app::PhoneTvApp;
use crate::theme;

pub fn draw_video(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.add_space(4.0);

    // === Stream URL ===
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(12.0)
        .fill(theme::card_bg(app.dark_mode))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("🔗 Lire une URL").strong().size(14.0));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let url_width = (ui.available_width() - 80.0).max(100.0);
                ui.add(
                    egui::TextEdit::singleline(&mut app.video_url)
                        .hint_text("https://... ou chemin local")
                        .desired_width(url_width),
                );

                if ui.button("▶ Lire").clicked() && !app.video_url.is_empty() {
                    if let Some(ref id) = app.get_selected_id() {
                        adb::play_video_url(id, &app.video_url);
                        app.log(&format!(
                            "Lecture: {}",
                            &app.video_url[..app.video_url.len().min(30)]
                        ));
                    }
                }
            });
        });

    ui.add_space(8.0);

    // === File transfer ===
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(12.0)
        .fill(theme::card_bg(app.dark_mode))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("📤 Transfert fichier").strong().size(14.0));
            ui.add_space(4.0);

            // Drag & drop support
            let dropped_file = ctx.input(|i| {
                i.raw.dropped_files
                    .first()
                    .and_then(|f| f.path.clone())
            });
            if let Some(path) = dropped_file {
                app.file_path = path.display().to_string();
                app.log(&format!("Fichier déposé: {}", app.file_path));
            }

            ui.horizontal(|ui| {
                let path_width = (ui.available_width() - 80.0).max(100.0);
                ui.add(
                    egui::TextEdit::singleline(&mut app.file_path)
                        .hint_text("Glissez un fichier ou parcourir...")
                        .desired_width(path_width),
                );

                if ui.button("📂").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Vidéos", &["mp4", "mkv", "avi", "mov", "webm"])
                        .add_filter("Tous", &["*"])
                        .pick_file()
                    {
                        app.file_path = path.display().to_string();
                    }
                }
            });

            ui.add_space(6.0);

            let transfer_state = app.transfer.lock().unwrap().clone();

            if transfer_state.active {
                let progress = if transfer_state.total_bytes > 0 {
                    transfer_state.transferred_bytes as f32 / transfer_state.total_bytes as f32
                } else {
                    0.0
                };

                ui.label(format!("📤 {}", transfer_state.filename));
                ui.add(
                    egui::ProgressBar::new(progress)
                        .text(format!(
                            "{:.0}% — {:.1} MB / {:.1} MB",
                            progress * 100.0,
                            transfer_state.transferred_bytes as f64 / 1_000_000.0,
                            transfer_state.total_bytes as f64 / 1_000_000.0
                        ))
                        .fill(theme::accent_color())
                        .animate(true),
                );

                if transfer_state.done {
                    ui.label(
                        egui::RichText::new("✓ Terminé!")
                            .color(theme::success_color())
                            .strong(),
                    );
                    if let Ok(mut t) = app.transfer.lock() {
                        t.active = false;
                        t.done = false;
                    }
                }

                ctx.request_repaint();
            } else {
                let file_ok = !app.file_path.is_empty() && Path::new(&app.file_path).exists();

                ui.horizontal(|ui| {
                    ui.add_enabled_ui(file_ok, |ui| {
                        if ui
                            .add_sized([140.0, 36.0], egui::Button::new("📤 Envoyer"))
                            .clicked()
                        {
                            if let Some(ref id) = app.get_selected_id() {
                                app.log("Transfert...");
                                let path = app.file_path.clone();
                                adb::start_transfer(id, &path, Arc::clone(&app.transfer), false);
                            }
                        }

                        if ui
                            .add_sized(
                                [140.0, 36.0],
                                egui::Button::new("▶ Envoyer+Lire")
                                    .fill(theme::success_color()),
                            )
                            .clicked()
                        {
                            if let Some(ref id) = app.get_selected_id() {
                                app.log("Envoi + lecture...");
                                let path = app.file_path.clone();
                                adb::start_transfer(id, &path, Arc::clone(&app.transfer), true);
                            }
                        }
                    });
                });

                if !file_ok && !app.file_path.is_empty() {
                    ui.label(
                        egui::RichText::new("⚠ Fichier introuvable")
                            .color(theme::danger_color())
                            .small(),
                    );
                }
            }
        });
}
