use rusqlite::Connection;
use crate::db::user::{UserDb, NewPendingRequest, PendingRequest, NewFriend, Friend};
use crate::db::error::DbError;
use crate::utils::timestamp::plateform::current_timestamp;
use serde::{Deserialize, Serialize};

/// Duree d'expiration des demandes d'ami (7 jours en secondes)
pub const REQUEST_EXPIRATION_SECS: i64 = 7 * 24 * 60 * 60;

/// Structure pour les informations d'ami a retourner au frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendInfo {
    pub id: i64,
    pub pseudo: String,
    pub username_hash: String,
    pub identity_key_public: String,  // hex encoded
    pub kyber_public_key: Option<String>,  // hex encoded
    pub x25519_public_key: Option<String>,  // hex encoded
    pub verified: bool,
    pub blocked: bool,
    pub created_at: i64,
    pub avatar: Option<String>,  // base64 encoded BLOB
}

impl From<Friend> for FriendInfo {
    fn from(f: Friend) -> Self {
        use base64::Engine as _;
        Self {
            id: f.id,
            pseudo: f.pseudo,
            username_hash: f.username_hash,
            identity_key_public: hex::encode(&f.identity_key_public),
            kyber_public_key: f.kyber_public_key.map(|k| hex::encode(&k)),
            x25519_public_key: f.x25519_public_key.map(|k| hex::encode(&k)),
            verified: f.verified,
            blocked: f.blocked,
            created_at: f.created_at,
            avatar: f.avatar.map(|b| base64::engine::general_purpose::STANDARD.encode(&b)),
        }
    }
}

/// Structure pour les demandes en attente a retourner au frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRequestInfo {
    pub id: i64,
    pub direction: String,
    pub remote_username_hash: String,
    pub remote_pseudo: Option<String>,
    pub remote_identity_key: String,  // hex encoded
    pub message: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

impl From<PendingRequest> for PendingRequestInfo {
    fn from(p: PendingRequest) -> Self {
        Self {
            id: p.id,
            direction: p.direction,
            remote_username_hash: p.remote_username_hash,
            remote_pseudo: p.remote_pseudo,
            remote_identity_key: hex::encode(&p.remote_identity_key),
            message: p.message,
            created_at: p.created_at,
            expires_at: p.expires_at,
        }
    }
}

/// Cree une nouvelle demande d'ami sortante
pub fn create_outgoing_request(
    conn: &Connection,
    remote_username_hash: &str,
    remote_pseudo: Option<String>,
    remote_identity_key: &[u8],
    remote_kyber_public_key: Option<Vec<u8>>,
    remote_x25519_public_key: Option<Vec<u8>>,
    signature: &[u8],
    message: Option<String>,
) -> Result<i64, DbError> {
    let now = current_timestamp() as i64;

    conn.execute(
        "INSERT INTO pending_friend_requests
         (direction, remote_username_hash, remote_pseudo, remote_identity_key,
          remote_kyber_public_key, remote_x25519_public_key, dilithium_signature,
          status, message, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(remote_username_hash) DO UPDATE SET
           direction = excluded.direction,
           remote_pseudo = excluded.remote_pseudo,
           remote_identity_key = excluded.remote_identity_key,
           remote_kyber_public_key = excluded.remote_kyber_public_key,
           remote_x25519_public_key = excluded.remote_x25519_public_key,
           dilithium_signature = excluded.dilithium_signature,
           status = excluded.status,
           message = excluded.message,
           created_at = excluded.created_at,
           expires_at = excluded.expires_at",
        rusqlite::params![
            "outgoing",
            remote_username_hash,
            remote_pseudo,
            remote_identity_key,
            remote_kyber_public_key,
            remote_x25519_public_key,
            signature,
            "pending",
            message,
            now,
            now + REQUEST_EXPIRATION_SECS,
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Cree une nouvelle demande d'ami entrante
pub fn create_incoming_request(
    db: &UserDb,
    remote_username_hash: &str,
    remote_pseudo: Option<String>,
    remote_identity_key: &[u8],
    remote_kyber_public_key: Option<Vec<u8>>,
    remote_x25519_public_key: Option<Vec<u8>>,
    signature: &[u8],
    message: Option<String>,
) -> Result<i64, DbError> {
    let now = current_timestamp() as i64;

    let request = NewPendingRequest {
        direction: "incoming".to_string(),
        remote_username_hash: remote_username_hash.to_string(),
        remote_pseudo,
        remote_identity_key: remote_identity_key.to_vec(),
        remote_kyber_public_key,
        remote_x25519_public_key,
        dilithium_signature: signature.to_vec(),
        status: "pending".to_string(),
        message,
        created_at: now,
        expires_at: Some(now + REQUEST_EXPIRATION_SECS),
    };

    db.add_pending_request(&request)
}

/// Accepte une demande d'ami et cree l'ami avec signatures d'amitie
///
/// # Arguments
/// * `db` - Base de donnees utilisateur
/// * `remote_hash` - Hash SHA256 du demandeur
/// * `local_signature` - Notre signature Dilithium de l'amitie (signe: "FRIENDSHIP:" || our_hash || remote_hash)
/// * `remote_signature` - Signature Dilithium de l'ami (reçue avec la demande originale)
pub fn accept_friend_request(
    conn: &Connection,
    remote_hash: &str,
    local_signature: Option<Vec<u8>>,
    remote_signature: Option<Vec<u8>>,
    custom_pseudo: Option<String>,
) -> Result<i64, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, direction, remote_username_hash, remote_pseudo, remote_identity_key,
                remote_kyber_public_key, remote_x25519_public_key, dilithium_signature,
                status, message, created_at, expires_at
         FROM pending_friend_requests WHERE remote_username_hash = ?1"
    )?;

    let request = match stmt.query_row([remote_hash], |row| {
        Ok(PendingRequest {
            id: row.get(0)?,
            direction: row.get(1)?,
            remote_username_hash: row.get(2)?,
            remote_pseudo: row.get(3)?,
            remote_identity_key: row.get(4)?,
            remote_kyber_public_key: row.get(5)?,
            remote_x25519_public_key: row.get(6)?,
            dilithium_signature: row.get(7)?,
            status: row.get(8)?,
            message: row.get(9)?,
            created_at: row.get(10)?,
            expires_at: row.get(11)?,
        })
    }) {
        Ok(r) => r,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(DbError::NotFound(format!("Request not found: {}", remote_hash))),
        Err(e) => return Err(DbError::Sqlite(e)),
    };

    if request.direction != "incoming" {
        return Err(DbError::InvalidOperation("Cannot accept outgoing request".to_string()));
    }

    let now = current_timestamp() as i64;

    let pseudo = custom_pseudo
        .filter(|p| !p.trim().is_empty())
        .or(request.remote_pseudo.clone())
        .unwrap_or_else(|| request.remote_username_hash[..8].to_string());

    conn.execute(
        "INSERT INTO friends (pseudo, username_hash, identity_key_public, kyber_public_key,
            x25519_public_key, friendship_signature_local, friendship_signature_remote,
            verified, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        rusqlite::params![
            pseudo,
            request.remote_username_hash,
            request.remote_identity_key,
            request.remote_kyber_public_key,
            request.remote_x25519_public_key,
            local_signature,
            remote_signature.or(Some(request.dilithium_signature)),
            false,
            now,
        ],
    )?;

    let friend_id = conn.last_insert_rowid();

    conn.execute(
        "UPDATE pending_friend_requests SET status = ?1 WHERE remote_username_hash = ?2",
        ["accepted", &request.remote_username_hash],
    )?;

    Ok(friend_id)
}

/// Rejette une demande d'ami
pub fn reject_friend_request(
    conn: &Connection,
    remote_hash: &str,
) -> Result<(), DbError> {
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pending_friend_requests WHERE remote_username_hash = ?1",
        [remote_hash],
        |row| row.get::<_, i64>(0),
    ).map(|c| c > 0).unwrap_or(false);

    if !exists {
        return Err(DbError::NotFound(format!("Request not found: {}", remote_hash)));
    }

    conn.execute(
        "UPDATE pending_friend_requests SET status = ?1 WHERE remote_username_hash = ?2",
        ["rejected", remote_hash],
    )?;

    Ok(())
}

/// Ajoute un ami apres acceptation de notre demande sortante
///
/// # Arguments
/// * `local_signature` - Notre signature d'amitie (créée maintenant)
/// * `remote_signature` - Signature d'amitie reçue de l'ami (dans sa FriendResponse)
pub fn add_friend_from_accepted_request(
    db: &UserDb,
    remote_hash: &str,
    remote_pseudo: Option<String>,
    remote_identity_key: &[u8],
    remote_kyber_public_key: Option<Vec<u8>>,
    remote_x25519_public_key: Option<Vec<u8>>,
    local_signature: Option<Vec<u8>>,
    remote_signature: Option<Vec<u8>>,
) -> Result<i64, DbError> {
    let now = current_timestamp() as i64;

    let friend = NewFriend {
        pseudo: remote_pseudo.unwrap_or_else(|| remote_hash[..8].to_string()),
        username_hash: remote_hash.to_string(),
        identity_key_public: remote_identity_key.to_vec(),
        kyber_public_key: remote_kyber_public_key,
        x25519_public_key: remote_x25519_public_key,
        friendship_signature_local: local_signature,
        friendship_signature_remote: remote_signature,
        verified: false,
        created_at: now,
    };

    let friend_id = db.add_friend(&friend)?;

    // Mettre a jour le statut de la demande sortante
    db.update_pending_request_status(remote_hash, "accepted")?;

    Ok(friend_id)
}
