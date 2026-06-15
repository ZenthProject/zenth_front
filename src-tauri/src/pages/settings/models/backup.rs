/// Settings for encrypted backups
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupSettings {
    pub encrypted_backup_enabled: i32,
    pub backup_frequency: String,
    pub backup_location: String,
    pub include_media_in_backup: i32,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            encrypted_backup_enabled: 0,
            backup_frequency: "weekly".to_string(),
            backup_location: "local".to_string(),
            include_media_in_backup: 0,
        }
    }
}
