use crate::pentest::rootcheck::RootStatus;

pub fn app_analysis_prompt(apps: &[(String, Vec<String>, String)]) -> String {
    let mut app_list = String::new();
    for (pkg, perms, source) in apps {
        app_list.push_str(&format!("- {} (source: {}, permissions: {})\n",
            pkg, source,
            if perms.is_empty() { "aucune".to_string() } else { perms.join(", ") }
        ));
    }

    format!(r#"Tu es un expert en securite mobile Android.
Analyse les apps Android suivantes et determine pour chacune:
- verdict: "bloatware" / "legitime" / "suspect"
- category: "tracker" / "bloatware" / "google" / "microsoft" / "enterprise" / "misc"
- profile: "minimal" / "moderate" / "aggressive"
- explanation: explication courte (1 phrase)

Reponds UNIQUEMENT en JSON valide, format:
[{{"package":"com.example","verdict":"bloatware","category":"tracker","profile":"minimal","explanation":"Tracker publicitaire"}}]

APPS:
{}"#, app_list)
}

pub fn pentest_prompt(
    model: &str,
    android_version: &str,
    sdk: u32,
    security_patch: &str,
    selinux: &str,
    bootloader: &str,
    root: &RootStatus,
    suspicious_apps: &[String],
    open_ports: &[u16],
) -> String {
    let root_status = if root.is_rooted {
        format!("Roote ({})", root.method.as_deref().unwrap_or("inconnu"))
    } else {
        "Non roote".to_string()
    };

    format!(r#"Tu es un expert en securite mobile Android.
Analyse ce telephone et identifie les failles de securite.

APPAREIL:
- Modele: {}
- Android: {} (SDK {})
- Patch securite: {}
- SELinux: {}
- Bootloader: {}
- Root: {}

APPS SUSPECTES:
{}

PORTS OUVERTS:
{:?}

Pour chaque faille trouvee, reponds UNIQUEMENT en JSON valide:
[{{"description":"...","severity":"critique/haute/moyenne/basse","patchable":true/false,"fix_action":"commande ou null","risk":"..."}}]"#,
        model, android_version, sdk, security_patch, selinux, bootloader, root_status,
        suspicious_apps.join("\n"),
        open_ports
    )
}
