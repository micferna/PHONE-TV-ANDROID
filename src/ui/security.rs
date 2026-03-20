use eframe::egui;

use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::SecurityView;

pub fn draw_security(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
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
        SecurityView::Score => draw_score_stub(ui, app),
        SecurityView::Apps => draw_apps_stub(ui, app),
        SecurityView::Permissions => draw_permissions_stub(ui, app),
        SecurityView::Blacklist => draw_blacklist_stub(ui, app),
        SecurityView::Monitoring => draw_monitoring_stub(ui, app),
        SecurityView::Posture => draw_posture_stub(ui, app),
    }
}

fn draw_score_stub(ui: &mut egui::Ui, _app: &PhoneTvApp) {
    ui.label(egui::RichText::new("Security Score — coming soon").italics());
}

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
