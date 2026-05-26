pub mod types;

use std::path::PathBuf;
use types::{BloatwareEntry, BrandDb, BrandMeta, CleanProfile};

fn brands_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("phone-tv")
        .join("brands");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn load_brand(brand_name: &str) -> Option<BrandDb> {
    let path = brands_dir().join(format!("{}.toml", brand_name.to_lowercase()));
    if path.exists() {
        let content = std::fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    } else {
        let default = match brand_name.to_lowercase().as_str() {
            "samsung" => Some(include_str!("../assets/brands/samsung.toml")),
            "motorola" => Some(include_str!("../assets/brands/motorola.toml")),
            _ => None,
        };
        if let Some(content) = default {
            let db: BrandDb = toml::from_str(content).ok()?;
            let _ = std::fs::write(&path, content);
            Some(db)
        } else {
            None
        }
    }
}

pub fn save_brand(db: &BrandDb) -> bool {
    let path = brands_dir().join(format!("{}.toml", db.meta.brand));
    toml::to_string_pretty(db)
        .ok()
        .and_then(|s| std::fs::write(&path, s).ok())
        .is_some()
}

pub fn add_entry(brand_name: &str, entry: BloatwareEntry) -> bool {
    let mut db = match load_brand(brand_name) {
        Some(db) => db,
        None => BrandDb {
            meta: BrandMeta {
                brand: brand_name.to_lowercase(),
                display_name: brand_name.to_string(),
                prefixes: Vec::new(),
                last_updated: chrono::Local::now().format("%Y-%m-%d").to_string(),
            },
            bloatware: Vec::new(),
        },
    };
    if db.bloatware.iter().any(|e| e.package == entry.package) {
        return true;
    }
    db.bloatware.push(entry);
    db.meta.last_updated = chrono::Local::now().format("%Y-%m-%d").to_string();
    save_brand(&db)
}

pub fn entries_for_profile<'a>(db: &'a BrandDb, profile: &CleanProfile) -> Vec<&'a BloatwareEntry> {
    db.bloatware
        .iter()
        .filter(|e| match profile {
            CleanProfile::Minimal => e.profile == CleanProfile::Minimal,
            CleanProfile::Moderate => {
                e.profile == CleanProfile::Minimal || e.profile == CleanProfile::Moderate
            }
            CleanProfile::Aggressive => true,
        })
        .collect()
}
