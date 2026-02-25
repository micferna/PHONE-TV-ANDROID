use eframe::egui;

// Dark palette
const DARK_BG: egui::Color32 = egui::Color32::from_rgb(26, 26, 46);
const DARK_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(22, 33, 62);
const DARK_PANEL: egui::Color32 = egui::Color32::from_rgb(30, 30, 50);
const DARK_WIDGET_BG: egui::Color32 = egui::Color32::from_rgb(40, 42, 65);
const DARK_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(50, 55, 80);
const DARK_WIDGET_ACTIVE: egui::Color32 = egui::Color32::from_rgb(15, 52, 96);
const DARK_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 230, 240);
const DARK_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(150, 155, 175);

// Light palette
const LIGHT_BG: egui::Color32 = egui::Color32::from_rgb(245, 245, 250);
const LIGHT_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(235, 235, 245);
const LIGHT_PANEL: egui::Color32 = egui::Color32::from_rgb(250, 250, 255);
const LIGHT_WIDGET_BG: egui::Color32 = egui::Color32::from_rgb(225, 225, 235);
const LIGHT_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(210, 215, 230);
const LIGHT_WIDGET_ACTIVE: egui::Color32 = egui::Color32::from_rgb(180, 195, 220);
const LIGHT_TEXT: egui::Color32 = egui::Color32::from_rgb(30, 30, 40);
const LIGHT_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(100, 100, 120);

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(15, 52, 96);
pub const ACCENT_BRIGHT: egui::Color32 = egui::Color32::from_rgb(233, 69, 96);

pub fn accent_color() -> egui::Color32 {
    ACCENT_BRIGHT
}

pub fn success_color() -> egui::Color32 {
    egui::Color32::from_rgb(40, 167, 69)
}

pub fn warning_color() -> egui::Color32 {
    egui::Color32::from_rgb(200, 160, 30)
}

pub fn danger_color() -> egui::Color32 {
    egui::Color32::from_rgb(200, 40, 40)
}

pub fn sidebar_fill(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_SIDEBAR } else { LIGHT_SIDEBAR }
}

pub fn card_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(35, 38, 58)
    } else {
        egui::Color32::from_rgb(240, 240, 248)
    }
}

pub fn card_selected(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(30, 45, 85)
    } else {
        egui::Color32::from_rgb(210, 225, 250)
    }
}

pub fn apply_theme(ctx: &egui::Context, dark_mode: bool) {
    let mut style = (*ctx.style()).clone();

    if dark_mode {
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = DARK_BG;
        style.visuals.panel_fill = DARK_PANEL;
        style.visuals.extreme_bg_color = DARK_WIDGET_BG;
        style.visuals.widgets.noninteractive.bg_fill = DARK_PANEL;
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, DARK_TEXT_DIM);
        style.visuals.widgets.inactive.bg_fill = DARK_WIDGET_BG;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, DARK_TEXT);
        style.visuals.widgets.hovered.bg_fill = DARK_WIDGET_HOVER;
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.widgets.active.bg_fill = DARK_WIDGET_ACTIVE;
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BRIGHT);
    } else {
        style.visuals = egui::Visuals::light();
        style.visuals.window_fill = LIGHT_BG;
        style.visuals.panel_fill = LIGHT_PANEL;
        style.visuals.extreme_bg_color = LIGHT_WIDGET_BG;
        style.visuals.widgets.noninteractive.bg_fill = LIGHT_PANEL;
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT_DIM);
        style.visuals.widgets.inactive.bg_fill = LIGHT_WIDGET_BG;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.widgets.hovered.bg_fill = LIGHT_WIDGET_HOVER;
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.widgets.active.bg_fill = LIGHT_WIDGET_ACTIVE;
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(180, 200, 240);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    }

    // Common styling
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    let cr = egui::CornerRadius::same(6);
    style.visuals.widgets.noninteractive.corner_radius = cr;
    style.visuals.widgets.inactive.corner_radius = cr;
    style.visuals.widgets.hovered.corner_radius = cr;
    style.visuals.widgets.active.corner_radius = cr;
    style.visuals.window_corner_radius = egui::CornerRadius::same(8);

    ctx.set_style(style);
}
