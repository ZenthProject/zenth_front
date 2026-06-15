use crate::session::get_session_by_token_async;

const SETTINGS_KEY: &str = "app_settings";

#[tauri::command]
pub async fn save_setting(session_token: String, key: String, value: String) -> Result<String, String> {
    let session = get_session_by_token_async(session_token).await?;

    // Load current settings, merge, and save
    let current_json = session.get_setting(SETTINGS_KEY)
        .unwrap_or(None);

    let mut settings_val: serde_json::Value = match current_json {
        Some(json) => serde_json::from_str(&json)
            .unwrap_or(serde_json::Value::Object(Default::default())),
        None => serde_json::Value::Object(Default::default()),
    };

    // Automatic type detection
    let typed_value = if value == "true" {
        serde_json::Value::Bool(true)
    } else if value == "false" {
        serde_json::Value::Bool(false)
    } else if let Ok(n) = value.parse::<i64>() {
        serde_json::Value::Number(n.into())
    } else {
        serde_json::Value::String(value.clone())
    };

    if let Some(obj) = settings_val.as_object_mut() {
        obj.insert(key.clone(), typed_value);
    }

    let json = serde_json::to_string(&settings_val)
        .map_err(|e| format!("Erreur sérialisation: {}", e))?;

    session.with_db(|conn| {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SETTINGS_KEY, &json],
        )
        .map_err(|e| format!("Erreur sauvegarde: {}", e))?;
        Ok(format!("Setting '{}' sauvegardé", key))
    })
}

#[tauri::command]
pub async fn save_all_settings(session_token: String, settings: String) -> Result<String, String> {
    // Validate JSON first
    let _: serde_json::Value = serde_json::from_str(&settings)
        .map_err(|e| format!("JSON invalide: {}", e))?;

    let session = get_session_by_token_async(session_token).await?;

    session.with_db(|conn| {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SETTINGS_KEY, &settings],
        )
        .map_err(|e| format!("Erreur sauvegarde: {}", e))?;

        Ok("Tous les paramètres ont été sauvegardés".to_string())
    })
}
