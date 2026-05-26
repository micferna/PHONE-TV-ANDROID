use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupManifest {
    pub serial: String,
    pub timestamp: String,
    pub apks: Vec<BackedUpApk>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackedUpApk {
    pub package: String,
    pub file: String,
}

fn backups_root() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("phone-tv")
        .join("backups");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn session_dir(serial: &str, timestamp: &str) -> PathBuf {
    let dir = backups_root().join(serial).join(timestamp);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Pull the base APK for a package into `dest_dir`. Returns the local file path on success.
pub fn backup_apk(device_id: &str, package: &str, dest_dir: &Path) -> Option<PathBuf> {
    let remote_path = Command::new("adb")
        .args(["-s", device_id, "shell", "pm", "path", package])
        .output()
        .ok()?;

    if !remote_path.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&remote_path.stdout);
    let remote = stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("package:"))?
        .trim()
        .to_string();

    let local = dest_dir.join(format!("{}.apk", package));
    let local_str = local.to_string_lossy().to_string();

    let pulled = Command::new("adb")
        .args(["-s", device_id, "pull", &remote, &local_str])
        .output()
        .ok()?;

    if pulled.status.success() && local.exists() {
        Some(local)
    } else {
        None
    }
}

pub fn write_manifest(dir: &Path, manifest: &BackupManifest) -> bool {
    let path = dir.join("manifest.json");
    serde_json::to_string_pretty(manifest)
        .ok()
        .and_then(|s| std::fs::write(&path, s).ok())
        .is_some()
}

/// List backup sessions for a given serial, newest first.
pub fn list_sessions(serial: &str) -> Vec<(String, BackupManifest)> {
    let root = backups_root().join(serial);
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let manifest_path = entry.path().join("manifest.json");
            if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                if let Ok(manifest) = serde_json::from_str::<BackupManifest>(&content) {
                    sessions.push((entry.path().to_string_lossy().to_string(), manifest));
                }
            }
        }
    }
    sessions.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
    sessions
}

pub fn restore_apk(device_id: &str, local_apk: &str) -> (bool, String) {
    let output = Command::new("adb")
        .args(["-s", device_id, "install", "-r", local_apk])
        .output();
    match output {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            (combined.contains("Success"), combined.trim().to_string())
        }
        Err(e) => (false, format!("Erreur adb: {}", e)),
    }
}
