/// Settings for file storage, compression, and cleanup
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageSettings {
    pub auto_download_files: i32,
    pub max_file_size_mb: i32,
    pub compress_images: i32,
    pub compress_quality: i32,
    pub auto_cleanup_old_messages: i32,
    pub cleanup_after_days: i32,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            auto_download_files: 0,
            max_file_size_mb: 50,
            compress_images: 1,
            compress_quality: 85,
            auto_cleanup_old_messages: 0,
            cleanup_after_days: 90,
        }
    }
}
