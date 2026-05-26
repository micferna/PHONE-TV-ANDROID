pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;
use types::{CleanSession, DeviceHistory};

fn history_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("phone-tv")
        .join("history");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn load_history(serial: &str) -> Option<DeviceHistory> {
    let path = history_dir().join(format!("{}.json", serial));
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_history(history: &DeviceHistory) -> bool {
    let path = history_dir().join(format!("{}.json", history.serial));
    serde_json::to_string_pretty(history)
        .ok()
        .and_then(|s| std::fs::write(&path, s).ok())
        .is_some()
}

pub fn add_session(serial: &str, session: CleanSession) -> bool {
    let mut history = match load_history(serial) {
        Some(h) => h,
        None => return false,
    };
    history.sessions.push(session);
    save_history(&history)
}

pub fn create_history(serial: &str, brand: &str, model: &str, display_name: &str) -> DeviceHistory {
    let history = DeviceHistory {
        serial: serial.to_string(),
        brand: brand.to_string(),
        model: model.to_string(),
        display_name: display_name.to_string(),
        first_seen: chrono::Local::now().format("%Y-%m-%d").to_string(),
        sessions: Vec::new(),
    };
    let _ = save_history(&history);
    history
}

/// Returns the set of packages that were uninstalled or disabled in any prior
/// session but are present again in `current_apps`.
pub fn reappeared_packages(history: &DeviceHistory, current_apps: &[String]) -> Vec<String> {
    let previously_removed: HashSet<&str> = history
        .sessions
        .iter()
        .flat_map(|s| s.apps_removed.iter().chain(s.apps_disabled.iter()))
        .map(|s| s.as_str())
        .collect();

    current_apps
        .iter()
        .filter(|a| previously_removed.contains(a.as_str()))
        .cloned()
        .collect()
}
