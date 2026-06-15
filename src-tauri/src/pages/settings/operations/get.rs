use crate::session::get_session_by_token_async;

const SETTINGS_KEY: &str = "app_settings";

/// Tauri command to get a single setting value from the encrypted user database
#[tauri::command]
pub async fn get_setting(session_token: String, key: String) -> Result<String, String> {
    let session = get_session_by_token_async(session_token).await?;

    let json = session.get_setting(SETTINGS_KEY)
        .map_err(|e| format!("Erreur lecture settings: {}", e))?
        .unwrap_or_else(|| "{}".to_string());

    let settings_val: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Erreur parsing JSON: {}", e))?;

    let field = settings_val.get(&key)
        .ok_or_else(|| format!("Setting '{}' non trouvé", key))?;

    let value = match field {
        serde_json::Value::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    Ok(value)
}
