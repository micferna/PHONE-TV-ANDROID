use crate::types::{DataUsage, ProcessInfo, WakelockInfo};

pub fn get_processes(_device_id: &str) -> Vec<ProcessInfo> {
    vec![]
}

pub fn get_data_usage(_device_id: &str) -> Vec<DataUsage> {
    vec![]
}

pub fn get_wakelocks(_device_id: &str) -> Vec<WakelockInfo> {
    vec![]
}
