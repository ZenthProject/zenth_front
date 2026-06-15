/// Settings for metadata protection and traffic analysis resistance
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetadataSettings {
    pub enable_message_padding: i32,
    pub padding_size: i32,
    pub random_delay_messages: i32,
    pub max_delay_seconds: i32,
    pub hide_message_size: i32,
}

impl Default for MetadataSettings {
    fn default() -> Self {
        Self {
            enable_message_padding: 1,
            padding_size: 256,
            random_delay_messages: 0,
            max_delay_seconds: 5,
            hide_message_size: 1,
        }
    }
}
