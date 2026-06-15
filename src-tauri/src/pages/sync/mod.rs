use zenth_crypto::encoding::base64::{EncodeImpl, EncodeSecure};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, aead::{Aead, KeyInit}};
use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{PublicKey, SecretKey, DetachedSignature};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use crate::db::UserDb;
use zenth_crypto::kem::{PublicKey as KemPublicKey, SecretKey as KemSecretKey};

// Constantes de taille clés
const DILITHIUM2_PUB_LEN: usize = 1312;

// Payloads QR (minuscules - sans les clés)
/// QR affiché par le NOUVEL APPAREIL - contient uniquement un ID court.
#[derive(Deserialize)]
struct NewDeviceQr {
    pid: String, // pairing_id hex (16 bytes = 32 chars)
    h: String,   // SHA256(dil_pub || kyber_pub) base64 - engagement anti-substitution
    v: String,   // "1"
}

/// QR retour de l'APPAREIL DE CONFIANCE - ID court + flag retour + username.
#[derive(Deserialize)]
struct TrustedDeviceQr {
    pid: String,          // même pairing_id
    v: String,            // "1"
    r: String,            // "1"
    u: Option<String>,    // username de l'appareil de confiance
}

/// Résultat de sync_accounts_user.
#[derive(Serialize)]
pub struct DeviceSyncKeys {
    pub dilithium_pubkey: String,
    pub kyber_pubkey: String,
}

/// Résultat de generate_pairing_qr.
#[derive(Serialize)]
pub struct PairingQrResult {
    pub return_qr: String,          // JSON minuscule à afficher comme QR
    pub dilithium_pubkey_pc: String, // clé dil du nouvel appareil (pour send_sync_key)
    pub kyber_pubkey_pc: String,     // clé kyber du nouvel appareil (pour send_sync_key)
}

/// Résultat de verify_pairing_qr.
#[derive(Serialize)]
pub struct VerifyPairingResult {
    pub pubkey: String,             // base64 Dilithium pubkey de l'appareil de confiance
    pub username_hash_hex: String,  // hex du username_hash de l'appareil de confiance
    pub trusted_username: String,   // username en clair de l'appareil de confiance
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Signe `data` avec la clé secrète Dilithium2. Retourne un Vec vide en cas d'erreur.
async fn dilithium_sign(secret: Vec<u8>, data: Vec<u8>) -> Vec<u8> {
    tokio::task::spawn_blocking(move || {
        if let Ok(sk) = dilithium2::SecretKey::from_bytes(&secret) {
            dilithium2::detached_sign(&data, &sk).as_bytes().to_vec()
        } else {
            vec![]
        }
    })
    .await
    .unwrap_or_default()
}

fn dht_config() -> crate::api::register::RegisterConfig {
    crate::api::register::RegisterConfig {
        base_url: crate::config::dht_api_url(),
        darknet: crate::api::register::DarknetType::Http,
        timeout_secs: 10,
        max_retries: 1,
        retry_delay_ms: 500,
    }
}

// Commandes Tauri
/// This function return Public Key of this devices
/// Always disponible if others functions have need with asynchrone function
#[tauri::command]
pub async fn sync_accounts_user(
    session_token: String,
    password: String,
) -> Result<DeviceSyncKeys, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username = session.username.clone();

    tokio::task::spawn_blocking(move || -> Result<DeviceSyncKeys, String> {
        let db = UserDb::open(&username, &password)
            .map_err(|_| "Mot de passe incorrect".to_string())?;
        let entry = db.get_user_sync()
            .map_err(|_| "Données utilisateur introuvables".to_string())?;
        Ok(DeviceSyncKeys {
            dilithium_pubkey: EncodeImpl::base64encode(&entry.identity_key_public),
            kyber_pubkey: EncodeImpl::base64encode(&entry.kyber_public_key),
        })
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))?
}

/// RÔLE : Nouvel appareil - Étape 1
///
/// Vérifie le mdp, publie les clés sur le DHT (TTL 5 min),
/// retourne le JSON du QR minuscule :
///   { pid: "<32hex>", h: "<sha256 b64>", v: "1" }
#[tauri::command]
pub async fn publish_pairing_keys(
    session_token: String,
    password: String,
) -> Result<String, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username = session.username.clone();
    let sender_pubkey = session.identity_key_public.clone();
    let sender_secret = session.dilithium_secret.clone();

    let (dil_pub, kyber_pub) = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, Vec<u8>), String> {
        let db = UserDb::open(&username, &password)
            .map_err(|_| "Mot de passe incorrect".to_string())?;
        let entry = db.get_user_sync()
            .map_err(|_| "Données utilisateur introuvables".to_string())?;
        Ok((entry.identity_key_public, entry.kyber_public_key))
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    // pairing_id aléatoire (16 bytes → 32 hex chars dans le QR)
    let pairing_id: [u8; 16] = rand::random();

    // h = SHA256(dil_pub || kyber_pub) - engagement dans le QR
    let mut hasher = Sha256::new();
    hasher.update(&dil_pub);
    hasher.update(&kyber_pub);
    let key_hash = EncodeImpl::base64encode(hasher.finalize().as_slice());

    // ciphertext du blob = dil_pub || kyber_pub (taille fixe connue)
    let mut ciphertext = Vec::with_capacity(dil_pub.len() + kyber_pub.len());
    ciphertext.extend_from_slice(&dil_pub);
    ciphertext.extend_from_slice(&kyber_pub);

    // auth_signature = sign(pairing_id || ciphertext || timestamp) par le nouvel appareil
    let ts = now_secs();
    let mut auth_msg = Vec::new();
    auth_msg.extend_from_slice(&pairing_id);
    auth_msg.extend_from_slice(&ciphertext);
    auth_msg.extend_from_slice(&ts.to_le_bytes());
    let auth_sig = dilithium_sign(sender_secret, auth_msg).await;

    let client = crate::api::SyncApiClient::new(dht_config()).await
        .map_err(|e| format!("DHT indisponible: {}", e))?;
    client.push_blob(crate::api::sync::SyncBlob {
        for_device_dilithium_pubkey: pairing_id.to_vec(),
        ciphertext,
        signature: vec![],
        ttl_secs: 300,
        sender_dilithium_pubkey: sender_pubkey,
        auth_signature: auth_sig,
        timestamp: ts,
    })
    .await
    .map_err(|e| format!("Impossible de publier sur le DHT: {}", e))?;

    Ok(serde_json::json!({
        "pid": hex::encode(pairing_id),
        "h":   key_hash,
        "v":   "1",
    })
    .to_string())
}

/// RÔLE : Appareil de confiance - Étape 2
///
/// Scanne le petit QR du nouvel appareil, récupère ses clés depuis le DHT,
/// vérifie l'engagement h, signe, publie la réponse sur le DHT (TTL 5 min),
/// retourne le QR retour (minuscule) + clés du nouvel appareil pour send_sync_key.
#[tauri::command]
pub async fn generate_pairing_qr(
    session_token: String,
    password: String,
    scanned_qr_json: String,
) -> Result<PairingQrResult, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let dilithium_secret = session.dilithium_secret.clone();
    let dilithium_pubkey_tel = session.identity_key_public.clone();
    let username = session.username.clone();

    // Présence physique
    tokio::task::spawn_blocking(move || {
        UserDb::open(&username, &password)
            .map_err(|_| "Mot de passe incorrect".to_string())
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    let qr: NewDeviceQr = serde_json::from_str(&scanned_qr_json)
        .map_err(|_| "QR invalide - format JSON incorrect".to_string())?;
    if qr.v != "1" {
        return Err("Version de protocole non supportée".to_string());
    }

    let pairing_id = hex::decode(&qr.pid)
        .map_err(|_| "pairing_id invalide".to_string())?;

    // Signe le fetch du blob de pairing (appareil de confiance prouve qu'il a une clé Dilithium)
    let client = crate::api::SyncApiClient::new(dht_config()).await
        .map_err(|e| format!("DHT indisponible: {}", e))?;

    let ts_fetch = now_secs();
    let mut fetch_sign_data = pairing_id.clone();
    fetch_sign_data.extend_from_slice(&ts_fetch.to_le_bytes());
    let fetch_sig = dilithium_sign(dilithium_secret.clone(), fetch_sign_data).await;

    let blob = client.fetch_blob(
        &pairing_id,
        fetch_sig,
        ts_fetch,
        dilithium_pubkey_tel.clone(),
    ).await
        .map_err(|e| format!("Clés du nouvel appareil introuvables sur le DHT: {}", e))?;

    if blob.ciphertext.len() < DILITHIUM2_PUB_LEN {
        return Err("Données DHT corrompues".to_string());
    }
    let dil_pub_pc  = blob.ciphertext[..DILITHIUM2_PUB_LEN].to_vec();
    let kyber_pub_pc = blob.ciphertext[DILITHIUM2_PUB_LEN..].to_vec();

    // Vérifie l'engagement
    let mut hasher = Sha256::new();
    hasher.update(&dil_pub_pc);
    hasher.update(&kyber_pub_pc);
    if EncodeImpl::base64encode(hasher.finalize().as_slice()) != qr.h {
        return Err("Clés DHT ne correspondent pas au QR - attaque possible".to_string());
    }

    let timestamp = now_secs();
    let expiry    = timestamp + 120;

    // Clé DHT de la réponse = pairing_id + [0xFF]
    let mut response_key = pairing_id.clone();
    response_key.push(0xFF);

    // ciphertext réponse = dil_pub_tel || timestamp_le64 || expiry_le64 || dil_pub_pc || kyber_pub_pc || user_hash_tel(32)
    let user_hash_tel = session.user_hash.clone();
    let mut response_ct = Vec::new();
    response_ct.extend_from_slice(&dilithium_pubkey_tel);
    response_ct.extend_from_slice(&timestamp.to_le_bytes());
    response_ct.extend_from_slice(&expiry.to_le_bytes());
    response_ct.extend_from_slice(&dil_pub_pc);
    response_ct.extend_from_slice(&kyber_pub_pc);
    response_ct.extend_from_slice(&user_hash_tel);

    // content_msg construit AVANT la closure pour ne pas capturer dil_pub_pc / kyber_pub_pc
    // (ils seront encore nécessaires dans PairingQrResult)
    let ts_push = timestamp;
    let mut content_msg = Vec::new();
    content_msg.extend_from_slice(&dil_pub_pc);
    content_msg.extend_from_slice(&kyber_pub_pc);
    content_msg.extend_from_slice(&ts_push.to_le_bytes());

    let rk = response_key.clone();
    let rct = response_ct.clone();
    let (sig_bytes, auth_sig_bytes) = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, Vec<u8>), String> {
        let sk = dilithium2::SecretKey::from_bytes(&dilithium_secret)
            .map_err(|_| "Clé privée Dilithium invalide".to_string())?;

        // content_sig = sign(dil_pub_pc || kyber_pub_pc || timestamp) - E2E, vérifié par le nouvel appareil
        let content_sig = dilithium2::detached_sign(&content_msg, &sk).as_bytes().to_vec();

        // auth_sig = sign(response_key || response_ct || timestamp) - auth serveur uniquement
        let mut auth_msg = Vec::new();
        auth_msg.extend_from_slice(&rk);
        auth_msg.extend_from_slice(&rct);
        auth_msg.extend_from_slice(&ts_push.to_le_bytes());
        let auth_sig = dilithium2::detached_sign(&auth_msg, &sk).as_bytes().to_vec();

        Ok((content_sig, auth_sig))
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    client.push_blob(crate::api::sync::SyncBlob {
        for_device_dilithium_pubkey: response_key,
        ciphertext: response_ct,
        signature: sig_bytes,
        ttl_secs: 300,
        sender_dilithium_pubkey: dilithium_pubkey_tel,
        auth_signature: auth_sig_bytes,
        timestamp,
    })
    .await
    .map_err(|e| format!("Impossible de publier la réponse sur le DHT: {}", e))?;

    Ok(PairingQrResult {
        return_qr: serde_json::json!({
            "pid": qr.pid,
            "v": "1",
            "r": "1",
            "u": session.username,
        }).to_string(),
        dilithium_pubkey_pc: EncodeImpl::base64encode(&dil_pub_pc),
        kyber_pubkey_pc:     EncodeImpl::base64encode(&kyber_pub_pc),
    })
}

/// RÔLE : Nouvel appareil - Étape 3
///
/// Scanne le QR retour (minuscule), récupère et vérifie la signature depuis le DHT.
/// Retourne dilithium_pubkey_tel (base64).
#[tauri::command]
pub async fn verify_pairing_qr(
    session_token: String,
    qr_json: String,
) -> Result<VerifyPairingResult, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let our_pubkey = session.identity_key_public.clone();
    let our_secret = session.dilithium_secret.clone();

    let qr: TrustedDeviceQr = serde_json::from_str(&qr_json)
        .map_err(|_| "QR invalide - format JSON incorrect".to_string())?;
    if qr.v != "1" || qr.r != "1" {
        return Err("QR invalide - ce n'est pas un QR retour Zenth".to_string());
    }

    let pairing_id = hex::decode(&qr.pid)
        .map_err(|_| "pairing_id invalide".to_string())?;

    let client = crate::api::SyncApiClient::new(dht_config()).await
        .map_err(|e| format!("DHT indisponible: {}", e))?;

    // Clé DHT de la réponse = pairing_id + [0xFF]
    let mut response_key = pairing_id.clone();
    response_key.push(0xFF);

    // Le nouvel appareil signe le fetch avec sa propre clé Dilithium
    let ts_fetch = now_secs();
    let mut fetch_sign_data = response_key.clone();
    fetch_sign_data.extend_from_slice(&ts_fetch.to_le_bytes());
    let fetch_sig = dilithium_sign(our_secret, fetch_sign_data).await;

    let resp = client.fetch_blob(
        &response_key,
        fetch_sig,
        ts_fetch,
        our_pubkey,
    ).await
        .map_err(|e| format!("Réponse introuvable sur le DHT: {}", e))?;

    // Parse: dil_pub_tel(1312) | ts(8) | exp(8) | dil_pub_pc(1312) | kyber_pub_pc(var)
    let min_len = DILITHIUM2_PUB_LEN + 8 + 8 + DILITHIUM2_PUB_LEN;
    if resp.ciphertext.len() < min_len {
        return Err("Réponse DHT corrompue".to_string());
    }

    let mut off = 0;
    let dil_pub_tel  = resp.ciphertext[off..off + DILITHIUM2_PUB_LEN].to_vec(); off += DILITHIUM2_PUB_LEN;
    let timestamp    = u64::from_le_bytes(resp.ciphertext[off..off + 8].try_into().unwrap()); off += 8;
    let expiry       = u64::from_le_bytes(resp.ciphertext[off..off + 8].try_into().unwrap()); off += 8;
    let dil_pub_pc   = resp.ciphertext[off..off + DILITHIUM2_PUB_LEN].to_vec(); off += DILITHIUM2_PUB_LEN;
    // user_hash_tel (32 bytes SHA256) est appended à la fin après kyber_pub_pc
    let remaining = &resp.ciphertext[off..];
    let (kyber_pub_pc, user_hash_tel) = if remaining.len() > 32 {
        let split = remaining.len() - 32;
        (remaining[..split].to_vec(), remaining[split..].to_vec())
    } else {
        (remaining.to_vec(), vec![])
    };

    if expiry <= now_secs() {
        return Err("QR expiré (délai de 2 minutes dépassé)".to_string());
    }

    // Reconstitue le message signé (sans user_hash_tel - ajouté après signature)
    let mut msg = Vec::new();
    msg.extend_from_slice(&dil_pub_pc);
    msg.extend_from_slice(&kyber_pub_pc);
    msg.extend_from_slice(&timestamp.to_le_bytes());

    let sig_bytes    = resp.signature.clone();
    let dil_pub_tel2 = dil_pub_tel.clone();

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let pub_key = dilithium2::PublicKey::from_bytes(&dil_pub_tel2)
            .map_err(|_| "Clé publique Dilithium invalide".to_string())?;
        let sig = dilithium2::DetachedSignature::from_bytes(&sig_bytes)
            .map_err(|_| "Format de signature invalide".to_string())?;
        dilithium2::verify_detached_signature(&sig, &msg, &pub_key)
            .map_err(|_| "Échec de la vérification - appairage refusé".to_string())
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    Ok(VerifyPairingResult {
        pubkey: EncodeImpl::base64encode(&dil_pub_tel),
        username_hash_hex: hex::encode(&user_hash_tel),
        trusted_username: qr.u.unwrap_or_default(),
    })
}

// Phase 2 - Échange de Sync Key via Kyber
/// RÔLE : Appareil de confiance - Étape 4
///
/// Encapsule une Sync Key Kyber pour le nouvel appareil, signe le ciphertext,
/// publie le blob sur le DHT.
#[tauri::command]
pub async fn send_sync_key(
    session_token: String,
    kyber_pubkey_pc_base64: String,
    dilithium_pubkey_pc_base64: String,
) -> Result<String, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let dilithium_secret      = session.dilithium_secret.clone();
    let username              = session.username.clone();
    let password              = session.password.clone();
    let username_for_manifest = username.clone();
    let password_for_manifest = password.clone();
    let own_hash_for_manifest = session.user_hash_hex.clone();

    let dilithium_pubkey_pc = EncodeImpl::base64_vecdecode(&dilithium_pubkey_pc_base64)
        .map_err(|_| "dilithium_pubkey_pc_base64 invalide".to_string())?;
    let peer_pubkey = dilithium_pubkey_pc.clone();

    let sender_pubkey = session.identity_key_public.clone();
    let ts_push = now_secs();
    let dil_pub_pc_for_auth = dilithium_pubkey_pc.clone();

    let (ciphertext_b64, sig_b64, auth_sig_bytes, shared_secret) =
        tokio::task::spawn_blocking(move || -> Result<(String, String, Vec<u8>, Vec<u8>), String> {
            let kyber_pubkey_bytes = EncodeImpl::base64_vecdecode(&kyber_pubkey_pc_base64)
                .map_err(|_| "kyber_pubkey_pc base64 invalide".to_string())?;

            let kyber_pub = KemPublicKey::deserialize(&kyber_pubkey_bytes)
                .map_err(|_| "Clé Kyber publique invalide".to_string())?;
            let (shared_secret, ciphertext) = kyber_pub.encapsulate_with_os_rng()
                .map_err(|_| "Encapsulation Kyber échouée".to_string())?;

            let sk = dilithium2::SecretKey::from_bytes(&dilithium_secret)
                .map_err(|_| "Clé privée Dilithium invalide".to_string())?;

            // content_sig = sign(ciphertext) - vérifié E2E par le nouvel appareil
            let content_sig = dilithium2::detached_sign(&ciphertext, &sk);

            // auth_sig = sign(for_device || ciphertext || timestamp) - auth serveur
            let mut auth_msg = Vec::new();
            auth_msg.extend_from_slice(&dil_pub_pc_for_auth);
            auth_msg.extend_from_slice(&ciphertext);
            auth_msg.extend_from_slice(&ts_push.to_le_bytes());
            let auth_sig = dilithium2::detached_sign(&auth_msg, &sk).as_bytes().to_vec();

            Ok((
                EncodeImpl::base64encode(&ciphertext),
                EncodeImpl::base64encode(content_sig.as_bytes()),
                auth_sig,
                shared_secret.to_vec(),
            ))
        })
        .await
        .map_err(|e| format!("Thread pool error: {}", e))??;

    let client = crate::api::SyncApiClient::new(dht_config()).await
        .map_err(|e| format!("DHT indisponible: {}", e))?;

    let blob = crate::api::sync::SyncBlob {
        for_device_dilithium_pubkey: dilithium_pubkey_pc,
        ciphertext: EncodeImpl::base64_vecdecode(&ciphertext_b64).unwrap_or_default(),
        signature:  EncodeImpl::base64_vecdecode(&sig_b64).unwrap_or_default(),
        ttl_secs:   3600,
        sender_dilithium_pubkey: sender_pubkey,
        auth_signature: auth_sig_bytes,
        timestamp: ts_push,
    };
    let _ = client.push_blob(blob).await;

    // Stocker la Sync Key côté appareil de confiance
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(db) = crate::db::user::UserDb::open(&username, &password) {
            let _ = db.save_paired_device(&peer_pubkey, &shared_secret);
        }
    }).await;

    // Pousse le manifeste MD5 : le nouvel appareil recevra un SyncPatch
    // avec uniquement ce qui lui manque (contacts, messages, settings)
    let _ = relay_push_manifest_internal(username_for_manifest, password_for_manifest, own_hash_for_manifest).await;

    Ok("sync_key_sent".to_string())
}

/// RÔLE : Nouvel appareil - Étape 5
///
/// Récupère la Sync Key depuis le DHT, vérifie la signature Dilithium,
/// décapsule via Kyber, stocke localement la Sync Key + pubkey du device de confiance.
/// Si `trusted_username_hash_hex` est différent du hash local, supprime le compte
/// temporaire du DHT et adopte le username_hash de l'appareil de confiance.
#[tauri::command]
pub async fn fetch_sync_key(
    session_token: String,
    dilithium_pubkey_tel_base64: String,
    trusted_username_hash_hex: String,
    trusted_username: String,
) -> Result<String, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let kyber_secret        = session.kyber_secret.clone();
    let identity_key_public = session.identity_key_public.clone();
    let dilithium_secret    = session.dilithium_secret.clone();
    let username            = session.username.clone();

    let client = crate::api::SyncApiClient::new(dht_config()).await
        .map_err(|e| format!("Client sync: {}", e))?;

    // Le nouvel appareil signe avec sa propre clé (for_device == requester ici)
    let ts_fetch = now_secs();
    let mut fetch_sign_data = identity_key_public.clone();
    fetch_sign_data.extend_from_slice(&ts_fetch.to_le_bytes());
    let fetch_sig = dilithium_sign(dilithium_secret.clone(), fetch_sign_data).await;

    let blob = client.fetch_blob(
        &identity_key_public,
        fetch_sig,
        ts_fetch,
        vec![], // requester == for_device
    ).await
        .map_err(|e| format!("Blob non disponible: {}", e))?;

    let pubkey_tel_bytes = EncodeImpl::base64_vecdecode(&dilithium_pubkey_tel_base64)
        .map_err(|_| "dilithium_pubkey_tel base64 invalide".to_string())?;

    let shared_secret = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let pub_key = dilithium2::PublicKey::from_bytes(&pubkey_tel_bytes)
            .map_err(|_| "Clé publique Dilithium invalide".to_string())?;
        let sig = dilithium2::DetachedSignature::from_bytes(&blob.signature)
            .map_err(|_| "Signature blob invalide".to_string())?;
        dilithium2::verify_detached_signature(&sig, &blob.ciphertext, &pub_key)
            .map_err(|_| "Signature blob invalide - blob rejeté".to_string())?;

        let kyber_sec = KemSecretKey::deserialize(&kyber_secret)
            .map_err(|_| "Clé Kyber secrète invalide".to_string())?;
        kyber_sec.decapsulate(&blob.ciphertext)
            .map(|ss| ss.to_vec())
            .map_err(|_| "Décapsulation Kyber échouée".to_string())
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    // Signe le delete avec la clé du nouvel appareil
    let ts_del = now_secs();
    let mut del_sign_data = identity_key_public.clone();
    del_sign_data.extend_from_slice(&ts_del.to_le_bytes());
    let del_sig = dilithium_sign(dilithium_secret, del_sign_data).await;
    let _ = client.delete_blob(&identity_key_public, del_sig, ts_del, vec![]).await;

    // Stocker la Sync Key + pubkey du device de confiance (pour les relay)
    let pubkey_tel = EncodeImpl::base64_vecdecode(&dilithium_pubkey_tel_base64)
        .unwrap_or_default();
    let pubkey_tel_for_rollback = pubkey_tel.clone();
    let shared_clone = shared_secret.clone();
    let password_clone = session.password.clone();
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(db) = crate::db::user::UserDb::open(&username, &password_clone) {
            let _ = db.save_paired_device(&pubkey_tel, &shared_clone);
        }
    }).await;

    // Envoie notre manifeste MD5 à l'appareil de confiance :
    // il calculera le diff et nous enverra uniquement ce qui manque.
    let _ = relay_push_manifest_internal(session.username.clone(), session.password.clone(), session.user_hash_hex.clone()).await;

    // Nettoyage du compte temporaire
    // Si Device B s'est enregistré avec un username temporaire différent de celui
    // de Device A, on supprime le compte temporaire du DHT et on adopte le hash
    // de Device A dans la base locale.
    let mut effective_username = session.username.clone();

    if !trusted_username_hash_hex.is_empty()
        && trusted_username_hash_hex != session.user_hash_hex
    {
        if let Ok(trusted_hash_bytes) = hex::decode(&trusted_username_hash_hex) {
            if trusted_hash_bytes.len() == 32 {
                // 1. Signe et supprime le compte temporaire du DHT
                use crate::utils::timestamp::plateform::current_timestamp;
                let ts = current_timestamp();
                let mut msg_to_sign = Vec::new();
                msg_to_sign.extend_from_slice(&session.user_hash);
                msg_to_sign.extend_from_slice(&ts.to_le_bytes());
                let sk_bytes = session.dilithium_secret.clone();
                let sig = tokio::task::spawn_blocking(move || -> Vec<u8> {
                    if let Ok(sk) = dilithium2::SecretKey::from_bytes(&sk_bytes) {
                        dilithium2::detached_sign(&msg_to_sign, &sk).as_bytes().to_vec()
                    } else {
                        vec![]
                    }
                }).await.unwrap_or_default();

                if !sig.is_empty() {
                    if let Ok(dht_client) = crate::api::DeleteApiClient::new(dht_config()).await {
                        let _ = dht_client.delete_account(
                            session.user_hash.clone(),
                            sig,
                            ts,
                        ).await;
                    }
                }

                // 2. Adopte le username_hash de Device A et renomme le compte local.
                // Si le rename échoue (ex. même appareil : entrée déjà existante),
                // on annule update_username_hash et on retire l'appareil associé,
                // puis on remonte une Err pour que l'UI affiche l'erreur.
                let username_upd    = session.username.clone();
                let password_upd    = session.password.clone();
                let orig_hash_hex   = session.user_hash_hex.clone();
                let new_hash_hex    = trusted_username_hash_hex.clone();
                let new_username    = trusted_username.clone();
                let rollback_pubkey = pubkey_tel_for_rollback;
                let rename_result = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let db = crate::db::user::UserDb::open(&username_upd, &password_upd)
                        .map_err(|e| format!("db: {}", e))?;
                    db.update_username_hash(&new_hash_hex)
                        .map_err(|e| format!("hash update: {}", e))?;
                    if !new_username.is_empty() && new_username != username_upd {
                        let master = crate::db::MasterDb::open()
                            .map_err(|e| format!("master db: {}", e))?;
                        if master.rename_user(&username_upd, &new_username).is_err() {
                            // Rollback : rétablit le hash d'origine et supprime l'appairage
                            let _ = db.update_username_hash(&orig_hash_hex);
                            let _ = db.remove_paired_device(&rollback_pubkey);
                            return Err("sync_account_conflict".to_string());
                        }
                        return Ok(new_username);
                    }
                    Ok(username_upd)
                }).await.map_err(|e| format!("Thread error: {}", e))?;

                match rename_result {
                    Ok(name) => effective_username = name,
                    Err(e) => return Err(e),
                }
            }
        }
    }

    Ok(serde_json::json!({ "effective_username": effective_username }).to_string())
}

// Helpers ChaCha20-Poly1305
fn chacha_encrypt(key32: &[u8; 32], plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 12]), String> {
    let nonce_bytes: [u8; 12] = rand::random();
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key32));
    let ct = cipher.encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|e| format!("Chiffrement relay: {}", e))?;
    Ok((ct, nonce_bytes))
}

fn chacha_decrypt(key32: &[u8; 32], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if nonce.len() != 12 {
        return Err("Nonce relay invalide".to_string());
    }
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key32));
    cipher.decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "Déchiffrement relay échoué - message corrompu ou clé incorrecte".to_string())
}

// Helpers MD5
/// Calcule le MD5 de `data` et retourne le résultat en hex.
/// Utilisé uniquement pour la comparaison de manifestes de sync - pas pour la sécurité.
fn md5_hex(data: &[u8]) -> String {
    use zenth_crypto::hashing::hash::CryptographicHash;
    let mut h = CryptographicHash::new("MD5", 1).expect("MD5 always available");
    h.update(data);
    hex::encode(h.finalize())
}

// Structs pour les patches de sync
#[derive(Serialize, Deserialize, Clone)]
struct PatchMessage {
    mid: String,   // message_id hex
    fh:  String,   // friend_username_hash
    out: bool,
    mt:  String,
    ct:  String,   // base64(inner_bytes - peut être vault-chiffré si ve=true)
    ts:  i64,
    #[serde(default)]
    ve:  bool,     // vault_encrypted
}

#[derive(Serialize, Deserialize, Clone)]
struct PatchContact {
    fh: String,
    fp: String,
    ik: String,
    kk: Option<String>,
    xk: Option<String>,
    sl: Option<String>,
    sr: Option<String>,
}

// Commandes relay
/// Événement relay typé - serialisé en JSON puis chiffré avec la Sync Key.
#[derive(Serialize, Deserialize)]
#[serde(tag = "t")]
enum RelayEvent {
    /// Message de chat (reçu ou envoyé)
    #[serde(rename = "msg")]
    Msg {
        mid: String,          // message_id hex
        fh:  String,          // friend_username_hash hex
        fp:  String,          // friend_pseudo
        out: bool,            // is_outgoing
        mt:  String,          // message_type
        ct:  String,          // base64(inner_message_bytes)
        ts:  i64,
    },
    /// Contact confirmé (ami accepté)
    #[serde(rename = "friend")]
    Friend {
        fh: String,           // username_hash hex
        fp: String,           // pseudo
        ik: String,           // identity_key_public base64
        kk: Option<String>,   // kyber_public_key base64
        xk: Option<String>,   // x25519_public_key base64
        sl: Option<String>,   // friendship_signature_local base64
        sr: Option<String>,   // friendship_signature_remote base64
    },
    /// Demande d'ami (entrante ou sortante)
    #[serde(rename = "freq")]
    FriendRequest {
        fh:  String,
        fp:  Option<String>,
        ik:  Option<String>,
        kk:  Option<String>,
        xk:  Option<String>,
        dir: String,
        sig: String,
        msg: Option<String>,
    },

    /// Manifeste de synchronisation : liste des hashes MD5 de ce qu'on possède.
    /// L'autre appareil compare avec ses propres hashes et envoie un SyncPatch
    /// avec uniquement ce qui manque ou diffère.
    #[serde(rename = "sync_manifest")]
    SyncManifest {
        msg_hashes:     Vec<String>, // MD5 hex de chaque message_id (triés)
        settings_hash:  String,      // MD5 global de tous les settings
        contact_hashes: Vec<String>, // MD5 hex de chaque username_hash (triés)
    },

    /// Patch de synchronisation : données manquantes en réponse à un SyncManifest.
    #[serde(rename = "sync_patch")]
    SyncPatch {
        messages: Vec<PatchMessage>,
        settings: Option<String>,    // JSON "{key:value,...}" si settings_hash diffère
        contacts: Vec<PatchContact>,
    },
}

/// Relaie un message vers tous les appareils jumelés.
///
/// Appelé en best-effort après réception ou envoi d'un message.
pub async fn relay_push_message(
    username: String,
    password: String,
    message_id: String,
    friend_username_hash: String,
    friend_pseudo: String,
    is_outgoing: bool,
    message_type: String,
    inner_bytes_b64: String,
    timestamp: i64,
) -> Result<(), String> {
    relay_push_event(&username, &password, RelayEvent::Msg {
        mid: message_id,
        fh:  friend_username_hash,
        fp:  friend_pseudo,
        out: is_outgoing,
        mt:  message_type,
        ct:  inner_bytes_b64,
        ts:  timestamp,
    }).await
}

/// Helper : charge les paired devices depuis la DB et pousse un événement relay.
async fn relay_push_event(username: &str, password: &str, event: RelayEvent) -> Result<(), String> {
    let un = username.to_string();
    let pw = password.to_string();

    // Récupère la session (déjà en cache) pour obtenir les clés de signature
    let session = tokio::task::spawn_blocking({
        let un = un.clone();
        let pw = pw.clone();
        move || crate::session::get_session(&un, &pw)
    })
    .await
    .map_err(|e| format!("Thread: {}", e))??;

    let sender_pubkey = session.identity_key_public.clone();
    let sender_secret = session.dilithium_secret.clone();

    let pairs = tokio::task::spawn_blocking(move || -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        let db = crate::db::user::UserDb::open(&un, &pw).map_err(|e| format!("DB: {}", e))?;
        db.get_all_paired_devices().map_err(|e| format!("DB: {}", e))
    })
    .await
    .map_err(|e| format!("Thread: {}", e))??;

    if pairs.is_empty() {
        return Ok(());
    }

    let payload = serde_json::to_vec(&event).map_err(|e| format!("Sérialisation: {}", e))?;
    let client = crate::api::SyncApiClient::new(dht_config()).await
        .map_err(|e| format!("DHT: {}", e))?;

    for (peer_pubkey, sync_key) in pairs {
        if sync_key.len() < 32 {
            continue;
        }
        let key32: [u8; 32] = sync_key[..32].try_into().unwrap();
        match chacha_encrypt(&key32, &payload) {
            Ok((ct, nonce)) => {
                // sender_signature = sign(peer_pubkey || timestamp) par l'expéditeur
                let ts = now_secs();
                let mut sign_data = peer_pubkey.clone();
                sign_data.extend_from_slice(&ts.to_le_bytes());
                let sender_sig = dilithium_sign(sender_secret.clone(), sign_data).await;

                let _ = client.relay_push(
                    peer_pubkey.clone(),
                    ct,
                    nonce.to_vec(),
                    sender_pubkey.clone(),
                    sender_sig,
                    ts,
                ).await;
            }
            Err(_) => {}
        }
    }
    Ok(())
}

/// Relaie un contact confirmé vers tous les appareils jumelés.
pub async fn relay_push_friend(
    username: String,
    password: String,
    fh: String,
    fp: String,
    ik: Vec<u8>,
    kk: Option<Vec<u8>>,
    xk: Option<Vec<u8>>,
    sl: Option<Vec<u8>>,
    sr: Option<Vec<u8>>,
) {
    let event = RelayEvent::Friend {
        fh,
        fp,
        ik:  EncodeImpl::base64encode(&ik),
        kk:  kk.map(|k| EncodeImpl::base64encode(&k)),
        xk:  xk.map(|k| EncodeImpl::base64encode(&k)),
        sl:  sl.map(|k| EncodeImpl::base64encode(&k)),
        sr:  sr.map(|k| EncodeImpl::base64encode(&k)),
    };
    let _ = relay_push_event(&username, &password, event).await;
}

/// Relaie une demande d'ami (entrante ou sortante) vers tous les appareils jumelés.
pub async fn relay_push_freq(
    username: String,
    password: String,
    fh: String,
    fp: Option<String>,
    ik: Option<Vec<u8>>,
    kk: Option<Vec<u8>>,
    xk: Option<Vec<u8>>,
    direction: String,
    sig: Vec<u8>,
    msg: Option<String>,
) {
    let event = RelayEvent::FriendRequest {
        fh,
        fp,
        ik:  ik.map(|k| EncodeImpl::base64encode(&k)),
        kk:  kk.map(|k| EncodeImpl::base64encode(&k)),
        xk:  xk.map(|k| EncodeImpl::base64encode(&k)),
        dir: direction,
        sig: EncodeImpl::base64encode(&sig),
        msg,
    };
    let _ = relay_push_event(&username, &password, event).await;
}

// Sync manifest / patch
/// Calcule le manifeste MD5 de l'état local (messages, settings, contacts).
/// `own_hash` : hash de l'utilisateur lui-même, exclu des contacts (Mon espace).
fn compute_sync_manifest(conn: &rusqlite::Connection, own_hash: &str) -> RelayEvent {
    // Messages - MD5 de chaque message_id, trié
    let mut msg_hashes: Vec<String> = conn
        .prepare("SELECT message_id FROM messages ORDER BY timestamp ASC")
        .map(|mut s| {
            s.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).map(|id| md5_hex(id.as_bytes())).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    msg_hashes.sort();

    // Settings - MD5 global de toutes les paires key=value
    let settings_str: String = conn
        .prepare("SELECT key, value FROM settings ORDER BY key")
        .map(|mut s| {
            s.query_map([], |row| {
                Ok(format!("{}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>().join("|"))
            .unwrap_or_default()
        })
        .unwrap_or_default();
    let settings_hash = md5_hex(settings_str.as_bytes());

    // Contacts - MD5 de chaque username_hash, trié.
    // Seuls les contacts réels (clés non vides, hors soi-même) sont comptés.
    let mut contact_hashes: Vec<String> = conn
        .prepare("SELECT username_hash FROM friends WHERE blocked = 0 AND length(identity_key_public) > 0 AND username_hash != ?1 ORDER BY username_hash")
        .map(|mut s| {
            s.query_map(rusqlite::params![own_hash], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).map(|h| md5_hex(h.as_bytes())).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    contact_hashes.sort();

    RelayEvent::SyncManifest { msg_hashes, settings_hash, contact_hashes }
}

/// Compare le manifeste distant avec l'état local et construit le patch des données manquantes.
/// Retourne `None` si tout est à jour.
fn build_sync_patch(
    conn: &rusqlite::Connection,
    data_key: &[u8],
    own_hash: &str,
    remote_msg_hashes: &[String],
    remote_settings_hash: &str,
    remote_contact_hashes: &[String],
) -> Option<RelayEvent> {
    use std::collections::HashSet;

    // Messages manquants
    let remote_msg_set: HashSet<&str> = remote_msg_hashes.iter().map(|s| s.as_str()).collect();

    // Charge tous les messages locaux avec leur hash
    let local_messages: Vec<(String, PatchMessage)> = conn
        .prepare(
            "SELECT m.message_id, m.is_outgoing, m.message_type,
                    m.encrypted_content, m.timestamp, f.username_hash,
                    m.vault_encrypted
             FROM messages m
             JOIN friends f ON f.id = m.friend_id
             ORDER BY m.timestamp ASC"
        )
        .map(|mut s| {
            s.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,   // message_id hex string
                    row.get::<_, bool>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,  // encrypted_content
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,   // friend username_hash
                    row.get::<_, i32>(6).map(|v| v == 1).unwrap_or(false), // vault_encrypted
                ))
            })
            .map(|rows| {
                rows.filter_map(|r| r.ok())
                    .filter_map(|(mid_hex, out, mt, enc, ts, fh, ve)| {
                        let hash = md5_hex(mid_hex.as_bytes());
                        // Déchiffre seulement la couche ChaCha20 - la couche vault
                        // est conservée telle quelle pour être re-chiffrée sur Device B
                        let inner = crate::pages::chat::database::encryption::decrypt_message(
                            &enc, data_key, &mid_hex
                        ).ok()?;
                        use base64::Engine as _;
                        let ct = base64::engine::general_purpose::STANDARD.encode(&inner);
                        Some((hash, PatchMessage { mid: mid_hex, fh, out, mt, ct, ts, ve }))
                    })
                    .collect()
            })
            .unwrap_or_default()
        })
        .unwrap_or_default();

    let missing_messages: Vec<PatchMessage> = local_messages
        .into_iter()
        .filter(|(hash, _)| !remote_msg_set.contains(hash.as_str()))
        .map(|(_, msg)| msg)
        .collect();

    // Settings différents
    let local_settings_str: String = conn
        .prepare("SELECT key, value FROM settings ORDER BY key")
        .map(|mut s| {
            s.query_map([], |row| {
                Ok(format!("{}={}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>().join("|"))
            .unwrap_or_default()
        })
        .unwrap_or_default();
    let local_settings_hash = md5_hex(local_settings_str.as_bytes());

    let settings_patch = if local_settings_hash != remote_settings_hash {
        // Sérialise en JSON {key: value, ...}
        conn.prepare("SELECT key, value FROM settings ORDER BY key")
            .ok()
            .and_then(|mut s| {
                s.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }).ok()
                .map(|rows| {
                    let map: serde_json::Map<String, serde_json::Value> = rows
                        .filter_map(|r| r.ok())
                        .map(|(k, v)| (k, serde_json::Value::String(v)))
                        .collect();
                    serde_json::to_string(&map).unwrap_or_default()
                })
            })
    } else {
        None
    };

    // Contacts manquants
    let remote_contact_set: HashSet<&str> = remote_contact_hashes.iter().map(|s| s.as_str()).collect();

    let missing_contacts: Vec<PatchContact> = conn
        .prepare(
            "SELECT username_hash, pseudo, identity_key_public,
                    kyber_public_key, x25519_public_key,
                    friendship_signature_local, friendship_signature_remote
             FROM friends WHERE blocked = 0 AND length(identity_key_public) > 0
             AND username_hash != ?1"
        )
        .map(|mut s| {
            s.query_map(rusqlite::params![own_hash], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Option<Vec<u8>>>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, Option<Vec<u8>>>(6)?,
                ))
            })
            .map(|rows| {
                rows.filter_map(|r| r.ok())
                    .filter(|(fh, ..)| !remote_contact_set.contains(md5_hex(fh.as_bytes()).as_str()))
                    .map(|(fh, fp, ik, kk, xk, sl, sr)| {
                        use base64::Engine as _;
                        let enc = base64::engine::general_purpose::STANDARD;
                        PatchContact {
                            fh,
                            fp,
                            ik: enc.encode(&ik),
                            kk: kk.map(|k| enc.encode(&k)),
                            xk: xk.map(|k| enc.encode(&k)),
                            sl: sl.map(|k| enc.encode(&k)),
                            sr: sr.map(|k| enc.encode(&k)),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
        })
        .unwrap_or_default();

    // Si rien ne manque, pas de patch à envoyer
    if missing_messages.is_empty() && settings_patch.is_none() && missing_contacts.is_empty() {
        return None;
    }

    Some(RelayEvent::SyncPatch {
        messages: missing_messages,
        settings: settings_patch,
        contacts: missing_contacts,
    })
}

/// Pousse le manifeste MD5 de l'état local vers les appareils jumelés.
/// L'appareil qui le reçoit calcule le diff et envoie un SyncPatch en retour.
#[tauri::command]
pub async fn relay_push_manifest(session_token: String) -> Result<(), String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username  = session.username.clone();
    let password  = session.password.clone();
    let own_hash  = session.user_hash_hex.clone();
    relay_push_manifest_internal(username, password, own_hash).await
}

async fn relay_push_manifest_internal(username: String, password: String, own_hash: String) -> Result<(), String> {
    let un = username.clone();
    let pw = password.clone();
    let oh = own_hash.clone();

    let manifest = tokio::task::spawn_blocking(move || -> Result<RelayEvent, String> {
        let db = crate::db::user::UserDb::open(&un, &pw)
            .map_err(|e| format!("DB: {}", e))?;
        Ok(compute_sync_manifest(db.conn(), &oh))
    })
    .await
    .map_err(|e| format!("Thread: {}", e))??;

    relay_push_event(&username, &password, manifest).await
}

/// Pousse tous les amis (avec leurs pseudos) vers les appareils jumelés.
///
/// À appeler après un appairage réussi pour que le nouvel appareil reçoive
/// le carnet d'adresses complet avec les bons pseudos - le DHT ne stocke
/// jamais les pseudos pour des raisons de vie privée, seul le relay le peut.
#[tauri::command]
pub async fn relay_push_all_contacts(session_token: String) -> Result<u32, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;

    let friends = session.list_friends()
        .map_err(|e| format!("Failed to list friends: {}", e))?;

    let count = friends.len() as u32;
    for friend in friends {
        relay_push_friend(
            session.username.clone(),
            session.password.clone(),
            friend.username_hash,
            friend.pseudo,
            friend.identity_key_public,
            friend.kyber_public_key,
            friend.x25519_public_key,
            friend.friendship_signature_local,
            friend.friendship_signature_remote,
        ).await;
    }

    Ok(count)
}

/// Liste les appareils jumelés avec leur date d'ajout.
#[tauri::command]
pub async fn list_paired_devices(session_token: String) -> Result<Vec<serde_json::Value>, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username = session.username.clone();
    let password = session.password.clone();

    tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>, String> {
        let db = crate::db::user::UserDb::open(&username, &password)
            .map_err(|e| format!("DB: {}", e))?;
        let devices = db.list_paired_devices_full().map_err(|e| format!("DB: {}", e))?;
        Ok(devices.iter().map(|d| serde_json::json!({
            "pubkey_hex": hex::encode(&d.peer_dilithium_pubkey),
            "added_at": d.added_at,
        })).collect())
    })
    .await
    .map_err(|e| format!("Thread: {}", e))?
}

/// Révoque un appareil jumelé (supprime sa sync key locale).
#[tauri::command]
pub async fn revoke_paired_device(session_token: String, pubkey_hex: String) -> Result<(), String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username = session.username.clone();
    let password = session.password.clone();

    let pubkey_bytes = hex::decode(&pubkey_hex)
        .map_err(|_| "pubkey_hex invalide".to_string())?;

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let db = crate::db::user::UserDb::open(&username, &password)
            .map_err(|e| format!("DB: {}", e))?;
        db.remove_paired_device(&pubkey_bytes)
            .map_err(|e| format!("DB: {}", e))
    })
    .await
    .map_err(|e| format!("Thread: {}", e))?
}

/// Diagnostic : nombre d'appareils jumelés et curseur relay.
#[tauri::command]
pub async fn get_relay_status(session_token: String) -> Result<serde_json::Value, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username = session.username.clone();
    let password = session.password.clone();

    tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let db = crate::db::user::UserDb::open(&username, &password)
            .map_err(|e| format!("DB: {}", e))?;
        let pairs = db.get_all_paired_devices().map_err(|e| format!("DB: {}", e))?;
        let cursor = db.get_relay_cursor().unwrap_or(0);
        Ok(serde_json::json!({
            "paired_devices": pairs.len(),
            "relay_cursor": cursor,
        }))
    })
    .await
    .map_err(|e| format!("Thread: {}", e))?
}

/// Récupère et insère les messages relayés depuis le DHT.
///
/// Retourne le nombre de nouveaux événements insérés.
#[tauri::command]
pub async fn relay_pull_messages(session_token: String) -> Result<u32, String> {
    let session = crate::session::get_session_by_token_async(session_token).await?;
    let username            = session.username.clone();
    let our_pubkey          = session.identity_key_public.clone();
    let dilithium_secret    = session.dilithium_secret.clone();
    let data_key            = session.password_hash.clone();
    let own_hash            = session.user_hash_hex.clone();

    let password = session.password.clone();

    // Récupère les paired devices + curseur
    let (pairs, cursor) = tokio::task::spawn_blocking({
        let username = username.clone();
        let password = password.clone();
        move || -> Result<(Vec<(Vec<u8>, Vec<u8>)>, i64), String> {
            let db = crate::db::user::UserDb::open(&username, &password)
                .map_err(|e| format!("DB: {}", e))?;
            let pairs = db.get_all_paired_devices().map_err(|e| format!("DB: {}", e))?;
            let cursor = db.get_relay_cursor().unwrap_or(0);
            Ok((pairs, cursor))
        }
    })
    .await
    .map_err(|e| format!("Thread: {}", e))??;

    if pairs.is_empty() {
        return Ok(0);
    }

    let client = crate::api::SyncApiClient::new(dht_config()).await
        .map_err(|e| format!("DHT indisponible: {}", e))?;

    // signature = sign(our_pubkey || since_id || timestamp) par le propriétaire de la mailbox
    let ts_fetch = now_secs();
    let mut fetch_sign_data = our_pubkey.clone();
    fetch_sign_data.extend_from_slice(&cursor.to_le_bytes());
    fetch_sign_data.extend_from_slice(&ts_fetch.to_le_bytes());
    let fetch_sig = dilithium_sign(dilithium_secret.clone(), fetch_sign_data).await;

    let resp = client.relay_fetch(our_pubkey.clone(), cursor, fetch_sig, ts_fetch).await
        .map_err(|e| format!("Relay fetch: {}", e))?;

    if resp.entries.is_empty() {
        return Ok(0);
    }

    let max_id = resp.entries.iter().map(|e| e.id).max().unwrap_or(0);
    let mut inserted = 0u32;

    for entry in &resp.entries {
        // Tente chaque sync key jusqu'à déchiffrement réussi
        let mut plaintext_opt: Option<Vec<u8>> = None;
        for (_, sync_key) in &pairs {
            if sync_key.len() < 32 { continue; }
            let key32: [u8; 32] = sync_key[..32].try_into().unwrap();
            if let Ok(pt) = chacha_decrypt(&key32, &entry.nonce, &entry.ciphertext) {
                plaintext_opt = Some(pt);
                break;
            }
        }
        let plaintext = match plaintext_opt {
            Some(p) => p,
            None => continue,
        };
        let event: RelayEvent = match serde_json::from_slice(&plaintext) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let username2 = username.clone();
        let password2 = password.clone();
        let data_key2 = data_key.clone();
        let own_hash2 = own_hash.clone();

        let saved = tokio::task::spawn_blocking(move || -> bool {
            let db = match crate::db::user::UserDb::open(&username2, &password2) {
                Ok(d) => d,
                Err(_) => return false,
            };

            match event {
                // Message de chat
                RelayEvent::Msg { mid, fh, fp, out, mt, ct, ts } => {
                    let inner_bytes = match EncodeImpl::base64_vecdecode(&ct) {
                        Ok(b) => b,
                        Err(_) => return false,
                    };

                    // Trouve ou crée le contact placeholder
                    let friend_id = match db.get_friend_by_hash(&fh) {
                        Ok(Some(f)) => f.id,
                        Ok(None) => {
                            let _ = db.conn().execute(
                                "INSERT OR IGNORE INTO friends
                                 (pseudo, username_hash, identity_key_public, created_at, updated_at)
                                 VALUES (?1, ?2, X'', ?3, ?3)",
                                rusqlite::params![fp, fh, ts],
                            );
                            // Toujours relire l'id - last_insert_rowid() est 0 si INSERT OR IGNORE a ignoré
                            match db.conn().query_row(
                                "SELECT id FROM friends WHERE username_hash = ?1",
                                [&fh],
                                |row| row.get::<_, i64>(0),
                            ) {
                                Ok(id) => id,
                                Err(_) => return false,
                            }
                        }
                        Err(_) => return false,
                    };

                    use crate::pages::chat::database::encryption::encrypt_message;
                    let enc = match encrypt_message(&inner_bytes, &data_key2, &mid) {
                        Ok(e) => e,
                        Err(_) => return false,
                    };
                    let status = if out { "sent" } else { "delivered" };
                    db.conn().execute(
                        "INSERT OR IGNORE INTO messages
                         (friend_id, message_id, is_outgoing, message_type,
                          encrypted_content, content_iv, timestamp, status)
                         VALUES (?1, ?2, ?3, ?4, ?5, X'', ?6, ?7)",
                        rusqlite::params![friend_id, mid, out as i64, mt, enc, ts, status],
                    ).map(|n| n > 0).unwrap_or(false)
                }

                // Contact confirmé
                RelayEvent::Friend { fh, fp, ik, kk, xk, sl, sr } => {
                    let decode_opt = |s: Option<String>| -> Option<Vec<u8>> {
                        s.and_then(|b| EncodeImpl::base64_vecdecode(&b).ok())
                    };
                    let ik_bytes = match EncodeImpl::base64_vecdecode(&ik) {
                        Ok(b) => b,
                        Err(_) => return false,
                    };
                    let kk_bytes = decode_opt(kk);
                    let xk_bytes = decode_opt(xk);
                    let sl_bytes = decode_opt(sl);
                    let sr_bytes = decode_opt(sr);
                    let now = crate::utils::timestamp::plateform::current_timestamp() as i64;

                    // Préfixe de hash utilisé comme pseudo par défaut quand aucun nom n'est connu
                    let hash_default = fh[..8.min(fh.len())].to_string();

                    match db.get_friend_by_hash(&fh) {
                        Ok(Some(existing)) if !existing.identity_key_public.is_empty() => {
                            // Ami réel déjà présent sur cet appareil.
                            // Le pseudo local est prioritaire : si l'utilisateur l'a personnalisé,
                            // on ne l'écrase jamais avec celui venu d'un autre appareil.
                            let _ = db.conn().execute(
                                "DELETE FROM pending_friend_requests WHERE remote_username_hash = ?1",
                                [&fh],
                            );
                            return false;
                        }
                        Ok(Some(existing)) => {
                            // Placeholder (clés vides) → on complète avec les vraies données.
                            // Pseudo : on garde le pseudo local s'il a été personnalisé,
                            // sinon on prend celui venu du relay.
                            let pseudo_to_use = if existing.pseudo.is_empty()
                                || existing.pseudo == hash_default
                            {
                                fp.clone() // pseudo par défaut → prendre celui du relay
                            } else {
                                existing.pseudo.clone() // pseudo personnalisé → le conserver
                            };

                            let ok = db.conn().execute(
                                "UPDATE friends SET pseudo = ?1, identity_key_public = ?2,
                                 kyber_public_key = ?3, x25519_public_key = ?4,
                                 friendship_signature_local = ?5, friendship_signature_remote = ?6,
                                 updated_at = ?7 WHERE id = ?8",
                                rusqlite::params![pseudo_to_use, ik_bytes, kk_bytes, xk_bytes, sl_bytes, sr_bytes, now, existing.id],
                            ).map(|n| n > 0).unwrap_or(false);
                            if ok {
                                let _ = db.conn().execute(
                                    "DELETE FROM pending_friend_requests WHERE remote_username_hash = ?1",
                                    [&fh],
                                );
                            } else {
                            }
                            return ok;
                        }
                        Ok(None) => {}
                        Err(_) => {
                            return false;
                        }
                    }

                    let friend = crate::db::user::NewFriend {
                        pseudo: fp,
                        username_hash: fh.clone(),
                        identity_key_public: ik_bytes,
                        kyber_public_key: kk_bytes,
                        x25519_public_key: xk_bytes,
                        friendship_signature_local: sl_bytes,
                        friendship_signature_remote: sr_bytes,
                        verified: false,
                        created_at: now,
                    };
                    // Le DELETE ne s'exécute QUE si l'insertion réussit.
                    // Si add_friend échoue, la demande pending reste visible → l'utilisateur peut ré-essayer.
                    match db.add_friend(&friend) {
                        Ok(_) => {
                            let _ = db.conn().execute(
                                "DELETE FROM pending_friend_requests WHERE remote_username_hash = ?1",
                                [&fh],
                            );
                            true
                        }
                        Err(_) => false,
                    }
                }

                // Demande d'ami
                RelayEvent::FriendRequest { fh, fp, ik, kk, xk, dir, sig, msg } => {
                    // Ignorer si déjà ami
                    if db.get_friend_by_hash(&fh).ok().flatten().is_some() { return false; }
                    // Ignorer seulement si une demande ACTIVE (pending) existe déjà
                    let already_pending = db.conn().query_row(
                        "SELECT COUNT(*) FROM pending_friend_requests WHERE remote_username_hash = ?1 AND status = 'pending'",
                        [&fh],
                        |row| row.get::<_, i64>(0),
                    ).map(|c| c > 0).unwrap_or(false);
                    if already_pending { return false; }

                    let decode_opt = |s: Option<String>| -> Option<Vec<u8>> {
                        s.and_then(|b| EncodeImpl::base64_vecdecode(&b).ok())
                    };
                    let ik_bytes = match ik {
                        Some(ref s) => match EncodeImpl::base64_vecdecode(s) {
                            Ok(b) => b,
                            Err(_) => return false,
                        },
                        None => vec![],
                    };
                    let sig_bytes = match EncodeImpl::base64_vecdecode(&sig) { Ok(b) => b, Err(_) => return false };
                    let now = crate::utils::timestamp::plateform::current_timestamp() as i64;

                    let request = crate::db::user::NewPendingRequest {
                        direction: dir,
                        remote_username_hash: fh,
                        remote_pseudo: fp,
                        remote_identity_key: ik_bytes,
                        remote_kyber_public_key: decode_opt(kk),
                        remote_x25519_public_key: decode_opt(xk),
                        dilithium_signature: sig_bytes,
                        status: "pending".to_string(),
                        message: msg,
                        created_at: now,
                        expires_at: Some(now + 7 * 24 * 3600),
                    };
                    db.add_pending_request(&request).is_ok()
                }

                // Manifeste MD5 reçu : calculer le diff et renvoyer le patch
                RelayEvent::SyncManifest { msg_hashes, settings_hash, contact_hashes } => {
                    if let Ok(db2) = crate::db::user::UserDb::open(&username2, &password2) {
                        if let Some(patch) = build_sync_patch(
                            db2.conn(), &data_key2, &own_hash2,
                            &msg_hashes, &settings_hash, &contact_hashes,
                        ) {
                            // Envoi du patch en background (ne bloque pas le traitement)
                            let un = username2.clone();
                            let pw = password2.clone();
                            tokio::task::spawn(async move {
                                let _ = relay_push_event(&un, &pw, patch).await;
                            });
                        }
                    }
                    true
                }

                // Patch reçu : appliquer uniquement ce qui manque
                RelayEvent::SyncPatch { messages, settings, contacts } => {
                    let mut applied = false;

                    // Messages manquants
                    for pm in messages {
                        let mid_bytes = match hex::decode(&pm.mid) { Ok(b) => b, Err(_) => continue };
                        let already: bool = db.conn().query_row(
                            "SELECT COUNT(*) FROM messages WHERE message_id = ?1",
                            rusqlite::params![mid_bytes],
                            |row| row.get::<_, i64>(0),
                        ).map(|c| c > 0).unwrap_or(false);
                        if already { continue; }

                        use base64::Engine as _;
                        let inner_bytes = match base64::engine::general_purpose::STANDARD.decode(&pm.ct) {
                            Ok(b) => b, Err(_) => continue,
                        };
                        let enc = match crate::pages::chat::database::encryption::encrypt_message(
                            &inner_bytes, &data_key2, &pm.mid,
                        ) { Ok(e) => e, Err(_) => continue };

                        // Trouve le friend_id depuis le username_hash
                        let friend_id: Option<i64> = db.conn().query_row(
                            "SELECT id FROM friends WHERE username_hash = ?1",
                            [&pm.fh],
                            |row| row.get(0),
                        ).ok();
                        let friend_id = match friend_id { Some(id) => id, None => continue };

                        let parsed = crate::pages::chat::messages::parse_inner_for_sync(&inner_bytes);
                        let _ = db.conn().execute(
                            "INSERT OR IGNORE INTO messages
                             (friend_id, message_id, is_outgoing, message_type,
                              encrypted_content, content_iv, timestamp, status, vault_encrypted)
                             VALUES (?1, ?2, ?3, ?4, ?5, X'', ?6, 'delivered', ?7)",
                            rusqlite::params![friend_id, pm.mid, pm.out as i64, parsed.0, enc, pm.ts, pm.ve as i64],
                        );
                        applied = true;
                    }

                    // Settings différents
                    if let Some(settings_json) = settings {
                        if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&settings_json) {
                            for (k, v) in map {
                                if let Some(val) = v.as_str() {
                                    let _ = db.conn().execute(
                                        "INSERT INTO settings (key, value) VALUES (?1, ?2)
                                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                                        rusqlite::params![k, val],
                                    );
                                    applied = true;
                                }
                            }
                        }
                    }

                    // Contacts manquants ou incomplets (placeholders)
                    for pc in contacts {
                        let decode_opt = |s: Option<String>| -> Option<Vec<u8>> {
                            s.and_then(|b| EncodeImpl::base64_vecdecode(&b).ok())
                        };
                        let ik_bytes = match EncodeImpl::base64_vecdecode(&pc.ik) {
                            Ok(b) => b, Err(_) => continue,
                        };
                        let now = crate::utils::timestamp::plateform::current_timestamp() as i64;

                        // Vérifie si un enregistrement existe, et s'il a déjà de vraies clés
                        let existing = db.conn().query_row(
                            "SELECT id, length(identity_key_public) FROM friends WHERE username_hash = ?1",
                            [&pc.fh],
                            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                        ).ok();

                        match existing {
                            Some((_, key_len)) if key_len > 0 => {
                                // Contact complet déjà présent, rien à faire
                                continue;
                            }
                            Some((id, _)) => {
                                // Placeholder : on complète avec les vraies données
                                let _ = db.conn().execute(
                                    "UPDATE friends SET pseudo = ?1, identity_key_public = ?2,
                                     kyber_public_key = ?3, x25519_public_key = ?4,
                                     friendship_signature_local = ?5, friendship_signature_remote = ?6,
                                     updated_at = ?7 WHERE id = ?8",
                                    rusqlite::params![
                                        pc.fp, ik_bytes,
                                        decode_opt(pc.kk), decode_opt(pc.xk),
                                        decode_opt(pc.sl), decode_opt(pc.sr),
                                        now, id,
                                    ],
                                );
                                applied = true;
                            }
                            None => {
                                // Nouveau contact : insertion complète
                                let _ = db.conn().execute(
                                    "INSERT OR IGNORE INTO friends
                                     (pseudo, username_hash, identity_key_public, kyber_public_key,
                                      x25519_public_key, friendship_signature_local,
                                      friendship_signature_remote, verified, blocked, created_at, updated_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8, ?8)",
                                    rusqlite::params![
                                        pc.fp, pc.fh, ik_bytes,
                                        decode_opt(pc.kk), decode_opt(pc.xk),
                                        decode_opt(pc.sl), decode_opt(pc.sr),
                                        now,
                                    ],
                                );
                                applied = true;
                            }
                        }
                    }

                    applied
                }
            }
        })
        .await
        .unwrap_or(false);

        if saved { inserted += 1; }
    }

    // Le relay handler ouvre une UserDb séparée (pas la connexion de session),
    // donc le friends_cache de la session est périmé après tout traitement de Friend event.
    // On invalide systématiquement pour forcer un rechargement depuis la DB.
    session.invalidate_friends_cache();

    // ACK + mise à jour curseur
    // signature = sign(our_pubkey || max_id || timestamp)
    let ts_ack = now_secs();
    let mut ack_sign_data = our_pubkey.clone();
    ack_sign_data.extend_from_slice(&max_id.to_le_bytes());
    ack_sign_data.extend_from_slice(&ts_ack.to_le_bytes());
    let ack_sig = dilithium_sign(dilithium_secret, ack_sign_data).await;
    let _ = client.relay_ack(our_pubkey, max_id, ack_sig, ts_ack).await;
    let username3 = username.clone();
    let password3 = password.clone();
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(db) = crate::db::user::UserDb::open(&username3, &password3) {
            let _ = db.set_relay_cursor(max_id);
        }
    }).await;

    Ok(inserted)
}
