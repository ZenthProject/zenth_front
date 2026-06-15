use crate::session::get_session_by_token_async;

const SETTINGS_KEY: &str = "app_settings";

#[tauri::command]
pub async fn load_settings(session_token: String) -> Result<String, String> {
    let session = get_session_by_token_async(session_token).await?;

    session.get_setting(SETTINGS_KEY)
        .map(|opt| opt.unwrap_or_else(|| "{}".to_string()))
}
