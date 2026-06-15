use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{SecretKey, DetachedSignature};
use crate::db::{UserDb, MasterDb};
use crate::db::paths::user_db_path;
use crate::utils::security::secure_delete::secure_delete;
use crate::api::{DeleteApiClient, RegisterConfig, DarknetType};
use crate::utils::timestamp::plateform::current_timestamp;

/// Efface toutes les données locales d'un utilisateur de manière sécurisée.
/// Nécessite un session_token valide pour vérifier l'identité.
#[tauri::command]
pub async fn wipe_user_data(session_token: String) -> Result<String, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username = session.username.clone();

    let name_hash = tokio::task::spawn_blocking({
        let username = username.clone();
        let password = session.password.clone();
        move || -> Result<String, String> {
            let db = UserDb::open(&username, &password)
                .map_err(|e| format!("Authentification échouée: {}", e))?;

            let master = MasterDb::open()
                .map_err(|e| format!("Erreur master DB: {}", e))?;
            let entry = master.get_user(&username)
                .map_err(|_| "Utilisateur introuvable".to_string())?;

            let _ = db.checkpoint_wal();
            Ok(entry.name_hash)
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    crate::session::clear_session_async(username).await;
    wipe_files_and_index(name_hash).await
}

/// Efface les données sans vérifier les credentials.
/// Appelé uniquement depuis Rust (login.rs) après N échecs - plus exposé au frontend.
pub async fn wipe_user_no_auth_internal(username: String) -> Result<String, String> {
    use crate::db::crypto::hash_username;

    let name_hash = tokio::task::spawn_blocking({
        let username = username.clone();
        move || -> Result<String, String> {
            let name_hash = hash_username(&username);
            let master = MasterDb::open()
                .map_err(|e| format!("Erreur: {}", e))?;
            if !master.user_exists(&name_hash)
                .map_err(|e| format!("{}", e))?
            {
                return Err("Utilisateur introuvable".to_string());
            }
            Ok(name_hash)
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    crate::session::clear_session_async(username).await;
    wipe_files_and_index(name_hash).await
}

/// Supprime le compte côté serveur ET les données locales.
/// Déclenché par une demande explicite de l'utilisateur (scénario 3).
#[tauri::command]
pub async fn delete_account(session_token: String) -> Result<String, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username = session.username.clone();
    let dilithium_secret_bytes = session.dilithium_secret.clone();
    let username_hash = session.user_hash.clone();

    // Get name_hash from master DB for file deletion
    let name_hash = tokio::task::spawn_blocking({
        let username = username.clone();
        let password = session.password.clone();
        move || -> Result<String, String> {
            let user_db = UserDb::open(&username, &password)
                .map_err(|e| format!("Authentification échouée: {}", e))?;
            let master = MasterDb::open()
                .map_err(|e| format!("Erreur master DB: {}", e))?;
            let entry = master.get_user(&username)
                .map_err(|_| "Utilisateur introuvable".to_string())?;
            let _ = user_db.checkpoint_wal();
            Ok(entry.name_hash)
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    // Phase 2 (async): signer et envoyer la demande de suppression au serveur
    let timestamp = current_timestamp();

    let mut msg = Vec::new();
    msg.extend_from_slice(&username_hash);
    msg.extend_from_slice(&timestamp.to_le_bytes());

    let signature = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let secret_key = dilithium2::SecretKey::from_bytes(&dilithium_secret_bytes)
            .map_err(|_| "Invalid Dilithium secret key")?;
        let sig = dilithium2::detached_sign(&msg, &secret_key);
        Ok(sig.as_bytes().to_vec())
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    let config = RegisterConfig {
        base_url: crate::config::dht_api_url(),
        darknet: DarknetType::Http,
        timeout_secs: 30,
        max_retries: 3,
        retry_delay_ms: 1000,
    };

    if let Ok(client) = DeleteApiClient::new(config).await {
        let _ = client.delete_account(username_hash, signature, timestamp).await;
        // Suppression serveur best-effort: on wipe localement même en cas d'échec réseau
    }

    // Phase 3: wipe local
    crate::session::clear_session_async(username.clone()).await;
    wipe_files_and_index(name_hash).await
}

async fn wipe_files_and_index(name_hash: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || -> Result<String, String> {
        let db_path = user_db_path(&name_hash);
        let wal_path = { let mut p = db_path.clone(); p.set_extension("db-wal"); p };
        let shm_path = { let mut p = db_path.clone(); p.set_extension("db-shm"); p };

        secure_delete(&wal_path).map_err(|e| format!("Erreur WAL: {}", e))?;
        secure_delete(&shm_path).map_err(|e| format!("Erreur SHM: {}", e))?;
        secure_delete(&db_path).map_err(|e| format!("Erreur DB: {}", e))?;

        let master = MasterDb::open()
            .map_err(|e| format!("Erreur master DB: {}", e))?;
        master.remove_entry_by_hash(&name_hash)
            .map_err(|e| format!("Erreur suppression index: {}", e))?;

        Ok("Data wiped successfully".to_string())
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))?
}
