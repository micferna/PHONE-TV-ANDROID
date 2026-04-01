use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanProfile {
    Minimal,
    Moderate,
    Aggressive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloatwareEntry {
    pub package: String,
    pub category: String,
    pub profile: CleanProfile,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandMeta {
    pub brand: String,
    pub display_name: String,
    pub prefixes: Vec<String>,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandDb {
    pub meta: BrandMeta,
    pub bloatware: Vec<BloatwareEntry>,
}
