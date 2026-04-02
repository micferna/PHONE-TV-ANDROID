use crate::pentest::rootcheck::RootStatus;

/// Prompt principal — L'IA analyse TOUTES les apps du telephone
/// C'est l'IA qui decide ce qui est bloatware, pas nos filtres
pub fn full_analysis_prompt(apps: &[(String, Vec<String>, String)]) -> String {
    let mut app_list = String::new();
    for (pkg, perms, source) in apps {
        app_list.push_str(&format!("- {} (source: {}, permissions: {})\n",
            pkg, source,
            if perms.is_empty() { "aucune".to_string() } else { perms.join(", ") }
        ));
    }

    format!(r#"Tu es un expert en securite mobile Android et en vie privee.

Voici la liste COMPLETE des applications installees sur un telephone Android.
Analyse CHAQUE application et identifie:
- Les bloatwares du fabricant (apps pre-installees inutiles)
- Les trackers et services publicitaires
- Les apps de telemetrie/analytics
- Les services enterprise inutiles pour un usage personnel
- Les apps avec des permissions dangereuses ou excessives
- Les apps suspectes ou potentiellement malveillantes

Pour CHAQUE app que tu identifies comme problematique, retourne:
- package: le nom du package
- verdict: "bloatware" / "suspect" / "tracker"
- category: "tracker" / "bloatware" / "google" / "microsoft" / "enterprise" / "misc"
- profile: "minimal" (trackers/pubs) / "moderate" (+ bloatware fabricant) / "aggressive" (+ apps non essentielles)
- explanation: explication courte en francais

NE RETOURNE QUE les apps problematiques. Les apps systeme essentielles (launcher, phone, settings, etc.) ne doivent PAS apparaitre.

Reponds UNIQUEMENT avec un tableau JSON valide, sans markdown, sans ```, juste le JSON:
[{{"package":"com.example","verdict":"bloatware","category":"tracker","profile":"minimal","explanation":"Tracker publicitaire"}}]

APPLICATIONS INSTALLEES:
{}"#, app_list)
}

pub fn rootability_prompt(brand: &str, model: &str, android_version: &str, security_patch: &str) -> String {
    format!(r#"Tu es un expert en securite mobile Android specialise dans le root.

TELEPHONE:
- Marque: {}
- Modele: {}
- Android: {}
- Patch securite: {}

Reponds UNIQUEMENT en JSON valide sans markdown sans ```, juste le JSON:
{{"rootable": true/false, "confidence": "haute/moyenne/basse", "method": "methode de root ou null", "details": "explication courte", "risks": "risques du root"}}"#,
        brand, model, android_version, security_patch
    )
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

Pour chaque faille trouvee, reponds UNIQUEMENT en JSON valide sans markdown:
[{{"description":"...","severity":"critique/haute/moyenne/basse","patchable":true/false,"fix_action":"commande ou null","risk":"..."}}]"#,
        model, android_version, sdk, security_patch, selinux, bootloader, root_status,
        suspicious_apps.join("\n"),
        open_ports
    )
}
