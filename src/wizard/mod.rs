pub mod report;
pub mod types;

use crate::adb;
use crate::backup;
use crate::brands;
use crate::history;
use crate::pentest;
use crate::security::{apps, posture, score};
use crate::types::*;
use eframe::egui;
use std::sync::mpsc;
use types::{CleanAction, DeviceInfo, VulnFix, WizardState, WizardStep};

impl WizardState {
    pub fn start(&mut self) {
        *self = WizardState {
            active: true,
            step: WizardStep::Detection,
            ..Default::default()
        };
    }

    pub fn stop(&mut self) {
        self.active = false;
    }
}

pub fn detect_device(device_id: &str) -> Option<DeviceInfo> {
    let serial = adb::adb_device(device_id, &["shell", "getprop", "ro.serialno"])?
        .trim()
        .to_string();
    let brand = adb::adb_device(device_id, &["shell", "getprop", "ro.product.brand"])?
        .trim()
        .to_lowercase();
    let model = adb::adb_device(device_id, &["shell", "getprop", "ro.product.model"])?
        .trim()
        .to_string();
    let android_version =
        adb::adb_device(device_id, &["shell", "getprop", "ro.build.version.release"])?
            .trim()
            .to_string();
    let sdk: u32 = adb::adb_device(device_id, &["shell", "getprop", "ro.build.version.sdk"])?
        .trim()
        .parse()
        .unwrap_or(0);
    let security_patch = adb::adb_device(
        device_id,
        &["shell", "getprop", "ro.build.version.security_patch"],
    )?
    .trim()
    .to_string();

    let display_name = format!(
        "{} {}",
        brand
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default()
            + &brand[1..],
        model
    );

    Some(DeviceInfo {
        serial,
        brand,
        model,
        display_name,
        android_version,
        sdk,
        security_patch,
    })
}

pub fn trigger_detection(device_id: &str, tx: &mpsc::Sender<BgEvent>, ctx: &egui::Context) {
    let id = device_id.to_string();
    let tx = tx.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        if let Some(info) = detect_device(&id) {
            let history = history::load_history(&info.serial);
            let _ = tx.send(BgEvent::HistoryLoaded { history });
            if let Some(db) = brands::load_brand(&info.brand) {
                let _ = tx.send(BgEvent::BrandsLoaded { db });
            }
            let _ = tx.send(BgEvent::WizardDeviceDetected { info });
        }
        ctx.request_repaint();
    });
}

pub fn trigger_scan(device_id: &str, tx: &mpsc::Sender<BgEvent>, ctx: &egui::Context) {
    let id = device_id.to_string();
    let tx = tx.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        // Envoyer un signal immédiat pour montrer que ça démarre
        let _ = tx.send(BgEvent::WizardScanProgress {
            current: 0,
            total: 0,
            package: "Chargement de la liste...".into(),
        });
        ctx.request_repaint();

        let all_packages = apps::list_packages(&id, AppFilter::All);
        let total = all_packages.len();

        // Envoyer le total dès qu'on l'a
        let _ = tx.send(BgEvent::WizardScanProgress {
            current: 0,
            total,
            package: "Demarrage de l'analyse...".into(),
        });
        ctx.request_repaint();

        let mut app_infos = Vec::new();
        for (i, pkg) in all_packages.iter().enumerate() {
            let _ = tx.send(BgEvent::WizardScanProgress {
                current: i + 1,
                total,
                package: pkg.clone(),
            });
            ctx.request_repaint();
            if let Some(info) = apps::get_app_detail(&id, pkg) {
                app_infos.push(info);
            }
        }
        let posture_checks = posture::check_device_posture(&id);
        let (score_val, issues) = score::calculate_score(&id);

        let _ = tx.send(BgEvent::WizardScanComplete {
            apps: app_infos,
            posture: posture_checks,
            score: score_val,
            issues,
        });
        ctx.request_repaint();
    });
}

pub fn trigger_pentest(device_id: &str, tx: &mpsc::Sender<BgEvent>, ctx: &egui::Context) {
    let id = device_id.to_string();
    let tx = tx.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = pentest::run_pentest(&id);
        let _ = tx.send(BgEvent::WizardPentestComplete {
            vulns: result.vulns,
            root: result.root_status,
            risk_score: result.risk_score,
        });
        ctx.request_repaint();
    });
}

pub fn build_clean_actions(
    apps: &[AppInfo],
    brand_db: Option<&brands::types::BrandDb>,
    profile: &brands::types::CleanProfile,
) -> (Vec<CleanAction>, Vec<String>) {
    let mut actions = Vec::new();
    let mut unknown = Vec::new();

    let known_entries: Vec<&brands::types::BloatwareEntry> = brand_db
        .map(|db| brands::entries_for_profile(db, profile))
        .unwrap_or_default();

    for app in apps {
        if let Some(entry) = known_entries.iter().find(|e| e.package == app.package) {
            actions.push(CleanAction {
                package: app.package.clone(),
                action: "uninstall".into(),
                description: entry.description.clone(),
                selected: true,
                from_ai: false,
            });
        }
    }

    if let Some(db) = brand_db {
        for app in apps {
            let is_known = db.bloatware.iter().any(|e| e.package == app.package);
            let matches_prefix = db
                .meta
                .prefixes
                .iter()
                .any(|p| app.package.starts_with(p.as_str()));
            if !is_known && !matches_prefix && app.installer == AppInstaller::Unknown {
                unknown.push(app.package.clone());
            }
        }
    }

    (actions, unknown)
}

pub fn trigger_cleaning(
    device_id: &str,
    serial: &str,
    actions: &[CleanAction],
    vuln_fixes: &[VulnFix],
    tx: &mpsc::Sender<BgEvent>,
    ctx: &egui::Context,
) {
    let id = device_id.to_string();
    let serial = serial.to_string();
    let tx = tx.clone();
    let ctx = ctx.clone();
    let actions: Vec<CleanAction> = actions.iter().filter(|a| a.selected).cloned().collect();
    let fixes: Vec<VulnFix> = vuln_fixes.iter().filter(|f| f.selected).cloned().collect();

    std::thread::spawn(move || {
        // Backup APKs of apps to be uninstalled (disable is reversible via pm enable)
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = backup::session_dir(&serial, &timestamp);
        let mut manifest = backup::BackupManifest {
            serial: serial.clone(),
            timestamp: timestamp.clone(),
            apks: Vec::new(),
        };
        for action in actions.iter().filter(|a| a.action == "uninstall") {
            if let Some(local) = backup::backup_apk(&id, &action.package, &backup_dir) {
                if let Some(file) = local.file_name() {
                    manifest.apks.push(backup::BackedUpApk {
                        package: action.package.clone(),
                        file: file.to_string_lossy().to_string(),
                    });
                }
            }
        }
        if !manifest.apks.is_empty() {
            backup::write_manifest(&backup_dir, &manifest);
            let _ = tx.send(BgEvent::Log(format!(
                "Sauvegarde: {} APK(s) dans {}",
                manifest.apks.len(),
                backup_dir.display()
            )));
            ctx.request_repaint();
        }

        for action in &actions {
            let (success, message) = match action.action.as_str() {
                "uninstall" => apps::uninstall_app(&id, &action.package),
                "disable" => apps::disable_app(&id, &action.package),
                _ => (false, "Action inconnue".into()),
            };
            let _ = tx.send(BgEvent::WizardCleanProgress {
                package: action.package.clone(),
                action: action.action.clone(),
                success,
                message,
            });
            ctx.request_repaint();
        }

        for fix in &fixes {
            let success = posture::fix_setting(&id, &fix.fix_command);
            let _ = tx.send(BgEvent::WizardCleanProgress {
                package: fix.vuln_id.clone(),
                action: "fix".into(),
                success,
                message: fix.description.clone(),
            });
            ctx.request_repaint();
        }

        let _ = tx.send(BgEvent::WizardCleanComplete);
        ctx.request_repaint();
    });
}
