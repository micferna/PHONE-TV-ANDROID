use eframe::egui;

use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::{DeviceType, Tab};

pub fn draw_devices(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.add_space(4.0);

    // Header with action buttons
    ui.horizontal(|ui| {
        ui.heading("Appareils");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let scan_text = if app.scanning {
                "⏳ Scan..."
            } else {
                "🔍 Scanner Réseau"
            };
            if ui
                .add_enabled(!app.scanning, egui::Button::new(scan_text))
                .clicked()
            {
                app.scan_network_async(ctx);
            }

            let refresh_text = if app.refreshing {
                "⏳ Actualiser..."
            } else {
                "🔄 Actualiser"
            };
            if ui
                .add_enabled(
                    !app.refreshing,
                    egui::Button::new(refresh_text).fill(theme::accent_color()),
                )
                .clicked()
            {
                app.refresh_async(ctx);
            }
        });
    });

    ui.add_space(8.0);

    // Network scan results
    if !app.network_devices.is_empty() {
        egui::Frame::NONE
            .corner_radius(8.0)
            .inner_margin(10.0)
            .fill(theme::card_bg(app.dark_mode))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new("📡 Appareils réseau détectés")
                        .strong()
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.add_space(4.0);

                let mut to_connect: Option<String> = None;
                for ip in &app.network_devices {
                    ui.horizontal(|ui| {
                        ui.label(format!("  {} :5555", ip));
                        if ui
                            .add_enabled(!app.connecting, egui::Button::new("Connecter"))
                            .clicked()
                        {
                            to_connect = Some(ip.clone());
                        }
                    });
                }
                if let Some(ip) = to_connect {
                    let addr = format!("{}:5555", ip);
                    app.connect_wifi_async(addr, ctx);
                }
            });
        ui.add_space(6.0);
    }

    // Manual IP connection
    egui::Frame::NONE
        .corner_radius(8.0)
        .inner_margin(10.0)
        .fill(theme::card_bg(app.dark_mode))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Connexion manuelle").strong());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("IP:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.manual_ip)
                        .hint_text("192.168.1.x")
                        .desired_width(150.0),
                );
                let can_connect = !app.manual_ip.is_empty() && !app.connecting;
                if ui
                    .add_enabled(can_connect, egui::Button::new("➕ Connecter"))
                    .clicked()
                {
                    let addr = if app.manual_ip.contains(':') {
                        app.manual_ip.clone()
                    } else {
                        format!("{}:5555", app.manual_ip)
                    };
                    app.manual_ip.clear();
                    app.connect_wifi_async(addr, ctx);
                }
            });
        });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // Device list
    if app.devices.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("⚠ Aucun appareil détecté")
                    .size(16.0)
                    .color(theme::warning_color()),
            );
            ui.add_space(8.0);
            ui.label("Connectez un téléphone/TV en USB");
            ui.label("ou scannez le réseau pour trouver les TV");
        });
    } else {
        let mut new_selection: Option<(usize, DeviceType)> = None;

        for (i, device) in app.devices.iter().enumerate() {
            let is_selected = app.selected_device == Some(i);
            let is_connected = device.status == "device";

            let fill = if is_selected {
                theme::card_selected(app.dark_mode)
            } else {
                theme::card_bg(app.dark_mode)
            };

            let frame = egui::Frame::NONE
                .corner_radius(8.0)
                .inner_margin(12.0)
                .fill(fill)
                .stroke(if is_selected {
                    egui::Stroke::new(1.5, theme::accent_color())
                } else {
                    egui::Stroke::NONE
                });

            frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (icon, type_str) = match device.device_type {
                        DeviceType::Phone => ("📱", "Phone"),
                        DeviceType::Tv => ("📺", "TV"),
                        DeviceType::Unknown => ("❓", "?"),
                    };

                    // Icon + name
                    let btn_text = format!("{} {} [{}]", icon, device.name, type_str);
                    let btn = egui::Button::new(egui::RichText::new(&btn_text).size(14.0).strong())
                        .fill(egui::Color32::TRANSPARENT);

                    if ui.add(btn).clicked() {
                        new_selection = Some((i, device.device_type.clone()));
                    }

                    // Status indicator on right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (status_color, status_text) = if is_connected {
                            (theme::success_color(), "connecté")
                        } else {
                            (theme::warning_color(), "offline")
                        };
                        ui.label(egui::RichText::new("●").color(status_color));
                        ui.label(
                            egui::RichText::new(status_text)
                                .small()
                                .color(status_color),
                        );
                    });
                });
            });
            ui.add_space(4.0);
        }

        if let Some((idx, dtype)) = new_selection {
            app.selected_device = Some(idx);
            let name = app.devices[idx].name.clone();
            app.log(&format!("→ {}", name));
            match dtype {
                DeviceType::Tv => app.active_tab = Tab::Tv,
                DeviceType::Phone => app.active_tab = Tab::Phone,
                _ => {}
            }
        }
    }
}
