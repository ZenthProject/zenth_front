use crate::session::get_session_by_token_async;

const SETTINGS_KEY: &str = "app_settings";

/// Réinitialise les paramètres en supprimant la clé.
/// Au prochain load_settings, le frontend tombera sur ses propres defaultSettings.
#[tauri::command]
pub async fn reset_settings(session_token: String) -> Result<String, String> {
    let session = get_session_by_token_async(session_token).await?;

    session.with_db(|conn| {
        conn.execute(
            "DELETE FROM settings WHERE key = ?1",
            [SETTINGS_KEY],
        ).map_err(|e| format!("Erreur reset: {}", e))?;

        Ok("Paramètres réinitialisés aux valeurs par défaut".to_string())
    })
}

/// Tauri command for delete_setting (not supported, use reset_settings instead)
#[tauri::command]
pub async fn delete_setting(_session_token: String, _key: String) -> Result<String, String> {
    Err("La suppression de paramètres individuels n'est pas supportée. Utilisez reset_settings() pour tout réinitialiser.".to_string())
}
