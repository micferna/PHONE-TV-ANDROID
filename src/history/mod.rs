pub mod types;

use std::path::PathBuf;
use types::{CleanSession, DeviceHistory, DiffResult};

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

pub fn compute_diff(history: &DeviceHistory, current_apps: &[String]) -> DiffResult {
    let last_session = history.sessions.last().cloned();

    if let Some(ref session) = last_session {
        let previous: std::collections::HashSet<&str> = session
            .apps_removed.iter()
            .chain(session.apps_disabled.iter())
            .map(|s| s.as_str())
            .collect();

        let new_apps: Vec<String> = current_apps
            .iter()
            .filter(|app| previous.contains(app.as_str()))
            .cloned()
            .collect();

        DiffResult {
            new_apps,
            removed_apps: Vec::new(),
            last_session: Some(session.clone()),
        }
    } else {
        DiffResult {
            new_apps: Vec::new(),
            removed_apps: Vec::new(),
            last_session: None,
        }
    }
}
