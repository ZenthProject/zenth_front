//! Centralized application configuration.
//!
//! In debug mode, environment variables can override default values.
//! In release mode, all values are hardcoded into the binary.

/// Returns the DHT API URL.
///
/// In debug mode, the `DHT_API_URL` environment variable can override the default.
pub fn dht_api_url() -> String {
    #[cfg(debug_assertions)]
    {
        if let Ok(url) = std::env::var("DHT_API_URL") {
            return url;
        }
        return "http://127.0.0.1:8081".to_string();
    }

    #[cfg(not(debug_assertions))]
    "https://api.zenth-project.com".to_string()
}

/// Returns the WebSocket server URL.
///
/// In debug mode, the VITE_TAURI_WS_HOST environment variable can override the default.
pub fn ws_url() -> String {
    #[cfg(debug_assertions)]
    {
        if let Ok(url) = std::env::var("VITE_TAURI_WS_HOST") {
            return url;
        }
        return "ws://127.0.0.1:8081".to_string();
    }

    #[cfg(not(debug_assertions))]
    "wss://api.zenth-project.com".to_string()
}

/// Returns whether invalid TLS certificates should be accepted.
///
/// Always false in release builds. In debug mode only, setting
/// ZENTH_ACCEPT_INVALID_CERTS=1 enables this option for local
/// development with self-signed certificates.
pub fn accept_invalid_certs() -> bool {
    #[cfg(debug_assertions)]
    {
        std::env::var("ZENTH_ACCEPT_INVALID_CERTS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false)
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}
