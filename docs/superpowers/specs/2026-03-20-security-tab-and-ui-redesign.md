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

---

## 2. New Security Tab

Available when any device (Phone or TV) is selected. Contains 6 sections.

### 2.1 Security Score (Header Widget)

A global score 0-100 displayed as a large gauge at the top of the tab.

**Scoring rules (deductions from 100):**
| Check | ADB Command | Deduction |
|-------|-------------|-----------|
| Unknown sources enabled | `settings get secure install_non_market_apps` | -20 |
| Developer mode enabled | `settings get global development_settings_enabled` | -10 |
| Play Protect disabled | `settings get global package_verifier_enable` | -25 |
| Accessibility services active | `settings get secure enabled_accessibility_services` | -15 per service |
| Sideloaded apps present | `dumpsys package <pkg>` → `installerPackageName != com.android.vending` | -3 per app |
| Apps with 3+ dangerous permissions granted | `dumpsys package <pkg>` → runtime permissions | -2 per app |

**Visual:** Large centered number with color (green 80+, orange 50-79, red <50). Semi-circular gauge below. List of deductions shown as dismissable items below the gauge.

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
- Clear cache: `pm clear <pkg>`

**UI layout:**
- Top bar: filter buttons (All / Third-party / System / Disabled) + search text input + sort dropdown (name / install date / source)
- Scrollable list of app cards, each showing:
  - Package name (bold) + version
  - Install date + source badge (green "Play Store" / red "Sideload" / orange "ADB" / gray "Unknown")
  - Target SDK (orange badge if < 28, red if < 23)
  - Action buttons row: Disable/Enable toggle, Force Stop, Clear Cache, Uninstall (red, with confirmation)

**Data flow:** Loading app list is expensive (one `dumpsys package` per app). Strategy:
1. First load: `pm list packages -3` for the list (fast)
2. Then batch-fetch details in a background thread, updating the UI progressively as each app's info arrives
3. Cache results in `app.security_apps: Vec<AppInfo>` — refresh on explicit user action only

### 2.3 Permission Audit

Two view modes toggled by buttons at the top.

**View A — By Permission:**
- List of dangerous permission groups: Camera, Microphone, Location, Contacts, SMS, Call Log, Storage, Phone, Calendar, Sensors
- Each group expandable → shows all apps that have this permission granted
- Per app: grant status + last usage timestamp (from `cmd appops get <pkg>`)
- Toggle button to revoke/grant: `pm revoke <pkg> <permission>` / `pm grant <pkg> <permission>`

**View B — By App:**
- Select an app → see all its requested permissions
- Each permission shows: granted/denied status, last usage time, revoke/grant button
- Dangerous permissions highlighted in red, normal in gray

**ADB commands:**
- List dangerous permissions: `pm list permissions -d -g`
- Per-app permissions: `dumpsys package <pkg>` → filter `runtime permissions:` section
- Last usage: `cmd appops get <pkg>` → parse `time=` field
- Revoke: `pm revoke <pkg> <permission>`
- Grant: `pm grant <pkg> <permission>`

**UI:** Color-coded permission badges. Red = dangerous + granted. Orange = dangerous + denied. Green = normal. Each with a toggle switch.

### 2.4 Blacklist

Persistent local list of forbidden packages. Stored in `~/.config/phone-tv/blacklist.txt` (one package name per line).

**Features:**
- Add by typing package name or selecting from the app list
- Remove from blacklist
- On security scan: compare installed apps against blacklist → flag matches with red alert banner
- Actions on blacklisted app found: Disable / Uninstall buttons directly in the alert
- Import/export blacklist (file dialog)

**UI:** Two sections:
1. Alert banner (top, red) if any blacklisted apps are currently installed — with action buttons
2. Blacklist editor: scrollable list with delete buttons + add input field + import/export buttons

**Data flow:** Blacklist loaded at app start from config file. Checked against installed apps on Security tab open. Results stored in `app.blacklist_alerts: Vec<String>`.

### 2.5 Real-time Monitoring

Live view of what's running on the device.

**Sub-sections:**

**A) Running Processes:**
- ADB: `ps -A` filtered to show only app processes (UID starting with `u0_a`)
- Display: package name, PID, RSS memory (MB), state (running/sleeping)
- Action: Kill button → `am force-stop <pkg>`
- Sort by memory usage (descending)

**B) Data Usage:**
- ADB: `dumpsys netstats detail` → parse per-UID rx/tx bytes
- Display: per-app table with WiFi RX/TX and Mobile RX/TX columns
- Sort by total data (descending)
- Map UID to package name via `pm list packages -U`

**C) Battery Drain (Wakelocks):**
- ADB: `dumpsys batterystats` → parse "Wakelock statistics" section
- Display: apps holding partial wakelocks, sorted by duration
- Flag apps with >5 minutes of wakelock as orange, >30 minutes as red

**UI:** Three sub-tabs within the monitoring section, each with a refresh button. Data loaded on-demand (not auto-refresh to avoid ADB overhead).

### 2.6 Device Security Posture

Dashboard of device-level security settings with traffic-light indicators.

**Checks:**
| Setting | ADB Command | Good Value | Display |
|---------|-------------|------------|---------|
| ADB enabled | `settings get global adb_enabled` | 0* | Yellow (expected since we use ADB) |
| Unknown sources | `settings get secure install_non_market_apps` | 0 | Green/Red |
| Developer mode | `settings get global development_settings_enabled` | 0 | Green/Red |
| Play Protect | `settings get global package_verifier_enable` | 1 | Green/Red |
| Verify ADB installs | `settings get global verifier_verify_adb_installs` | 1 | Green/Red |
| Accessibility services | `settings get secure enabled_accessibility_services` | empty/null | Green/Red + list services |
| Default keyboard | `settings get secure default_input_method` | known IME | Green/Orange |
| Screen lock | `settings get secure lockscreen.password_type` | >= 65536 | Green/Red |
| Location mode | `settings get secure location_mode` | any (info) | Info badge |

**Actions:** Where possible, offer a "Fix" button:
- Disable unknown sources: `settings put secure install_non_market_apps 0`
- Enable Play Protect: `settings put global package_verifier_enable 1`
- Enable ADB install verification: `settings put global verifier_verify_adb_installs 1`

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
   - 🛡 Sécurité (if device selected)
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
- **Transfer history** (optional, in-memory): last 5 transfers with status

### 3.7 Log Panel Redesign

- Slimmer footer, monospace font
- Color-coded log entries: info (default), success (green), warning (orange), error (red)
- Timestamp prefix on each entry
- Collapsible (same as current)

---

## 4. Data Structures

### New Types (types.rs additions)

```rust
// Security app info
pub struct AppInfo {
    pub package: String,
    pub version_name: String,
    pub version_code: u32,
    pub first_install: String,
    pub last_update: String,
    pub installer: AppInstaller,
    pub target_sdk: u32,
    pub enabled: bool,
    pub dangerous_permissions: Vec<PermissionInfo>,
}

pub enum AppInstaller {
    PlayStore,
    Sideload,
    Adb,
    Unknown,
}

pub struct PermissionInfo {
    pub name: String,           // e.g. "android.permission.CAMERA"
    pub granted: bool,
    pub last_used: Option<String>, // e.g. "2h ago"
    pub dangerous: bool,
}

pub struct SecurityIssue {
    pub description: String,
    pub severity: Severity,
    pub points: i32,        // deduction from 100
    pub fixable: bool,
    pub fix_command: Option<String>,
}

pub enum Severity {
    Critical,
    Warning,
    Info,
}

pub struct ProcessInfo {
    pub package: String,
    pub pid: u32,
    pub memory_kb: u64,
    pub state: String,
}

pub struct DataUsage {
    pub package: String,
    pub wifi_rx: u64,
    pub wifi_tx: u64,
    pub mobile_rx: u64,
    pub mobile_tx: u64,
}

pub struct DevicePosture {
    pub name: String,
    pub value: String,
    pub status: PostureStatus,
    pub fix_command: Option<String>,
}

pub enum PostureStatus {
    Good,
    Warning,
    Bad,
}

// Security tab view state
pub enum SecurityView {
    Score,
    Apps,
    Permissions,
    Blacklist,
    Monitoring,
    Posture,
}

pub enum PermissionView {
    ByPermission,
    ByApp,
}

pub enum MonitoringView {
    Processes,
    DataUsage,
    Wakelocks,
}

pub enum AppFilter {
    All,
    ThirdParty,
    System,
    Disabled,
}
```

### New BgEvent variants

```rust
pub enum BgEvent {
    // ... existing variants ...
    SecurityScore { score: u8, issues: Vec<SecurityIssue> },
    SecurityApps { apps: Vec<AppInfo> },
    SecurityAppDetail { package: String, info: AppInfo },
    SecurityProcesses { processes: Vec<ProcessInfo> },
    SecurityDataUsage { usage: Vec<DataUsage> },
    SecurityPosture { checks: Vec<DevicePosture> },
    SecurityPermissionUsage { package: String, ops: Vec<PermissionInfo> },
    BlacklistAlert { found: Vec<String> },
    AppActionResult { package: String, action: String, success: bool },
}
```

### New PhoneTvApp fields

```rust
// Security tab state
pub security_view: SecurityView,
pub security_score: Option<(u8, Vec<SecurityIssue>)>,
pub security_apps: Vec<AppInfo>,
pub security_apps_filter: AppFilter,
pub security_apps_search: String,
pub security_apps_loading: bool,
pub security_permission_view: PermissionView,
pub security_selected_app: Option<String>,
pub security_monitoring_view: MonitoringView,
pub security_processes: Vec<ProcessInfo>,
pub security_data_usage: Vec<DataUsage>,
pub security_posture: Vec<DevicePosture>,
pub blacklist: Vec<String>,
pub blacklist_alerts: Vec<String>,
pub blacklist_new_entry: String,
pub security_score_loading: bool,
```

---

## 5. File Structure

### New files:
- `src/ui/security.rs` — Security tab UI (all 6 sections)
- `src/security.rs` — Security ADB commands and data parsing logic

### Modified files:
- `src/types.rs` — New types listed above
- `src/app.rs` — New fields, new BgEvent handling, security initialization
- `src/theme.rs` — Complete color palette overhaul
- `src/main.rs` — Window size adjustment if needed
- `src/config.rs` — Blacklist load/save, possibly new settings
- `src/ui/mod.rs` — Export new security module
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
| App details | `dumpsys package <pkg>` | Regex/line parsing for versioName, firstInstallTime, etc. |
| App permissions | `dumpsys package <pkg>` | Filter lines containing "permission:" in runtime section |
| Permission last usage | `cmd appops get <pkg>` | Parse "time=" and "duration=" fields |
| Revoke permission | `pm revoke <pkg> <perm>` | Check exit code |
| Grant permission | `pm grant <pkg> <perm>` | Check exit code |
| Disable app | `pm disable-user --user 0 <pkg>` | Check output contains "disabled" |
| Enable app | `pm enable <pkg>` | Check output contains "enabled" |
| Force stop | `am force-stop <pkg>` | Fire and forget |
| Clear cache | `pm clear <pkg>` | Check output |
| Uninstall | `pm uninstall <pkg>` | Check "Success" in output |
| Running processes | `ps -A` | Split columns, filter u0_a* UIDs |
| Data usage | `dumpsys netstats detail` | Parse per-UID buckets, sum rx/tx |
| UID to package mapping | `pm list packages -U` | Parse "uid:" field |
| Battery wakelocks | `dumpsys batterystats` | Grep "Wakelock statistics" section |
| Security settings | `settings get secure/global <key>` | Single value per call |
| Fix settings | `settings put secure/global <key> <val>` | Fire and forget |
| Security score | Combination of above settings checks | Aggregate |

---

## 7. Performance Considerations

- **App list loading:** `dumpsys package` is slow (~200ms per app). Load list first (`pm list packages`), then fetch details in background thread. Show list immediately with package names, fill in details progressively.
- **Security score:** Runs ~10 quick `settings get` commands + 1 `pm list packages`. Should complete in <2s.
- **Permission audit:** Load on-demand per app. Cache in memory.
- **Monitoring:** Load on-demand only. No auto-refresh (ADB commands have overhead on the device).
- **All ADB calls in background threads** via existing `bg_tx`/`bg_rx` pattern.

---

## 8. Non-Goals

- No root-required features
- No auto-remediation without user confirmation (except blacklist alerts which are just visual)
- No network traffic interception or deep packet inspection
- No app binary analysis or malware scanning
- No persistent database of historical data (all in-memory, refresh on demand)
