pub mod prompts;
pub mod types;

use types::{AppVerdict, LlmVuln};

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
        "max_tokens": 4096
    });

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Erreur reseau: {}", e))?;

    let status = resp.status();
    // Lire le body comme texte brut d'abord
    let body_text = resp.text().map_err(|e| format!("Erreur lecture reponse: {}", e))?;

    if !status.is_success() {
        return Err(format!("Erreur API {}: {}", status, &body_text[..200.min(body_text.len())]));
    }

    let json: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Erreur parsing JSON: {} — reponse brute: {}", e, &body_text[..300.min(body_text.len())]))?;

    let message = &json["choices"][0]["message"];

    // Essayer content d'abord, puis reasoning (modeles de raisonnement)
    if let Some(content) = message["content"].as_str() {
        if !content.is_empty() {
            return Ok(content.to_string());
        }
    }
    // Certains modeles mettent la reponse dans reasoning
    if let Some(reasoning) = message["reasoning"].as_str() {
        if !reasoning.is_empty() {
            return Ok(reasoning.to_string());
        }
    }
    // Ou dans reasoning_details[].text
    if let Some(details) = message["reasoning_details"].as_array() {
        let texts: Vec<&str> = details.iter()
            .filter_map(|d| d["text"].as_str())
            .collect();
        if !texts.is_empty() {
            return Ok(texts.join(""));
        }
    }

    Err(format!("Reponse vide du LLM — reponse brute: {}", &body_text[..500.min(body_text.len())]))
}

pub fn analyze_apps(api_key: &str, model: &str, apps: &[(String, Vec<String>, String)]) -> Result<Vec<AppVerdict>, String> {
    let prompt = prompts::app_analysis_prompt(apps);
    let response = call_openrouter(api_key, model, &prompt)?;
    let json_str = extract_json(&response);

    serde_json::from_str::<Vec<AppVerdict>>(json_str)
        .map_err(|e| format!("Erreur parsing verdicts: {}", e))
}

pub fn analyze_pentest(api_key: &str, model: &str, prompt: &str) -> Result<Vec<LlmVuln>, String> {
    let response = call_openrouter(api_key, model, prompt)?;
    let json_str = extract_json(&response);

    serde_json::from_str::<Vec<LlmVuln>>(json_str)
        .map_err(|e| format!("Erreur parsing pentest: {}", e))
}

pub fn check_rootability(api_key: &str, model: &str, brand: &str, device_model: &str, android_version: &str, security_patch: &str) -> Result<types::RootabilityResult, String> {
    let prompt = prompts::rootability_prompt(brand, device_model, android_version, security_patch);
    let response = call_openrouter(api_key, model, &prompt)?;
    let json_str = extract_json_object(&response);

    serde_json::from_str::<types::RootabilityResult>(json_str)
        .map_err(|e| format!("Erreur parsing rootabilite: {}", e))
}

pub fn validate_model(api_key: &str, model: &str) -> Result<bool, String> {
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "test"}],
        "max_tokens": 1
    });

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Erreur reseau: {}", e))?;

    if resp.status().is_success() {
        Ok(true)
    } else {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if body.contains("model") || body.contains("invalid") || status.as_u16() == 404 {
            Err(format!("Modele '{}' invalide ou non disponible", model))
        } else if status.as_u16() == 401 {
            Err("Cle API invalide".to_string())
        } else {
            Err(format!("Erreur {}: {}", status, &body[..100.min(body.len())]))
        }
    }
}

fn extract_json_object(text: &str) -> &str {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return &text[start..=end];
        }
    }
    text
}

fn extract_json(text: &str) -> &str {
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            return &text[start..=end];
        }
    }
    text
}
