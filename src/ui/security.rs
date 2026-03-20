use eframe::egui;
use std::sync::atomic::Ordering;

use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::*;

// ── Section helper (matches phone.rs pattern) ──────────────────────
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

// ── Card frame ─────────────────────────────────────────────────────
fn card_frame(dark_mode: bool) -> egui::Frame {
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(12.0)
        .fill(theme::card_bg(dark_mode))
        .stroke(egui::Stroke::new(0.5, theme::card_border(dark_mode)))
}

// ── Tinted card frame for issues ───────────────────────────────────
fn tinted_card_frame(dark_mode: bool, severity: &Severity) -> egui::Frame {
    let base = theme::card_bg(dark_mode);
    let tint = match severity {
        Severity::Critical => {
            if dark_mode {
                egui::Color32::from_rgb(
                    base.r().saturating_add(30),
                    base.g(),
                    base.b(),
                )
            } else {
                egui::Color32::from_rgb(255, 235, 235)
            }
        }
        Severity::Warning => {
            if dark_mode {
                egui::Color32::from_rgb(
                    base.r().saturating_add(20),
                    base.g().saturating_add(15),
                    base.b(),
                )
            } else {
                egui::Color32::from_rgb(255, 248, 230)
            }
        }
        Severity::Info => theme::card_bg(dark_mode),
    };
    let border = match severity {
        Severity::Critical => theme::danger_color(),
        Severity::Warning => theme::warning_color(),
        Severity::Info => theme::card_border(dark_mode),
    };
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(12.0)
        .fill(tint)
        .stroke(egui::Stroke::new(1.0, border))
}

// ── Color helpers ──────────────────────────────────────────────────
fn score_color(score: u8) -> egui::Color32 {
    if score >= 80 {
        theme::success_color()
    } else if score >= 50 {
        theme::warning_color()
    } else {
        theme::danger_color()
    }
}

fn severity_color(severity: &Severity) -> egui::Color32 {
    match severity {
        Severity::Critical => theme::danger_color(),
        Severity::Warning => theme::warning_color(),
        Severity::Info => theme::accent_purple(),
    }
}

// ── Badge helper ───────────────────────────────────────────────────
fn badge(ui: &mut egui::Ui, text: &str, bg: egui::Color32) {
    let text_widget = egui::RichText::new(text)
        .small()
        .strong()
        .color(egui::Color32::WHITE);
    egui::Frame::NONE
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .fill(bg)
        .show(ui, |ui| {
            ui.label(text_widget);
        });
}

// ── Arc gauge ──────────────────────────────────────────────────────
fn draw_arc_gauge(ui: &mut egui::Ui, score: u8, color: egui::Color32, dark_mode: bool) {
    let desired_size = egui::vec2(200.0, 120.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter();
    let center = egui::pos2(rect.center().x, rect.max.y - 10.0);
    let radius = 80.0;
    let stroke_width = 12.0;

    // Background arc (180 degrees, from left to right)
    let bg_color = if dark_mode {
        egui::Color32::from_rgb(48, 54, 61)
    } else {
        egui::Color32::from_rgb(208, 215, 222)
    };
    let segments = 60;
    for i in 0..segments {
        let angle1 = std::f32::consts::PI + (i as f32 / segments as f32) * std::f32::consts::PI;
        let angle2 =
            std::f32::consts::PI + ((i + 1) as f32 / segments as f32) * std::f32::consts::PI;
        let p1 = center + egui::vec2(angle1.cos() * radius, angle1.sin() * radius);
        let p2 = center + egui::vec2(angle2.cos() * radius, angle2.sin() * radius);
        painter.line_segment([p1, p2], egui::Stroke::new(stroke_width, bg_color));
    }

    // Filled arc proportional to score
    let filled_segments = (segments as f32 * score as f32 / 100.0) as usize;
    for i in 0..filled_segments {
        let angle1 = std::f32::consts::PI + (i as f32 / segments as f32) * std::f32::consts::PI;
        let angle2 =
            std::f32::consts::PI + ((i + 1) as f32 / segments as f32) * std::f32::consts::PI;
        let p1 = center + egui::vec2(angle1.cos() * radius, angle1.sin() * radius);
        let p2 = center + egui::vec2(angle2.cos() * radius, angle2.sin() * radius);
        painter.line_segment([p1, p2], egui::Stroke::new(stroke_width, color));
    }

    // Score text in center
    painter.text(
        center + egui::vec2(0.0, -20.0),
        egui::Align2::CENTER_CENTER,
        format!("{}", score),
        egui::FontId::proportional(42.0),
        color,
    );
    painter.text(
        center + egui::vec2(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "/ 100",
        egui::FontId::proportional(14.0),
        if dark_mode {
            egui::Color32::from_rgb(139, 148, 158)
        } else {
            egui::Color32::from_rgb(101, 109, 118)
        },
    );
}

// ── Trigger helpers ────────────────────────────────────────────────
fn trigger_score_load(app: &mut PhoneTvApp, ctx: &egui::Context) {
    if let Some(id) = app.get_selected_id() {
        app.security_score_loading = true;
        let tx = app.bg_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let (score, issues) = crate::security::score::calculate_score(&id);
            let _ = tx.send(BgEvent::SecurityScore { score, issues });
            ctx.request_repaint();
        });
    }
}

fn trigger_apps_load(app: &mut PhoneTvApp, ctx: &egui::Context) {
    if let Some(id) = app.get_selected_id() {
        app.security_apps_loading = true;
        app.security_loading_cancel.store(false, Ordering::Relaxed);
        let tx = app.bg_tx.clone();
        let ctx2 = ctx.clone();
        let filter = app.security_apps_filter;
        let cancel = app.security_loading_cancel.clone();
        std::thread::spawn(move || {
            let packages = crate::security::apps::list_packages(&id, filter);
            let _ = tx.send(BgEvent::SecurityAppsList {
                packages: packages.clone(),
            });
            ctx2.request_repaint();

            for pkg in &packages {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                if let Some(info) = crate::security::apps::get_app_detail(&id, pkg) {
                    let _ = tx.send(BgEvent::SecurityAppDetail {
                        package: pkg.clone(),
                        info,
                    });
                    ctx2.request_repaint();
                }
            }

            let _ = tx.send(BgEvent::SecurityAppsLoadingDone);
            ctx2.request_repaint();
        });
    }
}

fn trigger_posture_load(app: &mut PhoneTvApp, ctx: &egui::Context, device_id: &str) {
    if app.security_posture_loading { return; }
    app.security_posture_loading = true;
    let tx = app.bg_tx.clone();
    let ctx2 = ctx.clone();
    let id = device_id.to_string();
    std::thread::spawn(move || {
        let checks = crate::security::posture::check_device_posture(&id);
        let _ = tx.send(BgEvent::SecurityPosture { checks });
        ctx2.request_repaint();
    });
}

fn trigger_processes_load(app: &mut PhoneTvApp, ctx: &egui::Context, device_id: &str) {
    if app.security_processes_loading { return; }
    app.security_processes_loading = true;
    let tx = app.bg_tx.clone();
    let ctx2 = ctx.clone();
    let id = device_id.to_string();
    std::thread::spawn(move || {
        let processes = crate::security::monitoring::get_running_processes(&id);
        let _ = tx.send(BgEvent::SecurityProcesses { processes });
        ctx2.request_repaint();
    });
}

fn trigger_permissions_load(app: &mut PhoneTvApp, ctx: &egui::Context, device_id: &str) {
    if app.security_permissions_loading { return; }
    app.security_permissions_loading = true;
    let apps: Vec<String> = app
        .security_apps
        .iter()
        .map(|a| a.package.clone())
        .collect();
    let tx = app.bg_tx.clone();
    let ctx2 = ctx.clone();
    let id = device_id.to_string();
    std::thread::spawn(move || {
        for pkg in &apps {
            let perms = crate::security::permissions::get_app_permissions(&id, pkg);
            let _ = tx.send(BgEvent::SecurityPermissions {
                package: pkg.clone(),
                permissions: perms,
            });
            ctx2.request_repaint();
        }
        // Signal done — clear loading flag via a Log event
        let _ = tx.send(BgEvent::Log("Permissions chargées".into()));
        ctx2.request_repaint();
    });
}

// ── Danger score for app sorting ───────────────────────────────────
fn app_danger_score(info: &AppInfo) -> u32 {
    let mut score = 0u32;
    // Sideloaded = highest danger
    if info.installer == AppInstaller::Sideload {
        score += 10000;
    }
    if info.installer == AppInstaller::Adb {
        score += 5000;
    }
    if info.installer == AppInstaller::Unknown {
        score += 3000;
    }
    // Very old SDK
    if info.target_sdk > 0 && info.target_sdk < 23 {
        score += 2000;
    } else if info.target_sdk > 0 && info.target_sdk < 28 {
        score += 1000;
    }
    // Disabled apps are less concerning
    if !info.enabled {
        score = score.saturating_sub(500);
    }
    score
}

// ═══════════════════════════════════════════════════════════════════
// MAIN ENTRY POINT
// ═══════════════════════════════════════════════════════════════════
pub fn draw_security(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.add_space(4.0);

    let dark = app.dark_mode;
    let device_selected = app.get_selected_id().is_some();

    // ── Auto-load on entering Security tab (once per device) ───────
    if device_selected {
        let current_device = app.get_selected_id();
        let device_changed = app.security_auto_loaded_device.as_ref() != current_device.as_ref();

        if device_changed {
            // Reset everything for the new device
            app.security_auto_loaded_device = current_device.clone();
            app.security_score = None;
            app.security_apps.clear();
            app.security_posture.clear();
            app.security_permission_cache.clear();
            app.security_processes.clear();
            app.security_data_usage.clear();
            app.security_wakelocks.clear();
            app.security_posture_loading = false;
            app.security_processes_loading = false;
            app.security_permissions_loading = false;
            app.security_data_usage_loading = false;
            app.security_wakelocks_loading = false;
            // Trigger initial loads
            trigger_apps_load(app, ctx);
            trigger_score_load(app, ctx);
            if let Some(id) = current_device {
                trigger_posture_load(app, ctx, &id);
            }
        } else if app.security_apps.is_empty() && !app.security_apps_loading {
            // First time entering tab — no device change but nothing loaded yet
            trigger_apps_load(app, ctx);
            if app.security_score.is_none() && !app.security_score_loading {
                trigger_score_load(app, ctx);
            }
            if app.security_posture.is_empty() && !app.security_posture_loading {
                if let Some(id) = app.get_selected_id() {
                    trigger_posture_load(app, ctx, &id);
                }
            }
        }
    }

    // ── Sub-tab navigation bar with icons ──────────────────────────
    section(ui, dark, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            let score_value = app.security_score.as_ref().map(|(s, _)| *s);
            let has_blacklist_alerts = !app.blacklist_alerts.is_empty();

            let tabs: &[(SecurityView, &str, Option<egui::Color32>)] = &[
                (
                    SecurityView::Score,
                    "\u{1f6e1} Score",
                    score_value.and_then(|s| if s < 80 { Some(theme::danger_color()) } else { None }),
                ),
                (SecurityView::Apps, "\u{1f4e6} Apps", None),
                (SecurityView::Permissions, "\u{1f510} Permissions", None),
                (
                    SecurityView::Blacklist,
                    "\u{1f6ab} Blacklist",
                    if has_blacklist_alerts { Some(theme::danger_color()) } else { None },
                ),
                (SecurityView::Monitoring, "\u{1f4ca} Monitoring", None),
                (SecurityView::Posture, "\u{2699} Posture", None),
            ];

            for (view, label, alert_color) in tabs {
                let selected = app.security_view == *view;
                let mut text = egui::RichText::new(*label).size(13.0);
                text = if selected {
                    text.strong().color(theme::accent_color())
                } else {
                    text.color(theme::text_secondary(dark))
                };
                let btn = egui::Button::new(text)
                    .corner_radius(6.0)
                    .min_size(egui::vec2(0.0, 32.0));
                let btn = if selected {
                    btn.fill(theme::card_selected(dark))
                } else {
                    btn.fill(egui::Color32::TRANSPARENT)
                };
                let response = ui.add(btn);
                // Draw alert dot
                if let Some(dot_color) = alert_color {
                    let dot_pos = response.rect.right_top() + egui::vec2(-4.0, 4.0);
                    ui.painter().circle_filled(dot_pos, 4.0, *dot_color);
                }
                if response.clicked() {
                    app.security_view = *view;
                }
            }
        });
    });

    // Dispatch to sub-views
    match app.security_view {
        SecurityView::Score => draw_score(ui, app, ctx),
        SecurityView::Apps => draw_apps(ui, app, ctx),
        SecurityView::Permissions => draw_permissions(ui, app, ctx),
        SecurityView::Blacklist => draw_blacklist(ui, app, ctx),
        SecurityView::Monitoring => draw_monitoring(ui, app, ctx),
        SecurityView::Posture => draw_posture(ui, app, ctx),
    }
}

// ═══════════════════════════════════════════════════════════════════
// SCORE VIEW
// ═══════════════════════════════════════════════════════════════════
fn draw_score(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let dark = app.dark_mode;

    if app.get_selected_id().is_none() {
        ui.label(
            egui::RichText::new("Sélectionnez un appareil pour voir le score de sécurité.")
                .color(theme::text_secondary(dark)),
        );
        return;
    }

    // Auto-load
    if app.security_score.is_none() && !app.security_score_loading {
        trigger_score_load(app, ctx);
    }

    // Header with refresh
    ui.horizontal(|ui| {
        section_title(ui, "\u{1f6e1} Score de sécurité");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let refresh_enabled = !app.security_score_loading;
            if ui
                .add_enabled(
                    refresh_enabled,
                    egui::Button::new(
                        egui::RichText::new("\u{1f504} Rafraîchir")
                            .color(egui::Color32::WHITE),
                    )
                    .fill(theme::accent_blue())
                    .corner_radius(6.0),
                )
                .clicked()
            {
                trigger_score_load(app, ctx);
            }
        });
    });

    ui.add_space(4.0);

    if app.security_score_loading {
        section(ui, dark, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.spinner();
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Calcul du score en cours...")
                        .size(14.0)
                        .color(theme::text_secondary(dark)),
                );
                ui.add_space(20.0);
            });
        });
        return;
    }

    if let Some((score, issues)) = app.security_score.clone() {
        // Score arc gauge card
        section(ui, dark, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                let color = score_color(score);
                draw_arc_gauge(ui, score, color, dark);

                // Qualitative label
                let label_text = if score >= 90 {
                    "Excellent"
                } else if score >= 80 {
                    "Bon"
                } else if score >= 50 {
                    "Moyen"
                } else {
                    "Critique"
                };
                ui.label(
                    egui::RichText::new(label_text)
                        .size(16.0)
                        .strong()
                        .color(color),
                );
                ui.add_space(8.0);
            });
        });

        // Summary bar
        let critical_count = issues
            .iter()
            .filter(|i| i.severity == Severity::Critical)
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
        let info_count = issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .count();

        if issues.is_empty() {
            section(ui, dark, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("\u{2705} Aucun problème détecté !")
                            .size(16.0)
                            .strong()
                            .color(theme::success_color()),
                    );
                });
            });
        } else {
            // Summary badges
            section(ui, dark, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} problème(s) détecté(s)", issues.len()))
                            .size(14.0)
                            .strong(),
                    );
                    ui.add_space(12.0);
                    if critical_count > 0 {
                        badge(
                            ui,
                            &format!("{} critique(s)", critical_count),
                            theme::danger_color(),
                        );
                    }
                    if warning_count > 0 {
                        badge(
                            ui,
                            &format!("{} avertissement(s)", warning_count),
                            theme::warning_color(),
                        );
                    }
                    if info_count > 0 {
                        badge(
                            ui,
                            &format!("{} info(s)", info_count),
                            theme::accent_purple(),
                        );
                    }
                });
            });

            ui.add_space(4.0);

            // Issues list
            for issue in &issues {
                tinted_card_frame(dark, &issue.severity).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        // Top line: severity + description
                        ui.horizontal(|ui| {
                            let sev_label = match issue.severity {
                                Severity::Critical => "\u{1f534} CRITIQUE",
                                Severity::Warning => "\u{1f7e0} ATTENTION",
                                Severity::Info => "\u{1f535} INFO",
                            };
                            ui.label(
                                egui::RichText::new(sev_label)
                                    .strong()
                                    .color(severity_color(&issue.severity)),
                            );
                            ui.label(
                                egui::RichText::new(format!("[{}]", issue.id))
                                    .small()
                                    .color(theme::text_secondary(dark)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{} pts", issue.points))
                                            .small()
                                            .color(theme::text_secondary(dark)),
                                    );
                                },
                            );
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(&issue.description).size(13.0),
                        );

                        // Fix button (prominent)
                        if let Some(ref cmd) = issue.fix_command {
                            ui.add_space(8.0);
                            if let Some(dev) = app.get_selected_id() {
                                let cmd = cmd.clone();
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("\u{1f527} Corriger")
                                                .size(14.0)
                                                .strong()
                                                .color(egui::Color32::WHITE),
                                        )
                                        .fill(theme::success_color())
                                        .corner_radius(6.0)
                                        .min_size(egui::vec2(140.0, 32.0)),
                                    )
                                    .clicked()
                                {
                                    let tx = app.bg_tx.clone();
                                    let ctx2 = ctx.clone();
                                    std::thread::spawn(move || {
                                        let _ = std::process::Command::new("adb")
                                            .args(["-s", &dev, "shell", &cmd])
                                            .output();
                                        let _ = tx.send(BgEvent::AppActionResult {
                                            package: "security".into(),
                                            action: "fix".into(),
                                            success: true,
                                            message: "Correction appliquée".into(),
                                        });
                                        // Auto-refresh score after fix
                                        let (score, issues) =
                                            crate::security::score::calculate_score(&dev);
                                        let _ = tx.send(BgEvent::SecurityScore { score, issues });
                                        ctx2.request_repaint();
                                    });
                                }
                            }
                        }
                    });
                });
                ui.add_space(4.0);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// APPS VIEW
// ═══════════════════════════════════════════════════════════════════
fn draw_apps(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let dark = app.dark_mode;
    let device_id = match app.get_selected_id() {
        Some(id) => id,
        None => {
            ui.label(
                egui::RichText::new("Sélectionnez un appareil.")
                    .color(theme::text_secondary(dark)),
            );
            return;
        }
    };

    // Auto-load on first visit
    if app.security_apps.is_empty() && !app.security_apps_loading {
        trigger_apps_load(app, ctx);
    }

    // ── Loading progress bar ───────────────────────────────────────
    if app.security_apps_loading {
        let total = app.security_apps.len();
        let loaded = app.security_apps_loaded_count;
        section(ui, dark, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    if total > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "Chargement des applications : {} / {}",
                                loaded, total
                            ))
                            .size(14.0)
                            .color(theme::text_secondary(dark)),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Récupération de la liste des applications...")
                                .size(14.0)
                                .color(theme::text_secondary(dark)),
                        );
                    }
                });
                if total > 0 {
                    ui.add_space(4.0);
                    let progress = loaded as f32 / total as f32;
                    ui.add(
                        egui::ProgressBar::new(progress)
                            .fill(theme::accent_blue())
                            .desired_width(ui.available_width())
                            .corner_radius(4.0),
                    );
                }
                ui.add_space(4.0);
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Annuler").color(theme::danger_color()),
                        )
                        .corner_radius(6.0),
                    )
                    .clicked()
                {
                    app.security_loading_cancel.store(true, Ordering::Relaxed);
                }
                ui.add_space(4.0);
            });
        });
    }

    // ── Filter bar ─────────────────────────────────────────────────
    section(ui, dark, |ui| {
        ui.horizontal_wrapped(|ui| {
            // Filter buttons
            let filters = [
                (AppFilter::All, "Toutes"),
                (AppFilter::ThirdParty, "Tierces"),
                (AppFilter::System, "Système"),
                (AppFilter::Disabled, "Désactivées"),
            ];
            for (filter, label) in filters {
                let selected = app.security_apps_filter == filter;
                let mut text = egui::RichText::new(label).size(12.0);
                text = if selected {
                    text.strong().color(theme::accent_color())
                } else {
                    text.color(theme::text_secondary(dark))
                };
                let btn = egui::Button::new(text).corner_radius(4.0);
                let btn = if selected {
                    btn.fill(theme::card_selected(dark))
                } else {
                    btn.fill(egui::Color32::TRANSPARENT)
                };
                if ui.add(btn).clicked() {
                    app.security_apps_filter = filter;
                }
            }

            ui.separator();

            // Sort combo
            ui.label(
                egui::RichText::new("Tri :").size(12.0).color(theme::text_secondary(dark)),
            );
            egui::ComboBox::from_id_salt("app_sort")
                .selected_text(match app.security_apps_sort {
                    AppSort::Danger => "\u{26a0} Danger",
                    AppSort::Name => "Nom",
                    AppSort::InstallDate => "Date",
                    AppSort::Source => "Source",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.security_apps_sort,
                        AppSort::Danger,
                        "\u{26a0} Danger",
                    );
                    ui.selectable_value(&mut app.security_apps_sort, AppSort::Name, "Nom");
                    ui.selectable_value(
                        &mut app.security_apps_sort,
                        AppSort::InstallDate,
                        "Date d'installation",
                    );
                    ui.selectable_value(&mut app.security_apps_sort, AppSort::Source, "Source");
                });

            ui.separator();

            // Search
            ui.add(
                egui::TextEdit::singleline(&mut app.security_apps_search)
                    .desired_width(180.0)
                    .hint_text("\u{1f50d} Rechercher..."),
            );

            ui.separator();

            // Refresh button
            let load_enabled = !app.security_apps_loading;
            if ui
                .add_enabled(
                    load_enabled,
                    egui::Button::new(
                        egui::RichText::new("\u{1f504} Recharger")
                            .color(egui::Color32::WHITE),
                    )
                    .fill(theme::accent_blue())
                    .corner_radius(6.0),
                )
                .clicked()
            {
                trigger_apps_load(app, ctx);
            }
        });
    });

    // Confirmation dialogs
    draw_confirm_dialogs(app, ctx, &device_id);

    // Filter and sort display list
    let search_lower = app.security_apps_search.to_lowercase();
    let mut display_apps: Vec<AppInfo> = app
        .security_apps
        .iter()
        .filter(|a| {
            if search_lower.is_empty() {
                true
            } else {
                a.package.to_lowercase().contains(&search_lower)
            }
        })
        .cloned()
        .collect();

    match app.security_apps_sort {
        AppSort::Danger => display_apps.sort_by(|a, b| {
            app_danger_score(b).cmp(&app_danger_score(a))
        }),
        AppSort::Name => display_apps.sort_by(|a, b| a.package.cmp(&b.package)),
        AppSort::InstallDate => {
            display_apps.sort_by(|a, b| b.first_install.cmp(&a.first_install))
        }
        AppSort::Source => display_apps.sort_by(|a, b| {
            format!("{:?}", a.installer).cmp(&format!("{:?}", b.installer))
        }),
    }

    // Count summary
    let sideloaded = display_apps
        .iter()
        .filter(|a| a.installer == AppInstaller::Sideload)
        .count();
    let old_sdk = display_apps
        .iter()
        .filter(|a| a.target_sdk > 0 && a.target_sdk < 28)
        .count();

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!("{} application(s)", display_apps.len()))
                .size(13.0)
                .color(theme::text_secondary(dark)),
        );
        if sideloaded > 0 {
            ui.add_space(8.0);
            badge(
                ui,
                &format!("{} sideload", sideloaded),
                theme::danger_color(),
            );
        }
        if old_sdk > 0 {
            ui.add_space(4.0);
            badge(
                ui,
                &format!("{} SDK ancien", old_sdk),
                theme::warning_color(),
            );
        }
    });
    ui.add_space(4.0);

    // App list
    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for app_info in &display_apps {
                draw_app_card(ui, app, ctx, app_info, &device_id);
                ui.add_space(4.0);
            }
        });
}

fn draw_app_card(
    ui: &mut egui::Ui,
    app: &mut PhoneTvApp,
    ctx: &egui::Context,
    info: &AppInfo,
    device_id: &str,
) {
    let dark = app.dark_mode;
    card_frame(dark).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            // Left: info
            ui.vertical(|ui| {
                ui.set_max_width(ui.available_width() - 280.0);

                // Package name (truncated)
                let display_name = if info.package.len() > 45 {
                    format!("{}...", &info.package[..42])
                } else {
                    info.package.clone()
                };
                ui.label(egui::RichText::new(display_name).strong().size(13.0));

                if info.details_loaded {
                    // Version + date on same line
                    ui.horizontal_wrapped(|ui| {
                        if !info.version_name.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("v{}", info.version_name))
                                    .small()
                                    .color(theme::text_secondary(dark)),
                            );
                        }
                        if !info.first_install.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("| {}", info.first_install))
                                    .small()
                                    .color(theme::text_secondary(dark)),
                            );
                        }
                    });

                    // Badges line
                    ui.horizontal_wrapped(|ui| {
                        // Source badge
                        match info.installer {
                            AppInstaller::PlayStore => {
                                badge(ui, "Play Store", theme::success_color());
                            }
                            AppInstaller::Sideload => {
                                badge(ui, "SIDELOAD", theme::danger_color());
                            }
                            AppInstaller::Adb => {
                                badge(ui, "ADB", theme::accent_blue());
                            }
                            AppInstaller::Unknown => {
                                badge(ui, "Inconnu", theme::text_secondary(dark));
                            }
                        }

                        // SDK badges
                        if info.target_sdk > 0 && info.target_sdk < 23 {
                            badge(
                                ui,
                                &format!("SDK TRÈS ANCIEN ({})", info.target_sdk),
                                theme::danger_color(),
                            );
                        } else if info.target_sdk > 0 && info.target_sdk < 28 {
                            badge(
                                ui,
                                &format!("SDK ANCIEN ({})", info.target_sdk),
                                theme::warning_color(),
                            );
                        } else if info.target_sdk > 0 {
                            ui.label(
                                egui::RichText::new(format!("SDK {}", info.target_sdk))
                                    .small()
                                    .color(theme::text_secondary(dark)),
                            );
                        }

                        if !info.enabled {
                            badge(ui, "DÉSACTIVÉ", theme::danger_color());
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Chargement...")
                                .small()
                                .italics()
                                .color(theme::text_dim(dark)),
                        );
                    });
                }
            });

            // Right: action buttons
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pkg = info.package.clone();
                let dev = device_id.to_string();

                // Uninstall
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Désinstaller")
                                .small()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(theme::danger_color())
                        .corner_radius(4.0),
                    )
                    .clicked()
                {
                    app.confirm_uninstall = Some(pkg.clone());
                }

                // Clear data
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Effacer données").small(),
                        )
                        .corner_radius(4.0),
                    )
                    .clicked()
                {
                    app.confirm_clear_data = Some(pkg.clone());
                }

                // Force stop
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Forcer arrêt").small(),
                        )
                        .corner_radius(4.0),
                    )
                    .clicked()
                {
                    let tx = app.bg_tx.clone();
                    let ctx2 = ctx.clone();
                    let pkg2 = pkg.clone();
                    let dev2 = dev.clone();
                    std::thread::spawn(move || {
                        crate::security::apps::force_stop_app(&dev2, &pkg2);
                        let _ = tx.send(BgEvent::AppActionResult {
                            package: pkg2,
                            action: "force-stop".into(),
                            success: true,
                            message: "OK".into(),
                        });
                        ctx2.request_repaint();
                    });
                }

                // Enable/Disable
                if info.enabled {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Désactiver").small(),
                            )
                            .corner_radius(4.0),
                        )
                        .clicked()
                    {
                        let tx = app.bg_tx.clone();
                        let ctx2 = ctx.clone();
                        let pkg2 = pkg.clone();
                        let dev2 = dev.clone();
                        std::thread::spawn(move || {
                            let (success, message) =
                                crate::security::apps::disable_app(&dev2, &pkg2);
                            let _ = tx.send(BgEvent::AppActionResult {
                                package: pkg2,
                                action: "disable".into(),
                                success,
                                message,
                            });
                            ctx2.request_repaint();
                        });
                    }
                } else if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Activer")
                                .small()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(theme::success_color())
                        .corner_radius(4.0),
                    )
                    .clicked()
                {
                    let tx = app.bg_tx.clone();
                    let ctx2 = ctx.clone();
                    let pkg2 = pkg.clone();
                    let dev2 = dev.clone();
                    std::thread::spawn(move || {
                        let (success, message) =
                            crate::security::apps::enable_app(&dev2, &pkg2);
                        let _ = tx.send(BgEvent::AppActionResult {
                            package: pkg2,
                            action: "enable".into(),
                            success,
                            message,
                        });
                        ctx2.request_repaint();
                    });
                }
            });
        });
    });
}

fn draw_confirm_dialogs(app: &mut PhoneTvApp, ctx: &egui::Context, device_id: &str) {
    // Clear data confirmation
    if let Some(pkg) = app.confirm_clear_data.clone() {
        let mut open = true;
        egui::Window::new("Confirmer effacement")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Effacer toutes les données de {} ?",
                    pkg
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Confirmer")
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(theme::danger_color())
                            .corner_radius(6.0),
                        )
                        .clicked()
                    {
                        let tx = app.bg_tx.clone();
                        let ctx2 = ctx.clone();
                        let dev = device_id.to_string();
                        let pkg2 = pkg.clone();
                        std::thread::spawn(move || {
                            let (success, message) =
                                crate::security::apps::clear_app_data(&dev, &pkg2);
                            let _ = tx.send(BgEvent::AppActionResult {
                                package: pkg2,
                                action: "clear-data".into(),
                                success,
                                message,
                            });
                            ctx2.request_repaint();
                        });
                        app.confirm_clear_data = None;
                    }
                    if ui.button("Annuler").clicked() {
                        app.confirm_clear_data = None;
                    }
                });
            });
        if !open {
            app.confirm_clear_data = None;
        }
    }

    // Uninstall confirmation
    if let Some(pkg) = app.confirm_uninstall.clone() {
        let mut open = true;
        egui::Window::new("Confirmer désinstallation")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("Désinstaller {} ?", pkg));
                ui.label(
                    egui::RichText::new("Cette action est irréversible.")
                        .color(theme::danger_color()),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Désinstaller")
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(theme::danger_color())
                            .corner_radius(6.0),
                        )
                        .clicked()
                    {
                        let tx = app.bg_tx.clone();
                        let ctx2 = ctx.clone();
                        let dev = device_id.to_string();
                        let pkg2 = pkg.clone();
                        std::thread::spawn(move || {
                            let (success, message) =
                                crate::security::apps::uninstall_app(&dev, &pkg2);
                            let _ = tx.send(BgEvent::AppActionResult {
                                package: pkg2,
                                action: "uninstall".into(),
                                success,
                                message,
                            });
                            ctx2.request_repaint();
                        });
                        app.confirm_uninstall = None;
                    }
                    if ui.button("Annuler").clicked() {
                        app.confirm_uninstall = None;
                    }
                });
            });
        if !open {
            app.confirm_uninstall = None;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// PERMISSIONS VIEW
// ═══════════════════════════════════════════════════════════════════
fn draw_permissions(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let dark = app.dark_mode;
    let device_id = match app.get_selected_id() {
        Some(id) => id,
        None => {
            ui.label(
                egui::RichText::new("Sélectionnez un appareil.")
                    .color(theme::text_secondary(dark)),
            );
            return;
        }
    };

    // Auto-load permissions when we have apps but no permission cache
    if !app.security_apps.is_empty() && app.security_permission_cache.is_empty() {
        trigger_permissions_load(app, ctx, &device_id);
    }

    // Toggle buttons + header
    ui.horizontal(|ui| {
        section_title(ui, "\u{1f510} Permissions");

        let views = [
            (PermissionView::ByPermission, "Par permission"),
            (PermissionView::ByApp, "Par application"),
        ];
        for (view, label) in views {
            let selected = app.security_permission_view == view;
            let mut text = egui::RichText::new(label).size(12.0);
            text = if selected {
                text.strong().color(theme::accent_color())
            } else {
                text.color(theme::text_secondary(dark))
            };
            let btn = egui::Button::new(text).corner_radius(4.0);
            let btn = if selected {
                btn.fill(theme::card_selected(dark))
            } else {
                btn.fill(egui::Color32::TRANSPARENT)
            };
            if ui.add(btn).clicked() {
                app.security_permission_view = view;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("\u{1f504} Recharger permissions")
                            .color(egui::Color32::WHITE),
                    )
                    .fill(theme::accent_blue())
                    .corner_radius(6.0),
                )
                .clicked()
            {
                trigger_permissions_load(app, ctx, &device_id);
            }
        });
    });

    ui.add_space(8.0);

    if app.security_permission_cache.is_empty() && !app.security_apps.is_empty() {
        section(ui, dark, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Chargement des permissions...");
            });
        });
        return;
    }

    match app.security_permission_view {
        PermissionView::ByPermission => {
            draw_permissions_by_permission(ui, app, ctx, &device_id)
        }
        PermissionView::ByApp => draw_permissions_by_app(ui, app, ctx, &device_id),
    }
}

fn draw_permissions_by_permission(
    ui: &mut egui::Ui,
    app: &mut PhoneTvApp,
    ctx: &egui::Context,
    device_id: &str,
) {
    let groups: &[(&str, &str, &[&str])] = &[
        ("\u{1f4f7}", "Caméra", &["android.permission.CAMERA"]),
        ("\u{1f3a4}", "Microphone", &["android.permission.RECORD_AUDIO"]),
        (
            "\u{1f4cd}",
            "Localisation",
            &[
                "android.permission.ACCESS_FINE_LOCATION",
                "android.permission.ACCESS_COARSE_LOCATION",
                "android.permission.ACCESS_BACKGROUND_LOCATION",
            ],
        ),
        (
            "\u{1f4d2}",
            "Contacts",
            &[
                "android.permission.READ_CONTACTS",
                "android.permission.WRITE_CONTACTS",
            ],
        ),
        (
            "\u{1f4e8}",
            "SMS",
            &[
                "android.permission.READ_SMS",
                "android.permission.SEND_SMS",
            ],
        ),
        (
            "\u{1f4de}",
            "Journal d'appels",
            &["android.permission.READ_CALL_LOG"],
        ),
        (
            "\u{1f4be}",
            "Stockage",
            &[
                "android.permission.READ_EXTERNAL_STORAGE",
                "android.permission.WRITE_EXTERNAL_STORAGE",
            ],
        ),
        (
            "\u{1f4f1}",
            "Téléphone",
            &["android.permission.READ_PHONE_STATE"],
        ),
        (
            "\u{1f4c5}",
            "Calendrier",
            &[
                "android.permission.READ_CALENDAR",
                "android.permission.WRITE_CALENDAR",
            ],
        ),
        ("\u{1f9e0}", "Capteurs", &["android.permission.BODY_SENSORS"]),
    ];

    let dark = app.dark_mode;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for (icon, group_name, perm_names) in groups {
                let mut apps_with_perm: Vec<(String, String, Option<String>, bool)> = Vec::new();
                for (pkg, perms) in &app.security_permission_cache {
                    for perm in perms {
                        if perm_names.contains(&perm.name.as_str()) && perm.granted {
                            apps_with_perm.push((
                                pkg.clone(),
                                perm.name.clone(),
                                perm.last_used.clone(),
                                perm.is_runtime,
                            ));
                        }
                    }
                }

                let count = apps_with_perm.len();
                let header_color = if count > 5 {
                    theme::warning_color()
                } else {
                    theme::text_primary(dark)
                };

                egui::CollapsingHeader::new(
                    egui::RichText::new(format!("{} {} ({})", icon, group_name, count))
                        .size(14.0)
                        .strong()
                        .color(header_color),
                )
                .show(ui, |ui| {
                    if apps_with_perm.is_empty() {
                        ui.label(
                            egui::RichText::new("Aucune application")
                                .small()
                                .color(theme::text_secondary(dark)),
                        );
                    } else {
                        for (pkg, perm_name, last_used, is_runtime) in &apps_with_perm {
                            card_frame(dark).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let display_pkg = if pkg.len() > 40 {
                                        format!("{}...", &pkg[..37])
                                    } else {
                                        pkg.clone()
                                    };
                                    ui.label(
                                        egui::RichText::new(display_pkg).size(12.0),
                                    );
                                    badge(ui, "ACCORDÉ", theme::success_color());
                                    if let Some(used) = last_used {
                                        ui.label(
                                            egui::RichText::new(used)
                                                .small()
                                                .color(theme::text_secondary(dark)),
                                        );
                                    }
                                    if *is_runtime {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            egui::RichText::new("Révoquer")
                                                                .small()
                                                                .color(egui::Color32::WHITE),
                                                        )
                                                        .fill(theme::warning_color())
                                                        .corner_radius(4.0),
                                                    )
                                                    .clicked()
                                                {
                                                    let tx = app.bg_tx.clone();
                                                    let ctx2 = ctx.clone();
                                                    let dev = device_id.to_string();
                                                    let pkg2 = pkg.clone();
                                                    let perm2 = perm_name.clone();
                                                    std::thread::spawn(move || {
                                                        let (success, message) =
                                                            crate::security::permissions::revoke_permission(
                                                                &dev, &pkg2, &perm2,
                                                            );
                                                        let _ = tx.send(BgEvent::AppActionResult {
                                                            package: pkg2,
                                                            action: format!("revoke {}", perm2),
                                                            success,
                                                            message,
                                                        });
                                                        ctx2.request_repaint();
                                                    });
                                                }
                                            },
                                        );
                                    }
                                });
                            });
                            ui.add_space(2.0);
                        }
                    }
                });
            }
        });
}

fn draw_permissions_by_app(
    ui: &mut egui::Ui,
    app: &mut PhoneTvApp,
    ctx: &egui::Context,
    device_id: &str,
) {
    let dark = app.dark_mode;

    let app_names: Vec<String> = app
        .security_apps
        .iter()
        .map(|a| a.package.clone())
        .collect();

    ui.horizontal(|ui| {
        ui.label("Application :");
        let current = app
            .security_selected_app
            .clone()
            .unwrap_or_else(|| "-- Choisir --".to_string());

        egui::ComboBox::from_id_salt("perm_app_select")
            .selected_text(&current)
            .width(350.0)
            .show_ui(ui, |ui| {
                for name in &app_names {
                    if ui
                        .selectable_label(
                            app.security_selected_app.as_deref() == Some(name.as_str()),
                            name,
                        )
                        .clicked()
                    {
                        app.security_selected_app = Some(name.clone());
                        if !app.security_permission_cache.contains_key(name) {
                            let tx = app.bg_tx.clone();
                            let ctx2 = ctx.clone();
                            let id = device_id.to_string();
                            let pkg = name.clone();
                            std::thread::spawn(move || {
                                let perms =
                                    crate::security::permissions::get_app_permissions(&id, &pkg);
                                let _ = tx.send(BgEvent::SecurityPermissions {
                                    package: pkg,
                                    permissions: perms,
                                });
                                ctx2.request_repaint();
                            });
                        }
                    }
                }
            });
    });

    ui.add_space(8.0);

    if let Some(ref selected) = app.security_selected_app.clone() {
        if let Some(perms) = app.security_permission_cache.get(selected) {
            if perms.is_empty() {
                ui.label(
                    egui::RichText::new("Aucune permission runtime")
                        .color(theme::text_secondary(dark)),
                );
            } else {
                // Count dangerous granted
                let dangerous_count = perms
                    .iter()
                    .filter(|p| p.dangerous && p.granted)
                    .count();
                if dangerous_count > 0 {
                    ui.horizontal(|ui| {
                        badge(
                            ui,
                            &format!("{} permission(s) dangereuse(s) accordée(s)", dangerous_count),
                            theme::danger_color(),
                        );
                    });
                    ui.add_space(4.0);
                }

                egui::ScrollArea::vertical()
                    .max_height(ui.available_height() - 20.0)
                    .show(ui, |ui| {
                        for perm in perms {
                            card_frame(dark).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let perm_short = perm
                                        .name
                                        .strip_prefix("android.permission.")
                                        .unwrap_or(&perm.name);
                                    ui.label(
                                        egui::RichText::new(perm_short).size(12.0),
                                    );

                                    if perm.granted {
                                        badge(ui, "ACCORDÉ", theme::success_color());
                                    } else {
                                        badge(ui, "REFUSÉ", theme::danger_color());
                                    }

                                    if perm.dangerous {
                                        badge(ui, "DANGEREUX", theme::warning_color());
                                    }

                                    if let Some(ref used) = perm.last_used {
                                        ui.label(
                                            egui::RichText::new(used)
                                                .small()
                                                .color(theme::text_secondary(dark)),
                                        );
                                    }

                                    if perm.granted && perm.is_runtime {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            egui::RichText::new("Révoquer")
                                                                .small()
                                                                .color(egui::Color32::WHITE),
                                                        )
                                                        .fill(theme::warning_color())
                                                        .corner_radius(4.0),
                                                    )
                                                    .clicked()
                                                {
                                                    let tx = app.bg_tx.clone();
                                                    let ctx2 = ctx.clone();
                                                    let dev = device_id.to_string();
                                                    let pkg = selected.clone();
                                                    let perm_name = perm.name.clone();
                                                    std::thread::spawn(move || {
                                                        let (success, message) =
                                                            crate::security::permissions::revoke_permission(
                                                                &dev, &pkg, &perm_name,
                                                            );
                                                        let _ = tx.send(
                                                            BgEvent::AppActionResult {
                                                                package: pkg,
                                                                action: format!(
                                                                    "revoke {}",
                                                                    perm_name
                                                                ),
                                                                success,
                                                                message,
                                                            },
                                                        );
                                                        ctx2.request_repaint();
                                                    });
                                                }
                                            },
                                        );
                                    }
                                });
                            });
                            ui.add_space(2.0);
                        }
                    });
            }
        } else {
            section(ui, dark, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Chargement des permissions...");
                });
            });
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// BLACKLIST VIEW
// ═══════════════════════════════════════════════════════════════════
fn draw_blacklist(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let dark = app.dark_mode;
    let device_id = app.get_selected_id();

    // Auto-check alerts when apps are loaded
    if app.blacklist_alerts.is_empty() && !app.blacklist.is_empty() && !app.security_apps.is_empty()
    {
        let found: Vec<String> = app
            .blacklist
            .iter()
            .filter(|b| app.security_apps.iter().any(|a| &a.package == *b))
            .cloned()
            .collect();
        if !found.is_empty() {
            app.blacklist_alerts = found;
        }
    }

    // Alert banner
    if !app.blacklist_alerts.is_empty() {
        egui::Frame::NONE
            .corner_radius(8.0)
            .inner_margin(12.0)
            .fill(if dark {
                egui::Color32::from_rgb(80, 20, 20)
            } else {
                egui::Color32::from_rgb(255, 230, 230)
            })
            .stroke(egui::Stroke::new(1.0, theme::danger_color()))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("\u{1f6a8} Applications blacklistées détectées !")
                        .strong()
                        .size(14.0)
                        .color(theme::danger_color()),
                );
                ui.add_space(4.0);

                let alerts = app.blacklist_alerts.clone();
                for pkg in &alerts {
                    ui.horizontal(|ui| {
                        badge(ui, pkg, theme::danger_color());

                        if let Some(ref dev) = device_id {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Désactiver")
                                            .small()
                                            .color(egui::Color32::WHITE),
                                    )
                                    .fill(theme::warning_color())
                                    .corner_radius(4.0),
                                )
                                .clicked()
                            {
                                let tx = app.bg_tx.clone();
                                let ctx2 = ctx.clone();
                                let dev2 = dev.clone();
                                let pkg2 = pkg.clone();
                                std::thread::spawn(move || {
                                    let (success, message) =
                                        crate::security::apps::disable_app(&dev2, &pkg2);
                                    let _ = tx.send(BgEvent::AppActionResult {
                                        package: pkg2,
                                        action: "disable".into(),
                                        success,
                                        message,
                                    });
                                    ctx2.request_repaint();
                                });
                            }

                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Désinstaller")
                                            .small()
                                            .color(egui::Color32::WHITE),
                                    )
                                    .fill(theme::danger_color())
                                    .corner_radius(4.0),
                                )
                                .clicked()
                            {
                                let tx = app.bg_tx.clone();
                                let ctx2 = ctx.clone();
                                let dev2 = dev.clone();
                                let pkg2 = pkg.clone();
                                std::thread::spawn(move || {
                                    let (success, message) =
                                        crate::security::apps::uninstall_app(&dev2, &pkg2);
                                    let _ = tx.send(BgEvent::AppActionResult {
                                        package: pkg2,
                                        action: "uninstall".into(),
                                        success,
                                        message,
                                    });
                                    ctx2.request_repaint();
                                });
                            }
                        }
                    });
                }
            });
        ui.add_space(8.0);
    }

    // Header
    section(ui, dark, |ui| {
        ui.horizontal(|ui| {
            section_title(ui, "\u{1f6ab} Blacklist");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("\u{1f50d} Vérifier alertes")
                                .color(egui::Color32::WHITE),
                        )
                        .fill(theme::accent_blue())
                        .corner_radius(6.0),
                    )
                    .clicked()
                {
                    let found: Vec<String> = app
                        .blacklist
                        .iter()
                        .filter(|b| app.security_apps.iter().any(|a| &a.package == *b))
                        .cloned()
                        .collect();
                    app.blacklist_alerts = found;
                }
            });
        });

        ui.add_space(4.0);

        // Add entry
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.blacklist_new_entry)
                    .desired_width(300.0)
                    .hint_text("com.example.package"),
            );
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("+ Ajouter")
                            .color(egui::Color32::WHITE),
                    )
                    .fill(theme::success_color())
                    .corner_radius(6.0),
                )
                .clicked()
                && !app.blacklist_new_entry.is_empty()
            {
                let entry = app.blacklist_new_entry.trim().to_string();
                if !app.blacklist.contains(&entry) {
                    app.blacklist.push(entry);
                    crate::config::save_blacklist(&app.blacklist);
                }
                app.blacklist_new_entry.clear();
            }
        });

        ui.add_space(4.0);

        // Import / Export
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new("Importer").corner_radius(4.0))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        for line in content.lines() {
                            let line = line.trim().to_string();
                            if !line.is_empty() && !app.blacklist.contains(&line) {
                                app.blacklist.push(line);
                            }
                        }
                        crate::config::save_blacklist(&app.blacklist);
                        app.log("Blacklist importée");
                    }
                }
            }
            if ui
                .add(egui::Button::new("Exporter").corner_radius(4.0))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().save_file() {
                    let content = app.blacklist.join("\n");
                    if std::fs::write(&path, content).is_ok() {
                        app.log("Blacklist exportée");
                    }
                }
            }
        });
    });

    // Blacklist entries
    ui.label(
        egui::RichText::new(format!("{} entrée(s)", app.blacklist.len()))
            .size(13.0)
            .color(theme::text_secondary(dark)),
    );
    ui.add_space(4.0);

    let mut to_remove: Option<usize> = None;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for (i, entry) in app.blacklist.iter().enumerate() {
                card_frame(dark).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(entry).size(13.0));
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("\u{2715}")
                                                .color(theme::danger_color()),
                                        )
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    to_remove = Some(i);
                                }
                            },
                        );
                    });
                });
                ui.add_space(2.0);
            }
        });

    if let Some(idx) = to_remove {
        app.blacklist.remove(idx);
        crate::config::save_blacklist(&app.blacklist);
    }
}

// ── Format bytes ───────────────────────────────────────────────────
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return "0".to_string();
    }
    let kb = bytes / 1024;
    if kb < 1024 {
        return format!("{} KB", kb);
    }
    let mb = kb / 1024;
    if mb < 1024 {
        return format!("{} MB", mb);
    }
    let gb = mb / 1024;
    format!("{} GB", gb)
}

// ═══════════════════════════════════════════════════════════════════
// MONITORING VIEW
// ═══════════════════════════════════════════════════════════════════
fn draw_monitoring(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let dark = app.dark_mode;
    let device_id = match app.get_selected_id() {
        Some(id) => id,
        None => {
            ui.label(
                egui::RichText::new("Sélectionnez un appareil.")
                    .color(theme::text_secondary(dark)),
            );
            return;
        }
    };

    // Auto-load processes on first visit
    if app.security_processes.is_empty() {
        trigger_processes_load(app, ctx, &device_id);
    }

    // Sub-view toggle
    section(ui, dark, |ui| {
        ui.horizontal(|ui| {
            section_title(ui, "\u{1f4ca} Monitoring");
            let views = [
                (MonitoringView::Processes, "Processus"),
                (MonitoringView::DataUsage, "Données"),
                (MonitoringView::Wakelocks, "Wakelocks"),
            ];
            for (view, label) in views {
                let selected = app.security_monitoring_view == view;
                let mut text = egui::RichText::new(label).size(12.0);
                text = if selected {
                    text.strong().color(theme::accent_color())
                } else {
                    text.color(theme::text_secondary(dark))
                };
                let btn = egui::Button::new(text).corner_radius(4.0);
                let btn = if selected {
                    btn.fill(theme::card_selected(dark))
                } else {
                    btn.fill(egui::Color32::TRANSPARENT)
                };
                if ui.add(btn).clicked() {
                    app.security_monitoring_view = view;
                }
            }
        });
    });

    match app.security_monitoring_view {
        MonitoringView::Processes => draw_processes(ui, app, ctx, &device_id),
        MonitoringView::DataUsage => draw_data_usage(ui, app, ctx, &device_id),
        MonitoringView::Wakelocks => draw_wakelocks(ui, app, ctx, &device_id),
    }
}

fn draw_processes(
    ui: &mut egui::Ui,
    app: &mut PhoneTvApp,
    ctx: &egui::Context,
    device_id: &str,
) {
    let dark = app.dark_mode;

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("\u{1f504} Rafraîchir")
                        .color(egui::Color32::WHITE),
                )
                .fill(theme::accent_blue())
                .corner_radius(6.0),
            )
            .clicked()
        {
            trigger_processes_load(app, ctx, device_id);
        }
        ui.label(
            egui::RichText::new(format!("{} processus", app.security_processes.len()))
                .color(theme::text_secondary(dark)),
        );
    });

    ui.add_space(8.0);

    if app.security_processes.is_empty() {
        section(ui, dark, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Chargement des processus...");
            });
        });
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            egui::Grid::new("processes_grid")
                .num_columns(6)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Package").strong());
                    ui.label(egui::RichText::new("PID").strong());
                    ui.label(egui::RichText::new("Mémoire").strong());
                    ui.label(egui::RichText::new("Adj").strong());
                    ui.label(egui::RichText::new("État").strong());
                    ui.label(egui::RichText::new("Action").strong());
                    ui.end_row();

                    let processes = app.security_processes.clone();
                    for proc in &processes {
                        let display_pkg = if proc.package.len() > 35 {
                            format!("{}...", &proc.package[..32])
                        } else {
                            proc.package.clone()
                        };
                        ui.label(egui::RichText::new(display_pkg).size(12.0));
                        ui.label(
                            egui::RichText::new(format!("{}", proc.pid))
                                .size(12.0)
                                .color(theme::text_secondary(dark)),
                        );
                        ui.label(
                            egui::RichText::new(format!("{} MB", proc.memory_kb / 1024))
                                .size(12.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("{}", proc.adj))
                                .size(12.0)
                                .color(theme::text_secondary(dark)),
                        );

                        let state_color = match proc.state.as_str() {
                            "foreground" | "visible" => theme::success_color(),
                            "service" => theme::accent_blue(),
                            _ => theme::text_dim(dark),
                        };
                        ui.label(
                            egui::RichText::new(&proc.state)
                                .size(12.0)
                                .color(state_color),
                        );

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Kill")
                                        .small()
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(theme::danger_color())
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            let tx = app.bg_tx.clone();
                            let ctx2 = ctx.clone();
                            let dev = device_id.to_string();
                            let pkg = proc.package.clone();
                            std::thread::spawn(move || {
                                crate::security::apps::force_stop_app(&dev, &pkg);
                                let _ = tx.send(BgEvent::AppActionResult {
                                    package: pkg,
                                    action: "kill".into(),
                                    success: true,
                                    message: "OK".into(),
                                });
                                let processes =
                                    crate::security::monitoring::get_running_processes(&dev);
                                let _ = tx.send(BgEvent::SecurityProcesses { processes });
                                ctx2.request_repaint();
                            });
                        }
                        ui.end_row();
                    }
                });
        });
}

fn draw_data_usage(
    ui: &mut egui::Ui,
    app: &mut PhoneTvApp,
    ctx: &egui::Context,
    device_id: &str,
) {
    let dark = app.dark_mode;

    // Auto-load (with guard)
    if app.security_data_usage.is_empty() && !app.security_data_usage_loading {
        app.security_data_usage_loading = true;
        let tx = app.bg_tx.clone();
        let ctx2 = ctx.clone();
        let id = device_id.to_string();
        std::thread::spawn(move || {
            let usage = crate::security::monitoring::get_data_usage(&id);
            let _ = tx.send(BgEvent::SecurityDataUsage { usage });
            ctx2.request_repaint();
        });
    }

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("\u{1f504} Rafraîchir")
                        .color(egui::Color32::WHITE),
                )
                .fill(theme::accent_blue())
                .corner_radius(6.0),
            )
            .clicked()
        {
            let tx = app.bg_tx.clone();
            let ctx2 = ctx.clone();
            let id = device_id.to_string();
            std::thread::spawn(move || {
                let usage = crate::security::monitoring::get_data_usage(&id);
                let _ = tx.send(BgEvent::SecurityDataUsage { usage });
                ctx2.request_repaint();
            });
        }
        ui.label(
            egui::RichText::new("Données cumulées depuis le dernier reset")
                .small()
                .color(theme::text_secondary(dark)),
        );
    });

    ui.add_space(8.0);

    if app.security_data_usage.is_empty() {
        section(ui, dark, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Chargement...");
            });
        });
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            egui::Grid::new("data_usage_grid")
                .num_columns(5)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Package").strong());
                    ui.label(egui::RichText::new("WiFi RX").strong());
                    ui.label(egui::RichText::new("WiFi TX").strong());
                    ui.label(egui::RichText::new("Mobile RX").strong());
                    ui.label(egui::RichText::new("Mobile TX").strong());
                    ui.end_row();

                    for usage in &app.security_data_usage {
                        ui.label(
                            egui::RichText::new(&usage.package).size(12.0),
                        );
                        ui.label(
                            egui::RichText::new(format_bytes(usage.wifi_rx))
                                .size(12.0)
                                .color(theme::text_secondary(dark)),
                        );
                        ui.label(
                            egui::RichText::new(format_bytes(usage.wifi_tx))
                                .size(12.0)
                                .color(theme::text_secondary(dark)),
                        );
                        ui.label(
                            egui::RichText::new(format_bytes(usage.mobile_rx))
                                .size(12.0)
                                .color(theme::text_secondary(dark)),
                        );
                        ui.label(
                            egui::RichText::new(format_bytes(usage.mobile_tx))
                                .size(12.0)
                                .color(theme::text_secondary(dark)),
                        );
                        ui.end_row();
                    }
                });
        });
}

fn draw_wakelocks(
    ui: &mut egui::Ui,
    app: &mut PhoneTvApp,
    ctx: &egui::Context,
    device_id: &str,
) {
    let dark = app.dark_mode;

    // Auto-load (with guard)
    if app.security_wakelocks.is_empty() && !app.security_wakelocks_loading {
        app.security_wakelocks_loading = true;
        let tx = app.bg_tx.clone();
        let ctx2 = ctx.clone();
        let id = device_id.to_string();
        std::thread::spawn(move || {
            let wakelocks = crate::security::monitoring::get_wakelocks(&id);
            let _ = tx.send(BgEvent::SecurityWakelocks { wakelocks });
            ctx2.request_repaint();
        });
    }

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("\u{1f504} Rafraîchir")
                        .color(egui::Color32::WHITE),
                )
                .fill(theme::accent_blue())
                .corner_radius(6.0),
            )
            .clicked()
        {
            let tx = app.bg_tx.clone();
            let ctx2 = ctx.clone();
            let id = device_id.to_string();
            std::thread::spawn(move || {
                let wakelocks = crate::security::monitoring::get_wakelocks(&id);
                let _ = tx.send(BgEvent::SecurityWakelocks { wakelocks });
                ctx2.request_repaint();
            });
        }
        ui.label(
            egui::RichText::new(format!("{} wakelock(s)", app.security_wakelocks.len()))
                .color(theme::text_secondary(dark)),
        );
    });

    ui.add_space(8.0);

    if app.security_wakelocks.is_empty() {
        section(ui, dark, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Chargement...");
            });
        });
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            egui::Grid::new("wakelocks_grid")
                .num_columns(2)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Package").strong());
                    ui.label(egui::RichText::new("Durée").strong());
                    ui.end_row();

                    for wl in &app.security_wakelocks {
                        ui.label(egui::RichText::new(&wl.package).size(12.0));

                        let duration_color = if wl.duration_ms > 30 * 60 * 1000 {
                            theme::danger_color()
                        } else if wl.duration_ms > 5 * 60 * 1000 {
                            theme::warning_color()
                        } else {
                            theme::text_secondary(dark)
                        };
                        ui.label(
                            egui::RichText::new(&wl.duration_human)
                                .size(12.0)
                                .color(duration_color),
                        );
                        ui.end_row();
                    }
                });
        });
}

// ═══════════════════════════════════════════════════════════════════
// POSTURE VIEW
// ═══════════════════════════════════════════════════════════════════
fn draw_posture(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let dark = app.dark_mode;
    let device_id = match app.get_selected_id() {
        Some(id) => id,
        None => {
            ui.label(
                egui::RichText::new("Sélectionnez un appareil.")
                    .color(theme::text_secondary(dark)),
            );
            return;
        }
    };

    // Auto-load
    if app.security_posture.is_empty() {
        trigger_posture_load(app, ctx, &device_id);
    }

    // Header with refresh
    ui.horizontal(|ui| {
        section_title(ui, "\u{2699} Posture de l'appareil");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("\u{1f504} Rafraîchir")
                            .color(egui::Color32::WHITE),
                    )
                    .fill(theme::accent_blue())
                    .corner_radius(6.0),
                )
                .clicked()
            {
                trigger_posture_load(app, ctx, &device_id);
            }
        });
    });

    ui.add_space(8.0);

    if app.security_posture.is_empty() {
        section(ui, dark, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Chargement...");
            });
        });
        return;
    }

    // Summary
    let good_count = app
        .security_posture
        .iter()
        .filter(|c| c.status == PostureStatus::Good)
        .count();
    let bad_count = app
        .security_posture
        .iter()
        .filter(|c| c.status == PostureStatus::Bad)
        .count();
    let warn_count = app
        .security_posture
        .iter()
        .filter(|c| c.status == PostureStatus::Warning)
        .count();

    ui.horizontal_wrapped(|ui| {
        if good_count > 0 {
            badge(ui, &format!("{} OK", good_count), theme::success_color());
        }
        if warn_count > 0 {
            badge(
                ui,
                &format!("{} avertissement(s)", warn_count),
                theme::warning_color(),
            );
        }
        if bad_count > 0 {
            badge(
                ui,
                &format!("{} problème(s)", bad_count),
                theme::danger_color(),
            );
        }
    });
    ui.add_space(8.0);

    let posture = app.security_posture.clone();

    // 2-column grid of status cards
    egui::Grid::new("posture_grid")
        .num_columns(2)
        .spacing([12.0, 12.0])
        .show(ui, |ui| {
            for (i, check) in posture.iter().enumerate() {
                let frame = match check.status {
                    PostureStatus::Good => card_frame(dark),
                    PostureStatus::Warning => tinted_card_frame(dark, &Severity::Warning),
                    PostureStatus::Bad => tinted_card_frame(dark, &Severity::Critical),
                };
                frame.show(ui, |ui| {
                    ui.set_min_width(250.0);
                    ui.horizontal(|ui| {
                        // Status dot
                        let dot_color = match check.status {
                            PostureStatus::Good => theme::success_color(),
                            PostureStatus::Warning => theme::warning_color(),
                            PostureStatus::Bad => theme::danger_color(),
                        };
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(8.0, 8.0),
                            egui::Sense::hover(),
                        );
                        ui.painter()
                            .circle_filled(rect.center(), 4.0, dot_color);

                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&check.name)
                                    .strong()
                                    .size(13.0),
                            );
                            ui.label(
                                egui::RichText::new(&check.value)
                                    .size(12.0)
                                    .color(theme::text_secondary(dark)),
                            );
                        });

                        if check.fix_command.is_some() {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("\u{1f527} Corriger")
                                                    .color(egui::Color32::WHITE),
                                            )
                                            .fill(theme::success_color())
                                            .corner_radius(6.0),
                                        )
                                        .clicked()
                                    {
                                        let tx = app.bg_tx.clone();
                                        let ctx2 = ctx.clone();
                                        let dev = device_id.clone();
                                        let cmd = check.fix_command.clone().unwrap();
                                        std::thread::spawn(move || {
                                            let success =
                                                crate::security::posture::fix_setting(&dev, &cmd);
                                            let _ = tx.send(BgEvent::Log(format!(
                                                "Fix {} : {}",
                                                cmd,
                                                if success { "OK" } else { "ÉCHEC" }
                                            )));
                                            let checks =
                                                crate::security::posture::check_device_posture(
                                                    &dev,
                                                );
                                            let _ =
                                                tx.send(BgEvent::SecurityPosture { checks });
                                            ctx2.request_repaint();
                                        });
                                    }
                                },
                            );
                        }
                    });
                });

                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}
