use base64::Engine as _;
use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{SecretKey, DetachedSignature};
use prost::Message;
use serde::{Deserialize, Serialize};

use crate::db::{UserDb, MasterDb};
use crate::pages::register::crypto::key::derive_keys_from_password_with_salt;
use crate::utils::security::cipher_key::decrypt_key_with_password;
use crate::session::{get_session_async, get_session_by_token_async};
use crate::api::{FriendConfig, DarknetType, RegisterConfig};
use crate::pages::friends::database::queries::{
    FriendInfo, PendingRequestInfo,
    create_outgoing_request,
    create_incoming_request,
    accept_friend_request as db_accept_friend_request,
    reject_friend_request as db_reject_friend_request,
    add_friend_from_accepted_request,
};
use crate::utils::timestamp::plateform::current_timestamp;
use zenth_dto::{
    FriendRequest as DtoFriendRequest, DhtRequest, DhtResponse, Method,
    PreKeyBundle, IdentityKey, KemPublicKey, SignatureKeyType, KemKeyType,
    FetchFriendRequestsRequest, FetchFriendRequestsResponse,
    FetchFriendResponsesRequest, FetchFriendResponsesResponse,
    FriendResponse as DtoFriendResponse,
};
use zenth_requests::{
    implementations::RequestsNetwork,
    request::Request,
    transports::Transport,
};

/// Informations publiques d'un utilisateur (retour de recherche)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublicInfoResponse {
    pub username_hash: String,
    pub identity_key: String,
    pub kyber_public_key: String,
    pub x25519_public_key: Option<String>,
}

/// Recherche un utilisateur par son hash de username
///
/// NOTE: Server-side user lookup is not yet implemented.
/// This currently validates the hash format only.
/// Full lookup will be available when the DHT protocol supports it.
#[tauri::command]
pub async fn search_user(
    session_token: String,
    target_hash: String,
) -> Result<UserPublicInfoResponse, String> {

    let _session = get_session_by_token_async(session_token).await?;

    if target_hash.len() != 64 {
        return Err("Invalid hash format: must be 64 hex characters (SHA256)".to_string());
    }

    let _target_hash_bytes = hex::decode(&target_hash)
        .map_err(|e| format!("Invalid hash format: {}", e))?;

    Ok(UserPublicInfoResponse {
        username_hash: target_hash.clone(),
        identity_key: "pending".to_string(),
        kyber_public_key: "pending".to_string(),
        x25519_public_key: None,
    })
}

/// Envoie une demande d'ami au serveur DHT
#[tauri::command]
pub async fn send_friend_request(
    session_token: String,
    target_hash: String,
    target_pseudo: Option<String>,
    message: Option<String>,
) -> Result<String, String> {

    // Use cached session - avoids all Argon2id work if session is warm
    let session = get_session_by_token_async(session_token).await?;

    let dilithium_secret = session.dilithium_secret.clone();
    let our_hash_bytes = session.user_hash.clone();
    let identity_key_public = session.identity_key_public.clone();
    let kyber_public_key = session.kyber_public_key.clone();
    let x25519_public_key = session.x25519_public_key.clone();
    let registration_id = session.registration_id;

    let friend_request = tokio::task::spawn_blocking({
        let session = session.clone();
        let target_hash = target_hash.clone();
        let target_pseudo = target_pseudo.clone();
        let message = message.clone();
        move || -> Result<DtoFriendRequest, String> {
            // Check duplicates using cached connection - no Argon2id
            session.with_db(|conn| {
                let already_friend: bool = conn.query_row(
                    "SELECT COUNT(*) FROM friends WHERE username_hash = ?1",
                    [&target_hash],
                    |row| row.get::<_, i64>(0),
                ).map(|c| c > 0).unwrap_or(false);
                if already_friend {
                    return Err("User is already a friend".to_string());
                }

                let pending_exists: bool = conn.query_row(
                    "SELECT COUNT(*) FROM pending_friend_requests WHERE remote_username_hash = ?1 AND status = 'pending'",
                    [&target_hash],
                    |row| row.get::<_, i64>(0),
                ).map(|c| c > 0).unwrap_or(false);
                if pending_exists {
                    return Err("A pending request already exists for this user".to_string());
                }

                Ok(())
            })?;

            let target_hash_bytes = hex::decode(&target_hash)
                .map_err(|e| format!("Invalid target hash format: {}", e))?;

            let timestamp = current_timestamp();

            let pre_key_bundle = create_pre_key_bundle(
                &our_hash_bytes,
                registration_id as u32,
                &identity_key_public,
                &kyber_public_key,
                x25519_public_key.as_deref(),
            )?;

            let mut message_to_sign = Vec::new();
            message_to_sign.extend_from_slice(&target_hash_bytes);
            message_to_sign.extend_from_slice(&pre_key_bundle);
            message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

            let signature = sign_with_dilithium2(&dilithium_secret, &message_to_sign)?;

            // Save locally using cached connection - no Argon2id
            session.with_db(|conn| {
                create_outgoing_request(
                    conn,
                    &target_hash,
                    target_pseudo.clone(),
                    &target_hash_bytes,
                    None,
                    None,
                    &signature,
                    message.clone(),
                ).map(|_| ()).map_err(|e| format!("Failed to save request locally: {}", e))
            })?;

            Ok(DtoFriendRequest {
                requester_hash_id: our_hash_bytes,
                target_hash_id: target_hash_bytes,
                pre_key_bundle,
                dilithium_signature: signature,
                encrypted_message: message.map(|m| m.as_bytes().to_vec()).unwrap_or_default(),
                timestamp,
            })
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    let _ = send_friend_request_to_server(&friend_request).await;

    // Relay vers les appareils jumelés - device B voit la demande sortante
    crate::pages::sync::relay_push_freq(
        session.username.clone(),
        session.password.clone(),
        target_hash.clone(),
        target_pseudo.clone(),
        None,  // identity key de la cible inconnue à ce stade
        None,
        None,
        "outgoing".to_string(),
        friend_request.dilithium_signature.clone(),
        None,
    ).await;

    Ok("Friend request sent successfully!".to_string())
}

async fn send_friend_request_to_server(friend_request: &DtoFriendRequest) -> Result<(), String> {
    let transport = RequestsNetwork::new("http")
        .await
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    // Serialiser la FriendRequest
    let mut request_bytes = Vec::new();
    friend_request.encode(&mut request_bytes)
        .map_err(|e| format!("Failed to encode FriendRequest: {}", e))?;

    // Generer un request_id
    let request_id: [u8; 16] = rand::random();

    // Creer la DhtRequest avec METHOD_CONTACT (6)
    let dht_request = DhtRequest {
        method: Method::Contact as i32,
        payload: request_bytes,
        timestamp: current_timestamp(),
        request_id: request_id.to_vec(),
    };

    // Serialiser la DhtRequest
    let mut dht_request_bytes = Vec::new();
    dht_request.encode(&mut dht_request_bytes)
        .map_err(|e| format!("Failed to encode DhtRequest: {}", e))?;

    // URL du serveur (depuis env ou default)
    let base_url = crate::config::dht_api_url();

    // Creer la requete HTTP
    let req = Request {
        url: format!("{}/", base_url),
        method: "POST".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/x-protobuf".to_string()),
            ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
        ],
        body: Some(dht_request_bytes),
    };

    // Envoyer la requete
    let response = transport
        .send(req)
        .await
        .map_err(|e| format!("Transport send failed: {}", e))?;

    // Decoder la reponse
    let dht_response = DhtResponse::decode(&response.body[..])
        .map_err(|e| format!("Failed to decode DhtResponse: {}", e))?;

    if !dht_response.success {
        return Err(format!("Server error: {}", dht_response.error_message));
    }

    Ok(())
}

async fn fetch_incoming_requests_from_server(
    user_hash: &[u8],
    dilithium_secret: &[u8],
    since_timestamp: u64,
) -> Result<Vec<DtoFriendRequest>, String> {
    let transport = RequestsNetwork::new("http")
        .await
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    let timestamp = current_timestamp();

    let mut message_to_sign = Vec::new();
    message_to_sign.extend_from_slice(user_hash);
    message_to_sign.extend_from_slice(&since_timestamp.to_le_bytes());
    message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

    let signature = sign_with_dilithium2(dilithium_secret, &message_to_sign)?;

    let fetch_request = FetchFriendRequestsRequest {
        user_hash: user_hash.to_vec(),
        session_token: vec![],
        since_timestamp,
        timestamp,
        dilithium_signature: signature,
    };

    let mut request_bytes = Vec::new();
    fetch_request.encode(&mut request_bytes)
        .map_err(|e| format!("Failed to encode FetchFriendRequestsRequest: {}", e))?;

    let request_id: [u8; 16] = rand::random();

    let dht_request = DhtRequest {
        method: Method::FetchFriendRequests as i32,
        payload: request_bytes,
        timestamp,
        request_id: request_id.to_vec(),
    };

    let mut dht_request_bytes = Vec::new();
    dht_request.encode(&mut dht_request_bytes)
        .map_err(|e| format!("Failed to encode DhtRequest: {}", e))?;

    let base_url = crate::config::dht_api_url();

    let req = Request {
        url: format!("{}/", base_url),
        method: "POST".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/x-protobuf".to_string()),
            ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
        ],
        body: Some(dht_request_bytes),
    };

    let response = transport
        .send(req)
        .await
        .map_err(|e| format!("Transport send failed: {}", e))?;

    let dht_response = DhtResponse::decode(&response.body[..])
        .map_err(|e| format!("Failed to decode DhtResponse: {}", e))?;

    if !dht_response.success {
        return Err(format!("Server error: {}", dht_response.error_message));
    }

    let fetch_response = FetchFriendRequestsResponse::decode(&dht_response.payload[..])
        .map_err(|e| format!("Failed to decode FetchFriendRequestsResponse: {}", e))?;

    Ok(fetch_response.requests)
}

async fn send_friend_response_to_server(
    response: &DtoFriendResponse,
) -> Result<(), String> {
    let transport = RequestsNetwork::new("http")
        .await
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    let mut response_bytes = Vec::new();
    response.encode(&mut response_bytes)
        .map_err(|e| format!("Failed to encode FriendResponse: {}", e))?;

    let request_id: [u8; 16] = rand::random();

    let dht_request = DhtRequest {
        method: Method::RespondFriendRequest as i32,
        payload: response_bytes,
        timestamp: current_timestamp(),
        request_id: request_id.to_vec(),
    };

    let mut dht_request_bytes = Vec::new();
    dht_request.encode(&mut dht_request_bytes)
        .map_err(|e| format!("Failed to encode DhtRequest: {}", e))?;

    let base_url = crate::config::dht_api_url();

    let req = Request {
        url: format!("{}/", base_url),
        method: "POST".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/x-protobuf".to_string()),
            ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
        ],
        body: Some(dht_request_bytes),
    };

    let response = transport
        .send(req)
        .await
        .map_err(|e| format!("Transport send failed: {}", e))?;

    let dht_response = DhtResponse::decode(&response.body[..])
        .map_err(|e| format!("Failed to decode DhtResponse: {}", e))?;

    if !dht_response.success {
        return Err(format!("Server error: {}", dht_response.error_message));
    }

    Ok(())
}

/// Synchronise les demandes d'ami avec le serveur (fetch incoming + process responses)
#[tauri::command]
pub async fn sync_friend_requests(
    session_token: String,
) -> Result<SyncResult, String> {

    let session = get_session_by_token_async(session_token).await?;

    let dilithium_secret_bytes = &session.dilithium_secret;
    let user_hash = &session.user_hash;
    let user_hash_hex = &session.user_hash_hex;

    let server_result = fetch_incoming_requests_from_server(
        user_hash,
        dilithium_secret_bytes,
        0,
    ).await;

    let mut new_incoming = 0;
    let mut new_accepted = 0;
    let mut errors: Vec<String> = vec![];

    match server_result {
        Ok(incoming_requests) => {

            for request in incoming_requests {
                let requester_hash = hex::encode(&request.requester_hash_id);

                if requester_hash == *user_hash_hex {
                    continue;
                }

                if let Ok(Some(_)) = session.get_friend_by_hash(&requester_hash) {
                    continue;
                }

                if let Ok(Some(_)) = session.get_pending_request_by_hash(&requester_hash) {
                    continue;
                }

                let pre_key_bundle = PreKeyBundle::decode(&request.pre_key_bundle[..])
                    .map_err(|e| format!("Failed to decode PreKeyBundle: {}", e))?;

                let identity_key = pre_key_bundle.identity_key
                    .map(|ik| ik.public_key)
                    .unwrap_or_default();

                let kyber_key = pre_key_bundle.pq_pre_key_public
                    .map(|k| k.public_key);

                let x25519_key = if pre_key_bundle.pre_key_public.is_empty() {
                    None
                } else {
                    Some(pre_key_bundle.pre_key_public)
                };

                let message = if request.encrypted_message.is_empty() {
                    None
                } else {
                    String::from_utf8(request.encrypted_message).ok()
                };

                let user_db = session.get_user_db()
                    .map_err(|e| format!("DB open failed: {}", e))?;

                match create_incoming_request(
                    &user_db,
                    &requester_hash,
                    None,
                    &identity_key,
                    kyber_key.clone(),
                    x25519_key.clone(),
                    &request.dilithium_signature,
                    message.clone(),
                ) {
                    Ok(_) => {
                        new_incoming += 1;
                        // Relay la demande entrante vers les appareils jumelés
                        crate::pages::sync::relay_push_freq(
                            session.username.clone(),
                            session.password.clone(),
                            requester_hash.clone(),
                            None,
                            Some(identity_key.clone()),
                            kyber_key,
                            x25519_key,
                            "incoming".to_string(),
                            request.dilithium_signature.clone(),
                            message,
                        ).await;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to save request from {}: {}", &requester_hash[..8], e));
                    }
                }
            }
        }
        Err(e) => {
            errors.push(format!("Server fetch failed: {}", e));
        }
    }

    let _ = session.cleanup_expired_requests();

    Ok(SyncResult {
        new_incoming,
        new_accepted,
        errors,
    })
}

/// Resultat de la synchronisation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub new_incoming: u32,
    pub new_accepted: u32,
    pub errors: Vec<String>,
}

/// Liste les demandes d'ami en attente
#[tauri::command]
pub async fn list_pending_requests(
    session_token: String,
) -> Result<Vec<PendingRequestInfo>, String> {

    let session = get_session_by_token_async(session_token).await?;

    let _ = session.cleanup_expired_requests();

    let requests = session.list_pending_requests(None)
        .map_err(|e| format!("Failed to list requests: {}", e))?;

    let result: Vec<PendingRequestInfo> = requests.into_iter().map(Into::into).collect();
    Ok(result)
}

#[tauri::command]
pub async fn accept_friend_request(
    session_token: String,
    requester_hash: String,
    pseudo: Option<String>,
) -> Result<String, String> {

    // Use cached session - avoids all Argon2id work if session is warm
    let session = get_session_by_token_async(session_token).await?;

    let dilithium_secret = session.dilithium_secret.clone();
    let our_hash_bytes = session.user_hash.clone();
    let identity_key_public = session.identity_key_public.clone();
    let kyber_public_key = session.kyber_public_key.clone();
    let x25519_public_key = session.x25519_public_key.clone();
    let registration_id = session.registration_id;

    let friend_response = tokio::task::spawn_blocking({
        let session = session.clone();
        let requester_hash = requester_hash.clone();
        move || -> Result<DtoFriendResponse, String> {
            let remote_hash_bytes = hex::decode(&requester_hash)
                .map_err(|e| format!("Failed to decode remote hash: {}", e))?;

            let mut friendship_message = Vec::new();
            friendship_message.extend_from_slice(b"FRIENDSHIP:");
            friendship_message.extend_from_slice(&our_hash_bytes);
            friendship_message.extend_from_slice(&remote_hash_bytes);

            let friendship_signature = sign_with_dilithium2(&dilithium_secret, &friendship_message)?;

            // DB write using cached connection - no Argon2id
            session.with_db(|conn| {
                db_accept_friend_request(conn, &requester_hash, Some(friendship_signature.clone()), None, pseudo)
                    .map_err(|e| format!("Failed to accept request locally: {}", e))
            })?;

            let timestamp = current_timestamp();

            let pre_key_bundle = create_pre_key_bundle(
                &our_hash_bytes,
                registration_id as u32,
                &identity_key_public,
                &kyber_public_key,
                x25519_public_key.as_deref(),
            )?;

            Ok(DtoFriendResponse {
                responder_hash_id: our_hash_bytes,
                requester_hash_id: remote_hash_bytes,
                accepted: true,
                pre_key_bundle,
                dilithium_signature: friendship_signature,
                timestamp,
            })
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    let _ = send_friend_response_to_server(&friend_response).await;
    session.invalidate_friends_cache();

    // Relay le contact confirmé vers les appareils jumelés
    // On relit le contact depuis la DB pour avoir les clés complètes
    if let Ok(Some(friend)) = session.get_friend_by_hash(&requester_hash) {
        crate::pages::sync::relay_push_friend(
            session.username.clone(),
            session.password.clone(),
            friend.username_hash.clone(),
            friend.pseudo.clone(),
            friend.identity_key_public.clone(),
            friend.kyber_public_key.clone(),
            friend.x25519_public_key.clone(),
            friend.friendship_signature_local.clone(),
            friend.friendship_signature_remote.clone(),
        ).await;
    }

    Ok("Friend request accepted".to_string())
}

/// Rejette une demande d'ami
#[tauri::command]
pub async fn reject_friend_request(
    session_token: String,
    requester_hash: String,
) -> Result<String, String> {

    // Use cached session - avoids all Argon2id work if session is warm
    let session = get_session_by_token_async(session_token).await?;

    let dilithium_secret = session.dilithium_secret.clone();
    let our_hash_bytes = session.user_hash.clone();

    let friend_response = tokio::task::spawn_blocking({
        let session = session.clone();
        let requester_hash = requester_hash.clone();
        move || -> Result<DtoFriendResponse, String> {
            let requester_hash_bytes = hex::decode(&requester_hash)
                .map_err(|e| format!("Failed to decode requester hash: {}", e))?;

            let timestamp = current_timestamp();

            let mut message_to_sign = Vec::new();
            message_to_sign.extend_from_slice(&requester_hash_bytes);
            message_to_sign.push(0u8);
            message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

            let signature = sign_with_dilithium2(&dilithium_secret, &message_to_sign)?;

            // DB write using cached connection - no Argon2id
            session.with_db(|conn| {
                db_reject_friend_request(conn, &requester_hash)
                    .map_err(|e| format!("Failed to reject request: {}", e))
            })?;

            Ok(DtoFriendResponse {
                responder_hash_id: our_hash_bytes,
                requester_hash_id: requester_hash_bytes,
                accepted: false,
                pre_key_bundle: vec![],
                dilithium_signature: signature,
                timestamp,
            })
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    let _ = send_friend_response_to_server(&friend_response).await;
    Ok("Friend request rejected".to_string())
}

#[tauri::command]
pub async fn list_friends(
    session_token: String,
) -> Result<Vec<FriendInfo>, String> {

    let session = get_session_by_token_async(session_token).await?;

    let friends = session.list_friends()
        .map_err(|e| format!("Failed to list friends: {}", e))?;

    let result: Vec<FriendInfo> = friends.into_iter().map(Into::into).collect();

    Ok(result)
}

#[tauri::command]
pub async fn remove_friend(
    session_token: String,
    friend_id: i64,
) -> Result<String, String> {
    let session = get_session_by_token_async(session_token).await?;

    // Récupère le hash avant suppression pour nettoyer les pending requests
    let username_hash: Option<String> = session.with_db(|conn| {
        conn.query_row(
            "SELECT username_hash FROM friends WHERE id = ?1",
            rusqlite::params![friend_id],
            |row| row.get::<_, String>(0),
        ).map_err(|e| format!("Friend not found: {}", e))
    }).ok();

    session.remove_friend(friend_id)
        .map_err(|e| format!("Failed to remove friend: {}", e))?;

    // Nettoie toutes les demandes liées à cet utilisateur (accepted, rejected, pending)
    if let Some(hash) = &username_hash {
        let _ = session.delete_pending_request(hash);
    }

    // Notifie le serveur (best-effort) pour que la demande d'ami soit retirée côté serveur
    // et ne revienne pas lors du prochain sync.
    if let Some(hash) = username_hash {
        let dilithium_secret = session.dilithium_secret.clone();
        let our_hash_bytes = session.user_hash.clone();
        tokio::spawn(async move {
            let join = tokio::task::spawn_blocking(move || -> Result<DtoFriendResponse, String> {
                let requester_hash_bytes = hex::decode(&hash)
                    .map_err(|e| format!("decode hash: {}", e))?;
                let timestamp = current_timestamp();
                let mut msg = Vec::new();
                msg.extend_from_slice(&requester_hash_bytes);
                msg.push(0u8);
                msg.extend_from_slice(&timestamp.to_le_bytes());
                let signature = sign_with_dilithium2(&dilithium_secret, &msg)?;
                Ok(DtoFriendResponse {
                    responder_hash_id: our_hash_bytes,
                    requester_hash_id: requester_hash_bytes,
                    accepted: false,
                    pre_key_bundle: vec![],
                    dilithium_signature: signature,
                    timestamp,
                })
            }).await;

            if let Ok(Ok(response)) = join {
                let _ = send_friend_response_to_server(&response).await;
            }
        });
    }

    session.invalidate_friends_cache();
    Ok("Friend removed successfully".to_string())
}

#[tauri::command]
pub async fn get_my_public_key(
    session_token: String,
) -> Result<String, String> {
    let session = get_session_by_token_async(session_token).await?;
    Ok(session.user_hash_hex.clone())
}

/// Synchronise les reponses aux demandes d'ami envoyees
/// Recupere les acceptations/rejets des demandes que nous avons envoyees
#[tauri::command]
pub async fn sync_friend_responses(
    session_token: String,
) -> Result<SyncResult, String> {

    let session = get_session_by_token_async(session_token).await?;

    let dilithium_secret_bytes = &session.dilithium_secret;
    let user_hash = &session.user_hash;

    let since_ts: u64 = session
        .get_setting("last_response_sync_ts")
        .unwrap_or(None)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let server_result = fetch_friend_responses_from_server(
        user_hash,
        dilithium_secret_bytes,
        since_ts,
    ).await;

    let mut new_incoming = 0u32;
    let mut new_accepted = 0u32;
    let mut errors: Vec<String> = vec![];

    match server_result {
        Ok(responses) => {

            for response in responses {
                let responder_hash = hex::encode(&response.responder_hash_id);

                let pending_opt = match session.get_pending_request_by_hash(&responder_hash) {
                    Ok(Some(p)) if p.direction == "outgoing" => Some(p),
                    Ok(Some(_)) => continue,
                    Ok(None) => None, // Device B: pas de demande locale, on traite quand même
                    Err(e) => {
                        errors.push(format!("DB error: {}", e));
                        continue;
                    }
                };

                if response.accepted {

                    // Decoder le PreKeyBundle pour extraire les cles
                    let pre_key_bundle = PreKeyBundle::decode(&response.pre_key_bundle[..])
                        .map_err(|e| format!("Failed to decode PreKeyBundle: {}", e))?;

                    let identity_key = pre_key_bundle.identity_key
                        .map(|ik| ik.public_key)
                        .unwrap_or_default();

                    let kyber_key = pre_key_bundle.pq_pre_key_public
                        .map(|k| k.public_key);

                    let x25519_key = if pre_key_bundle.pre_key_public.is_empty() {
                        None
                    } else {
                        Some(pre_key_bundle.pre_key_public)
                    };

                    let remote_pseudo = pending_opt.as_ref().and_then(|p| p.remote_pseudo.clone());

                    let user_db = session.get_user_db()
                        .map_err(|e| format!("DB open failed: {}", e))?;

                    let add_result = add_friend_from_accepted_request(
                        &user_db,
                        &responder_hash,
                        remote_pseudo.clone(),
                        &identity_key,
                        kyber_key.clone(),
                        x25519_key.clone(),
                        None,
                        Some(response.dilithium_signature.clone()),
                    );

                    let success = match add_result {
                        Ok(_) => true,
                        Err(ref e) if e.to_string().to_lowercase().contains("unique") => {
                            // Ami déjà présent - pas une nouveauté, on nettoie juste le pending
                            let _ = session.delete_pending_request(&responder_hash);
                            false
                        }
                        Err(e) => {
                            errors.push(format!("Failed to add friend: {}", e));
                            false
                        }
                    };

                    if success {
                        new_accepted += 1;
                        session.invalidate_friends_cache();
                        // Relay le contact confirmé vers les appareils jumelés
                        crate::pages::sync::relay_push_friend(
                            session.username.clone(),
                            session.password.clone(),
                            responder_hash.clone(),
                            remote_pseudo.unwrap_or_default(),
                            identity_key.clone(),
                            kyber_key,
                            x25519_key,
                            None,
                            Some(response.dilithium_signature.clone()),
                        ).await;
                    }
                } else {
                    // Demande rejetee - supprimer la demande (use cached connection)
                    let _ = session.delete_pending_request(&responder_hash);
                }
            }
        }
        Err(e) => {
            errors.push(format!("Server fetch failed: {}", e));
        }
    }

    // Mémoriser le timestamp pour ne pas retraiter les mêmes réponses au prochain sync
    let now_ts = crate::utils::timestamp::plateform::current_timestamp().to_string();
    let _ = session.set_setting("last_response_sync_ts", &now_ts);

    Ok(SyncResult {
        new_incoming,
        new_accepted,
        errors,
    })
}

/// Fetch les reponses aux demandes d'ami depuis le serveur DHT
async fn fetch_friend_responses_from_server(
    user_hash: &[u8],
    dilithium_secret: &[u8],
    since_timestamp: u64,
) -> Result<Vec<DtoFriendResponse>, String> {
    let transport = RequestsNetwork::new("http")
        .await
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    let timestamp = current_timestamp();

    // Creer le message a signer: user_hash || since_timestamp || timestamp
    let mut message_to_sign = Vec::new();
    message_to_sign.extend_from_slice(user_hash);
    message_to_sign.extend_from_slice(&since_timestamp.to_le_bytes());
    message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

    let signature = sign_with_dilithium2(dilithium_secret, &message_to_sign)?;

    let fetch_request = FetchFriendResponsesRequest {
        user_hash: user_hash.to_vec(),
        since_timestamp,
        timestamp,
        dilithium_signature: signature,
    };

    let mut request_bytes = Vec::new();
    fetch_request.encode(&mut request_bytes)
        .map_err(|e| format!("Failed to encode FetchFriendResponsesRequest: {}", e))?;

    let request_id: [u8; 16] = rand::random();

    // Utiliser la methode FETCH_FRIEND_RESPONSES (valeur 14, pas encore dans l'enum proto)
    const METHOD_FETCH_FRIEND_RESPONSES: i32 = 14;
    let dht_request = DhtRequest {
        method: METHOD_FETCH_FRIEND_RESPONSES,
        payload: request_bytes,
        timestamp,
        request_id: request_id.to_vec(),
    };

    let mut dht_request_bytes = Vec::new();
    dht_request.encode(&mut dht_request_bytes)
        .map_err(|e| format!("Failed to encode DhtRequest: {}", e))?;

    let base_url = crate::config::dht_api_url();

    let req = Request {
        url: format!("{}/", base_url),
        method: "POST".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/x-protobuf".to_string()),
            ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
        ],
        body: Some(dht_request_bytes),
    };

    let response = transport
        .send(req)
        .await
        .map_err(|e| format!("Transport send failed: {}", e))?;

    let dht_response = DhtResponse::decode(&response.body[..])
        .map_err(|e| format!("Failed to decode DhtResponse: {}", e))?;

    if !dht_response.success {
        return Err(format!("Server error: {}", dht_response.error_message));
    }

    let fetch_response = FetchFriendResponsesResponse::decode(&dht_response.payload[..])
        .map_err(|e| format!("Failed to decode FetchFriendResponsesResponse: {}", e))?;

    Ok(fetch_response.responses)
}

/// Signe un message avec Dilithium2 (compatible avec le backend)
fn sign_with_dilithium2(secret_key_bytes: &[u8], message: &[u8]) -> Result<Vec<u8>, String> {
    let secret_key = dilithium2::SecretKey::from_bytes(secret_key_bytes)
        .map_err(|_| "Invalid Dilithium2 secret key format")?;

    let signature = dilithium2::detached_sign(message, &secret_key);

    Ok(signature.as_bytes().to_vec())
}

/// Cree un PreKeyBundle serialise a partir des donnees de l'utilisateur
fn create_pre_key_bundle(
    user_hash_id: &[u8],
    registration_id: u32,
    identity_key_public: &[u8],
    kyber_public_key: &[u8],
    x25519_public_key: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    // Creer le PreKeyBundle
    let pre_key_bundle = PreKeyBundle {
        user_hash_id: user_hash_id.to_vec(),
        registration_id,
        identity_key: Some(IdentityKey {
            key_type: SignatureKeyType::Dilithium2 as i32,
            public_key: identity_key_public.to_vec(),
        }),
        // Les pre-keys sont optionnelles pour une demande d'ami
        pre_key_id: 0,
        pre_key_public: x25519_public_key.map(|k| k.to_vec()).unwrap_or_default(),
        signed_pre_key_id: 0,
        signed_pre_key_public: vec![],
        signed_pre_key_signature: vec![],
        pq_pre_key_id: 0,
        pq_pre_key_public: Some(KemPublicKey {
            key_type: KemKeyType::Kyber1024 as i32,
            public_key: kyber_public_key.to_vec(),
        }),
        pq_last_resort_key_id: 0,
        pq_last_resort_key_public: None,
    };

    // Serialiser le PreKeyBundle
    let mut buf = Vec::new();
    pre_key_bundle.encode(&mut buf)
        .map_err(|e| format!("Failed to encode PreKeyBundle: {}", e))?;

    Ok(buf)
}

/// Bloque un ami (le masque de la liste mais conserve les données)
#[tauri::command]
pub async fn block_friend(
    session_token: String,
    friend_id: i64,
) -> Result<(), String> {
    let session = get_session_by_token_async(session_token).await?;

    // Récupère le hash avant de bloquer pour nettoyer les pending_requests
    let username_hash: Option<String> = session.with_db(|conn| {
        conn.query_row(
            "SELECT username_hash FROM friends WHERE id = ?1",
            rusqlite::params![friend_id],
            |row| row.get::<_, String>(0),
        ).map_err(|e| format!("Friend not found: {}", e))
    }).ok();

    session.with_db(|conn| {
        conn.execute("UPDATE friends SET blocked = 1 WHERE id = ?1", [friend_id])
            .map_err(|e| format!("Failed to block friend: {}", e))?;
        Ok(())
    })?;

    // Supprime toute demande pending de cet utilisateur pour qu'elle ne reste pas visible
    if let Some(hash) = username_hash {
        let _ = session.delete_pending_request(&hash);
    }

    session.invalidate_friends_cache();
    Ok(())
}

/// Débloque un ami
#[tauri::command]
pub async fn unblock_friend(
    session_token: String,
    friend_id: i64,
) -> Result<(), String> {
    let session = get_session_by_token_async(session_token).await?;
    session.with_db(|conn| {
        conn.execute("UPDATE friends SET blocked = 0 WHERE id = ?1", [friend_id])
            .map_err(|e| format!("Failed to unblock friend: {}", e))?;
        Ok(())
    })?;
    session.invalidate_friends_cache();
    Ok(())
}

/// Liste les amis bloqués
#[tauri::command]
pub async fn list_blocked_friends(
    session_token: String,
) -> Result<Vec<FriendInfo>, String> {
    let session = get_session_by_token_async(session_token).await?;
    let friends = session.with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                    x25519_public_key, friendship_signature_local, friendship_signature_remote,
                    verified, blocked, created_at, updated_at, avatar
             FROM friends WHERE blocked = 1 ORDER BY pseudo"
        ).map_err(|e| format!("Prepare error: {}", e))?;

        let rows = stmt.query_map([], |row| {
            Ok(crate::db::user::Friend {
                id: row.get(0)?,
                pseudo: row.get(1)?,
                username_hash: row.get(2)?,
                identity_key_public: row.get(3)?,
                kyber_public_key: row.get(4)?,
                x25519_public_key: row.get(5)?,
                friendship_signature_local: row.get(6)?,
                friendship_signature_remote: row.get(7)?,
                verified: row.get(8)?,
                blocked: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
                avatar: row.get(12)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;

        let mut result = Vec::new();
        for f in rows {
            result.push(f.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    })?;
    Ok(friends.into_iter().map(Into::into).collect())
}

/// Définit l'avatar de l'utilisateur (BLOB brut depuis le frontend en base64)
#[tauri::command]
pub async fn set_my_avatar(
    session_token: String,
    avatar_b64: String,
) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&avatar_b64)
        .map_err(|e| format!("Invalid base64: {}", e))?;
    let session = get_session_by_token_async(session_token).await?;
    let db = session.get_user_db()?;
    db.set_my_avatar(&bytes)
        .map_err(|e| format!("Failed to save avatar: {}", e))
}

/// Récupère l'avatar de l'utilisateur (retourné en base64)
#[tauri::command]
pub async fn get_my_avatar(
    session_token: String,
) -> Result<Option<String>, String> {
    let session = get_session_by_token_async(session_token).await?;
    let db = session.get_user_db()?;
    let avatar = db.get_my_avatar()
        .map_err(|e| format!("Failed to get avatar: {}", e))?;
    Ok(avatar.map(|b| base64::engine::general_purpose::STANDARD.encode(&b)))
}

/// Définit l'avatar d'un ami (BLOB brut depuis le frontend en base64)
#[tauri::command]
pub async fn set_friend_avatar(
    session_token: String,
    friend_id: i64,
    avatar_b64: String,
) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&avatar_b64)
        .map_err(|e| format!("Invalid base64: {}", e))?;
    let session = get_session_by_token_async(session_token).await?;
    session.with_db(|conn| {
        let now = current_timestamp() as i64;
        conn.execute(
            "UPDATE friends SET avatar = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![bytes, now, friend_id],
        ).map_err(|e| format!("Failed to save friend avatar: {}", e))?;
        Ok(())
    })?;
    session.invalidate_friends_cache();
    Ok(())
}

#[tauri::command]
pub async fn rename_friend(
    session_token: String,
    friend_id: i64,
    new_pseudo: String,
) -> Result<(), String> {
    let pseudo = new_pseudo.trim().to_string();
    if pseudo.is_empty() {
        return Err("Le nom ne peut pas être vide".to_string());
    }
    let session = get_session_by_token_async(session_token).await?;
    session.with_db(|conn| -> Result<(), String> {
        let now = crate::utils::timestamp::plateform::current_timestamp() as i64;
        conn.execute(
            "UPDATE friends SET pseudo = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![pseudo, now, friend_id],
        ).map_err(|e| format!("Failed to rename friend: {}", e))?;
        Ok(())
    })?;
    session.invalidate_friends_cache();
    Ok(())
}

/// Calcule le fingerprint d'amitié (safety number) entre l'utilisateur et un ami.
/// Les deux parties obtiennent le même résultat en triant les clés avant de hacher.
#[tauri::command]
pub async fn get_friend_fingerprint(
    session_token: String,
    friend_id: i64,
) -> Result<String, String> {
    let session = get_session_by_token_async(session_token).await?;

    let friend = session.get_friend(friend_id)
        .map_err(|e| format!("Friend not found: {}", e))?;

    use sha2::{Sha256, Digest};

    // user_hash = SHA256²(username) calculé à l'inscription (generate_username_hash, 2 passes)
    // Même valeur sur tous les appareils du compte car déterministe et sans sel
    let our_id: Vec<u8> = session.user_hash.clone();

    // username_hash de l'ami : même algorithme SHA256², reçu du serveur
    let friend_id_bytes = hex::decode(&friend.username_hash)
        .map_err(|e| format!("Invalid friend hash: {}", e))?;

    let (first, second) = if our_id.as_slice() < friend_id_bytes.as_slice() {
        (our_id, friend_id_bytes)
    } else {
        (friend_id_bytes, our_id)
    };

    let mut hasher = Sha256::new();
    hasher.update(b"ZENTH_FINGERPRINT_V1");
    hasher.update(&first);
    hasher.update(&second);
    let hash = hasher.finalize();

    // 12 groupes de 5 chiffres, lisibles à voix haute
    let digits: String = hash.iter()
        .flat_map(|b| [b / 100, (b / 10) % 10, b % 10])
        .take(60)
        .map(|d| d.to_string())
        .collect();

    let groups: Vec<String> = digits
        .as_bytes()
        .chunks(5)
        .map(|c| std::str::from_utf8(c).unwrap_or("").to_string())
        .collect();

    Ok(groups.join(" "))
}

/// Marque un ami comme vérifié (après comparaison du fingerprint hors-bande).
#[tauri::command]
pub async fn mark_friend_verified(
    session_token: String,
    friend_id: i64,
) -> Result<(), String> {
    let session = get_session_by_token_async(session_token).await?;
    session.with_db(|conn| {
        conn.execute(
            "UPDATE friends SET verified = 1 WHERE id = ?1",
            [friend_id],
        ).map_err(|e| format!("Failed to mark verified: {}", e))?;
        Ok(())
    })?;
    session.invalidate_friends_cache();
    Ok(())
}

/// Annule une demande d'ami sortante
///
/// Permet de supprimer une demande sortante qui a échoué ou qu'on souhaite annuler.
/// Utile quand l'envoi au serveur a échoué et qu'on veut réessayer.
#[tauri::command]
pub async fn cancel_friend_request(
    session_token: String,
    target_hash: String,
) -> Result<String, String> {

    let session = get_session_by_token_async(session_token).await?;

    let request = match session.get_pending_request_by_hash(&target_hash) {
        Ok(Some(r)) => r,
        Ok(None) => return Err("No pending request found for this user".to_string()),
        Err(e) => return Err(format!("Database error: {}", e)),
    };

    if request.direction != "outgoing" {
        return Err("Can only cancel outgoing requests".to_string());
    }

    session.delete_pending_request(&target_hash)
        .map_err(|e| format!("Failed to delete request: {}", e))?;

    Ok("Friend request cancelled".to_string())
}

/// Réessaie d'envoyer une demande d'ami sortante existante
///
/// Utile quand la demande a été sauvegardée localement mais l'envoi au serveur a échoué.
#[tauri::command]
pub async fn retry_friend_request(
    session_token: String,
    target_hash: String,
) -> Result<String, String> {

    let session = get_session_by_token_async(session_token).await?;

    let request = match session.get_pending_request_by_hash(&target_hash) {
        Ok(Some(r)) => r,
        Ok(None) => return Err("No pending request found for this user".to_string()),
        Err(e) => return Err(format!("Database error: {}", e)),
    };
    if request.direction != "outgoing" {
        return Err("Can only retry outgoing requests".to_string());
    }

    let dilithium_secret = session.dilithium_secret.clone();
    let our_hash_bytes = session.user_hash.clone();
    let identity_key_public = session.identity_key_public.clone();
    let kyber_public_key = session.kyber_public_key.clone();
    let x25519_public_key = session.x25519_public_key.clone();
    let registration_id = session.registration_id;

    let friend_request = tokio::task::spawn_blocking({
        let target_hash = target_hash.clone();
        move || -> Result<DtoFriendRequest, String> {
            let target_hash_bytes = hex::decode(&target_hash)
                .map_err(|e| format!("Invalid target hash format: {}", e))?;
            let timestamp = current_timestamp();

            let pre_key_bundle = create_pre_key_bundle(
                &our_hash_bytes,
                registration_id as u32,
                &identity_key_public,
                &kyber_public_key,
                x25519_public_key.as_deref(),
            )?;

            let mut message_to_sign = Vec::new();
            message_to_sign.extend_from_slice(&target_hash_bytes);
            message_to_sign.extend_from_slice(&pre_key_bundle);
            message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

            let signature = sign_with_dilithium2(&dilithium_secret, &message_to_sign)?;

            Ok(DtoFriendRequest {
                requester_hash_id: our_hash_bytes,
                target_hash_id: target_hash_bytes,
                pre_key_bundle,
                dilithium_signature: signature,
                encrypted_message: request.message.as_ref().map(|m| m.as_bytes().to_vec()).unwrap_or_default(),
                timestamp,
            })
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    // Phase 2 (async): HTTP
    send_friend_request_to_server(&friend_request).await?;

    Ok("Friend request sent successfully!".to_string())
}

/// Synchronise les contacts acceptés côté "responder" (demandes REÇUES et acceptées).
///
/// Cas non couvert par sync_friend_responses : quand un autre appareil appelle
/// sync_friend_requests, il voit la demande encore en attente sur le DHT et la remet
/// en "pending" au lieu de retrouver l'ami. Ce sync récupère directement les contacts
/// qu'on a acceptés (nous sommes responder) depuis le DHT (METHOD 27).
#[tauri::command]
pub async fn sync_accepted_contacts(
    session_token: String,
) -> Result<SyncResult, String> {
    let session = get_session_by_token_async(session_token).await?;

    let dilithium_secret_bytes = &session.dilithium_secret;
    let user_hash = &session.user_hash;

    let since_ts: u64 = session
        .get_setting("last_accepted_sync_ts")
        .unwrap_or(None)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let responses = fetch_my_accepted_from_server(user_hash, dilithium_secret_bytes, since_ts).await
        .unwrap_or_default();

    let mut new_accepted = 0u32;
    let mut errors: Vec<String> = vec![];

    for response in responses {
        let requester_hash = hex::encode(&response.requester_hash_id);

        // Déjà présent en tant qu'ami → skip
        if let Ok(Some(_)) = session.get_friend_by_hash(&requester_hash) {
            continue;
        }

        let pre_key_bundle = match PreKeyBundle::decode(&response.pre_key_bundle[..]) {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("PreKeyBundle invalide pour {}: {}", &requester_hash[..8], e));
                continue;
            }
        };

        let identity_key = pre_key_bundle.identity_key
            .map(|ik| ik.public_key)
            .unwrap_or_default();

        let kyber_key = pre_key_bundle.pq_pre_key_public.map(|k| k.public_key);
        let x25519_key = if pre_key_bundle.pre_key_public.is_empty() {
            None
        } else {
            Some(pre_key_bundle.pre_key_public)
        };

        let user_db = match session.get_user_db() {
            Ok(db) => db,
            Err(e) => { errors.push(format!("DB: {}", e)); continue; }
        };

        match add_friend_from_accepted_request(
            &user_db,
            &requester_hash,
            None,
            &identity_key,
            kyber_key,
            x25519_key,
            None,
            Some(response.dilithium_signature.clone()),
        ) {
            Ok(_) => {
                new_accepted += 1;
                session.invalidate_friends_cache();
            }
            Err(ref e) if e.to_string().to_lowercase().contains("unique") => {
                // Déjà présent - nettoie juste le pending si besoin
                let _ = session.delete_pending_request(&requester_hash);
            }
            Err(e) => {
                errors.push(format!("Ajout ami échoué ({}): {}", &requester_hash[..8], e));
            }
        }

        // Nettoie la demande pendante si elle existe encore localement
        let _ = session.delete_pending_request(&requester_hash);
    }

    let now_ts = current_timestamp().to_string();
    let _ = session.set_setting("last_accepted_sync_ts", &now_ts);

    Ok(SyncResult { new_incoming: 0, new_accepted, errors })
}

async fn fetch_my_accepted_from_server(
    user_hash: &[u8],
    dilithium_secret: &[u8],
    since_timestamp: u64,
) -> Result<Vec<DtoFriendResponse>, String> {
    use zenth_dto::FetchFriendResponsesRequest;
    let transport = RequestsNetwork::new("http")
        .await
        .map_err(|e| format!("Transport: {}", e))?;

    let timestamp = current_timestamp();
    let mut msg = Vec::new();
    msg.extend_from_slice(user_hash);
    msg.extend_from_slice(&since_timestamp.to_le_bytes());
    msg.extend_from_slice(&timestamp.to_le_bytes());

    let signature = sign_with_dilithium2(dilithium_secret, &msg)?;

    let req = FetchFriendResponsesRequest {
        user_hash: user_hash.to_vec(),
        since_timestamp,
        timestamp,
        dilithium_signature: signature,
    };

    let mut req_bytes = Vec::new();
    req.encode(&mut req_bytes)
        .map_err(|e| format!("Encode: {}", e))?;

    let request_id: [u8; 16] = rand::random();
    let dht_request = zenth_dto::DhtRequest {
        method: 27, // METHOD_FETCH_MY_ACCEPTED
        payload: req_bytes,
        timestamp,
        request_id: request_id.to_vec(),
    };

    let mut dht_bytes = Vec::new();
    dht_request.encode(&mut dht_bytes)
        .map_err(|e| format!("Encode DhtRequest: {}", e))?;

    let base_url = crate::config::dht_api_url();
    let http_req = zenth_requests::request::Request {
        url: format!("{}/", base_url),
        method: "POST".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/x-protobuf".to_string()),
            ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
        ],
        body: Some(dht_bytes),
    };

    use zenth_requests::transports::Transport;
    let raw = transport.send(http_req).await
        .map_err(|e| format!("Transport: {}", e))?;

    let dht_resp = zenth_dto::DhtResponse::decode(&raw.body[..])
        .map_err(|e| format!("DhtResponse decode: {}", e))?;

    if !dht_resp.success {
        return Ok(vec![]); // Aucun contact accepté ou erreur non bloquante
    }

    use zenth_dto::FetchFriendResponsesResponse;
    let resp = FetchFriendResponsesResponse::decode(&dht_resp.payload[..])
        .map_err(|e| format!("FetchFriendResponsesResponse decode: {}", e))?;

    Ok(resp.responses)
}

// Conversation lock (PIN)
/// Pose un verrou PIN sur une conversation (la cache de la liste).
/// Le PIN est haché avec Argon2id avant stockage.
#[tauri::command]
pub async fn lock_conversation(
    session_token: String,
    friend_id: i64,
    pin: String,
) -> Result<(), String> {
    use argon2::{password_hash::{rand_core::OsRng, PasswordHasher, SaltString}, Argon2};

    let pin_hash = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(pin.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| format!("Hash error: {}", e))
    }).await.map_err(|e| format!("Task: {}", e))??;

    let session = get_session_by_token_async(session_token).await?;
    session.with_db(|conn| {
        let updated = conn.execute(
            "UPDATE friends SET pin_hash = ?1 WHERE id = ?2 AND blocked = 0 AND pin_hash IS NULL",
            rusqlite::params![pin_hash, friend_id],
        ).map_err(|e| format!("DB: {}", e))?;
        if updated == 0 {
            return Err("Conversation introuvable ou déjà verrouillée".to_string());
        }
        Ok(())
    })?;
    session.invalidate_friends_cache();
    Ok(())
}

/// Vérifie un PIN contre toutes les convs verrouillées.
/// Retourne les FriendInfo des convs dont le PIN correspond.
#[tauri::command]
pub async fn check_conversation_pin(
    session_token: String,
    pin: String,
) -> Result<Vec<FriendInfo>, String> {
    use argon2::{password_hash::{PasswordHash, PasswordVerifier}, Argon2};

    let session = get_session_by_token_async(session_token).await?;

    let locked: Vec<(i64, String)> = session.with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, pin_hash FROM friends WHERE pin_hash IS NOT NULL AND blocked = 0"
        ).map_err(|e| format!("Prepare: {}", e))?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        }).map_err(|e| format!("Query: {}", e))?;
        let mut result = Vec::new();
        for row in rows { result.push(row.map_err(|e| format!("Row: {}", e))?); }
        Ok(result)
    })?;

    if locked.is_empty() {
        return Ok(vec![]);
    }

    let pin_bytes = pin.as_bytes().to_vec();
    let locked_clone = locked.clone();
    let matched_ids: Vec<i64> = tokio::task::spawn_blocking(move || {
        let argon = Argon2::default();
        locked_clone.iter()
            .filter_map(|(fid, hash_str)| {
                PasswordHash::new(hash_str).ok().and_then(|parsed| {
                    argon.verify_password(&pin_bytes, &parsed).ok().map(|_| *fid)
                })
            })
            .collect()
    }).await.map_err(|e| format!("Task: {}", e))?;

    if matched_ids.is_empty() {
        return Ok(vec![]);
    }

    let friends: Vec<FriendInfo> = session.with_db(|conn| {
        let mut result = Vec::new();
        for fid in &matched_ids {
            let mut stmt = conn.prepare(
                "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                        x25519_public_key, friendship_signature_local, friendship_signature_remote,
                        verified, blocked, created_at, updated_at, avatar
                 FROM friends WHERE id = ?1 AND pin_hash IS NOT NULL"
            ).map_err(|e| format!("Prepare: {}", e))?;
            let friend = stmt.query_row([fid], |row| {
                Ok(crate::db::user::Friend {
                    id: row.get(0)?,
                    pseudo: row.get(1)?,
                    username_hash: row.get(2)?,
                    identity_key_public: row.get(3)?,
                    kyber_public_key: row.get(4)?,
                    x25519_public_key: row.get(5)?,
                    friendship_signature_local: row.get(6)?,
                    friendship_signature_remote: row.get(7)?,
                    verified: row.get(8)?,
                    blocked: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                    avatar: row.get(12)?,
                })
            }).map_err(|e| format!("Row: {}", e))?;
            result.push(FriendInfo::from(friend));
        }
        Ok(result)
    })?;

    Ok(friends)
}

/// Retire le verrou d'une conversation après vérification du PIN.
#[tauri::command]
pub async fn remove_conversation_lock(
    session_token: String,
    friend_id: i64,
    pin: String,
) -> Result<(), String> {
    use argon2::{password_hash::{PasswordHash, PasswordVerifier}, Argon2};

    let session = get_session_by_token_async(session_token).await?;

    let stored_hash: Option<String> = session.with_db(|conn| {
        conn.query_row(
            "SELECT pin_hash FROM friends WHERE id = ?1",
            [friend_id],
            |row| row.get::<_, Option<String>>(0),
        ).map_err(|e| format!("DB: {}", e))
    })?;

    let hash_str = stored_hash.ok_or_else(|| "Cette conversation n'est pas verrouillée".to_string())?;
    let pin_bytes = pin.as_bytes().to_vec();

    let ok = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash_str)
            .ok()
            .map(|parsed| Argon2::default().verify_password(&pin_bytes, &parsed).is_ok())
            .unwrap_or(false)
    }).await.unwrap_or(false);

    if !ok {
        return Err("PIN incorrect".to_string());
    }

    session.with_db(|conn| {
        conn.execute("UPDATE friends SET pin_hash = NULL WHERE id = ?1", [friend_id])
            .map(|_| ())
            .map_err(|e| format!("DB: {}", e))
    })?;
    session.invalidate_friends_cache();
    Ok(())
}

