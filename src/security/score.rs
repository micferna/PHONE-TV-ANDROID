use crate::types::SecurityIssue;

pub fn calculate_score(_device_id: &str) -> (u8, Vec<SecurityIssue>) {
    (100, vec![])
}
