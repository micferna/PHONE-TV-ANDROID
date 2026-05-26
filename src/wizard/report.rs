use crate::wizard::types::WizardState;

pub fn build_markdown_report(state: &WizardState) -> String {
    let mut out = String::new();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

    out.push_str("# Rapport d'audit Phone-TV\n\n");
    out.push_str(&format!("_Genere le {}_\n\n", now));

    if let Some(info) = &state.device_info {
        out.push_str("## Appareil\n\n");
        out.push_str(&format!("- **Modele**: {}\n", info.display_name));
        out.push_str(&format!("- **Marque**: {}\n", info.brand));
        out.push_str(&format!("- **Serial**: `{}`\n", info.serial));
        out.push_str(&format!(
            "- **Android**: {} (SDK {})\n",
            info.android_version, info.sdk
        ));
        out.push_str(&format!(
            "- **Patch securite**: {}\n\n",
            info.security_patch
        ));
    }

    out.push_str("## Scores\n\n");
    if let Some((before, _)) = &state.score_before {
        let after = state
            .score_after
            .as_ref()
            .map(|(s, _)| *s)
            .unwrap_or(*before);
        let diff = after as i32 - *before as i32;
        out.push_str(&format!(
            "- **Score securite**: {} → {} ({:+})\n",
            before, after, diff
        ));
    }
    if let Some(risk_before) = state.risk_score {
        let risk_after = state.risk_score_after.unwrap_or(risk_before);
        let diff = risk_after as i32 - risk_before as i32;
        out.push_str(&format!(
            "- **Score de risque**: {} → {} ({:+})\n",
            risk_before, risk_after, diff
        ));
    }
    out.push('\n');

    if let Some(root) = &state.root_status {
        out.push_str("## Statut root\n\n");
        if root.is_rooted {
            out.push_str(&format!(
                "Appareil **roote** ({})\n\n",
                root.method.as_deref().unwrap_or("methode inconnue")
            ));
        } else {
            out.push_str("Appareil **non roote**\n\n");
        }
    }

    if !state.vulns.is_empty() {
        out.push_str(&format!(
            "## Vulnerabilites detectees ({})\n\n",
            state.vulns.len()
        ));
        for v in &state.vulns {
            out.push_str(&format!(
                "- **[{:?}]** `{}` — {}\n",
                v.severity, v.id, v.description
            ));
        }
        out.push('\n');
    }

    if !state.posture.is_empty() {
        use crate::types::PostureStatus;
        let issues: Vec<_> = state
            .posture
            .iter()
            .filter(|p| p.status != PostureStatus::Good)
            .collect();
        if !issues.is_empty() {
            out.push_str(&format!(
                "## Posture systeme ({} alertes)\n\n",
                issues.len()
            ));
            for check in issues {
                out.push_str(&format!(
                    "- **{}**: {} (`{:?}`)\n",
                    check.name, check.value, check.status
                ));
            }
            out.push('\n');
        }
    }

    let removed: Vec<&str> = state
        .clean_results
        .iter()
        .filter(|r| r.success && r.action == "uninstall")
        .map(|r| r.package.as_str())
        .collect();
    let disabled: Vec<&str> = state
        .clean_results
        .iter()
        .filter(|r| r.success && r.action == "disable")
        .map(|r| r.package.as_str())
        .collect();
    let failed: Vec<&str> = state
        .clean_results
        .iter()
        .filter(|r| !r.success)
        .map(|r| r.package.as_str())
        .collect();

    if !removed.is_empty() || !disabled.is_empty() || !failed.is_empty() {
        out.push_str("## Actions de nettoyage\n\n");
        if !removed.is_empty() {
            out.push_str(&format!("### Supprimees ({})\n\n", removed.len()));
            for pkg in &removed {
                out.push_str(&format!("- `{}`\n", pkg));
            }
            out.push('\n');
        }
        if !disabled.is_empty() {
            out.push_str(&format!("### Desactivees ({})\n\n", disabled.len()));
            for pkg in &disabled {
                out.push_str(&format!("- `{}`\n", pkg));
            }
            out.push('\n');
        }
        if !failed.is_empty() {
            out.push_str(&format!("### Echecs ({})\n\n", failed.len()));
            for pkg in &failed {
                out.push_str(&format!("- `{}`\n", pkg));
            }
            out.push('\n');
        }
    }

    out
}
