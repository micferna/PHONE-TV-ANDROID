use eframe::egui;

// ── Dark palette ────────────────────────────────────────────────────
const DARK_BG: egui::Color32 = egui::Color32::from_rgb(0x0d, 0x11, 0x17);
const DARK_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(0x16, 0x1b, 0x22);
const DARK_CARD: egui::Color32 = egui::Color32::from_rgb(0x1c, 0x21, 0x28);
const DARK_CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(0x30, 0x36, 0x3d);
const DARK_WIDGET_BG: egui::Color32 = egui::Color32::from_rgb(0x21, 0x26, 0x2d);
const DARK_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(0x30, 0x36, 0x3d);
const DARK_WIDGET_ACTIVE: egui::Color32 = egui::Color32::from_rgb(0x1a, 0x3a, 0x5c);
const DARK_TEXT: egui::Color32 = egui::Color32::from_rgb(0xe6, 0xed, 0xf3);
const DARK_TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0x8b, 0x94, 0x9e);
const DARK_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x48, 0x4f, 0x58);

// ── Light palette ───────────────────────────────────────────────────
const LIGHT_BG: egui::Color32 = egui::Color32::from_rgb(0xf0, 0xf2, 0xf5);
const LIGHT_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(0xe4, 0xe7, 0xeb);
const LIGHT_CARD: egui::Color32 = egui::Color32::from_rgb(0xff, 0xff, 0xff);
const LIGHT_CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(0xd0, 0xd7, 0xde);
const LIGHT_WIDGET_BG: egui::Color32 = egui::Color32::from_rgb(0xe4, 0xe7, 0xeb);
const LIGHT_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(0xd0, 0xd7, 0xde);
const LIGHT_WIDGET_ACTIVE: egui::Color32 = egui::Color32::from_rgb(0xb0, 0xc4, 0xe0);
const LIGHT_TEXT: egui::Color32 = egui::Color32::from_rgb(0x1f, 0x23, 0x28);
const LIGHT_TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(0x65, 0x6d, 0x76);

// ── Accent colors ───────────────────────────────────────────────────
const ACCENT_BLUE: egui::Color32 = egui::Color32::from_rgb(0x58, 0xa6, 0xff);
const ACCENT_SUCCESS: egui::Color32 = egui::Color32::from_rgb(0x39, 0xd3, 0x53);
const ACCENT_WARNING: egui::Color32 = egui::Color32::from_rgb(0xd2, 0x99, 0x22);
const ACCENT_DANGER: egui::Color32 = egui::Color32::from_rgb(0xf8, 0x51, 0x49);
const ACCENT_PURPLE: egui::Color32 = egui::Color32::from_rgb(0xbc, 0x8c, 0xff);

// ── Public color functions ──────────────────────────────────────────

pub fn accent_color() -> egui::Color32 {
    ACCENT_BLUE
}

pub fn accent_blue() -> egui::Color32 {
    ACCENT_BLUE
}

pub fn accent_purple() -> egui::Color32 {
    ACCENT_PURPLE
}

pub fn success_color() -> egui::Color32 {
    ACCENT_SUCCESS
}

pub fn warning_color() -> egui::Color32 {
    ACCENT_WARNING
}

pub fn danger_color() -> egui::Color32 {
    ACCENT_DANGER
}

pub fn sidebar_fill(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        DARK_SIDEBAR
    } else {
        LIGHT_SIDEBAR
    }
}

pub fn card_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        DARK_CARD
    } else {
        LIGHT_CARD
    }
}

pub fn card_border(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        DARK_CARD_BORDER
    } else {
        LIGHT_CARD_BORDER
    }
}

pub fn card_selected(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(0x1a, 0x3a, 0x5c)
    } else {
        egui::Color32::from_rgb(0xd0, 0xe0, 0xf8)
    }
}

pub fn text_primary(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        DARK_TEXT
    } else {
        LIGHT_TEXT
    }
}

pub fn text_secondary(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        DARK_TEXT_SECONDARY
    } else {
        LIGHT_TEXT_SECONDARY
    }
}

pub fn text_dim(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        DARK_TEXT_DIM
    } else {
        LIGHT_TEXT_SECONDARY
    }
}

pub fn widget_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        DARK_WIDGET_BG
    } else {
        LIGHT_WIDGET_BG
    }
}

// ── Theme application ───────────────────────────────────────────────

pub fn apply_theme(ctx: &egui::Context, dark_mode: bool) {
    let mut style = (*ctx.global_style()).clone();

    if dark_mode {
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = DARK_BG;
        style.visuals.panel_fill = DARK_CARD;
        style.visuals.extreme_bg_color = DARK_WIDGET_BG;
        style.visuals.widgets.noninteractive.bg_fill = DARK_CARD;
        style.visuals.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0, DARK_TEXT_SECONDARY);
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, DARK_CARD_BORDER);
        style.visuals.widgets.inactive.bg_fill = DARK_WIDGET_BG;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, DARK_TEXT);
        style.visuals.widgets.hovered.bg_fill = DARK_WIDGET_HOVER;
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.widgets.active.bg_fill = DARK_WIDGET_ACTIVE;
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.selection.bg_fill = ACCENT_BLUE;
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BLUE);
    } else {
        style.visuals = egui::Visuals::light();
        style.visuals.window_fill = LIGHT_BG;
        style.visuals.panel_fill = LIGHT_CARD;
        style.visuals.extreme_bg_color = LIGHT_WIDGET_BG;
        style.visuals.widgets.noninteractive.bg_fill = LIGHT_CARD;
        style.visuals.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0, LIGHT_TEXT_SECONDARY);
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LIGHT_CARD_BORDER);
        style.visuals.widgets.inactive.bg_fill = LIGHT_WIDGET_BG;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.widgets.hovered.bg_fill = LIGHT_WIDGET_HOVER;
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.widgets.active.bg_fill = LIGHT_WIDGET_ACTIVE;
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(0xb0, 0xc8, 0xf0);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BLUE);
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

    ctx.set_global_style(style);
}
