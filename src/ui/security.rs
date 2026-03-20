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
        SecurityView::Apps => draw_apps_stub(ui, app),
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

// ── Stubs for remaining views ───────────────────────────────────────
fn draw_apps_stub(ui: &mut egui::Ui, _app: &PhoneTvApp) {
    ui.label(egui::RichText::new("Apps management — coming soon").italics());
}

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
