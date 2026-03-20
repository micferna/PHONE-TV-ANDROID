use crate::types::AppInfo;

pub fn list_packages(_device_id: &str) -> Vec<String> {
    vec![]
}

pub fn get_app_detail(_device_id: &str, _package: &str) -> AppInfo {
    AppInfo::default()
}
