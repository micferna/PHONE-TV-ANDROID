use std::collections::HashMap;

use crate::adb::adb_device;
use crate::types::{DataUsage, ProcessInfo, WakelockInfo};

/// Map every installed package to its UID.
///
/// `dumpsys netstats` and `dumpsys batterystats` both report UIDs, never package
/// names, so both views need this table to show something a human can read.
fn uid_package_map(device_id: &str) -> HashMap<u32, String> {
    let mut map: HashMap<u32, String> = HashMap::new();
    let Some(output) = adb_device(device_id, &["shell", "pm", "list", "packages", "-U"]) else {
        return map;
    };

    for line in output.lines() {
        // Format: package:com.example.app uid:10123
        let Some(rest) = line.trim().strip_prefix("package:") else {
            continue;
        };
        let mut pkg: Option<&str> = None;
        let mut uid: Option<u32> = None;
        for part in rest.split_whitespace() {
            if let Some(u) = part.strip_prefix("uid:") {
                uid = u.parse().ok();
            } else if pkg.is_none() {
                pkg = Some(part);
            }
        }
        if let (Some(p), Some(u)) = (pkg, uid) {
            // Several packages can share a UID; the first one is a good enough label.
            map.entry(u).or_insert_with(|| p.to_string());
        }
    }
    map
}

/// Read `key=` from a line and return the raw token that follows it.
fn field_after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pos = line.find(key)?;
    let rest = &line[pos + key.len()..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ',' || c == '}')
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

fn int_after(line: &str, key: &str) -> Option<i64> {
    field_after(line, key)?.parse().ok()
}

fn uint_after(line: &str, key: &str) -> Option<u64> {
    field_after(line, key)?.parse().ok()
}

// ---------------------------------------------------------------------------
// PROCESSES
// ---------------------------------------------------------------------------

pub fn get_running_processes(device_id: &str) -> Vec<ProcessInfo> {
    let mut processes = Vec::new();

    let output = match adb_device(device_id, &["shell", "dumpsys", "activity", "processes"]) {
        Some(o) => o,
        None => return processes,
    };

    let mut current: Option<ProcessInfo> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        // A process block opens with the record itself:
        //   *APP* UID 10142 ProcessRecord{5baaf1 10517:com.google.android.youtube/u0a142}
        // The same record is echoed further down as `proc=ProcessRecord{...}` inside the
        // per-UID summary; counting those would list every process twice.
        if trimmed.contains("ProcessRecord{") && !trimmed.starts_with("proc=") {
            if let Some(proc) = current.take() {
                processes.push(proc);
            }
            current = parse_process_record(trimmed);
            continue;
        }

        let Some(proc) = current.as_mut() else {
            continue;
        };

        // lastPss=158MB lastSwapPss=77MB ...  — the value carries its unit, and on a
        // French-locale device the decimal separator is a comma ("1,6MB").
        if let Some(raw) = field_after(trimmed, "lastPss=") {
            if let Some(kb) = parse_size_kb(raw) {
                proc.memory_kb = kb;
            }
        }

        // oom adj: max=1001 curRaw=915 setRaw=915 cur=915 set=915
        if trimmed.starts_with("oom adj:") {
            if let Some(adj) = int_after(trimmed, "cur=").or_else(|| int_after(trimmed, "set=")) {
                proc.adj = adj as i32;
                proc.state = adj_to_state(proc.adj);
            }
        } else if let Some(adj) =
            int_after(trimmed, "curAdj=").or_else(|| int_after(trimmed, "setAdj="))
        {
            // Older platforms spelled it curAdj=/setAdj= on their own line.
            proc.adj = adj as i32;
            proc.state = adj_to_state(proc.adj);
        }
    }

    if let Some(proc) = current.take() {
        processes.push(proc);
    }

    processes
}

/// Pull the package and PID out of `ProcessRecord{<hash> <pid>:<name>/<uid>}`.
fn parse_process_record(line: &str) -> Option<ProcessInfo> {
    let start = line.find("ProcessRecord{")? + "ProcessRecord{".len();
    let rest = &line[start..];
    let end = rest.find('}').unwrap_or(rest.len());
    let inner = &rest[..end];

    // inner = "5baaf1 10517:com.google.android.youtube/u0a142"
    let colon = inner.rfind(':')?;
    let name = inner[colon + 1..].split('/').next().unwrap_or("").trim();
    if name.is_empty() || !name.contains('.') {
        return None;
    }
    let pid = inner[..colon]
        .split_whitespace()
        .next_back()
        .and_then(|t| t.parse().ok())
        .unwrap_or(0);

    Some(ProcessInfo {
        package: name.to_string(),
        pid,
        memory_kb: 0,
        adj: 0,
        // dumpsys only prints an oom adj line for part of the process list; leaving the
        // rest at adj 0 would label every one of them "foreground", which is a guess.
        state: "inconnu".to_string(),
    })
}

/// Parse a memory figure such as `158MB`, `1,6MB`, `0,00` or a bare KB count.
fn parse_size_kb(raw: &str) -> Option<u64> {
    let digits_end = raw
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != ',')
        .unwrap_or(raw.len());
    if digits_end == 0 {
        return None;
    }
    let value: f64 = raw[..digits_end].replace(',', ".").parse().ok()?;
    let unit: String = raw[digits_end..]
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();

    let kb = match unit.to_ascii_uppercase().as_str() {
        "GB" => value * 1024.0 * 1024.0,
        "MB" => value * 1024.0,
        "B" => value / 1024.0,
        // "KB", "K" and the unit-less legacy form are already kilobytes.
        _ => value,
    };
    Some(kb.round().max(0.0) as u64)
}

fn adj_to_state(adj: i32) -> String {
    match adj {
        a if a <= 0 => "foreground".to_string(),
        1..=99 => "foreground".to_string(),
        100..=299 => "visible".to_string(),
        300..=699 => "service".to_string(),
        _ => "cached".to_string(),
    }
}

// ---------------------------------------------------------------------------
// DATA USAGE
// ---------------------------------------------------------------------------

pub fn get_data_usage(device_id: &str) -> Vec<DataUsage> {
    let uid_to_pkg = uid_package_map(device_id);
    let mut usage_map: HashMap<u32, DataUsage> = HashMap::new();

    let Some(output) = adb_device(device_id, &["shell", "dumpsys", "netstats", "detail"]) else {
        return Vec::new();
    };

    // The dump is a list of buckets. Each one opens with a header naming the network
    // and the UID it belongs to:
    //   ident=[{type=1, ...}] uid=10142 set=DEFAULT tag=0x0
    // and is followed by indented per-interval samples:
    //   st=1787745600 rb=624 rp=6 tb=296 tp=4 op=0
    // `rb`/`tb` are the received/transmitted byte counts.
    let mut current: Option<(u32, bool)> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.contains("uid=") && trimmed.contains("set=") && trimmed.contains("tag=") {
            current = parse_bucket_header(trimmed);
            continue;
        }

        let Some((uid, is_wifi)) = current else {
            continue;
        };

        // Newer platforms use rb=/tb=, older ones rxBytes=/txBytes=.
        let rx = uint_after(trimmed, "rb=").or_else(|| uint_after(trimmed, "rxBytes="));
        let tx = uint_after(trimmed, "tb=").or_else(|| uint_after(trimmed, "txBytes="));
        let (Some(rx), Some(tx)) = (rx, tx) else {
            continue;
        };

        let entry = usage_map.entry(uid).or_insert_with(|| DataUsage {
            package: uid_to_pkg
                .get(&uid)
                .cloned()
                .unwrap_or_else(|| format!("uid {}", uid)),
            uid,
            wifi_rx: 0,
            wifi_tx: 0,
            mobile_rx: 0,
            mobile_tx: 0,
        });

        if is_wifi {
            entry.wifi_rx += rx;
            entry.wifi_tx += tx;
        } else {
            entry.mobile_rx += rx;
            entry.mobile_tx += tx;
        }
    }

    let mut result: Vec<DataUsage> = usage_map.into_values().collect();
    result.sort_by_key(|u| std::cmp::Reverse(u.wifi_rx + u.wifi_tx + u.mobile_rx + u.mobile_tx));
    result
}

/// Decode a bucket header, or `None` when the bucket must not be counted.
fn parse_bucket_header(line: &str) -> Option<(u32, bool)> {
    // Tagged buckets break the same traffic down by in-app tag; they are a subset of
    // tag=0x0, so summing every tag would count most bytes several times over.
    if field_after(line, "tag=")? != "0x0" {
        return None;
    }

    // uid=-1 is the interface-wide rollup, not an app.
    let uid = int_after(line, "uid=")?;
    let uid = u32::try_from(uid).ok()?;

    // ident=[{type=1, ratType=COMBINED, ...}] — type 1 is wifi, 0 is mobile.
    // `ratType=` cannot be mistaken for `type=`: the match is case-sensitive.
    let is_wifi = int_after(line, "type=") == Some(1);

    Some((uid, is_wifi))
}

// ---------------------------------------------------------------------------
// WAKELOCKS
// ---------------------------------------------------------------------------

pub fn get_wakelocks(device_id: &str) -> Vec<WakelockInfo> {
    let uid_to_pkg = uid_package_map(device_id);

    let output = match adb_device(device_id, &["shell", "dumpsys", "batterystats"]) {
        Some(o) => o,
        None => return Vec::new(),
    };

    let mut totals: HashMap<String, u64> = HashMap::new();
    let mut in_section = false;

    for line in output.lines() {
        let trimmed = line.trim();

        // batterystats mentions "Wake lock ..." in a dozen places — per-app breakdowns,
        // daily histories. Only the two summary sections hold device-wide totals.
        if trimmed == "All kernel wake locks:" || trimmed == "All partial wake locks:" {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }

        let (owner, rest) = if let Some(rest) = trimmed.strip_prefix("Kernel Wake lock ") {
            (None, rest)
        } else if let Some(rest) = trimmed.strip_prefix("Wake lock ") {
            let mut parts = rest.splitn(2, ' ');
            let uid_token = parts.next().unwrap_or("");
            match parts.next() {
                Some(tail) => (Some(uid_token), tail),
                None => continue,
            }
        } else {
            // Anything else — a blank line or the next section header — ends the section.
            in_section = false;
            continue;
        };

        let Some((_name, duration)) = split_wakelock_entry(rest) else {
            continue;
        };
        let Some(duration_ms) = parse_duration_from_line(duration) else {
            continue;
        };

        let label = match owner {
            Some(token) => match parse_wakelock_uid(token) {
                Some(uid) => uid_to_pkg
                    .get(&uid)
                    .cloned()
                    .unwrap_or_else(|| format!("uid {}", token)),
                None => format!("uid {}", token),
            },
            None => "[kernel]".to_string(),
        };

        *totals.entry(label).or_insert(0) += duration_ms;
    }

    let mut wakelocks: Vec<WakelockInfo> = totals
        .into_iter()
        .map(|(package, duration_ms)| WakelockInfo {
            package,
            duration_ms,
            duration_human: format_duration_ms(duration_ms),
        })
        .collect();

    wakelocks.sort_by_key(|w| std::cmp::Reverse(w.duration_ms));
    wakelocks
}

/// Split `<name>: <duration> (N times) max=... realtime` into name and duration.
///
/// The name itself may contain colons (`Doze:KeyguardIndication`) and the trailing
/// `max=`/`actual=` counters are microsecond figures that must not be read as part of
/// the duration, so the split anchors on the `(N times)` marker that always follows it.
fn split_wakelock_entry(entry: &str) -> Option<(&str, &str)> {
    let cut = find_times_marker(entry).unwrap_or(entry.len());
    let head = &entry[..cut];
    let colon = head.rfind(':')?;
    Some((head[..colon].trim(), head[colon + 1..].trim()))
}

fn find_times_marker(s: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(offset) = s[from..].find(" (") {
        let at = from + offset;
        let after = &s[at + 2..];
        let digits = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        if digits > 0 && after[digits..].starts_with(" times)") {
            return Some(at);
        }
        from = at + 2;
    }
    None
}

/// `u0a135` is app UID 10135; system wakelocks use the bare UID (`1002`).
fn parse_wakelock_uid(token: &str) -> Option<u32> {
    match token.strip_prefix("u0a") {
        Some(n) => n.parse::<u32>().ok().map(|v| v + 10000),
        None => token.parse().ok(),
    }
}

fn parse_duration_from_line(line: &str) -> Option<u64> {
    // Durations look like "1h 42m 4s 84ms" — any subset, in that order.
    let mut total_ms: u64 = 0;
    let mut found_any = false;

    let line = line.replace('+', "");
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(num) = line[start..i].parse::<u64>() {
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

    if found_any {
        Some(total_ms)
    } else {
        None
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pid_and_package_from_a_process_record() {
        let line = "*APP* UID 10142 ProcessRecord{5baaf1 10517:com.google.android.youtube/u0a142}";
        let proc = parse_process_record(line).expect("record parses");
        assert_eq!(proc.package, "com.google.android.youtube");
        assert_eq!(proc.pid, 10517);
    }

    #[test]
    fn sizes_carry_their_unit() {
        assert_eq!(parse_size_kb("158MB"), Some(161_792));
        assert_eq!(parse_size_kb("1,6MB"), Some(1_638));
        assert_eq!(parse_size_kb("0,00"), Some(0));
        assert_eq!(parse_size_kb("4096"), Some(4_096));
    }

    #[test]
    fn tagged_and_rollup_buckets_are_skipped() {
        let wifi = "ident=[{type=1, ratType=COMBINED, metered=true}] uid=10142 set=DEFAULT tag=0x0";
        assert_eq!(parse_bucket_header(wifi), Some((10142, true)));

        let mobile = "ident=[{type=0, ratType=13, subId=3}] uid=10142 set=DEFAULT tag=0x0";
        assert_eq!(parse_bucket_header(mobile), Some((10142, false)));

        let tagged = "ident=[{type=1, ratType=COMBINED}] uid=10142 set=DEFAULT tag=0xffffff82";
        assert_eq!(parse_bucket_header(tagged), None);

        let rollup = "ident=[{type=0, ratType=13}] uid=-1 set=ALL tag=0x0";
        assert_eq!(parse_bucket_header(rollup), None);
    }

    #[test]
    fn wakelock_duration_ignores_the_trailing_counters() {
        let entry = "AudioMix: 1h 42m 4s 84ms (2 times) max=2039210 actual=9952836 realtime";
        let (name, duration) = split_wakelock_entry(entry).expect("entry parses");
        assert_eq!(name, "AudioMix");
        assert_eq!(parse_duration_from_line(duration), Some(6_124_084));
    }

    #[test]
    fn wakelock_names_may_contain_colons() {
        let entry = "Doze:KeyguardIndication: 21s 730ms (92 times) max=918 actual=61759 realtime";
        let (name, duration) = split_wakelock_entry(entry).expect("entry parses");
        assert_eq!(name, "Doze:KeyguardIndication");
        assert_eq!(parse_duration_from_line(duration), Some(21_730));
    }

    #[test]
    fn app_uids_resolve_from_the_u0a_form() {
        assert_eq!(parse_wakelock_uid("u0a135"), Some(10135));
        assert_eq!(parse_wakelock_uid("1002"), Some(1002));
        assert_eq!(parse_wakelock_uid("kernel"), None);
    }
}
