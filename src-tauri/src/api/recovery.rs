use prost::Message;
use zenth_dto::{
    DhtRequest, DhtResponse, Method,
    PublishRecoveryKeyRequest, PublishRecoveryKeyResponse,
    RecoveryClaimRequest, RecoveryClaimResponse,
};
use zenth_requests::{implementations::RequestsNetwork, request::Request, transports::Transport};
use crate::api::errors::{ApiError, ApiResult, check_http_status};
use crate::api::register::{RegisterConfig, DarknetType};
use crate::utils::timestamp::plateform::current_timestamp;

pub struct RecoveryApiClient {
    config: RegisterConfig,
    transport: Box<dyn Transport + Send + Sync>,
}

impl RecoveryApiClient {
    pub async fn new(config: RegisterConfig) -> ApiResult<Self> {
        let transport = match config.darknet {
            DarknetType::Http => RequestsNetwork::new("http")
                .await
                .map_err(|e| ApiError::Network(format!("HTTP transport: {}", e)))?,
            DarknetType::Tor => RequestsNetwork::new("tor")
                .await
                .map_err(|e| ApiError::Network(format!("Tor transport: {}", e)))?,
            _ => return Err(ApiError::Network("Transport non supporté".to_string())),
        };
        Ok(Self { config, transport })
    }

    async fn send_dht<Req: Message, Resp: Message + Default>(
        &self,
        method: Method,
        payload: Req,
    ) -> ApiResult<Resp> {
        let mut payload_bytes = Vec::new();
        payload.encode(&mut payload_bytes).map_err(|e| ApiError::Network(e.to_string()))?;

        let request_id: [u8; 16] = rand::random();
        let dht_request = DhtRequest {
            method: method as i32,
            payload: payload_bytes,
            timestamp: current_timestamp(),
            request_id: request_id.to_vec(),
        };

        let mut dht_bytes = Vec::new();
        dht_request.encode(&mut dht_bytes).map_err(|e| ApiError::Network(e.to_string()))?;

        let req = Request {
            url: format!("{}/", self.config.base_url),
            method: "POST".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/x-protobuf".to_string()),
                ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
            ],
            body: Some(dht_bytes),
        };

        let raw = self.transport
            .send(req)
            .await
            .map_err(|e| ApiError::Network(format!("Transport: {}", e)))?;

        check_http_status(raw.status, &raw.body)?;
        let dht_response = DhtResponse::decode(&raw.body[..])
            .map_err(|e| ApiError::InvalidResponse(format!("DhtResponse: {}", e)))?;

        if !dht_response.success {
            return Err(ApiError::InvalidResponse(dht_response.error_message));
        }

        Resp::decode(dht_response.payload.as_slice())
            .map_err(|e| ApiError::InvalidResponse(format!("Response payload: {}", e)))
    }

    /// Method 29 - publish recovery Dilithium2 public key (auth with main identity key)
    pub async fn publish_recovery_key(
        &self,
        username_hash: Vec<u8>,
        recovery_pubkey: Vec<u8>,
        auth_signature: Vec<u8>,
    ) -> ApiResult<PublishRecoveryKeyResponse> {
        self.send_dht(
            Method::PublishRecoveryKey,
            PublishRecoveryKeyRequest {
                username_hash,
                recovery_dilithium_pubkey: recovery_pubkey,
                auth_signature,
                timestamp: current_timestamp(),
            },
        ).await
    }

    /// Method 30 - submit a recovery claim to replace the main identity key
    pub async fn recovery_claim(
        &self,
        username_hash: Vec<u8>,
        new_identity_pubkey: Vec<u8>,
        new_identity_signature: Vec<u8>,
        new_pre_key_bundle: Vec<u8>,
        recovery_signature: Vec<u8>,
    ) -> ApiResult<RecoveryClaimResponse> {
        self.send_dht(
            Method::RecoveryClaim,
            RecoveryClaimRequest {
                username_hash,
                new_identity_dilithium_pubkey: new_identity_pubkey,
                new_identity_signature,
                new_pre_key_bundle: new_pre_key_bundle,
                recovery_signature,
                timestamp: current_timestamp(),
            },
        ).await
    }
}
