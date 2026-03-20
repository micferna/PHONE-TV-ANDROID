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
        SecurityView::Permissions => draw_permissions(ui, app, ctx),
        SecurityView::Blacklist => draw_blacklist(ui, app, ctx),
        SecurityView::Monitoring => draw_monitoring(ui, app, ctx),
        SecurityView::Posture => draw_posture(ui, app, ctx),
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

// ═══════════════════════════════════════════════════════════════════
// Task 13: Permission audit UI
// ═══════════════════════════════════════════════════════════════════
fn draw_permissions(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
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

    // Toggle buttons
    ui.horizontal(|ui| {
        let views = [
            (PermissionView::ByPermission, "Par permission"),
            (PermissionView::ByApp, "Par application"),
        ];
        for (view, label) in views {
            let selected = app.security_permission_view == view;
            let text = egui::RichText::new(label).size(13.0);
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
                app.security_permission_view = view;
            }
        }

        ui.separator();

        // Load permissions button
        if ui.button("Charger permissions").clicked() {
            let apps: Vec<String> = app
                .security_apps
                .iter()
                .map(|a| a.package.clone())
                .collect();
            let tx = app.bg_tx.clone();
            let ctx2 = ctx.clone();
            let id = device_id.clone();
            std::thread::spawn(move || {
                for pkg in &apps {
                    let perms =
                        crate::security::permissions::get_app_permissions(&id, pkg);
                    let _ = tx.send(BgEvent::SecurityPermissions {
                        package: pkg.clone(),
                        permissions: perms,
                    });
                    ctx2.request_repaint();
                }
            });
        }
    });

    ui.add_space(8.0);

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
    let groups: &[(&str, &[&str])] = &[
        ("Camera", &["android.permission.CAMERA"]),
        ("Microphone", &["android.permission.RECORD_AUDIO"]),
        (
            "Localisation",
            &[
                "android.permission.ACCESS_FINE_LOCATION",
                "android.permission.ACCESS_COARSE_LOCATION",
                "android.permission.ACCESS_BACKGROUND_LOCATION",
            ],
        ),
        (
            "Contacts",
            &[
                "android.permission.READ_CONTACTS",
                "android.permission.WRITE_CONTACTS",
            ],
        ),
        (
            "SMS",
            &[
                "android.permission.READ_SMS",
                "android.permission.SEND_SMS",
            ],
        ),
        (
            "Journal d'appels",
            &["android.permission.READ_CALL_LOG"],
        ),
        (
            "Stockage",
            &[
                "android.permission.READ_EXTERNAL_STORAGE",
                "android.permission.WRITE_EXTERNAL_STORAGE",
            ],
        ),
        (
            "Telephone",
            &["android.permission.READ_PHONE_STATE"],
        ),
        (
            "Calendrier",
            &[
                "android.permission.READ_CALENDAR",
                "android.permission.WRITE_CALENDAR",
            ],
        ),
        ("Capteurs", &["android.permission.BODY_SENSORS"]),
    ];

    let dark = app.dark_mode;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for (group_name, perm_names) in groups {
                // Collect apps with these permissions granted
                let mut apps_with_perm: Vec<(String, String, Option<String>)> = Vec::new();
                for (pkg, perms) in &app.security_permission_cache {
                    for perm in perms {
                        if perm_names.contains(&perm.name.as_str()) && perm.granted {
                            apps_with_perm.push((
                                pkg.clone(),
                                perm.name.clone(),
                                perm.last_used.clone(),
                            ));
                        }
                    }
                }

                let count = apps_with_perm.len();
                let header_text = format!("{} ({})", group_name, count);

                egui::CollapsingHeader::new(
                    egui::RichText::new(header_text).size(14.0).strong(),
                )
                .show(ui, |ui| {
                    if apps_with_perm.is_empty() {
                        ui.label(
                            egui::RichText::new("Aucune application")
                                .small()
                                .color(theme::text_secondary(dark)),
                        );
                    } else {
                        for (pkg, perm_name, last_used) in &apps_with_perm {
                            card_frame(dark).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(pkg).size(12.0),
                                    );
                                    ui.label(
                                        egui::RichText::new("ACCORDE")
                                            .small()
                                            .strong()
                                            .color(theme::success_color()),
                                    );
                                    if let Some(used) = last_used {
                                        ui.label(
                                            egui::RichText::new(used)
                                                .small()
                                                .color(theme::text_secondary(dark)),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("Revoquer").clicked() {
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

    // App selector
    let app_names: Vec<String> = app
        .security_apps
        .iter()
        .map(|a| a.package.clone())
        .collect();

    ui.horizontal(|ui| {
        ui.label("Application:");
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
                        // Load permissions if not cached
                        if !app.security_permission_cache.contains_key(name) {
                            let tx = app.bg_tx.clone();
                            let ctx2 = ctx.clone();
                            let id = device_id.to_string();
                            let pkg = name.clone();
                            std::thread::spawn(move || {
                                let perms =
                                    crate::security::permissions::get_app_permissions(
                                        &id, &pkg,
                                    );
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

    // Display permissions for selected app
    if let Some(ref selected) = app.security_selected_app.clone() {
        if let Some(perms) = app.security_permission_cache.get(selected) {
            if perms.is_empty() {
                ui.label(
                    egui::RichText::new("Aucune permission runtime")
                        .color(theme::text_secondary(dark)),
                );
            } else {
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height() - 20.0)
                    .show(ui, |ui| {
                        for perm in perms {
                            card_frame(dark).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&perm.name).size(12.0),
                                    );

                                    let (status_text, status_color) = if perm.granted {
                                        ("ACCORDE", theme::success_color())
                                    } else {
                                        ("REFUSE", theme::danger_color())
                                    };
                                    ui.label(
                                        egui::RichText::new(status_text)
                                            .small()
                                            .strong()
                                            .color(status_color),
                                    );

                                    if perm.dangerous {
                                        ui.label(
                                            egui::RichText::new("DANGEREUX")
                                                .small()
                                                .strong()
                                                .color(theme::warning_color()),
                                        );
                                    }

                                    if let Some(ref used) = perm.last_used {
                                        ui.label(
                                            egui::RichText::new(used)
                                                .small()
                                                .color(theme::text_secondary(dark)),
                                        );
                                    }

                                    if perm.granted {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("Revoquer").clicked() {
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
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Chargement des permissions...");
            });
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Task 14: Blacklist UI
// ═══════════════════════════════════════════════════════════════════
fn draw_blacklist(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
    let device_id = app.get_selected_id();

    // Check alerts on first visit
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
            .fill(egui::Color32::from_rgb(80, 20, 20))
            .stroke(egui::Stroke::new(1.0, theme::danger_color()))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Applications blacklistees detectees !")
                        .strong()
                        .color(theme::danger_color()),
                );
                ui.add_space(4.0);

                let alerts = app.blacklist_alerts.clone();
                for pkg in &alerts {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("  {}", pkg))
                                .color(egui::Color32::WHITE),
                        );

                        if let Some(ref dev) = device_id {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Desactiver")
                                            .small()
                                            .color(theme::warning_color()),
                                    )
                                    .small(),
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
                                        egui::RichText::new("Desinstaller")
                                            .small()
                                            .color(theme::danger_color()),
                                    )
                                    .small(),
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

    // Refresh alerts button
    ui.horizontal(|ui| {
        ui.heading(egui::RichText::new("Blacklist").size(16.0));
        if ui.button("Verifier alertes").clicked() {
            let found: Vec<String> = app
                .blacklist
                .iter()
                .filter(|b| app.security_apps.iter().any(|a| &a.package == *b))
                .cloned()
                .collect();
            app.blacklist_alerts = found;
        }
    });

    ui.add_space(8.0);

    // Add entry
    ui.horizontal(|ui| {
        ui.label("Ajouter:");
        ui.add(
            egui::TextEdit::singleline(&mut app.blacklist_new_entry)
                .desired_width(300.0)
                .hint_text("com.example.package"),
        );
        if ui.button("Ajouter").clicked() && !app.blacklist_new_entry.is_empty() {
            let entry = app.blacklist_new_entry.trim().to_string();
            if !app.blacklist.contains(&entry) {
                app.blacklist.push(entry);
                crate::config::save_blacklist(&app.blacklist);
            }
            app.blacklist_new_entry.clear();
        }
    });

    // Import / Export
    ui.horizontal(|ui| {
        if ui.button("Importer").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        let line = line.trim().to_string();
                        if !line.is_empty() && !app.blacklist.contains(&line) {
                            app.blacklist.push(line);
                        }
                    }
                    crate::config::save_blacklist(&app.blacklist);
                    app.log("Blacklist importee");
                }
            }
        }
        if ui.button("Exporter").clicked() {
            if let Some(path) = rfd::FileDialog::new().save_file() {
                let content = app.blacklist.join("\n");
                if std::fs::write(&path, content).is_ok() {
                    app.log("Blacklist exportee");
                }
            }
        }
    });

    ui.add_space(8.0);

    // Blacklist entries
    ui.label(
        egui::RichText::new(format!("{} entree(s)", app.blacklist.len()))
            .size(13.0)
            .color(theme::text_secondary(app.dark_mode)),
    );

    let mut to_remove: Option<usize> = None;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for (i, entry) in app.blacklist.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(entry);
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("\u{2715}").color(theme::danger_color()),
                            )
                            .small(),
                        )
                        .clicked()
                    {
                        to_remove = Some(i);
                    }
                });
            }
        });

    if let Some(idx) = to_remove {
        app.blacklist.remove(idx);
        crate::config::save_blacklist(&app.blacklist);
    }
}

// ── Helper: format bytes ────────────────────────────────────────────
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
// Task 15: Monitoring UI
// ═══════════════════════════════════════════════════════════════════
fn draw_monitoring(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
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

    // Sub-view toggle
    ui.horizontal(|ui| {
        let views = [
            (MonitoringView::Processes, "Processus"),
            (MonitoringView::DataUsage, "Donnees"),
            (MonitoringView::Wakelocks, "Wakelocks"),
        ];
        for (view, label) in views {
            let selected = app.security_monitoring_view == view;
            let text = egui::RichText::new(label).size(13.0);
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
                app.security_monitoring_view = view;
            }
        }
    });

    ui.add_space(8.0);

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
    ui.horizontal(|ui| {
        if ui.button("Rafraichir").clicked() {
            let tx = app.bg_tx.clone();
            let ctx2 = ctx.clone();
            let id = device_id.to_string();
            std::thread::spawn(move || {
                let processes =
                    crate::security::monitoring::get_running_processes(&id);
                let _ = tx.send(BgEvent::SecurityProcesses { processes });
                ctx2.request_repaint();
            });
        }
        ui.label(
            egui::RichText::new(format!("{} processus", app.security_processes.len()))
                .color(theme::text_secondary(app.dark_mode)),
        );
    });

    ui.add_space(8.0);

    let dark = app.dark_mode;

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            egui::Grid::new("processes_grid")
                .num_columns(5)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    // Header
                    ui.label(egui::RichText::new("Package").strong());
                    ui.label(egui::RichText::new("PID").strong());
                    ui.label(egui::RichText::new("Memoire").strong());
                    ui.label(egui::RichText::new("Etat").strong());
                    ui.label(egui::RichText::new("Action").strong());
                    ui.end_row();

                    let processes = app.security_processes.clone();
                    for proc in &processes {
                        ui.label(
                            egui::RichText::new(&proc.package).size(12.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("{}", proc.pid))
                                .size(12.0)
                                .color(theme::text_secondary(dark)),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} MB",
                                proc.memory_kb / 1024
                            ))
                            .size(12.0),
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

                        if ui.small_button("Kill").clicked() {
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
                                // Refresh processes
                                let processes =
                                    crate::security::monitoring::get_running_processes(
                                        &dev,
                                    );
                                let _ = tx
                                    .send(BgEvent::SecurityProcesses { processes });
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

    ui.horizontal(|ui| {
        if ui.button("Rafraichir").clicked() {
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
            egui::RichText::new("Donnees cumulees depuis le dernier reset")
                .small()
                .color(theme::text_secondary(dark)),
        );
    });

    ui.add_space(8.0);

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

    ui.horizontal(|ui| {
        if ui.button("Rafraichir").clicked() {
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

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            egui::Grid::new("wakelocks_grid")
                .num_columns(2)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Package").strong());
                    ui.label(egui::RichText::new("Duree").strong());
                    ui.end_row();

                    for wl in &app.security_wakelocks {
                        ui.label(
                            egui::RichText::new(&wl.package).size(12.0),
                        );

                        // Color: >30min red, >5min orange, else normal
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
// Task 16: Device posture UI
// ═══════════════════════════════════════════════════════════════════
fn draw_posture(ui: &mut egui::Ui, app: &mut PhoneTvApp, ctx: &egui::Context) {
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

    // Auto-load on first visit
    if app.security_posture.is_empty() {
        trigger_posture_load(app, ctx, &device_id);
    }

    // Header with refresh
    ui.horizontal(|ui| {
        ui.heading(egui::RichText::new("Posture de l'appareil").size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Rafraichir").clicked() {
                trigger_posture_load(app, ctx, &device_id);
            }
        });
    });

    ui.add_space(8.0);

    if app.security_posture.is_empty() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Chargement...");
        });
        return;
    }

    let dark = app.dark_mode;
    let posture = app.security_posture.clone();

    // 2-column grid of status cards
    egui::Grid::new("posture_grid")
        .num_columns(2)
        .spacing([12.0, 12.0])
        .show(ui, |ui| {
            for (i, check) in posture.iter().enumerate() {
                card_frame(dark).show(ui, |ui| {
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
                        ui.painter().circle_filled(
                            rect.center(),
                            4.0,
                            dot_color,
                        );

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
                                    if ui.small_button("Corriger").clicked() {
                                        let tx = app.bg_tx.clone();
                                        let ctx2 = ctx.clone();
                                        let dev = device_id.clone();
                                        let cmd =
                                            check.fix_command.clone().unwrap();
                                        std::thread::spawn(move || {
                                            let success =
                                                crate::security::posture::fix_setting(
                                                    &dev, &cmd,
                                                );
                                            let _ = tx.send(BgEvent::Log(format!(
                                                "Fix {}: {}",
                                                cmd,
                                                if success { "OK" } else { "FAIL" }
                                            )));
                                            // Refresh posture
                                            let checks =
                                                crate::security::posture::check_device_posture(
                                                    &dev,
                                                );
                                            let _ = tx.send(
                                                BgEvent::SecurityPosture { checks },
                                            );
                                            ctx2.request_repaint();
                                        });
                                    }
                                },
                            );
                        }
                    });
                });

                // End row every 2 items
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}

fn trigger_posture_load(app: &mut PhoneTvApp, ctx: &egui::Context, device_id: &str) {
    let tx = app.bg_tx.clone();
    let ctx2 = ctx.clone();
    let id = device_id.to_string();
    std::thread::spawn(move || {
        let checks = crate::security::posture::check_device_posture(&id);
        let _ = tx.send(BgEvent::SecurityPosture { checks });
        ctx2.request_repaint();
    });
}
