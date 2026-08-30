use std::process::{Child, ChildStdin};

use crate::brands::types::BrandDb;
use crate::history::types::DeviceHistory;
use crate::llm::types::AppVerdict;
use crate::pentest::rootcheck::RootStatus;
use crate::pentest::vulns::Vulnerability;
use crate::wizard::types::DeviceInfo;

#[derive(Clone, Default)]
pub struct TransferState {
    pub active: bool,
    pub filename: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub done: bool,
    pub play_after: bool,
}

/// One entry listed from a remote directory on the phone (for the "Récupérer" feature).
#[derive(Clone)]
pub struct RemoteEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub selected: bool,
}

#[allow(dead_code)]
pub enum BgEvent {
    DevicesLoaded(Vec<Device>),
    NetworkScanDone(Vec<String>),
    WifiConnected {
        addr: String,
        success: bool,
    },
    WifiPaired {
        addr: String,
        success: bool,
        message: String,
    },
    WebcamSwitched {
        child: Option<Child>,
        /// Transport scrcpy is bound to (`ip:5555` when wireless, USB serial otherwise).
        device_id: String,
        /// True when the stream runs over wireless ADB and survives an USB unplug.
        wifi: bool,
        /// True when *we* opened TCP/5555 on the phone, so we know to close it again
        /// on shutdown. A port that was already open belongs to whoever opened it.
        opened_port: bool,
    },
    StorageInfo {
        device_id: String,
        total: String,
        used: String,
        avail: String,
        percent: f32,
    },
    BatteryInfo {
        device_id: String,
        level: u8,
        status: String,
    },
    PhoneApps {
        device_id: String,
        apps: Vec<String>,
    },
    ScreenshotReady {
        device_id: String,
        data: Vec<u8>,
    },
    ApkInstalled {
        success: bool,
        message: String,
    },
    FileTransferDone {
        success: bool,
        message: String,
    },
    RemoteDirListed {
        path: String,
        entries: Vec<RemoteEntry>,
    },
    FilePullProgress {
        done: usize,
        total: usize,
    },
    Log(String),
    SecurityScore {
        score: u8,
        issues: Vec<SecurityIssue>,
    },
    SecurityAppsList {
        packages: Vec<String>,
    },
    SecurityAppDetail {
        package: String,
        info: AppInfo,
    },
    SecurityProcesses {
        processes: Vec<ProcessInfo>,
    },
    SecurityDataUsage {
        usage: Vec<DataUsage>,
    },
    SecurityWakelocks {
        wakelocks: Vec<WakelockInfo>,
    },
    /// One full round of the background watch: the three dumps plus the alerts it
    /// already raised. The thread owns the diffing so alerts fire even when the
    /// window is hidden and no frame is being drawn.
    MonitoringWatch {
        processes: Vec<ProcessInfo>,
        usage: Vec<DataUsage>,
        wakelocks: Vec<WakelockInfo>,
        alerts: Vec<MonitoringAlert>,
    },
    SecurityPosture {
        checks: Vec<DevicePosture>,
    },
    SecurityPermissions {
        package: String,
        permissions: Vec<PermissionInfo>,
    },
    BlacklistAlert {
        found: Vec<String>,
    },
    AppActionResult {
        package: String,
        action: String,
        success: bool,
        message: String,
    },
    SecurityAppsLoadingDone,
    // Wizard events
    WizardDeviceDetected {
        info: DeviceInfo,
    },
    WizardScanProgress {
        current: usize,
        total: usize,
        package: String,
    },
    WizardScanComplete {
        apps: Vec<AppInfo>,
        posture: Vec<DevicePosture>,
        score: u8,
        issues: Vec<SecurityIssue>,
    },
    WizardPentestComplete {
        vulns: Vec<Vulnerability>,
        root: RootStatus,
        risk_score: u8,
    },
    WizardCleanProgress {
        package: String,
        action: String,
        success: bool,
        message: String,
    },
    WizardCleanComplete,
    // Model validation
    LlmModelValid {
        valid: bool,
        model: String,
        error: Option<String>,
    },
    // LLM events
    LlmAppVerdicts {
        verdicts: Vec<AppVerdict>,
    },
    LlmPentestReport {
        vulns: Vec<Vulnerability>,
    },
    LlmError {
        message: String,
    },
    // Rootability
    WizardRootabilityResult {
        rootable: bool,
        method: Option<String>,
        confidence: String,
        details: String,
    },
    // Brands events
    BrandsLoaded {
        db: BrandDb,
    },
    // History events
    HistoryLoaded {
        history: Option<DeviceHistory>,
    },
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
    Audit,
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
    pub dangerous_perm_count: u32,
    pub dangerous_perm_names: Vec<String>,
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    Danger,
    Name,
    InstallDate,
    Source,
}

/// Severity of a monitoring alert, which drives its colour in the UI.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AlertLevel {
    Info,
    Warning,
    Danger,
}

/// One thing that changed on the device while monitoring was watching.
#[derive(Clone, Debug)]
pub struct MonitoringAlert {
    /// Local time the alert fired, `HH:MM:SS`.
    pub time: String,
    pub level: AlertLevel,
    pub message: String,
}

/// What scrcpy captures on the audio side of a stream.
///
/// The distinction that matters in practice is `Media` vs `All`: Android's playback
/// capture API only hands over media, and lets apps opt out of even that, so
/// notification sounds — Snapchat, Messenger, WhatsApp — never come through it. Only
/// the whole-output capture carries them, and it takes the sound off the phone
/// speaker in exchange.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum AudioMode {
    /// No audio stream at all.
    Off,
    /// The phone's microphone.
    Mic,
    /// App playback, duplicated so the phone keeps making sound. No notifications.
    Media,
    /// Everything the phone plays, notifications included; the phone goes silent.
    All,
}

impl AudioMode {
    /// The scrcpy flags for this mode.
    pub fn scrcpy_args(self) -> Vec<String> {
        match self {
            AudioMode::Off => vec!["--no-audio".to_string()],
            AudioMode::Mic => vec!["--audio-source=mic".to_string()],
            AudioMode::Media => vec![
                "--audio-source=playback".to_string(),
                "--audio-dup".to_string(),
            ],
            AudioMode::All => vec!["--audio-source=output".to_string()],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AudioMode::Off => "\u{1f507} Aucun",
            AudioMode::Mic => "\u{1f3a4} Micro",
            AudioMode::Media => "\u{1f3b5} Média",
            AudioMode::All => "\u{1f50a} Tout",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            AudioMode::Off => "Aucun son du téléphone sur le PC.",
            AudioMode::Mic => "Le micro du téléphone (Android 11+).",
            AudioMode::Media => {
                "Le son des apps, qui continue aussi sur le téléphone \
                                 (Android 13+). Les sons de notification (Snap, Messenger…) \
                                 ne passent pas : Android ne les expose pas, et une app peut \
                                 refuser d'être capturée."
            }
            AudioMode::All => {
                "Tout ce que joue le téléphone, notifications comprises \
                               (Android 11+). En contrepartie le haut-parleur du téléphone \
                               se tait pendant le stream."
            }
        }
    }
}

#[cfg(test)]
mod audio_mode_tests {
    use super::AudioMode;

    #[test]
    fn only_whole_output_capture_carries_notifications() {
        // "playback" lets apps opt out and never exposes notification sounds, so the
        // mode meant to carry them must ask for the whole output instead.
        assert_eq!(AudioMode::All.scrcpy_args(), ["--audio-source=output"]);
        assert!(AudioMode::Media
            .scrcpy_args()
            .contains(&"--audio-dup".to_string()));
        assert_eq!(AudioMode::Off.scrcpy_args(), ["--no-audio"]);
    }
}
