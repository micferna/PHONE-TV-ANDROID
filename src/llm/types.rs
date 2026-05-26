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
pub struct RootabilityResult {
    pub rootable: bool,
    pub confidence: String,
    pub method: Option<String>,
    pub details: String,
    pub risks: String,
}
