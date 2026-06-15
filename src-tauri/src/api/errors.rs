use thiserror::Error;
use displaydoc::Display;
use prost::Message as ProstMessage;

#[derive(Display, Error, Debug)]
pub enum ApiError {
    /// Network error: {0}
    Network(String),
    /// Request timeout after {0}s
    Timeout(u64),
    /// Server error ({0}): {1}
    Server(u16, String),
    /// Invalid response format: {0}
    InvalidResponse(String),
    /// Serialization error: {0}
    Serialization(String),
    /// I2P connection not implemented yet
    I2pNotImplemented,
    /// Lokinet connection not implemented yet
    LokinetNotImplemented,
    /// Invalid registration data: {0}
    InvalidRegistrationData(String),
    /// Authentication failed: {0}
    AuthenticationFailed(String),
    /// Max retries ({0}) exceeded
    MaxRetriesExceeded(u32),
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ApiError::Timeout(30)
        } else if err.is_connect() {
            ApiError::Network(format!("Connection failed: {}", err))
        } else {
            ApiError::Network(err.to_string())
        }
    }
}

impl From<prost::EncodeError> for ApiError {
    fn from(err: prost::EncodeError) -> Self {
        ApiError::Serialization(format!("Protobuf encoding error: {}", err))
    }
}

impl From<prost::DecodeError> for ApiError {
    fn from(err: prost::DecodeError) -> Self {
        ApiError::InvalidResponse(format!("Protobuf decoding error: {}", err))
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;

/// Vérifie le status HTTP avant toute tentative de décodage Protobuf.
/// Retourne une erreur propre si le serveur est indisponible ou en erreur.
pub fn check_http_status(status: u16, body: &[u8]) -> ApiResult<()> {
    if status == 200 {
        return Ok(());
    }
    let msg = match status {
        0                => "SERVER_UNAVAILABLE: no response (network unreachable or DNS failure)".to_string(),
        408 | 504        => "SERVER_UNAVAILABLE: request timed out".to_string(),
        429              => "SERVER_UNAVAILABLE: too many requests, please wait".to_string(),
        500..=599        => format!("SERVER_UNAVAILABLE: server error (HTTP {})", status),
        400..=499        => {
            // Essayer de décoder DhtResponse → error_message ou payload → LoginResponse
            if let Ok(dht) = zenth_dto::DhtResponse::decode(body) {
                if !dht.error_message.is_empty() {
                    return Err(ApiError::Server(status, dht.error_message));
                }
                if !dht.payload.is_empty() {
                    if let Ok(login_resp) = zenth_dto::LoginResponse::decode(dht.payload.as_slice()) {
                        // Champ version_outdated prioritaire: reformater pour que Login.tsx le détecte
                        // min_version = version minimale exigée par le serveur (ce vers quoi upgrader)
                        // latest_version peut contenir la version du client rejetée (écho serveur)
                        if let Some(ref outdated) = login_resp.version_outdated {
                            let v = if !outdated.min_version.is_empty() {
                                &outdated.min_version
                            } else {
                                &outdated.latest_version
                            };
                            return Err(ApiError::Server(status, format!("VERSION_OUTDATED:{}", v)));
                        }
                        if !login_resp.error_message.is_empty() {
                            return Err(ApiError::Server(status, login_resp.error_message));
                        }
                    }
                }
            }
            // Fallback : body brut sans caractères non-imprimables
            let preview: String = body[..body.len().min(200)]
                .iter()
                .map(|&b| if b >= 0x20 && b < 0x7f { b as char } else { '?' })
                .collect::<String>()
                .split('?')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            format!("SERVER_ERROR: client error (HTTP {}): {}", status, preview)
        }
        _                => format!("SERVER_ERROR: unexpected HTTP status {}", status),
    };
    Err(ApiError::Server(status, msg))
}
