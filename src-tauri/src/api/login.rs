use zenth_requests::{
    implementations::RequestsNetwork,
    request::Request,
    transports::Transport,
};
use zenth_dto::{LoginRequest, LoginResponse, AuthChallenge, AuthProof, DhtRequest, DhtResponse, Method};
use prost::Message;
use crate::api::errors::{ApiError, ApiResult, check_http_status};
use crate::api::register::{RegisterConfig, DarknetType};
use crate::utils::timestamp::plateform::current_timestamp;

pub struct LoginApiClient {
    config: RegisterConfig,
    transport: Box<dyn Transport + Send + Sync>,
}

impl LoginApiClient {
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
                return Err(ApiError::I2pNotImplemented);
            }
            DarknetType::Lokinet => {
                return Err(ApiError::LokinetNotImplemented);
            }
        };

        Ok(Self { config, transport })
    }

    pub async fn request_challenge(&self, user_hash_id: Vec<u8>) -> ApiResult<AuthChallenge> {
        let login_request = LoginRequest {
            user_hash_id: user_hash_id.clone(),
            request_challenge: true,
            proof: None,
            timestamp: current_timestamp(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let response = self.send_login_request(login_request).await?;

        check_version_outdated(&response)?;

        if !response.success {
            return Err(ApiError::Server(400, response.error_message));
        }

        response.challenge.ok_or_else(|| {
            ApiError::InvalidResponse("Server did not return a challenge".to_string())
        })
    }

    pub async fn submit_proof(&self, user_hash_id: Vec<u8>, proof: AuthProof) -> ApiResult<LoginResponse> {
        let login_request = LoginRequest {
            user_hash_id,
            request_challenge: false,
            proof: Some(proof),
            timestamp: current_timestamp(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let response = self.send_login_request(login_request).await?;

        check_version_outdated(&response)?;

        if !response.success {
            return Err(ApiError::AuthenticationFailed(response.error_message.clone()));
        }

        Ok(response)
    }

    async fn send_login_request(&self, request: LoginRequest) -> ApiResult<LoginResponse> {
        let mut request_bytes = Vec::new();
        request.encode(&mut request_bytes)?;

        let endpoint = format!("{}/", self.config.base_url);

        let request_id: [u8; 16] = rand::random();

        let dht_request = DhtRequest {
            method: Method::Login as i32,
            payload: request_bytes,
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

        let dht_response = DhtResponse::decode(&response.body[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode DhtResponse: {}", e)))?;

        if !dht_response.success {
            if !dht_response.payload.is_empty() {
                if let Ok(login_response) = LoginResponse::decode(&dht_response.payload[..]) {
                    check_version_outdated(&login_response)?;
                    return Err(ApiError::Server(response.status as u16, login_response.error_message));
                }
            }
            return Err(ApiError::Server(response.status as u16, dht_response.error_message));
        }

        let login_response = LoginResponse::decode(&dht_response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Failed to decode LoginResponse: {}", e)))?;

        Ok(login_response)
    }
}

/// Vérifie si la réponse contient un champ `version_outdated` et retourne l'erreur appropriée.
/// Utilise `min_version` en priorité (version minimale exigée par le serveur).
fn check_version_outdated(response: &LoginResponse) -> ApiResult<()> {
    if let Some(ref outdated) = response.version_outdated {
        let v = if !outdated.min_version.is_empty() {
            &outdated.min_version
        } else {
            &outdated.latest_version
        };
        return Err(ApiError::Server(400, format!("VERSION_OUTDATED:{}", v)));
    }
    Ok(())
}


