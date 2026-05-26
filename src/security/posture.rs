use std::process::Command;

use crate::adb::adb_device;
use crate::types::{DevicePosture, PostureStatus};

pub fn check_device_posture(device_id: &str) -> Vec<DevicePosture> {
    let mut checks = Vec::new();

    // 1. ADB activé
    if let Some(val) = adb_device(
        device_id,
        &["shell", "settings", "get", "global", "adb_enabled"],
    ) {
        checks.push(DevicePosture {
            name: "ADB activé".into(),
            value: val.trim().to_string(),
            status: PostureStatus::Warning, // Always warning — we need ADB
            fix_command: None,
        });
    }

    // 2. Sources inconnues
    if let Some(val) = adb_device(
        device_id,
        &[
            "shell",
            "settings",
            "get",
            "secure",
            "install_non_market_apps",
        ],
    ) {
        let trimmed = val.trim();
        checks.push(DevicePosture {
            name: "Sources inconnues".into(),
            value: trimmed.to_string(),
            status: if trimmed == "1" {
                PostureStatus::Bad
            } else {
                PostureStatus::Good
            },
            fix_command: None,
        });
    }

    // 3. Mode développeur (required for ADB — never show as Bad)
    if let Some(val) = adb_device(
        device_id,
        &[
            "shell",
            "settings",
            "get",
            "global",
            "development_settings_enabled",
        ],
    ) {
        let trimmed = val.trim();
        checks.push(DevicePosture {
            name: "Mode développeur".into(),
            value: if trimmed == "1" {
                "Activé (requis pour ADB)".into()
            } else {
                trimmed.to_string()
            },
            status: if trimmed == "1" {
                PostureStatus::Warning
            } else {
                PostureStatus::Good
            },
            fix_command: None,
        });
    }

    // 4. Play Protect
    if let Some(val) = adb_device(
        device_id,
        &[
            "shell",
            "settings",
            "get",
            "global",
            "package_verifier_enable",
        ],
    ) {
        let trimmed = val.trim();
        checks.push(DevicePosture {
            name: "Play Protect".into(),
            value: trimmed.to_string(),
            status: if trimmed == "1" {
                PostureStatus::Good
            } else {
                PostureStatus::Bad
            },
            fix_command: Some("settings put global package_verifier_enable 1".into()),
        });
    }

    // 5. Vérification ADB
    if let Some(val) = adb_device(
        device_id,
        &[
            "shell",
            "settings",
            "get",
            "global",
            "verifier_verify_adb_installs",
        ],
    ) {
        let trimmed = val.trim();
        checks.push(DevicePosture {
            name: "Vérification ADB".into(),
            value: trimmed.to_string(),
            status: if trimmed == "0" {
                PostureStatus::Bad
            } else {
                PostureStatus::Good
            },
            fix_command: Some("settings put global verifier_verify_adb_installs 1".into()),
        });
    }

    // 6. Services accessibilité
    if let Some(val) = adb_device(
        device_id,
        &[
            "shell",
            "settings",
            "get",
            "secure",
            "enabled_accessibility_services",
        ],
    ) {
        let trimmed = val.trim();
        let is_empty = trimmed.is_empty() || trimmed == "null";
        checks.push(DevicePosture {
            name: "Services accessibilité".into(),
            value: if is_empty {
                "Aucun".into()
            } else {
                trimmed.to_string()
            },
            status: if is_empty {
                PostureStatus::Good
            } else {
                PostureStatus::Bad
            },
            fix_command: None,
        });
    }

    // 7. Clavier par défaut
    if let Some(val) = adb_device(
        device_id,
        &["shell", "settings", "get", "secure", "default_input_method"],
    ) {
        checks.push(DevicePosture {
            name: "Clavier par défaut".into(),
            value: val.trim().to_string(),
            status: PostureStatus::Warning, // Info only
            fix_command: None,
        });
    }

    // 8. Verrouillage écran
    if let Some(val) = adb_device(
        device_id,
        &[
            "shell",
            "settings",
            "get",
            "secure",
            "lockscreen.password_type",
        ],
    ) {
        let trimmed = val.trim();
        let status = trimmed
            .parse::<i64>()
            .map(|v| {
                if v >= 65536 {
                    PostureStatus::Good
                } else {
                    PostureStatus::Bad
                }
            })
            .unwrap_or(PostureStatus::Bad);
        checks.push(DevicePosture {
            name: "Verrouillage écran".into(),
            value: trimmed.to_string(),
            status,
            fix_command: None,
        });
    }

    // 9. Mode localisation
    if let Some(val) = adb_device(
        device_id,
        &["shell", "settings", "get", "secure", "location_mode"],
    ) {
        checks.push(DevicePosture {
            name: "Mode localisation".into(),
            value: val.trim().to_string(),
            status: PostureStatus::Warning, // Info only
            fix_command: None,
        });
    }

    checks
}

pub fn fix_setting(device_id: &str, command: &str) -> bool {
    let full_cmd = format!("adb -s {} shell {}", device_id, command);
    let parts: Vec<&str> = full_cmd.split_whitespace().collect();
    if parts.len() < 2 {
        return false;
    }

    Command::new("adb")
        .args(["-s", device_id, "shell"])
        .args(command.split_whitespace())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
