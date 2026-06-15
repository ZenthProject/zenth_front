/// Système de récupération de compte Zenth.
///
/// # Architecture
///
/// - **Clé Dilithium2 de récupération** : générée une fois à la création du compte.
///   La clé privée n'est jamais affichée. Elle est stockée chiffrée localement
///   (ChaCha20 via password_hash) et incluse dans le backup .zbc.
///
/// - **Phrase mnémotechnique (24 mots BIP39)** : générée à partir de 32 bytes aléatoires.
///   Sert de `password1` pour déchiffrer le .zbc. L'utilisateur la note sur papier.
///
/// - **Fichier .zbc** : export sélectif (contacts + messages optionnels) + clé privée recovery.
///   Double chiffrement : Serpent-256-CBC(password1/mnemonic) → AES-256-GCM(password2).

pub mod bip39;

use crate::session::get_session_by_token_async;
use crate::api::{RecoveryApiClient, RegisterConfig, DarknetType};
use zenth_crypto::kdf::argon2id::Argon2idHasher;
use zenth_crypto::symmetric::{
    Aes256GcmEncryption, Aes256GcmDecryption,
    serpent_256_cbc_encrypt, serpent_256_cbc_decrypt,
};
use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{PublicKey as _, SecretKey as _, DetachedSignature as _};
use serde::{Serialize, Deserialize};

// Constantes format .zbc
const ZBC_MAGIC: &[u8; 4] = b"ZBC1";
const ZBC_VERSION: u8 = 1;

// Structures payload
#[derive(Serialize, Deserialize)]
struct ZbcPayload {
    version: u8,
    created_at: i64,
    username_hash: String,
    recovery_dilithium_pubkey: String,   // hex
    recovery_dilithium_privkey: String,  // hex - jamais affiché dans l'UI
    contacts: Vec<ZbcContact>,
}

#[derive(Serialize, Deserialize)]
struct ZbcContact {
    username_hash: String,
    pseudo: String,
    identity_key_public: String,
    kyber_public_key: Option<String>,
    x25519_public_key: Option<String>,
    friendship_signature_local: Option<String>,
    friendship_signature_remote: Option<String>,
    messages: Vec<ZbcMessage>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ZbcMessage {
    pub message_id: String,
    pub is_outgoing: bool,
    pub message_type: String,
    pub content: String,
    pub file_data: Option<String>,
    pub file_name: Option<String>,
    pub file_mime: Option<String>,
    pub timestamp: i64,
}

#[derive(Serialize)]
pub struct InitRecoveryResult {
    pub words: Vec<String>,
    pub pubkey_hex: String,
}

#[derive(Serialize)]
pub struct ImportBackupResult {
    pub contacts_imported: u32,
    pub messages_imported: u32,
    pub has_recovery_key: bool,
}

// Chiffrement .zbc
/// Format binaire :
/// [4]  magic ZBC1
/// [1]  version
/// [32] salt_serpent  (Argon2id pour password1)
/// [32] salt_aes      (Argon2id pour password2)
/// [16] iv_serpent    (CBC IV)
/// [12] nonce_aes     (GCM nonce)
/// [16] tag_aes       (GCM auth tag)
/// [N]  ciphertext
fn zbc_encrypt(payload: &[u8], password1: &str, password2: &str) -> Result<Vec<u8>, String> {
    let salt_serpent: [u8; 32] = rand::random();
    let salt_aes: [u8; 32]     = rand::random();
    let iv_serpent: [u8; 16]   = rand::random();
    let nonce_aes: [u8; 12]    = rand::random();

    let hasher = Argon2idHasher::new()
        .map_err(|e| format!("Argon2id: {}", e))?;

    let key1_vec = hasher.derive_key(password1, &salt_serpent, 32)
        .map_err(|e| format!("Argon2id key1: {}", e))?;
    let key2_vec = hasher.derive_key(password2, &salt_aes, 32)
        .map_err(|e| format!("Argon2id key2: {}", e))?;
    let key2: [u8; 32] = key2_vec.try_into().unwrap();

    // Layer 1 : Serpent-256-CBC
    let layer1 = serpent_256_cbc_encrypt(payload, &key1_vec, &iv_serpent)
        .map_err(|e| format!("Serpent-CBC: {:?}", e))?;

    // Layer 2 : AES-256-GCM
    let mut enc = Aes256GcmEncryption::new(&key2, &nonce_aes, b"ZBC_BACKUP")
        .map_err(|e| format!("AES-GCM init: {:?}", e))?;
    let mut buf = layer1.clone();
    enc.encrypt(&mut buf);
    let tag = enc.compute_tag();

    let mut out = Vec::with_capacity(4 + 1 + 32 + 32 + 16 + 12 + 16 + buf.len());
    out.extend_from_slice(ZBC_MAGIC);
    out.push(ZBC_VERSION);
    out.extend_from_slice(&salt_serpent);
    out.extend_from_slice(&salt_aes);
    out.extend_from_slice(&iv_serpent);
    out.extend_from_slice(&nonce_aes);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&buf);
    Ok(out)
}

fn zbc_decrypt(data: &[u8], password1: &str, password2: &str) -> Result<Vec<u8>, String> {
    if data.len() < 4 + 1 + 32 + 32 + 16 + 12 + 16 {
        return Err("Fichier .zbc trop court".to_string());
    }
    if &data[..4] != ZBC_MAGIC {
        return Err("Fichier .zbc invalide (magic incorrect)".to_string());
    }

    let mut off = 5usize; // magic + version
    let salt_serpent = &data[off..off+32]; off += 32;
    let salt_aes     = &data[off..off+32]; off += 32;
    let iv_serpent   = &data[off..off+16]; off += 16;
    let nonce_aes    = &data[off..off+12]; off += 12;
    let tag_aes      = &data[off..off+16]; off += 16;
    let ciphertext   = &data[off..];

    let hasher = Argon2idHasher::new()
        .map_err(|e| format!("Argon2id: {}", e))?;

    let key1_vec = hasher.derive_key(password1, salt_serpent, 32)
        .map_err(|e| format!("Argon2id key1: {}", e))?;
    let key2_vec = hasher.derive_key(password2, salt_aes, 32)
        .map_err(|e| format!("Argon2id key2: {}", e))?;
    let key2: [u8; 32] = key2_vec.try_into().unwrap();

    // Déchiffre AES-256-GCM (layer 2)
    let mut dec = Aes256GcmDecryption::new(&key2, nonce_aes, b"ZBC_BACKUP")
        .map_err(|e| format!("AES-GCM init: {:?}", e))?;
    let mut layer1 = ciphertext.to_vec();
    dec.decrypt(&mut layer1);
    dec.verify_tag(tag_aes)
        .map_err(|_| "Mot de passe 2 incorrect ou fichier corrompu".to_string())?;

    // Déchiffre Serpent-256-CBC (layer 1)
    serpent_256_cbc_decrypt(&layer1, &key1_vec, iv_serpent)
        .map_err(|_| "Mot de passe 1 (mnémotechnique) incorrect".to_string())
}

// Gestion clé recovery
/// Génère la clé Dilithium2 de récupération depuis le seed du mnémotechnique.
/// La clé privée est stockée chiffrée dans les settings (jamais visible dans l'UI).
fn generate_and_store_recovery_key(
    session: &crate::session::CachedSession,
    mnemonic_entropy: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let (pk, sk) = dilithium2::keypair();
    let pubkey_bytes = pk.as_bytes().to_vec();
    let privkey_bytes = sk.as_bytes().to_vec();

    // Chiffre la clé privée avec ChaCha20 (même mécanisme que les messages)
    use crate::pages::chat::database::encryption::encrypt_message;
    let data_key = &session.password_hash;
    let enc_privkey = encrypt_message(&privkey_bytes, data_key, "recovery_dilithium_privkey")
        .map_err(|e| format!("Chiffrement clé recovery: {:?}", e))?;

    let _ = session.set_setting("recovery_dilithium_pubkey", &hex::encode(&pubkey_bytes));
    let _ = session.set_setting("recovery_dilithium_privkey_enc", &hex::encode(&enc_privkey));
    let _ = session.set_setting("recovery_mnemonic_entropy_enc", &hex::encode(mnemonic_entropy));

    Ok((pubkey_bytes, privkey_bytes))
}

fn load_recovery_privkey(session: &crate::session::CachedSession) -> Result<Vec<u8>, String> {
    use crate::pages::chat::database::encryption::decrypt_message;
    let enc_hex = session.get_setting("recovery_dilithium_privkey_enc")
        .unwrap_or(None)
        .ok_or("Clé recovery non initialisée")?;
    let enc = hex::decode(&enc_hex)
        .map_err(|_| "Clé recovery corrompue")?;
    decrypt_message(&enc, &session.password_hash, "recovery_dilithium_privkey")
        .map_err(|e| format!("Déchiffrement clé recovery: {:?}", e))
}

// Commandes Tauri
/// Initialise la clé de récupération et retourne les 24 mots mnémotechniques.
/// À appeler une seule fois à la création du compte (ou pour régénérer).
#[tauri::command]
pub async fn init_recovery_key(session_token: String) -> Result<InitRecoveryResult, String> {
    let session = get_session_by_token_async(session_token).await?;

    // Vérifie si déjà initialisé
    if let Ok(Some(pk)) = session.get_setting("recovery_dilithium_pubkey") {
        if !pk.is_empty() {
            return Err("ALREADY_INITIALIZED".to_string());
        }
    }

    let entropy: [u8; 32] = rand::random();
    let words = bip39::entropy_to_mnemonic(&entropy);
    let (pubkey, _) = generate_and_store_recovery_key(&session, &entropy)?;

    Ok(InitRecoveryResult {
        words,
        pubkey_hex: hex::encode(&pubkey),
    })
}

/// Vérifie que les mots saisis correspondent à la clé recovery stockée.
#[tauri::command]
pub async fn verify_recovery_words(
    session_token: String,
    words: Vec<String>,
) -> Result<bool, String> {
    let session = get_session_by_token_async(session_token).await?;

    let stored_enc = match session.get_setting("recovery_mnemonic_entropy_enc").unwrap_or(None) {
        Some(v) if !v.is_empty() => v,
        _ => return Ok(false),
    };
    let stored_entropy_bytes = hex::decode(&stored_enc).map_err(|_| "Entropy corrompue")?;
    if stored_entropy_bytes.len() != 32 { return Ok(false); }
    let mut stored_entropy = [0u8; 32];
    stored_entropy.copy_from_slice(&stored_entropy_bytes);

    let input_entropy = bip39::mnemonic_to_entropy(&words)?;
    Ok(stored_entropy == input_entropy)
}

/// Retourne les infos recovery sans exposer la clé privée.
#[tauri::command]
pub async fn get_recovery_status(session_token: String) -> Result<serde_json::Value, String> {
    let session = get_session_by_token_async(session_token).await?;
    let has_key = session.get_setting("recovery_dilithium_pubkey")
        .unwrap_or(None)
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    Ok(serde_json::json!({ "initialized": has_key }))
}

/// Exporte un backup .zbc chiffré.
///
/// - `password1` : phrase mnémotechnique (24 mots séparés par des espaces)
/// - `password2` : mot de passe supplémentaire choisi par l'utilisateur
/// - `friend_ids` : IDs des amis dont on exporte le profil
/// - `message_friend_ids` : sous-ensemble de friend_ids dont on exporte aussi les messages
#[tauri::command]
pub async fn export_backup(
    session_token: String,
    password1: String,
    password2: String,
    friend_ids: Vec<i64>,
    message_friend_ids: Vec<i64>,
) -> Result<Vec<u8>, String> {
    let session = get_session_by_token_async(session_token).await?;

    // Valide le mnémotechnique (password1 = 24 mots espace-séparés)
    let words: Vec<String> = password1.split_whitespace().map(|s| s.to_string()).collect();
    bip39::mnemonic_to_entropy(&words)
        .map_err(|_| "Phrase mnémotechnique incorrecte (password1 doit être 24 mots)".to_string())?;

    // Clé recovery
    let pubkey_hex = session.get_setting("recovery_dilithium_pubkey")
        .unwrap_or(None)
        .unwrap_or_default();
    let privkey_bytes = load_recovery_privkey(&session)?;
    let privkey_hex = hex::encode(&privkey_bytes);

    // Contacts sélectionnés
    let include_msgs: std::collections::HashSet<i64> =
        message_friend_ids.iter().copied().collect();

    let mut contacts: Vec<ZbcContact> = Vec::new();

    for fid in &friend_ids {
        let friend = session.get_friend(*fid)
            .map_err(|e| format!("Ami introuvable (id={}): {}", fid, e))?;

        use base64::Engine as _;
        let enc = base64::engine::general_purpose::STANDARD;
        let to_b64 = |v: Option<Vec<u8>>| v.map(|b| enc.encode(&b));

        let messages = if include_msgs.contains(fid) {
            load_messages_for_export(&session, *fid)?
        } else {
            vec![]
        };

        contacts.push(ZbcContact {
            username_hash: friend.username_hash.clone(),
            pseudo: friend.pseudo.clone(),
            identity_key_public: enc.encode(&friend.identity_key_public),
            kyber_public_key: to_b64(friend.kyber_public_key.clone()),
            x25519_public_key: to_b64(friend.x25519_public_key.clone()),
            friendship_signature_local: to_b64(friend.friendship_signature_local.clone()),
            friendship_signature_remote: to_b64(friend.friendship_signature_remote.clone()),
            messages,
        });
    }

    let payload = ZbcPayload {
        version: ZBC_VERSION,
        created_at: crate::utils::timestamp::plateform::current_timestamp() as i64,
        username_hash: session.user_hash_hex.clone(),
        recovery_dilithium_pubkey: pubkey_hex,
        recovery_dilithium_privkey: privkey_hex,
        contacts,
    };

    let json = serde_json::to_vec(&payload)
        .map_err(|e| format!("Sérialisation: {}", e))?;

    zbc_encrypt(&json, &password1, &password2)
}

/// Importe un backup .zbc.
#[tauri::command]
pub async fn import_backup(
    session_token: String,
    data: Vec<u8>,
    password1: String,
    password2: String,
) -> Result<ImportBackupResult, String> {
    let session = get_session_by_token_async(session_token).await?;

    let json = zbc_decrypt(&data, &password1, &password2)?;
    let payload: ZbcPayload = serde_json::from_slice(&json)
        .map_err(|e| format!("Déserialisation: {}", e))?;

    let mut contacts_imported = 0u32;
    let mut messages_imported = 0u32;
    let user_db = session.get_user_db()
        .map_err(|e| format!("DB: {}", e))?;

    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;

    for contact in &payload.contacts {
        // Insère le contact s'il n'existe pas
        let already: bool = user_db.conn().query_row(
            "SELECT COUNT(*) FROM friends WHERE username_hash = ?1",
            [&contact.username_hash],
            |row| row.get::<_, i64>(0),
        ).map(|c| c > 0).unwrap_or(false);

        let ik = match b64.decode(&contact.identity_key_public) {
            Ok(b) => b, Err(_) => continue,
        };
        let decode_opt = |s: &Option<String>| -> Option<Vec<u8>> {
            s.as_ref().and_then(|b| b64.decode(b).ok())
        };

        let now = crate::utils::timestamp::plateform::current_timestamp() as i64;

        if !already {
            let _ = user_db.conn().execute(
                "INSERT OR IGNORE INTO friends
                 (pseudo, username_hash, identity_key_public, kyber_public_key,
                  x25519_public_key, friendship_signature_local,
                  friendship_signature_remote, verified, blocked, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,0,0,?8,?8)",
                rusqlite::params![
                    contact.pseudo, contact.username_hash, ik,
                    decode_opt(&contact.kyber_public_key),
                    decode_opt(&contact.x25519_public_key),
                    decode_opt(&contact.friendship_signature_local),
                    decode_opt(&contact.friendship_signature_remote),
                    now,
                ],
            );
            contacts_imported += 1;
        }

        // Récupère l'ID ami
        let friend_id: Option<i64> = user_db.conn().query_row(
            "SELECT id FROM friends WHERE username_hash = ?1",
            [&contact.username_hash],
            |row| row.get(0),
        ).ok();
        let friend_id = match friend_id { Some(id) => id, None => continue };

        // Importe les messages
        for msg in &contact.messages {
            let mid_bytes = match hex::decode(&msg.message_id) {
                Ok(b) => b, Err(_) => continue,
            };
            let already_msg: bool = user_db.conn().query_row(
                "SELECT COUNT(*) FROM messages WHERE message_id = ?1",
                rusqlite::params![mid_bytes],
                |row| row.get::<_, i64>(0),
            ).map(|c| c > 0).unwrap_or(false);
            if already_msg { continue; }

            // Reconstruit inner_bytes depuis le contenu plaintext
            use prost::Message;
            use zenth_dto::{InnerMessage, InnerMessageType};
            let inner = InnerMessage {
                r#type: InnerMessageType::InnerText as i32,
                text: msg.content.clone(),
                file_data: msg.file_data.as_ref()
                    .and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok())
                    .unwrap_or_default(),
                file_name: msg.file_name.clone().unwrap_or_default(),
                file_mime: msg.file_mime.clone().unwrap_or_default(),
                reply_to_id: String::new(),
                file_offer: None,
            };
            let inner_bytes = inner.encode_to_vec();

            use crate::pages::chat::database::encryption::encrypt_message;
            let enc = match encrypt_message(&inner_bytes, &session.password_hash, &msg.message_id) {
                Ok(e) => e, Err(_) => continue,
            };

            let _ = user_db.conn().execute(
                "INSERT OR IGNORE INTO messages
                 (friend_id, message_id, is_outgoing, message_type,
                  encrypted_content, content_iv, timestamp, status, vault_encrypted)
                 VALUES (?1,?2,?3,?4,?5,X'',?6,'delivered',0)",
                rusqlite::params![
                    friend_id, msg.message_id, msg.is_outgoing as i64,
                    msg.message_type, enc, msg.timestamp,
                ],
            );
            messages_imported += 1;
        }
    }

    // Restaure la clé de récupération si absente sur cet appareil
    let has_recovery = !payload.recovery_dilithium_privkey.is_empty();
    if has_recovery {
        let current = session.get_setting("recovery_dilithium_pubkey")
            .unwrap_or(None).unwrap_or_default();
        if current.is_empty() {
            if let Ok(privkey_bytes) = hex::decode(&payload.recovery_dilithium_privkey) {
                use crate::pages::chat::database::encryption::encrypt_message;
                if let Ok(enc) = encrypt_message(
                    &privkey_bytes, &session.password_hash, "recovery_dilithium_privkey"
                ) {
                    let _ = session.set_setting("recovery_dilithium_pubkey",
                        &payload.recovery_dilithium_pubkey);
                    let _ = session.set_setting("recovery_dilithium_privkey_enc",
                        &hex::encode(&enc));
                }
            }
        }
    }

    Ok(ImportBackupResult { contacts_imported, messages_imported, has_recovery_key: has_recovery })
}

// Helpers
fn load_messages_for_export(
    session: &crate::session::CachedSession,
    friend_id: i64,
) -> Result<Vec<ZbcMessage>, String> {
    use crate::pages::chat::database::encryption::decrypt_message;
    use prost::Message;
    use zenth_dto::InnerMessage;

    let user_db = session.get_user_db()
        .map_err(|e| format!("DB: {}", e))?;

    let rows: Vec<(Vec<u8>, Vec<u8>, i32, String, i64, i32)> = user_db.conn()
        .prepare(
            "SELECT message_id, encrypted_content, is_outgoing, message_type,
                    timestamp, vault_encrypted
             FROM messages
             WHERE friend_id = ?1 AND deleted = 0
             ORDER BY timestamp ASC"
        )
        .and_then(|mut stmt| {
            let r: rusqlite::Result<Vec<_>> = stmt.query_map([friend_id], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i32>(5)?,
                ))
            })?.collect();
            r
        })
        .map_err(|e| format!("Query messages: {}", e))?;

    let mut result = Vec::new();
    for (mid_bytes, enc, outgoing_i, mt, ts, vault_enc_i) in rows {
        let mid_hex = hex::encode(&mid_bytes);
        let outgoing = outgoing_i != 0;
        let vault_enc = vault_enc_i != 0;

        // Déchiffre couche ChaCha20 → puis couche vault si nécessaire
        let inner_bytes = match crate::pages::vault::decrypt_with_vault_layer(
            &enc, &session.password_hash, &mid_hex, vault_enc, "",
        ) {
            Ok(b) => b,
            Err(_) => continue, // vault verrouillé → skip
        };

        // Décode InnerMessage protobuf
        let inner = match InnerMessage::decode(inner_bytes.as_slice()) {
            Ok(m) => m,
            Err(_) => continue,
        };

        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;
        result.push(ZbcMessage {
            message_id: mid_hex,
            is_outgoing: outgoing,
            message_type: mt,
            content: inner.text,
            file_data: if inner.file_data.is_empty() { None } else { Some(b64.encode(&inner.file_data)) },
            file_name: if inner.file_name.is_empty() { None } else { Some(inner.file_name) },
            file_mime: if inner.file_mime.is_empty() { None } else { Some(inner.file_mime) },
            timestamp: ts,
        });
    }
    Ok(result)
}

// Commandes DHT
/// Publie la clé recovery Dilithium2 sur le DHT (méthode 29).
/// À appeler une fois après `init_recovery_key`, quand la session est active.
#[tauri::command]
pub async fn publish_recovery_key_dht(session_token: String) -> Result<(), String> {
    let session = get_session_by_token_async(session_token).await?;

    let recovery_pubkey_hex = session
        .get_setting("recovery_dilithium_pubkey")
        .unwrap_or(None)
        .ok_or("Clé recovery non initialisée")?;
    if recovery_pubkey_hex.is_empty() {
        return Err("Clé recovery non initialisée".to_string());
    }
    let recovery_pubkey = hex::decode(&recovery_pubkey_hex)
        .map_err(|_| "recovery_dilithium_pubkey corrompu")?;

    // Signed message : username_hash || recovery_pubkey || timestamp (8 bytes LE)
    let ts = crate::utils::timestamp::plateform::current_timestamp();
    let mut signed_data = Vec::with_capacity(32 + recovery_pubkey.len() + 8);
    signed_data.extend_from_slice(&session.user_hash);
    signed_data.extend_from_slice(&recovery_pubkey);
    signed_data.extend_from_slice(&ts.to_le_bytes());

    let dilithium_sk = dilithium2::SecretKey::from_bytes(&session.dilithium_secret)
        .map_err(|_| "Clé Dilithium principale invalide")?;
    let sig = dilithium2::detached_sign(&signed_data, &dilithium_sk);

    let base_url = crate::config::dht_api_url();
    let config = RegisterConfig { base_url, darknet: DarknetType::Http, ..RegisterConfig::default() };
    let client = RecoveryApiClient::new(config).await
        .map_err(|e| format!("Client DHT: {:?}", e))?;

    let resp = client.publish_recovery_key(
        session.user_hash.clone(),
        recovery_pubkey,
        sig.as_bytes().to_vec(),
    ).await.map_err(|e| format!("DHT method 29: {:?}", e))?;

    if resp.success {
        Ok(())
    } else {
        Err(resp.error_message)
    }
}

/// Résultat d'un claim de récupération
#[derive(Serialize)]
pub struct RecoveryClaimResult {
    pub session_token_hex: String,
}

/// Soumet un claim de récupération (méthode 30) depuis un fichier .zbc.
///
/// Cette commande est appelée SANS session active (l'utilisateur a perdu son appareil).
/// Elle :
///   1. Déchiffre le .zbc pour obtenir la clé recovery et le username_hash
///   2. Génère une nouvelle paire de clés Dilithium2 principale
///   3. Signe le claim avec la clé recovery
///   4. Soumet sur le DHT → récupère un nouveau session_token
#[tauri::command]
pub async fn submit_recovery_claim(
    zbc_data: Vec<u8>,
    password1: String,   // phrase mnémotechnique (24 mots)
    password2: String,   // mot de passe de protection
    new_pre_key_bundle: Vec<u8>,  // bundle X3DH sérialisé (généré en amont par le keygen)
    new_identity_pubkey_hex: String,  // hex de la nouvelle clé Dilithium2 principale
    new_identity_privkey_hex: String, // hex du secret (pour signer new_identity_signature)
) -> Result<RecoveryClaimResult, String> {
    // 1. Déchiffre le .zbc
    let words: Vec<String> = password1.split_whitespace().map(|s| s.to_string()).collect();
    let p1_sentence = words.join(" ");
    let json = zbc_decrypt(&zbc_data, &p1_sentence, &password2)?;
    let payload: ZbcPayload = serde_json::from_slice(&json)
        .map_err(|e| format!("Déserialisation: {}", e))?;

    let username_hash = hex::decode(&payload.username_hash)
        .map_err(|_| "username_hash invalide dans .zbc")?;

    let recovery_privkey_bytes = hex::decode(&payload.recovery_dilithium_privkey)
        .map_err(|_| "recovery_dilithium_privkey invalide")?;
    let new_identity_pubkey = hex::decode(&new_identity_pubkey_hex)
        .map_err(|_| "new_identity_pubkey_hex invalide")?;
    let new_identity_privkey = hex::decode(&new_identity_privkey_hex)
        .map_err(|_| "new_identity_privkey_hex invalide")?;

    // 2. Signe avec la nouvelle clé principale : username_hash || timestamp
    let ts = crate::utils::timestamp::plateform::current_timestamp();
    let mut ident_signed = Vec::with_capacity(32 + 8);
    ident_signed.extend_from_slice(&username_hash);
    ident_signed.extend_from_slice(&ts.to_le_bytes());

    let new_sk = dilithium2::SecretKey::from_bytes(&new_identity_privkey)
        .map_err(|_| "Clé principale invalide")?;
    let new_identity_sig = dilithium2::detached_sign(&ident_signed, &new_sk);

    // 3. Signe le claim avec la clé recovery : username_hash || new_identity_pubkey || timestamp
    let mut recovery_signed = Vec::with_capacity(32 + new_identity_pubkey.len() + 8);
    recovery_signed.extend_from_slice(&username_hash);
    recovery_signed.extend_from_slice(&new_identity_pubkey);
    recovery_signed.extend_from_slice(&ts.to_le_bytes());

    let recovery_sk = dilithium2::SecretKey::from_bytes(&recovery_privkey_bytes)
        .map_err(|_| "Clé recovery invalide")?;
    let recovery_sig = dilithium2::detached_sign(&recovery_signed, &recovery_sk);

    // 4. Soumet sur le DHT
    let base_url = crate::config::dht_api_url();
    let config = RegisterConfig { base_url, darknet: DarknetType::Http, ..RegisterConfig::default() };
    let client = RecoveryApiClient::new(config).await
        .map_err(|e| format!("Client DHT: {:?}", e))?;

    let resp = client.recovery_claim(
        username_hash,
        new_identity_pubkey,
        new_identity_sig.as_bytes().to_vec(),
        new_pre_key_bundle,
        recovery_sig.as_bytes().to_vec(),
    ).await.map_err(|e| format!("DHT method 30: {:?}", e))?;

    if !resp.success {
        return Err(resp.error_message);
    }

    Ok(RecoveryClaimResult {
        session_token_hex: hex::encode(&resp.session_token),
    })
}
