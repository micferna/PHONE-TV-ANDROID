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
        .stroke(egui::Stroke::new(0.5, theme::card_border(app.dark_mode)))
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
        .stroke(egui::Stroke::new(0.5, theme::card_border(app.dark_mode)))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new("📤 Transfert fichier")
                    .strong()
                    .size(14.0),
            );
            ui.add_space(4.0);

            // Drag & drop support
            let dropped_file =
                ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()));
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

            // Drop zone when no file selected and not transferring
            let transfer_preview = app.transfer.lock().unwrap().clone();
            if app.file_path.is_empty() && !transfer_preview.active {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 80.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    egui::Stroke::new(2.0, theme::text_dim(app.dark_mode)),
                    egui::StrokeKind::Outside,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📂 Glissez un fichier ici",
                    egui::FontId::proportional(14.0),
                    theme::text_secondary(app.dark_mode),
                );
            }

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
                                egui::Button::new("▶ Envoyer+Lire").fill(theme::success_color()),
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

    ui.add_space(8.0);

    // === Retrieve from phone (pull) ===
    draw_retrieve(app, ui, ctx);
}

#[derive(Clone, Copy)]
enum PullKind {
    Selection,
    Whole,
}

/// Human-readable byte size (o / Ko / Mo / Go / To).
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["o", "Ko", "Mo", "Go", "To"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", v, UNITS[u])
    }
}

fn draw_retrieve(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let selected_id = app.get_selected_id();

    // Deferred actions, applied after the frame closure to avoid borrow conflicts.
    let mut list_path: Option<String> = None;
    let mut navigate_into: Option<String> = None;
    let mut pull_action: Option<PullKind> = None;
    let mut save_dest = false;

    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(12.0)
        .fill(theme::card_bg(app.dark_mode))
        .stroke(egui::Stroke::new(0.5, theme::card_border(app.dark_mode)))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new("📥 Récupérer du téléphone")
                    .strong()
                    .size(14.0),
            );
            ui.add_space(6.0);

            // ── Quick folder shortcuts ──────────────────────────────
            ui.horizontal_wrapped(|ui| {
                if ui.button("📱 /sdcard").clicked() {
                    list_path = Some("/sdcard/".to_string());
                }
                if ui.button("🎬 Movies").clicked() {
                    list_path = Some("/sdcard/Movies/".to_string());
                }
                if ui.button("📷 DCIM").clicked() {
                    list_path = Some("/sdcard/DCIM/".to_string());
                }
                if ui.button("⬇ Download").clicked() {
                    list_path = Some("/sdcard/Download/".to_string());
                }
            });

            ui.add_space(4.0);

            // ── Current path: editable field + open + refresh ───────
            ui.horizontal(|ui| {
                let path_width = (ui.available_width() - 130.0).max(100.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut app.pull_remote_path)
                        .hint_text("/sdcard/...")
                        .desired_width(path_width),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    list_path = Some(app.pull_remote_path.clone());
                }
                if ui.button("📂 Ouvrir").clicked() {
                    list_path = Some(app.pull_remote_path.clone());
                }
                if !app.pull_entries.is_empty()
                    && ui.button("↻").on_hover_text("Rafraîchir").clicked()
                {
                    list_path = Some(app.pull_remote_path.clone());
                }
            });

            // ── Clickable breadcrumb of the current location ────────
            if !app.pull_entries.is_empty() || app.pull_listing {
                ui.add_space(2.0);
                let path = PhoneTvApp::normalize_remote_dir(&app.pull_remote_path);
                let trimmed = path.trim_matches('/');
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if ui.small_button("📱").on_hover_text("/sdcard").clicked() {
                        list_path = Some("/sdcard/".to_string());
                    }
                    let mut acc = String::new();
                    for part in trimmed.split('/').filter(|p| !p.is_empty()) {
                        ui.label(
                            egui::RichText::new("/")
                                .color(theme::text_secondary(app.dark_mode)),
                        );
                        acc.push('/');
                        acc.push_str(part);
                        if ui.small_button(part).clicked() {
                            list_path = Some(format!("{}/", acc));
                        }
                    }
                });
            }

            ui.add_space(6.0);

            // ── Body: loading / listing / empty ─────────────────────
            if app.pull_listing {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Chargement du dossier...");
                });
                ctx.request_repaint();
            } else if !app.pull_entries.is_empty() {
                // Parent navigation
                if let Some(parent) = PhoneTvApp::remote_parent(&app.pull_remote_path) {
                    if ui
                        .button(egui::RichText::new("⬆ .. (dossier parent)").small())
                        .clicked()
                    {
                        list_path = Some(parent);
                    }
                }

                // Selection helpers + running totals
                ui.horizontal_wrapped(|ui| {
                    if ui.button("☑ Tout").clicked() {
                        for e in app.pull_entries.iter_mut() {
                            if !e.is_dir {
                                e.selected = true;
                            }
                        }
                    }
                    if ui.button("☐ Aucun").clicked() {
                        for e in app.pull_entries.iter_mut() {
                            e.selected = false;
                        }
                    }
                    let n = app.pull_entries.iter().filter(|e| e.selected).count();
                    let files = app.pull_entries.iter().filter(|e| !e.is_dir).count();
                    let sel_size: u64 = app
                        .pull_entries
                        .iter()
                        .filter(|e| e.selected)
                        .map(|e| e.size)
                        .sum();
                    ui.label(
                        egui::RichText::new(format!(
                            "{}/{} fichier(s) · {}",
                            n,
                            files,
                            human_size(sel_size)
                        ))
                        .small()
                        .color(theme::text_secondary(app.dark_mode)),
                    );
                });

                ui.add_space(4.0);

                // File / directory list with sizes
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for e in app.pull_entries.iter_mut() {
                            ui.horizontal(|ui| {
                                if e.is_dir {
                                    if ui
                                        .button(format!("📁 {}", e.name))
                                        .on_hover_text("Ouvrir ce dossier")
                                        .clicked()
                                    {
                                        navigate_into = Some(e.name.clone());
                                    }
                                } else {
                                    ui.checkbox(&mut e.selected, format!("📄 {}", e.name));
                                    ui.label(
                                        egui::RichText::new(human_size(e.size))
                                            .small()
                                            .color(theme::text_secondary(app.dark_mode)),
                                    );
                                }
                            });
                        }
                    });

                ui.add_space(8.0);

                // ── Destination on the PC (saved between sessions) ──
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("💾 Vers PC :").small());
                    let dest_width = (ui.available_width() - 50.0).max(100.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut app.pull_dest_dir)
                            .hint_text("dossier de destination...")
                            .desired_width(dest_width),
                    );
                    if ui.button("📂").on_hover_text("Choisir le dossier").clicked() {
                        if let Some(dir) = rfd::FileDialog::new()
                            .set_title("Dossier de destination sur le PC")
                            .pick_folder()
                        {
                            app.pull_dest_dir = dir.display().to_string();
                            save_dest = true;
                        }
                    }
                });

                ui.add_space(6.0);

                // ── Pull actions ────────────────────────────────────
                let any_selected = app.pull_entries.iter().any(|e| e.selected);
                let busy = app.file_transferring;
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(any_selected && !busy, |ui| {
                        if ui
                            .add_sized(
                                [190.0, 34.0],
                                egui::Button::new("📥 Récupérer la sélection")
                                    .fill(theme::success_color()),
                            )
                            .clicked()
                        {
                            pull_action = Some(PullKind::Selection);
                        }
                    });
                    ui.add_enabled_ui(!busy, |ui| {
                        if ui
                            .add_sized(
                                [160.0, 34.0],
                                egui::Button::new("📥 Tout le dossier").fill(theme::accent_color()),
                            )
                            .clicked()
                        {
                            pull_action = Some(PullKind::Whole);
                        }
                    });
                });

                // ── Progress ────────────────────────────────────────
                if let Some((done, total)) = app.pull_progress {
                    ui.add_space(4.0);
                    let frac = if total > 0 {
                        done as f32 / total as f32
                    } else {
                        0.0
                    };
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .text(format!("{}/{} fichier(s)", done, total))
                            .fill(theme::accent_color())
                            .animate(true),
                    );
                    ctx.request_repaint();
                }
            } else {
                // Empty state: one prominent button to open the phone storage.
                ui.add_space(2.0);
                if ui
                    .add_sized(
                        [220.0, 36.0],
                        egui::Button::new("📂 Parcourir le téléphone")
                            .fill(theme::accent_color()),
                    )
                    .clicked()
                {
                    list_path = Some("/sdcard/".to_string());
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Ouvre le stockage, navigue dans les dossiers, coche les fichiers à rapatrier.")
                        .small()
                        .color(theme::text_secondary(app.dark_mode)),
                );
            }
        });

    // Persist the destination folder when it was changed via the picker.
    if save_dest {
        app.save_settings();
    }

    // Resolve a directory click into a list request for the sub-folder.
    if let Some(name) = navigate_into {
        let base = PhoneTvApp::normalize_remote_dir(&app.pull_remote_path);
        list_path = Some(format!("{}{}", base, name));
    }

    // Apply a (re)list request.
    if let Some(path) = list_path {
        match selected_id.clone() {
            Some(id) => app.list_remote_async(id, path, ctx),
            None => app.log("⚠ Aucun appareil sélectionné"),
        }
    }

    // Apply a pull request straight to the saved PC folder (no dialog → quick).
    if let Some(kind) = pull_action {
        if let Some(id) = selected_id {
            let base = PhoneTvApp::normalize_remote_dir(&app.pull_remote_path);
            let remotes: Vec<String> = match kind {
                PullKind::Selection => app
                    .pull_entries
                    .iter()
                    .filter(|e| e.selected)
                    .map(|e| format!("{}{}", base, e.name))
                    .collect(),
                PullKind::Whole => {
                    vec![app.pull_remote_path.trim_end_matches('/').to_string()]
                }
            };

            if remotes.is_empty() {
                app.log("⚠ Rien à récupérer");
            } else {
                // Fall back to a folder picker only if no destination is set.
                let mut dest = app.pull_dest_dir.trim().to_string();
                if dest.is_empty() {
                    dest = rfd::FileDialog::new()
                        .set_title("Dossier de destination sur le PC")
                        .pick_folder()
                        .map(|d| d.display().to_string())
                        .unwrap_or_default();
                }
                if dest.is_empty() {
                    app.log("⚠ Aucun dossier de destination");
                } else {
                    let _ = std::fs::create_dir_all(&dest);
                    app.pull_dest_dir = dest.clone();
                    app.save_settings();
                    app.pull_files_async(id, remotes, dest, ctx);
                }
            }
        } else {
            app.log("⚠ Aucun appareil sélectionné");
        }
    }
}
