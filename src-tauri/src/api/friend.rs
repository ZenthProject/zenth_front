// Friend API Client
//
// NOTE: The server-side friend request methods (LookupUser, SendFriendRequest,
// FetchFriendRequests, RespondFriendRequest) are not yet implemented in zenth_dto.
// This client provides the structure for when they are available.
// For now, friend management works locally with the database.

use crate::api::errors::{ApiError, ApiResult};
use crate::api::{DarknetType, RegisterConfig};
use serde::{Deserialize, Serialize};

/// Configuration pour le client API d'amis
#[derive(Debug, Clone)]
pub struct FriendConfig {
    pub base_url: String,
    pub darknet: DarknetType,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

impl Default for FriendConfig {
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

impl From<RegisterConfig> for FriendConfig {
    fn from(config: RegisterConfig) -> Self {
        Self {
            base_url: config.base_url,
            darknet: config.darknet,
            timeout_secs: config.timeout_secs,
            max_retries: config.max_retries,
            retry_delay_ms: config.retry_delay_ms,
        }
    }
}

/// Informations publiques d'un utilisateur (retour de lookup)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublicInfo {
    pub username_hash: String,
    pub identity_key_dilithium: Vec<u8>,
    pub kyber_public_key: Vec<u8>,
    pub x25519_public_key: Option<Vec<u8>>,
    pub signed_pre_key: Option<Vec<u8>>,
    pub signed_pre_key_signature: Option<Vec<u8>>,
    pub one_time_pre_key: Option<Vec<u8>>,
}

/// Demande d'ami a envoyer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequestPayload {
    pub from_username_hash: Vec<u8>,
    pub to_username_hash: Vec<u8>,
    pub from_identity_key: Vec<u8>,
    pub from_kyber_public_key: Vec<u8>,
    pub from_x25519_public_key: Option<Vec<u8>>,
    pub from_pseudo: Option<String>,
    pub message: Option<String>,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

/// Reponse a une demande d'ami
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendResponsePayload {
    pub from_username_hash: Vec<u8>,
    pub to_username_hash: Vec<u8>,
    pub accepted: bool,
    pub from_identity_key: Vec<u8>,
    pub from_kyber_public_key: Vec<u8>,
    pub from_x25519_public_key: Option<Vec<u8>>,
    pub from_pseudo: Option<String>,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

/// Demande d'ami recue du serveur
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingFriendRequest {
    pub from_username_hash: String,
    pub from_identity_key: Vec<u8>,
    pub from_kyber_public_key: Vec<u8>,
    pub from_x25519_public_key: Option<Vec<u8>>,
    pub from_pseudo: Option<String>,
    pub message: Option<String>,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

/// Client API pour les operations liees aux amis
///
/// NOTE: Server-side operations are not yet implemented in the protocol.
/// For now, this provides local-only functionality.
pub struct FriendApiClient {
    config: FriendConfig,
}

impl FriendApiClient {
    /// Cree un nouveau client API pour les amis
    pub async fn new(config: FriendConfig) -> ApiResult<Self> {
        if config.base_url.is_empty() {
            return Err(ApiError::InvalidRegistrationData(
                "Base URL cannot be empty".to_string()
            ));
        }

        // NOTE: We don't initialize the transport here since friend methods
        // are not yet implemented in the server protocol.
        // When they are added to zenth_dto, we'll add transport initialization.

        Ok(Self { config })
    }

    /// Recherche un utilisateur par son hash de username
    ///
    /// TODO: This will query the DHT/relay when the protocol supports it.
    /// For now, returns an error indicating the feature is not available.
    pub async fn lookup_user(&self, target_hash: &[u8]) -> ApiResult<UserPublicInfo> {
        // TODO: Implement when Method::LookupUser is added to zenth_dto
        //
        // Future implementation:
        // let dht_request = DhtRequest {
        //     method: Method::LookupUser as i32,
        //     payload: target_hash.to_vec(),
        //     ...
        // };
        // let response = self.send_dht_request(&dht_request).await?;
        // let pre_key_bundle = PreKeyBundle::decode(&response.payload[..])?;
        // return Ok(UserPublicInfo { ... });

        Err(ApiError::Network(
            "User lookup not yet implemented in server protocol. \
             Friend requests currently work with direct key exchange only.".to_string()
        ))
    }

    /// Envoie une demande d'ami
    ///
    /// TODO: This will send through the relay when the protocol supports it.
    /// For now, the request is stored locally and needs manual key exchange.
    pub async fn send_friend_request(&self, _request: FriendRequestPayload) -> ApiResult<()> {
        // TODO: Implement when Method::SendFriendRequest is added to zenth_dto
        //
        // Future implementation:
        // let dht_request = DhtRequest {
        //     method: Method::SendFriendRequest as i32,
        //     payload: bincode::serialize(&request)?,
        //     ...
        // };
        // let response = self.send_dht_request(&dht_request).await?;

        // For now, we just return Ok - the request is stored locally
        // and will be synced when the protocol supports it.
        Ok(())
    }

    /// Recupere les demandes d'ami en attente
    ///
    /// TODO: This will fetch from the relay when the protocol supports it.
    pub async fn fetch_pending_requests(
        &self,
        _user_hash: &[u8],
        _session_token: &[u8],
    ) -> ApiResult<Vec<IncomingFriendRequest>> {
        // TODO: Implement when Method::FetchFriendRequests is added to zenth_dto

        // For now, return empty - requests are managed locally
        Ok(vec![])
    }

    /// Repond a une demande d'ami (accepter ou rejeter)
    ///
    /// TODO: This will send through the relay when the protocol supports it.
    pub async fn respond_to_request(&self, _response: FriendResponsePayload) -> ApiResult<()> {
        // TODO: Implement when Method::RespondFriendRequest is added to zenth_dto

        // For now, we just return Ok - the response is stored locally
        Ok(())
    }

    /// Retourne la configuration
    pub fn config(&self) -> &FriendConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FriendConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_config_from_register() {
        let register_config = RegisterConfig {
            base_url: "http://example.com".to_string(),
            darknet: DarknetType::Http,
            timeout_secs: 60,
            max_retries: 5,
            retry_delay_ms: 2000,
        };
        let friend_config: FriendConfig = register_config.into();
        assert_eq!(friend_config.base_url, "http://example.com");
        assert_eq!(friend_config.timeout_secs, 60);
    }
}
