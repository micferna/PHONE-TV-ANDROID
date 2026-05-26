use eframe::egui;

use crate::app::PhoneTvApp;
use crate::llm;
use crate::theme;
use crate::types::BgEvent;

pub fn draw_audit(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let dark = app.dark_mode;

    ui.heading(egui::RichText::new("Audit & Nettoyage").size(20.0).strong());
    ui.add_space(12.0);

    // ── Device info + Config IA (collapsible) ────────────────────────
    let device_name = app
        .get_selected()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "Aucun appareil".to_string());

    let header_text = format!("Appareil: {}  |  Configuration IA", device_name);
    egui::CollapsingHeader::new(egui::RichText::new(header_text).size(14.0))
        .default_open(true)
        .show(ui, |ui| {
            egui::Frame::NONE
                .corner_radius(8.0)
                .inner_margin(12.0)
                .fill(theme::card_bg(dark))
                .stroke(egui::Stroke::new(0.5, theme::card_border(dark)))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // Device info
                    if let Some(device) = app.get_selected() {
                        ui.horizontal(|ui| {
                            let icon = match device.device_type {
                                crate::types::DeviceType::Phone => "📱",
                                crate::types::DeviceType::Tv => "📺",
                                crate::types::DeviceType::Unknown => "❓",
                            };
                            ui.label(
                                egui::RichText::new(format!("{} {}", icon, device.name))
                                    .size(14.0)
                                    .strong(),
                            );
                            let status_color = if device.status == "device" {
                                theme::success_color()
                            } else {
                                theme::danger_color()
                            };
                            ui.label(egui::RichText::new("●").color(status_color).size(12.0));
                            ui.label(
                                egui::RichText::new(&device.status)
                                    .size(12.0)
                                    .color(theme::text_secondary(dark)),
                            );
                        });
                        ui.add_space(8.0);
                    }

                    // API key
                    ui.label(egui::RichText::new("Cle API OpenRouter:").size(13.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings.openrouter_api_key)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(6.0);

                    // Model selector
                    ui.label(egui::RichText::new("Modele LLM:").size(13.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings.llm_model)
                            .desired_width(f32::INFINITY)
                            .hint_text("ex: anthropic/claude-sonnet-4"),
                    );
                    ui.add_space(4.0);

                    // Preset model buttons
                    let models = [
                        "anthropic/claude-sonnet-4",
                        "anthropic/claude-haiku-4",
                        "google/gemini-2.5-flash",
                        "google/gemini-2.5-pro",
                        "openai/gpt-4o",
                        "openai/gpt-4o-mini",
                        "meta-llama/llama-4-maverick",
                        "meta-llama/llama-4-scout",
                        "deepseek/deepseek-chat-v3",
                        "qwen/qwen3-235b-a22b",
                        "mistralai/mistral-medium",
                    ];
                    ui.horizontal_wrapped(|ui| {
                        for model in &models {
                            if ui
                                .selectable_label(app.settings.llm_model == *model, *model)
                                .clicked()
                            {
                                app.settings.llm_model = model.to_string();
                                app.llm_model_status = None;
                            }
                        }
                    });
                    ui.add_space(6.0);

                    // Validate + Save buttons
                    ui.horizontal(|ui| {
                        let can_test = !app.settings.openrouter_api_key.is_empty()
                            && !app.settings.llm_model.is_empty();
                        if ui
                            .add_enabled(
                                can_test,
                                egui::Button::new(
                                    egui::RichText::new("Tester le modele")
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(theme::accent_blue())
                                .corner_radius(6.0),
                            )
                            .clicked()
                        {
                            app.llm_model_status = None;
                            let api_key = app.settings.openrouter_api_key.clone();
                            let model = app.settings.llm_model.clone();
                            let tx = app.bg_tx.clone();
                            let ctx2 = ctx.clone();
                            std::thread::spawn(move || {
                                let result = llm::validate_model(&api_key, &model);
                                let (valid, error) = match result {
                                    Ok(_) => (true, None),
                                    Err(e) => (false, Some(e)),
                                };
                                let _ = tx.send(BgEvent::LlmModelValid {
                                    valid,
                                    model,
                                    error,
                                });
                                ctx2.request_repaint();
                            });
                        }

                        if ui.button(egui::RichText::new("Sauvegarder")).clicked() {
                            app.save_settings();
                        }
                    });

                    // Validation status
                    if let Some((valid, message)) = &app.llm_model_status {
                        ui.add_space(4.0);
                        if *valid {
                            ui.label(
                                egui::RichText::new(format!("✓ {}", message))
                                    .size(13.0)
                                    .color(theme::success_color()),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(format!("✗ {}", message))
                                    .size(13.0)
                                    .color(theme::danger_color()),
                            );
                        }
                    }
                });
        });

    ui.add_space(16.0);

    // ── Wizard launch ────────────────────────────────────────────────
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(16.0)
        .fill(theme::card_bg(dark))
        .stroke(egui::Stroke::new(0.5, theme::card_border(dark)))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Assistant de nettoyage")
                        .size(16.0)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Analyse complete: apps, securite, posture, IA")
                        .size(12.0)
                        .color(theme::text_secondary(dark)),
                );
                ui.add_space(12.0);

                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Lancer l'assistant de nettoyage complet")
                                .size(15.0)
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .min_size(egui::vec2(320.0, 44.0))
                        .fill(theme::accent_color())
                        .corner_radius(8.0),
                    )
                    .clicked()
                {
                    app.wizard.start();
                }
            });
        });

    ui.add_space(16.0);

    // ── Device history ───────────────────────────────────────────────
    if let Some(id) = app.get_selected_id() {
        if let Some(serial_raw) = crate::adb::adb_device(&id, &["shell", "getprop", "ro.serialno"])
        {
            let serial = serial_raw.trim().to_string();
            if let Some(history) = crate::history::load_history(&serial) {
                egui::Frame::NONE
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .fill(theme::card_bg(dark))
                    .stroke(egui::Stroke::new(0.5, theme::card_border(dark)))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("Historique").size(16.0).strong());
                        ui.add_space(4.0);
                        ui.label(format!("Appareil: {}", history.display_name));
                        ui.label(format!("Premiere connexion: {}", history.first_seen));

                        for session in history.sessions.iter().rev().take(5) {
                            ui.add_space(8.0);
                            egui::Frame::NONE
                                .corner_radius(6.0)
                                .inner_margin(10.0)
                                .fill(theme::widget_bg(dark))
                                .stroke(egui::Stroke::new(0.5, theme::card_border(dark)))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(egui::RichText::new(&session.date).strong());
                                    ui.label(format!(
                                        "Score: {} → {}",
                                        session.score_before, session.score_after
                                    ));
                                    ui.label(format!("Profil: {}", session.profile_used));
                                    ui.label(format!(
                                        "{} supprimees, {} desactivees, {} echecs",
                                        session.apps_removed.len(),
                                        session.apps_disabled.len(),
                                        session.apps_failed.len(),
                                    ));
                                });
                        }
                    });
            }

            // ── Backups ──────────────────────────────────────────────
            let sessions = crate::backup::list_sessions(&serial);
            if !sessions.is_empty() {
                ui.add_space(12.0);
                egui::Frame::NONE
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .fill(theme::card_bg(dark))
                    .stroke(egui::Stroke::new(0.5, theme::card_border(dark)))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("💾 Sauvegardes APK")
                                .size(16.0)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("Restaure une app supprimée par le wizard")
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(6.0);

                        for (dir, manifest) in sessions.iter().take(5) {
                            ui.add_space(6.0);
                            egui::Frame::NONE
                                .corner_radius(6.0)
                                .inner_margin(10.0)
                                .fill(theme::widget_bg(dark))
                                .stroke(egui::Stroke::new(0.5, theme::card_border(dark)))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} — {} APK",
                                            manifest.timestamp,
                                            manifest.apks.len()
                                        ))
                                        .strong(),
                                    );
                                    for apk in &manifest.apks {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(&apk.package)
                                                    .family(egui::FontFamily::Monospace)
                                                    .size(11.0),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui.small_button("↩ Restaurer").clicked() {
                                                        let path = std::path::PathBuf::from(dir)
                                                            .join(&apk.file);
                                                        let id_clone = id.clone();
                                                        let pkg = apk.package.clone();
                                                        let tx = app.bg_tx.clone();
                                                        std::thread::spawn(move || {
                                                            let local =
                                                                path.to_string_lossy().to_string();
                                                            let (success, message) =
                                                                crate::backup::restore_apk(
                                                                    &id_clone, &local,
                                                                );
                                                            let _ =
                                                                tx.send(BgEvent::AppActionResult {
                                                                    package: pkg,
                                                                    action: "restore".into(),
                                                                    success,
                                                                    message,
                                                                });
                                                        });
                                                    }
                                                },
                                            );
                                        });
                                    }
                                });
                        }
                    });
            }
        }
    }
}
