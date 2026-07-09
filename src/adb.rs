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

/// Move a USB-connected device onto wireless ADB (TCP/IP, port 5555) and connect to
/// it over WiFi, so a stream bound to the returned id keeps running after the USB
/// cable is unplugged.
///
/// Returns the wireless device id (`ip:5555`) on success. If `id` is already a
/// network transport (contains ':') it is returned unchanged. Returns `None` when the
/// phone has no reachable WiFi IP or the wireless connection can't be established —
/// callers should then fall back to the original (USB) transport.
pub fn enable_wifi_adb(id: &str) -> Option<String> {
    use std::time::Duration;

    // Already a network transport (ip:port): nothing to switch.
    if id.contains(':') {
        return Some(id.to_string());
    }

    // Read the phone's WiFi IP while the USB transport is still up.
    let ip = get_device_wifi_ip(id)?;
    let addr = format!("{}:5555", ip);

    // Fast path: adbd may already be in TCP/IP mode (e.g. from a previous run).
    // A short, bounded TCP probe — `adb connect` itself can hang forever on an
    // unreachable host, so we never call it without first proving the port is open.
    if port_reachable(&addr, Duration::from_millis(600)) {
        return connect_adb_wifi(&addr).then(|| addr.clone());
    }

    // Don't disturb the working USB transport if the phone's WiFi IP can't possibly
    // be on our LAN (different subnet, or blocked — e.g. a VPN with no LAN sharing).
    if !same_subnet_as_host(&ip) {
        return None;
    }

    // Open port 5555 on the device, then wait (bounded) for it to come up. USB keeps
    // working throughout; this just additionally exposes adbd over TCP.
    Command::new("adb")
        .args(["-s", id, "tcpip", "5555"])
        .output()
        .ok()?;
    if !wait_port_reachable(&addr, Duration::from_secs(3)) {
        return None; // caller falls back to the USB transport
    }
    connect_adb_wifi(&addr).then(|| addr.clone())
}

/// True if a single TCP connect to `addr` succeeds within `timeout`. Never blocks
/// longer than `timeout` — unlike `adb connect`, which hangs on an unreachable host.
fn port_reachable(addr: &str, timeout: std::time::Duration) -> bool {
    use std::net::{SocketAddr, TcpStream};
    addr.parse::<SocketAddr>()
        .map(|sock| TcpStream::connect_timeout(&sock, timeout).is_ok())
        .unwrap_or(false)
}

/// Poll `addr` until it accepts a connection or `budget` elapses.
fn wait_port_reachable(addr: &str, budget: std::time::Duration) -> bool {
    use std::time::{Duration, Instant};
    let start = Instant::now();
    loop {
        if port_reachable(addr, Duration::from_millis(500)) {
            return true;
        }
        if start.elapsed() >= budget {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// True when `ip` shares the host's primary LAN /24 prefix. Used to avoid flipping
/// the device to TCP/IP mode when wireless ADB obviously can't reach it.
fn same_subnet_as_host(ip: &str) -> bool {
    get_local_ip_prefix()
        .map(|prefix| ip.starts_with(&prefix))
        .unwrap_or(false)
}

/// Find the device's WiFi LAN IPv4 address via `adb shell ip`. Tries `wlan0` first,
/// then falls back to scanning every interface for a private-range address.
fn get_device_wifi_ip(id: &str) -> Option<String> {
    let probes: [&[&str]; 2] = [
        &["shell", "ip", "-f", "inet", "addr", "show", "wlan0"],
        &["shell", "ip", "-f", "inet", "addr"],
    ];
    for args in probes {
        if let Some(out) = adb_device(id, args) {
            if let Some(ip) = parse_device_lan_ip(&out) {
                return Some(ip);
            }
        }
    }
    None
}

/// Pull the first private-LAN IPv4 out of `ip addr` output. Split out for unit testing.
fn parse_device_lan_ip(out: &str) -> Option<String> {
    out.lines()
        .filter_map(|line| line.trim().strip_prefix("inet "))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|cidr| cidr.split('/').next())
        .find(|ip| is_private_lan_ip(ip))
        .map(|s| s.to_string())
}

/// True for RFC-1918 private IPv4 ranges (10/8, 172.16/12, 192.168/16).
fn is_private_lan_ip(ip: &str) -> bool {
    let octets: Vec<&str> = ip.split('.').collect();
    if octets.len() != 4 {
        return false;
    }
    let parsed: Option<Vec<u8>> = octets.iter().map(|p| p.parse::<u8>().ok()).collect();
    match parsed.as_deref() {
        Some([10, ..]) => true,
        Some([172, b, ..]) if (16..=31).contains(b) => true,
        Some([192, 168, ..]) => true,
        _ => false,
    }
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

/// Phones with a single system-wide camera slot (the Unisoc-based moto g14, among
/// others) let a vendor HAL evict any other camera client. Face unlock does exactly
/// that on every lock-screen wake, then holds the sensor for a few seconds. A scrcpy
/// started inside that window dies immediately with "the system-wide limit for number
/// of open cameras has been reached", so give each attempt time to fail and retry
/// until the sensor comes back. Bounded, so a phone that never yields still gives up.
const WEBCAM_ATTEMPTS: u32 = 5;
/// How long a freshly spawned scrcpy must stay alive before we call it started.
/// A camera-busy scrcpy exits well inside this window.
const WEBCAM_SETTLE: std::time::Duration = std::time::Duration::from_secs(3);
const WEBCAM_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

/// Blocks for up to ~30s while retrying; call it off the UI thread.
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

    for attempt in 1..=WEBCAM_ATTEMPTS {
        // A failure to spawn at all means no scrcpy binary: retrying won't help.
        let mut child = scrcpy_command().args(&args).spawn().ok()?;
        std::thread::sleep(WEBCAM_SETTLE);
        match child.try_wait() {
            Ok(None) => {
                #[cfg(target_os = "linux")]
                ensure_pipewire_camera_node();
                return Some(child);
            }
            Ok(Some(_)) => {} // already reaped
            Err(_) => kill_child_tree(&mut child),
        }
        if attempt < WEBCAM_ATTEMPTS {
            std::thread::sleep(WEBCAM_RETRY_DELAY);
        }
    }
    None
}

/// With v4l2loopback `exclusive_caps=1`, /dev/video10 only advertises capture
/// capabilities while scrcpy is writing to it. If WirePlumber probed the device
/// before the stream existed (e.g. right after boot), it never creates the
/// PipeWire source node, and browsers that enumerate cameras through PipeWire
/// (Firefox) don't see the camera even though direct-V4L2 apps (Discord) do.
/// Once the stream is up, restart WirePlumber so it re-probes the device.
#[cfg(target_os = "linux")]
fn ensure_pipewire_camera_node() {
    std::thread::spawn(|| {
        for i in 0..10 {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if pipewire_has_video10_source() {
                return;
            }
            // Two attempts: scrcpy may not have opened the sink yet at the
            // first check, so a restart then would re-probe a still-idle device.
            if i == 1 || i == 4 {
                let _ = Command::new("systemctl")
                    .args(["--user", "restart", "wireplumber"])
                    .output();
            }
        }
    });
}

#[cfg(target_os = "linux")]
fn pipewire_has_video10_source() -> bool {
    let Ok(out) = Command::new("pw-dump").output() else {
        return true; // pas de PipeWire → rien à réparer
    };
    let Ok(objects) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return true;
    };
    objects.as_array().is_some_and(|objs| {
        objs.iter().any(|o| {
            let props = &o["info"]["props"];
            props["media.class"] == "Video/Source"
                && props["object.path"]
                    .as_str()
                    .is_some_and(|p| p.contains("/dev/video10"))
        })
    })
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

/// Skip `n` whitespace-separated fields and return the remainder, preserving the
/// original spacing of what's left (so filenames with spaces survive intact).
fn rest_after_fields(line: &str, n: usize) -> Option<&str> {
    let mut rest = line.trim_start();
    for _ in 0..n {
        let ws = rest.find(char::is_whitespace)?;
        rest = rest[ws..].trim_start();
    }
    if rest.is_empty() {
        None
    } else {
        Some(rest)
    }
}

/// A `ls -l` mode column looks like `drwxrwx---` (10 chars, known leading type char).
fn looks_like_mode(tok: &str) -> bool {
    tok.len() >= 10
        && matches!(
            tok.as_bytes()[0],
            b'd' | b'-' | b'l' | b'c' | b'b' | b's' | b'p'
        )
}

/// List a remote directory on the device. Returns (name, is_dir, size_bytes) entries,
/// directories first then files, both alphabetically.
///
/// Parses `ls -lp`: `-l` gives the size column, `-p` appends `/` to directories.
/// Falls back to a name-only reading per line if a row doesn't match the long format,
/// so it degrades gracefully across toybox/busybox variants.
pub fn list_remote_dir(id: &str, remote: &str) -> Vec<(String, bool, u64)> {
    let out = adb_device(id, &["shell", "ls", "-lp", remote]).unwrap_or_default();
    parse_ls_output(&out)
}

/// Pure parser for `ls -lp` output, split out so it can be unit-tested without adb.
fn parse_ls_output(out: &str) -> Vec<(String, bool, u64)> {
    let mut entries: Vec<(String, bool, u64)> = Vec::new();

    for line in out.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with("total ") {
            continue;
        }
        if line.contains("No such file") || line.contains("Permission denied") {
            continue;
        }

        let toks: Vec<&str> = line.split_whitespace().collect();
        // Long format: mode links owner group size date time name...
        if toks.len() >= 8 && looks_like_mode(toks[0]) {
            let is_dir = toks[0].starts_with('d');
            let size = toks[4].parse::<u64>().unwrap_or(0);
            if let Some(rest) = rest_after_fields(line, 7) {
                // Drop the "-> target" part of symlinks.
                let name = rest
                    .split(" -> ")
                    .next()
                    .unwrap_or(rest)
                    .trim_end()
                    .trim_end_matches('/');
                if !name.is_empty() && name != "." && name != ".." {
                    entries.push((name.to_string(), is_dir, if is_dir { 0 } else { size }));
                }
            }
        } else {
            // Fallback: a bare name, dir marked by trailing '/'.
            let trimmed = line.trim();
            let is_dir = trimmed.ends_with('/');
            let name = trimmed.trim_end_matches('/');
            if !name.is_empty() && name != "." && name != ".." {
                entries.push((name.to_string(), is_dir, 0));
            }
        }
    }

    entries.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    entries
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

#[cfg(test)]
mod tests {
    use super::{is_private_lan_ip, parse_device_lan_ip, parse_ls_output};

    #[test]
    fn extracts_wlan_ip_from_ip_addr_output() {
        // `adb shell ip -f inet addr show wlan0` on a typical phone.
        let out = "34: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500\n\
            \x20   inet 192.168.1.42/24 brd 192.168.1.255 scope global wlan0\n\
            \x20      valid_lft forever preferred_lft forever\n";
        assert_eq!(parse_device_lan_ip(out).as_deref(), Some("192.168.1.42"));
    }

    #[test]
    fn skips_loopback_and_takes_first_private_addr() {
        // Full interface dump: loopback must be ignored, wlan picked.
        let out = "1: lo: <LOOPBACK,UP> mtu 65536\n\
            \x20   inet 127.0.0.1/8 scope host lo\n\
            42: wlan0: <BROADCAST,MULTICAST,UP> mtu 1500\n\
            \x20   inet 10.0.0.7/24 brd 10.0.0.255 scope global wlan0\n";
        assert_eq!(parse_device_lan_ip(out).as_deref(), Some("10.0.0.7"));
    }

    #[test]
    fn private_lan_ranges() {
        assert!(is_private_lan_ip("192.168.0.1"));
        assert!(is_private_lan_ip("10.255.0.1"));
        assert!(is_private_lan_ip("172.16.5.5"));
        assert!(is_private_lan_ip("172.31.0.1"));
        assert!(!is_private_lan_ip("172.15.0.1"));
        assert!(!is_private_lan_ip("172.32.0.1"));
        assert!(!is_private_lan_ip("127.0.0.1"));
        assert!(!is_private_lan_ip("8.8.8.8"));
        assert!(!is_private_lan_ip("not.an.ip"));
    }

    #[test]
    fn parses_toybox_long_format_with_sizes() {
        // Typical `adb shell ls -lp /sdcard/DCIM/` output (toybox).
        let out = "total 48\n\
            drwxrwx--- 4 u0_a123 media_rw     3452 2024-01-02 10:30 Camera/\n\
            -rw-rw---- 1 u0_a123 media_rw  2411724 2024-01-01 12:00 IMG_0001.jpg\n\
            -rw-rw---- 1 u0_a123 media_rw 45123900 2024-01-01 12:05 VID_0002.mp4\n";
        let e = parse_ls_output(out);
        assert_eq!(e.len(), 3);
        // Directory sorts first, size zeroed.
        assert_eq!(e[0], ("Camera".to_string(), true, 0));
        // Files keep their byte sizes.
        assert_eq!(e[1], ("IMG_0001.jpg".to_string(), false, 2_411_724));
        assert_eq!(e[2], ("VID_0002.mp4".to_string(), false, 45_123_900));
    }

    #[test]
    fn keeps_spaces_in_filenames() {
        let out = "-rw-rw---- 1 u0_a123 media_rw 1024 2024-01-01 12:00 My Holiday Clip.mp4\n";
        let e = parse_ls_output(out);
        assert_eq!(e, vec![("My Holiday Clip.mp4".to_string(), false, 1024)]);
    }

    #[test]
    fn falls_back_to_bare_names() {
        // Degraded output (no -l support): just names, dirs end with '/'.
        let out = "Camera/\nIMG_0001.jpg\n";
        let e = parse_ls_output(out);
        assert_eq!(e[0], ("Camera".to_string(), true, 0));
        assert_eq!(e[1], ("IMG_0001.jpg".to_string(), false, 0));
    }

    #[test]
    fn skips_errors_and_dot_entries() {
        let out = "ls: /sdcard/nope: No such file or directory\n";
        assert!(parse_ls_output(out).is_empty());
    }
}
