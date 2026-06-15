/// Settings for network connection parameters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectionSettings {
    pub connection_timeout_seconds: i32,
    pub max_retry_attempts: i32,
    pub use_websocket_compression: i32,
    pub keepalive_interval_seconds: i32,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            connection_timeout_seconds: 30,
            max_retry_attempts: 3,
            use_websocket_compression: 1,
            keepalive_interval_seconds: 60,
        }
    }
}
