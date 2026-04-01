mod adb;
mod app;
mod config;
mod pentest;
mod theme;
mod types;
mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let settings = config::load_settings();
    let win_size = settings.window_size;
    let dark_mode = settings.dark_mode;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([win_size.0, win_size.1])
            .with_title("Phone-TV"),
        ..Default::default()
    };

    eframe::run_native(
        "Phone-TV",
        options,
        Box::new(move |cc| {
            theme::apply_theme(&cc.egui_ctx, dark_mode);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::PhoneTvApp::new(settings)))
        }),
    )
}
