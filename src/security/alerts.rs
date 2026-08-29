//! Alerts raised while the monitoring view watches a device.
//!
//! Monitoring polls three dumps (processes, per-app network usage, wakelocks) and
//! shows the *current* state. What a human actually wants to be told about is the
//! *change* between two polls: an app that just started, one that suddenly pushed
//! megabytes over WiFi or mobile data, a wakelock that keeps the phone awake. These
//! helpers diff two snapshots and turn the differences into alerts, optionally
//! announced with a sound and a desktop notification so they land even when the
//! window is in the background.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::types::{AlertLevel, DataUsage, MonitoringAlert, ProcessInfo, WakelockInfo};

/// Per-app traffic on one network, in one polling interval, above which we speak up.
/// Background sync is a few hundred KB; a couple of MB in a handful of seconds is
/// something the user did not necessarily ask for.
const DATA_ALERT_BYTES: u64 = 2 * 1024 * 1024;
/// Wakelock time gained between two polls before it counts as "keeps the phone awake".
const WAKELOCK_ALERT_MS: u64 = 60_000;
/// Alerts kept in the panel; older ones scroll out of history.
pub const MAX_ALERTS: usize = 200;

fn now_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn alert(level: AlertLevel, message: String) -> MonitoringAlert {
    MonitoringAlert {
        time: now_hms(),
        level,
        message,
    }
}

/// Snapshot of the processes seen in a poll: the package names, nothing more.
pub fn processes_snapshot(processes: &[ProcessInfo]) -> HashSet<String> {
    processes.iter().map(|p| p.package.clone()).collect()
}

/// Snapshot of network counters: package → (wifi total, mobile total).
pub fn data_snapshot(usage: &[DataUsage]) -> HashMap<String, (u64, u64)> {
    usage
        .iter()
        .map(|u| {
            (
                u.package.clone(),
                (u.wifi_rx + u.wifi_tx, u.mobile_rx + u.mobile_tx),
            )
        })
        .collect()
}

/// Snapshot of wakelock totals: owner → cumulated duration.
pub fn wakelocks_snapshot(wakelocks: &[WakelockInfo]) -> HashMap<String, u64> {
    wakelocks
        .iter()
        .map(|w| (w.package.clone(), w.duration_ms))
        .collect()
}

/// Processes that were not running at the previous poll.
pub fn diff_processes(
    prev: &HashSet<String>,
    processes: &[ProcessInfo],
    blacklist: &[String],
) -> Vec<MonitoringAlert> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut alerts = Vec::new();
    for proc in processes {
        if prev.contains(&proc.package) || !seen.insert(proc.package.as_str()) {
            continue;
        }
        if blacklist.iter().any(|b| b == &proc.package) {
            alerts.push(alert(
                AlertLevel::Danger,
                format!("\u{1f6ab} App blacklistée démarrée : {}", proc.package),
            ));
        } else {
            alerts.push(alert(
                AlertLevel::Info,
                format!("\u{25b6} Nouveau processus : {}", proc.package),
            ));
        }
    }
    alerts
}

/// Apps that moved a noticeable amount of data since the previous poll, reported per
/// network so WiFi and mobile stay distinguishable.
pub fn diff_data_usage(
    prev: &HashMap<String, (u64, u64)>,
    usage: &[DataUsage],
) -> Vec<MonitoringAlert> {
    let mut alerts = Vec::new();
    for u in usage {
        // An app absent from the previous snapshot has no baseline: its counters are
        // cumulative since the last stats reset, not traffic that just happened.
        let Some(&(prev_wifi, prev_mobile)) = prev.get(&u.package) else {
            continue;
        };
        let wifi = u.wifi_rx + u.wifi_tx;
        let mobile = u.mobile_rx + u.mobile_tx;
        // netstats buckets roll over; a counter going backwards is a reset, not traffic.
        for (label, delta) in [
            ("\u{1f4f6} WiFi", wifi.saturating_sub(prev_wifi)),
            ("\u{1f4f1} Mobile", mobile.saturating_sub(prev_mobile)),
        ] {
            if delta >= DATA_ALERT_BYTES {
                alerts.push(alert(
                    AlertLevel::Warning,
                    format!(
                        "{} : {} a transféré {}",
                        label,
                        u.package,
                        human_bytes(delta)
                    ),
                ));
            }
        }
    }
    alerts
}

/// Wakelocks whose held time grew by more than a minute between two polls.
pub fn diff_wakelocks(
    prev: &HashMap<String, u64>,
    wakelocks: &[WakelockInfo],
) -> Vec<MonitoringAlert> {
    let mut alerts = Vec::new();
    for wl in wakelocks {
        let Some(&before) = prev.get(&wl.package) else {
            continue;
        };
        let delta = wl.duration_ms.saturating_sub(before);
        if delta >= WAKELOCK_ALERT_MS {
            alerts.push(alert(
                AlertLevel::Warning,
                format!(
                    "\u{23f0} Wakelock : {} maintient le téléphone éveillé (+{})",
                    wl.package,
                    human_duration(delta)
                ),
            ));
        }
    }
    alerts
}

fn human_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{} KB", b / KB),
        b => format!("{} B", b),
    }
}

fn human_duration(ms: u64) -> String {
    let secs = ms / 1000;
    if secs >= 3600 {
        format!("{}h{:02}", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

// ---------------------------------------------------------------------------
// ANNOUNCING
// ---------------------------------------------------------------------------

/// Play a sound and/or raise a desktop notification for a fresh batch of alerts.
///
/// One announcement per batch, never one per alert: a poll that turns up eight
/// changes must not fire eight beeps.
pub fn announce(alerts: &[MonitoringAlert], sound: bool, desktop: bool) {
    if alerts.is_empty() {
        return;
    }
    if sound {
        play_sound();
    }
    if desktop {
        let body: Vec<String> = alerts
            .iter()
            .take(3)
            .map(|a| a.message.clone())
            .chain((alerts.len() > 3).then(|| format!("… et {} autre(s)", alerts.len() - 3)))
            .collect();
        let summary = format!("Phone-TV — {} alerte(s)", alerts.len());
        spawn_detached(
            "notify-send",
            &["-a", "Phone-TV", &summary, &body.join("\n")],
        );
    }
}

/// The alert sound to use, resolved once: desktops ship different players and
/// different sound themes, and probing on every alert would be silly.
fn sound_command() -> Option<&'static (String, Vec<String>)> {
    static CMD: OnceLock<Option<(String, Vec<String>)>> = OnceLock::new();
    CMD.get_or_init(|| {
        if in_path("canberra-gtk-play") {
            return Some((
                "canberra-gtk-play".to_string(),
                vec!["-i".into(), "message-new-instant".into()],
            ));
        }
        let files = [
            "/usr/share/sounds/freedesktop/stereo/message.oga",
            "/usr/share/sounds/freedesktop/stereo/bell.oga",
        ];
        // pw-play (PipeWire) and paplay (PulseAudio) both read the theme's ogg files.
        if let Some(f) = files.iter().find(|f| Path::new(f).exists()) {
            for player in ["pw-play", "paplay"] {
                if in_path(player) {
                    return Some((player.to_string(), vec![f.to_string()]));
                }
            }
        }
        let wav = "/usr/share/sounds/alsa/Front_Center.wav";
        if in_path("aplay") && Path::new(wav).exists() {
            return Some(("aplay".to_string(), vec!["-q".into(), wav.to_string()]));
        }
        None
    })
    .as_ref()
}

fn play_sound() {
    match sound_command() {
        Some((prog, args)) => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            spawn_detached(prog, &args);
        }
        // No player installed: the terminal bell is all that is left.
        None => {
            use std::io::Write;
            let mut err = std::io::stderr();
            let _ = err.write_all(b"\x07");
            let _ = err.flush();
        }
    }
}

fn in_path(prog: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(prog).is_file())
}

/// Run a helper we do not care about the output of, reaping it on a side thread so
/// the alert players do not pile up as zombies over a long monitoring session.
fn spawn_detached(prog: &str, args: &[&str]) {
    let child = Command::new(prog)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pkg: &str) -> ProcessInfo {
        ProcessInfo {
            package: pkg.to_string(),
            pid: 1,
            memory_kb: 0,
            adj: 0,
            state: "service".into(),
        }
    }

    #[test]
    fn only_new_processes_alert() {
        let prev: HashSet<String> = ["com.a".to_string()].into_iter().collect();
        let alerts = diff_processes(&prev, &[proc("com.a"), proc("com.b")], &[]);
        assert_eq!(alerts.len(), 1);
        assert!(alerts[0].message.contains("com.b"));
    }

    #[test]
    fn blacklisted_process_is_danger() {
        let alerts = diff_processes(&HashSet::new(), &[proc("com.bad")], &["com.bad".into()]);
        assert_eq!(alerts[0].level, AlertLevel::Danger);
    }

    #[test]
    fn data_alerts_split_wifi_and_mobile() {
        let usage = vec![DataUsage {
            package: "com.a".into(),
            uid: 10,
            wifi_rx: 5 * 1024 * 1024,
            wifi_tx: 0,
            mobile_rx: 3 * 1024 * 1024,
            mobile_tx: 0,
        }];
        let prev = data_snapshot(&[DataUsage {
            package: "com.a".into(),
            uid: 10,
            wifi_rx: 0,
            wifi_tx: 0,
            mobile_rx: 0,
            mobile_tx: 0,
        }]);
        let alerts = diff_data_usage(&prev, &usage);
        assert_eq!(alerts.len(), 2);
        assert!(alerts[0].message.contains("WiFi"));
        assert!(alerts[1].message.contains("Mobile"));
    }

    #[test]
    fn first_snapshot_never_alerts() {
        let usage = vec![DataUsage {
            package: "com.a".into(),
            uid: 10,
            wifi_rx: 900 * 1024 * 1024,
            wifi_tx: 0,
            mobile_rx: 0,
            mobile_tx: 0,
        }];
        assert!(diff_data_usage(&HashMap::new(), &usage).is_empty());
    }

    #[test]
    fn counter_reset_is_not_traffic() {
        let prev = data_snapshot(&[DataUsage {
            package: "com.a".into(),
            uid: 10,
            wifi_rx: 500 * 1024 * 1024,
            wifi_tx: 0,
            mobile_rx: 0,
            mobile_tx: 0,
        }]);
        let usage = vec![DataUsage {
            package: "com.a".into(),
            uid: 10,
            wifi_rx: 1024,
            wifi_tx: 0,
            mobile_rx: 0,
            mobile_tx: 0,
        }];
        assert!(diff_data_usage(&prev, &usage).is_empty());
    }

    #[test]
    fn wakelock_growth_alerts_once_past_a_minute() {
        let prev: HashMap<String, u64> = [("com.a".to_string(), 10_000)].into_iter().collect();
        let wl = |ms| WakelockInfo {
            package: "com.a".into(),
            duration_ms: ms,
            duration_human: String::new(),
        };
        assert!(diff_wakelocks(&prev, &[wl(40_000)]).is_empty());
        assert_eq!(diff_wakelocks(&prev, &[wl(120_000)]).len(), 1);
    }
}
