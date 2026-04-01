use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppVerdict {
    pub package: String,
    pub verdict: String,
    pub category: String,
    pub profile: String,
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmVuln {
    pub description: String,
    pub severity: String,
    pub patchable: bool,
    pub fix_action: Option<String>,
    pub risk: String,
}

#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub api_key: String,
    pub model: String,
}
