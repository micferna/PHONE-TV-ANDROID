use eframe::egui;

use crate::adb;
use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::BgEvent;

/// Capture, display and save a device screenshot. Shared by phone and TV views.
pub fn screenshot_panel(
    app: &mut PhoneTvApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    device_id: &str,
) {
    let taking = app.screenshot_loading;
    let btn_text = if taking {
        "⏳ Capture..."
    } else {
        "📸 Capturer"
    };

    if ui
        .add_enabled(
            !taking,
            egui::Button::new(btn_text)
                .fill(theme::accent_color())
                .corner_radius(8.0)
                .min_size(egui::vec2(ui.available_width(), 32.0)),
        )
        .clicked()
    {
        app.screenshot_loading = true;
        let id_clone = device_id.to_string();
        let tx = app.bg_tx.clone();
        std::thread::spawn(move || {
            if let Some(data) = adb::take_screenshot(&id_clone) {
                let _ = tx.send(BgEvent::ScreenshotReady {
                    device_id: id_clone,
                    data,
                });
            } else {
                let _ = tx.send(BgEvent::Log("Échec capture".into()));
            }
        });
    }

    if let Some(ref data) = app.screenshot {
        ui.add_space(4.0);
        let uri = "bytes://device_screenshot.png";
        ctx.include_bytes(uri, data.clone());
        ui.add(
            egui::Image::new(uri)
                .max_width(ui.available_width())
                .corner_radius(6.0),
        );

        ui.add_space(4.0);
        if ui
            .add(
                egui::Button::new("💾 Sauvegarder")
                    .corner_radius(8.0)
                    .min_size(egui::vec2(ui.available_width(), 28.0)),
            )
            .clicked()
        {
            let default_name = format!(
                "screenshot_{}.png",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name(&default_name)
                .save_file()
            {
                match std::fs::write(&path, data) {
                    Ok(_) => app.log(&format!("Capture sauvegardée: {}", path.display())),
                    Err(e) => app.log(&format!("Erreur sauvegarde: {}", e)),
                }
            }
        }
    }
}
