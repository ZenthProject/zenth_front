/// Settings for identity verification and trust management
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentitySettings {
    pub require_safety_numbers: i32,
    pub warn_identity_change: i32,
    pub auto_accept_new_keys: i32,
    pub verify_all_devices: i32,
}

impl Default for IdentitySettings {
    fn default() -> Self {
        Self {
            require_safety_numbers: 1,
            warn_identity_change: 1,
            auto_accept_new_keys: 0,
            verify_all_devices: 1,
        }
    }
}
