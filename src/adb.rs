use std::path::Path;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

use crate::types::{Device, DeviceType, TransferState};

pub fn adb(args: &[&str]) -> Option<String> {
    Command::new("adb")
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

pub fn adb_device(id: &str, args: &[&str]) -> Option<String> {
    let mut full_args = vec!["-s", id];
    full_args.extend(args);
    adb(&full_args)
}

pub fn adb_fire(id: &str, args: &[&str]) {
    let mut full_args = vec!["-s", id];
    full_args.extend(args);
    let _ = Command::new("adb").args(&full_args).spawn();
}

pub fn get_all_devices() -> Vec<Device> {
    let mut devices = Vec::new();

    if let Some(output) = adb(&["devices", "-l"]) {
        for line in output.lines().skip(1) {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let id = parts[0].to_string();
                let status = parts[1].to_string();

                let name = if status == "device" {
                    adb_device(&id, &["shell", "getprop", "ro.product.model"])
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| id.clone())
                } else {
                    parts
                        .iter()
                        .find(|p| p.starts_with("model:"))
                        .map(|p| p.replace("model:", ""))
                        .unwrap_or_else(|| id.clone())
                };

                let device_type = if status == "device" {
                    let features =
                        adb_device(&id, &["shell", "getprop", "ro.build.characteristics"])
                            .unwrap_or_default()
                            .to_lowercase();
                    let product = adb_device(&id, &["shell", "getprop", "ro.product.name"])
                        .unwrap_or_default()
                        .to_lowercase();

                    if features.contains("tv")
                        || product.contains("tv")
                        || name.to_lowercase().contains("tv")
                        || name.to_lowercase().contains("shield")
                        || name.to_lowercase().contains("chromecast")
                        || name.to_lowercase().contains("mibox")
                    {
                        DeviceType::Tv
                    } else {
                        DeviceType::Phone
                    }
                } else {
                    DeviceType::Unknown
                };

                devices.push(Device {
                    id,
                    name,
                    status,
                    device_type,
                });
            }
        }
    }
    devices
}

pub fn set_stay_awake_cmd(id: &str, enabled: bool) {
    let value = if enabled { "true" } else { "false" };
    adb_fire(id, &["shell", "svc", "power", "stayon", value]);
}

pub fn press_key(id: &str, key: &str) {
    adb_fire(id, &["shell", "input", "keyevent", key]);
}

pub fn open_camera(id: &str) {
    adb_fire(
        id,
        &["shell", "am", "start", "-a", "android.media.action.IMAGE_CAPTURE"],
    );
}

pub fn open_video(id: &str) {
    adb_fire(
        id,
        &["shell", "am", "start", "-a", "android.media.action.VIDEO_CAPTURE"],
    );
}

pub fn open_mic(id: &str) {
    adb_fire(
        id,
        &[
            "shell",
            "am",
            "start",
            "-a",
            "android.provider.MediaStore.RECORD_SOUND",
        ],
    );
}

pub fn play_video_url(id: &str, url: &str) {
    adb_fire(
        id,
        &[
            "shell",
            "am",
            "start",
            "-a",
            "android.intent.action.VIEW",
            "-d",
            url,
            "-t",
            "video/*",
        ],
    );
}

pub fn start_transfer(
    id: &str,
    local_path: &str,
    state: Arc<Mutex<TransferState>>,
    play_after: bool,
) {
    let path = Path::new(local_path);
    let filename = match path.file_name() {
        Some(n) => n.to_string_lossy().to_string(),
        None => return,
    };

    let total_bytes = std::fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);
    let remote = format!("/sdcard/Movies/{}", filename);
    let id = id.to_string();
    let local = local_path.to_string();

    if let Ok(mut t) = state.lock() {
        t.active = true;
        t.filename = filename;
        t.total_bytes = total_bytes;
        t.transferred_bytes = 0;
        t.done = false;
        t.play_after = play_after;
    }

    let monitor_state = Arc::clone(&state);
    let monitor_id = id.clone();
    let monitor_remote = remote.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));

        if let Some(output) =
            adb_device(&monitor_id, &["shell", "stat", "-c", "%s", &monitor_remote])
        {
            if let Ok(size) = output.trim().parse::<u64>() {
                if let Ok(mut t) = monitor_state.lock() {
                    t.transferred_bytes = size;
                    if t.done || !t.active {
                        break;
                    }
                }
            }
        }

        if let Ok(t) = monitor_state.lock() {
            if t.done || !t.active {
                break;
            }
        }
    });

    std::thread::spawn(move || {
        let output = Command::new("adb")
            .args(["-s", &id, "push", &local, &remote])
            .output();

        let success = output.map(|o| o.status.success()).unwrap_or(false);

        if let Ok(mut t) = state.lock() {
            t.transferred_bytes = t.total_bytes;
            t.done = true;

            if success && t.play_after {
                let _ = Command::new("adb")
                    .args([
                        "-s",
                        &id,
                        "shell",
                        "am",
                        "start",
                        "-a",
                        "android.intent.action.VIEW",
                        "-d",
                        &format!("file://{}", remote),
                        "-t",
                        "video/*",
                    ])
                    .spawn();
            }
        }
    });
}

pub fn connect_adb_wifi(addr: &str) -> bool {
    Command::new("adb")
        .args(["connect", addr])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout).to_lowercase();
            out.contains("connected") && !out.contains("cannot") && !out.contains("failed")
        })
        .unwrap_or(false)
}

pub fn get_local_ip_prefix() -> Option<String> {
    if let Ok(output) = Command::new("hostname").arg("-I").output() {
        let ips = String::from_utf8_lossy(&output.stdout);
        for ip in ips.split_whitespace() {
            if ip.starts_with("192.168.") || ip.starts_with("10.") || ip.starts_with("172.") {
                let parts: Vec<&str> = ip.split('.').collect();
                if parts.len() >= 3 {
                    return Some(format!("{}.{}.{}.", parts[0], parts[1], parts[2]));
                }
            }
        }
    }
    None
}

pub fn scan_network_for_adb() -> Vec<String> {
    let mut found = Vec::new();

    if let Some(prefix) = get_local_ip_prefix() {
        let script = format!(
            r#"for i in $(seq 1 254); do
                (timeout 1 bash -c "echo >/dev/tcp/{prefix}$i/5555" 2>/dev/null && echo "{prefix}$i") &
            done; wait"#,
            prefix = prefix
        );

        if let Ok(output) = Command::new("bash").args(["-c", &script]).output() {
            let result = String::from_utf8_lossy(&output.stdout);
            for line in result.lines() {
                let ip = line.trim();
                if !ip.is_empty() && ip.starts_with(&prefix[..prefix.len() - 1]) {
                    found.push(ip.to_string());
                }
            }
        }
    }

    found
}

pub fn kill_child_tree(child: &mut Child) {
    let pid = child.id();
    let _ = Command::new("pkill")
        .args(["-P", &pid.to_string()])
        .output();
    let _ = child.kill();
    let _ = child.wait();
}

pub fn start_webcam_process(
    id: &str,
    front: bool,
    with_mic: bool,
    audio_output: bool,
) -> Option<Child> {
    let facing_arg = format!("--camera-facing={}", if front { "front" } else { "back" });
    let mut args = vec![
        "run".to_string(),
        "--command=scrcpy".to_string(),
        "io.github.IshuSinghSE.aurynk".to_string(),
        "-s".to_string(),
        id.to_string(),
        "--video-source=camera".to_string(),
        facing_arg,
        "--camera-size=1280x720".to_string(),
        "--v4l2-sink=/dev/video10".to_string(),
    ];

    if with_mic && audio_output {
        args.push("--audio-source=mic".to_string());
    } else if with_mic {
        args.push("--audio-source=mic".to_string());
    } else if audio_output {
        args.push("--audio-source=playback".to_string());
        args.push("--audio-dup".to_string());
    } else {
        args.push("--no-audio".to_string());
    }
    Command::new("flatpak").args(&args).spawn().ok()
}

pub fn start_mirror_process(id: &str, stay_awake: bool) -> Option<Child> {
    let mut args = vec![
        "run".to_string(),
        "--command=scrcpy".to_string(),
        "io.github.IshuSinghSE.aurynk".to_string(),
        "-s".to_string(),
        id.to_string(),
        "--no-audio".to_string(),
        "--turn-screen-off".to_string(),
    ];
    if stay_awake {
        args.push("--stay-awake".to_string());
    }
    Command::new("flatpak").args(&args).spawn().ok()
}

pub fn send_text_to_device(id: &str, text: &str) {
    let escaped = text.replace(' ', "%s").replace('&', "\\&").replace('<', "\\<").replace('>', "\\>").replace('\'', "\\'").replace('"', "\\\"");
    adb_fire(id, &["shell", "input", "text", &escaped]);
}

pub fn get_battery_info(id: &str) -> Option<(u8, String)> {
    let output = adb_device(id, &["shell", "dumpsys", "battery"])?;
    let mut level: Option<u8> = None;
    let mut status_str = String::from("unknown");

    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("level:") {
            level = line.split(':').nth(1).and_then(|s| s.trim().parse().ok());
        } else if line.starts_with("status:") {
            let code: u8 = line.split(':').nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            status_str = match code {
                2 => "En charge",
                3 => "Décharge",
                4 => "Pas en charge",
                5 => "Plein",
                _ => "Inconnu",
            }.to_string();
        }
    }

    level.map(|l| (l, status_str))
}

pub fn get_third_party_apps(id: &str) -> Vec<String> {
    adb_device(id, &["shell", "pm", "list", "packages", "-3"])
        .map(|output| {
            output
                .lines()
                .filter_map(|line| line.strip_prefix("package:").map(|s| s.trim().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn uninstall_app(id: &str, package: &str) -> bool {
    Command::new("adb")
        .args(["-s", id, "uninstall", package])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Success"))
        .unwrap_or(false)
}

pub fn ring_phone(id: &str) {
    // Max volume
    adb_fire(id, &["shell", "media", "volume", "--set", "15", "--stream", "2"]);
    // Play alarm sound
    adb_fire(id, &["shell", "am", "start", "-a", "android.intent.action.CALL", "-d", "tel:0000000000"]);
}

pub fn stop_ring(id: &str) {
    adb_fire(id, &["shell", "input", "keyevent", "KEYCODE_ENDCALL"]);
}

pub fn take_screenshot(id: &str) -> Option<Vec<u8>> {
    let remote_path = "/sdcard/screenshot_tmp.png";
    // Take screenshot on device
    let _ = Command::new("adb")
        .args(["-s", id, "shell", "screencap", "-p", remote_path])
        .output();

    // Pull to temp file
    let local_tmp = std::env::temp_dir().join("phone_tv_screenshot.png");
    let local_str = local_tmp.to_string_lossy().to_string();
    let pull_ok = Command::new("adb")
        .args(["-s", id, "pull", remote_path, &local_str])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Clean remote
    let _ = Command::new("adb")
        .args(["-s", id, "shell", "rm", remote_path])
        .spawn();

    if pull_ok {
        std::fs::read(&local_tmp).ok()
    } else {
        None
    }
}
