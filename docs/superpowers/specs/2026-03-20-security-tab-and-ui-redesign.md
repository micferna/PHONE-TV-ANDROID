# Phone-TV v5.0 — Security Tab & Full UI Redesign

**Date:** 2026-03-20
**Scope:** New Security tab + full UI redesign (dashboard monitoring style)

---

## 1. Overview

Add a comprehensive Security tab accessible for both Phone and TV devices, and redesign the entire UI with a dashboard/monitoring aesthetic. The security features leverage ADB commands that work identically on Android phones and Android TV.

### Goals
- Full security audit and management of Android devices via ADB
- Modern dashboard-style UI across all tabs
- Visual risk indicators, scores, and status badges throughout
- Consistent monitoring aesthetic (dark theme emphasis, data-dense cards, colored indicators)

### Error Handling Strategy
All ADB commands can fail (device disconnected, permission denied, unsupported command). Strategy:
- Actions (uninstall, disable, revoke) → display result in logs with success/error message
- Data fetches → show "Erreur de chargement" in the UI section, log the error
- `AppActionResult` event carries `success: bool` + `message: String` for user-visible feedback
- Graceful degradation: if a command is unsupported (e.g., `cmd appops` on old Android), skip that data and show "Non disponible"

---

## 2. New Security Tab

Available when any device (Phone or TV) is selected. Tab enum: `Tab::Security`. Tab enabled rule: `Tab::Security => self.get_selected_id().is_some()`.

Contains 6 sections, navigated via sub-tabs at the top of the security panel.

### 2.1 Security Score (Header Widget)

A global score 0-100 displayed as a large number at the top of the tab.

**Scoring rules (deductions from 100, clamped to minimum 0):**
| Check | ADB Command | Deduction | Notes |
|-------|-------------|-----------|-------|
| Unknown sources enabled | Android <8: `settings get secure install_non_market_apps`; Android 8+: `cmd appops get <pkg> REQUEST_INSTALL_PACKAGES` for known app stores | -20 | Deprecated on Android 8+, use fallback |
| Developer mode enabled | `settings get global development_settings_enabled` | -10 | |
| Play Protect disabled | `settings get global package_verifier_enable` | -25 | |
| Accessibility services active | `settings get secure enabled_accessibility_services` | -15 (total, not per service) | Cap at -15 regardless of count |
| Sideloaded apps present | `dumpsys package <pkg>` → `installerPackageName` not in `[com.android.vending, com.google.android.packageinstaller]` | -3 per app, max -15 | Cap at 5 apps |
| Apps with 3+ dangerous permissions granted | `dumpsys package <pkg>` → runtime permissions | -2 per app, max -10 | Cap at 5 apps |

Score formula: `max(0, 100 - total_deductions)`. Stored as `u8`.

**Visual:** Large centered number with color (green 80+, orange 50-79, red <50). Rendered with `egui::Painter` as a colored arc (270-degree semi-circle) with the score number in the center. List of deductions shown below as info items (not dismissable — they reset on each scan to avoid stale state).

**Data flow:** On tab open or refresh, spawn a background thread that runs all checks, sends results via `BgEvent::SecurityScore { score: u8, issues: Vec<SecurityIssue> }`.

### 2.2 Apps & Management

Full app manager with filtering, sorting, and batch actions.

**Data per app (from `dumpsys package <pkg>`):**
- Package name
- Version name + version code
- First install time
- Last update time
- Installer package name (Play Store / sideload / ADB / unknown)
- Target SDK version
- Enabled/disabled state

**ADB commands:**
- List third-party: `pm list packages -3`
- List system: `pm list packages -s`
- List disabled: `pm list packages -d`
- App details: `dumpsys package <pkg>`
- Uninstall: `pm uninstall <pkg>` (user apps only)
- Disable: `pm disable-user --user 0 <pkg>`
- Enable: `pm enable <pkg>`
- Force stop: `am force-stop <pkg>`
- Clear data: `pm clear <pkg>` (**WARNING: this clears ALL app data, not just cache**)

**UI layout:**
- Top bar: filter buttons (All / Third-party / System / Disabled) + search text input + sort dropdown (name / install date / source)
- Scrollable list of app cards, each showing:
  - Package name (bold) + version
  - Install date + source badge (green "Play Store" / red "Sideload" / orange "ADB" / gray "Unknown")
  - Target SDK (orange badge if < 28, red if < 23)
  - Action buttons row: Disable/Enable toggle, Force Stop, Clear Data (with confirmation dialog "Toutes les données seront supprimées"), Uninstall (red, with confirmation)

**Data flow:** Two-phase loading:
1. `BgEvent::SecurityAppsList { packages: Vec<String> }` — fast list from `pm list packages`, displayed immediately as package names
2. `BgEvent::SecurityAppDetail { package: String, info: AppInfo }` — sent one at a time as each app's details are fetched in background. Handler updates the matching entry in `security_apps`.
3. Background thread uses `AtomicBool` cancellation token (`security_loading_cancel`) to abort if user switches tab/device.
4. Cache results in `app.security_apps: Vec<AppInfo>` — refresh on explicit user action only

### 2.3 Permission Audit

Two view modes toggled by buttons at the top.

**View A — By Permission:**
- List of dangerous permission groups: Camera, Microphone, Location, Contacts, SMS, Call Log, Storage, Phone, Calendar, Sensors
- Each group expandable → shows all apps that have this permission granted
- Per app: grant status + last usage timestamp (from `cmd appops get <pkg>`)
- Revoke button only (no grant — `pm grant` is unreliable and fails silently in many cases)

**View B — By App:**
- Select an app → see all its requested permissions
- Each permission shows: granted/denied status, last usage time
- Dangerous permissions: revoke button. Non-dangerous: display only (cannot be revoked)
- Dangerous permissions highlighted in red, normal in gray

**ADB commands:**
- Per-app permissions: `dumpsys package <pkg>` → filter `runtime permissions:` section
- Last usage: `cmd appops get <pkg>` → parse `time=` field (see parsing rules below)
- Revoke: `pm revoke <pkg> <permission>` (runtime permissions only)

**Appops time parsing:** The `time=` field format is `+XdYhZmWs ago` (relative) or an epoch timestamp. Parse strategy:
- Extract numeric components from `+(\d+d)?(\d+h)?(\d+m)?(\d+s)?` pattern
- Convert to human-readable French: "il y a 2h", "il y a 3j", "il y a 5min"
- If parsing fails, display raw string

**Permission data is loaded on-demand** when the user opens the Permissions sub-tab or selects an app. Stored in `security_permission_cache: HashMap<String, Vec<PermissionInfo>>`. Not pre-loaded with apps.

**UI:** Color-coded permission badges. Red = dangerous + granted. Orange = dangerous + denied. Green = normal.

### 2.4 Blacklist

Persistent local list of forbidden packages. Stored in `~/.config/phone-tv/blacklist.txt` (one package name per line).

**Features:**
- Add by typing package name or selecting from the app list
- Remove from blacklist
- On security scan: compare installed apps against blacklist → flag matches with red alert banner
- Actions on blacklisted app found: Disable / Uninstall buttons directly in the alert
- Import/export blacklist via `rfd` file dialog (already in Cargo.toml dependencies)

**UI:** Two sections:
1. Alert banner (top, red) if any blacklisted apps are currently installed — with action buttons
2. Blacklist editor: scrollable list with delete buttons + add input field + import/export buttons

**Data flow:** Blacklist loaded at app start from config file. Checked against installed apps on Security tab open. Results stored in `app.blacklist_alerts: Vec<String>`.

### 2.5 Real-time Monitoring

Live view of what's running on the device.

**Sub-sections:**

**A) Running Processes:**
- ADB: `dumpsys activity processes` (more reliable and richer than `ps -A` across Android versions)
- Parse: extract `ProcessRecord` entries with pid, processName, adj score, memory (pssPss)
- Display: package name, PID, RSS memory (MB), adj score (foreground/background/cached), state
- Action: Kill button → `am force-stop <pkg>`
- Sort by memory usage (descending)

**B) Data Usage:**
- ADB: `dumpsys netstats detail` → parse per-UID rx/tx bytes
- Parse strategy: look for lines matching `ident=` (interface), `uid=` (app), `rxBytes=`/`txBytes=` fields. Sum across all time buckets per UID.
- UID to package mapping: `pm list packages -U` → parse `uid:NNNNN` field. Note: shared UIDs map to multiple packages (display first match + "+N others"). System UIDs (<10000) labeled as "Système".
- Display: per-app table with WiFi RX/TX and Mobile RX/TX columns (human-readable: KB/MB/GB)
- Sort by total data (descending)
- Note: this is cumulative data, not real-time. Label clearly: "Données cumulées depuis le dernier reset"

**C) Battery Drain (Wakelocks):**
- ADB: `dumpsys batterystats` → grep for section containing "wake lock" (case-insensitive, handles "All partial wake locks", "Wake lock statistics", etc.)
- Parse: extract package name + total duration from lines matching pattern `pkg_name: +Xh Ym Zs`
- Display: apps holding partial wakelocks, sorted by duration
- Flag apps with >5 minutes of wakelock as orange, >30 minutes as red

**UI:** Three sub-tabs within the monitoring section, each with a refresh button. Data loaded on-demand (not auto-refresh to avoid ADB overhead).

### 2.6 Device Security Posture

Dashboard of device-level security settings with traffic-light indicators.

**Checks:**
| Setting | ADB Command | Good Value | Display |
|---------|-------------|------------|---------|
| ADB enabled | `settings get global adb_enabled` | 0* | Yellow (expected since we use ADB) |
| Unknown sources | Android <8: `settings get secure install_non_market_apps`; Android 8+: note "Géré par app" | 0 / null | Green/Red/Info |
| Developer mode | `settings get global development_settings_enabled` | 0 | Green/Red |
| Play Protect | `settings get global package_verifier_enable` | 1 | Green/Red |
| Verify ADB installs | `settings get global verifier_verify_adb_installs` | 1 | Green/Red |
| Accessibility services | `settings get secure enabled_accessibility_services` | empty/null | Green/Red + list services |
| Default keyboard | `settings get secure default_input_method` | display IME package name, no auto-judgment | Info (user decides if suspicious) |
| Screen lock | `settings get secure lockscreen.password_type` | >= 65536 | Green/Red |
| Location mode | `settings get secure location_mode` | any (info) | Info badge |

**Actions:** Where possible, offer a "Fix" button:
- Enable Play Protect: `settings put global package_verifier_enable 1`
- Enable ADB install verification: `settings put global verifier_verify_adb_installs 1`
- Note: `install_non_market_apps` fix removed (deprecated on Android 8+, no-op)

**UI:** Grid of status cards (2 columns), each with:
- Setting name
- Current value
- Status indicator (green circle / red circle / orange circle)
- Fix button (if applicable)

---

## 3. UI Redesign — All Tabs

### 3.1 Theme System Overhaul

New dashboard/monitoring color palette:

**Dark mode (primary):**
```
background:       #0d1117  (GitHub dark style, very dark)
sidebar:          #161b22
card_bg:          #1c2128
card_border:      #30363d
card_hover:       #252c35
widget_bg:        #21262d
text_primary:     #e6edf3
text_secondary:   #8b949e
text_dim:         #484f58
```

**Accent colors (both modes):**
```
accent_blue:      #58a6ff  (links, active states)
accent_cyan:      #39d353  (success, live indicators)
accent_orange:    #d29922  (warnings)
accent_red:       #f85149  (danger, critical)
accent_purple:    #bc8cff  (info badges)
```

**Light mode:**
```
background:       #f0f2f5
sidebar:          #e4e7eb
card_bg:          #ffffff
card_border:      #d0d7de
text_primary:     #1f2328
text_secondary:   #656d76
```

**Typography hierarchy:**
- Metric numbers: 28-32px, bold
- Section titles: 15-16px, semibold
- Card titles: 13-14px, semibold
- Body text: 12-13px, regular
- Labels/badges: 10-11px

**Card style:**
- Corner radius: 8px
- Inner margin: 12-16px
- Subtle border (1px card_border color)
- No heavy shadows (flat monitoring style)

### 3.2 Sidebar Redesign

**Layout (top to bottom):**
1. App title "Phone-TV" + version badge (small, dimmed)
2. Separator
3. Device selector card:
   - Device icon (📱 or 📺)
   - Device name (bold)
   - Connection status dot (green/red)
   - Mini battery bar (if available)
   - Dropdown arrow to switch device
4. Separator
5. Navigation tabs (vertical list):
   - 📡 Appareils (always visible)
   - 📱 Phone (if phone selected)
   - 📺 TV (if TV selected)
   - 🎬 Vidéo (if device selected)
   - 🛡 Sécurité (if device selected — phone OR TV)
   - Active tab: accent_blue left border bar + slightly brighter background
6. Spacer (flex)
7. Footer:
   - 🌙/☀ Dark/Light toggle
   - 🛑 STOP ALL button (red)

### 3.3 Devices Tab Redesign

**Layout:**
- Top: "Appareils connectés" title + Refresh button + Scan réseau button
- Device cards (grid, 2 columns if space allows):
  - Icon (📱/📺) + Name (bold)
  - ID (dimmed, monospace)
  - Status badge: "Connecté" green / "Offline" red / "Inconnu" gray
  - Device type badge
  - Click to select
- Bottom section: Manual connection
  - IP input + port + Connect button
  - Found devices from scan as clickable cards

### 3.4 Phone Tab Redesign

**Layout (dashboard grid):**
- Top row (2 columns):
  - **Webcam card:** Camera toggle (front/back as segmented control), mic/audio checkboxes, Start/Stop button with LIVE badge (pulsing red dot), v4l2 warning if needed
  - **Mirror card:** Stay awake toggle, Start/Stop, ACTIF badge
- Middle row (3 columns):
  - **Actions tiles:** 6 large square tiles (Photo, Video, Micro, Home, Back, Recent) — icon-forward design
  - **Battery widget:** circular gauge (like a speedometer), percentage in center, status text below, auto-refresh option
  - **Find phone:** Ring/Stop buttons, volume indicator

### 3.5 TV Tab Redesign

**Layout:**
- Top row: Remote control (D-pad center, media buttons around it) — larger, more tactile look with subtle gradients/shadows on buttons
- Middle row (2 columns):
  - **Apps grid:** larger icons, 3-column grid, labels below
  - **Channels grid:** 4-column grid, number + name, edit mode toggle
- Bottom row (2 columns):
  - **Storage widget:** progress bar + used/total/free labels
  - **Screenshot widget:** capture button + preview thumbnail
  - **Text input:** for keyboard input to TV
  - **Power controls:** power off, reboot, sleep buttons

### 3.6 Video Tab Redesign

**Layout:**
- **URL playback card:** URL input + Play button, wider input field
- **File transfer card:**
  - Large drop zone (dashed border, icon centered) for drag & drop
  - Or file picker button
  - Transfer progress: progress bar with percentage + speed (MB/s) + ETA
  - "Play after transfer" checkbox

### 3.7 Log Panel Redesign

- Slimmer footer, monospace font
- Color-coded log entries: info (default), success (green), warning (orange), error (red)
- Timestamp prefix on each entry
- Collapsible (same as current)

---

## 4. Data Structures

### New Types (types.rs additions)

All new types derive `Clone, Debug`. Types sent through channels also derive `Send`.

```rust
#[derive(Clone, Debug)]
pub struct AppInfo {
    pub package: String,
    pub version_name: String,
    pub version_code: u32,
    pub first_install: String,
    pub last_update: String,
    pub installer: AppInstaller,
    pub target_sdk: u32,
    pub enabled: bool,
    pub details_loaded: bool,  // false = only package name known, true = full details fetched
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppInstaller {
    PlayStore,
    Sideload,
    Adb,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct PermissionInfo {
    pub name: String,           // e.g. "android.permission.CAMERA"
    pub granted: bool,
    pub last_used: Option<String>, // human-readable: "il y a 2h"
    pub dangerous: bool,
    pub is_runtime: bool,       // only runtime permissions can be revoked
}

#[derive(Clone, Debug)]
pub struct SecurityIssue {
    pub id: String,             // unique identifier for deduplication
    pub description: String,
    pub severity: Severity,
    pub points: i32,            // deduction from 100
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
    pub adj: i32,               // OOM adjustment: 0=foreground, 900+=cached
    pub state: String,          // "foreground" / "background" / "cached"
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
    pub duration_ms: u64,       // total wakelock duration in milliseconds
    pub duration_human: String, // "5m30s", "2h15m"
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

// Security tab view state
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

### Tab enum update

```rust
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Devices,
    Tv,
    Phone,
    Video,
    Security,  // NEW
}
```

### New BgEvent variants

```rust
pub enum BgEvent {
    // ... existing variants ...
    SecurityScore { score: u8, issues: Vec<SecurityIssue> },
    SecurityAppsList { packages: Vec<String> },          // Phase 1: fast list
    SecurityAppDetail { package: String, info: AppInfo }, // Phase 2: progressive detail
    SecurityProcesses { processes: Vec<ProcessInfo> },
    SecurityDataUsage { usage: Vec<DataUsage> },
    SecurityWakelocks { wakelocks: Vec<WakelockInfo> },
    SecurityPosture { checks: Vec<DevicePosture> },
    SecurityPermissions { package: String, permissions: Vec<PermissionInfo> },
    BlacklistAlert { found: Vec<String> },
    AppActionResult { package: String, action: String, success: bool, message: String },
}
```

### New PhoneTvApp fields

```rust
// Security tab state
pub security_view: SecurityView,
pub security_score: Option<(u8, Vec<SecurityIssue>)>,
pub security_score_loading: bool,
pub security_apps: Vec<AppInfo>,
pub security_apps_filter: AppFilter,
pub security_apps_sort: AppSort,
pub security_apps_search: String,
pub security_apps_loading: bool,
pub security_loading_cancel: Arc<AtomicBool>,  // cancellation token
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

### BgEvent handler rules

- `SecurityAppsList` → populate `security_apps` with `AppInfo { package, details_loaded: false, ..Default::default() }`
- `SecurityAppDetail` → find matching entry in `security_apps` by package name, update fields, set `details_loaded = true`
- `SecurityPermissions` → insert/update in `security_permission_cache` HashMap
- `SecurityWakelocks` → store in `security_wakelocks`
- `AppActionResult` → log message to user, refresh app list if action was uninstall/disable/enable

---

## 5. File Structure

### New files:
- `src/security/mod.rs` — Module exports
- `src/security/score.rs` — Security score calculation
- `src/security/apps.rs` — App listing and detail parsing from `dumpsys package`
- `src/security/permissions.rs` — Permission parsing and appops
- `src/security/monitoring.rs` — Process listing, netstats, wakelocks
- `src/security/posture.rs` — Device security settings checks
- `src/ui/security.rs` — Security tab UI (all 6 sections)

### Modified files:
- `src/types.rs` — New types listed above + Tab::Security
- `src/app.rs` — New fields, new BgEvent handling, tab_enabled for Security, security initialization
- `src/theme.rs` — Complete color palette overhaul
- `src/main.rs` — Window size adjustment if needed
- `src/config.rs` — Blacklist load/save
- `src/ui/mod.rs` — Export new security module + draw_security function
- `src/ui/sidebar.rs` — Redesigned sidebar with Security tab
- `src/ui/devices.rs` — Dashboard card layout
- `src/ui/phone.rs` — Dashboard tile layout, remove old apps section (moved to security)
- `src/ui/tv.rs` — Dashboard layout improvements
- `src/ui/video.rs` — Drop zone and progress improvements

### New config file:
- `~/.config/phone-tv/blacklist.txt` — One package name per line

---

## 6. ADB Command Summary

All commands used by the security tab, for reference:

| Feature | Command | Parse Strategy |
|---------|---------|---------------|
| List third-party apps | `pm list packages -3` | Split by newline, strip "package:" prefix |
| List system apps | `pm list packages -s` | Same |
| List disabled apps | `pm list packages -d` | Same |
| App details | `dumpsys package <pkg>` | Line parsing for versionName, firstInstallTime, installerPackageName, etc. |
| App permissions | `dumpsys package <pkg>` | Filter lines in `runtime permissions:` section, parse `granted=true/false` |
| Permission last usage | `cmd appops get <pkg>` | Parse `time=+XdYhZmWs ago` pattern |
| Revoke permission | `pm revoke <pkg> <perm>` | Check exit code + stderr |
| Disable app | `pm disable-user --user 0 <pkg>` | Check output contains "disabled" |
| Enable app | `pm enable <pkg>` | Check output contains "enabled" |
| Force stop | `am force-stop <pkg>` | Fire and forget |
| Clear data | `pm clear <pkg>` | Check output (WARNING: clears ALL data) |
| Uninstall | `pm uninstall <pkg>` | Check "Success" in output |
| Running processes | `dumpsys activity processes` | Parse ProcessRecord entries |
| Data usage | `dumpsys netstats detail` | Parse per-UID ident/rxBytes/txBytes, sum buckets |
| UID to package mapping | `pm list packages -U` | Parse "uid:" field |
| Battery wakelocks | `dumpsys batterystats` | Case-insensitive grep for wake lock section |
| Security settings | `settings get secure/global <key>` | Single value per call |
| Fix settings | `settings put secure/global <key> <val>` | Fire and forget |

---

## 7. Performance Considerations

- **App list loading:** Two-phase: `pm list packages` (fast, <1s) then `dumpsys package` per app (slow, ~200ms each). UI shows package names immediately, fills in details progressively. Background thread uses `AtomicBool` cancellation token.
- **Security score:** Runs ~10 quick `settings get` commands + 1 `pm list packages`. Should complete in <2s.
- **Permission audit:** Load on-demand per app. Cached in `HashMap<String, Vec<PermissionInfo>>`.
- **Monitoring:** Load on-demand only. No auto-refresh (ADB commands have overhead on the device).
- **All ADB calls in background threads** via existing `bg_tx`/`bg_rx` pattern.
- **Netstats parsing:** Output can be large (MB). Parse line-by-line in streaming fashion, don't load entire output in memory at once.

---

## 8. Non-Goals

- No root-required features
- No auto-remediation without user confirmation (except blacklist alerts which are just visual)
- No network traffic interception or deep packet inspection
- No app binary analysis or malware scanning
- No persistent database of historical data (all in-memory, refresh on demand)
- No `pm grant` (unreliable, fails silently on many permission types)
- No transfer history (deferred to future version)
