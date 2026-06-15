use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{SecretKey, DetachedSignature};
use zenth_dto::AuthProof;
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;

use crate::pages::register::crypto::key::{generate_username_hash, derive_keys_from_password_with_salt};
use crate::db::{UserDb, MasterDb};
use crate::db::error::DbError;
use crate::utils::security::cipher_key::decrypt_key_with_password;
use crate::api::login::LoginApiClient;
use crate::api::register::{RegisterConfig, DarknetType};
use crate::utils::timestamp::plateform::current_timestamp;

// Compteur d'échecs de connexion - en mémoire Rust, jamais exposé au frontend
static FAIL_COUNTER: Lazy<Mutex<HashMap<String, u32>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// Config wipe par utilisateur - mise à jour via configure_wipe() après login
static WIPE_CONFIG: Lazy<Mutex<HashMap<String, (bool, u32)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Met à jour la configuration de wipe pour un utilisateur.
/// Appelé depuis les paramètres après un login réussi.
pub fn set_wipe_config(username: &str, enabled: bool, max_attempts: u32) {
    WIPE_CONFIG.lock().unwrap_or_else(|e| e.into_inner())
        .insert(username.to_string(), (enabled, max_attempts));
}

/// Incrémente le compteur d'échecs pour un utilisateur.
/// Si le seuil est atteint et le wipe activé, efface les données localement.
async fn handle_failed_attempt(username: &str) {
    let count = {
        let mut map = FAIL_COUNTER.lock().unwrap_or_else(|e| e.into_inner());
        let c = map.entry(username.to_string()).or_insert(0);
        *c += 1;
        *c
    };

    let (wipe_enabled, max_attempts) = WIPE_CONFIG.lock().unwrap_or_else(|e| e.into_inner())
        .get(username)
        .copied()
        .unwrap_or((false, 10));

    if wipe_enabled && count >= max_attempts {
        FAIL_COUNTER.lock().unwrap_or_else(|e| e.into_inner()).remove(username);
        let _ = crate::pages::wipe::wipe_user_no_auth_internal(username.to_string()).await;
    }
}

/// Réinitialise le compteur d'échecs après un login réussi.
fn reset_fail_counter(username: &str) {
    FAIL_COUNTER.lock().unwrap_or_else(|e| e.into_inner()).remove(username);
}

#[tauri::command]
pub async fn login(username: String, password: String) -> Result<String, String> {

    // Phase 1 (blocking thread): DB open + Argon2 + crypto - libère le runtime async
    let (username_hash, user_db, dilithium_secret_bytes) = match tokio::task::spawn_blocking({
        let username = username.clone();
        let password = password.clone();
        move || -> Result<(Vec<u8>, UserDb, Vec<u8>), String> {
            let username_hash = generate_username_hash(&username)?;

            let user_db = UserDb::open(&username, &password)
                .map_err(|e| match e {
                    DbError::UserNotFound(_) => "Invalid username or password".to_string(),
                    DbError::InvalidPassword => "Invalid username or password".to_string(),
                    _ => "Failed to open user database".to_string(),
                })?;

            let user_info = user_db.get_user_info()
                .map_err(|e| format!("Failed to get user info: {}", e))?;

            let master_db = MasterDb::open()
                .map_err(|e| format!("Failed to open master database: {}", e))?;
            let user_entry = master_db.get_user(&username)
                .map_err(|e| format!("User not found in master database: {}", e))?;

            // Argon2id (1-2 sec) - sur le thread pool, pas sur le runtime async
            let hash_password = derive_keys_from_password_with_salt(&password, &user_entry.salt)?;

            let decrypted_keys_json = decrypt_key_with_password(
                &user_info.encrypted_identity_keys,
                &hash_password,
                username.as_bytes(),
            ).map_err(|e| format!("Failed to decrypt keys: {:?}", e))?;

            let keys: serde_json::Value = serde_json::from_slice(&decrypted_keys_json)
                .map_err(|e| format!("Failed to parse decrypted keys: {}", e))?;

            let dilithium_secret_hex = keys["dilithium_secret"]
                .as_str()
                .ok_or("Missing dilithium_secret in stored keys")?;

            let dilithium_secret_bytes = hex::decode(dilithium_secret_hex)
                .map_err(|e| format!("Failed to decode dilithium secret: {}", e))?;

            Ok((username_hash, user_db, dilithium_secret_bytes))
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))?
    {
        Ok(v) => v,
        Err(e) => {
            // Incrémenter le compteur d'échecs côté Rust (wipe si seuil atteint)
            handle_failed_attempt(&username).await;
            return Err(e);
        }
    };

    // Phase 2+3 (async): server auth + Dilithium signature.
    // Wrapped so that paired devices (whose DHT account was deleted by fetch_sync_key)
    // can fall back to local-only login when server auth fails.
    let server_auth: Result<_, String> = async move {
        let config = RegisterConfig {
            base_url: crate::config::dht_api_url(),
            darknet: DarknetType::Http,
            timeout_secs: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
        };

        let client = LoginApiClient::new(config)
            .await
            .map_err(|e| format!("Failed to create API client: {}", e))?;

        let challenge = client.request_challenge(username_hash.clone())
            .await
            .map_err(|e| format!("Failed to get challenge: {}", e))?;

        let timestamp = current_timestamp();

        let mut message_to_sign = Vec::new();
        message_to_sign.extend_from_slice(&challenge.nonce);
        message_to_sign.extend_from_slice(&username_hash);
        message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

        let signature = sign_with_dilithium(&dilithium_secret_bytes, &message_to_sign)?;

        let proof = AuthProof {
            challenge_id: challenge.challenge_id,
            user_hash_id: username_hash.clone(),
            proof_type: challenge.required_proof_type,
            proof: signature,
            public_inputs: vec![],
            timestamp,
        };

        let login_response = client.submit_proof(username_hash, proof)
            .await
            .map_err(|e| format!("Login failed: {}", e))?;

        if let Some(outdated) = &login_response.version_outdated {
            let v = if !outdated.min_version.is_empty() {
                &outdated.min_version
            } else {
                &outdated.latest_version
            };
            return Err(format!("VERSION_OUTDATED:{}", v));
        }

        if !login_response.success {
            return Err(format!("Login failed: {}", login_response.error_message));
        }

        Ok(login_response)
    }.await;

    match server_auth {
        Ok(login_response) => {
            let _ = user_db.set_setting("session_token", &hex::encode(&login_response.session_token));
            let _ = user_db.set_setting("session_expiry", &login_response.session_expiry.to_string());
            reset_fail_counter(&username);
        }
        Err(e) => {
            // Allow local-only login for paired devices: after pairing, fetch_sync_key
            // deletes the device's DHT account, making server auth permanently impossible.
            // The device operates via relay sync using the stored sync key.
            let is_paired = user_db.get_all_paired_devices()
                .map(|d| !d.is_empty())
                .unwrap_or(false);
            if !is_paired {
                handle_failed_attempt(&username).await;
                return Err(e);
            }
        }
    }

    // Create session cache entry and get back the UUID session token
    let session = crate::session::get_session_async(username.clone(), password.clone()).await?;
    let token = session.session_token.clone();

    let token_bg = token.clone();
    tokio::spawn(async move {
        match crate::session::get_session_by_token_async(token_bg.clone()).await {
            Ok(session) => {
                if session.config.background_sync {
                    let _ = crate::pages::friends::sync_friend_responses(token_bg.clone()).await;
                    let _ = crate::pages::friends::sync_friend_requests(token_bg.clone()).await;
                    let _ = crate::pages::chat::sync_messages(token_bg.clone()).await;
                }
            }
            Err(_) => {}
        }
    });

    Ok(token)
}

/// Configure le wipe automatique après N échecs - appliqué au prochain redémarrage de session.
/// Nécessite un session_token valide.
#[tauri::command]
pub async fn configure_wipe(
    session_token: String,
    enabled: bool,
    max_attempts: u32,
) -> Result<(), String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    set_wipe_config(&session.username, enabled, max_attempts);
    Ok(())
}

/// Response for WebSocket authentication data
#[derive(serde::Serialize)]
pub struct WsAuthData {
    pub user_hash: String,      // hex encoded
    pub session_token: String,  // hex encoded
    pub ws_url: String,
}

/// Get WebSocket authentication data
#[tauri::command]
pub async fn get_ws_auth(session_token: String) -> Result<WsAuthData, String> {
    let session = crate::session::get_session_by_token_async(session_token.clone()).await?;

    let user_hash_hex = session.user_hash_hex.clone();

    let server_token = session.get_setting("session_token")
        .unwrap_or(None)
        .unwrap_or_default();

    if let Ok(Some(expiry_str)) = session.get_setting("session_expiry") {
        if let Ok(expiry) = expiry_str.parse::<u64>() {
            let now = current_timestamp();
            if now > expiry {
                return Err("Session expired. Please login again.".to_string());
            }
        }
    }

    let base_url = crate::config::dht_api_url();
    let ws_url = if base_url.starts_with("https://") {
        base_url.replace("https://", "wss://")
    } else {
        base_url.replace("http://", "ws://")
    };

    Ok(WsAuthData {
        user_hash: user_hash_hex,
        session_token: server_token,
        ws_url,
    })
}

/// Logout: clear session cache and invalidate session token in DB
#[tauri::command]
pub async fn logout(session_token: String) -> Result<(), String> {
    // Best-effort: invalidate DB token before clearing cache
    if let Ok(session) = crate::session::get_session_by_token_async(session_token.clone()).await {
        let _ = session.set_setting("session_token", "");
        let _ = session.set_setting("session_expiry", "0");
        // Clear in-memory session cache by username
        crate::session::clear_session_async(session.username.clone()).await;
    } else {
        // Token may already be gone or invalid - clear by token directly
        tokio::task::spawn_blocking(move || {
            crate::session::clear_session_by_token(&session_token);
        }).await.ok();
    }

    Ok(())
}

/// Vérifie qu'un session token est toujours valide en mémoire Rust
#[tauri::command]
pub async fn check_session(session_token: String) -> Result<(), String> {
    crate::session::get_session_by_token_async(session_token).await.map(|_| ())
}

/// Sign a message with Dilithium2 private key
fn sign_with_dilithium(secret_key_bytes: &[u8], message: &[u8]) -> Result<Vec<u8>, String> {
    let secret_key = dilithium2::SecretKey::from_bytes(secret_key_bytes)
        .map_err(|_| "Invalid Dilithium secret key format")?;

    let signature = dilithium2::detached_sign(message, &secret_key);

    Ok(signature.as_bytes().to_vec())
}
