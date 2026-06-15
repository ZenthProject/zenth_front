/// Settings for cryptographic algorithms and key management
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CryptographySettings {
    pub key_rotation_enabled: i32,
    pub key_rotation_days: i32,
    pub use_post_quantum: i32,
    pub signature_algorithm: String,
    pub kem_algorithm: String,
}

impl Default for CryptographySettings {
    fn default() -> Self {
        Self {
            key_rotation_enabled: 1,
            key_rotation_days: 30,
            use_post_quantum: 1,
            signature_algorithm: "dilithium2".to_string(),
            kem_algorithm: "kyber1024".to_string(),
        }
    }
}
