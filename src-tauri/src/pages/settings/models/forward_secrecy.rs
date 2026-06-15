/// Settings for Perfect Forward Secrecy and Double Ratchet protocol
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForwardSecrecySettings {
    pub double_ratchet_enabled: i32,
    pub max_skip_message_keys: i32,
    pub ratchet_on_every_message: i32,
}

impl Default for ForwardSecrecySettings {
    fn default() -> Self {
        Self {
            double_ratchet_enabled: 1,
            max_skip_message_keys: 1000,
            ratchet_on_every_message: 0,
        }
    }
}
