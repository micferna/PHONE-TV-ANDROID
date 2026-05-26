use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceHistory {
    pub serial: String,
    pub brand: String,
    pub model: String,
    pub display_name: String,
    pub first_seen: String,
    pub sessions: Vec<CleanSession>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanSession {
    pub date: String,
    pub android_version: String,
    pub security_patch: String,
    pub score_before: u8,
    pub score_after: u8,
    pub risk_score_before: u8,
    pub risk_score_after: u8,
    pub apps_removed: Vec<String>,
    pub apps_disabled: Vec<String>,
    pub apps_failed: Vec<String>,
    pub vulns_found: u32,
    pub vulns_patched: u32,
    pub profile_used: String,
    pub ai_suggestions_accepted: u32,
}
