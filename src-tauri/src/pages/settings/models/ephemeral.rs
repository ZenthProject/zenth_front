/// Settings for ephemeral (self-destructing) messages
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EphemeralSettings {
    pub ephemeral_messages_default: i32,
    pub default_ephemeral_timer: i32,
    pub ephemeral_after_read: i32,
}

impl Default for EphemeralSettings {
    fn default() -> Self {
        Self {
            ephemeral_messages_default: 0,
            default_ephemeral_timer: 86400,
            ephemeral_after_read: 0,
        }
    }
}
