//! Database operations for Double Ratchet sessions and messages
//!
//! Handles session persistence with encryption using the functions from encryption.rs

use crate::db::user::UserDb;
use crate::db::error::DbError;
use crate::pages::chat::database::encryption::{
    encrypt_session, decrypt_session,
};

use crate::utils::timestamp::plateform::current_timestamp;

/// Double Ratchet session state stored in database
#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub friend_id: i64,
    pub root_key: Vec<u8>,
    pub sending_chain_key: Vec<u8>,
    pub receiving_chain_key: Option<Vec<u8>>,
    pub sending_counter: u32,
    pub receiving_counter: u32,
    pub dh_public: Vec<u8>,
    pub dh_private: Vec<u8>,
    pub remote_dh_public: Option<Vec<u8>>,
    pub created_at: i64,
    pub last_used_at: i64,
}

/// New session to be inserted
#[derive(Debug, Clone)]
pub struct NewSession {
    pub friend_id: i64,
    pub root_key: Vec<u8>,
    pub sending_chain_key: Vec<u8>,
    pub receiving_chain_key: Option<Vec<u8>>,
    pub sending_counter: u32,
    pub receiving_counter: u32,
    pub dh_public: Vec<u8>,
    pub dh_private: Vec<u8>,
    pub remote_dh_public: Option<Vec<u8>>,
}

/// Save or update a Double Ratchet session
///
/// Encrypts sensitive keys before storage using the data_key.
/// Uses UPSERT to handle both insert and update.
pub fn save_session(
    db: &UserDb,
    session: &NewSession,
    data_key: &[u8],
) -> Result<i64, DbError> {
    let friend_id_str = session.friend_id.to_string();

    // Encrypt sensitive data
    let root_key_encrypted = encrypt_session(&session.root_key, data_key, &friend_id_str)
        .map_err(|e| DbError::Encryption(e.to_string()))?;

    let sending_chain_encrypted = encrypt_session(&session.sending_chain_key, data_key, &friend_id_str)
        .map_err(|e| DbError::Encryption(e.to_string()))?;

    let receiving_chain_encrypted = match &session.receiving_chain_key {
        Some(key) => Some(encrypt_session(key, data_key, &friend_id_str)
            .map_err(|e| DbError::Encryption(e.to_string()))?),
        None => None,
    };

    let dh_private_encrypted = encrypt_session(&session.dh_private, data_key, &friend_id_str)
        .map_err(|e| DbError::Encryption(e.to_string()))?;

    let now = current_timestamp();

    // Empty IV vectors (IV is included in the encrypted data)
    let empty_iv: Vec<u8> = vec![];

    db.conn().execute(
        "INSERT INTO sessions (
            friend_id, root_key_encrypted, root_key_iv,
            sending_chain_key_encrypted, sending_chain_iv,
            receiving_chain_key_encrypted, receiving_chain_iv,
            sending_counter, receiving_counter,
            dh_public, dh_private_encrypted, dh_private_iv,
            remote_dh_public, created_at, last_used_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)
        ON CONFLICT(friend_id) DO UPDATE SET
            root_key_encrypted = excluded.root_key_encrypted,
            sending_chain_key_encrypted = excluded.sending_chain_key_encrypted,
            receiving_chain_key_encrypted = excluded.receiving_chain_key_encrypted,
            sending_counter = excluded.sending_counter,
            receiving_counter = excluded.receiving_counter,
            dh_public = excluded.dh_public,
            dh_private_encrypted = excluded.dh_private_encrypted,
            remote_dh_public = excluded.remote_dh_public,
            last_used_at = excluded.last_used_at",
        rusqlite::params![
            session.friend_id,
            root_key_encrypted,
            empty_iv,
            sending_chain_encrypted,
            empty_iv,
            receiving_chain_encrypted,
            empty_iv,
            session.sending_counter,
            session.receiving_counter,
            session.dh_public,
            dh_private_encrypted,
            empty_iv,
            session.remote_dh_public,
            now,
        ],
    )?;

    Ok(db.conn().last_insert_rowid())
}

/// Load a Double Ratchet session for a friend
///
/// Decrypts sensitive keys after retrieval.
pub fn load_session(
    db: &UserDb,
    friend_id: i64,
    data_key: &[u8],
) -> Result<Option<Session>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, friend_id, root_key_encrypted, sending_chain_key_encrypted,
                receiving_chain_key_encrypted, sending_counter, receiving_counter,
                dh_public, dh_private_encrypted, remote_dh_public,
                created_at, last_used_at
         FROM sessions WHERE friend_id = ?1"
    )?;

    let result = stmt.query_row([friend_id], |row| {
        Ok(SessionRow {
            id: row.get(0)?,
            friend_id: row.get(1)?,
            root_key_encrypted: row.get(2)?,
            sending_chain_encrypted: row.get(3)?,
            receiving_chain_encrypted: row.get(4)?,
            sending_counter: row.get(5)?,
            receiving_counter: row.get(6)?,
            dh_public: row.get(7)?,
            dh_private_encrypted: row.get(8)?,
            remote_dh_public: row.get(9)?,
            created_at: row.get(10)?,
            last_used_at: row.get(11)?,
        })
    });

    match result {
        Ok(row) => {
            let friend_id_str = row.friend_id.to_string();

            // Decrypt sensitive data
            let root_key = decrypt_session(&row.root_key_encrypted, data_key, &friend_id_str)
                .map_err(|e| DbError::Encryption(e.to_string()))?;

            let sending_chain_key = decrypt_session(&row.sending_chain_encrypted, data_key, &friend_id_str)
                .map_err(|e| DbError::Encryption(e.to_string()))?;

            let receiving_chain_key = match &row.receiving_chain_encrypted {
                Some(encrypted) if !encrypted.is_empty() => {
                    Some(decrypt_session(encrypted, data_key, &friend_id_str)
                        .map_err(|e| DbError::Encryption(e.to_string()))?)
                }
                _ => None,
            };

            let dh_private = decrypt_session(&row.dh_private_encrypted, data_key, &friend_id_str)
                .map_err(|e| DbError::Encryption(e.to_string()))?;

            Ok(Some(Session {
                id: row.id,
                friend_id: row.friend_id,
                root_key,
                sending_chain_key,
                receiving_chain_key,
                sending_counter: row.sending_counter,
                receiving_counter: row.receiving_counter,
                dh_public: row.dh_public,
                dh_private,
                remote_dh_public: row.remote_dh_public,
                created_at: row.created_at,
                last_used_at: row.last_used_at,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DbError::Sqlite(e)),
    }
}

/// Delete a session for a friend
pub fn delete_session(db: &UserDb, friend_id: i64) -> Result<bool, DbError> {
    let count = db.conn().execute(
        "DELETE FROM sessions WHERE friend_id = ?1",
        [friend_id],
    )?;

    Ok(count > 0)
}

/// List all active sessions (without decrypting keys)
///
/// Returns session metadata only, useful for session management UI.
pub fn list_active_sessions(db: &UserDb) -> Result<Vec<SessionInfo>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT s.id, s.friend_id, f.pseudo, f.username_hash,
                s.sending_counter, s.receiving_counter,
                s.created_at, s.last_used_at
         FROM sessions s
         JOIN friends f ON s.friend_id = f.id
         ORDER BY s.last_used_at DESC"
    )?;

    let sessions = stmt.query_map([], |row| {
        Ok(SessionInfo {
            id: row.get(0)?,
            friend_id: row.get(1)?,
            friend_pseudo: row.get(2)?,
            friend_hash: row.get(3)?,
            sending_counter: row.get(4)?,
            receiving_counter: row.get(5)?,
            created_at: row.get(6)?,
            last_used_at: row.get(7)?,
        })
    })?;

    let mut result = Vec::new();
    for session in sessions {
        result.push(session?);
    }

    Ok(result)
}

/// Check if a session exists for a friend
pub fn session_exists(db: &UserDb, friend_id: i64) -> Result<bool, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT 1 FROM sessions WHERE friend_id = ?1 LIMIT 1"
    )?;

    let exists = stmt.exists([friend_id])?;
    Ok(exists)
}

/// Update session counters after sending/receiving a message
pub fn update_session_counters(
    db: &UserDb,
    friend_id: i64,
    sending_counter: Option<u32>,
    receiving_counter: Option<u32>,
) -> Result<(), DbError> {
    let now = current_timestamp();

    match (sending_counter, receiving_counter) {
        (Some(sc), Some(rc)) => {
            db.conn().execute(
                "UPDATE sessions SET sending_counter = ?1, receiving_counter = ?2, last_used_at = ?3 WHERE friend_id = ?4",
                rusqlite::params![sc, rc, now, friend_id],
            )?;
        }
        (Some(sc), None) => {
            db.conn().execute(
                "UPDATE sessions SET sending_counter = ?1, last_used_at = ?2 WHERE friend_id = ?3",
                rusqlite::params![sc, now, friend_id],
            )?;
        }
        (None, Some(rc)) => {
            db.conn().execute(
                "UPDATE sessions SET receiving_counter = ?1, last_used_at = ?2 WHERE friend_id = ?3",
                rusqlite::params![rc, now, friend_id],
            )?;
        }
        (None, None) => {
            db.conn().execute(
                "UPDATE sessions SET last_used_at = ?1 WHERE friend_id = ?2",
                rusqlite::params![now, friend_id],
            )?;
        }
    }

    Ok(())
}

/// Mark a pre-key as used
pub fn mark_prekey_used(db: &UserDb, prekey_id: u32) -> Result<bool, DbError> {
    let now = current_timestamp();

    let count = db.conn().execute(
        "UPDATE pre_keys SET used = 1, used_at = ?1 WHERE pre_key_id = ?2",
        rusqlite::params![now, prekey_id],
    )?;

    Ok(count > 0)
}

/// Get count of unused pre-keys
pub fn get_unused_prekey_count(db: &UserDb) -> Result<u32, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT COUNT(*) FROM pre_keys WHERE used = 0"
    )?;

    let count: u32 = stmt.query_row([], |row| row.get(0))?;
    Ok(count)
}

/// Delete used pre-keys older than a certain age
pub fn cleanup_used_prekeys(db: &UserDb, max_age_secs: i64) -> Result<usize, DbError> {
    let cutoff = current_timestamp() as i64 - max_age_secs;

    let count = db.conn().execute(
        "DELETE FROM pre_keys WHERE used = 1 AND used_at < ?1",
        [cutoff],
    )?;

    Ok(count)
}

// === Internal types ===

/// Raw session row from database (encrypted)
struct SessionRow {
    id: i64,
    friend_id: i64,
    root_key_encrypted: Vec<u8>,
    sending_chain_encrypted: Vec<u8>,
    receiving_chain_encrypted: Option<Vec<u8>>,
    sending_counter: u32,
    receiving_counter: u32,
    dh_public: Vec<u8>,
    dh_private_encrypted: Vec<u8>,
    remote_dh_public: Option<Vec<u8>>,
    created_at: i64,
    last_used_at: i64,
}

/// Session metadata (no sensitive keys)
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: i64,
    pub friend_id: i64,
    pub friend_pseudo: String,
    pub friend_hash: String,
    pub sending_counter: u32,
    pub receiving_counter: u32,
    pub created_at: i64,
    pub last_used_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests require a mock UserDb or integration test setup
    // These are placeholder tests showing expected usage

    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
    }
}
