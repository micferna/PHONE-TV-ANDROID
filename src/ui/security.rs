use eframe::egui;
use std::sync::atomic::Ordering;

use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::*;

pub fn draw_security(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.add_space(4.0);
    ui.heading("Securite");
    ui.add_space(8.0);

    // Sub-tab navigation bar
    ui.horizontal(|ui| {
        let tabs = [
            (SecurityView::Score, "Score"),
            (SecurityView::Apps, "Apps"),
            (SecurityView::Permissions, "Permissions"),
            (SecurityView::Blacklist, "Blacklist"),
            (SecurityView::Monitoring, "Monitoring"),
            (SecurityView::Posture, "Posture"),
        ];

        for (view, label) in tabs {
            let selected = app.security_view == view;
            let text = egui::RichText::new(label).size(13.0);
            let text = if selected {
                text.strong().color(theme::accent_color())
            } else {
                text.color(theme::text_secondary(app.dark_mode))
            };
            let btn = egui::Button::new(text)
                .corner_radius(6.0)
                .min_size(egui::vec2(0.0, 30.0));
            let btn = if selected {
                btn.fill(theme::card_selected(app.dark_mode))
            } else {
                btn.fill(egui::Color32::TRANSPARENT)
            };
            if ui.add(btn).clicked() {
                app.security_view = view;
            }
        }
    });

    ui.separator();
    ui.add_space(8.0);

    // Dispatch to sub-views
    match app.security_view {
        SecurityView::Score => draw_score(ui, app, ctx),
        SecurityView::Apps => draw_apps(ui, app, ctx),
        SecurityView::Permissions => draw_permissions_stub(ui, app),
        SecurityView::Blacklist => draw_blacklist_stub(ui, app),
        SecurityView::Monitoring => draw_monitoring_stub(ui, app),
        SecurityView::Posture => draw_posture_stub(ui, app),
    }
}

// ── Helper: card frame ──────────────────────────────────────────────
fn card_frame(dark_mode: bool) -> egui::Frame {
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(12.0)
        .fill(theme::card_bg(dark_mode))
        .stroke(egui::Stroke::new(0.5, theme::card_border(dark_mode)))
}

// ── Helper: score color ─────────────────────────────────────────────
fn score_color(score: u8) -> egui::Color32 {
    if score >= 80 {
        theme::success_color()
    } else if score >= 50 {
        theme::warning_color()
    } else {
        theme::danger_color()
    }
}

// ── Helper: severity color ──────────────────────────────────────────
fn severity_color(severity: &Severity) -> egui::Color32 {
    match severity {
        Severity::Critical => theme::danger_color(),
        Severity::Warning => theme::warning_color(),
        Severity::Info => theme::accent_blue(),
    }
}

// ── Helper: trigger score loading ───────────────────────────────────
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

// ═══════════════════════════════════════════════════════════════════
// Task 11: Score UI
// ═══════════════════════════════════════════════════════════════════
fn draw_score(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let device_selected = app.get_selected_id().is_some();

    if !device_selected {
        ui.label(
            egui::RichText::new("Selectionnez un appareil pour voir le score de securite.")
                .color(theme::text_secondary(app.dark_mode)),
        );
        return;
    }

    // Auto-load
    if app.security_score.is_none() && !app.security_score_loading {
        trigger_score_load(app, ctx);
    }

    // Header with refresh button
    ui.horizontal(|ui| {
        ui.heading(egui::RichText::new("Score de securite").size(18.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let refresh_enabled = !app.security_score_loading;
            if ui
                .add_enabled(refresh_enabled, egui::Button::new("Rafraichir"))
                .clicked()
            {
                trigger_score_load(app, ctx);
            }
        });
    });

    ui.add_space(8.0);

    if app.security_score_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Calcul du score en cours...");
        });
        return;
    }

    if let Some((score, issues)) = app.security_score.clone() {
        // Score display card
        card_frame(app.dark_mode).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);
                // Large score number
                let color = score_color(score);
                ui.label(
                    egui::RichText::new(format!("{}", score))
                        .size(48.0)
                        .strong()
                        .color(color),
                );
                ui.label(
                    egui::RichText::new("/ 100")
                        .size(16.0)
                        .color(theme::text_secondary(app.dark_mode)),
                );
                ui.add_space(8.0);

                // Progress bar as gauge
                let bar = egui::ProgressBar::new(score as f32 / 100.0)
                    .fill(color)
                    .desired_width(300.0);
                ui.add(bar);
                ui.add_space(12.0);
            });
        });

        ui.add_space(12.0);

        // Issues list
        if issues.is_empty() {
            ui.label(
                egui::RichText::new("Aucun probleme detecte !")
                    .color(theme::success_color()),
            );
        } else {
            ui.label(
                egui::RichText::new(format!("{} probleme(s) detecte(s)", issues.len()))
                    .size(14.0)
                    .strong(),
            );
            ui.add_space(4.0);

            for issue in &issues {
                card_frame(app.dark_mode).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let sev_label = match issue.severity {
                            Severity::Critical => "CRITIQUE",
                            Severity::Warning => "ATTENTION",
                            Severity::Info => "INFO",
                        };
                        let sev_color = severity_color(&issue.severity);
                        ui.label(
                            egui::RichText::new(sev_label)
                                .small()
                                .strong()
                                .color(sev_color),
                        );
                        ui.label(&issue.description);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} pts", issue.points))
                                        .small()
                                        .color(theme::text_secondary(app.dark_mode)),
                                );
                            },
                        );
                    });
                });
                ui.add_space(2.0);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Task 12: Apps management UI
// ═══════════════════════════════════════════════════════════════════
fn draw_apps(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let device_id = match app.get_selected_id() {
        Some(id) => id,
        None => {
            ui.label(
                egui::RichText::new("Selectionnez un appareil.")
                    .color(theme::text_secondary(app.dark_mode)),
            );
            return;
        }
    };

    // Top bar: filter buttons
    ui.horizontal(|ui| {
        let filters = [
            (AppFilter::All, "Toutes"),
            (AppFilter::ThirdParty, "Tierces"),
            (AppFilter::System, "Systeme"),
            (AppFilter::Disabled, "Desactivees"),
        ];
        for (filter, label) in filters {
            let selected = app.security_apps_filter == filter;
            let text = egui::RichText::new(label).size(12.0);
            let text = if selected {
                text.strong().color(theme::accent_color())
            } else {
                text.color(theme::text_secondary(app.dark_mode))
            };
            let btn = egui::Button::new(text).corner_radius(4.0);
            let btn = if selected {
                btn.fill(theme::card_selected(app.dark_mode))
            } else {
                btn.fill(egui::Color32::TRANSPARENT)
            };
            if ui.add(btn).clicked() {
                app.security_apps_filter = filter;
            }
        }
    });

    ui.add_space(4.0);

    // Search + Sort + Load button
    ui.horizontal(|ui| {
        ui.label("Recherche:");
        ui.add(
            egui::TextEdit::singleline(&mut app.security_apps_search)
                .desired_width(200.0)
                .hint_text("Filtrer par nom..."),
        );

        ui.separator();

        ui.label("Tri:");
        egui::ComboBox::from_id_salt("app_sort")
            .selected_text(match app.security_apps_sort {
                AppSort::Name => "Nom",
                AppSort::InstallDate => "Date d'installation",
                AppSort::Source => "Source",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.security_apps_sort, AppSort::Name, "Nom");
                ui.selectable_value(
                    &mut app.security_apps_sort,
                    AppSort::InstallDate,
                    "Date d'installation",
                );
                ui.selectable_value(&mut app.security_apps_sort, AppSort::Source, "Source");
            });

        ui.separator();

        let load_enabled = !app.security_apps_loading;
        if ui
            .add_enabled(load_enabled, egui::Button::new("Charger"))
            .clicked()
        {
            app.security_apps_loading = true;
            app.security_loading_cancel
                .store(false, Ordering::Relaxed);
            let tx = app.bg_tx.clone();
            let ctx2 = ctx.clone();
            let id = device_id.clone();
            let filter = app.security_apps_filter;
            let cancel = app.security_loading_cancel.clone();
            std::thread::spawn(move || {
                let packages =
                    crate::security::apps::list_packages(&id, filter);
                let _ = tx.send(BgEvent::SecurityAppsList {
                    packages: packages.clone(),
                });
                ctx2.request_repaint();

                for pkg in &packages {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(info) =
                        crate::security::apps::get_app_detail(&id, pkg)
                    {
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

        if app.security_apps_loading {
            if ui.button("Annuler").clicked() {
                app.security_loading_cancel
                    .store(true, Ordering::Relaxed);
            }
            ui.spinner();
        }
    });

    ui.add_space(8.0);

    // Confirmation dialogs
    draw_confirm_dialogs(app, ctx, &device_id);

    // Filter and sort display list (clone to avoid borrow issues)
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
        AppSort::Name => display_apps.sort_by(|a, b| a.package.cmp(&b.package)),
        AppSort::InstallDate => {
            display_apps.sort_by(|a, b| b.first_install.cmp(&a.first_install))
        }
        AppSort::Source => display_apps.sort_by(|a, b| {
            format!("{:?}", a.installer).cmp(&format!("{:?}", b.installer))
        }),
    }

    ui.label(
        egui::RichText::new(format!("{} application(s)", display_apps.len()))
            .size(13.0)
            .color(theme::text_secondary(app.dark_mode)),
    );
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
        ui.horizontal(|ui| {
            // Left: info
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&info.package).strong().size(13.0));
                if info.details_loaded {
                    ui.horizontal(|ui| {
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
                    ui.horizontal(|ui| {
                        // Source badge
                        let (src_label, src_color) = match info.installer {
                            AppInstaller::PlayStore => ("Play Store", theme::success_color()),
                            AppInstaller::Sideload => ("Sideload", theme::warning_color()),
                            AppInstaller::Adb => ("ADB", theme::accent_blue()),
                            AppInstaller::Unknown => ("Inconnu", theme::text_secondary(dark)),
                        };
                        ui.label(
                            egui::RichText::new(src_label)
                                .small()
                                .strong()
                                .color(src_color),
                        );

                        if info.target_sdk > 0 {
                            ui.label(
                                egui::RichText::new(format!("SDK {}", info.target_sdk))
                                    .small()
                                    .color(theme::text_secondary(dark)),
                            );
                        }

                        if !info.enabled {
                            ui.label(
                                egui::RichText::new("DESACTIVE")
                                    .small()
                                    .strong()
                                    .color(theme::danger_color()),
                            );
                        }
                    });
                } else {
                    ui.label(
                        egui::RichText::new("Chargement details...")
                            .small()
                            .italics()
                            .color(theme::text_dim(dark)),
                    );
                }
            });

            // Right: action buttons
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pkg = info.package.clone();
                let dev = device_id.to_string();

                // Uninstall (red)
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Desinstaller").color(theme::danger_color()),
                        )
                        .small(),
                    )
                    .clicked()
                {
                    app.confirm_uninstall = Some(pkg.clone());
                }

                // Clear data
                if ui.small_button("Effacer donnees").clicked() {
                    app.confirm_clear_data = Some(pkg.clone());
                }

                // Force stop
                if ui.small_button("Forcer arret").clicked() {
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

                // Enable/Disable toggle
                if info.enabled {
                    if ui.small_button("Desactiver").clicked() {
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
                } else if ui.small_button("Activer").clicked() {
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
                    "Effacer toutes les donnees de {} ?",
                    pkg
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Confirmer").clicked() {
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
        egui::Window::new("Confirmer desinstallation")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("Desinstaller {} ?", pkg));
                ui.label(
                    egui::RichText::new("Cette action est irreversible.")
                        .color(theme::danger_color()),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("Desinstaller").color(theme::danger_color()),
                        ))
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

// ── Stubs for remaining views ───────────────────────────────────────
fn draw_permissions_stub(ui: &mut egui::Ui, _app: &PhoneTvApp) {
    ui.label(egui::RichText::new("Permission audit — coming soon").italics());
}

fn draw_blacklist_stub(ui: &mut egui::Ui, _app: &PhoneTvApp) {
    ui.label(egui::RichText::new("Blacklist — coming soon").italics());
}

fn draw_monitoring_stub(ui: &mut egui::Ui, _app: &PhoneTvApp) {
    ui.label(egui::RichText::new("Monitoring — coming soon").italics());
}

fn draw_posture_stub(ui: &mut egui::Ui, _app: &PhoneTvApp) {
    ui.label(egui::RichText::new("Device posture — coming soon").italics());
}
