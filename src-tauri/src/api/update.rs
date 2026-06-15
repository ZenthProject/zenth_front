//! Client de mise à jour via le DHT (METHOD 19 = manifest, METHOD 20 = chunk).
//!
//! NOTE: Les méthodes 19 et 20 et le champ `platform` seront disponibles dans
//! zenth_dto après le prochain push. En attendant, on passe les valeurs brutes.

use prost::Message;
use zenth_dto::{
    DhtRequest, DhtResponse,
    UpdateManifestRequest, UpdateManifestResponse,
    UpdateChunkRequest, UpdateChunkResponse,
};
use zenth_requests::{implementations::RequestsNetwork, request::Request, transports::Transport};
use crate::api::errors::{ApiError, ApiResult, check_http_status};
use crate::utils::timestamp::plateform::current_timestamp;

// Valeurs brutes en attendant le push zenth_dto
const METHOD_GET_UPDATE_MANIFEST: i32 = 19;
const METHOD_GET_UPDATE_CHUNK: i32    = 20;

const CHUNK_SIZE: u32 = 256 * 1024; // 256 Ko par requête

pub struct UpdateApiClient {
    transport: Box<dyn Transport + Send + Sync>,
    base_url:  String,
}

impl UpdateApiClient {
    pub async fn new() -> ApiResult<Self> {
        let transport = RequestsNetwork::new("http")
            .await
            .map_err(|e| ApiError::Network(format!("Transport: {}", e)))?;
        Ok(Self {
            transport,
            base_url: crate::config::dht_api_url(),
        })
    }

    /// Récupère le manifest de la dernière version disponible.
    /// Le champ `current_version` est utilisé pour passer la plateforme
    /// jusqu'au push zenth_dto qui ajoute le champ dédié `platform`.
    pub async fn get_manifest(&self, platform: &str) -> ApiResult<UpdateManifestResponse> {
        let req = UpdateManifestRequest {
            current_version: String::new(),
            platform: platform.to_string(),
        };
        let resp = self.send_raw(METHOD_GET_UPDATE_MANIFEST, &req.encode_to_vec()).await?;
        UpdateManifestResponse::decode(&resp.payload[..])
            .map_err(|e| ApiError::InvalidResponse(format!("UpdateManifestResponse: {}", e)))
    }

    /// Télécharge le binaire en chunks et écrit dans `dest`.
    /// Appelle `on_progress(bytes_received, total)` à chaque chunk.
    pub async fn download_binary(
        &self,
        platform: &str,
        total_size: u64,
        dest: &mut impl std::io::Write,
        on_progress: &mut impl FnMut(u64, u64),
    ) -> ApiResult<()> {
        let mut offset: u64 = 0;

        loop {
            let req = UpdateChunkRequest {
                current_version: platform.to_string(),
                offset,
                chunk_size: CHUNK_SIZE,
            };

            let resp = self.send_raw(METHOD_GET_UPDATE_CHUNK, &req.encode_to_vec()).await?;
            let chunk = UpdateChunkResponse::decode(&resp.payload[..])
                .map_err(|e| ApiError::InvalidResponse(format!("UpdateChunkResponse: {}", e)))?;

            dest.write_all(&chunk.data)
                .map_err(|e| ApiError::Network(format!("Write chunk: {}", e)))?;

            offset += chunk.data.len() as u64;
            on_progress(offset, total_size);

            if chunk.is_last || offset >= total_size {
                break;
            }
        }
        Ok(())
    }

    async fn send_raw(&self, method: i32, payload: &[u8]) -> ApiResult<DhtResponse> {
        let request_id: [u8; 16] = rand::random();
        let dht_request = DhtRequest {
            method,
            payload:    payload.to_vec(),
            timestamp:  current_timestamp(),
            request_id: request_id.to_vec(),
        };
        let mut body = Vec::new();
        dht_request.encode(&mut body)?;

        let req = Request {
            url:     format!("{}/", self.base_url),
            method:  "POST".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/x-protobuf".to_string()),
                ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
            ],
            body: Some(body),
        };

        let response = self.transport.send(req).await
            .map_err(|e| ApiError::Network(format!("Transport: {}", e)))?;

        check_http_status(response.status, &response.body)?;
        DhtResponse::decode(&response.body[..])
            .map_err(|e| ApiError::InvalidResponse(format!("DhtResponse: {}", e)))
    }
}
