use std::collections::HashMap;

use crate::adb::adb_device;
use crate::types::{DataUsage, ProcessInfo, WakelockInfo};

pub fn get_running_processes(device_id: &str) -> Vec<ProcessInfo> {
    let mut processes = Vec::new();

    let output = match adb_device(device_id, &["shell", "dumpsys", "activity", "processes"]) {
        Some(o) => o,
        None => return processes,
    };

    let mut current_name: Option<String> = None;
    let mut current_pid: u32 = 0;
    let mut current_memory: u64 = 0;
    let mut current_adj: i32 = 0;

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.contains("ProcessRecord{") {
            // Save previous if any
            if let Some(ref name) = current_name {
                if name.contains('.') {
                    let state = adj_to_state(current_adj);
                    processes.push(ProcessInfo {
                        package: name.clone(),
                        pid: current_pid,
                        memory_kb: current_memory,
                        adj: current_adj,
                        state,
                    });
                }
            }

            // Extract process name: last token before '}'
            // Format: ProcessRecord{hash pid:name/uid}
            current_name = None;
            current_pid = 0;
            current_memory = 0;
            current_adj = 0;

            // Try to extract name after last space, before '}'
            if let Some(brace_end) = trimmed.rfind('}') {
                let inner = &trimmed[..brace_end];
                // Look for pattern like "pid:name/uid" or just extract the process name
                if let Some(colon_pos) = inner.rfind(':') {
                    let after_colon = &inner[colon_pos + 1..];
                    let name = after_colon.split('/').next().unwrap_or(after_colon).trim();
                    if !name.is_empty() {
                        current_name = Some(name.to_string());
                    }
                }
            }
        }

        if current_name.is_some() {
            if let Some(pos) = trimmed.find("pid=") {
                let val = &trimmed[pos + 4..];
                if let Some(num) = val.split(|c: char| !c.is_ascii_digit()).next() {
                    current_pid = num.parse().unwrap_or(0);
                }
            }

            if let Some(pos) = trimmed.find("lastPss=") {
                let val = &trimmed[pos + 8..];
                if let Some(num) = val.split(|c: char| !c.is_ascii_digit()).next() {
                    current_memory = num.parse().unwrap_or(0);
                }
            } else if let Some(pos) = trimmed.find("pss=") {
                let val = &trimmed[pos + 4..];
                if let Some(num) = val.split(|c: char| !c.is_ascii_digit()).next() {
                    current_memory = num.parse().unwrap_or(0);
                }
            }

            if let Some(pos) = trimmed.find("curAdj=") {
                let val = &trimmed[pos + 7..];
                if let Some(num_str) = val.split(|c: char| !c.is_ascii_digit() && c != '-').next() {
                    current_adj = num_str.parse().unwrap_or(0);
                }
            } else if let Some(pos) = trimmed.find("setAdj=") {
                let val = &trimmed[pos + 7..];
                if let Some(num_str) = val.split(|c: char| !c.is_ascii_digit() && c != '-').next() {
                    current_adj = num_str.parse().unwrap_or(0);
                }
            }
        }
    }

    // Don't forget the last entry
    if let Some(ref name) = current_name {
        if name.contains('.') {
            let state = adj_to_state(current_adj);
            processes.push(ProcessInfo {
                package: name.clone(),
                pid: current_pid,
                memory_kb: current_memory,
                adj: current_adj,
                state,
            });
        }
    }

    processes
}

fn adj_to_state(adj: i32) -> String {
    match adj {
        a if a <= 0 => "foreground".to_string(),
        100..=299 => "visible".to_string(),
        300..=699 => "service".to_string(),
        _ => "cached".to_string(),
    }
}

pub fn get_data_usage(device_id: &str) -> Vec<DataUsage> {
    let mut usage_map: HashMap<u32, DataUsage> = HashMap::new();

    // Build UID → package name mapping
    let mut uid_to_pkg: HashMap<u32, String> = HashMap::new();
    if let Some(output) = adb_device(device_id, &["shell", "pm", "list", "packages", "-U"]) {
        for line in output.lines() {
            let line = line.trim();
            // Format: package:com.example.app uid:10123
            if let Some(rest) = line.strip_prefix("package:") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() >= 2 {
                    let pkg = parts[0].to_string();
                    if let Some(uid_str) = parts[1].strip_prefix("uid:") {
                        if let Ok(uid) = uid_str.parse::<u32>() {
                            uid_to_pkg.insert(uid, pkg);
                        }
                    }
                }
            }
        }
    }

    // Parse netstats
    if let Some(output) = adb_device(device_id, &["shell", "dumpsys", "netstats", "detail"]) {
        let mut current_uid: Option<u32> = None;
        let mut is_wifi = false;

        for line in output.lines() {
            let trimmed = line.trim();

            // Detect network type from ident line
            if trimmed.starts_with("ident=") || trimmed.contains("ident=") {
                let upper = trimmed.to_uppercase();
                is_wifi = upper.contains("WIFI");
            }

            // Parse uid=NNNNN
            if let Some(pos) = trimmed.find("uid=") {
                let val = &trimmed[pos + 4..];
                if let Some(num_str) = val.split(|c: char| !c.is_ascii_digit()).next() {
                    if let Ok(uid) = num_str.parse::<u32>() {
                        current_uid = Some(uid);
                    }
                }
            }

            if let Some(uid) = current_uid {
                let mut rx: Option<u64> = None;
                let mut tx: Option<u64> = None;

                if let Some(pos) = trimmed.find("rxBytes=") {
                    let val = &trimmed[pos + 8..];
                    if let Some(num_str) = val.split(|c: char| !c.is_ascii_digit()).next() {
                        rx = num_str.parse().ok();
                    }
                }

                if let Some(pos) = trimmed.find("txBytes=") {
                    let val = &trimmed[pos + 8..];
                    if let Some(num_str) = val.split(|c: char| !c.is_ascii_digit()).next() {
                        tx = num_str.parse().ok();
                    }
                }

                if let (Some(rx_val), Some(tx_val)) = (rx, tx) {
                    let pkg = uid_to_pkg.get(&uid).cloned().unwrap_or_default();
                    if pkg.is_empty() {
                        continue;
                    }
                    let entry = usage_map.entry(uid).or_insert_with(|| DataUsage {
                        package: pkg,
                        uid,
                        wifi_rx: 0,
                        wifi_tx: 0,
                        mobile_rx: 0,
                        mobile_tx: 0,
                    });

                    if is_wifi {
                        entry.wifi_rx += rx_val;
                        entry.wifi_tx += tx_val;
                    } else {
                        entry.mobile_rx += rx_val;
                        entry.mobile_tx += tx_val;
                    }
                }
            }
        }
    }

    let mut result: Vec<DataUsage> = usage_map.into_values().collect();
    result.sort_by(|a, b| {
        let total_b = b.wifi_rx + b.wifi_tx + b.mobile_rx + b.mobile_tx;
        let total_a = a.wifi_rx + a.wifi_tx + a.mobile_rx + a.mobile_tx;
        total_b.cmp(&total_a)
    });
    result
}

pub fn get_wakelocks(device_id: &str) -> Vec<WakelockInfo> {
    let mut wakelocks: Vec<WakelockInfo> = Vec::new();

    let output = match adb_device(device_id, &["shell", "dumpsys", "batterystats"]) {
        Some(o) => o,
        None => return wakelocks,
    };

    let mut in_wakelock_section = false;
    let mut wakelock_map: HashMap<String, u64> = HashMap::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Look for wake lock section header (case-insensitive)
        if trimmed.to_lowercase().contains("wake lock")
            && (trimmed.contains("All partial wake locks")
                || trimmed.contains("all partial wake locks")
                || trimmed.to_lowercase().contains("wake lock"))
            && trimmed.ends_with(':')
        {
            in_wakelock_section = true;
            continue;
        }

        if in_wakelock_section {
            // Empty line or new section header ends the section
            if trimmed.is_empty() || (trimmed.ends_with(':') && !trimmed.contains("realtime")) {
                in_wakelock_section = false;
                continue;
            }

            // Parse wakelock lines — various formats
            // Try to find a package name (contains dot) and a duration
            if let Some(duration_ms) = parse_duration_from_line(trimmed) {
                // Extract package name: look for tokens containing dots
                let pkg = extract_package_from_line(trimmed);
                if let Some(pkg) = pkg {
                    let entry = wakelock_map.entry(pkg).or_insert(0);
                    *entry += duration_ms;
                }
            }
        }
    }

    for (package, duration_ms) in wakelock_map {
        wakelocks.push(WakelockInfo {
            package,
            duration_ms,
            duration_human: format_duration_ms(duration_ms),
        });
    }

    wakelocks.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));
    wakelocks
}

fn parse_duration_from_line(line: &str) -> Option<u64> {
    // Try to parse durations like "1h 2m 3s 4ms", "+1h2m3s", "5m 30s 100ms", "12s 345ms"
    let mut total_ms: u64 = 0;
    let mut found_any = false;

    // Look for patterns: Nh, Nm, Ns, Nms
    let line = line.replace('+', "");

    let mut i = 0;
    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let num_str = &line[start..i];
            if let Ok(num) = num_str.parse::<u64>() {
                // Check suffix
                if i < bytes.len() {
                    if i + 1 < bytes.len() && bytes[i] == b'm' && bytes[i + 1] == b's' {
                        total_ms += num;
                        found_any = true;
                        i += 2;
                        continue;
                    } else if bytes[i] == b'h' {
                        total_ms += num * 3_600_000;
                        found_any = true;
                        i += 1;
                        continue;
                    } else if bytes[i] == b'm' {
                        total_ms += num * 60_000;
                        found_any = true;
                        i += 1;
                        continue;
                    } else if bytes[i] == b's' {
                        total_ms += num * 1_000;
                        found_any = true;
                        i += 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    if found_any { Some(total_ms) } else { None }
}

fn extract_package_from_line(line: &str) -> Option<String> {
    // Look for tokens that look like package names (contain dots, start with com/org/net/io etc)
    for token in line.split(|c: char| c.is_whitespace() || c == ':' || c == '"' || c == '\'') {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '.');
        if token.contains('.')
            && (token.starts_with("com.")
                || token.starts_with("org.")
                || token.starts_with("net.")
                || token.starts_with("io.")
                || token.starts_with("me.")
                || token.starts_with("dev.")
                || token.starts_with("app.")
                || token.starts_with("tv."))
        {
            return Some(token.to_string());
        }
    }
    None
}

fn format_duration_ms(ms: u64) -> String {
    if ms == 0 {
        return "0s".to_string();
    }

    let hours = ms / 3_600_000;
    let mins = (ms % 3_600_000) / 60_000;
    let secs = (ms % 60_000) / 1_000;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if mins > 0 {
        parts.push(format!("{}m", mins));
    }
    if secs > 0 || parts.is_empty() {
        parts.push(format!("{}s", secs));
    }
    parts.join(" ")
}
