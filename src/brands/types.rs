use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanProfile {
    Minimal,
    Moderate,
    Aggressive,
}

impl std::fmt::Display for CleanProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleanProfile::Minimal => write!(f, "Minimal"),
            CleanProfile::Moderate => write!(f, "Modere"),
            CleanProfile::Aggressive => write!(f, "Agressif"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BloatCategory {
    Tracker,
    Bloatware,
    Google,
    Microsoft,
    Enterprise,
    Misc,
}

impl std::fmt::Display for BloatCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BloatCategory::Tracker => write!(f, "Tracker/Pub"),
            BloatCategory::Bloatware => write!(f, "Bloatware"),
            BloatCategory::Google => write!(f, "Google"),
            BloatCategory::Microsoft => write!(f, "Microsoft"),
            BloatCategory::Enterprise => write!(f, "Enterprise"),
            BloatCategory::Misc => write!(f, "Divers"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrandMeta {
    pub brand: String,
    pub display_name: String,
    pub prefixes: Vec<String>,
    pub last_updated: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloatwareEntry {
    pub package: String,
    pub category: BloatCategory,
    pub profile: CleanProfile,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrandDb {
    pub meta: BrandMeta,
    pub bloatware: Vec<BloatwareEntry>,
}
