use crate::brands::types::{BrandDb, CleanProfile};
use crate::history::types::DeviceHistory;
use crate::llm::types::AppVerdict;
use crate::pentest::rootcheck::RootStatus;
use crate::pentest::vulns::Vulnerability;
use crate::types::{AppInfo, DevicePosture, SecurityIssue};

#[derive(Clone, Debug, PartialEq)]
pub enum WizardStep {
    Detection,
    Scanning,
    Pentest,
    ProfileSelection,
    AiAnalysis,
    Cleaning,
    Report,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub serial: String,
    pub brand: String,
    pub model: String,
    pub display_name: String,
    pub android_version: String,
    pub sdk: u32,
    pub security_patch: String,
}

#[derive(Clone, Debug)]
pub struct CleanAction {
    pub package: String,
    pub action: String,
    pub description: String,
    pub selected: bool,
    pub from_ai: bool,
}

#[derive(Clone, Debug)]
pub struct VulnFix {
    pub vuln_id: String,
    pub description: String,
    pub fix_command: String,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct CleanResult {
    pub package: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

pub struct WizardState {
    pub active: bool,
    pub step: WizardStep,
    pub device_info: Option<DeviceInfo>,
    pub history: Option<DeviceHistory>,
    pub brand_db: Option<BrandDb>,
    pub apps: Vec<AppInfo>,
    pub posture: Vec<DevicePosture>,
    pub score_before: Option<(u8, Vec<SecurityIssue>)>,
    pub detection_triggered: bool,
    pub scan_loading: bool,
    pub scan_progress: f32,
    pub scan_current: usize,
    pub scan_total: usize,
    pub scan_current_package: String,
    pub vulns: Vec<Vulnerability>,
    pub root_status: Option<RootStatus>,
    pub pentest_loading: bool,
    pub risk_score: Option<u8>,
    pub selected_profile: CleanProfile,
    pub clean_actions: Vec<CleanAction>,
    pub vuln_fixes: Vec<VulnFix>,
    pub ai_loading: bool,
    pub ai_verdicts: Vec<(String, AppVerdict)>,
    pub unknown_apps: Vec<String>,
    pub cleaning: bool,
    pub clean_results: Vec<CleanResult>,
    pub clean_progress: usize,
    pub clean_total: usize,
    pub score_after: Option<(u8, Vec<SecurityIssue>)>,
    pub risk_score_after: Option<u8>,
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            active: false,
            step: WizardStep::Detection,
            device_info: None,
            history: None,
            brand_db: None,
            apps: Vec::new(),
            posture: Vec::new(),
            score_before: None,
            detection_triggered: false,
            scan_loading: false,
            scan_progress: 0.0,
            scan_current: 0,
            scan_total: 0,
            scan_current_package: String::new(),
            vulns: Vec::new(),
            root_status: None,
            pentest_loading: false,
            risk_score: None,
            selected_profile: CleanProfile::Moderate,
            clean_actions: Vec::new(),
            vuln_fixes: Vec::new(),
            ai_loading: false,
            ai_verdicts: Vec::new(),
            unknown_apps: Vec::new(),
            cleaning: false,
            clean_results: Vec::new(),
            clean_progress: 0,
            clean_total: 0,
            score_after: None,
            risk_score_after: None,
        }
    }
}
