use zenth_requests::{
    implementations::RequestsNetwork,
    request::Request,
    transports::Transport,
};
use zenth_dto::{DhtRequest, DhtResponse, Method};
use prost::Message;
use crate::api::errors::{ApiError, ApiResult};
use crate::api::register::{RegisterConfig, DarknetType};

#[derive(Clone, prost::Message)]
pub struct DeleteRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub user_hash_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub dilithium_signature: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub timestamp: u64,
}

#[derive(Clone, prost::Message)]
pub struct DeleteResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(string, tag = "2")]
    pub error_message: String,
}

pub struct DeleteApiClient {
    config: RegisterConfig,
    transport: Box<dyn Transport + Send + Sync>,
}

impl DeleteApiClient {
    pub async fn new(config: RegisterConfig) -> ApiResult<Self> {
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
            DarknetType::I2P => return Err(ApiError::I2pNotImplemented),
            DarknetType::Lokinet => return Err(ApiError::LokinetNotImplemented),
        };

        Ok(Self { config, transport })
    }

    pub async fn delete_account(
        &self,
        user_hash_id: Vec<u8>,
        dilithium_signature: Vec<u8>,
        timestamp: u64,
    ) -> ApiResult<DeleteResponse> {
        let req = DeleteRequest { user_hash_id, dilithium_signature, timestamp };

        let mut payload = Vec::new();
        req.encode(&mut payload)?;

        let request_id: [u8; 16] = rand::random();

        let dht_request = DhtRequest {
            method: Method::Delete as i32,
            payload,
            timestamp,
            request_id: request_id.to_vec(),
        };

        let mut dht_bytes = Vec::new();
        dht_request.encode(&mut dht_bytes)?;

        let http_req = Request {
            url: format!("{}/", self.config.base_url),
            method: "POST".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/x-protobuf".to_string()),
                ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
            ],
            body: Some(dht_bytes),
        };

        let response = self.transport
            .send(http_req)
            .await
            .map_err(|e| ApiError::Network(format!("Transport error: {}", e)))?;

        let dht_response = DhtResponse::decode(&response.body[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode DhtResponse: {}", e)))?;

        let delete_response = DeleteResponse::decode(&dht_response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode DeleteResponse: {}", e)))?;

        Ok(delete_response)
    }
}
