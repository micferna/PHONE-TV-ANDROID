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
        &[
            "shell",
            "am",
            "start",
            "-a",
            "android.media.action.IMAGE_CAPTURE",
        ],
    );
}

pub fn open_video(id: &str) {
    adb_fire(
        id,
        &[
            "shell",
            "am",
            "start",
            "-a",
            "android.media.action.VIDEO_CAPTURE",
        ],
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

/// Pair with a device using Android 11+ wireless pairing.
/// `addr` is the pairing address (IP:port) shown on the phone, `code` is the 6-digit code.
/// Returns (success, message).
pub fn pair_adb_wifi(addr: &str, code: &str) -> (bool, String) {
    let output = Command::new("adb").args(["pair", addr, code]).output();
    match output {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            let success = combined.to_lowercase().contains("successfully paired");
            (success, combined.trim().to_string())
        }
        Err(e) => (false, format!("Erreur exécution adb: {}", e)),
    }
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

/// Discover the local /24 prefix by asking the OS routing table which interface
/// it would use to reach a public IP. Pure-Rust, works on Linux/macOS/Windows.
pub fn get_local_ip_prefix() -> Option<String> {
    use std::net::UdpSocket;
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    // No packet is actually sent: connect on UDP just sets the default route.
    sock.connect("8.8.8.8:80").ok()?;
    let local = sock.local_addr().ok()?.ip();
    let ip_str = local.to_string();
    let parts: Vec<&str> = ip_str.split('.').collect();
    if parts.len() == 4 {
        Some(format!("{}.{}.{}.", parts[0], parts[1], parts[2]))
    } else {
        None
    }
}

/// Scan the local /24 for hosts listening on TCP/5555 (ADB wireless port).
/// Pure-Rust parallel scan — works on Linux, macOS and Windows.
pub fn scan_network_for_adb() -> Vec<String> {
    use std::net::{SocketAddr, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    let prefix = match get_local_ip_prefix() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let found: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::with_capacity(254);

    for i in 1..=254u8 {
        let ip = format!("{}{}", prefix, i);
        let found = Arc::clone(&found);
        handles.push(std::thread::spawn(move || {
            if let Ok(addr) = format!("{}:5555", ip).parse::<SocketAddr>() {
                if TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok() {
                    if let Ok(mut v) = found.lock() {
                        v.push(ip);
                    }
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    let mut result = Arc::try_unwrap(found)
        .ok()
        .and_then(|m| m.into_inner().ok())
        .unwrap_or_default();
    result.sort();
    result
}

pub fn kill_child_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id();
        let _ = Command::new("pkill")
            .args(["-P", &pid.to_string()])
            .output();
    }
    #[cfg(windows)]
    {
        let pid = child.id();
        // /T = also terminate child processes, /F = force
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Build a scrcpy invocation that works on Linux (via flatpak aurynk) or other
/// platforms (direct `scrcpy` / `scrcpy.exe` binary on PATH).
fn scrcpy_command() -> Command {
    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("flatpak");
        cmd.args(["run", "--command=scrcpy", "io.github.IshuSinghSE.aurynk"]);
        cmd
    }
    #[cfg(not(target_os = "linux"))]
    {
        Command::new("scrcpy")
    }
}

pub fn start_webcam_process(
    id: &str,
    front: bool,
    with_mic: bool,
    audio_output: bool,
) -> Option<Child> {
    let facing_arg = format!("--camera-facing={}", if front { "front" } else { "back" });
    let mut args = vec![
        "-s".to_string(),
        id.to_string(),
        "--video-source=camera".to_string(),
        facing_arg,
        "--camera-size=1280x720".to_string(),
    ];

    // Linux: pipe directly into a v4l2loopback virtual device.
    // Windows/macOS: just show a scrcpy window; the user routes it to a virtual
    // camera via OBS Virtual Camera (or equivalent).
    #[cfg(target_os = "linux")]
    args.push("--v4l2-sink=/dev/video10".to_string());

    if with_mic {
        args.push("--audio-source=mic".to_string());
    } else if audio_output {
        args.push("--audio-source=playback".to_string());
        args.push("--audio-dup".to_string());
    } else {
        args.push("--no-audio".to_string());
    }

    scrcpy_command().args(&args).spawn().ok()
}

pub fn start_mirror_process(id: &str, stay_awake: bool) -> Option<Child> {
    let mut args = vec![
        "-s".to_string(),
        id.to_string(),
        "--no-audio".to_string(),
        "--turn-screen-off".to_string(),
    ];
    if stay_awake {
        args.push("--stay-awake".to_string());
    }
    scrcpy_command().args(&args).spawn().ok()
}

/// Returns true on platforms that can route the phone camera straight to a
/// system-visible virtual webcam without extra software (Linux + v4l2loopback).
/// On Windows/macOS the user needs OBS Virtual Camera or similar.
pub const fn webcam_direct_supported() -> bool {
    cfg!(target_os = "linux")
}

pub fn send_text_to_device(id: &str, text: &str) {
    let escaped = text
        .replace(' ', "%s")
        .replace('&', "\\&")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('\'', "\\'")
        .replace('"', "\\\"");
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
            let code: u8 = line
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            status_str = match code {
                2 => "En charge",
                3 => "Décharge",
                4 => "Pas en charge",
                5 => "Plein",
                _ => "Inconnu",
            }
            .to_string();
        }
    }

    level.map(|l| (l, status_str))
}

pub fn ring_phone(id: &str) {
    // Max volume
    adb_fire(
        id,
        &["shell", "media", "volume", "--set", "15", "--stream", "2"],
    );
    // Play alarm sound
    adb_fire(
        id,
        &[
            "shell",
            "am",
            "start",
            "-a",
            "android.intent.action.CALL",
            "-d",
            "tel:0000000000",
        ],
    );
}

pub fn stop_ring(id: &str) {
    adb_fire(id, &["shell", "input", "keyevent", "KEYCODE_ENDCALL"]);
}

/// Push a local file to a remote path on the device.
pub fn push_file(id: &str, local: &str, remote: &str) -> (bool, String) {
    let output = Command::new("adb")
        .args(["-s", id, "push", local, remote])
        .output();
    match output {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            (o.status.success(), combined.trim().to_string())
        }
        Err(e) => (false, format!("Erreur adb: {}", e)),
    }
}

/// Pull a remote file from the device to a local path.
pub fn pull_file(id: &str, remote: &str, local: &str) -> (bool, String) {
    let output = Command::new("adb")
        .args(["-s", id, "pull", remote, local])
        .output();
    match output {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            (o.status.success(), combined.trim().to_string())
        }
        Err(e) => (false, format!("Erreur adb: {}", e)),
    }
}

/// Install an APK on the device. Uses `-r` to reinstall keeping data, `-g` to grant runtime perms.
/// Returns (success, stdout+stderr).
pub fn install_apk(id: &str, apk_path: &str) -> (bool, String) {
    let output = Command::new("adb")
        .args(["-s", id, "install", "-r", "-g", apk_path])
        .output();

    match output {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            // adb install prints "Success" on stdout when ok
            (
                o.status.success() && combined.contains("Success"),
                combined.trim().to_string(),
            )
        }
        Err(e) => (false, format!("Erreur exécution adb: {}", e)),
    }
}

/// Start a background `adb shell screenrecord` writing to a remote path.
/// The returned Child must be killed to stop recording. Returns (child, remote_path).
pub fn start_screenrecord(id: &str) -> Option<(Child, String)> {
    let remote = "/sdcard/phone_tv_recording.mp4".to_string();
    let child = Command::new("adb")
        .args(["-s", id, "shell", "screenrecord", &remote])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    Some((child, remote))
}

/// Stop a running screenrecord child and pull the remote .mp4 to `local_dest`.
/// Returns true on success.
pub fn stop_screenrecord_and_pull(
    id: &str,
    child: &mut Child,
    remote: &str,
    local_dest: &Path,
) -> bool {
    // Killing the adb client makes the on-device screenrecord stop, but with a delay.
    kill_child_tree(child);
    // Give the device a moment to finalize the mp4 header.
    std::thread::sleep(std::time::Duration::from_millis(800));

    let local_str = local_dest.to_string_lossy().to_string();
    let pulled = Command::new("adb")
        .args(["-s", id, "pull", remote, &local_str])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Best-effort cleanup remote file
    let _ = Command::new("adb")
        .args(["-s", id, "shell", "rm", remote])
        .spawn();

    pulled && local_dest.exists()
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
