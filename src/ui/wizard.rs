use eframe::egui;
use crate::app::PhoneTvApp;
use crate::theme;
use crate::wizard;
use crate::wizard::types::{WizardStep, CleanAction, VulnFix};
use crate::brands::types::CleanProfile;
use crate::types::BgEvent;

pub fn draw_wizard(app: &mut PhoneTvApp, ctx: &egui::Context) {
    if !app.wizard.active {
        return;
    }

    let screen = ctx.screen_rect();

    // Dark overlay background
    egui::Area::new(egui::Id::new("wizard_overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_black_alpha(200));
        });

    // Wizard card
    let card_size = egui::vec2(800.0, 600.0);
    let card_pos = egui::pos2(
        (screen.width() - card_size.x) / 2.0,
        (screen.height() - card_size.y) / 2.0,
    );

    egui::Area::new(egui::Id::new("wizard_card"))
        .fixed_pos(card_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(theme::card_bg(app.dark_mode))
                .stroke(egui::Stroke::new(1.0, theme::card_border(app.dark_mode)))
                .rounding(12.0)
                .inner_margin(20.0)
                .show(ui, |ui| {
                    ui.set_min_size(card_size - egui::vec2(40.0, 40.0));
                    ui.set_max_width(card_size.x - 40.0);

                    draw_header(app, ui);
                    ui.add_space(16.0);

                    egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                        match app.wizard.step.clone() {
                            WizardStep::Detection => draw_step_detection(app, ui, ctx),
                            WizardStep::Scanning => draw_step_scanning(app, ui, ctx),
                            WizardStep::Pentest => draw_step_pentest(app, ui, ctx),
                            WizardStep::ProfileSelection => draw_step_profile(app, ui, ctx),
                            WizardStep::AiAnalysis => draw_step_ai(app, ui, ctx),
                            WizardStep::Cleaning => draw_step_cleaning(app, ui, ctx),
                            WizardStep::Report => draw_step_report(app, ui, ctx),
                        }
                    });
                });
        });
}

fn draw_header(app: &mut PhoneTvApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading(egui::RichText::new("Assistant Nettoyage").size(20.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("X Fermer").clicked() {
                app.wizard.stop();
            }
        });
    });

    ui.add_space(8.0);
    let steps = ["Detection", "Scan", "Pentest", "Profil", "IA", "Nettoyage", "Rapport"];
    let current = match app.wizard.step {
        WizardStep::Detection => 0,
        WizardStep::Scanning => 1,
        WizardStep::Pentest => 2,
        WizardStep::ProfileSelection => 3,
        WizardStep::AiAnalysis => 4,
        WizardStep::Cleaning => 5,
        WizardStep::Report => 6,
    };

    ui.horizontal(|ui| {
        for (i, step) in steps.iter().enumerate() {
            let color = if i < current {
                theme::success_color()
            } else if i == current {
                theme::accent_blue()
            } else {
                theme::text_secondary(app.dark_mode)
            };
            ui.label(egui::RichText::new(*step).size(12.0).color(color).strong());
            if i < steps.len() - 1 {
                ui.label(
                    egui::RichText::new(" > ")
                        .size(12.0)
                        .color(theme::text_secondary(app.dark_mode)),
                );
            }
        }
    });
}

fn draw_step_detection(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading(egui::RichText::new("Detection de l'appareil").size(16.0));
    ui.add_space(8.0);

    if app.wizard.device_info.is_none() {
        // Show spinner and auto-trigger detection
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Detection de l'appareil en cours...");
        });

        // Trigger detection only once
        if let Some(device_id) = app.get_selected_id() {
            if !app.wizard.detection_triggered {
                app.wizard.detection_triggered = true;
                let tx = app.bg_tx.clone();
                let ctx_clone = ctx.clone();
                wizard::trigger_detection(&device_id, &tx, &ctx_clone);
            }
        } else {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Aucun appareil connecte. Connectez un telephone Android.")
                    .color(theme::warning_color()),
            );
        }
        return;
    }

    if let Some(info) = &app.wizard.device_info.clone() {
        egui::Frame::none()
            .fill(theme::widget_bg(app.dark_mode))
            .rounding(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                egui::Grid::new("device_info_grid")
                    .num_columns(2)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Marque").color(theme::text_secondary(app.dark_mode)),
                        );
                        ui.label(egui::RichText::new(&info.brand).strong());
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Modele").color(theme::text_secondary(app.dark_mode)),
                        );
                        ui.label(egui::RichText::new(&info.model).strong());
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Android").color(theme::text_secondary(app.dark_mode)),
                        );
                        ui.label(egui::RichText::new(&info.android_version).strong());
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("SDK").color(theme::text_secondary(app.dark_mode)),
                        );
                        ui.label(egui::RichText::new(info.sdk.to_string()).strong());
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Patch securite")
                                .color(theme::text_secondary(app.dark_mode)),
                        );
                        ui.label(egui::RichText::new(&info.security_patch).strong());
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Serie").color(theme::text_secondary(app.dark_mode)),
                        );
                        ui.label(egui::RichText::new(&info.serial).strong());
                        ui.end_row();
                    });
            });

        // Show history info if available
        if let Some(history) = &app.wizard.history {
            ui.add_space(12.0);
            egui::Frame::none()
                .fill(theme::widget_bg(app.dark_mode))
                .rounding(8.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Historique")
                            .size(14.0)
                            .strong()
                            .color(theme::accent_blue()),
                    );
                    ui.add_space(4.0);
                    ui.label(format!("Premiere detection: {}", history.first_seen));
                    ui.label(format!("Nombre de sessions: {}", history.sessions.len()));
                    if let Some(last) = history.sessions.last() {
                        ui.label(format!("Derniere session: {}", last.date));
                        ui.label(format!(
                            "Score: {} -> {}",
                            last.score_before, last.score_after
                        ));
                    }
                });
        }

        ui.add_space(16.0);
        if ui
            .add(egui::Button::new(
                egui::RichText::new("Lancer le scan complet").size(14.0).strong(),
            ))
            .clicked()
        {
            app.wizard.step = WizardStep::Scanning;
        }
    }
}

fn draw_step_scanning(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading(egui::RichText::new("Scan de l'appareil").size(16.0));
    ui.add_space(8.0);

    if app.wizard.scan_loading {
        let current = app.wizard.scan_current;
        let total = app.wizard.scan_total;
        let progress = app.wizard.scan_progress;

        ui.label(egui::RichText::new("Scan en cours...").size(16.0).strong());
        ui.add_space(12.0);

        // Toujours afficher la barre
        if total > 0 {
            // Barre avec compteur dedans
            let bar_text = format!("{} / {} applications ({:.0}%)", current, total, progress * 100.0);
            ui.add(egui::ProgressBar::new(progress)
                .text(bar_text)
                .desired_width(ui.available_width()));
        } else {
            // Barre indéterminée animée pendant le chargement initial
            ui.add(egui::ProgressBar::new(0.0)
                .text("Recuperation de la liste des packages...")
                .desired_width(ui.available_width())
                .animate(true));
        }

        ui.add_space(10.0);

        // Package en cours d'analyse — toujours visible
        ui.horizontal(|ui| {
            ui.spinner();
            let pkg_text = if app.wizard.scan_current_package.is_empty() {
                "Connexion a l'appareil...".to_string()
            } else {
                app.wizard.scan_current_package.clone()
            };
            ui.label(
                egui::RichText::new(pkg_text)
                    .size(13.0)
                    .family(egui::FontFamily::Monospace)
                    .color(theme::accent_blue()),
            );
        });

        // Estimation du temps restant
        if total > 0 && current > 0 {
            let remaining = total - current;
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("~{} applications restantes", remaining))
                    .size(11.0)
                    .color(theme::text_secondary(app.dark_mode)),
            );
        }

        return;
    }

    // If scan not started yet, trigger it
    if app.wizard.score_before.is_none() && app.wizard.apps.is_empty() {
        if let Some(device_id) = app.get_selected_id() {
            app.wizard.scan_loading = true;
            let tx = app.bg_tx.clone();
            wizard::trigger_scan(&device_id, &tx, ctx);
        } else {
            ui.label(
                egui::RichText::new("Aucun appareil connecte.")
                    .color(theme::danger_color()),
            );
        }
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Demarrage du scan...");
        });
        return;
    }

    // Show scan results
    if let Some((score, issues)) = &app.wizard.score_before {
        egui::Frame::none()
            .fill(theme::widget_bg(app.dark_mode))
            .rounding(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Resultats du scan").size(14.0).strong(),
                );
                ui.add_space(4.0);

                let score_color = if *score >= 70 {
                    theme::success_color()
                } else if *score >= 40 {
                    theme::warning_color()
                } else {
                    theme::danger_color()
                };

                ui.label(
                    egui::RichText::new(format!("Score securite: {}/100", score))
                        .size(18.0)
                        .strong()
                        .color(score_color),
                );
                ui.add_space(4.0);
                ui.label(format!("Applications trouvees: {}", app.wizard.apps.len()));
                ui.label(format!("Problemes detectes: {}", issues.len()));
                ui.label(format!("Verifications posture: {}", app.wizard.posture.len()));
            });

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Scan termine. Lancement du pentest...")
                .color(theme::text_secondary(app.dark_mode)),
        );

        // Auto-trigger pentest
        if !app.wizard.pentest_loading && app.wizard.vulns.is_empty() && app.wizard.risk_score.is_none() {
            if let Some(device_id) = app.get_selected_id() {
                app.wizard.pentest_loading = true;
                let tx = app.bg_tx.clone();
                wizard::trigger_pentest(&device_id, &tx, ctx);
            }
        }
    }
}

fn draw_step_pentest(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading(egui::RichText::new("Analyse de vulnerabilites").size(16.0));
    ui.add_space(8.0);

    if app.wizard.pentest_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Analyse securite en cours...");
        });
        return;
    }

    // Auto-trigger if not started
    if app.wizard.risk_score.is_none() {
        if let Some(device_id) = app.get_selected_id() {
            app.wizard.pentest_loading = true;
            let tx = app.bg_tx.clone();
            wizard::trigger_pentest(&device_id, &tx, ctx);
        }
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Demarrage du pentest...");
        });
        return;
    }

    // Show results
    if let Some(risk_score) = app.wizard.risk_score {
        let score_color = if risk_score >= 70 {
            theme::success_color()
        } else if risk_score >= 40 {
            theme::warning_color()
        } else {
            theme::danger_color()
        };

        egui::Frame::none()
            .fill(theme::widget_bg(app.dark_mode))
            .rounding(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("Score de risque: {}/100", risk_score))
                        .size(18.0)
                        .strong()
                        .color(score_color),
                );

                if let Some(root_status) = &app.wizard.root_status {
                    ui.add_space(4.0);
                    if root_status.is_rooted {
                        let method = root_status.method.as_deref().unwrap_or("inconnu");
                        ui.label(
                            egui::RichText::new(format!("ROOTE: {}", method))
                                .color(theme::danger_color())
                                .strong(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Non roote")
                                .color(theme::success_color()),
                        );
                    }
                    if root_status.bootloader_unlocked {
                        ui.label(
                            egui::RichText::new("Bootloader deverrouille")
                                .color(theme::warning_color()),
                        );
                    }

                    // Rootability info
                    ui.add_space(8.0);
                    match &root_status.rootable {
                        Some(true) => {
                            ui.label(
                                egui::RichText::new(format!("Rootable: OUI — {}",
                                    root_status.root_method.as_deref().unwrap_or("methode inconnue")))
                                    .color(theme::accent_blue())
                                    .strong(),
                            );
                        }
                        Some(false) => {
                            ui.label(
                                egui::RichText::new("Rootable: NON (ou pas de methode connue)")
                                    .color(theme::text_secondary(app.dark_mode)),
                            );
                        }
                        None => {
                            if !app.settings.openrouter_api_key.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Rootable: ?").color(theme::warning_color()));
                                    if ui.button("Verifier via IA").clicked() {
                                        if let Some(ref info) = app.wizard.device_info.clone() {
                                            let api_key = app.settings.openrouter_api_key.clone();
                                            let model = app.settings.llm_model.clone();
                                            let brand = info.brand.clone();
                                            let dev_model = info.model.clone();
                                            let android = info.android_version.clone();
                                            let patch = info.security_patch.clone();
                                            let tx = app.bg_tx.clone();
                                            let ctx2 = ctx.clone();
                                            std::thread::spawn(move || {
                                                match crate::llm::check_rootability(&api_key, &model, &brand, &dev_model, &android, &patch) {
                                                    Ok(result) => {
                                                        let _ = tx.send(BgEvent::Log(format!(
                                                            "Rootable: {} (confiance: {}) — {}",
                                                            if result.rootable { "OUI" } else { "NON" },
                                                            result.confidence,
                                                            result.details
                                                        )));
                                                        let _ = tx.send(BgEvent::WizardRootabilityResult {
                                                            rootable: result.rootable,
                                                            method: result.method,
                                                            confidence: result.confidence,
                                                            details: result.details,
                                                        });
                                                    }
                                                    Err(e) => {
                                                        let _ = tx.send(BgEvent::LlmError { message: e });
                                                    }
                                                }
                                                ctx2.request_repaint();
                                            });
                                        }
                                    }
                                });
                            } else {
                                ui.label(
                                    egui::RichText::new("Rootable: ? (configurez l'IA pour verifier)")
                                        .color(theme::text_secondary(app.dark_mode)),
                                );
                            }
                        }
                    }
                }
            });

        if !app.wizard.vulns.is_empty() {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(format!("{} vulnerabilite(s) detectee(s)", app.wizard.vulns.len()))
                    .size(14.0)
                    .strong(),
            );
            ui.add_space(4.0);

            for vuln in &app.wizard.vulns {
                use crate::types::Severity;
                let sev_color = match vuln.severity {
                    Severity::Critical => theme::danger_color(),
                    Severity::Warning => theme::warning_color(),
                    Severity::Info => theme::accent_blue(),
                };
                egui::Frame::none()
                    .fill(theme::widget_bg(app.dark_mode))
                    .rounding(6.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let sev_label = match vuln.severity {
                                Severity::Critical => "CRITIQUE",
                                Severity::Warning => "ATTENTION",
                                Severity::Info => "INFO",
                            };
                            ui.label(
                                egui::RichText::new(sev_label)
                                    .size(11.0)
                                    .strong()
                                    .color(sev_color),
                            );
                            ui.label(egui::RichText::new(&vuln.description).size(13.0));
                        });
                        if !vuln.risk_if_unpatched.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("Risque: {}", vuln.risk_if_unpatched))
                                    .size(11.0)
                                    .color(theme::text_secondary(app.dark_mode)),
                            );
                        }
                    });
                ui.add_space(4.0);
            }
        } else {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Aucune vulnerabilite critique detectee.")
                    .color(theme::success_color()),
            );
        }

        ui.add_space(16.0);
        if ui
            .add(egui::Button::new(
                egui::RichText::new("Choisir le profil de nettoyage")
                    .size(14.0)
                    .strong(),
            ))
            .clicked()
        {
            // Build vuln fixes from patchable vulns
            let vuln_fixes: Vec<VulnFix> = app.wizard.vulns.iter()
                .filter(|v| v.patchable && v.fix_action.is_some())
                .map(|v| VulnFix {
                    vuln_id: v.id.clone(),
                    description: v.description.clone(),
                    fix_command: v.fix_action.clone().unwrap_or_default(),
                    selected: true,
                })
                .collect();
            app.wizard.vuln_fixes = vuln_fixes;

            // Build clean actions
            let (actions, unknown) = wizard::build_clean_actions(
                &app.wizard.apps.clone(),
                app.wizard.brand_db.as_ref(),
                &app.wizard.selected_profile.clone(),
            );
            app.wizard.clean_actions = actions;
            app.wizard.unknown_apps = unknown;
            app.wizard.step = WizardStep::ProfileSelection;
        }
    }
}

fn draw_step_profile(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.heading(egui::RichText::new("Profil de nettoyage").size(16.0));
    ui.add_space(8.0);

    // Profile selection buttons
    ui.horizontal(|ui| {
        let profiles = [
            (CleanProfile::Minimal, "Minimal", "Apps clairement inutiles"),
            (CleanProfile::Moderate, "Modere", "Apps suspectes + inutiles"),
            (CleanProfile::Aggressive, "Agressif", "Tout le bloatware"),
        ];

        for (profile, label, desc) in &profiles {
            let selected = &app.wizard.selected_profile == profile;
            let btn_color = if selected {
                theme::accent_blue()
            } else {
                theme::text_secondary(app.dark_mode)
            };

            let btn = egui::Button::new(
                egui::RichText::new(format!("{}\n{}", label, desc))
                    .size(12.0)
                    .color(btn_color),
            );

            if ui.add(btn).clicked() && !selected {
                app.wizard.selected_profile = profile.clone();
                // Rebuild actions with new profile
                let (actions, unknown) = wizard::build_clean_actions(
                    &app.wizard.apps.clone(),
                    app.wizard.brand_db.as_ref(),
                    &app.wizard.selected_profile.clone(),
                );
                app.wizard.clean_actions = actions;
                app.wizard.unknown_apps = unknown;
            }
        }
    });

    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(format!(
            "{} action(s) selectionnee(s)",
            app.wizard.clean_actions.iter().filter(|a| a.selected).count()
        ))
        .size(14.0)
        .strong(),
    );

    if !app.wizard.clean_actions.is_empty() {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Applications a supprimer:")
                .size(13.0)
                .color(theme::text_secondary(app.dark_mode)),
        );

        let actions_clone: Vec<CleanAction> = app.wizard.clean_actions.clone();
        for (i, action) in actions_clone.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut selected = action.selected;
                if ui.checkbox(&mut selected, "").changed() {
                    app.wizard.clean_actions[i].selected = selected;
                }
                ui.label(egui::RichText::new(&action.package).size(12.0));
                if !action.description.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("({})", action.description))
                            .size(11.0)
                            .color(theme::text_secondary(app.dark_mode)),
                    );
                }
                if action.from_ai {
                    ui.label(
                        egui::RichText::new("[IA]")
                            .size(11.0)
                            .color(theme::accent_blue()),
                    );
                }
            });
        }
    }

    if !app.wizard.vuln_fixes.is_empty() {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Corrections de vulnerabilites:")
                .size(13.0)
                .color(theme::text_secondary(app.dark_mode)),
        );

        let fixes_clone: Vec<VulnFix> = app.wizard.vuln_fixes.clone();
        for (i, fix) in fixes_clone.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut selected = fix.selected;
                if ui.checkbox(&mut selected, "").changed() {
                    app.wizard.vuln_fixes[i].selected = selected;
                }
                ui.label(egui::RichText::new(&fix.description).size(12.0));
            });
        }
    }

    ui.add_space(16.0);
    ui.horizontal(|ui| {
        if !app.wizard.unknown_apps.is_empty() {
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(format!(
                        "Analyser {} apps inconnues avec IA",
                        app.wizard.unknown_apps.len()
                    ))
                    .size(13.0),
                ))
                .clicked()
            {
                app.wizard.step = WizardStep::AiAnalysis;
            }
            ui.add_space(8.0);
        }

        if ui
            .add(egui::Button::new(
                egui::RichText::new("Lancer le nettoyage").size(14.0).strong(),
            ))
            .clicked()
        {
            app.wizard.step = WizardStep::Cleaning;
        }
    });
}

fn draw_step_ai(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading(egui::RichText::new("Analyse IA des applications").size(16.0));
    ui.add_space(8.0);

    // Check API key
    if app.settings.openrouter_api_key.is_empty() {
        egui::Frame::none()
            .fill(theme::widget_bg(app.dark_mode))
            .rounding(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Cle API OpenRouter non configuree.")
                        .color(theme::warning_color())
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Configurez votre cle API dans les parametres pour utiliser l'IA.",
                    )
                    .color(theme::text_secondary(app.dark_mode)),
                );
            });

        ui.add_space(12.0);
        if ui
            .add(egui::Button::new(
                egui::RichText::new("Passer (sans IA)").size(13.0),
            ))
            .clicked()
        {
            app.wizard.step = WizardStep::Cleaning;
        }
        return;
    }

    if app.wizard.ai_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Analyse IA en cours...");
        });
        return;
    }

    // No verdicts yet - show trigger button
    if app.wizard.ai_verdicts.is_empty() {
        ui.label(format!(
            "{} application(s) inconnue(s) a analyser.",
            app.wizard.unknown_apps.len()
        ));
        ui.add_space(8.0);

        if ui
            .add(egui::Button::new(
                egui::RichText::new("Lancer l'analyse IA").size(14.0).strong(),
            ))
            .clicked()
        {
            let api_key = app.settings.openrouter_api_key.clone();
            let model = app.settings.llm_model.clone();
            let unknown_apps = app.wizard.unknown_apps.clone();
            let apps = app.wizard.apps.clone();
            let tx = app.bg_tx.clone();
            let ctx_clone = ctx.clone();

            app.wizard.ai_loading = true;

            std::thread::spawn(move || {
                // Build (package, permissions, installer) tuples for unknown apps
                let app_data: Vec<(String, Vec<String>, String)> = apps
                    .iter()
                    .filter(|a| unknown_apps.contains(&a.package))
                    .map(|a| {
                        (
                            a.package.clone(),
                            a.dangerous_perm_names.clone(),
                            format!("{:?}", a.installer),
                        )
                    })
                    .collect();

                match crate::llm::analyze_apps(&api_key, &model, &app_data) {
                    Ok(verdicts) => {
                        let _ = tx.send(BgEvent::LlmAppVerdicts { verdicts });
                    }
                    Err(e) => {
                        let _ = tx.send(BgEvent::LlmError { message: e });
                    }
                }
                ctx_clone.request_repaint();
            });
        }

        ui.add_space(8.0);
        if ui
            .add(egui::Button::new(
                egui::RichText::new("Passer (sans IA)").size(13.0),
            ))
            .clicked()
        {
            app.wizard.step = WizardStep::Cleaning;
        }
        return;
    }

    // Show verdicts
    ui.label(
        egui::RichText::new(format!(
            "{} verdict(s) recus",
            app.wizard.ai_verdicts.len()
        ))
        .size(14.0)
        .strong(),
    );
    ui.add_space(8.0);

    let verdicts_clone: Vec<(String, crate::llm::types::AppVerdict)> = app.wizard.ai_verdicts.clone();
    for (_, verdict) in &verdicts_clone {
        let v_lower = verdict.verdict.to_lowercase();
        let verdict_color = if v_lower.contains("bloatware") || v_lower.contains("danger") || v_lower.contains("malware") {
            theme::danger_color()
        } else if v_lower.contains("suspect") || v_lower.contains("unknown") {
            theme::warning_color()
        } else {
            theme::success_color()
        };

        egui::Frame::none()
            .fill(theme::widget_bg(app.dark_mode))
            .rounding(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&verdict.verdict)
                            .size(11.0)
                            .strong()
                            .color(verdict_color),
                    );
                    ui.label(egui::RichText::new(&verdict.package).size(12.0));
                    ui.label(
                        egui::RichText::new(format!("[{}]", verdict.category))
                            .size(11.0)
                            .color(theme::text_secondary(app.dark_mode)),
                    );
                });
                if !verdict.explanation.is_empty() {
                    ui.label(
                        egui::RichText::new(&verdict.explanation)
                            .size(11.0)
                            .color(theme::text_secondary(app.dark_mode)),
                    );
                }
            });
        ui.add_space(4.0);
    }

    ui.add_space(16.0);
    if ui
        .add(egui::Button::new(
            egui::RichText::new("Ajouter le bloatware identifie et continuer")
                .size(14.0)
                .strong(),
        ))
        .clicked()
    {
        // Add bloatware verdicts as clean actions
        for (_, verdict) in &app.wizard.ai_verdicts {
            let v_lower = verdict.verdict.to_lowercase();
            if v_lower.contains("bloatware") || v_lower.contains("danger") || v_lower.contains("malware") {
                let already_exists = app.wizard.clean_actions.iter().any(|a| a.package == verdict.package);
                if !already_exists {
                    app.wizard.clean_actions.push(CleanAction {
                        package: verdict.package.clone(),
                        action: "uninstall".into(),
                        description: verdict.explanation.clone(),
                        selected: true,
                        from_ai: true,
                    });
                }
            }
        }
        app.wizard.step = WizardStep::Cleaning;
    }
}

fn draw_step_cleaning(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading(egui::RichText::new("Nettoyage").size(16.0));
    ui.add_space(8.0);

    if !app.wizard.cleaning && app.wizard.clean_results.is_empty() {
        // Show recap before starting
        let selected_actions = app.wizard.clean_actions.iter().filter(|a| a.selected).count();
        let selected_fixes = app.wizard.vuln_fixes.iter().filter(|f| f.selected).count();

        egui::Frame::none()
            .fill(theme::widget_bg(app.dark_mode))
            .rounding(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Recapitulatif").size(14.0).strong(),
                );
                ui.add_space(4.0);
                ui.label(format!("{} application(s) a supprimer/desactiver", selected_actions));
                ui.label(format!("{} correction(s) de securite a appliquer", selected_fixes));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Cette operation ne peut pas etre annulee facilement.",
                    )
                    .size(12.0)
                    .color(theme::warning_color()),
                );
            });

        ui.add_space(16.0);
        if ui
            .add(egui::Button::new(
                egui::RichText::new("Lancer le nettoyage").size(14.0).strong(),
            ))
            .clicked()
        {
            if let Some(device_id) = app.get_selected_id() {
                let total = app.wizard.clean_actions.iter().filter(|a| a.selected).count()
                    + app.wizard.vuln_fixes.iter().filter(|f| f.selected).count();
                app.wizard.clean_total = total;
                app.wizard.clean_progress = 0;
                app.wizard.cleaning = true;

                let tx = app.bg_tx.clone();
                wizard::trigger_cleaning(
                    &device_id,
                    &app.wizard.clean_actions,
                    &app.wizard.vuln_fixes,
                    &tx,
                    ctx,
                );
            }
        }
        return;
    }

    // Show progress
    if app.wizard.clean_total > 0 {
        let progress = app.wizard.clean_progress as f32 / app.wizard.clean_total as f32;
        let bar_text = format!("{} / {} ({:.0}%)", app.wizard.clean_progress, app.wizard.clean_total, progress * 100.0);
        ui.add(egui::ProgressBar::new(progress)
            .text(bar_text)
            .desired_width(ui.available_width()));
        ui.add_space(8.0);
    }

    if app.wizard.cleaning {
        ui.horizontal(|ui| {
            ui.spinner();
            // Show the last action being processed
            if let Some(last) = app.wizard.clean_results.last() {
                ui.label(
                    egui::RichText::new(format!("En cours: {}", last.package))
                        .size(12.0)
                        .family(egui::FontFamily::Monospace)
                        .color(theme::text_secondary(app.dark_mode)),
                );
            } else {
                ui.label("Demarrage du nettoyage...");
            }
        });
        ui.add_space(8.0);
    }

    // Show results list
    for result in &app.wizard.clean_results {
        let (icon, color) = if result.success {
            ("OK", theme::success_color())
        } else {
            ("ECHEC", theme::danger_color())
        };

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(icon)
                    .size(11.0)
                    .strong()
                    .color(color),
            );
            ui.label(egui::RichText::new(&result.package).size(12.0));
            ui.label(
                egui::RichText::new(format!("[{}]", result.action))
                    .size(11.0)
                    .color(theme::text_secondary(app.dark_mode)),
            );
            if !result.message.is_empty() {
                ui.label(
                    egui::RichText::new(&result.message)
                        .size(11.0)
                        .color(theme::text_secondary(app.dark_mode)),
                );
            }
        });
    }
}

fn draw_step_report(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.heading(egui::RichText::new("Rapport de nettoyage").size(16.0));
    ui.add_space(8.0);

    // Scores
    egui::Frame::none()
        .fill(theme::widget_bg(app.dark_mode))
        .rounding(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Scores").size(14.0).strong());
            ui.add_space(4.0);

            if let Some((score_before, _)) = &app.wizard.score_before {
                let score_after_val = app.wizard.score_after.as_ref().map(|(s, _)| *s).unwrap_or(*score_before);
                let diff: i32 = score_after_val as i32 - *score_before as i32;
                let diff_color = if diff >= 0 { theme::success_color() } else { theme::danger_color() };

                ui.horizontal(|ui| {
                    ui.label(format!("Score securite: {} -> {}", score_before, score_after_val));
                    ui.label(
                        egui::RichText::new(format!("({:+})", diff))
                            .color(diff_color)
                            .strong(),
                    );
                });
            }

            if let Some(risk_before) = app.wizard.risk_score {
                let risk_after = app.wizard.risk_score_after.unwrap_or(risk_before);
                let diff: i32 = risk_after as i32 - risk_before as i32;
                let diff_color = if diff >= 0 { theme::success_color() } else { theme::danger_color() };

                ui.horizontal(|ui| {
                    ui.label(format!("Score de risque: {} -> {}", risk_before, risk_after));
                    ui.label(
                        egui::RichText::new(format!("({:+})", diff))
                            .color(diff_color)
                            .strong(),
                    );
                });
            }
        });

    ui.add_space(8.0);

    // Success/failure counts
    let successes = app.wizard.clean_results.iter().filter(|r| r.success).count();
    let failures = app.wizard.clean_results.iter().filter(|r| !r.success).count();

    egui::Frame::none()
        .fill(theme::widget_bg(app.dark_mode))
        .rounding(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Resultats").size(14.0).strong());
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("{} operation(s) reussie(s)", successes))
                    .color(theme::success_color()),
            );
            if failures > 0 {
                ui.label(
                    egui::RichText::new(format!("{} operation(s) echouee(s)", failures))
                        .color(theme::danger_color()),
                );
            }
        });

    // Apps removed/disabled summary
    let removed: Vec<&str> = app.wizard.clean_results.iter()
        .filter(|r| r.success && r.action == "uninstall")
        .map(|r| r.package.as_str())
        .collect();
    let disabled: Vec<&str> = app.wizard.clean_results.iter()
        .filter(|r| r.success && r.action == "disable")
        .map(|r| r.package.as_str())
        .collect();

    if !removed.is_empty() || !disabled.is_empty() {
        ui.add_space(8.0);
        egui::Frame::none()
            .fill(theme::widget_bg(app.dark_mode))
            .rounding(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Applications traitees").size(14.0).strong());
                ui.add_space(4.0);
                if !removed.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("Supprimees ({}): {}", removed.len(), removed.join(", ")))
                            .size(12.0)
                            .color(theme::text_secondary(app.dark_mode)),
                    );
                }
                if !disabled.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("Desactivees ({}): {}", disabled.len(), disabled.join(", ")))
                            .size(12.0)
                            .color(theme::text_secondary(app.dark_mode)),
                    );
                }
            });
    }

    ui.add_space(16.0);
    if ui
        .add(egui::Button::new(
            egui::RichText::new("Sauvegarder et fermer").size(14.0).strong(),
        ))
        .clicked()
    {
        // Save to history
        if let Some(info) = &app.wizard.device_info.clone() {
            let score_before = app.wizard.score_before.as_ref().map(|(s, _)| *s).unwrap_or(0);
            let score_after = app.wizard.score_after.as_ref().map(|(s, _)| *s).unwrap_or(score_before);
            let risk_before = app.wizard.risk_score.unwrap_or(0);
            let risk_after = app.wizard.risk_score_after.unwrap_or(risk_before);

            let apps_removed: Vec<String> = app.wizard.clean_results.iter()
                .filter(|r| r.success && r.action == "uninstall")
                .map(|r| r.package.clone())
                .collect();
            let apps_disabled: Vec<String> = app.wizard.clean_results.iter()
                .filter(|r| r.success && r.action == "disable")
                .map(|r| r.package.clone())
                .collect();
            let apps_failed: Vec<String> = app.wizard.clean_results.iter()
                .filter(|r| !r.success)
                .map(|r| r.package.clone())
                .collect();

            let vulns_found = app.wizard.vulns.len() as u32;
            let vulns_patched = app.wizard.clean_results.iter()
                .filter(|r| r.success && r.action == "fix")
                .count() as u32;
            let ai_suggestions_accepted = app.wizard.clean_actions.iter()
                .filter(|a| a.from_ai && a.selected)
                .count() as u32;

            let profile_used = match &app.wizard.selected_profile {
                CleanProfile::Minimal => "minimal",
                CleanProfile::Moderate => "moderate",
                CleanProfile::Aggressive => "aggressive",
            };

            let session = crate::history::types::CleanSession {
                date: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
                android_version: info.android_version.clone(),
                security_patch: info.security_patch.clone(),
                score_before,
                score_after,
                risk_score_before: risk_before,
                risk_score_after: risk_after,
                apps_removed: apps_removed.clone(),
                apps_disabled: apps_disabled.clone(),
                apps_failed,
                vulns_found,
                vulns_patched,
                profile_used: profile_used.to_string(),
                ai_suggestions_accepted,
            };

            // Create or update history
            if app.wizard.history.is_some() {
                crate::history::add_session(&info.serial, session);
            } else {
                let mut new_history = crate::history::create_history(
                    &info.serial,
                    &info.brand,
                    &info.model,
                    &info.display_name,
                );
                new_history.sessions.push(session);
                crate::history::save_history(&new_history);
            }

            // Add AI verdicts to brands DB
            if !app.wizard.ai_verdicts.is_empty() {
                for (_, verdict) in &app.wizard.ai_verdicts {
                    let v_lower = verdict.verdict.to_lowercase();
                    if v_lower.contains("bloatware") || v_lower.contains("danger") {
                        let profile = match verdict.profile.to_lowercase().as_str() {
                            "minimal" => CleanProfile::Minimal,
                            "aggressive" => CleanProfile::Aggressive,
                            _ => CleanProfile::Moderate,
                        };
                        let entry = crate::brands::types::BloatwareEntry {
                            package: verdict.package.clone(),
                            category: verdict.category.clone(),
                            profile,
                            description: verdict.explanation.clone(),
                        };
                        crate::brands::add_entry(&info.brand, entry);
                    }
                }
            }
        }

        app.wizard.stop();
    }
}
