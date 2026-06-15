use zenth_requests::{
    implementations::RequestsNetwork,
    request::Request,
    response::Response,
    transports::Transport,
};
use zenth_dto::{RegistrationRequest, RegistrationResponse, DhtRequest, DhtResponse, Method};
use prost::Message;
use std::time::Duration;
use rand::Rng;
use crate::api::errors::{ApiError, ApiResult};
use crate::utils::timestamp::plateform::current_timestamp;

#[derive(Debug, Clone, PartialEq)]
pub enum DarknetType {
    Tor,
    I2P,
    Lokinet,
    Http,
}

impl DarknetType {
    fn as_str(&self) -> &str {
        match self {
            DarknetType::Tor => "tor",
            DarknetType::I2P => "i2p",
            DarknetType::Lokinet => "lokinet",
            DarknetType::Http => "http",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterConfig {
    /// Base URL of the registration endpoint
    pub base_url: String,
    /// Darknet type (Tor, I2P, Lokinet, Http)
    pub darknet: DarknetType,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for RegisterConfig {
    fn default() -> Self {
        Self {
            base_url: crate::config::dht_api_url(),
            darknet: DarknetType::Tor,
            timeout_secs: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

pub struct RegisterApiClient {
    config: RegisterConfig,
    transport: Box<dyn Transport + Send + Sync>,
}

impl RegisterApiClient {

    pub async fn new(config: RegisterConfig) -> ApiResult<Self> {
        if config.base_url.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Base URL cannot be empty".to_string()
            ));
        }

        let transport = match config.darknet {
            DarknetType::Tor => {
                RequestsNetwork::new("tor")
                    .await
                    .map_err(|e| ApiError::Network(format!("Failed to initialize Tor transport: {}", e)))?
            }
            DarknetType::Http => {
                RequestsNetwork::new("http")
                    .await
                    .map_err(|e| ApiError::Network(format!("Failed to initialize HTTP transport: {}", e)))?
            }
            DarknetType::I2P => {
                // TODO: Implement I2P connection
                // This requires:
                // 1. I2P router running locally (typically on port 4444)
                // 2. I2P proxy configuration in zenth_requests
                // 3. I2P address resolution (.i2p domains)
                // 4. Uncomment I2P transport in zenth_requests/src/implementations.rs
                //
                // Example implementation:
                // RequestsNetwork::new("i2p")
                //     .await
                //     .map_err(|e| ApiError::Network(format!("Failed to initialize I2P transport: {}", e)))?

                return Err(ApiError::I2pNotImplemented);
            }
            DarknetType::Lokinet => {
                // TODO: Implement Lokinet connection
                // This requires:
                // 1. Lokinet daemon running
                // 2. Lokinet proxy configuration in zenth_requests
                // 3. Lokinet address resolution (.loki domains)
                // 4. Proper Lokinet transport implementation
                //
                // Currently, zenth_requests falls back to HTTP for Lokinet
                // Example implementation:
                // RequestsNetwork::new("lokinet")
                //     .await
                //     .map_err(|e| ApiError::Network(format!("Failed to initialize Lokinet transport: {}", e)))?

                return Err(ApiError::LokinetNotImplemented);
            }
        };

        Ok(Self { config, transport })
    }

    pub async fn register(&self, request: RegistrationRequest) -> ApiResult<RegistrationResponse> {
        self.validate_request(&request)?;
        let mut request_bytes = Vec::new();
        request.encode(&mut request_bytes)?;
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(
                    self.config.retry_delay_ms * (attempt as u64)
                )).await;

            }

            match self.send_request(&request_bytes).await {
                Ok(response) => {
                    if !response.success && !response.error_message.is_empty() {
                        return Err(ApiError::Server(
                            400,
                            response.error_message.clone()
                        ));
                    }
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);

                    if let Some(ApiError::InvalidRegistrationData(_)) = &last_error {
                        break;
                    }
                    if let Some(ApiError::AuthenticationFailed(_)) = &last_error {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ApiError::MaxRetriesExceeded(self.config.max_retries)
        }))
    }

    async fn send_request(&self, request_bytes: &[u8]) -> ApiResult<RegistrationResponse> {
        let endpoint = format!("{}/", self.config.base_url);
        let request_id: [u8; 16] = rand::random();
        let dht_request = DhtRequest {
            method: Method::Register as i32,
            payload: request_bytes.to_vec(),
            timestamp: current_timestamp(),
            request_id: request_id.to_vec(),
        };

        let mut dht_request_bytes = Vec::new();
        dht_request.encode(&mut dht_request_bytes)?;

        let req = Request {
            url: endpoint,
            method: "POST".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/x-protobuf".to_string()),
                ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
            ],
            body: Some(dht_request_bytes),
        };

        let response = self.transport
            .send(req)
            .await
            .map_err(|e| ApiError::Network(format!("Transport send failed: {}", e)))?;

        let dht_response = DhtResponse::decode(&response.body[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode DhtResponse: {}", e)))?;

        if !dht_response.success {
            if !dht_response.payload.is_empty() {
                if let Ok(reg_response) = RegistrationResponse::decode(&dht_response.payload[..]) {
                    return Err(ApiError::Server(response.status as u16, reg_response.error_message));
                }
            }
            return Err(ApiError::Server(response.status as u16, dht_response.error_message));
        }

        let registration_response = RegistrationResponse::decode(&dht_response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode RegistrationResponse: {}", e)))?;

        Ok(registration_response)
    }

    fn validate_request(&self, request: &RegistrationRequest) -> ApiResult<()> {
        if request.username_hash.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Username hash cannot be empty".to_string()
            ));
        }

        if request.username_hash.len() != 32 {
            return Err(ApiError::InvalidRegistrationData(
                format!("Invalid username hash length: expected 32 (SHA256), got {}",
                        request.username_hash.len())
            ));
        }

        if request.pre_key_bundle.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Pre-key bundle cannot be empty".to_string()
            ));
        }

        if request.password_commitment.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Password commitment cannot be empty".to_string()
            ));
        }

        if request.initial_proof.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Initial proof cannot be empty".to_string()
            ));
        }

        if request.identity_key_dilithium.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Identity key cannot be empty".to_string()
            ));
        }

        if request.identity_signature.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Identity signature cannot be empty".to_string()
            ));
        }

        if request.timestamp == 0 {
            return Err(ApiError::InvalidRegistrationData(
                "Timestamp cannot be zero".to_string()
            ));
        }

        Ok(())
    }

    pub fn config(&self) -> &RegisterConfig {
        &self.config
    }

    pub async fn health_check(&self) -> ApiResult<bool> {
        let endpoint = format!("{}/", self.config.base_url);

        let req = Request {
            url: endpoint,
            method: "POST".to_string(),
            headers: vec![
                ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
            ],
            body: None,
        };

        match self.transport.send(req).await {
            Ok(response) => Ok(response.status >= 200 && response.status < 500),
            Err(e) => Err(ApiError::Network(format!("Health check failed: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RegisterConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.darknet, DarknetType::Tor);
    }

    #[test]
    fn test_darknet_type_as_str() {
        assert_eq!(DarknetType::Tor.as_str(), "tor");
        assert_eq!(DarknetType::I2P.as_str(), "i2p");
        assert_eq!(DarknetType::Lokinet.as_str(), "lokinet");
        assert_eq!(DarknetType::Http.as_str(), "http");
    }

    #[tokio::test]
    async fn test_validate_empty_username_hash() {
        let config = RegisterConfig {
            darknet: DarknetType::Http,
            ..Default::default()
        };
        let client = RegisterApiClient::new(config).await.unwrap();

        let request = RegistrationRequest {
            username_hash: vec![],
            pre_key_bundle: vec![1, 2, 3],
            password_commitment: vec![1, 2, 3],
            initial_proof: vec![1, 2, 3],
            identity_key_dilithium: vec![1, 2, 3],
            identity_signature: vec![1, 2, 3],
            timestamp: 12345,
            proof_type: 0,
        };

        let result = client.validate_request(&request);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_i2p_not_implemented() {
        let config = RegisterConfig {
            darknet: DarknetType::I2P,
            ..Default::default()
        };
        let result = RegisterApiClient::new(config).await;
        assert!(matches!(result, Err(ApiError::I2pNotImplemented)));
    }

    #[tokio::test]
    async fn test_lokinet_not_implemented() {
        let config = RegisterConfig {
            darknet: DarknetType::Lokinet,
            ..Default::default()
        };
        let result = RegisterApiClient::new(config).await;
        assert!(matches!(result, Err(ApiError::LokinetNotImplemented)));
    }
}
