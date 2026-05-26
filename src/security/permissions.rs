use std::process::Command;

use crate::adb::adb_device;
use crate::types::PermissionInfo;

const DANGEROUS_PERMISSIONS: &[&str] = &[
    "CAMERA",
    "RECORD_AUDIO",
    "ACCESS_FINE_LOCATION",
    "ACCESS_COARSE_LOCATION",
    "READ_CONTACTS",
    "WRITE_CONTACTS",
    "READ_SMS",
    "SEND_SMS",
    "READ_CALL_LOG",
    "READ_PHONE_STATE",
    "READ_EXTERNAL_STORAGE",
    "WRITE_EXTERNAL_STORAGE",
    "READ_CALENDAR",
    "WRITE_CALENDAR",
    "BODY_SENSORS",
    "ACCESS_BACKGROUND_LOCATION",
];

fn appops_to_permission(op: &str) -> Option<&'static str> {
    match op {
        "CAMERA" => Some("android.permission.CAMERA"),
        "COARSE_LOCATION" => Some("android.permission.ACCESS_COARSE_LOCATION"),
        "FINE_LOCATION" => Some("android.permission.ACCESS_FINE_LOCATION"),
        "READ_CONTACTS" => Some("android.permission.READ_CONTACTS"),
        "WRITE_CONTACTS" => Some("android.permission.WRITE_CONTACTS"),
        "RECORD_AUDIO" => Some("android.permission.RECORD_AUDIO"),
        "READ_SMS" => Some("android.permission.READ_SMS"),
        "SEND_SMS" => Some("android.permission.SEND_SMS"),
        "READ_CALL_LOG" => Some("android.permission.READ_CALL_LOG"),
        "READ_PHONE_STATE" => Some("android.permission.READ_PHONE_STATE"),
        "READ_EXTERNAL_STORAGE" => Some("android.permission.READ_EXTERNAL_STORAGE"),
        "WRITE_EXTERNAL_STORAGE" => Some("android.permission.WRITE_EXTERNAL_STORAGE"),
        "READ_CALENDAR" => Some("android.permission.READ_CALENDAR"),
        "WRITE_CALENDAR" => Some("android.permission.WRITE_CALENDAR"),
        "BODY_SENSORS" => Some("android.permission.BODY_SENSORS"),
        _ => None,
    }
}

fn parse_appops_time(time_str: &str) -> Option<String> {
    // Parse time=+1d2h3m4s or time=+5m30s etc.
    let s = time_str.trim_start_matches('+');
    let mut remaining = s;
    let mut days = 0u64;
    let mut hours = 0u64;
    let mut mins = 0u64;
    let mut secs = 0u64;

    if let Some(pos) = remaining.find('d') {
        days = remaining[..pos].parse().unwrap_or(0);
        remaining = &remaining[pos + 1..];
    }
    if let Some(pos) = remaining.find('h') {
        hours = remaining[..pos].parse().unwrap_or(0);
        remaining = &remaining[pos + 1..];
    }
    if let Some(pos) = remaining.find('m') {
        // make sure it's not 'ms'
        if pos + 1 < remaining.len() && remaining.as_bytes().get(pos + 1) == Some(&b's') {
            // this is milliseconds at the end, ignore
        } else {
            mins = remaining[..pos].parse().unwrap_or(0);
            remaining = &remaining[pos + 1..];
        }
    }
    if let Some(pos) = remaining.find('s') {
        // Check it's not part of 'ms'
        if pos > 0 && remaining.as_bytes().get(pos - 1) == Some(&b'm') {
            // milliseconds, skip
        } else {
            secs = remaining[..pos].parse().unwrap_or(0);
        }
    }

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}j", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if mins > 0 {
        parts.push(format!("{}min", mins));
    }
    if secs > 0 && days == 0 {
        parts.push(format!("{}s", secs));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("il y a {}", parts.join(" ")))
    }
}

pub fn get_app_permissions(device_id: &str, package: &str) -> Vec<PermissionInfo> {
    let mut permissions: Vec<PermissionInfo> = Vec::new();

    // Parse runtime permissions from dumpsys package
    if let Some(dump) = adb_device(device_id, &["shell", "dumpsys", "package", package]) {
        let mut in_runtime_perms = false;

        for line in dump.lines() {
            let trimmed = line.trim();

            if trimmed.contains("runtime permissions:") {
                in_runtime_perms = true;
                continue;
            }

            if in_runtime_perms {
                // End of section: blank line or line not starting with a permission
                if trimmed.is_empty() {
                    in_runtime_perms = false;
                    continue;
                }

                // Parse: android.permission.CAMERA: granted=true, flags=[ USER_SET ]
                if let Some(colon_pos) = trimmed.find(": granted=") {
                    let name = trimmed[..colon_pos].trim().to_string();
                    let granted = trimmed.contains("granted=true");

                    let short_name = name.strip_prefix("android.permission.").unwrap_or(&name);
                    let dangerous = DANGEROUS_PERMISSIONS.contains(&short_name);

                    permissions.push(PermissionInfo {
                        name,
                        granted,
                        last_used: None,
                        dangerous,
                        is_runtime: true,
                    });
                }
            }
        }
    }

    // Parse appops for last_used times
    if let Some(appops) = adb_device(device_id, &["shell", "cmd", "appops", "get", package]) {
        let mut current_op: Option<String> = None;

        for line in appops.lines() {
            let trimmed = line.trim();

            // Lines like "CAMERA: allow; time=+5m30s ago"
            // or "CAMERA:" followed by indented lines
            if !trimmed.starts_with(' ') && !trimmed.is_empty() {
                // Extract op name
                if let Some(colon_pos) = trimmed.find(':') {
                    current_op = Some(trimmed[..colon_pos].trim().to_string());
                }
            }

            // Look for time= on this line
            if let Some(time_pos) = trimmed.find("time=") {
                let time_part = &trimmed[time_pos + 5..];
                // Extract until space or end
                let time_val = time_part.split_whitespace().next().unwrap_or(time_part);

                if let Some(human) = parse_appops_time(time_val) {
                    if let Some(ref op) = current_op {
                        if let Some(perm_name) = appops_to_permission(op) {
                            // Find matching permission and set last_used
                            for perm in &mut permissions {
                                if perm.name == perm_name {
                                    perm.last_used = Some(human.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    permissions
}

pub fn revoke_permission(device_id: &str, package: &str, permission: &str) -> (bool, String) {
    match Command::new("adb")
        .args([
            "-s", device_id, "shell", "pm", "revoke", package, permission,
        ])
        .output()
    {
        Ok(o) => {
            let success = o.status.success();
            let msg = if success {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            } else {
                String::from_utf8_lossy(&o.stderr).trim().to_string()
            };
            (
                success,
                if msg.is_empty() {
                    "OK".to_string()
                } else {
                    msg
                },
            )
        }
        Err(e) => (false, format!("Erreur: {}", e)),
    }
}
