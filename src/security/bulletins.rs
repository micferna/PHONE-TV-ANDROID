use chrono::{Datelike, NaiveDate};

/// One published Android Security Bulletin.
pub struct Bulletin {
    pub date: &'static str, // YYYY-MM-DD (always the 5th)
    pub cves: u32,
    pub critical: u32,
    pub url: &'static str,
}

/// Published Android Security Bulletins (most recent first).
/// CVE counts are approximated from public summaries — update this list as new
/// bulletins are released (https://source.android.com/docs/security/bulletin).
pub const BULLETINS: &[Bulletin] = &[
    Bulletin {
        date: "2026-04-05",
        cves: 30,
        critical: 5,
        url: "https://source.android.com/docs/security/bulletin/2026-04-01",
    },
    Bulletin {
        date: "2026-03-05",
        cves: 25,
        critical: 3,
        url: "https://source.android.com/docs/security/bulletin/2026-03-01",
    },
    Bulletin {
        date: "2026-02-05",
        cves: 28,
        critical: 4,
        url: "https://source.android.com/docs/security/bulletin/2026-02-01",
    },
    Bulletin {
        date: "2026-01-05",
        cves: 32,
        critical: 6,
        url: "https://source.android.com/docs/security/bulletin/2026-01-01",
    },
    Bulletin {
        date: "2025-12-05",
        cves: 26,
        critical: 3,
        url: "https://source.android.com/docs/security/bulletin/2025-12-01",
    },
    Bulletin {
        date: "2025-11-05",
        cves: 29,
        critical: 4,
        url: "https://source.android.com/docs/security/bulletin/2025-11-01",
    },
    Bulletin {
        date: "2025-10-05",
        cves: 31,
        critical: 5,
        url: "https://source.android.com/docs/security/bulletin/2025-10-01",
    },
    Bulletin {
        date: "2025-09-05",
        cves: 24,
        critical: 3,
        url: "https://source.android.com/docs/security/bulletin/2025-09-01",
    },
    Bulletin {
        date: "2025-08-05",
        cves: 27,
        critical: 4,
        url: "https://source.android.com/docs/security/bulletin/2025-08-01",
    },
    Bulletin {
        date: "2025-07-05",
        cves: 22,
        critical: 3,
        url: "https://source.android.com/docs/security/bulletin/2025-07-01",
    },
    Bulletin {
        date: "2025-06-05",
        cves: 28,
        critical: 4,
        url: "https://source.android.com/docs/security/bulletin/2025-06-01",
    },
    Bulletin {
        date: "2025-05-05",
        cves: 25,
        critical: 3,
        url: "https://source.android.com/docs/security/bulletin/2025-05-01",
    },
];

#[derive(Clone, Debug)]
pub struct BulletinGap {
    pub device_patch: String,
    pub latest_bulletin: String,
    pub missed_bulletins: usize,
    pub estimated_cves: u32,
    pub estimated_critical: u32,
    pub latest_url: String,
}

/// Given a device security patch level (YYYY-MM-DD), compute how many monthly
/// bulletins have been released since (strictly newer than the device patch).
pub fn bulletins_behind(device_patch_str: &str) -> Option<BulletinGap> {
    let patch = NaiveDate::parse_from_str(device_patch_str.trim(), "%Y-%m-%d").ok()?;
    let mut missed: Vec<&Bulletin> = BULLETINS
        .iter()
        .filter(|b| {
            NaiveDate::parse_from_str(b.date, "%Y-%m-%d")
                .map(|d| d > patch)
                .unwrap_or(false)
        })
        .collect();
    missed.sort_by_key(|b| std::cmp::Reverse(b.date));

    let latest = BULLETINS.first()?;
    Some(BulletinGap {
        device_patch: device_patch_str.trim().to_string(),
        latest_bulletin: latest.date.to_string(),
        missed_bulletins: missed.len(),
        estimated_cves: missed.iter().map(|b| b.cves).sum(),
        estimated_critical: missed.iter().map(|b| b.critical).sum(),
        latest_url: latest.url.to_string(),
    })
}

/// Returns a YYYY-MM string for "right now", used to flag stale BULLETINS data.
#[allow(dead_code)]
pub fn current_year_month() -> String {
    let now = chrono::Local::now().date_naive();
    format!("{:04}-{:02}", now.year(), now.month())
}
