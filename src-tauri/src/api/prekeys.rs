//! Pre-key API client for X3DH key exchange
//!
//! Handles:
//! - Uploading pre-keys after registration
//! - Fetching pre-key bundles for initiating sessions
//! - Checking pre-key count
//! - Replenishing pre-keys when low

use zenth_requests::{
    implementations::RequestsNetwork,
    request::Request,
    transports::Transport,
};
use zenth_dto::{
    DhtRequest, DhtResponse, Method,
    PreKey, SignedPreKey, KyberPreKey, PreKeyBundle,
    UploadPreKeysRequest, UploadPreKeysResponse,
    FetchPreKeyBundleRequest, FetchPreKeyBundleResponse,
    CheckPreKeyCountRequest, CheckPreKeyCountResponse,
    ReplenishPreKeysRequest, ReplenishPreKeysResponse,
};
use prost::Message;
use std::time::Duration;
use crate::api::errors::{ApiError, ApiResult, check_http_status};
use crate::api::register::DarknetType;
use crate::utils::timestamp::plateform::current_timestamp;

/// Configuration for the pre-key API client
#[derive(Debug, Clone)]
pub struct PreKeyConfig {
    /// Base URL of the DHT endpoint
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

impl Default for PreKeyConfig {
    fn default() -> Self {
        Self {
            base_url: crate::config::dht_api_url(),
            darknet: DarknetType::Http,
            timeout_secs: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// Pre-key API client
pub struct PreKeyApiClient {
    config: PreKeyConfig,
    transport: Box<dyn Transport + Send + Sync>,
}

impl PreKeyApiClient {
    /// Create a new pre-key API client
    pub async fn new(config: PreKeyConfig) -> ApiResult<Self> {
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
            DarknetType::Lokinet => {
                RequestsNetwork::new("lokinet")
                    .await
                    .map_err(|e| ApiError::Network(format!("Failed to initialize Lokinet transport: {}", e)))?
            }
            DarknetType::I2P => {
                return Err(ApiError::I2pNotImplemented);
            }
        };

        Ok(Self { config, transport })
    }

    /// Upload pre-keys to the server (after registration or key rotation)
    pub async fn upload_prekeys(
        &self,
        username_hash: Vec<u8>,
        one_time_prekeys: Vec<PreKey>,
        signed_prekey: SignedPreKey,
        kyber_prekey: KyberPreKey,
        kyber_last_resort: KyberPreKey,
        auth_signature: Vec<u8>,
    ) -> ApiResult<UploadPreKeysResponse> {
        let request = UploadPreKeysRequest {
            username_hash,
            one_time_prekeys,
            signed_prekey: Some(signed_prekey),
            kyber_prekey: Some(kyber_prekey),
            kyber_last_resort: Some(kyber_last_resort),
            auth_signature,
            timestamp: current_timestamp(),
        };

        let response = self.send_request(Method::UploadPrekeys, &request.encode_to_vec()).await?;

        UploadPreKeysResponse::decode(&response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode UploadPreKeysResponse: {}", e)))
    }

    /// Fetch pre-key bundle for a target user (for X3DH initiation)
    pub async fn fetch_prekey_bundle(
        &self,
        requester_hash: Vec<u8>,
        target_hash: Vec<u8>,
        auth_signature: Vec<u8>,
    ) -> ApiResult<FetchPreKeyBundleResponse> {
        let request = FetchPreKeyBundleRequest {
            requester_hash,
            target_hash,
            auth_signature,
            timestamp: current_timestamp(),
        };

        let response = self.send_request(Method::FetchPrekeyBundle, &request.encode_to_vec()).await?;

        FetchPreKeyBundleResponse::decode(&response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode FetchPreKeyBundleResponse: {}", e)))
    }

    /// Check remaining pre-key count
    pub async fn check_prekey_count(
        &self,
        username_hash: Vec<u8>,
        auth_signature: Vec<u8>,
    ) -> ApiResult<CheckPreKeyCountResponse> {
        let request = CheckPreKeyCountRequest {
            username_hash,
            auth_signature,
            timestamp: current_timestamp(),
        };

        let response = self.send_request(Method::CheckPrekeyCount, &request.encode_to_vec()).await?;

        CheckPreKeyCountResponse::decode(&response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode CheckPreKeyCountResponse: {}", e)))
    }

    /// Replenish one-time pre-keys
    pub async fn replenish_prekeys(
        &self,
        username_hash: Vec<u8>,
        new_prekeys: Vec<PreKey>,
        auth_signature: Vec<u8>,
    ) -> ApiResult<ReplenishPreKeysResponse> {
        let request = ReplenishPreKeysRequest {
            username_hash,
            new_prekeys,
            auth_signature,
            timestamp: current_timestamp(),
        };

        let response = self.send_request(Method::ReplenishPrekeys, &request.encode_to_vec()).await?;

        ReplenishPreKeysResponse::decode(&response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode ReplenishPreKeysResponse: {}", e)))
    }

    /// Send a request to the DHT endpoint with retries
    async fn send_request(&self, method: Method, payload: &[u8]) -> ApiResult<DhtResponse> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(
                    self.config.retry_delay_ms * (attempt as u64)
                )).await;
            }

            match self.send_request_once(method, payload).await {
                Ok(response) => {
                    if !response.success && !response.error_message.is_empty() {
                        return Err(ApiError::Server(400, response.error_message));
                    }
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ApiError::MaxRetriesExceeded(self.config.max_retries)
        }))
    }

    /// Send a single request (no retry)
    async fn send_request_once(&self, method: Method, payload: &[u8]) -> ApiResult<DhtResponse> {
        let endpoint = format!("{}/", self.config.base_url);
        let request_id: [u8; 16] = rand::random();

        let dht_request = DhtRequest {
            method: method as i32,
            payload: payload.to_vec(),
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

        check_http_status(response.status, &response.body)?;
        DhtResponse::decode(&response.body[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode DhtResponse: {}", e)))
    }

    /// Get current configuration
    pub fn config(&self) -> &PreKeyConfig {
        &self.config
    }
}

/// Minimum pre-key threshold before replenishment
pub const MIN_PREKEY_COUNT: u32 = 20;

/// Default number of pre-keys to generate
pub const DEFAULT_PREKEY_COUNT: u32 = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PreKeyConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.darknet, DarknetType::Http);
    }

    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
    }
}
