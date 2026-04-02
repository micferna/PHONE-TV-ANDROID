# Phone-TV v5.0 — Security Tab & Full UI Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a comprehensive Security tab for Android device auditing/management and redesign all UI with a dashboard monitoring aesthetic.

**Architecture:** The security backend is split into `src/security/` submodules (score, apps, permissions, monitoring, posture) that parse ADB output and return typed data. All ADB calls happen in background threads via the existing `bg_tx`/`bg_rx` mpsc pattern. The UI is in `src/ui/security.rs` with 6 sub-views. The theme overhaul in `src/theme.rs` affects all tabs.

**Tech Stack:** Rust, eframe/egui 0.33, ADB shell commands, TOML/text config files, rfd (file dialogs)

**Spec:** `docs/superpowers/specs/2026-03-20-security-tab-and-ui-redesign.md`

---

## Milestone 1: Foundation — Types, Theme, Plumbing

### Task 1: Add new types to types.rs

**Files:**
- Modify: `src/types.rs`

- [ ] **Step 1: Add Tab::Security variant**

In `src/types.rs`, add `Security` to the `Tab` enum after `Video`:

```rust
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Devices,
    Tv,
    Phone,
    Video,
    Security,
}
```

- [ ] **Step 2: Add all security data types**

Append these types at the end of `src/types.rs` (after the existing `TvShell` struct):

```rust
// === Security types ===

#[derive(Clone, Debug, Default)]
pub struct AppInfo {
    pub package: String,
    pub version_name: String,
    pub version_code: u32,
    pub first_install: String,
    pub last_update: String,
    pub installer: AppInstaller,
    pub target_sdk: u32,
    pub enabled: bool,
    pub details_loaded: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum AppInstaller {
    PlayStore,
    Sideload,
    Adb,
    #[default]
    Unknown,
}

#[derive(Clone, Debug)]
pub struct PermissionInfo {
    pub name: String,
    pub granted: bool,
    pub last_used: Option<String>,
    pub dangerous: bool,
    pub is_runtime: bool,
}

#[derive(Clone, Debug)]
pub struct SecurityIssue {
    pub id: String,
    pub description: String,
    pub severity: Severity,
    pub points: i32,
    pub fixable: bool,
    pub fix_command: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub package: String,
    pub pid: u32,
    pub memory_kb: u64,
    pub adj: i32,
    pub state: String,
}

#[derive(Clone, Debug)]
pub struct DataUsage {
    pub package: String,
    pub uid: u32,
    pub wifi_rx: u64,
    pub wifi_tx: u64,
    pub mobile_rx: u64,
    pub mobile_tx: u64,
}

#[derive(Clone, Debug)]
pub struct WakelockInfo {
    pub package: String,
    pub duration_ms: u64,
    pub duration_human: String,
}

#[derive(Clone, Debug)]
pub struct DevicePosture {
    pub name: String,
    pub value: String,
    pub status: PostureStatus,
    pub fix_command: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PostureStatus {
    Good,
    Warning,
    Bad,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SecurityView {
    Score,
    Apps,
    Permissions,
    Blacklist,
    Monitoring,
    Posture,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PermissionView {
    ByPermission,
    ByApp,
}

#[derive(Clone, Copy, PartialEq)]
pub enum MonitoringView {
    Processes,
    DataUsage,
    Wakelocks,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AppFilter {
    All,
    ThirdParty,
    System,
    Disabled,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AppSort {
    Name,
    InstallDate,
    Source,
}
```

- [ ] **Step 3: Add new BgEvent variants**

In the existing `BgEvent` enum in `src/types.rs`, add these variants:

```rust
SecurityScore { score: u8, issues: Vec<SecurityIssue> },
SecurityAppsList { packages: Vec<String> },
SecurityAppDetail { package: String, info: AppInfo },
SecurityProcesses { processes: Vec<ProcessInfo> },
SecurityDataUsage { usage: Vec<DataUsage> },
SecurityWakelocks { wakelocks: Vec<WakelockInfo> },
SecurityPosture { checks: Vec<DevicePosture> },
SecurityPermissions { package: String, permissions: Vec<PermissionInfo> },
BlacklistAlert { found: Vec<String> },
AppActionResult { package: String, action: String, success: bool, message: String },
SecurityAppsLoadingDone,
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: Warnings about unused variants (expected at this stage), but no errors.

- [ ] **Step 5: Commit**

```bash
git add src/types.rs
git commit -m "feat: add security data types, Tab::Security, and BgEvent variants"
```

---

### Task 2: Theme system overhaul

**Files:**
- Modify: `src/theme.rs`

- [ ] **Step 1: Replace the entire color palette and add new helper functions**

Rewrite `src/theme.rs` with the new dashboard monitoring palette. Keep the same function signatures so existing UI code doesn't break. Key changes:
- Dark palette: `#0d1117` background, `#161b22` sidebar, `#1c2128` cards
- Light palette: `#f0f2f5` background, `#e4e7eb` sidebar, `#ffffff` cards
- New accent colors: `accent_blue (#58a6ff)`, `accent_cyan (#39d353)`, `accent_orange (#d29922)`, `accent_red (#f85149)`, `accent_purple (#bc8cff)`
- New helper functions: `card_border()`, `text_primary()`, `text_secondary()`, `text_dim()`, `widget_bg()`, `accent_blue()`, `accent_purple()`
- Existing functions `accent_color()`, `success_color()`, `warning_color()`, `danger_color()`, `sidebar_fill()`, `card_bg()`, `card_selected()`, `apply_theme()` keep same names but updated colors
- In `apply_theme()`: add `style.visuals.widgets.noninteractive.bg_stroke` for subtle card borders

```rust
use eframe::egui;

// Dark palette (dashboard monitoring style)
const DARK_BG: egui::Color32 = egui::Color32::from_rgb(13, 17, 23);
const DARK_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(22, 27, 34);
const DARK_CARD: egui::Color32 = egui::Color32::from_rgb(28, 33, 40);
const DARK_CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(48, 54, 61);
const DARK_WIDGET_BG: egui::Color32 = egui::Color32::from_rgb(33, 38, 45);
const DARK_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(37, 44, 53);
const DARK_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 237, 243);
const DARK_TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(139, 148, 158);
const DARK_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(72, 79, 88);

// Light palette
const LIGHT_BG: egui::Color32 = egui::Color32::from_rgb(240, 242, 245);
const LIGHT_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(228, 231, 235);
const LIGHT_CARD: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
const LIGHT_CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(208, 215, 222);
const LIGHT_WIDGET_BG: egui::Color32 = egui::Color32::from_rgb(225, 228, 232);
const LIGHT_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(210, 215, 222);
const LIGHT_TEXT: egui::Color32 = egui::Color32::from_rgb(31, 35, 40);
const LIGHT_TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(101, 109, 118);
const LIGHT_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(150, 158, 168);

// Accent colors (both modes)
pub const ACCENT_BLUE: egui::Color32 = egui::Color32::from_rgb(88, 166, 255);
pub const ACCENT_CYAN: egui::Color32 = egui::Color32::from_rgb(57, 211, 83);
pub const ACCENT_ORANGE: egui::Color32 = egui::Color32::from_rgb(210, 153, 34);
pub const ACCENT_RED: egui::Color32 = egui::Color32::from_rgb(248, 81, 73);
pub const ACCENT_PURPLE: egui::Color32 = egui::Color32::from_rgb(188, 140, 255);

pub fn accent_color() -> egui::Color32 { ACCENT_BLUE }
pub fn accent_blue() -> egui::Color32 { ACCENT_BLUE }
pub fn accent_purple() -> egui::Color32 { ACCENT_PURPLE }
pub fn success_color() -> egui::Color32 { ACCENT_CYAN }
pub fn warning_color() -> egui::Color32 { ACCENT_ORANGE }
pub fn danger_color() -> egui::Color32 { ACCENT_RED }

pub fn sidebar_fill(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_SIDEBAR } else { LIGHT_SIDEBAR }
}

pub fn card_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_CARD } else { LIGHT_CARD }
}

pub fn card_border(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_CARD_BORDER } else { LIGHT_CARD_BORDER }
}

pub fn card_selected(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(32, 42, 58)
    } else {
        egui::Color32::from_rgb(210, 225, 250)
    }
}

pub fn widget_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_WIDGET_BG } else { LIGHT_WIDGET_BG }
}

pub fn text_primary(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_TEXT } else { LIGHT_TEXT }
}

pub fn text_secondary(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_TEXT_SECONDARY } else { LIGHT_TEXT_SECONDARY }
}

pub fn text_dim(dark_mode: bool) -> egui::Color32 {
    if dark_mode { DARK_TEXT_DIM } else { LIGHT_TEXT_DIM }
}

pub fn apply_theme(ctx: &egui::Context, dark_mode: bool) {
    let mut style = (*ctx.style()).clone();

    if dark_mode {
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = DARK_BG;
        style.visuals.panel_fill = DARK_BG;
        style.visuals.extreme_bg_color = DARK_WIDGET_BG;
        style.visuals.widgets.noninteractive.bg_fill = DARK_CARD;
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, DARK_TEXT_SECONDARY);
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, DARK_CARD_BORDER);
        style.visuals.widgets.inactive.bg_fill = DARK_WIDGET_BG;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, DARK_TEXT);
        style.visuals.widgets.hovered.bg_fill = DARK_WIDGET_HOVER;
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.widgets.active.bg_fill = ACCENT_BLUE;
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(23, 47, 82);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BLUE);
    } else {
        style.visuals = egui::Visuals::light();
        style.visuals.window_fill = LIGHT_BG;
        style.visuals.panel_fill = LIGHT_BG;
        style.visuals.extreme_bg_color = LIGHT_WIDGET_BG;
        style.visuals.widgets.noninteractive.bg_fill = LIGHT_CARD;
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT_SECONDARY);
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, LIGHT_CARD_BORDER);
        style.visuals.widgets.inactive.bg_fill = LIGHT_WIDGET_BG;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.widgets.hovered.bg_fill = LIGHT_WIDGET_HOVER;
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, LIGHT_TEXT);
        style.visuals.widgets.active.bg_fill = ACCENT_BLUE;
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(180, 200, 240);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BLUE);
    }

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
```

- [ ] **Step 2: Fix compilation — update references to removed constants**

The old `theme.rs` exported `ACCENT` and `ACCENT_BRIGHT`. Search for usages in all UI files and replace:
- `theme::ACCENT` → `theme::accent_color()` (used in `src/ui/devices.rs:34`, `src/ui/tv.rs:167,319,379`)
- `theme::ACCENT_BRIGHT` is no longer used (was only in `accent_color()`)

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: No errors. Visual changes will be visible at runtime.

- [ ] **Step 4: Commit**

```bash
git add src/theme.rs src/ui/devices.rs src/ui/tv.rs
git commit -m "feat: dashboard monitoring theme overhaul"
```

---

### Task 3: App state + BgEvent handlers + blacklist config

**Files:**
- Modify: `src/app.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Add blacklist load/save to config.rs**

Add to `src/config.rs` after the `save_channels` function:

```rust
fn blacklist_path() -> PathBuf {
    config_dir().join("blacklist.txt")
}

pub fn load_blacklist() -> Vec<String> {
    std::fs::read_to_string(blacklist_path())
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn save_blacklist(blacklist: &[String]) {
    let content = blacklist.join("\n");
    let _ = std::fs::write(blacklist_path(), content);
}
```

- [ ] **Step 2: Add security fields to PhoneTvApp**

In `src/app.rs`, add these imports at the top:

```rust
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
```

Add these fields to the `PhoneTvApp` struct (after `phone_apps_loading`):

```rust
// Security
pub security_view: SecurityView,
pub security_score: Option<(u8, Vec<SecurityIssue>)>,
pub security_score_loading: bool,
pub security_apps: Vec<AppInfo>,
pub security_apps_filter: AppFilter,
pub security_apps_sort: AppSort,
pub security_apps_search: String,
pub security_apps_loading: bool,
pub security_loading_cancel: Arc<AtomicBool>,
pub security_permission_view: PermissionView,
pub security_permission_cache: HashMap<String, Vec<PermissionInfo>>,
pub security_selected_app: Option<String>,
pub security_monitoring_view: MonitoringView,
pub security_processes: Vec<ProcessInfo>,
pub security_data_usage: Vec<DataUsage>,
pub security_wakelocks: Vec<WakelockInfo>,
pub security_posture: Vec<DevicePosture>,
pub blacklist: Vec<String>,
pub blacklist_alerts: Vec<String>,
pub blacklist_new_entry: String,
```

And their initialization in `PhoneTvApp::new()`:

```rust
security_view: SecurityView::Score,
security_score: None,
security_score_loading: false,
security_apps: Vec::new(),
security_apps_filter: AppFilter::All,
security_apps_sort: AppSort::Name,
security_apps_search: String::new(),
security_apps_loading: false,
security_loading_cancel: Arc::new(AtomicBool::new(false)),
security_permission_view: PermissionView::ByPermission,
security_permission_cache: HashMap::new(),
security_selected_app: None,
security_monitoring_view: MonitoringView::Processes,
security_processes: Vec::new(),
security_data_usage: Vec::new(),
security_wakelocks: Vec::new(),
security_posture: Vec::new(),
blacklist: config::load_blacklist(),
blacklist_alerts: Vec::new(),
blacklist_new_entry: String::new(),
```

- [ ] **Step 3: Add Tab::Security to tab_enabled()**

In `src/app.rs`, update `tab_enabled()`:

```rust
pub fn tab_enabled(&self, tab: Tab) -> bool {
    match tab {
        Tab::Devices => true,
        Tab::Tv => self.get_selected().map(|d| d.device_type == DeviceType::Tv).unwrap_or(false),
        Tab::Phone => self.get_selected().map(|d| d.device_type == DeviceType::Phone).unwrap_or(false),
        Tab::Video => self.get_selected_id().is_some(),
        Tab::Security => self.get_selected_id().is_some(),
    }
}
```

- [ ] **Step 4: Add BgEvent handlers for all security events**

In `src/app.rs` in the `process_bg_events()` method, add match arms for each new variant:

```rust
BgEvent::SecurityScore { score, issues } => {
    self.security_score = Some((score, issues));
    self.security_score_loading = false;
}
BgEvent::SecurityAppsList { packages } => {
    self.security_apps = packages.into_iter().map(|p| AppInfo {
        package: p,
        ..Default::default()
    }).collect();
    self.security_apps_loading = true; // details still loading
}
BgEvent::SecurityAppsLoadingDone => {
    self.security_apps_loading = false;
}
BgEvent::SecurityAppDetail { package, info } => {
    if let Some(app) = self.security_apps.iter_mut().find(|a| a.package == package) {
        *app = info;
    }
}
BgEvent::SecurityProcesses { processes } => {
    self.security_processes = processes;
}
BgEvent::SecurityDataUsage { usage } => {
    self.security_data_usage = usage;
}
BgEvent::SecurityWakelocks { wakelocks } => {
    self.security_wakelocks = wakelocks;
}
BgEvent::SecurityPosture { checks } => {
    self.security_posture = checks;
}
BgEvent::SecurityPermissions { package, permissions } => {
    self.security_permission_cache.insert(package, permissions);
}
BgEvent::BlacklistAlert { found } => {
    self.blacklist_alerts = found;
}
BgEvent::AppActionResult { package, action, success, message } => {
    self.log(&message);
    if success && matches!(action.as_str(), "uninstall" | "disable" | "enable") {
        self.security_apps.retain(|a| !(a.package == package && action == "uninstall"));
        if action == "disable" {
            if let Some(a) = self.security_apps.iter_mut().find(|a| a.package == package) {
                a.enabled = false;
            }
        }
        if action == "enable" {
            if let Some(a) = self.security_apps.iter_mut().find(|a| a.package == package) {
                a.enabled = true;
            }
        }
    }
}
```

- [ ] **Step 5: Add Tab::Security to central panel match**

In `src/app.rs` in the `update()` method, in the central panel match:

```rust
Tab::Security => ui::draw_security(self, ui, ctx),
```

- [ ] **Step 6: Update use statements in app.rs**

Add to the `use crate::types::*;` imports (already using `*` so nothing needed) and ensure `SecurityView`, `AppFilter`, `AppSort`, `PermissionView`, `MonitoringView`, `AppInfo` are accessible.

- [ ] **Step 7: Verify it compiles**

Run: `cargo check`
Expected: Error about `ui::draw_security` not existing yet (we'll create it next). That's fine — temporarily comment out the `Tab::Security` match arm, verify rest compiles, then uncomment.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs src/config.rs
git commit -m "feat: add security state fields, BgEvent handlers, blacklist config"
```

---

### Task 4: Security module skeleton + UI stub

**Files:**
- Create: `src/security/mod.rs`
- Create: `src/security/score.rs`
- Create: `src/security/apps.rs`
- Create: `src/security/permissions.rs`
- Create: `src/security/monitoring.rs`
- Create: `src/security/posture.rs`
- Create: `src/ui/security.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create security module files with stubs**

Create `src/security/mod.rs`:
```rust
pub mod score;
pub mod apps;
pub mod permissions;
pub mod monitoring;
pub mod posture;
```

Create `src/security/score.rs`:
```rust
use crate::types::SecurityIssue;

pub fn calculate_score(device_id: &str) -> (u8, Vec<SecurityIssue>) {
    // TODO: implement
    (100, Vec::new())
}
```

Create `src/security/apps.rs`:
```rust
use crate::types::AppInfo;

pub fn list_packages(device_id: &str, filter: &str) -> Vec<String> {
    // TODO: implement
    Vec::new()
}

pub fn get_app_detail(device_id: &str, package: &str) -> Option<AppInfo> {
    // TODO: implement
    None
}

pub fn uninstall_app(device_id: &str, package: &str) -> (bool, String) {
    // TODO: implement
    (false, "Not implemented".into())
}

pub fn disable_app(device_id: &str, package: &str) -> (bool, String) {
    (false, "Not implemented".into())
}

pub fn enable_app(device_id: &str, package: &str) -> (bool, String) {
    (false, "Not implemented".into())
}

pub fn force_stop_app(device_id: &str, package: &str) {
    // TODO: implement
}

pub fn clear_app_data(device_id: &str, package: &str) -> (bool, String) {
    (false, "Not implemented".into())
}
```

Create `src/security/permissions.rs`:
```rust
use crate::types::PermissionInfo;

pub fn get_app_permissions(device_id: &str, package: &str) -> Vec<PermissionInfo> {
    // TODO: implement
    Vec::new()
}

pub fn revoke_permission(device_id: &str, package: &str, permission: &str) -> (bool, String) {
    (false, "Not implemented".into())
}
```

Create `src/security/monitoring.rs`:
```rust
use crate::types::{ProcessInfo, DataUsage, WakelockInfo};

pub fn get_running_processes(device_id: &str) -> Vec<ProcessInfo> {
    Vec::new()
}

pub fn get_data_usage(device_id: &str) -> Vec<DataUsage> {
    Vec::new()
}

pub fn get_wakelocks(device_id: &str) -> Vec<WakelockInfo> {
    Vec::new()
}
```

Create `src/security/posture.rs`:
```rust
use crate::types::DevicePosture;

pub fn check_device_posture(device_id: &str) -> Vec<DevicePosture> {
    Vec::new()
}

pub fn fix_setting(device_id: &str, command: &str) -> bool {
    false
}
```

- [ ] **Step 2: Create UI security stub**

Create `src/ui/security.rs`:
```rust
use eframe::egui;

use crate::app::PhoneTvApp;
use crate::theme;
use crate::types::SecurityView;

pub fn draw_security(app: &mut PhoneTvApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.add_space(4.0);

    // Sub-tab navigation
    ui.horizontal(|ui| {
        let tabs = [
            (SecurityView::Score, "🛡 Score"),
            (SecurityView::Apps, "📦 Apps"),
            (SecurityView::Permissions, "🔐 Permissions"),
            (SecurityView::Blacklist, "🚫 Blacklist"),
            (SecurityView::Monitoring, "📊 Monitoring"),
            (SecurityView::Posture, "⚙ Posture"),
        ];
        for (view, label) in tabs {
            let selected = app.security_view == view;
            let text = egui::RichText::new(label).size(13.0);
            let text = if selected { text.strong().color(theme::accent_color()) } else { text };
            let btn = egui::Button::new(text)
                .corner_radius(8.0)
                .fill(if selected { theme::card_selected(app.dark_mode) } else { egui::Color32::TRANSPARENT });
            if ui.add(btn).clicked() {
                app.security_view = view;
            }
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    match app.security_view {
        SecurityView::Score => draw_score(app, ui, ctx),
        SecurityView::Apps => draw_apps(app, ui, ctx),
        SecurityView::Permissions => draw_permissions(app, ui, ctx),
        SecurityView::Blacklist => draw_blacklist(app, ui, ctx),
        SecurityView::Monitoring => draw_monitoring(app, ui, ctx),
        SecurityView::Posture => draw_posture(app, ui, ctx),
    }
}

fn draw_score(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("Score de sécurité").strong().size(16.0));
        ui.add_space(16.0);
        ui.label(egui::RichText::new("En construction...").color(theme::text_secondary(app.dark_mode)));
    });
}

fn draw_apps(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.label(egui::RichText::new("Apps & Gestion").strong().size(16.0));
    ui.label(egui::RichText::new("En construction...").color(theme::text_secondary(app.dark_mode)));
}

fn draw_permissions(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.label(egui::RichText::new("Audit Permissions").strong().size(16.0));
    ui.label(egui::RichText::new("En construction...").color(theme::text_secondary(app.dark_mode)));
}

fn draw_blacklist(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.label(egui::RichText::new("Blacklist").strong().size(16.0));
    ui.label(egui::RichText::new("En construction...").color(theme::text_secondary(app.dark_mode)));
}

fn draw_monitoring(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.label(egui::RichText::new("Monitoring").strong().size(16.0));
    ui.label(egui::RichText::new("En construction...").color(theme::text_secondary(app.dark_mode)));
}

fn draw_posture(app: &mut PhoneTvApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.label(egui::RichText::new("Posture Sécurité").strong().size(16.0));
    ui.label(egui::RichText::new("En construction...").color(theme::text_secondary(app.dark_mode)));
}
```

- [ ] **Step 3: Register modules**

Update `src/ui/mod.rs`:
```rust
mod sidebar;
mod devices;
mod tv;
mod phone;
mod video;
mod security;

pub use sidebar::draw_sidebar;
pub use devices::draw_devices;
pub use tv::draw_tv;
pub use phone::draw_phone;
pub use video::draw_video;
pub use security::draw_security;
```

Update `src/main.rs` — add `mod security;` after `mod ui;`:
```rust
mod adb;
mod app;
mod config;
mod security;
mod theme;
mod types;
mod ui;
```

- [ ] **Step 4: Uncomment Tab::Security match arm in app.rs**

Now that `draw_security` exists, ensure the `Tab::Security => ui::draw_security(self, ui, ctx)` arm is active.

- [ ] **Step 5: Verify it compiles and runs**

Run: `cargo run`
Expected: App launches with new dark theme. Security tab appears in sidebar when a device is selected. Clicking it shows stub "En construction..." pages.

- [ ] **Step 6: Commit**

```bash
git add src/security/ src/ui/security.rs src/ui/mod.rs src/main.rs src/app.rs
git commit -m "feat: security module skeleton with stub UI and sub-tab navigation"
```

---

### Task 5: Sidebar redesign with Security tab

**Files:**
- Modify: `src/ui/sidebar.rs`

- [ ] **Step 1: Rewrite sidebar with new design**

Update `src/ui/sidebar.rs` to include:
- Version bumped to "v5.0.0"
- `Tab::Security` in the tabs array with label `"🛡  Sécurité"`
- Active tab highlighted with accent_blue left bar (use `egui::Frame` with `inner_margin` asymmetric left padding + left border via `stroke`)
- Device selector with status dot

Key change in the tabs array:
```rust
let tabs = [
    (Tab::Devices, "📡  Appareils"),
    (Tab::Phone, "📱  Phone"),
    (Tab::Tv, "📺  TV"),
    (Tab::Video, "🎬  Vidéo"),
    (Tab::Security, "🛡  Sécurité"),
];
```

And the selected tab styling — replace the current `btn.fill(theme::card_selected())` with a left-accent bar approach:
```rust
if selected {
    ui.horizontal(|ui| {
        // Blue accent bar on left
        let (rect, _) = ui.allocate_exact_size(egui::vec2(3.0, 36.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, theme::accent_color());
        // Button
        let btn = egui::Button::new(text)
            .min_size(egui::vec2(ui.available_width(), 36.0))
            .corner_radius(8.0)
            .fill(theme::card_selected(app.dark_mode));
        if ui.add_enabled(enabled, btn).clicked() {
            app.active_tab = tab;
        }
    });
} else {
    // ... normal button
}
```

- [ ] **Step 2: Enhance device selector card**

In the device selector section of the sidebar, add a status dot and mini battery bar:
- After device name in the combobox, add a colored dot: `●` green if `device.status == "device"`, red otherwise
- If `app.phone_battery.is_some()` for the selected device, show a small progress bar (50px wide, 6px tall) colored by level

- [ ] **Step 3: Verify it compiles and test visually**

Run: `cargo run`
Expected: Sidebar shows Security tab. Active tab has blue left bar. Device selector shows status dot. Version shows v5.0.0.

- [ ] **Step 4: Commit**

```bash
git add src/ui/sidebar.rs
git commit -m "feat: sidebar redesign with Security tab, accent indicators, and device status"
```

---

## Milestone 2: Security Backend — ADB Parsing

### Task 6: Security score calculation

**Files:**
- Modify: `src/security/score.rs`

- [ ] **Step 1: Implement score calculation with all ADB checks**

Replace the stub in `src/security/score.rs` with full implementation:
- Use `crate::adb::adb_device()` for `settings get` commands
- Parse each setting, create `SecurityIssue` entries
- Apply `max(0, 100 - total_deductions)` clamping
- Handle missing/null settings gracefully (treat as "can't determine" → no deduction)

The function signature stays the same. Implement ALL 6 checks from spec section 2.1:

1. **Unknown sources** (-20): Android <8: `settings get secure install_non_market_apps` → "1" = enabled. Android 8+: this may return "null", in which case skip (no reliable fallback without per-app check).
2. **Developer mode** (-10): `settings get global development_settings_enabled` → "1" = on.
3. **Play Protect** (-25): `settings get global package_verifier_enable` → "0" or "null" = disabled.
4. **Accessibility services** (-15 total cap): `settings get secure enabled_accessibility_services` → non-empty means services active. Deduct -15 regardless of count.
5. **Sideloaded apps** (-3 each, max -15): `pm list packages -3 -i` → parse each line's `installer=` field. Count apps where installer is not `com.android.vending` or `com.google.android.packageinstaller`.
6. **Apps with 3+ dangerous permissions** (-2 each, max -10): For each third-party app, run `dumpsys package <pkg>` and count `granted=true` runtime permissions that are in the dangerous list. Count apps with 3+.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`

- [ ] **Step 3: Commit**

```bash
git add src/security/score.rs
git commit -m "feat: security score calculation with ADB settings checks"
```

---

### Task 7: App listing and detail parsing

**Files:**
- Modify: `src/security/apps.rs`

- [ ] **Step 1: Implement list_packages()**

Parse output of `pm list packages -3` (or `-s`, `-d` based on filter param). Strip `package:` prefix.

- [ ] **Step 2: Implement get_app_detail()**

Run `dumpsys package <pkg>` and parse:
- `versionName=X` → version_name
- `versionCode=X` → version_code
- `firstInstallTime=X` → first_install
- `lastUpdateTime=X` → last_update
- `installerPackageName=X` → map to AppInstaller enum
- `targetSdk=X` → target_sdk
- `enabled=X` → enabled (look for `User 0:` section, `enabled=` field)

Use line-by-line parsing with `line.trim().starts_with()` pattern matching.

- [ ] **Step 3: Implement action functions**

`uninstall_app`: run `adb -s {id} shell pm uninstall {pkg}`, check stdout for "Success".
`disable_app`: run `pm disable-user --user 0 {pkg}`, check for "disabled".
`enable_app`: run `pm enable {pkg}`, check for "enabled".
`force_stop_app`: run `am force-stop {pkg}`, fire-and-forget.
`clear_app_data`: run `pm clear {pkg}`, check output. Return warning message about data loss.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`

- [ ] **Step 5: Commit**

```bash
git add src/security/apps.rs
git commit -m "feat: app listing and detail parsing from dumpsys package"
```

---

### Task 8: Permission parsing and appops

**Files:**
- Modify: `src/security/permissions.rs`

- [ ] **Step 1: Implement get_app_permissions()**

Run `dumpsys package <pkg>` and parse the `runtime permissions:` section. For each line like:
```
android.permission.CAMERA: granted=true, flags=[ USER_SET ]
```
Extract permission name, granted status. Mark as `dangerous` if in the known dangerous list (CAMERA, MICROPHONE, ACCESS_FINE_LOCATION, READ_CONTACTS, READ_SMS, READ_CALL_LOG, etc.). Mark as `is_runtime: true`.

Then run `cmd appops get <pkg>` and parse time fields. Merge last_used into the corresponding PermissionInfo. Time parsing: extract from `time=+XdYhZmWs ago` pattern → convert to "il y a Xj", "il y a Xh", "il y a Xmin".

- [ ] **Step 2: Implement revoke_permission()**

Run `pm revoke <pkg> <permission>`. Check exit code. Return (success, message).

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/security/permissions.rs
git commit -m "feat: permission parsing with appops time tracking and revoke"
```

---

### Task 9: Monitoring — processes, data usage, wakelocks

**Files:**
- Modify: `src/security/monitoring.rs`

- [ ] **Step 1: Implement get_running_processes()**

Run `dumpsys activity processes`. Parse `ProcessRecord` entries. Extract:
- Process name from `processName=X`
- PID from `pid=X`
- Memory from `lastPss=X` or `pssPss=X`
- Adj from `adj=X` (map: 0=foreground, 100-200=visible, 200-300=service, 700+=cached)
- State string: "foreground" / "service" / "cached" based on adj range

Filter to only show app processes (package names with dots).

- [ ] **Step 2: Implement get_data_usage()**

Run `pm list packages -U` first to build UID→package HashMap.
Run `dumpsys netstats detail`. Parse per-UID entries:
- Look for `ident=` lines to determine interface (wifi vs mobile)
- Look for `uid=NNNNN` lines
- Sum `rxBytes=` and `txBytes=` across all buckets per UID
- Map UIDs to package names. System UIDs (<10000) → "Système"

- [ ] **Step 3: Implement get_wakelocks()**

Run `dumpsys batterystats`. Find the wakelock section (search case-insensitively for "wake lock" in section headers). Parse lines matching the pattern of package names with durations. Extract package name + total duration. Convert to human-readable string.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`

- [ ] **Step 5: Commit**

```bash
git add src/security/monitoring.rs
git commit -m "feat: monitoring — process list, data usage, wakelock parsing"
```

---

### Task 10: Device security posture checks

**Files:**
- Modify: `src/security/posture.rs`

- [ ] **Step 1: Implement check_device_posture()**

Run all the `settings get` commands from the spec (section 2.6). For each setting, create a `DevicePosture` entry with:
- name: human-readable French label
- value: raw value from ADB
- status: Good/Warning/Bad based on expected value
- fix_command: Some(command) if fixable, None otherwise

For `enabled_accessibility_services`: if non-empty, list the service names in the value field.
For `default_input_method`: just display the IME package name, status = Info (Warning style for visibility).
For `lockscreen.password_type`: `>= 65536` = Good, else Bad.

- [ ] **Step 2: Implement fix_setting()**

Run the provided command via `adb -s {id} shell {command}`. Return true if exit code is 0.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/security/posture.rs
git commit -m "feat: device security posture checks with fix actions"
```

---

## Milestone 3: Security UI — All 6 Views

### Task 11: Score UI with arc gauge

**Files:**
- Modify: `src/ui/security.rs`

- [ ] **Step 1: Implement draw_score()**

Replace the stub with:
- Refresh button that spawns background thread calling `security::score::calculate_score()`
- Auto-load on first visit (if `security_score.is_none()` and not loading)
- Large score number (32px, bold, colored by range)
- Arc gauge: use `egui::Painter::arc()` or `painter.circle_stroke()` with custom path. Draw a 270-degree arc background (dim), then a filled arc proportional to score (colored).
- Below: list of `SecurityIssue` items as colored cards (red for Critical, orange for Warning)

- [ ] **Step 2: Verify visually**

Run: `cargo run`, navigate to Security → Score
Expected: Score loads from device, displays with colored arc and issue list.

- [ ] **Step 3: Commit**

```bash
git add src/ui/security.rs
git commit -m "feat: security score UI with arc gauge and issue cards"
```

---

### Task 12: Apps management UI

**Files:**
- Modify: `src/ui/security.rs`

- [ ] **Step 1: Implement draw_apps()**

Replace the stub with full app management UI:
- Top bar: filter buttons (All / Third-party / System / Disabled), search TextEdit, sort ComboBox
- Load button that triggers two-phase loading (spawns thread: first `SecurityAppsList`, then loops sending `SecurityAppDetail` per app, checking `security_loading_cancel` each iteration)
- Scrollable list of app cards with:
  - Package name bold + version (if loaded)
  - Install date + source badge (colored `egui::Frame` with text)
  - Target SDK badge
  - Action buttons: Disable/Enable toggle, Force Stop, Clear Data (with confirmation window using `egui::Window`), Uninstall (red, with confirmation)
- Filter and sort applied on the Vec before rendering (don't modify the source Vec, use iterators)

For confirmation dialogs, use app-level state flags: `pub confirm_clear_data: Option<String>` and `pub confirm_uninstall: Option<String>` added to PhoneTvApp. Show an `egui::Window::new("Confirmation")` when set.

- [ ] **Step 2: Add confirmation state fields to app.rs**

Add to PhoneTvApp:
```rust
pub confirm_clear_data: Option<String>,
pub confirm_uninstall: Option<String>,
```
Initialize to `None`.

- [ ] **Step 3: Verify visually**

Run: `cargo run`, navigate to Security → Apps, click "Charger"
Expected: Apps load progressively, filter/sort works, action buttons trigger ADB commands.

- [ ] **Step 4: Commit**

```bash
git add src/ui/security.rs src/app.rs
git commit -m "feat: apps management UI with filter, sort, and actions"
```

---

### Task 13: Permission audit UI

**Files:**
- Modify: `src/ui/security.rs`

- [ ] **Step 1: Implement draw_permissions()**

Two-mode view with toggle buttons at top:
- **By Permission**: List dangerous permission groups as collapsible headers. On expand, load permissions for all apps (if not cached) and show apps that have this permission granted. Each with a Revoke button.
- **By App**: ComboBox/list to select an app. Load its permissions on-demand. Show each permission with granted/denied badge, last_used timestamp, revoke button (only for runtime + dangerous).

Permission loading: spawn thread that calls `security::permissions::get_app_permissions()`, sends `BgEvent::SecurityPermissions`.

- [ ] **Step 2: Verify visually**

Run: `cargo run`, navigate to Security → Permissions
Expected: Permission views work, last usage timestamps show, revoke triggers ADB command.

- [ ] **Step 3: Commit**

```bash
git add src/ui/security.rs
git commit -m "feat: permission audit UI with by-permission and by-app views"
```

---

### Task 14: Blacklist UI

**Files:**
- Modify: `src/ui/security.rs`

- [ ] **Step 1: Implement draw_blacklist()**

- Alert banner (red `egui::Frame`) at top if `blacklist_alerts` is not empty, listing found packages with Disable/Uninstall buttons
- Blacklist editor: scrollable list of entries with delete (X) button each
- Add field: TextEdit + "Ajouter" button. Also "Ajouter depuis Apps" button that copies from security_apps
- Import/Export buttons using `rfd::FileDialog` (same pattern as video.rs file picker)
- On add/remove: call `config::save_blacklist()`
- Check alerts: compare `blacklist` against `security_apps` packages

- [ ] **Step 2: Verify visually**

Run: `cargo run`, add items to blacklist, verify persistence across restart.

- [ ] **Step 3: Commit**

```bash
git add src/ui/security.rs
git commit -m "feat: blacklist UI with alerts, import/export, and persistence"
```

---

### Task 15: Monitoring UI

**Files:**
- Modify: `src/ui/security.rs`

- [ ] **Step 1: Implement draw_monitoring()**

Three sub-views with toggle buttons:
- **Processes**: Refresh button → spawns thread calling `get_running_processes()`. Table with columns: Package, PID, Memory (MB), State. Kill button per row. Sorted by memory desc.
- **Data Usage**: Refresh button → spawns thread calling `get_data_usage()`. Table: Package, WiFi RX/TX, Mobile RX/TX. Format bytes as KB/MB/GB. Label "Données cumulées".
- **Wakelocks**: Refresh button → spawns thread calling `get_wakelocks()`. Table: Package, Duration. Color: >5min orange, >30min red.

Use `egui::Grid` for table layout with headers.

- [ ] **Step 2: Verify visually**

Run: `cargo run`, navigate to Monitoring sub-views, click refresh.

- [ ] **Step 3: Commit**

```bash
git add src/ui/security.rs
git commit -m "feat: monitoring UI — processes, data usage, wakelocks"
```

---

### Task 16: Device posture UI

**Files:**
- Modify: `src/ui/security.rs`

- [ ] **Step 1: Implement draw_posture()**

- Refresh button → spawns thread calling `check_device_posture()`, sends `BgEvent::SecurityPosture`
- Auto-load on first visit
- Grid layout (2 columns) of status cards, each with:
  - Setting name (bold)
  - Current value (secondary text)
  - Status dot: green (Good), orange (Warning), red (Bad) — use `painter.circle_filled()`
  - Fix button (if `fix_command.is_some()`) — calls `security::posture::fix_setting()` in background thread, then refreshes

- [ ] **Step 2: Verify visually**

Run: `cargo run`, navigate to Security → Posture
Expected: Settings displayed with colored indicators. Fix buttons work.

- [ ] **Step 3: Commit**

```bash
git add src/ui/security.rs
git commit -m "feat: device posture UI with traffic-light indicators and fix actions"
```

---

## Milestone 4: UI Redesign — Existing Tabs

### Task 17: Phone tab redesign (remove old apps section)

**Files:**
- Modify: `src/ui/phone.rs`

- [ ] **Step 1: Remove the old "Apps tierces" section and restructure layout**

Delete the entire `ui.columns(2, ...)` block at lines 233-324 in `src/ui/phone.rs` that wraps "Sonnerie" and "Apps". Replace with a single full-width "Sonnerie" section (since apps are now in Security tab). The `ui.columns(2, ...)` wrapper at line 235 must be removed, not just the right column.

- [ ] **Step 1b: Redesign actions as dashboard tiles**

Replace the current small buttons grid (lines 158-190) with 6 large square tiles in a 3x2 grid. Each tile should be `~90x80px` with a large emoji icon (24px) above the label (12px). Use `egui::Frame` with `theme::card_bg()` fill and rounded corners for each tile.

- [ ] **Step 1c: Battery widget improvement**

Replace the simple percentage text (lines 211-230) with a circular gauge. Use `egui::Painter::arc()` to draw a 270-degree background arc (dim), then a filled arc proportional to battery level (colored green/orange/red). Show percentage number in the center at 28px bold.

- [ ] **Step 2: Modernize card styling**

Update `section()` helper to use new theme:
- `theme::card_bg()` + `egui::Stroke::new(1.0, theme::card_border())` for subtle borders
- Corner radius 8.0 (from 10.0)

Update section titles to use `theme::text_primary()` color.

- [ ] **Step 3: Improve Webcam/Mirror section**

For the LIVE indicator, add a pulsing effect:
```rust
let t = ui.ctx().input(|i| i.time);
let alpha = ((t * 3.0).sin() * 0.5 + 0.5) as u8 * 255;
let live_color = egui::Color32::from_rgba_unmultiplied(248, 81, 73, alpha as u8);
ui.label(egui::RichText::new("● LIVE").color(live_color).strong());
```

- [ ] **Step 4: Verify it compiles and test visually**

Run: `cargo run`, check Phone tab
Expected: No apps section, modernized cards, pulsing LIVE indicator.

- [ ] **Step 5: Commit**

```bash
git add src/ui/phone.rs
git commit -m "feat: phone tab redesign — remove apps section, modernize cards"
```

---

### Task 18: Devices tab redesign

**Files:**
- Modify: `src/ui/devices.rs`

- [ ] **Step 1: Modernize device cards**

Update the card frame to use `theme::card_border()` strokes. Add device ID in monospace dimmed text below the name. Use `theme::text_secondary()` for the ID line.

- [ ] **Step 2: Verify visually**

Run: `cargo run`

- [ ] **Step 3: Commit**

```bash
git add src/ui/devices.rs
git commit -m "feat: devices tab redesign with dashboard card styling"
```

---

### Task 19: TV tab redesign

**Files:**
- Modify: `src/ui/tv.rs`

- [ ] **Step 1: Modernize card styling**

Update `section()` helper same as phone tab. Update D-pad button colors to use new theme palette. Update channel grid button fill to use `theme::widget_bg()`.

- [ ] **Step 2: Verify visually**

Run: `cargo run`

- [ ] **Step 3: Commit**

```bash
git add src/ui/tv.rs
git commit -m "feat: TV tab redesign with dashboard styling"
```

---

### Task 20: Video tab redesign

**Files:**
- Modify: `src/ui/video.rs`

- [ ] **Step 1: Modernize card styling and drop zone**

Update card frames. For the drop zone, add a dashed border effect when no file is selected:
```rust
if app.file_path.is_empty() {
    let rect = ui.available_rect_before_wrap();
    let rect = rect.shrink(4.0);
    ui.painter().rect_stroke(rect, 8.0, egui::Stroke::new(2.0, theme::text_dim(app.dark_mode)));
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("📂 Glissez un fichier ici").size(14.0).color(theme::text_secondary(app.dark_mode)));
        ui.add_space(20.0);
    });
}
```

Update progress bar color to use `theme::accent_color()`.

- [ ] **Step 2: Verify visually**

Run: `cargo run`

- [ ] **Step 3: Commit**

```bash
git add src/ui/video.rs
git commit -m "feat: video tab redesign with drop zone and dashboard styling"
```

---

### Task 21: Log panel redesign

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add timestamp to log entries**

Modify the `log()` method to prepend a timestamp:
```rust
pub fn log(&mut self, msg: &str) {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    self.logs.push_back(format!("[{}] {}", now, msg));
    if self.logs.len() > 15 {
        self.logs.pop_front();
    }
}
```

Add `chrono` dependency to `Cargo.toml`:
```toml
chrono = "0.4"
```

- [ ] **Step 2: Add color-coded log entries**

Add a `LogLevel` enum to track log severity:
```rust
pub enum LogLevel { Info, Success, Warning, Error }
```

Change `logs: VecDeque<String>` to `logs: VecDeque<(LogLevel, String)>` in PhoneTvApp.

Update `log()` to default to `LogLevel::Info`. Add `log_success()`, `log_warning()`, `log_error()` convenience methods.

In the log panel rendering, color each entry based on level:
```rust
let color = match level {
    LogLevel::Info => theme::text_primary(self.dark_mode),
    LogLevel::Success => theme::success_color(),
    LogLevel::Warning => theme::warning_color(),
    LogLevel::Error => theme::danger_color(),
};
ui.label(egui::RichText::new(log).small().family(egui::FontFamily::Monospace).color(color));
```

Update all existing `app.log()` calls to use appropriate levels where relevant (e.g., errors use `log_error()`, successful actions use `log_success()`).

- [ ] **Step 3: Verify visually**

Run: `cargo run`, trigger some actions, check log panel shows colored timestamps.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs src/types.rs Cargo.toml
git commit -m "feat: log panel with timestamps, monospace font, and color-coded levels"
```

---

## Milestone 5: Final Integration

### Task 22: Window size and version update

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Update default window size and version**

In `src/config.rs`, update the default window size to accommodate the new content:
```rust
window_size: (1000.0, 800.0),
```

In `Cargo.toml`, update the version:
```toml
version = "5.0.0"
```

- [ ] **Step 2: Verify everything works end-to-end**

Run: `cargo run`
- Test all 5 tabs
- Test Security sub-views with a connected device
- Test blacklist persistence
- Test theme toggle
- Test all existing features (webcam, mirror, TV remote, etc.) still work

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "Phone-TV v5.0.0 — Security tab + dashboard UI redesign"
```
