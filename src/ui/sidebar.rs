use eframe::egui;

use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::{DeviceType, Tab};

pub fn draw_sidebar(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.vertical(|ui| {
        ui.add_space(12.0);

        // App title
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Phone-TV")
                    .size(20.0)
                    .strong()
                    .color(theme::accent_color()),
            );
            ui.label(
                egui::RichText::new("v5.0.0")
                    .size(11.0)
                    .color(egui::Color32::GRAY),
            );
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // Device selector
        ui.label(
            egui::RichText::new("Appareil")
                .size(11.0)
                .color(egui::Color32::GRAY),
        );
        ui.add_space(2.0);

        if app.devices.is_empty() {
            ui.label(
                egui::RichText::new("Aucun appareil")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        } else {
            let current_label = app
                .get_selected()
                .map(|d| {
                    let icon = match d.device_type {
                        DeviceType::Phone => "📱",
                        DeviceType::Tv => "📺",
                        DeviceType::Unknown => "❓",
                    };
                    format!("{} {}", icon, d.name)
                })
                .unwrap_or_else(|| "Sélectionner...".into());

            egui::ComboBox::from_id_salt("device_selector")
                .selected_text(&current_label)
                .width(ui.available_width() - 8.0)
                .show_ui(ui, |ui| {
                    for (i, device) in app.devices.iter().enumerate() {
                        let icon = match device.device_type {
                            DeviceType::Phone => "📱",
                            DeviceType::Tv => "📺",
                            DeviceType::Unknown => "❓",
                        };
                        let status_dot_color = if device.status == "device" {
                            theme::success_color()
                        } else {
                            theme::danger_color()
                        };
                        let status_dot =
                            egui::RichText::new("●").color(status_dot_color).size(10.0);
                        let label_text = format!("{} {}", icon, device.name);
                        let selected = app.selected_device == Some(i);
                        let response = ui.horizontal(|ui| {
                            let clicked = ui.selectable_label(selected, &label_text).clicked();
                            ui.label(status_dot);
                            clicked
                        });
                        if response.inner {
                            app.selected_device = Some(i);
                            // Auto-switch tab
                            match device.device_type {
                                DeviceType::Tv => app.active_tab = Tab::Tv,
                                DeviceType::Phone => app.active_tab = Tab::Phone,
                                _ => {}
                            }
                        }
                    }
                });
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // Navigation tabs
        let tabs = [
            (Tab::Devices, "📡  Appareils"),
            (Tab::Phone, "📱  Phone"),
            (Tab::Tv, "📺  TV"),
            (Tab::Video, "🎬  Vidéo"),
            (Tab::Security, "🛡  Sécurité"),
            (Tab::Audit, "🧹  Audit & Nettoyage"),
        ];

        for (tab, label) in tabs {
            let enabled = app.tab_enabled(tab);
            let selected = app.active_tab == tab;

            let text = egui::RichText::new(label).size(14.0);
            let text = if !enabled {
                text.color(egui::Color32::from_rgb(80, 80, 100))
            } else if selected {
                text.strong().color(theme::accent_color())
            } else {
                text
            };

            let btn = egui::Button::new(text)
                .min_size(egui::vec2(ui.available_width(), 36.0))
                .corner_radius(8.0);

            let btn = if selected {
                btn.fill(theme::card_selected(app.dark_mode))
            } else {
                btn.fill(egui::Color32::TRANSPARENT)
            };

            let response = ui.add_enabled(enabled, btn);

            // Draw accent bar on the left for selected tab
            if selected {
                let rect = response.rect;
                let accent_rect =
                    egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height()));
                ui.painter().rect_filled(
                    accent_rect,
                    egui::CornerRadius::same(1),
                    theme::accent_color(),
                );
            }

            if response.clicked() {
                app.active_tab = tab;
            }
        }

        // Push the rest to the bottom
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(8.0);

            // Dark/Light toggle
            let toggle_text = if app.dark_mode {
                "☀ Mode clair"
            } else {
                "🌙 Mode sombre"
            };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(toggle_text).size(12.0))
                        .min_size(egui::vec2(ui.available_width(), 30.0))
                        .fill(egui::Color32::TRANSPARENT),
                )
                .clicked()
            {
                app.dark_mode = !app.dark_mode;
                theme::apply_theme(ctx, app.dark_mode);
                app.save_settings();
            }

            ui.add_space(4.0);

            // Stop all button
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("🛑 STOP TOUT")
                            .strong()
                            .size(13.0)
                            .color(egui::Color32::WHITE),
                    )
                    .min_size(egui::vec2(ui.available_width(), 34.0))
                    .fill(theme::danger_color()),
                )
                .clicked()
            {
                app.stop_all();
                app.log("Tout stoppé");
            }
        });
    });
}
