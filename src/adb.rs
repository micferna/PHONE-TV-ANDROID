use std::path::Path;
use std::process::{Child, Command};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::types::{AudioMode, Device, DeviceType, TransferState};

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

/// Read several device properties in a single round trip.
///
/// Two costs stack up here, and neither is the work itself: an `adb shell` is ~30 ms
/// of round trip, and every `getprop` is another ~20 ms of process spawn on the phone.
/// Dumping the whole property table pays each once. Measured on a moto g14 over USB:
///
///   3 separate `adb shell getprop <p>`      90 ms
///   3 `getprop` inside one `adb shell`      67 ms
///   1 `adb shell getprop` (whole table)     36 ms
///
/// The 40 KB that comes back parses in microseconds, so the dump wins outright.
/// Always returns exactly `props.len()` entries; a property the device does not
/// define comes back empty, which is what the callers want.
fn get_props(id: &str, props: &[&str]) -> Vec<String> {
    let dump = adb_device(id, &["shell", "getprop"]).unwrap_or_default();
    let table: std::collections::HashMap<&str, &str> =
        dump.lines().filter_map(parse_getprop_line).collect();
    props
        .iter()
        .map(|p| table.get(p).copied().unwrap_or_default().to_string())
        .collect()
}

/// Split one line of a bare `getprop` dump, whose format is `[key]: [value]`.
/// Returns `None` for anything that doesn't match, so junk lines drop out.
fn parse_getprop_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once("]: [")?;
    Some((key.strip_prefix('[')?, value.strip_suffix(']')?))
}

/// True when the identifying strings point at a TV rather than a handset.
fn looks_like_tv(name: &str, features: &str, product: &str) -> bool {
    let name = name.to_lowercase();
    features.to_lowercase().contains("tv")
        || product.to_lowercase().contains("tv")
        || name.contains("tv")
        || name.contains("shield")
        || name.contains("chromecast")
        || name.contains("mibox")
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

                let (name, device_type) = if status == "device" {
                    let props = get_props(
                        &id,
                        &[
                            "ro.product.model",
                            "ro.build.characteristics",
                            "ro.product.name",
                        ],
                    );
                    let name = if props[0].is_empty() {
                        id.clone()
                    } else {
                        props[0].clone()
                    };
                    let kind = if looks_like_tv(&name, &props[1], &props[2]) {
                        DeviceType::Tv
                    } else {
                        DeviceType::Phone
                    };
                    (name, kind)
                } else {
                    // Unauthorized / offline: no shell to ask, so fall back to the
                    // model `adb devices -l` already printed.
                    let name = parts
                        .iter()
                        .find(|p| p.starts_with("model:"))
                        .map(|p| p.replace("model:", ""))
                        .unwrap_or_else(|| id.clone());
                    (name, DeviceType::Unknown)
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

/// Drop the screen: release the "stay on" pin and put the device back to sleep.
///
/// The pin outlives whatever set it (a mirror session, the Stay Awake switch), so
/// clearing it first is what makes the sleep stick.
pub fn screen_off(id: &str) {
    set_stay_awake_cmd(id, false);
    press_key(id, "KEYCODE_SLEEP");
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

/// Ask the device to open `url` in a video player.
///
/// The URL reaches the device's shell, so it is quoted like any other untrusted
/// argument (see [`send_text_to_device`]). `video/*` is quoted too: unquoted, the
/// device shell would glob it, and it only survived because the pattern happens to
/// match nothing in adbd's working directory.
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
            &shell_quote(url),
            "-t",
            "'video/*'",
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

        if let Some(output) = adb_device(
            &monitor_id,
            &["shell", "stat", "-c", "%s", &shell_quote(&monitor_remote)],
        ) {
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
                // `remote` carries the local filename the user picked, so it is
                // quoted for the device shell like any other untrusted argument.
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
                        &shell_quote(&format!("file://{}", remote)),
                        "-t",
                        "'video/*'",
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
/// Returns the wireless device id (`ip:5555`) and whether *this call* is what opened
/// the port, so the caller can close what it opened and leave alone what it found
/// already running. If `id` is already a network transport (contains ':') it is
/// returned unchanged. Returns `None` when the phone has no reachable WiFi IP or the
/// wireless connection can't be established — callers should then fall back to the
/// original (USB) transport.
pub fn enable_wifi_adb(id: &str) -> Option<(String, bool)> {
    use std::time::Duration;

    // Already a network transport (ip:port): nothing to switch.
    if id.contains(':') {
        return Some((id.to_string(), false));
    }

    // Read the phone's WiFi IP while the USB transport is still up.
    let ip = get_device_wifi_ip(id)?;
    let addr = format!("{}:5555", ip);

    // Fast path: adbd may already be in TCP/IP mode (e.g. from a previous run).
    // A short, bounded TCP probe — `adb connect` itself can hang forever on an
    // unreachable host, so we never call it without first proving the port is open.
    if port_reachable(&addr, Duration::from_millis(600)) {
        return connect_adb_wifi(&addr).then(|| (addr.clone(), false));
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
    connect_adb_wifi(&addr).then(|| (addr.clone(), true))
}

/// Put adbd back on USB only, closing the TCP/5555 listener this app opened.
///
/// `adb tcpip 5555` leaves the phone accepting debug connections from the whole LAN,
/// and nothing ever took that back: the exposure outlived the stream that needed it,
/// for as long as the phone stayed up. Only the caller that opened the port should
/// call this — a port that was already open belongs to whoever opened it.
///
/// Sending `usb` over the wireless transport is what tears it down, so the command
/// necessarily kills its own connection; a non-zero exit here is expected and says
/// nothing about whether the port closed.
pub fn disable_wifi_adb(id: &str) {
    let _ = Command::new("adb").args(["-s", id, "usb"]).output();
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

/// How many probes the network scan runs at once.
///
/// One OS thread per host meant 254 of them for a /24, nearly all of it spent parked
/// in `connect`, and 254 simultaneous SYNs is also the kind of burst consumer routers
/// meter. The trade is latency, since the timeout dominates — measured on a quiet /24
/// with the 400 ms budget below:
///
///   254 threads  0.46 s    32 workers  3.20 s
///    96 workers  1.20 s    64 workers  1.60 s
///
/// 96 keeps under half the threads for well under a second of extra wait, on a scan
/// that already runs off-thread behind a spinner.
const SCAN_WORKERS: usize = 96;
/// Per-host connect budget. Dominates the total: nearly every address on a home LAN
/// is silent and costs the full timeout.
const SCAN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(400);

/// Scan the local /24 for hosts listening on TCP/5555 (ADB wireless port).
/// Pure-Rust parallel scan — works on Linux, macOS and Windows.
pub fn scan_network_for_adb() -> Vec<String> {
    use std::net::{SocketAddr, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Arc, Mutex};

    let prefix = match get_local_ip_prefix() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let found: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    // Shared cursor rather than a fixed slice each: hosts that answer immediately
    // cost nothing, so a worker that gets a fast range simply moves on to more.
    let next = Arc::new(AtomicUsize::new(1));
    let mut handles = Vec::with_capacity(SCAN_WORKERS);

    for _ in 0..SCAN_WORKERS {
        let (found, next, prefix) = (Arc::clone(&found), Arc::clone(&next), prefix.clone());
        handles.push(std::thread::spawn(move || loop {
            let host = next.fetch_add(1, AtomicOrdering::Relaxed);
            if host > 254 {
                return;
            }
            let ip = format!("{}{}", prefix, host);
            if let Ok(addr) = format!("{}:5555", ip).parse::<SocketAddr>() {
                if TcpStream::connect_timeout(&addr, SCAN_TIMEOUT).is_ok() {
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

/// Stop `child` and everything it spawned, giving it a chance to clean up first.
///
/// SIGKILL alone left the phone lit: scrcpy restores `stay_on_while_plugged_in` and
/// the screen power mode in its own teardown, and a killed process never runs it, so
/// every `--stay-awake` mirror left the screen pinned on — burning battery long after
/// the window was gone. Ask politely, wait for the teardown, force only if it hangs.
pub fn kill_child_tree(child: &mut Child) {
    let pid = child.id().to_string();
    #[cfg(unix)]
    {
        let _ = Command::new("pkill").args(["-TERM", "-P", &pid]).output();
        let _ = Command::new("kill").args(["-TERM", &pid]).output();
    }
    #[cfg(windows)]
    {
        // Without /F, taskkill asks the process to close itself.
        let _ = Command::new("taskkill").args(["/T", "/PID", &pid]).output();
    }

    // The teardown is a couple of adb round trips. Poll instead of sleeping the whole
    // budget: a clean exit costs ~100 ms here, and this runs on the UI thread.
    for _ in 0..20 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    #[cfg(unix)]
    {
        let _ = Command::new("pkill").args(["-P", &pid]).output();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/F", "/T", "/PID", &pid]).output();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The flatpak the README recommends when no distribution package is available.
#[cfg(target_os = "linux")]
const AURYNK_FLATPAK: &str = "io.github.IshuSinghSE.aurynk";

/// Is `name` an executable on PATH?
fn on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

/// Is the aurynk flatpak actually installed? `flatpak` being on PATH says nothing
/// about the app, and `flatpak run` on a missing app spawns fine and then fails.
#[cfg(target_os = "linux")]
fn aurynk_installed() -> bool {
    Command::new("flatpak")
        .args(["info", AURYNK_FLATPAK])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build a scrcpy invocation, or `None` when scrcpy cannot be found.
///
/// A distribution package on PATH is the common case and is preferred; the flatpak
/// is the documented fallback. Resolving this up front — rather than always spawning
/// `flatpak` and letting it fail — is what lets callers tell the user *why* nothing
/// happened instead of silently doing nothing.
fn scrcpy_command() -> Option<Command> {
    if on_path("scrcpy") {
        return Some(Command::new("scrcpy"));
    }
    #[cfg(target_os = "linux")]
    {
        if on_path("flatpak") && aurynk_installed() {
            let mut cmd = Command::new("flatpak");
            cmd.args(["run", "--command=scrcpy", AURYNK_FLATPAK]);
            return Some(cmd);
        }
    }
    None
}

/// Why mirroring and the webcam cannot start, or `None` when scrcpy is available.
///
/// The UI shows this instead of a button click that appears to do nothing.
pub fn scrcpy_unavailable_reason() -> Option<String> {
    if scrcpy_command().is_some() {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        Some(format!(
            "scrcpy introuvable. Installez-le (`sudo apt install scrcpy`) \
             ou le flatpak : `flatpak install flathub {AURYNK_FLATPAK}`"
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Some("scrcpy introuvable. Ajoutez scrcpy au PATH.".to_string())
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

/// The v4l2loopback device scrcpy writes to.
#[cfg(target_os = "linux")]
const WEBCAM_SINK: &str = "/dev/video10";
/// Devices the fan-out copies frames to, one per consuming application: a device
/// serves exactly one, so this is the cap on how many apps can use the camera at
/// once. Declared in /etc/modprobe.d/v4l2loopback.conf; missing ones are skipped.
#[cfg(target_os = "linux")]
const FANOUT_SINKS: [&str; 4] = [
    "/dev/video12",
    "/dev/video13",
    "/dev/video14",
    "/dev/video15",
];

/// Blocks for up to ~30s while retrying; call it off the UI thread.
pub fn start_webcam_process(id: &str, front: bool, audio: AudioMode) -> Option<Child> {
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
    args.push(format!("--v4l2-sink={WEBCAM_SINK}"));

    args.extend(audio.scrcpy_args());

    for attempt in 1..=WEBCAM_ATTEMPTS {
        // A failure to spawn at all means no scrcpy binary: retrying won't help.
        let mut child = scrcpy_command()?.args(&args).spawn().ok()?;
        std::thread::sleep(WEBCAM_SETTLE);
        match child.try_wait() {
            Ok(None) => {
                #[cfg(target_os = "linux")]
                {
                    // Before the PipeWire check: the nodes it waits for are the
                    // fan-out sinks, which only advertise capture once ffmpeg writes.
                    start_webcam_fanout();
                    ensure_pipewire_camera_node();
                }
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

/// v4l2loopback hands out a single capture token per device (`V4L2L_TOKEN_CAPTURE`
/// is one bit, granted once), so exactly one application can stream from a given
/// device; every other reader's S_FMT/REQBUFS fails with -EBUSY. `max_openers` does
/// not help: it only bounds open(), which is all an app needs to *enumerate* the
/// camera. So Firefox and Discord can never share /dev/video10 directly.
///
/// Read the sink once and copy the frames out to a dedicated device per application:
///
///   scrcpy -> /dev/video10 -> ffmpeg -+-> /dev/video12
///                                     `-> /dev/video13
///
/// Each application then takes the token of *its own* device. Frames are copied, not
/// re-encoded. Mirrors scripts/webcam-fanout.sh, which does the same thing by hand.
#[cfg(target_os = "linux")]
static FANOUT: Mutex<Option<Child>> = Mutex::new(None);
/// Bumped on every start/stop so a fan-out that finishes spawning after its webcam
/// was already stopped kills itself instead of leaking an orphan ffmpeg.
#[cfg(target_os = "linux")]
static FANOUT_GEN: AtomicU64 = AtomicU64::new(0);

/// scrcpy may not have opened the sink yet when ffmpeg starts, and a later
/// face-unlock eviction kills the stream mid-flight: the fan-out thread waits for
/// the source, supervises ffmpeg, and relaunches it for as long as its generation
/// is the current one, instead of burning a fixed number of attempts.
#[cfg(target_os = "linux")]
const FANOUT_POLL: std::time::Duration = std::time::Duration::from_secs(1);
#[cfg(target_os = "linux")]
const FANOUT_SETTLE: std::time::Duration = std::time::Duration::from_millis(1500);
#[cfg(target_os = "linux")]
const FANOUT_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// True while a writer holds the scrcpy sink open. With `exclusive_caps=1` a
/// loopback device advertises "Video Capture" from the moment a writer opens it and
/// sets a format, and falls back to "Video Output" only once that writer closes.
///
/// So this answers "has scrcpy opened the sink" — the same check
/// scripts/webcam-fanout.sh waits on before starting ffmpeg — and *not* "are frames
/// flowing": a scrcpy frozen by a face-unlock eviction keeps the device advertising
/// capture (verified: SIGSTOP-ing a writer leaves the capability set untouched, and
/// v4l2loopback's sysfs `state`/`buffers`/`format` are equally static). Frame flow is
/// tracked from the fan-out's own counter instead — see [`webcam_stream_stalled`].
#[cfg(target_os = "linux")]
fn webcam_sink_has_writer() -> bool {
    match Command::new("v4l2-ctl")
        .args(["-d", WEBCAM_SINK, "-D"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains("Video Capture"),
        // Pas de v4l2-ctl : impossible de sonder, ne pas bloquer le fan-out.
        Err(_) => true,
    }
}

/// Frame counter of the running fan-out ffmpeg, fed by its `-progress` stream.
/// `None` whenever no fan-out is copying frames, so a missing fan-out never reads
/// as a stall.
#[cfg(target_os = "linux")]
struct FanoutHealth {
    /// Frames copied so far, as last reported by ffmpeg.
    frames: u64,
    /// When `frames` last moved — or when the fan-out started.
    last_advance: std::time::Instant,
}

#[cfg(target_os = "linux")]
static FANOUT_HEALTH: Mutex<Option<FanoutHealth>> = Mutex::new(None);

/// A fan-out that copies nothing for this long is reading a dead source, not a slow
/// one. Sized above the ~9s a face-unlock HAL holds the sensor, so a stream that is
/// merely about to recover on its own is not cut short.
#[cfg(target_os = "linux")]
const FANOUT_STALL_AFTER: std::time::Duration = std::time::Duration::from_secs(15);

/// True when the fan-out is up but has copied no new frame for
/// [`FANOUT_STALL_AFTER`]: scrcpy still holds the sink yet has gone silent, which is
/// the state a face-unlock eviction leaves behind. False when no fan-out runs —
/// without a frame counter there is nothing to conclude, and guessing would restart
/// healthy streams.
#[cfg(target_os = "linux")]
pub fn webcam_stream_stalled() -> bool {
    FANOUT_HEALTH.lock().is_ok_and(|health| {
        health
            .as_ref()
            .is_some_and(|h| h.last_advance.elapsed() >= FANOUT_STALL_AFTER)
    })
}

#[cfg(not(target_os = "linux"))]
pub fn webcam_stream_stalled() -> bool {
    false
}

/// Follow ffmpeg's `-progress` stream and keep [`FANOUT_HEALTH`] current. ffmpeg
/// blocks once an unread pipe fills, so this must run for as long as the child does;
/// EOF (the child exited) simply ends the thread, its lifetime being the supervisor's
/// business.
#[cfg(target_os = "linux")]
fn track_fanout_progress(stdout: std::process::ChildStdout, generation: u64) {
    use std::io::BufRead;

    for line in std::io::BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
    {
        if FANOUT_GEN.load(Ordering::SeqCst) != generation {
            return;
        }
        let Some(frames) = line
            .strip_prefix("frame=")
            .and_then(|n| n.trim().parse::<u64>().ok())
        else {
            continue;
        };
        let Ok(mut health) = FANOUT_HEALTH.lock() else {
            return;
        };
        if let Some(h) = health.as_mut() {
            if frames > h.frames {
                h.frames = frames;
                h.last_advance = std::time::Instant::now();
            }
        }
    }
}

/// Start (or clear) the frame counter the stall probe reads.
#[cfg(target_os = "linux")]
fn set_fanout_health(active: bool) {
    if let Ok(mut health) = FANOUT_HEALTH.lock() {
        *health = active.then(|| FanoutHealth {
            frames: 0,
            last_advance: std::time::Instant::now(),
        });
    }
}

/// The devices applications are meant to consume. Falls back to the raw scrcpy sink
/// when no fan-out device exists, which is the pre-fan-out single-consumer behaviour.
#[cfg(target_os = "linux")]
fn consumer_devices() -> Vec<&'static str> {
    let sinks: Vec<&'static str> = FANOUT_SINKS
        .iter()
        .copied()
        .filter(|s| Path::new(s).exists())
        .collect();
    if sinks.is_empty() {
        vec![WEBCAM_SINK]
    } else {
        sinks
    }
}

/// Non-blocking: the fan-out comes up on its own thread.
#[cfg(target_os = "linux")]
fn start_webcam_fanout() {
    stop_webcam_fanout();
    let sinks = consumer_devices();
    if sinks == [WEBCAM_SINK] {
        return; // rien à dupliquer
    }
    let generation = FANOUT_GEN.load(Ordering::SeqCst);

    std::thread::spawn(move || loop {
        if FANOUT_GEN.load(Ordering::SeqCst) != generation {
            return;
        }
        // Ne lancer ffmpeg que quand scrcpy pousse réellement des images : après
        // une éviction face-unlock la source peut rester muette bien plus
        // longtemps que quelques tentatives, on l'attend au lieu d'abandonner.
        if !webcam_sink_has_writer() {
            std::thread::sleep(FANOUT_POLL);
            continue;
        }

        let mut cmd = Command::new("ffmpeg");
        // `-progress pipe:1` turns stdout into the frame counter the stall probe
        // reads; `-nostats` keeps the human-readable status line off it.
        cmd.args([
            "-loglevel",
            "error",
            "-nostats",
            "-progress",
            "pipe:1",
            "-f",
            "v4l2",
            "-i",
            WEBCAM_SINK,
        ]);
        for sink in &sinks {
            cmd.args(["-map", "0:v", "-f", "v4l2", "-pix_fmt", "yuv420p", sink]);
        }
        // No ffmpeg on the box: retrying won't conjure one.
        let Ok(mut child) = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            return;
        };

        // Drain `-progress` from the start: an unread pipe eventually blocks ffmpeg.
        set_fanout_health(true);
        if let Some(stdout) = child.stdout.take() {
            std::thread::spawn(move || track_fanout_progress(stdout, generation));
        }

        std::thread::sleep(FANOUT_SETTLE);
        if !matches!(child.try_wait(), Ok(None)) {
            // Mort pendant le settle : la source a pu se taire entre le sondage
            // et le spawn, on repart l'attendre.
            kill_child_tree(&mut child);
            set_fanout_health(false);
            std::thread::sleep(FANOUT_RETRY_DELAY);
            continue;
        }

        {
            let mut slot = match FANOUT.lock() {
                Ok(slot) => slot,
                Err(_) => return,
            };
            // Stopped while we were settling: don't resurrect it.
            if FANOUT_GEN.load(Ordering::SeqCst) != generation {
                drop(slot);
                kill_child_tree(&mut child);
                set_fanout_health(false);
                return;
            }
            *slot = Some(child);
        }

        // Superviser : si ffmpeg meurt (crash, kill, -EBUSY tardif), on repart
        // attendre le flux plutôt que de laisser les devices des applications
        // orphelins. NB : la perte du writer ne suffit pas à le tuer —
        // v4l2loopback ne remonte pas d'EOF et ffmpeg rediffuse la dernière
        // image — c'est stop_webcam_fanout(), appelé sur les chemins d'arrêt
        // de la webcam, qui couvre ce cas via le saut de génération.
        loop {
            std::thread::sleep(FANOUT_POLL);
            let mut slot = match FANOUT.lock() {
                Ok(slot) => slot,
                Err(_) => return,
            };
            if FANOUT_GEN.load(Ordering::SeqCst) != generation {
                // stop_webcam_fanout a déjà récupéré et tué l'enfant.
                return;
            }
            match slot.as_mut().map(std::process::Child::try_wait) {
                Some(Ok(None)) => {}
                _ => {
                    *slot = None;
                    drop(slot);
                    set_fanout_health(false);
                    break;
                }
            }
        }
        std::thread::sleep(FANOUT_RETRY_DELAY);
    });
}

#[cfg(target_os = "linux")]
pub fn stop_webcam_fanout() {
    if let Ok(mut slot) = FANOUT.lock() {
        // Under the lock, so an in-flight start_webcam_fanout thread observes the
        // bump before it can store its child.
        FANOUT_GEN.fetch_add(1, Ordering::SeqCst);
        if let Some(mut child) = slot.take() {
            drop(slot);
            kill_child_tree(&mut child);
        }
    }
    // Plus de fan-out : plus de compteur d'images, donc plus de verdict « muet ».
    set_fanout_health(false);
}

#[cfg(not(target_os = "linux"))]
pub fn stop_webcam_fanout() {}

/// Make sure PipeWire actually publishes the fan-out devices as camera sources.
///
/// With `exclusive_caps=1` a loopback device advertises capture only while a writer
/// holds it open. WirePlumber enumerates through udev at start-up and does not
/// re-probe when a writer later appears — verified: opening a writer on an idle sink
/// leaves the PipeWire node count at zero indefinitely. So a device configured that
/// way and probed while idle stays invisible to everything that goes through
/// PipeWire (Firefox), even though direct-V4L2 apps (Discord) still find it.
///
/// Restarting WirePlumber forces the re-probe, but it is a blunt instrument: it is
/// the session manager for *audio* too, so every application's routing gets
/// re-evaluated because a camera was missing.
///
/// The way to not need this at all is to give the fan-out sinks `exclusive_caps=0`
/// (see setup-webcam.sh): they then advertise capture permanently, WirePlumber
/// publishes them at boot, and the check below simply passes. The source keeps
/// `exclusive_caps=1`, because [`webcam_sink_has_writer`] reads exactly that flip.
/// The restart therefore only ever fires on setups predating that change.
#[cfg(target_os = "linux")]
fn ensure_pipewire_camera_node() {
    std::thread::spawn(|| {
        for i in 0..10 {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if pipewire_has_camera_sources() {
                return;
            }
            // One restart, and not on the first pass: scrcpy and ffmpeg may not have
            // opened the sinks yet, and re-probing still-idle devices achieves
            // nothing but the disruption.
            if i == 1 {
                let _ = Command::new("systemctl")
                    .args(["--user", "restart", "wireplumber"])
                    .output();
            }
        }
    });
}

/// True once every device an application could pick has a PipeWire source node.
#[cfg(target_os = "linux")]
fn pipewire_has_camera_sources() -> bool {
    let Ok(out) = Command::new("pw-dump").output() else {
        return true; // pas de PipeWire → rien à réparer
    };
    let Ok(objects) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return true;
    };
    let Some(objs) = objects.as_array() else {
        return true;
    };
    consumer_devices().iter().all(|dev| {
        objs.iter().any(|o| {
            let props = &o["info"]["props"];
            props["media.class"] == "Video/Source"
                && props["object.path"]
                    .as_str()
                    .is_some_and(|p| p.contains(dev))
        })
    })
}

pub fn start_mirror_process(id: &str, stay_awake: bool, audio: AudioMode) -> Option<Child> {
    let mut args = vec![
        "-s".to_string(),
        id.to_string(),
        "--turn-screen-off".to_string(),
        // Leave the phone dark on the way out, whatever state the session ended in.
        "--power-off-on-close".to_string(),
    ];
    args.extend(audio.scrcpy_args());
    if stay_awake {
        args.push("--stay-awake".to_string());
    }
    scrcpy_command()?.args(&args).spawn().ok()
}

/// Returns true on platforms that can route the phone camera straight to a
/// system-visible virtual webcam without extra software (Linux + v4l2loopback).
/// On Windows/macOS the user needs OBS Virtual Camera or similar.
pub const fn webcam_direct_supported() -> bool {
    cfg!(target_os = "linux")
}

/// Type `text` on the device.
///
/// `adb shell` joins its arguments and hands the result to the *device's* shell, so
/// any metacharacter left unquoted runs as a command there — `;`, `` ` ``, `$(…)`,
/// `|` and `&` all did, since the previous escaping covered neither them nor the
/// backslash that would have escaped them. Wrap the payload in POSIX single quotes
/// instead: they are the one quoting form a shell does not interpret at all. Spaces
/// keep going through `input`'s own `%s` placeholder, which it expands itself, well
/// after the shell is done.
pub fn send_text_to_device(id: &str, text: &str) {
    let payload = shell_quote(&text.replace(' ', "%s"));
    adb_fire(id, &["shell", "input", "text", &payload]);
}

/// Quote `s` for the device shell. Single quotes suppress every expansion; an
/// embedded quote has to leave and re-enter them, which is what `'\''` does.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
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
    // A device path is untrusted input — it can hold spaces and shell metacharacters.
    let out = adb_device(id, &["shell", "ls", "-lp", &shell_quote(remote)]).unwrap_or_default();
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

    // Staging path for the pull. Not /tmp: a fixed name in a world-writable
    // directory lets any local account pre-plant a symlink there and have `adb pull`
    // follow it, overwriting a file of ours — and it would leave the screenshot
    // readable to everyone besides. The config dir is owner-only (0700).
    let cache = crate::config::config_dir().join("cache");
    let _ = std::fs::create_dir_all(&cache);
    let local_tmp = cache.join("screenshot.png");
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
    use super::{
        is_private_lan_ip, looks_like_tv, parse_device_lan_ip, parse_getprop_line, parse_ls_output,
        shell_quote,
    };

    #[test]
    fn parses_getprop_dump_lines() {
        assert_eq!(
            parse_getprop_line("[ro.product.model]: [moto g14]"),
            Some(("ro.product.model", "moto g14"))
        );
        // Empty value: the property exists but is unset.
        assert_eq!(
            parse_getprop_line("[ro.build.characteristics]: []"),
            Some(("ro.build.characteristics", ""))
        );
        // A bracket inside the value must not truncate it.
        assert_eq!(
            parse_getprop_line("[some.prop]: [a[b]c]"),
            Some(("some.prop", "a[b]c"))
        );
        // Anything not in `[k]: [v]` form is not a property line.
        assert_eq!(parse_getprop_line("garbage"), None);
        assert_eq!(parse_getprop_line(""), None);
    }

    #[test]
    fn recognises_tv_devices() {
        // `ro.build.characteristics` is the reliable signal when it's there.
        assert!(looks_like_tv("BRAVIA 4K", "tv,nosdcard", "atv"));
        // Otherwise fall back to well-known names, case-insensitively.
        assert!(looks_like_tv("NVIDIA SHIELD", "", ""));
        assert!(looks_like_tv("Chromecast", "", ""));
        assert!(looks_like_tv("MiBox S", "", ""));
        // A handset must not be mistaken for one.
        assert!(!looks_like_tv("moto g14", "default", "cancun_gen"));
        assert!(!looks_like_tv("Pixel 8", "", ""));
    }

    #[test]
    fn shell_quote_neutralises_metacharacters() {
        // Everything the device shell would otherwise act on stays literal.
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("a; reboot"), "'a; reboot'");
        assert_eq!(shell_quote("$(id)`id`"), "'$(id)`id`'");
        assert_eq!(shell_quote("a|b&c<d>e*"), "'a|b&c<d>e*'");
        // A backslash is data, not an escape: it must not be able to end the quote.
        assert_eq!(shell_quote(r"back\slash"), r"'back\slash'");
    }

    #[test]
    fn shell_quote_escapes_embedded_quotes() {
        // The payload leaves the quotes, contributes a literal ', and re-enters.
        assert_eq!(shell_quote("it's"), r#"'it'\''s'"#);
        // A closing quote followed by a command is the injection this blocks.
        assert_eq!(shell_quote("x'; id #"), r#"'x'\''; id #'"#);
    }

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
