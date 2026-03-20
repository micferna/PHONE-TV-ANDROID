use std::process::{Child, ChildStdin};

#[derive(Clone, Default)]
pub struct TransferState {
    pub active: bool,
    pub filename: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub done: bool,
    pub play_after: bool,
}

#[allow(dead_code)]
pub enum BgEvent {
    DevicesLoaded(Vec<Device>),
    NetworkScanDone(Vec<String>),
    WifiConnected { addr: String, success: bool },
    WebcamSwitched(Option<Child>),
    StorageInfo { device_id: String, total: String, used: String, avail: String, percent: f32 },
    BatteryInfo { device_id: String, level: u8, status: String },
    PhoneApps { device_id: String, apps: Vec<String> },
    ScreenshotReady { device_id: String, data: Vec<u8> },
    Log(String),
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
}

#[derive(Clone, PartialEq)]
pub enum DeviceType {
    Phone,
    Tv,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Devices,
    Tv,
    Phone,
    Video,
    Security,
}

#[derive(Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub status: String,
    pub device_type: DeviceType,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TvChannel {
    pub name: String,
    pub number: u32,
}

pub struct TvShell {
    pub device_id: String,
    pub child: Child,
    pub stdin: ChildStdin,
}

// ── Security types ──────────────────────────────────────────────────

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
