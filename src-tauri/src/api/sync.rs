use prost::Message;
use zenth_dto::{
    DhtRequest, DhtResponse, Method,
    SyncPushBlobRequest, SyncPushBlobResponse,
    SyncFetchBlobRequest, SyncFetchBlobResponse,
    SyncDeleteBlobRequest, SyncDeleteBlobResponse,
    RelayPushRequest, RelayPushResponse,
    RelayFetchRequest, RelayFetchResponse,
    RelayAckRequest, RelayAckResponse,
};
use zenth_requests::{implementations::RequestsNetwork, request::Request, transports::Transport};
use crate::api::errors::{ApiError, ApiResult, check_http_status};
use crate::api::register::{RegisterConfig, DarknetType};
use crate::utils::timestamp::plateform::current_timestamp;

/// Blob de Sync Key à publier sur le DHT.
pub struct SyncBlob {
    pub for_device_dilithium_pubkey: Vec<u8>,
    pub ciphertext: Vec<u8>,
    /// Signature E2E du contenu - stockée en DB et renvoyée au client final pour vérification.
    pub signature: Vec<u8>,
    pub ttl_secs: u64,
    /// Clé Dilithium RÉELLE de l'expéditeur (pour l'auth serveur).
    pub sender_dilithium_pubkey: Vec<u8>,
    /// Auth signature = sign(for_device_dilithium_pubkey || ciphertext || timestamp) par sender.
    pub auth_signature: Vec<u8>,
    pub timestamp: u64,
}

pub struct SyncApiClient {
    config: RegisterConfig,
    transport: Box<dyn Transport + Send + Sync>,
}

impl SyncApiClient {
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
            return Err(ApiError::Server(raw.status as u16, dht_response.error_message));
        }

        Resp::decode(&dht_response.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("Réponse payload: {}", e)))
    }

    /// Publie un blob chiffré sur le DHT.
    /// `blob.auth_signature` = sign(for_device_dilithium_pubkey || ciphertext || timestamp) par sender.
    pub async fn push_blob(&self, blob: SyncBlob) -> ApiResult<()> {
        let req = SyncPushBlobRequest {
            for_device_dilithium_pubkey: blob.for_device_dilithium_pubkey,
            ciphertext: blob.ciphertext,
            signature: blob.signature,
            ttl_secs: blob.ttl_secs,
            sender_dilithium_pubkey: blob.sender_dilithium_pubkey,
            timestamp: blob.timestamp,
            auth_signature: blob.auth_signature,
        };
        let resp: SyncPushBlobResponse = self.send_dht(Method::SyncPushBlob, req).await?;
        if resp.success {
            Ok(())
        } else {
            Err(ApiError::Server(400, resp.error_message))
        }
    }

    /// Récupère le blob destiné à `for_device_dilithium_pubkey` depuis le DHT.
    ///
    /// `signature` = sign(for_device_dilithium_pubkey || timestamp) par le requester.
    /// `requester_dilithium_pubkey` = clé réelle du demandeur (vide = même que `for_device`).
    pub async fn fetch_blob(
        &self,
        for_device_dilithium_pubkey: &[u8],
        signature: Vec<u8>,
        timestamp: u64,
        requester_dilithium_pubkey: Vec<u8>,
    ) -> ApiResult<SyncBlob> {
        let req = SyncFetchBlobRequest {
            for_device_dilithium_pubkey: for_device_dilithium_pubkey.to_vec(),
            signature,
            timestamp,
            requester_dilithium_pubkey,
        };
        let resp: SyncFetchBlobResponse = self.send_dht(Method::SyncFetchBlob, req).await?;
        if resp.success {
            Ok(SyncBlob {
                for_device_dilithium_pubkey: for_device_dilithium_pubkey.to_vec(),
                ciphertext: resp.ciphertext,
                signature: resp.signature,
                ttl_secs: 0,
                sender_dilithium_pubkey: vec![],
                auth_signature: vec![],
                timestamp: 0,
            })
        } else {
            Err(ApiError::Server(404, resp.error_message))
        }
    }

    /// Supprime le blob du DHT après déchiffrement réussi.
    ///
    /// `signature` = sign(for_device_dilithium_pubkey || timestamp) par le requester.
    /// `requester_dilithium_pubkey` = clé réelle du demandeur (vide = même que `for_device`).
    pub async fn delete_blob(
        &self,
        for_device_dilithium_pubkey: &[u8],
        signature: Vec<u8>,
        timestamp: u64,
        requester_dilithium_pubkey: Vec<u8>,
    ) -> ApiResult<()> {
        let req = SyncDeleteBlobRequest {
            for_device_dilithium_pubkey: for_device_dilithium_pubkey.to_vec(),
            signature,
            timestamp,
            requester_dilithium_pubkey,
        };
        let resp: SyncDeleteBlobResponse = self.send_dht(Method::SyncDeleteBlob, req).await?;
        if resp.success {
            Ok(())
        } else {
            Err(ApiError::Server(400, resp.error_message))
        }
    }

    /// Pousse un message relay chiffré dans la mailbox DHT du device destinataire.
    ///
    /// `sender_signature` = sign(for_device_dilithium_pubkey || timestamp) par sender.
    pub async fn relay_push(
        &self,
        for_device_dilithium_pubkey: Vec<u8>,
        ciphertext: Vec<u8>,
        nonce: Vec<u8>,
        sender_dilithium_pubkey: Vec<u8>,
        sender_signature: Vec<u8>,
        timestamp: u64,
    ) -> ApiResult<i64> {
        let req = RelayPushRequest {
            for_device_dilithium_pubkey,
            ciphertext,
            nonce,
            ttl_secs: 86400,
            sender_dilithium_pubkey,
            sender_signature,
            timestamp,
        };
        let resp: RelayPushResponse = self.send_dht(Method::RelayPush, req).await?;
        if resp.success {
            Ok(resp.relay_id)
        } else {
            Err(ApiError::Server(400, resp.error_message))
        }
    }

    /// Récupère les messages relay depuis le dernier curseur.
    ///
    /// `signature` = sign(for_device_dilithium_pubkey || since_id || timestamp) par le propriétaire de la mailbox.
    pub async fn relay_fetch(
        &self,
        our_dilithium_pubkey: Vec<u8>,
        since_id: i64,
        signature: Vec<u8>,
        timestamp: u64,
    ) -> ApiResult<RelayFetchResponse> {
        let req = RelayFetchRequest {
            for_device_dilithium_pubkey: our_dilithium_pubkey,
            since_id,
            limit: 50,
            signature,
            timestamp,
        };
        self.send_dht(Method::RelayFetch, req).await
    }

    /// Supprime les entrées relay traitées (up_to_id inclus).
    ///
    /// `signature` = sign(for_device_dilithium_pubkey || up_to_id || timestamp) par le propriétaire.
    pub async fn relay_ack(
        &self,
        our_dilithium_pubkey: Vec<u8>,
        up_to_id: i64,
        signature: Vec<u8>,
        timestamp: u64,
    ) -> ApiResult<()> {
        let req = RelayAckRequest {
            for_device_dilithium_pubkey: our_dilithium_pubkey,
            up_to_id,
            signature,
            timestamp,
        };
        let resp: RelayAckResponse = self.send_dht(Method::RelayAck, req).await?;
        if resp.success { Ok(()) } else { Err(ApiError::Server(400, resp.error_message)) }
    }
}
