pub mod prompts;
pub mod types;

use types::AppVerdict;

pub fn call_openrouter(api_key: &str, model: &str, prompt: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Erreur client: {}", e))?;

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.1,
        "max_tokens": 8192
    });

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Erreur reseau: {}", e))?;

    let status = resp.status();
    let body_text = resp
        .text()
        .map_err(|e| format!("Erreur lecture reponse: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "Erreur API {}: {}",
            status,
            &body_text[..500.min(body_text.len())]
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| {
        format!(
            "Erreur parsing reponse API: {} — debut: {}",
            e,
            &body_text[..300.min(body_text.len())]
        )
    })?;

    let message = &json["choices"][0]["message"];

    // Essayer content d'abord, puis reasoning (modeles de raisonnement)
    if let Some(content) = message["content"].as_str() {
        if !content.is_empty() {
            return Ok(content.to_string());
        }
    }
    if let Some(reasoning) = message["reasoning"].as_str() {
        if !reasoning.is_empty() {
            return Ok(reasoning.to_string());
        }
    }
    if let Some(details) = message["reasoning_details"].as_array() {
        let texts: Vec<&str> = details.iter().filter_map(|d| d["text"].as_str()).collect();
        if !texts.is_empty() {
            return Ok(texts.join(""));
        }
    }

    Err(format!(
        "Reponse vide du LLM: {}",
        &body_text[..500.min(body_text.len())]
    ))
}

/// Analyse COMPLETE de toutes les apps par l'IA — pas juste les inconnues
pub fn analyze_all_apps(
    api_key: &str,
    model: &str,
    apps: &[(String, Vec<String>, String)],
) -> Result<Vec<AppVerdict>, String> {
    let prompt = prompts::full_analysis_prompt(apps);
    let response = call_openrouter(api_key, model, &prompt)?;
    let json_str = extract_json_array(&response);

    serde_json::from_str::<Vec<AppVerdict>>(json_str).map_err(|e| {
        format!(
            "Erreur parsing IA: {} — extrait: {}",
            e,
            &json_str[..200.min(json_str.len())]
        )
    })
}

pub fn check_rootability(
    api_key: &str,
    model: &str,
    brand: &str,
    device_model: &str,
    android_version: &str,
    security_patch: &str,
) -> Result<types::RootabilityResult, String> {
    let prompt = prompts::rootability_prompt(brand, device_model, android_version, security_patch);
    let response = call_openrouter(api_key, model, &prompt)?;
    let json_str = extract_json_object(&response);

    serde_json::from_str::<types::RootabilityResult>(json_str)
        .map_err(|e| format!("Erreur parsing rootabilite: {}", e))
}

pub fn validate_model(api_key: &str, model: &str) -> Result<bool, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Erreur: {}", e))?;

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Reponds OK"}],
        "max_tokens": 5
    });

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Erreur reseau: {}", e))?;

    let status = resp.status();

    if status.is_success() {
        Ok(true)
    } else if status.as_u16() == 401 {
        Err("Cle API invalide".to_string())
    } else {
        let body_text = resp.text().unwrap_or_default();
        let snippet = &body_text[..200.min(body_text.len())];
        Err(format!(
            "Modele '{}' — erreur {}: {}",
            model, status, snippet
        ))
    }
}

/// Extrait un tableau JSON d'une reponse LLM (gere ```json ... ```)
fn extract_json_array(text: &str) -> &str {
    // D'abord retirer les blocs markdown ```json ... ```
    let clean = strip_markdown_code(text);

    if let Some(start) = clean.find('[') {
        if let Some(end) = clean.rfind(']') {
            return &clean[start..=end];
        }
    }
    clean
}

/// Extrait un objet JSON d'une reponse LLM
fn extract_json_object(text: &str) -> &str {
    let clean = strip_markdown_code(text);

    if let Some(start) = clean.find('{') {
        if let Some(end) = clean.rfind('}') {
            return &clean[start..=end];
        }
    }
    clean
}

/// Retire les blocs ```json ... ``` ou ``` ... ```
fn strip_markdown_code(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with("```") {
        // Trouver la fin du premier ``` (peut etre ```json\n)
        if let Some(first_newline) = trimmed.find('\n') {
            let rest = &trimmed[first_newline + 1..];
            // Trouver le ``` de fermeture
            if let Some(end) = rest.rfind("```") {
                return rest[..end].trim();
            }
            return rest.trim();
        }
    }
    trimmed
}
