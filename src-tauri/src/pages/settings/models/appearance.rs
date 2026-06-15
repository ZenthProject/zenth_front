/// Settings related to application appearance and UI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppearanceSettings {
    pub theme: String,
    pub language: String,
    pub font_size: String,
    pub compact_mode: i32,
    pub message_bubble_style: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "fr".to_string(),
            font_size: "medium".to_string(),
            compact_mode: 0,
            message_bubble_style: "rounded".to_string(),
        }
    }
}
