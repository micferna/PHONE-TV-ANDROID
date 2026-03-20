use std::process::Command;

use crate::adb::{adb_device, adb_fire};
use crate::types::{AppFilter, AppInfo, AppInstaller};

pub fn list_packages(device_id: &str, filter: AppFilter) -> Vec<String> {
    let args = match filter {
        AppFilter::All => vec!["shell", "pm", "list", "packages"],
        AppFilter::ThirdParty => vec!["shell", "pm", "list", "packages", "-3"],
        AppFilter::System => vec!["shell", "pm", "list", "packages", "-s"],
        AppFilter::Disabled => vec!["shell", "pm", "list", "packages", "-d"],
    };

    adb_device(device_id, &args)
        .map(|output| {
            output
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("package:")
                        .map(|s| s.trim().to_string())
                })
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn get_app_detail(device_id: &str, package: &str) -> Option<AppInfo> {
    let output = adb_device(device_id, &["shell", "dumpsys", "package", package])?;

    let mut info = AppInfo {
        package: package.to_string(),
        enabled: true,
        ..Default::default()
    };

    let mut in_user0 = false;

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.contains("User 0:") || trimmed.contains("userId=0") {
            in_user0 = true;
        }

        if let Some(pos) = trimmed.find("versionName=") {
            let val = &trimmed[pos + 12..];
            info.version_name = val.split_whitespace().next().unwrap_or(val).to_string();
        }

        if let Some(pos) = trimmed.find("versionCode=") {
            let val = &trimmed[pos + 12..];
            if let Some(num_str) = val.split(|c: char| !c.is_ascii_digit()).next() {
                info.version_code = num_str.parse().unwrap_or(0);
            }
        }

        if let Some(pos) = trimmed.find("firstInstallTime=") {
            info.first_install = trimmed[pos + 17..].trim().to_string();
        }

        if let Some(pos) = trimmed.find("lastUpdateTime=") {
            info.last_update = trimmed[pos + 15..].trim().to_string();
        }

        if let Some(pos) = trimmed.find("installerPackageName=") {
            let val = trimmed[pos + 21..].trim();
            let installer_name = val.split_whitespace().next().unwrap_or(val);
            info.installer = match installer_name {
                "com.android.vending" => AppInstaller::PlayStore,
                "com.google.android.packageinstaller" => AppInstaller::Sideload,
                "null" | "" => AppInstaller::Unknown,
                _ => AppInstaller::Adb,
            };
        }

        if let Some(pos) = trimmed.find("targetSdk=") {
            let val = &trimmed[pos + 10..];
            if let Some(num_str) = val.split(|c: char| !c.is_ascii_digit()).next() {
                info.target_sdk = num_str.parse().unwrap_or(0);
            }
        }

        if in_user0 {
            if let Some(pos) = trimmed.find("enabled=") {
                let val = &trimmed[pos + 8..];
                let enabled_val = val.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("0");
                // enabled=0 means default (enabled), enabled=1 means disabled by user,
                // enabled=2 means disabled, enabled=3 means disabled by user
                // Actually: 0=default(enabled), 1=enabled, 2=disabled, 3=disabled by user
                info.enabled = enabled_val != "2" && enabled_val != "3";
                in_user0 = false;
            }
        }
    }

    info.details_loaded = true;
    Some(info)
}

pub fn uninstall_app(device_id: &str, package: &str) -> (bool, String) {
    // Try `adb uninstall` first (works for user-installed apps)
    if let Ok(o) = Command::new("adb")
        .args(["-s", device_id, "uninstall", package])
        .output()
    {
        let stdout = String::from_utf8_lossy(&o.stdout).to_string();
        if stdout.contains("Success") {
            return (true, format!("{} désinstallé", package));
        }
    }

    // Fallback: `pm uninstall --user 0` (removes for current user, keeps on device)
    if let Ok(o) = Command::new("adb")
        .args(["-s", device_id, "shell", "pm", "uninstall", "--user", "0", package])
        .output()
    {
        let stdout = String::from_utf8_lossy(&o.stdout).to_string();
        if stdout.contains("Success") {
            return (true, format!("{} désinstallé (utilisateur)", package));
        }
        return (false, format!("Échec : {}", stdout.trim()));
    }

    (false, "Erreur de commande ADB".to_string())
}

pub fn disable_app(device_id: &str, package: &str) -> (bool, String) {
    match adb_device(device_id, &["shell", "pm", "disable-user", "--user", "0", package]) {
        Some(output) => {
            let success = output.to_lowercase().contains("disabled");
            (success, output.trim().to_string())
        }
        None => (false, "Commande échouée".to_string()),
    }
}

pub fn enable_app(device_id: &str, package: &str) -> (bool, String) {
    match adb_device(device_id, &["shell", "pm", "enable", package]) {
        Some(output) => {
            let success = output.to_lowercase().contains("enabled");
            (success, output.trim().to_string())
        }
        None => (false, "Commande échouée".to_string()),
    }
}

pub fn force_stop_app(device_id: &str, package: &str) {
    adb_fire(device_id, &["shell", "am", "force-stop", package]);
}

pub fn clear_app_data(device_id: &str, package: &str) -> (bool, String) {
    match adb_device(device_id, &["shell", "pm", "clear", package]) {
        Some(output) => {
            let success = output.trim().to_lowercase().contains("success");
            (success, output.trim().to_string())
        }
        None => (false, "Commande échouée".to_string()),
    }
}
