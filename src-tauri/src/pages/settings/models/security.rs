/// Settings for general security features (auto-lock, secure deletion, etc.)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecuritySettings {
    pub auto_lock_enabled: i32,
    pub auto_lock_timeout: i32,
    pub wipe_after_failed_attempts: i32,
    pub max_failed_attempts: i32,
    pub secure_delete_messages: i32,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            auto_lock_enabled: 1,
            auto_lock_timeout: 5,
            wipe_after_failed_attempts: 0,
            max_failed_attempts: 10,
            secure_delete_messages: 1,
        }
    }
}
