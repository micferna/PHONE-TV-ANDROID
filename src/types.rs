use std::process::{Child, ChildStdin};

#[derive(Clone, Default)]
pub struct TransferState {
    pub active: bool,
    pub filename: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub done: bool,
    pub play_after: bool,
}

#[allow(dead_code)]
pub enum BgEvent {
    DevicesLoaded(Vec<Device>),
    NetworkScanDone(Vec<String>),
    WifiConnected { addr: String, success: bool },
    WebcamSwitched(Option<Child>),
    StorageInfo { device_id: String, total: String, used: String, avail: String, percent: f32 },
    BatteryInfo { device_id: String, level: u8, status: String },
    PhoneApps { device_id: String, apps: Vec<String> },
    ScreenshotReady { device_id: String, data: Vec<u8> },
    Log(String),
}

#[derive(Clone, PartialEq)]
pub enum DeviceType {
    Phone,
    Tv,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Devices,
    Tv,
    Phone,
    Video,
}

#[derive(Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub status: String,
    pub device_type: DeviceType,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TvChannel {
    pub name: String,
    pub number: u32,
}

pub struct TvShell {
    pub device_id: String,
    pub child: Child,
    pub stdin: ChildStdin,
}
