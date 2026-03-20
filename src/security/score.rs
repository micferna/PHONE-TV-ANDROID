use crate::adb::adb_device;
use crate::types::{SecurityIssue, Severity};

pub fn calculate_score(device_id: &str) -> (u8, Vec<SecurityIssue>) {
    let mut issues = Vec::new();
    let mut deductions: i32 = 0;

    // 1. Developer mode (-10)
    if let Some(val) = adb_device(device_id, &["shell", "settings", "get", "global", "development_settings_enabled"]) {
        if val.trim() == "1" {
            deductions += 10;
            issues.push(SecurityIssue {
                id: "dev_mode".into(),
                description: "Mode développeur activé".into(),
                severity: Severity::Warning,
                points: -10,
                fixable: true,
                fix_command: Some("settings put global development_settings_enabled 0".into()),
            });
        }
    }

    // 2. Play Protect (-25)
    if let Some(val) = adb_device(device_id, &["shell", "settings", "get", "global", "package_verifier_enable"]) {
        let trimmed = val.trim();
        if trimmed == "0" || trimmed == "null" {
            deductions += 25;
            issues.push(SecurityIssue {
                id: "play_protect".into(),
                description: "Play Protect désactivé".into(),
                severity: Severity::Critical,
                points: -25,
                fixable: true,
                fix_command: Some("settings put global package_verifier_enable 1".into()),
            });
        }
    }

    // 3. Unknown sources (-20)
    if let Some(val) = adb_device(device_id, &["shell", "settings", "get", "secure", "install_non_market_apps"]) {
        let trimmed = val.trim();
        if trimmed == "1" {
            deductions += 20;
            issues.push(SecurityIssue {
                id: "unknown_sources".into(),
                description: "Sources inconnues activées".into(),
                severity: Severity::Critical,
                points: -20,
                fixable: true,
                fix_command: Some("settings put secure install_non_market_apps 0".into()),
            });
        }
        // "null" on Android 8+ — skip
    }

    // 4. Accessibility services (-15 cap)
    if let Some(val) = adb_device(device_id, &["shell", "settings", "get", "secure", "enabled_accessibility_services"]) {
        let trimmed = val.trim();
        if !trimmed.is_empty() && trimmed != "null" {
            deductions += 15;
            issues.push(SecurityIssue {
                id: "accessibility".into(),
                description: format!("Services d'accessibilité actifs: {}", trimmed),
                severity: Severity::Warning,
                points: -15,
                fixable: false,
                fix_command: None,
            });
        }
    }

    // 5. Sideloaded apps (-3 each, max -15)
    if let Some(output) = adb_device(device_id, &["shell", "pm", "list", "packages", "-3", "-i"]) {
        let mut sideloaded_count = 0;
        let mut sideloaded_names = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Format: package:com.example.app  installer=com.android.vending
            let is_store = line.contains("installer=com.android.vending")
                || line.contains("installer=com.google.android.packageinstaller");
            if !is_store {
                sideloaded_count += 1;
                if let Some(pkg) = line.strip_prefix("package:") {
                    let pkg_name = pkg.split_whitespace().next().unwrap_or(pkg);
                    sideloaded_names.push(pkg_name.to_string());
                }
            }
        }
        if sideloaded_count > 0 {
            let penalty = (sideloaded_count * 3).min(15);
            deductions += penalty;
            issues.push(SecurityIssue {
                id: "sideloaded".into(),
                description: format!("{} app(s) non Play Store: {}", sideloaded_count,
                    sideloaded_names.iter().take(5).cloned().collect::<Vec<_>>().join(", ")),
                severity: Severity::Warning,
                points: -penalty,
                fixable: false,
                fix_command: None,
            });
        }
    }

    // 6. Dangerous permissions apps (-2 each, max -10)
    // Get third-party packages
    if let Some(output) = adb_device(device_id, &["shell", "pm", "list", "packages", "-3"]) {
        let packages: Vec<String> = output.lines()
            .filter_map(|l| l.strip_prefix("package:").map(|s| s.trim().to_string()))
            .collect();

        let mut dangerous_apps = Vec::new();
        for pkg in &packages {
            if let Some(dump) = adb_device(device_id, &["shell", "dumpsys", "package", pkg]) {
                let mut in_runtime_perms = false;
                let mut granted_count = 0;
                for line in dump.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains("runtime permissions:") {
                        in_runtime_perms = true;
                        continue;
                    }
                    if in_runtime_perms {
                        if trimmed.is_empty() || (!trimmed.starts_with("android.permission.") && !trimmed.starts_with("com.")) {
                            if !trimmed.contains("granted=") {
                                in_runtime_perms = false;
                                continue;
                            }
                        }
                        if trimmed.contains("granted=true") {
                            granted_count += 1;
                        }
                    }
                }
                if granted_count >= 3 {
                    dangerous_apps.push(pkg.clone());
                }
            }
        }

        if !dangerous_apps.is_empty() {
            let count = dangerous_apps.len() as i32;
            let penalty = (count * 2).min(10);
            deductions += penalty;
            issues.push(SecurityIssue {
                id: "dangerous_perms".into(),
                description: format!("{} app(s) avec 3+ permissions dangereuses: {}",
                    count, dangerous_apps.iter().take(5).cloned().collect::<Vec<_>>().join(", ")),
                severity: Severity::Info,
                points: -penalty,
                fixable: false,
                fix_command: None,
            });
        }
    }

    let score = 0_i32.max(100 - deductions) as u8;
    (score, issues)
}
