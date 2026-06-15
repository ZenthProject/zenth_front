use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{SecretKey, DetachedSignature};
use prost::Message;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::user::NewMessage as DbNewMessage;
use crate::session::{get_session, get_session_async, get_session_by_token_async};
use crate::pages::chat::crypto::sessions::{
    Session, PreKeyBundleData, X3DHCompleteData, X3DHInitResult,
    get_or_create_session, has_session, save_session_state, SessionResult, SessionError,
    EncryptedMessageData,
};
use crate::pages::chat::database::queries::{
    load_session as db_load_session, save_session as db_save_session,
    Session as DbSession,
};
use crate::utils::timestamp::plateform::current_timestamp;
use zenth_crypto::protocols;
use zenth_crypto::protocols::EncryptedMessage as ProtocolsEncryptedMessage;
use crate::pages::chat::database::encryption::{encrypt_message, decrypt_message};
use crate::api::prekeys::{PreKeyApiClient, PreKeyConfig};
use crate::db::user::UserDb;
use zenth_dto::{
    DhtRequest, DhtResponse, Method,
    ZenthSignalEnvelope, EncryptedMessageBody, RatchetHeader, PreKeyMessage,
    FetchMessagesRequest, FetchMessagesResponse,
    SendMessageResponse, PreKeyBundle,
    InnerMessage, InnerMessageType,
};
use zenth_dto::zenth_signal_envelope::Content;
use crate::utils::sanitizer::parser::{FileParser, FileType};

struct ParsedInner {
    display_content: String,
    message_type: String,
    filename: Option<String>,
    mime_type: Option<String>,
    file_size: Option<i64>,
    file_data_b64: Option<String>,
    reply_to_id: Option<String>,
}

fn mime_to_message_type(mime: &str) -> &'static str {
    if mime.starts_with("image/") { "image" }
    else if mime.starts_with("audio/") { "audio" }
    else if mime.starts_with("video/") { "video" }
    else if mime == "application/pdf" { "pdf" }
    else { "file" }
}

// La colonne message_type en DB n'accepte pas 'pdf' (CHECK constraint).
// On stocke 'file' pour les PDFs ; le type affiché ('pdf') est recalculé
// depuis le MIME lors de la lecture via parse_inner_message.
fn db_message_type(t: &str) -> &str {
    if t == "pdf" { "file" } else { t }
}

/// Version publique de parse_inner_message pour le module sync.
/// Retourne (message_type_db, filename).
pub fn parse_inner_for_sync(bytes: &[u8]) -> (String, Option<String>) {
    let parsed = parse_inner_message(bytes);
    (db_message_type(&parsed.message_type).to_string(), parsed.filename)
}

fn parse_inner_message(bytes: &[u8]) -> ParsedInner {
    match InnerMessage::decode(bytes) {
        Ok(inner) if inner.r#type == InnerMessageType::InnerFile as i32 => {
            use base64::Engine;
            let file_size = inner.file_data.len() as i64;
            let data_b64 = base64::engine::general_purpose::STANDARD.encode(&inner.file_data);
            let message_type = mime_to_message_type(&inner.file_mime).to_string();
            let reply_to_id = if inner.reply_to_id.is_empty() { None } else { Some(inner.reply_to_id.clone()) };
            ParsedInner {
                display_content: inner.file_name.clone(),
                message_type,
                filename: Some(inner.file_name),
                mime_type: Some(inner.file_mime),
                file_size: Some(file_size),
                file_data_b64: Some(data_b64),
                reply_to_id,
            }
        }
        Ok(inner) => {
            let reply_to_id = if inner.reply_to_id.is_empty() { None } else { Some(inner.reply_to_id.clone()) };
            ParsedInner {
                display_content: inner.text,
                message_type: "text".to_string(),
                filename: None,
                mime_type: None,
                file_size: None,
                file_data_b64: None,
                reply_to_id,
            }
        }
        Err(_) => {
            // Fallback: legacy plain text (messages sent before this format)
            ParsedInner {
                display_content: String::from_utf8_lossy(bytes).to_string(),
                message_type: "text".to_string(),
                filename: None,
                mime_type: None,
                file_size: None,
                file_data_b64: None,
                reply_to_id: None,
            }
        }
    }
}

/// Sanitise un message entrant déchiffré.
/// Pour les fichiers : vérifie les magic bytes (ignore le MIME déclaré par l'expéditeur)
/// et passe les données via zenth_protect. Retourne Err si le fichier est invalide ou malveillant.
/// Pour les textes : passe sans modification.
fn sanitize_incoming_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let inner = match InnerMessage::decode(bytes) {
        Ok(m) => m,
        Err(_) => return Ok(bytes.to_vec()), // legacy plain text, pas de fichier
    };

    if inner.r#type != InnerMessageType::InnerFile as i32 {
        return Ok(bytes.to_vec()); // texte - rien à sanitiser
    }

    // Vérification par magic bytes (pas le MIME de l'expéditeur)
    // Si la signature est inconnue → rejet (fichier potentiellement malveillant)
    // Si la signature est connue mais que la sanitisation échoue → on passe brut
    // (ex: PDF linearisé, format inhabituel - l'expéditeur a déjà sanitisé)
    let file_data = match FileParser::parse_by_signature(&inner.file_data) {
        Ok(sanitized) => sanitized,
        Err(_) => {
            let mime = inner.file_mime.to_lowercase();

            // Format identifiable par magic bytes mais sanitiseur en échec
            // (JPEG inhabituels, HEIC détecté comme MP4, WebP, etc.).
            // Si on reconnaît le type par signature, on laisse passer tel quel :
            // l'expéditeur a déjà sanitisé et le viewer est sandboxé.
            if FileType::from_signature(&inner.file_data).is_ok() {
                inner.file_data.clone()
            } else {
                // Format totalement inconnu du sanitiseur.
                // On accepte uniquement audio/vidéo dont les magic bytes correspondent au MIME.
                let is_safe_media = mime.starts_with("audio/") || mime.starts_with("video/");
                let matches_mime = if mime.contains("webm") || mime.contains("ogg") || mime.contains("opus") {
                    inner.file_data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) // WebM/MKV
                    || inner.file_data.starts_with(b"OggS")                  // OGG
                } else if mime.contains("mp4") || mime.contains("m4a") {
                    inner.file_data.len() > 8 && &inner.file_data[4..8] == b"ftyp"
                } else if mime.starts_with("image/") {
                    // HEIC/AVIF/WebP : magic bytes spécifiques
                    (inner.file_data.len() > 12 && &inner.file_data[4..8] == b"ftyp") // HEIC/AVIF
                    || (inner.file_data.len() > 12 && &inner.file_data[0..4] == b"RIFF" && &inner.file_data[8..12] == b"WEBP") // WebP
                } else {
                    false
                };
                if is_safe_media && matches_mime {
                    inner.file_data.clone()
                } else if mime.starts_with("image/") && matches_mime {
                    inner.file_data.clone()
                } else {
                    return Err(format!("Fichier rejeté : format non reconnu ou MIME incohérent ({})", inner.file_mime));
                }
            }
        }
    };

    let clean = InnerMessage {
        r#type: InnerMessageType::InnerFile as i32,
        text: String::new(),
        file_data,
        file_name: inner.file_name,
        file_mime: inner.file_mime,
        reply_to_id: inner.reply_to_id,
        file_offer: None,
    };
    Ok(clean.encode_to_vec())
}

use zenth_requests::{
    implementations::RequestsNetwork,
    request::Request,
    transports::Transport,
};

// ============================================================================
// Helper functions for Double Ratchet integration
// ============================================================================

/// Convert PreKeyBundle from proto to our internal format and create X3DH session
///
/// This function bypasses the signature verification in the protocols library
/// because Zenth uses Dilithium signatures (verified by DHT server) instead of Ed25519.
///
/// IMPORTANT: In this system, the X25519 identity key IS the signed prekey.
/// We use bundle.signed_pre_key_public for both to ensure consistency,
/// as friend.x25519_public_key might be stale from an old friend exchange.
///
/// Returns the session data needed for Double Ratchet initialization.
fn create_session_from_proto_bundle(
    bundle: &PreKeyBundle,
    _friend_x25519_key: Option<&[u8]>, // Ignored - we use signed_pre_key_public instead
    our_x25519_private: &[u8; 32],
) -> Result<(Session, X3DHInitResult), String> {
    use x25519_dalek::{StaticSecret, PublicKey};
    use rand08::rngs::OsRng;
    use sha2::{Sha256, Digest};

    // Get their signed prekey (this IS their X25519 identity key in this system)
    let their_signed_prekey: [u8; 32] = bundle.signed_pre_key_public.as_slice()
        .try_into()
        .map_err(|_| format!("Invalid signed prekey length: {}", bundle.signed_pre_key_public.len()))?;

    // In this system, identity key = signed prekey (same X25519 key)
    // This ensures we always use the fresh key from the server bundle
    let their_identity_bytes: [u8; 32] = their_signed_prekey;

    // Get their one-time prekey if available
    let their_one_time_prekey: Option<[u8; 32]> = if !bundle.pre_key_public.is_empty() && bundle.pre_key_id > 0 {
        Some(bundle.pre_key_public.as_slice()
            .try_into()
            .map_err(|_| format!("Invalid one-time prekey length: {}", bundle.pre_key_public.len()))?)
    } else {
        None
    };

    // Create our identity key pair
    let our_identity_private = StaticSecret::from(*our_x25519_private);

    // Generate ephemeral key pair for this session (using StaticSecret for multiple DH)
    let ephemeral_bytes: [u8; 32] = rand08::random();
    let ephemeral_secret = StaticSecret::from(ephemeral_bytes);
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    // Convert their keys to x25519-dalek types
    let their_identity = PublicKey::from(their_identity_bytes);
    let their_signed_prekey_pub = PublicKey::from(their_signed_prekey);

    // Perform X3DH key agreement (Alice side)
    // DH1 = DH(IK_A, SPK_B)
    let dh1 = our_identity_private.diffie_hellman(&their_signed_prekey_pub);

    // DH2 = DH(EK_A, IK_B)
    let dh2 = ephemeral_secret.diffie_hellman(&their_identity);

    // DH3 = DH(EK_A, SPK_B)
    let dh3 = ephemeral_secret.diffie_hellman(&their_signed_prekey_pub);

    // DH4 = DH(EK_A, OPK_B) if one-time prekey is used
    let dh4 = their_one_time_prekey.map(|opk| {
        let opk_pub = PublicKey::from(opk);
        ephemeral_secret.diffie_hellman(&opk_pub)
    });

    // Derive the shared secret using HKDF
    // SK = KDF(DH1 || DH2 || DH3 || DH4)
    let mut ikm = Vec::with_capacity(128);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());
    if let Some(ref dh4_secret) = dh4 {
        ikm.extend_from_slice(dh4_secret.as_bytes());
    }

    // Use simple SHA256 for root key derivation (simplified HKDF)
    let mut hasher = Sha256::new();
    hasher.update(b"ZenthX3DH");
    hasher.update(&ikm);
    let root_key: [u8; 32] = hasher.finalize().into();

    // Get our public key for comparison
    let our_identity_pub = x25519_dalek::PublicKey::from(&our_identity_private);

    // Initialize session state using protocols library
    let session_state = protocols::SessionState::initialize_alice(
        &root_key,
        &their_signed_prekey,
        &mut OsRng,
    ).map_err(|e| format!("Failed to initialize session state: {:?}", e))?;

    let init_result = X3DHInitResult {
        ephemeral_public: hex::encode(ephemeral_public.as_bytes()),
        one_time_prekey_id: if bundle.pre_key_id > 0 { Some(bundle.pre_key_id) } else { None },
    };

    let session = Session::from_state(session_state);

    Ok((session, init_result))
}

fn create_bob_session_manual(
    our_x25519_private: &[u8; 32],
    our_signed_prekey_private: &[u8; 32],
    our_one_time_prekey: Option<(u32, [u8; 32])>,
    their_identity_key: &[u8],
    their_ephemeral_key: &[u8],
) -> Result<Session, String> {
    use x25519_dalek::{StaticSecret, PublicKey};
    use rand08::rngs::OsRng;
    use sha2::{Sha256, Digest};

    // Parse their keys
    let their_identity: [u8; 32] = their_identity_key.try_into()
        .map_err(|_| format!("Invalid identity key length: {}", their_identity_key.len()))?;
    let their_ephemeral: [u8; 32] = their_ephemeral_key.try_into()
        .map_err(|_| format!("Invalid ephemeral key length: {}", their_ephemeral_key.len()))?;

    let our_identity_private = StaticSecret::from(*our_x25519_private);
    let our_signed_prekey_secret = StaticSecret::from(*our_signed_prekey_private);

    let their_identity_pub = PublicKey::from(their_identity);
    let their_ephemeral_pub = PublicKey::from(their_ephemeral);

    let dh1 = our_signed_prekey_secret.diffie_hellman(&their_identity_pub);

    let dh2 = our_identity_private.diffie_hellman(&their_ephemeral_pub);

    let dh3 = our_signed_prekey_secret.diffie_hellman(&their_ephemeral_pub);

    let dh4 = our_one_time_prekey.map(|(_, opk_private)| {
        let opk_secret = StaticSecret::from(opk_private);
        opk_secret.diffie_hellman(&their_ephemeral_pub)
    });

    let mut ikm = Vec::with_capacity(128);
    ikm.extend_from_slice(dh1.as_bytes());
    ikm.extend_from_slice(dh2.as_bytes());
    ikm.extend_from_slice(dh3.as_bytes());
    if let Some(ref dh4_secret) = dh4 {
        ikm.extend_from_slice(dh4_secret.as_bytes());
    }

    // Use the same SHA256 derivation as Alice
    let mut hasher = Sha256::new();
    hasher.update(b"ZenthX3DH");
    hasher.update(&ikm);
    let root_key: [u8; 32] = hasher.finalize().into();

    // Get public keys for debug comparison with sender side
    let our_identity_pub = PublicKey::from(&our_identity_private);
    let our_spk_pub = PublicKey::from(&our_signed_prekey_secret);

    let our_signed_prekey_pub = PublicKey::from(&our_signed_prekey_secret);

    let signed_prekey_keypair = protocols::KeyPair::from_private_key(*our_signed_prekey_private);
    let session_state = protocols::SessionState::initialize_bob(
        &root_key,
        signed_prekey_keypair.private_key.clone(),
    ).map_err(|e| format!("Failed to initialize Bob session state: {:?}", e))?;


    Ok(Session::from_state(session_state))
}

fn create_ratchet_header(msg_data: &EncryptedMessageData) -> RatchetHeader {
    RatchetHeader {
        sender_ratchet_key: hex::decode(&msg_data.public_key).unwrap_or_default(),
        previous_counter: msg_data.previous_chain_length,
        counter: msg_data.message_number,
        pq_ciphertext: vec![],
    }
}

fn parse_ratchet_header(header: &RatchetHeader, ciphertext: &[u8], tag: &[u8]) -> EncryptedMessageData {
    EncryptedMessageData {
        version: 1,
        public_key: hex::encode(&header.sender_ratchet_key),
        previous_chain_length: header.previous_counter,
        message_number: header.counter,
        ciphertext: ciphertext.to_vec(),
        tag: hex::encode(tag),
    }
}

async fn fetch_friend_prekey_bundle(
    our_hash: &[u8],
    friend_hash: &[u8],
    dilithium_secret: &[u8],
) -> Result<PreKeyBundle, String> {
    let timestamp = current_timestamp();

    let mut message_to_sign = Vec::new();
    message_to_sign.extend_from_slice(our_hash);
    message_to_sign.extend_from_slice(friend_hash);
    message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

    let signature = sign_with_dilithium2(dilithium_secret, &message_to_sign)?;

    let config = PreKeyConfig::default();
    let client = PreKeyApiClient::new(config)
        .await
        .map_err(|e| format!("Failed to create API client: {:?}", e))?;

    let response = client.fetch_prekey_bundle(
        our_hash.to_vec(),
        friend_hash.to_vec(),
        signature,
    ).await.map_err(|e| format!("Failed to fetch prekey bundle: {:?}", e))?;

    response.bundle.ok_or("No prekey bundle in response".to_string())
}

fn decrypt_regular_message(
    msg: &EncryptedMessageBody,
    user_db: &UserDb,
    friend_id: i64,
    data_key: &[u8],
    our_x25519_private: &[u8; 32],
) -> Result<(Vec<u8>, Session), String> {
    let ratchet_header = msg.ratchet_header.as_ref()
        .ok_or("Missing ratchet header in regular message")?;

    let layers = msg.encrypted_layers.as_ref()
        .ok_or("Missing encrypted layers")?;

    let enc_data = parse_ratchet_header(ratchet_header, &layers.ciphertext, &layers.hmac);

    let db_session = db_load_session(user_db, friend_id, data_key)
        .map_err(|e| format!("Failed to load session: {:?}", e))?
        .ok_or("No session found for this friend")?;

    let mut session = Session::from_db_session_raw(&db_session, our_x25519_private)
        .map_err(|e| format!("Failed to reconstruct session: {:?}", e))?;

    let encrypted_msg = ProtocolsEncryptedMessage::try_from(&enc_data)
        .map_err(|e| format!("Failed to parse encrypted message: {:?}", e))?;

    let plaintext = session.decrypt(&encrypted_msg)
        .map_err(|e| format!("Double Ratchet decryption failed: {:?}", e))?;

    Ok((plaintext, session))
}

fn decrypt_prekey_message(
    prekey_msg: &PreKeyMessage,
    user_db: &UserDb,
    _friend_id: i64,
    _data_key: &[u8],
    our_x25519_private: &[u8; 32],
    session_obj: &std::sync::Arc<crate::session::CachedSession>,
) -> Result<(Vec<u8>, Session), String> {

    let msg_body = prekey_msg.message.as_ref()
        .ok_or("Missing message body in prekey message")?;

    let sender_identity = prekey_msg.identity_key.as_ref()
        .ok_or("Missing sender identity key")?;

    let signed_prekey_data = match session_obj.get_signed_prekey(prekey_msg.signed_pre_key_id) {
        Ok(data) => {
            data
        }
        Err(e) => {
            return Err(format!("Failed to get signed prekey: {}", e));
        }
    };

    let one_time_prekey = if prekey_msg.pre_key_id > 0 {
        session_obj.get_one_time_prekey(prekey_msg.pre_key_id)
            .ok()
            .flatten()
    } else {
        None
    };

    let x3dh_data = X3DHCompleteData {
        their_identity_key: hex::encode(&sender_identity.public_key),
        their_ephemeral_key: hex::encode(&prekey_msg.base_key),
        one_time_prekey_id: if prekey_msg.pre_key_id > 0 { Some(prekey_msg.pre_key_id) } else { None },
    };

    let mut session = create_bob_session_manual(
        &signed_prekey_data,    // Use signed prekey as identity (they're the same in this system)
        &signed_prekey_data,    // Signed prekey private
        one_time_prekey,
        &sender_identity.public_key,
        &prekey_msg.base_key,
    )?;

    if prekey_msg.pre_key_id > 0 {
        let _ = crate::pages::chat::database::queries::mark_prekey_used(user_db, prekey_msg.pre_key_id);
    }

    let ratchet_header = msg_body.ratchet_header.as_ref()
        .ok_or("Missing ratchet header in prekey message body")?;

    let layers = msg_body.encrypted_layers.as_ref()
        .ok_or("Missing encrypted layers")?;

    let enc_data = parse_ratchet_header(ratchet_header, &layers.ciphertext, &layers.hmac);
    let encrypted_msg = ProtocolsEncryptedMessage::try_from(&enc_data)
        .map_err(|e| format!("Failed to parse encrypted message: {:?}", e))?;

    let plaintext = session.decrypt(&encrypted_msg)
        .map_err(|e| format!("Double Ratchet decryption failed: {:?}", e))?;

    Ok((plaintext, session))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
    pub id: i64,
    pub message_id: String,
    pub friend_id: i64,
    pub content: String,
    pub is_outgoing: bool,
    pub timestamp: i64,
    pub status: String,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub message_type: String,
    pub file_name: Option<String>,
    pub file_mime: Option<String>,
    pub file_data: Option<String>, // base64 encoded
    pub reply_to_id: Option<String>,
}

/// Result of message sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSyncResult {
    pub new_messages: u32,
    pub errors: Vec<String>,
    /// IDs des amis qui ont reçu au moins un nouveau message
    pub updated_friend_ids: Vec<i64>,
}

/// Retourne le TTL DHT configuré pour cette conversation (0 = jamais).
#[tauri::command]
pub async fn get_chat_ttl(session_token: String, friend_id: i64) -> Result<u32, String> {
    let session = get_session_by_token_async(session_token).await?;
    session.with_db(|conn| {
        conn.query_row(
            "SELECT ttl_hours FROM chat_settings WHERE friend_id = ?1",
            [friend_id],
            |row| row.get::<_, u32>(0),
        )
        .or_else(|_| Ok(0)) // pas encore de réglage → 0 (jamais)
    })
}

/// Définit le TTL DHT pour une conversation.
/// ttl_hours = 0 → pas d'expiration ; 24 → 24h ; 48 → 48h ; 168 → 7j ; 720 → 30j
#[tauri::command]
pub async fn set_chat_ttl(
    session_token: String,
    friend_id: i64,
    ttl_hours: u32,
) -> Result<(), String> {
    let session = get_session_by_token_async(session_token).await?;
    session.with_db(|conn| {
        conn.execute(
            "INSERT INTO chat_settings (friend_id, ttl_hours) VALUES (?1, ?2)
             ON CONFLICT(friend_id) DO UPDATE SET ttl_hours = excluded.ttl_hours",
            rusqlite::params![friend_id, ttl_hours],
        )
        .map(|_| ())
        .map_err(|e| format!("Failed to save chat TTL: {}", e))
    })
}

/// Initialise (ou retrouve) l'entrée "Mon espace" - conversation de l'utilisateur avec lui-même.
/// Insère une entrée dans friends avec username_hash = notre propre hash (idempotent).
/// Retourne l'id de cette entrée.
#[tauri::command]
pub async fn init_self_space(
    session_token: String,
) -> Result<i64, String> {
    let session = get_session_by_token_async(session_token).await?;
    let user_db = session.get_user_db()
        .map_err(|e| format!("Failed to get user DB: {}", e))?;

    let our_hash_hex = session.user_hash_hex.clone();
    let now = current_timestamp() as i64;

    user_db.conn().execute(
        "INSERT OR IGNORE INTO friends
         (pseudo, username_hash, identity_key_public, kyber_public_key, x25519_public_key,
          verified, blocked, created_at, updated_at)
         VALUES ('Mon espace', ?1, ?2, ?3, ?4, 1, 0, ?5, ?5)",
        rusqlite::params![
            our_hash_hex,
            session.identity_key_public,
            session.kyber_public_key,
            session.x25519_public_key,
            now,
        ],
    ).map_err(|e| format!("Failed to init self space: {}", e))?;

    let self_id: i64 = user_db.conn().query_row(
        "SELECT id FROM friends WHERE username_hash = ?1",
        [&our_hash_hex as &dyn rusqlite::ToSql],
        |row| row.get(0),
    ).map_err(|e| format!("Failed to get self space id: {}", e))?;

    session.invalidate_friends_cache();

    Ok(self_id)
}

/// Envoie un message a un ami
#[tauri::command]
pub async fn send_message(
    session_token: String,
    friend_id: i64,
    content: String,
    file_data: Option<Vec<u8>>,
    file_name: Option<String>,
    file_mime: Option<String>,
    reply_to_id: Option<String>,
) -> Result<MessageInfo, String> {

    let session_token_ref = session_token.clone();
    let session = get_session_by_token_async(session_token).await?;

    let friend = session.get_friend(friend_id)
        .map_err(|e| format!("Friend not found: {}", e))?;

    let dilithium_secret_bytes = &session.dilithium_secret;

    let message_id = Uuid::new_v4().as_bytes().to_vec();
    let message_id_hex = hex::encode(&message_id);

    let timestamp = current_timestamp();

    let sender_hash = session.user_hash.clone();
    let recipient_hash = hex::decode(&friend.username_hash)
        .map_err(|e| format!("Failed to decode recipient hash: {}", e))?;

    let our_x25519_private = session.get_x25519_private()
        .map_err(|e| format!("Failed to get X25519 private key: {}", e))?;

    let user_db = session.get_user_db()
        .map_err(|e| format!("Failed to get user DB: {}", e))?;
    let data_key = &session.password_hash;

    async fn create_new_session_for_friend(
        user_hash: &[u8],
        friend_username_hash: &str,
        dilithium_secret: &[u8],
        friend_x25519_key: Option<&[u8]>,
        our_x25519_private: &[u8; 32],
    ) -> Result<(Session, X3DHInitResult, (u32, u32, String)), String> {
        let friend_hash_bytes = hex::decode(friend_username_hash)
            .map_err(|e| format!("Failed to decode friend hash: {}", e))?;

        let bundle_proto = fetch_friend_prekey_bundle(
            user_hash,
            &friend_hash_bytes,
            dilithium_secret,
        ).await?;


        let (new_session, init_result) = create_session_from_proto_bundle(
            &bundle_proto,
            friend_x25519_key,
            our_x25519_private,
        )?;

        let prekey_info = (
            bundle_proto.signed_pre_key_id,
            bundle_proto.pre_key_id,
            init_result.ephemeral_public.clone(),
        );

        Ok((new_session, init_result, prekey_info))
    }

    // Acquérir le lock de la conversation pour sérialiser lecture-chiffrement-sauvegarde.
    // Sans ce lock, deux send_message() parallèles liraient le même état ratchet,
    // produiraient des messages avec le même état, et le destinataire ne pourrait
    // déchiffrer que le premier.
    let send_lock = session.get_send_lock(friend_id);
    let _send_guard = send_lock.lock().await;

    let (mut ratchet_session, is_new_session, prekey_info) = {
        // Check if session exists
        let has_existing = has_session(&user_db, friend_id)
            .map_err(|e| format!("Failed to check session: {:?}", e))?;

        if has_existing {
            // Try to load existing session
            let db_session = db_load_session(&user_db, friend_id, data_key)
                .map_err(|e| format!("Failed to load session: {:?}", e))?
                .ok_or("Session exists but could not be loaded")?;

            match Session::from_db_session_raw(&db_session, &our_x25519_private) {
                Ok(loaded_session) => {
                    (loaded_session, false, None)
                }
                Err(e) => {
                    use crate::pages::chat::database::queries::delete_session;
                    let _ = delete_session(&user_db, friend_id);

                    let friend_x25519 = friend.x25519_public_key.as_deref();
                    let (new_session, _init_result, pk_info) = create_new_session_for_friend(
                        &session.user_hash,
                        &friend.username_hash,
                        &session.dilithium_secret,
                        friend_x25519,
                        &our_x25519_private,
                    ).await?;
                    (new_session, true, Some(pk_info))
                }
            }
        } else {
            let friend_x25519 = friend.x25519_public_key.as_deref();
            let (new_session, _init_result, pk_info) = create_new_session_for_friend(
                &session.user_hash,
                &friend.username_hash,
                &session.dilithium_secret,
                friend_x25519,
                &our_x25519_private,
            ).await?;
            (new_session, true, Some(pk_info))
        }
    };

    // Sérialisation InnerMessage protobuf avant chiffrement Double Ratchet
    let reply_id_str = reply_to_id.clone().unwrap_or_default();
    let (inner, message_type) = if let (Some(data), Some(name), Some(mime)) =
        (file_data.as_ref(), file_name.as_ref(), file_mime.as_ref())
    {
        let msg_type = mime_to_message_type(mime);
        let inner = InnerMessage {
            r#type: InnerMessageType::InnerFile as i32,
            text: String::new(),
            file_data: data.clone(),
            file_name: name.clone(),
            file_mime: mime.clone(),
            reply_to_id: reply_id_str,
            file_offer: None,
        };
        (inner, msg_type)
    } else {
        let inner = InnerMessage {
            r#type: InnerMessageType::InnerText as i32,
            text: content.clone(),
            file_data: vec![],
            file_name: String::new(),
            file_mime: String::new(),
            reply_to_id: reply_id_str,
            file_offer: None,
        };
        (inner, "text")
    };

    let payload_bytes = inner.encode_to_vec();
    let encrypt_result = ratchet_session.encrypt(&payload_bytes);

    let (encrypted_msg, is_new_session, prekey_info) = match encrypt_result {
        Ok(msg) => {
            (msg, is_new_session, prekey_info)
        }
        Err(e) if !is_new_session => {
            use crate::pages::chat::database::queries::delete_session;
            let _ = delete_session(&user_db, friend_id);

            let friend_x25519 = friend.x25519_public_key.as_deref();
            let (mut new_session, _init_result, pk_info) = Box::pin(async {
                let friend_hash_bytes = hex::decode(&friend.username_hash)
                    .map_err(|e| format!("Failed to decode friend hash: {}", e))?;
                let bundle_proto = fetch_friend_prekey_bundle(
                    &session.user_hash,
                    &friend_hash_bytes,
                    &session.dilithium_secret,
                ).await?;
                let (new_session, init_result) = create_session_from_proto_bundle(
                    &bundle_proto,
                    friend_x25519,
                    &our_x25519_private,
                )?;
                let ephemeral_pub = init_result.ephemeral_public.clone();
                Ok::<_, String>((new_session, init_result, (
                    bundle_proto.signed_pre_key_id,
                    bundle_proto.pre_key_id,
                    ephemeral_pub,
                )))
            }).await?;

            let encrypted = new_session.encrypt(&payload_bytes)
                .map_err(|e| format!("Double Ratchet encryption failed after retry: {:?}", e))?;
            ratchet_session = new_session;
            (encrypted, true, Some(pk_info))
        }
        Err(e) => {
            return Err(format!("Double Ratchet encryption failed: {:?}", e));
        }
    };

    let msg_data = EncryptedMessageData::from(&encrypted_msg);
    let ratchet_header = create_ratchet_header(&msg_data);

    let layer_keys = if is_new_session {
        let (signed_pre_key_id, pre_key_id, _) = prekey_info.as_ref()
            .map(|(s, p, e)| (*s, *p, e.clone()))
            .unwrap_or((1, 0, String::new()));
        vec![
            zenth_dto::LayerKeyWrap {
                layer_type: 0,
                key_id: signed_pre_key_id.to_le_bytes().to_vec(),
                wrapped_key: vec![],
            },
            zenth_dto::LayerKeyWrap {
                layer_type: 1,
                key_id: pre_key_id.to_le_bytes().to_vec(),
                wrapped_key: vec![],
            },
        ]
    } else {
        vec![]
    };

    let encrypted_body = EncryptedMessageBody {
        message_type: if is_new_session {
            zenth_dto::MessageType::PrekeyMessage as i32
        } else {
            zenth_dto::MessageType::RegularMessage as i32
        },
        ratchet_header: Some(ratchet_header),
        layer_keys,
        encrypted_layers: Some(zenth_dto::Layer1Ciphertext {
            ciphertext: msg_data.ciphertext.clone(),
            iv: vec![],
            hmac: hex::decode(&msg_data.tag).unwrap_or_default(),
            layer2: None,
        }),
        encrypted_metadata: vec![],
    };

    let envelope_content = if is_new_session {
        let (signed_pre_key_id, pre_key_id, ephemeral_public) = prekey_info
            .ok_or("Missing prekey info for new session")?;

        let our_x25519_public = {
            use x25519_dalek::{StaticSecret, PublicKey};
            let secret = StaticSecret::from(our_x25519_private);
            PublicKey::from(&secret)
        };

        let prekey_message = PreKeyMessage {
            pre_key_id,
            signed_pre_key_id,
            base_key: hex::decode(&ephemeral_public)
                .map_err(|e| format!("Failed to decode ephemeral key: {}", e))?,
            identity_key: Some(zenth_dto::IdentityKey {
                key_type: 0,
                public_key: our_x25519_public.as_bytes().to_vec(),
            }),
            message: Some(encrypted_body),
            pq_ciphertext: vec![],
            pq_pre_key_id: 0,
        };

        Content::PrekeyMessage(prekey_message)
    } else {
        Content::RegularMessage(encrypted_body)
    };

    let content_bytes = match &envelope_content {
        Content::RegularMessage(body) => body.encode_to_vec(),
        Content::PrekeyMessage(msg) => msg.encode_to_vec(),
    };

    let mut message_to_sign = Vec::new();
    message_to_sign.extend_from_slice(&sender_hash);
    message_to_sign.extend_from_slice(&recipient_hash);
    message_to_sign.extend_from_slice(&message_id);
    message_to_sign.extend_from_slice(&timestamp.to_le_bytes());
    message_to_sign.extend_from_slice(&content_bytes);

    let signature = sign_with_dilithium2(&dilithium_secret_bytes, &message_to_sign)?;

    // TTL souhaité par l'expéditeur, transporté dans sequence_number (0 = jamais).
    let ttl_hours: u32 = session.with_db(|conn| {
        conn.query_row(
            "SELECT ttl_hours FROM chat_settings WHERE friend_id = ?1",
            [friend_id],
            |row| row.get::<_, u32>(0),
        ).or_else(|_| Ok(0u32))
    }).unwrap_or(0);

    let envelope = ZenthSignalEnvelope {
        version: 1,
        sender_hash_id: sender_hash.clone(),
        recipient_hash_id: recipient_hash.clone(),
        content: Some(envelope_content),
        dilithium_signature: signature,
        timestamp,
        message_id: message_id.clone(),
        sequence_number: ttl_hours,
    };

    let now = current_timestamp() as i64;

    // Pour Mon espace : appliquer la couche vault ou bloquer si vault verrouillé.
    let is_self_message = friend.username_hash == session.user_hash_hex;
    let (local_encrypted, vault_encrypted_flag) = if is_self_message {
        let vault_active = session.get_setting("vault_enabled")
            .unwrap_or(None).map(|v| v == "1").unwrap_or(false)
            || session.with_db(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM messages m
                     JOIN friends f ON f.id = m.friend_id
                     WHERE f.username_hash = ?1 AND m.vault_encrypted = 1",
                    [&session.user_hash_hex],
                    |row| row.get::<_, i64>(0),
                ).map(|c| c > 0).or(Ok(false))
            }).unwrap_or(false);

        if vault_active {
            match crate::pages::vault::get_cached_vault_key(&session_token_ref) {
                Some(key) => {
                    let vault_layer = crate::pages::vault::vault_encrypt(&key, message_id_hex.as_bytes(), &payload_bytes)
                        .map_err(|e| format!("Vault encrypt: {}", e))?;
                    let enc = encrypt_message(&vault_layer, data_key, &message_id_hex)
                        .map_err(|e| format!("Failed to encrypt message locally: {:?}", e))?;
                    (enc, true)
                },
                None => return Err("VAULT_LOCKED".to_string()),
            }
        } else {
            let enc = encrypt_message(&payload_bytes, data_key, &message_id_hex)
                .map_err(|e| format!("Failed to encrypt message locally: {:?}", e))?;
            (enc, false)
        }
    } else {
        let enc = encrypt_message(&payload_bytes, data_key, &message_id_hex)
            .map_err(|e| format!("Failed to encrypt message locally: {:?}", e))?;
        (enc, false)
    };

    let db_message = DbNewMessage {
        friend_id,
        message_id: message_id_hex.clone(),
        is_outgoing: true,
        message_type: db_message_type(message_type).to_string(),
        encrypted_content: local_encrypted,
        content_iv: vec![],
        filename: file_name.clone(),
        file_size: file_data.as_ref().map(|d| d.len() as i64),
        mime_type: file_mime.clone(),
        timestamp: now,
        status: "pending".to_string(),
        vault_encrypted: vault_encrypted_flag,
        reply_to_id: reply_to_id.clone(),
    };

    let local_id = session.save_message(&db_message)
        .map_err(|e| format!("Failed to save message locally: {}", e))?;

    save_session_state(&user_db, friend_id, &ratchet_session, data_key)
        .map_err(|e| format!("Failed to save session state: {:?}", e))?;

    // Libère le lock ici - la section critique (lecture→chiffrement→sauvegarde) est terminée.
    // L'envoi réseau et le relay peuvent se faire en parallèle avec d'autres messages.
    drop(_send_guard);

    // Relay des messages envoyés vers les appareils jumelés (best-effort)
    {
        use base64::Engine;
        let inner_b64 = base64::engine::general_purpose::STANDARD.encode(&payload_bytes);
        let _ = crate::pages::sync::relay_push_message(
            session.username.clone(),
            session.password.clone(),
            message_id_hex.clone(),
            friend.username_hash.clone(),
            friend.pseudo.clone(),
            true,
            message_type.to_string(),
            inner_b64,
            now,
        ).await;
    }

    let server_result = send_message_to_server(&envelope).await;

    let final_status = match &server_result {
        Ok(response) => {
            if response.success {
                "sent"
            } else {
                "failed"
            }
        }
        Err(e) => {
            "failed"
        }
    };

    session.update_message_status(&message_id_hex, final_status)
        .map_err(|e| format!("Failed to update message status: {}", e))?;

    Ok(MessageInfo {
        id: local_id,
        message_id: message_id_hex,
        friend_id,
        content,
        is_outgoing: true,
        timestamp: now,
        status: final_status.to_string(),
        delivered_at: None,
        read_at: None,
        message_type: message_type.to_string(),
        file_name: file_name.clone(),
        file_mime: file_mime.clone(),
        file_data: file_data.map(|d| {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(&d)
        }),
        reply_to_id,
    })
}

async fn send_message_to_server(envelope: &ZenthSignalEnvelope) -> Result<SendMessageResponse, String> {
    let transport = RequestsNetwork::new("https")
        .await
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    let mut envelope_bytes = Vec::new();
    envelope.encode(&mut envelope_bytes)
        .map_err(|e| format!("Failed to encode envelope: {}", e))?;


    let request_id: [u8; 16] = rand::random();

    let dht_request = DhtRequest {
        method: Method::SendMessage as i32,
        payload: envelope_bytes,
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
        return Ok(SendMessageResponse {
            success: false,
            message_id: vec![],
            server_timestamp: 0,
            error_message: dht_response.error_message,
        });
    }

    // Decoder la payload en SendMessageResponse
    let send_response = SendMessageResponse::decode(&dht_response.payload[..])
        .map_err(|e| format!("Failed to decode SendMessageResponse: {}", e))?;

    Ok(send_response)
}

/// Recupere les messages locaux d'une conversation
#[tauri::command]
pub async fn get_messages(
    session_token: String,
    friend_id: i64,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<MessageInfo>, String> {

    let session = get_session_by_token_async(session_token.clone()).await?;
    let data_key = &session.password_hash;

    let messages = session.list_messages(friend_id, limit.unwrap_or(50), offset.unwrap_or(0))
        .map_err(|e| format!("Failed to list messages: {}", e))?;

    let result: Vec<MessageInfo> = messages.into_iter().map(|m| {
        let parsed = match crate::pages::vault::decrypt_with_vault_layer(
            &m.encrypted_content,
            data_key,
            &m.message_id,
            m.vault_encrypted,
            &session_token,
        ) {
            Ok(plaintext) => parse_inner_message(&plaintext),
            Err(_) => ParsedInner {
                // Vault verrouillé ou message corrompu - on ne retourne rien,
                // le frontend détecte l'état vault et affiche l'overlay de déverrouillage.
                display_content: String::new(),
                message_type: "vault_locked".to_string(),
                filename: None, mime_type: None, file_size: None, file_data_b64: None,
                reply_to_id: None,
            },
        };

        MessageInfo {
            id: m.id,
            message_id: m.message_id.clone(),
            friend_id: m.friend_id,
            content: parsed.display_content,
            is_outgoing: m.is_outgoing,
            timestamp: m.timestamp,
            status: m.status,
            delivered_at: m.delivered_at,
            read_at: m.read_at,
            message_type: parsed.message_type,
            file_name: parsed.filename,
            file_mime: parsed.mime_type,
            file_data: parsed.file_data_b64,
            reply_to_id: m.reply_to_id,
        }
    }).collect();

    Ok(result)
}

/// Synchronise les messages avec le serveur (fetch les nouveaux)
#[tauri::command]
pub async fn sync_messages(
    session_token: String,
) -> Result<MessageSyncResult, String> {

    let session = get_session_by_token_async(session_token).await?;

    // Si un sync est déjà en cours (ouverture SQLCipher PBKDF2 incluse), on abandonne
    // immédiatement plutôt que d'empiler N dérivations PBKDF2 en parallèle.
    let _sync_guard = match session.sync_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            return Ok(MessageSyncResult { new_messages: 0, errors: vec![], updated_friend_ids: vec![] });
        }
    };

    // Use cached values
    let dilithium_secret_bytes = &session.dilithium_secret;
    let user_hash = &session.user_hash;

    // Recuperer le dernier timestamp de sync (use cached connection)
    let last_sync = session.get_setting("last_message_sync")
        .unwrap_or(None)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // Vérifier le stock d'OTPKs et renouveler si nécessaire (best-effort, non bloquant)
    {
        use crate::api::prekeys::{PreKeyApiClient, PreKeyConfig, MIN_PREKEY_COUNT};
        use pqcrypto_dilithium::dilithium2;
        use pqcrypto_traits::sign::DetachedSignature as TraitSig2;

        if let Ok(client) = PreKeyApiClient::new(PreKeyConfig::default()).await {
            let ts = current_timestamp();
            let mut msg = Vec::new();
            msg.extend_from_slice(user_hash);
            msg.extend_from_slice(&ts.to_le_bytes());

            if let Ok(sk) = dilithium2::SecretKey::from_bytes(dilithium_secret_bytes) {
                let sig = dilithium2::detached_sign(&msg, &sk);
                if let Ok(resp) = client.check_prekey_count(
                    user_hash.to_vec(),
                    sig.as_bytes().to_vec(),
                ).await {
                    if resp.one_time_prekey_count < MIN_PREKEY_COUNT {
                        // Phase sync : génère + sauvegarde local (libère &UserDb avant await)
                        let public_prekeys = session.get_user_db().ok()
                            .and_then(|db| db.get_user_info().ok()
                                .and_then(|info| crate::pages::register::register::generate_and_save_otpks(
                                    &session.password_hash,
                                    session.username.as_bytes(),
                                    info.id,
                                    &db,
                                ).ok())
                            );

                        // Phase async : upload (plus de &UserDb en scope)
                        if let Some(prekeys) = public_prekeys {
                            let _ = crate::pages::register::register::upload_otpks(
                                user_hash.to_vec(),
                                dilithium_secret_bytes.to_vec(),
                                prekeys,
                            ).await;
                        }
                    }
                }
            }
        }
    }

    // Fetch les messages du serveur
    let server_result = fetch_messages_from_server(
        &user_hash,
        &dilithium_secret_bytes,
        last_sync,
        100,
    ).await;

    let mut new_messages = 0u32;
    let mut errors: Vec<String> = vec![];
    let mut updated_friend_ids: Vec<i64> = vec![];

    match server_result {
        Ok(response) => {
            let response_timestamp = response.timestamp; // ms depuis epoch (côté serveur)

            let total = response.messages.len();

            // Ouvrir la connexion DB et dériver la clé X25519 UNE SEULE FOIS pour tous les messages.
            // get_user_db() ouvre une connexion SQLCipher avec PBKDF2 (~2-5s) - l'appeler
            // par message multipliait ce coût par le nombre de messages reçus.
            let our_x25519_private = match session.get_x25519_private() {
                Ok(key) => key,
                Err(e) => {
                    errors.push(format!("Failed to get X25519 key: {}", e));
                    return Ok(MessageSyncResult { new_messages, errors, updated_friend_ids });
                }
            };
            let user_db = match session.get_user_db() {
                Ok(db) => db,
                Err(e) => {
                    errors.push(format!("Failed to get user DB: {}", e));
                    return Ok(MessageSyncResult { new_messages, errors, updated_friend_ids });
                }
            };
            let data_key = &session.password_hash;

            for (msg_idx, envelope) in response.messages.into_iter().enumerate() {
                let message_id_hex = hex::encode(&envelope.message_id);

                let sender_hash = hex::encode(&envelope.sender_hash_id);

                let friend = match session.get_friend_by_hash(&sender_hash) {
                    Ok(Some(f)) => f,
                    Ok(None) => continue,
                    Err(e) => {
                        errors.push(format!("DB error: {}", e));
                        continue;
                    }
                };

                let message_id_hex = hex::encode(&envelope.message_id);

                // Déchiffrement : on retourne (plaintext, session_avancée) sans sauver tout de suite.
                // La session n'est sauvée qu'APRÈS la sanitization pour éviter qu'un rejet de
                // sanitization avance le ratchet de façon irréversible (le message resterait sur le
                // serveur mais la clé serait consommée, rendant le retry impossible).
                let (content_bytes, pending_session): (Vec<u8>, Option<Session>) = match &envelope.content {
                    Some(Content::RegularMessage(ref msg)) => {
                        // Skip empty/corrupted messages (missing required data)
                        if msg.ratchet_header.is_none() && msg.encrypted_layers.is_none() {
                            continue;
                        }

                        // Check if this is actually a prekey message based on message_type field
                        if msg.message_type == zenth_dto::MessageType::PrekeyMessage as i32 {

                            let ratchet_header = msg.ratchet_header.as_ref();
                            let base_key = ratchet_header
                                .map(|h| h.sender_ratchet_key.clone())
                                .unwrap_or_default();

                            let friend_identity = friend.x25519_public_key.clone().unwrap_or_default();

                            let (signed_pre_key_id, pre_key_id) = if msg.layer_keys.len() >= 2 {
                                (
                                    u32::from_le_bytes(msg.layer_keys[0].key_id.get(..4).unwrap_or(&[1, 0, 0, 0]).try_into().unwrap_or([1, 0, 0, 0])),
                                    u32::from_le_bytes(msg.layer_keys[1].key_id.get(..4).unwrap_or(&[0, 0, 0, 0]).try_into().unwrap_or([0, 0, 0, 0])),
                                )
                            } else {
                                (1, 0)
                            };

                            let synthetic_prekey = PreKeyMessage {
                                pre_key_id,
                                signed_pre_key_id,
                                base_key,
                                identity_key: Some(zenth_dto::IdentityKey {
                                    key_type: 0,
                                    public_key: friend_identity,
                                }),
                                message: Some(msg.clone()),
                                pq_ciphertext: vec![],
                                pq_pre_key_id: 0,
                            };

                            match decrypt_prekey_message(
                                &synthetic_prekey,
                                &user_db,
                                friend.id,
                                data_key,
                                &our_x25519_private,
                                &session,
                            ) {
                                Ok((plaintext, new_session)) => (plaintext, Some(new_session)),
                                Err(_) => {
                                    continue;
                                }
                            }
                        } else {
                            // Regular message - load existing session and decrypt
                            match decrypt_regular_message(
                                msg,
                                &user_db,
                                friend.id,
                                data_key,
                                &our_x25519_private,
                            ) {
                                Ok((plaintext, updated_session)) => (plaintext, Some(updated_session)),
                                Err(_) => {
                                    // Session absente ou corrompue: on supprime l'état cassé pour
                                    // que le prochain message de cet ami reparte sur un PreKey frais.
                                    let _ = crate::pages::chat::database::queries::delete_session(&user_db, friend.id);
                                    continue;
                                }
                            }
                        }
                    }
                    Some(Content::PrekeyMessage(ref prekey_msg)) => {
                        match decrypt_prekey_message(
                            prekey_msg,
                            &user_db,
                            friend.id,
                            data_key,
                            &our_x25519_private,
                            &session,
                        ) {
                            Ok((plaintext, new_session)) => (plaintext, Some(new_session)),
                            Err(_) => {
                                continue;
                            }
                        }
                    }
                    None => {
                        continue;
                    }
                };

                // Sanitise le fichier entrant par magic bytes avant stockage.
                // Si la sanitization échoue, on abandonne SANS sauver la session :
                // le message reste sur le serveur et la clé ratchet reste disponible pour le retry.
                let content_bytes = match sanitize_incoming_bytes(&content_bytes) {
                    Ok(sanitized) => sanitized,
                    Err(e) => {
                        errors.push(format!("Message rejeté ({}) : {}", message_id_hex, e));
                        continue;
                    }
                };

                // Sauvegarder l'état de session avancé uniquement maintenant que le message
                // est validé (décrypté + sanitisé). Toute erreur ultérieure laisse le ratchet
                // dans cet état, mais le message sera déjà ackée ou rejeté définitivement.
                if let Some(sess) = pending_session {
                    let _ = save_session_state(&user_db, friend.id, &sess, data_key);
                }

                // Parse inner message to extract DB metadata
                let parsed = parse_inner_message(&content_bytes);

                // Encrypt inner message bytes locally before storing in DB
                let local_encrypted = match encrypt_message(&content_bytes, data_key, &message_id_hex) {
                    Ok(enc) => enc,
                    Err(e) => {
                        errors.push(format!("Failed to encrypt message locally: {:?}", e));
                        continue;
                    }
                };

                // Sauvegarder le message localement
                // Pour les messages reçus de soi-même (Mon espace), is_outgoing = true
                let db_message = DbNewMessage {
                    friend_id: friend.id,
                    message_id: message_id_hex.clone(),
                    is_outgoing: sender_hash == session.user_hash_hex,
                    message_type: db_message_type(&parsed.message_type).to_string(),
                    encrypted_content: local_encrypted,
                    content_iv: vec![],
                    filename: parsed.filename,
                    file_size: parsed.file_size,
                    mime_type: parsed.mime_type,
                    timestamp: envelope.timestamp as i64,
                    status: "delivered".to_string(),
                    vault_encrypted: false,
                    reply_to_id: parsed.reply_to_id,
                };


                match session.save_message(&db_message) {
                    Ok(_) => {
                        new_messages += 1;
                        if !updated_friend_ids.contains(&friend.id) {
                            updated_friend_ids.push(friend.id);
                        }
                        // Ack DHT : suppression immédiate du message côté serveur.
                        // Spawné en arrière-plan (best-effort, ne bloque pas le sync).
                        {
                            let ack_mid = envelope.message_id.clone();
                            let ack_recipient = session.user_hash.clone();
                            let ack_dilithium = session.dilithium_secret.clone();
                            let ack_ts = crate::utils::timestamp::plateform::current_timestamp();
                            tokio::spawn(async move {
                                let _ = send_message_ack(&ack_mid, &ack_recipient, &ack_dilithium, ack_ts).await;
                            });
                        }

                        // Relay vers les appareils jumelés
                        use base64::Engine as _;
                        let inner_b64 = base64::engine::general_purpose::STANDARD.encode(&content_bytes);
                        let (ru, rp, rid, rs, rpseudo, rtype, rb64, rts) = (
                            session.username.clone(),
                            session.password.clone(),
                            message_id_hex.clone(),
                            sender_hash.clone(),
                            friend.pseudo.clone(),
                            parsed.message_type.clone(),
                            inner_b64,
                            envelope.timestamp as i64,
                        );
                        tokio::spawn(async move {
                            let _ = crate::pages::sync::relay_push_message(
                                ru, rp, rid, rs, rpseudo, false, rtype, rb64, rts,
                            ).await;
                        });
                    }
                    Err(e) => {
                        if !e.to_string().contains("UNIQUE constraint") {
                            errors.push(format!("Failed to save message: {}", e));
                        }
                    }
                }
            }
            // Utilise le timestamp de réponse du serveur (en ms) comme curseur.
            // Le serveur stocke server_timestamp en ms, donc on compare en ms pour
            // éviter que since_timestamp (s) << server_timestamp (ms) ne renvoie
            // toujours tous les anciens messages et empêche les nouveaux de passer.
            let sync_ts = if response_timestamp > 0 {
                response_timestamp
            } else {
                current_timestamp() * 1000
            };
            let _ = session.set_setting("last_message_sync", &sync_ts.to_string());
        }
        Err(e) => {
            errors.push(format!("Server fetch failed: {}", e));
        }
    }

    Ok(MessageSyncResult {
        new_messages,
        errors,
        updated_friend_ids,
    })
}

async fn fetch_messages_from_server(
    user_hash: &[u8],
    dilithium_secret: &[u8],
    since_timestamp: u64,
    limit: u32,
) -> Result<FetchMessagesResponse, String> {
    let transport = RequestsNetwork::new("https")
        .await
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    let timestamp = current_timestamp();

    let mut message_to_sign = Vec::new();
    message_to_sign.extend_from_slice(user_hash);
    message_to_sign.extend_from_slice(&since_timestamp.to_le_bytes());
    message_to_sign.extend_from_slice(&limit.to_le_bytes());
    message_to_sign.extend_from_slice(&timestamp.to_le_bytes());

    let signature = sign_with_dilithium2(dilithium_secret, &message_to_sign)?;

    let fetch_request = FetchMessagesRequest {
        user_hash: user_hash.to_vec(),
        since_timestamp,
        limit,
        dilithium_signature: signature,
        timestamp,
    };

    let mut request_bytes = Vec::new();
    fetch_request.encode(&mut request_bytes)
        .map_err(|e| format!("Failed to encode FetchMessagesRequest: {}", e))?;

    let request_id: [u8; 16] = rand::random();

    let dht_request = DhtRequest {
        method: Method::FetchMessages as i32,
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

    let fetch_response = FetchMessagesResponse::decode(&dht_response.payload[..])
        .map_err(|e| format!("Failed to decode FetchMessagesResponse: {}", e))?;

    Ok(fetch_response)
}

#[tauri::command]
pub async fn mark_message_read(
    session_token: String,
    message_id: String,
) -> Result<(), String> {
    let session = get_session_by_token_async(session_token).await?;

    session.update_message_status(&message_id, "read")
        .map_err(|e| format!("Failed to update message status: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn clear_all_sessions(
    session_token: String,
) -> Result<u32, String> {

    let session = get_session_by_token_async(session_token).await?;
    let user_db = session.get_user_db()
        .map_err(|e| format!("Failed to get user DB: {}", e))?;

    let count = user_db.conn().execute(
        "DELETE FROM sessions",
        [],
    ).map_err(|e| format!("Failed to delete sessions: {}", e))?;

    Ok(count as u32)
}

#[tauri::command]
pub async fn delete_message_secure(
    session_token: String,
    message_id: String,
) -> Result<(), String> {
    let session = get_session_by_token_async(session_token).await?;
    let user_db = session.get_user_db()
        .map_err(|e| format!("Failed to get user DB: {}", e))?;
    user_db.delete_message_secure(&message_id)
        .map_err(|e| format!("Secure delete failed: {}", e))
}

/// Envoie un ack au serveur DHT pour supprimer immédiatement le message reçu.
/// Signature : Dilithium2(message_id || recipient_hash || timestamp)
async fn send_message_ack(
    message_id: &[u8],
    recipient_hash: &[u8],
    dilithium_secret: &[u8],
    timestamp: u64,
) -> Result<(), String> {
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(message_id);
    signed_data.extend_from_slice(recipient_hash);
    signed_data.extend_from_slice(&timestamp.to_le_bytes());

    let signature = sign_with_dilithium2(dilithium_secret, &signed_data)?;

    let ack = zenth_dto::MessageAck {
        message_id: message_id.to_vec(),
        recipient_hash_id: recipient_hash.to_vec(),
        delivered: true,
        read: false,
        timestamp,
        dilithium_signature: signature,
    };

    let mut ack_bytes = Vec::new();
    ack.encode(&mut ack_bytes)
        .map_err(|e| format!("Encode MessageAck: {}", e))?;

    let request_id: [u8; 16] = rand::random();
    let dht_request = DhtRequest {
        method: 28, // METHOD_ACK_MESSAGE
        payload: ack_bytes,
        timestamp,
        request_id: request_id.to_vec(),
    };

    let mut dht_bytes = Vec::new();
    dht_request.encode(&mut dht_bytes)
        .map_err(|e| format!("Encode DhtRequest: {}", e))?;

    let transport = zenth_requests::implementations::RequestsNetwork::new("https")
        .await
        .map_err(|e| format!("Transport: {}", e))?;

    let req = zenth_requests::request::Request {
        url: format!("{}/", crate::config::dht_api_url()),
        method: "POST".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/x-protobuf".to_string()),
            ("User-Agent".to_string(), "ZenthClient/1.0".to_string()),
        ],
        body: Some(dht_bytes),
    };

    use zenth_requests::transports::Transport;
    let _ = transport.send(req).await; // best-effort, on ignore les erreurs réseau
    Ok(())
}

fn sign_with_dilithium2(secret_key_bytes: &[u8], message: &[u8]) -> Result<Vec<u8>, String> {
    let secret_key = dilithium2::SecretKey::from_bytes(secret_key_bytes)
        .map_err(|_| "Invalid Dilithium2 secret key format")?;

    let signature = dilithium2::detached_sign(message, &secret_key);

    Ok(signature.as_bytes().to_vec())
}
