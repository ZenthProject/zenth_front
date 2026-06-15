/// Experimental and advanced security features
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExperimentalSettings {
    pub use_obfuscation: i32,
    pub quantum_resistant_mode: i32,
    pub paranoid_mode: i32,
}

impl Default for ExperimentalSettings {
    fn default() -> Self {
        Self {
            use_obfuscation: 0,
            quantum_resistant_mode: 1,
            paranoid_mode: 0,
        }
    }
}
