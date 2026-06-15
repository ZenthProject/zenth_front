//! Session cache module
//!
//! Caches user credentials, database connections, and derived keys
//! to avoid expensive Argon2 key derivation on every command.

mod config;
pub use config::SessionConfig;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use once_cell::sync::Lazy;
use rusqlite::Connection;
use uuid::Uuid;

use crate::db::MasterDb;
use crate::db::crypto::{derive_sqlcipher_key, key_to_sqlcipher_pragma};
use crate::pages::register::crypto::key::derive_keys_from_password_with_salt;
use crate::utils::security::cipher_key::decrypt_key_with_password;
use crate::utils::timestamp::plateform::current_timestamp;

/// Cached database connection with user data
pub struct CachedConnection {
    conn: Connection,
    pub user_id: i64,
    pub pseudo: String,
    pub user_hash_hex: String,
    pub registration_id: i64,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Vec<u8>,
    pub x25519_public_key: Option<Vec<u8>>,
}

impl CachedConnection {
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

pub struct CachedSession {
    pub username: String,
    pub password: String,
    pub session_token: String,
    pub password_hash: Vec<u8>,
    pub dilithium_secret: Vec<u8>,
    /// Clé secrète Kyber sérialisée (format zenth_crypto::kem : [type_byte, ...key_bytes])
    pub kyber_secret: Vec<u8>,
    pub user_hash: Vec<u8>,
    pub user_hash_hex: String,
    pub registration_id: i64,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Vec<u8>,
    pub x25519_public_key: Option<Vec<u8>>,
    db_conn: Mutex<Option<Connection>>,
    friends_cache: Mutex<Option<Vec<Friend>>>,
    pub config: SessionConfig,
    created_at: std::time::Instant,
    /// Un mutex par friend_id pour sérialiser lecture-chiffrement-sauvegarde du ratchet.
    /// Empêche la race condition quand plusieurs messages sont envoyés en parallèle.
    send_locks: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
    /// Mutex global de sync : si un sync est déjà en cours, try_lock() échoue immédiatement
    /// et le second appel est ignoré. Évite N ouvertures SQLCipher (PBKDF2) en parallèle.
    pub sync_lock: tokio::sync::Mutex<()>,
}

impl CachedSession {
    /// Returns `true` if the session has not yet exceeded its configured timeout.
    pub fn is_valid(&self) -> bool {
        self.created_at.elapsed().as_secs() < self.config.timeout_secs
    }

    /// Retourne le mutex de sérialisation pour une conversation donnée.
    /// Crée le mutex s'il n'existe pas encore pour ce friend_id.
    pub fn get_send_lock(&self, friend_id: i64) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.send_locks.lock().unwrap_or_else(|e| e.into_inner());
        locks.entry(friend_id)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Executes `f` with a shared reference to the cached database connection.
    ///
    /// Reopens the connection using `self.username` and `self.password` if it has been dropped.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the mutex is poisoned, if the database cannot be reopened,
    /// or if `f` itself returns an error.
    pub fn with_db<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, String>,
    {
        let mut guard = self.db_conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        if guard.is_none() {
            *guard = Some(open_user_db(&self.username, &self.password)?);
        }

        f(guard.as_ref().unwrap())
    }

    /// Executes `f` with a mutable reference to the cached database connection.
    ///
    /// Reopens the connection using `self.username` and `self.password` if it has been dropped.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the mutex is poisoned, if the database cannot be reopened,
    /// or if `f` itself returns an error.
    pub fn with_db_mut<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut Connection) -> Result<R, String>,
    {
        let mut guard = self.db_conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        if guard.is_none() {
            *guard = Some(open_user_db(&self.username, &self.password)?);
        }

        f(guard.as_mut().unwrap())
    }
}

/// Global session cache keyed by session_token (UUID)
static SESSION_CACHE: Lazy<RwLock<HashMap<String, Arc<CachedSession>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Reverse lookup: username → session_token (for clear_session by username)
static USERNAME_TO_TOKEN: Lazy<RwLock<HashMap<String, String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Opens and unlocks the SQLCipher database for `username`.
///
/// Derives the encryption key from `password` via Argon2id, verifies the key works,
/// then runs any pending idempotent schema migrations before returning the connection.
///
/// # Errors
///
/// Returns `Err` if the user is not found in the master database, if key derivation
/// fails, if `password` is incorrect, or if a non-idempotent migration fails.
fn open_user_db(username: &str, password: &str) -> Result<Connection, String> {
    let master = MasterDb::open()
        .map_err(|e| format!("Failed to open master database: {}", e))?;
    let entry = master.get_user(username)
        .map_err(|e| format!("User not found: {}", e))?;

    let db_path = MasterDb::user_db_path(&entry.name_hash);
    let key = derive_sqlcipher_key(password, &entry.salt)
        .map_err(|e| format!("Failed to derive key: {}", e))?;
    let pragma_key = key_to_sqlcipher_pragma(&key);

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;
    conn.pragma_update(None, "key", &*pragma_key)
        .map_err(|e| format!("Failed to set key: {}", e))?;

    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .map_err(|_| "Invalid password".to_string())?;

    // WAL mode: allows concurrent reads + 1 writer; busy_timeout retries on SQLITE_BUSY
    let _ = conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;");

    for sql in &[
        "ALTER TABLE friends ADD COLUMN avatar BLOB",
        "ALTER TABLE user ADD COLUMN avatar BLOB",
    ] {
        match conn.execute(sql, []) {
            Ok(_) => {}
            Err(e) => {
                if !e.to_string().contains("duplicate column") {
                    return Err(format!("Migration failed: {}", e));
                }
            }
        }
    }

    Ok(conn)
}

/// Returns a cached session for `username`, creating one if none exists or if it has expired.
///
/// Runs [`get_session`] on the blocking thread pool to avoid blocking the async runtime.
///
/// # Errors
///
/// Returns `Err` if session creation fails (see [`get_session`]).
pub async fn get_session_async(username: String, password: String) -> Result<Arc<CachedSession>, String> {
    tokio::task::spawn_blocking(move || get_session(&username, &password))
        .await
        .map_err(|e| format!("Thread pool error: {}", e))?
}

/// Returns the session identified by `token`, or `Err` if no valid session exists.
///
/// # Errors
///
/// Returns `Err` if the token is not found or the session has expired.
pub fn get_session_by_token(token: &str) -> Result<Arc<CachedSession>, String> {
    let cache = SESSION_CACHE.read().unwrap_or_else(|e| e.into_inner());
    match cache.get(token) {
        Some(session) if session.is_valid() => Ok(Arc::clone(session)),
        Some(_) => Err("Session expired. Please login again.".to_string()),
        None => Err("Invalid or expired session token.".to_string()),
    }
}

/// Async wrapper around [`get_session_by_token`].
pub async fn get_session_by_token_async(token: String) -> Result<Arc<CachedSession>, String> {
    tokio::task::spawn_blocking(move || get_session_by_token(&token))
        .await
        .map_err(|e| format!("Thread pool error: {}", e))?
}

/// Removes the session for `username` from the cache.
///
/// Async wrapper around [`clear_session`], runs on the blocking thread pool.
pub async fn clear_session_async(username: String) {
    tokio::task::spawn_blocking(move || clear_session(&username))
        .await
        .ok();
}

/// Returns the cached session for `username`, creating it if missing or expired.
///
/// Uses double-checked locking: a fast read-lock path first, then a write-lock
/// with a second check to prevent concurrent session creation, since Argon2id
/// key derivation in [`create_session`] is intentionally expensive (~1–3 s).
///
/// The session is keyed by its `session_token` (UUID). A reverse map from
/// username → token is maintained in `USERNAME_TO_TOKEN`.
///
/// # Errors
///
/// Returns `Err` if the session does not exist and creation fails (wrong password,
/// database unreachable, or decryption error).
pub fn get_session(username: &str, password: &str) -> Result<Arc<CachedSession>, String> {
    // Fast path: look up existing token for this username
    {
        let username_map = USERNAME_TO_TOKEN.read().unwrap_or_else(|e| e.into_inner());
        if let Some(token) = username_map.get(username) {
            let cache = SESSION_CACHE.read().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = cache.get(token.as_str()) {
                if session.is_valid() {
                    return Ok(Arc::clone(session));
                }
            }
        }
    }

    // Slow path: create a new session
    let mut username_map = USERNAME_TO_TOKEN.write().unwrap_or_else(|e| e.into_inner());
    // Remove stale token if present
    if let Some(old_token) = username_map.get(username) {
        let mut cache = SESSION_CACHE.write().unwrap_or_else(|e| e.into_inner());
        cache.remove(old_token.as_str());
    }

    let session = Arc::new(create_session(username, password)?);
    let token = session.session_token.clone();

    username_map.insert(username.to_string(), token.clone());
    drop(username_map);

    let mut cache = SESSION_CACHE.write().unwrap_or_else(|e| e.into_inner());
    cache.insert(token, Arc::clone(&session));
    Ok(session)
}

/// Creates a new session for `username`.
///
/// Opens the database, derives keys from `password` via Argon2id, decrypts the
/// Dilithium identity key, and optionally pre-loads the friends list into cache.
/// This is the slow path - only called once per login by [`get_session`].
///
/// # Errors
///
/// Returns `Err` if the database cannot be opened, if `password` is incorrect,
/// if identity key decryption fails, or if the `dilithium_secret` field is missing.
fn create_session(username: &str, password: &str) -> Result<CachedSession, String> {
    let start = std::time::Instant::now();

    let config = SessionConfig::load();
    let conn = open_user_db(username, password)?;

    let (_user_id, _pseudo, user_hash_hex, registration_id, identity_key_public, kyber_public_key, x25519_public_key, encrypted_identity_keys) = conn.query_row(
        "SELECT id, pseudo, username_hash, registration_id, identity_key_dilithium_public,
                kyber_public_key, x25519_public_key, encrypted_identity_keys
         FROM user LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, Option<Vec<u8>>>(6)?,
                row.get::<_, Vec<u8>>(7)?,
            ))
        },
    ).map_err(|e| format!("Failed to get user info: {}", e))?;

    let master_db = MasterDb::open()
        .map_err(|e| format!("Failed to open master database: {}", e))?;
    let user_entry = master_db.get_user(username)
        .map_err(|e| format!("User not found: {}", e))?;

    let password_hash = derive_keys_from_password_with_salt(password, &user_entry.salt)?;

    let decrypted_keys_json = decrypt_key_with_password(
        &encrypted_identity_keys,
        &password_hash,
        username.as_bytes(),
    ).map_err(|e| format!("Failed to decrypt keys: {:?}", e))?;

    let keys: serde_json::Value = serde_json::from_slice(&decrypted_keys_json)
        .map_err(|e| format!("Failed to parse keys: {}", e))?;

    let dilithium_secret_hex = keys["dilithium_secret"]
        .as_str()
        .ok_or("Missing dilithium_secret")?;

    let dilithium_secret = hex::decode(dilithium_secret_hex)
        .map_err(|e| format!("Failed to decode dilithium secret: {}", e))?;

    let kyber_secret_hex = keys["kyber_secret"]
        .as_str()
        .ok_or("Missing kyber_secret")?;

    let kyber_secret = hex::decode(kyber_secret_hex)
        .map_err(|e| format!("Failed to decode kyber secret: {}", e))?;

    let user_hash = hex::decode(&user_hash_hex)
        .map_err(|e| format!("Failed to decode user hash: {}", e))?;

    let friends_preloaded = if config.cache_friends {
        match load_friends_from_conn(&conn) {
            Ok(friends) => {
                Some(friends)
            }
            Err(e) => {
                None
            }
        }
    } else {
        None
    };

    let session_token = uuid::Uuid::new_v4().to_string();

    Ok(CachedSession {
        username: username.to_string(),
        password: password.to_string(),
        session_token,
        password_hash,
        dilithium_secret,
        kyber_secret,
        user_hash,
        user_hash_hex,
        registration_id,
        identity_key_public,
        kyber_public_key,
        x25519_public_key,
        db_conn: Mutex::new(Some(conn)),
        friends_cache: Mutex::new(friends_preloaded),
        config,
        created_at: std::time::Instant::now(),
        send_locks: Mutex::new(HashMap::new()),
        sync_lock: tokio::sync::Mutex::new(()),
    })
}

/// Loads all non-blocked friends from `conn`, ordered by pseudo.
///
/// # Errors
///
/// Returns `Err` if the SQL statement cannot be prepared or if a row fails to deserialize.
fn load_friends_from_conn(conn: &Connection) -> Result<Vec<Friend>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                x25519_public_key, friendship_signature_local, friendship_signature_remote,
                verified, blocked, created_at, updated_at, avatar
         FROM friends WHERE blocked = 0 AND (pin_hash IS NULL) ORDER BY pseudo"
    ).map_err(|e| format!("Prepare error: {}", e))?;

    let friends = stmt.query_map([], |row| {
        Ok(Friend {
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
    for friend in friends {
        result.push(friend.map_err(|e| format!("Row error: {}", e))?);
    }
    Ok(result)
}

/// Removes the session for `username` from the cache. Call this on logout.
pub fn clear_session(username: &str) {
    let mut username_map = USERNAME_TO_TOKEN.write().unwrap_or_else(|e| e.into_inner());
    if let Some(token) = username_map.remove(username) {
        let mut cache = SESSION_CACHE.write().unwrap_or_else(|e| e.into_inner());
        cache.remove(&token);
    }
}

/// Removes the session identified by `token` from the cache. Call this on logout by token.
pub fn clear_session_by_token(token: &str) {
    let mut cache = SESSION_CACHE.write().unwrap_or_else(|e| e.into_inner());
    if let Some(session) = cache.remove(token) {
        let mut username_map = USERNAME_TO_TOKEN.write().unwrap_or_else(|e| e.into_inner());
        username_map.remove(&session.username);
    }
}

/// Removes all sessions from the cache.
pub fn clear_all_sessions() {
    let mut cache = SESSION_CACHE.write().unwrap_or_else(|e| e.into_inner());
    cache.clear();
    let mut username_map = USERNAME_TO_TOKEN.write().unwrap_or_else(|e| e.into_inner());
    username_map.clear();
}

use crate::db::user::{Friend, NewFriend, Message, NewMessage, PendingRequest};

impl CachedSession {
    /// Clears the in-memory friends cache. Call this after any add/remove operation.
    pub fn invalidate_friends_cache(&self) {
        if let Ok(mut cache) = self.friends_cache.lock() {
            *cache = None;
        }
    }

    /// Returns the friends list from the in-memory cache, or queries the database on a miss.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the database query fails (only on a cache miss).
    pub fn list_friends(&self) -> Result<Vec<Friend>, String> {
        if let Ok(cache) = self.friends_cache.lock() {
            if let Some(ref friends) = *cache {
                return Ok(friends.clone());
            }
        }

        let friends = self.with_db(|conn| {
            load_friends_from_conn(conn)
        })?;

        if let Ok(mut cache) = self.friends_cache.lock() {
            *cache = Some(friends.clone());
        }

        Ok(friends)
    }

    /// Returns the friend matching `hash`, or `None` if no such friend exists.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the database query fails.
    pub fn get_friend_by_hash(&self, hash: &str) -> Result<Option<Friend>, String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                        x25519_public_key, friendship_signature_local, friendship_signature_remote,
                        verified, blocked, created_at, updated_at, avatar
                 FROM friends WHERE username_hash = ?1"
            ).map_err(|e| format!("Prepare error: {}", e))?;

            let result = stmt.query_row([hash], |row| {
                Ok(Friend {
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
            });

            match result {
                Ok(friend) => Ok(Some(friend)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query error: {}", e)),
            }
        })
    }

    /// Returns the friend with the given database `id`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the friend is not found or if the query fails.
    pub fn get_friend(&self, id: i64) -> Result<Friend, String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                        x25519_public_key, friendship_signature_local, friendship_signature_remote,
                        verified, blocked, created_at, updated_at, avatar
                 FROM friends WHERE id = ?1"
            ).map_err(|e| format!("Prepare error: {}", e))?;

            stmt.query_row([id], |row| {
                Ok(Friend {
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
            }).map_err(|e| format!("Query error: {}", e))
        })
    }

    /// Inserts `msg` into the messages table and returns the new row ID.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the insert fails.
    pub fn save_message(&self, msg: &NewMessage) -> Result<i64, String> {
        self.with_db(|conn| {
            conn.execute(
                "INSERT INTO messages (friend_id, message_id, is_outgoing, message_type,
                    encrypted_content, content_iv, filename, file_size, mime_type,
                    timestamp, status, vault_encrypted, reply_to_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                rusqlite::params![
                    msg.friend_id,
                    msg.message_id,
                    msg.is_outgoing,
                    msg.message_type,
                    msg.encrypted_content,
                    msg.content_iv,
                    msg.filename,
                    msg.file_size,
                    msg.mime_type,
                    msg.timestamp,
                    msg.status,
                    msg.vault_encrypted as i32,
                    msg.reply_to_id,
                ],
            ).map_err(|e| format!("Insert error: {}", e))?;

            Ok(conn.last_insert_rowid())
        })
    }

    /// Returns up to `limit` messages for `friend_id`, skipping `offset` rows, ordered by timestamp descending.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the query fails or if a row cannot be deserialized.
    pub fn list_messages(&self, friend_id: i64, limit: i64, offset: i64) -> Result<Vec<Message>, String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, friend_id, message_id, is_outgoing, message_type,
                        encrypted_content, content_iv, filename, file_size, mime_type,
                        timestamp, status, delivered_at, read_at, reply_to_id, edited, deleted,
                        vault_encrypted
                 FROM messages
                 WHERE friend_id = ?1 AND deleted = 0
                 ORDER BY timestamp DESC
                 LIMIT ?2 OFFSET ?3"
            ).map_err(|e| format!("Prepare error: {}", e))?;

            let messages = stmt.query_map(rusqlite::params![friend_id, limit, offset], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    friend_id: row.get(1)?,
                    message_id: row.get(2)?,
                    is_outgoing: row.get(3)?,
                    message_type: row.get(4)?,
                    encrypted_content: row.get(5)?,
                    content_iv: row.get(6)?,
                    filename: row.get(7)?,
                    file_size: row.get(8)?,
                    mime_type: row.get(9)?,
                    timestamp: row.get(10)?,
                    status: row.get(11)?,
                    delivered_at: row.get(12)?,
                    read_at: row.get(13)?,
                    reply_to_id: row.get(14)?,
                    edited: row.get(15)?,
                    deleted: row.get(16)?,
                    vault_encrypted: row.get::<_, i32>(17).map(|v| v == 1).unwrap_or(false),
                })
            }).map_err(|e| format!("Query error: {}", e))?;

            let mut result = Vec::new();
            for msg in messages {
                result.push(msg.map_err(|e| format!("Row error: {}", e))?);
            }
            Ok(result)
        })
    }

    /// Updates the status of the message identified by `message_id` to `status`.
    ///
    /// Accepted values for `status`: `"delivered"`, `"read"`. Other values update
    /// the status field only, leaving `delivered_at` and `read_at` unchanged.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the update query fails.
    pub fn update_message_status(&self, message_id: &str, status: &str) -> Result<(), String> {
        self.with_db(|conn| {
            let now = current_timestamp() as i64;

            let (delivered, read) = match status {
                "delivered" => (Some(now), None),
                "read" => (None, Some(now)),
                _ => (None, None),
            };

            conn.execute(
                "UPDATE messages SET status = ?1, delivered_at = COALESCE(?2, delivered_at),
                 read_at = COALESCE(?3, read_at) WHERE message_id = ?4",
                rusqlite::params![status, delivered, read, message_id],
            ).map_err(|e| format!("Update error: {}", e))?;

            Ok(())
        })
    }

    /// Returns the value associated with `key` in the settings table, or `None` if absent.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the query fails.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")
                .map_err(|e| format!("Prepare error: {}", e))?;

            let result = stmt.query_row([key], |row| row.get(0));

            match result {
                Ok(value) => Ok(Some(value)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query error: {}", e)),
            }
        })
    }

    /// Inserts or updates the setting `key` with `value` (upsert).
    ///
    /// # Errors
    ///
    /// Returns `Err` if the upsert query fails.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.with_db(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, value],
            ).map_err(|e| format!("Insert error: {}", e))?;

            Ok(())
        })
    }

    /// Returns the pending friend request from `hash`, or `None` if no such request exists.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the query fails.
    pub fn get_pending_request_by_hash(&self, hash: &str) -> Result<Option<PendingRequest>, String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, direction, remote_username_hash, remote_pseudo,
                        remote_identity_key, remote_kyber_public_key, remote_x25519_public_key,
                        dilithium_signature, status, message, created_at, expires_at
                 FROM pending_friend_requests WHERE remote_username_hash = ?1"
            ).map_err(|e| format!("Prepare error: {}", e))?;

            let result = stmt.query_row([hash], |row| {
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
            });

            match result {
                Ok(req) => Ok(Some(req)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query error: {}", e)),
            }
        })
    }

    /// Returns all pending friend requests, optionally filtered by `direction` (`"incoming"` or `"outgoing"`).
    ///
    /// # Errors
    ///
    /// Returns `Err` if the query fails or if a row cannot be deserialized.
    pub fn list_pending_requests(&self, direction: Option<&str>) -> Result<Vec<PendingRequest>, String> {
        self.with_db(|conn| {
            let map_row = |row: &rusqlite::Row| -> rusqlite::Result<PendingRequest> {
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
            };

            let rows: Vec<PendingRequest> = match direction {
                Some(dir) => {
                    let mut stmt = conn.prepare(
                        "SELECT id, direction, remote_username_hash, remote_pseudo,
                                remote_identity_key, remote_kyber_public_key, remote_x25519_public_key,
                                dilithium_signature, status, message, created_at, expires_at
                         FROM pending_friend_requests
                         WHERE direction = ?1 AND status = 'pending'
                         ORDER BY created_at DESC"
                    ).map_err(|e| format!("Prepare error: {}", e))?;
                    // and_then consomme MappedRows avant le ? - évite le borrow sur stmt
                    stmt.query_map(rusqlite::params![dir], map_row)
                        .and_then(|mapped| mapped.collect::<rusqlite::Result<Vec<_>>>())
                        .map_err(|e| format!("Query error: {}", e))?
                }
                None => {
                    let mut stmt = conn.prepare(
                        "SELECT id, direction, remote_username_hash, remote_pseudo,
                                remote_identity_key, remote_kyber_public_key, remote_x25519_public_key,
                                dilithium_signature, status, message, created_at, expires_at
                         FROM pending_friend_requests
                         WHERE status = 'pending'
                         ORDER BY created_at DESC"
                    ).map_err(|e| format!("Prepare error: {}", e))?;
                    stmt.query_map([], map_row)
                        .and_then(|mapped| mapped.collect::<rusqlite::Result<Vec<_>>>())
                        .map_err(|e| format!("Query error: {}", e))?
                }
            };

            Ok(rows)
        })
    }

    /// Deletes all pending requests whose `expires_at` is in the past.
    ///
    /// Returns the number of deleted rows.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the delete query fails.
    pub fn cleanup_expired_requests(&self) -> Result<usize, String> {
        self.with_db(|conn| {
            let now = current_timestamp() as i64;

            let deleted = conn.execute(
                "DELETE FROM pending_friend_requests WHERE expires_at < ?1",
                [now],
            ).map_err(|e| format!("Delete error: {}", e))?;

            Ok(deleted)
        })
    }

    /// Deletes the pending friend request associated with `hash`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the delete query fails.
    pub fn delete_pending_request(&self, hash: &str) -> Result<(), String> {
        self.with_db(|conn| {
            conn.execute(
                "DELETE FROM pending_friend_requests WHERE remote_username_hash = ?1",
                [hash],
            ).map_err(|e| format!("Delete error: {}", e))?;
            Ok(())
        })
    }

    /// Deletes the friend with the given `id` and invalidates the friends cache.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the delete query fails.
    pub fn remove_friend(&self, id: i64) -> Result<(), String> {
        self.with_db(|conn| {
            conn.execute("DELETE FROM friends WHERE id = ?1", [id])
                .map_err(|e| format!("Delete error: {}", e))?;
            Ok(())
        })?;
        self.invalidate_friends_cache();
        Ok(())
    }

    /// Inserts `friend` into the friends table, invalidates the cache, and returns the new row ID.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the insert fails.
    pub fn add_friend(&self, friend: &NewFriend) -> Result<i64, String> {
        let id = self.with_db(|conn| {
            let now = current_timestamp() as i64;

            conn.execute(
                "INSERT INTO friends (pseudo, username_hash, identity_key_public, kyber_public_key,
                    x25519_public_key, friendship_signature_local, friendship_signature_remote,
                    verified, blocked, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    friend.pseudo,
                    friend.username_hash,
                    friend.identity_key_public,
                    friend.kyber_public_key,
                    friend.x25519_public_key,
                    friend.friendship_signature_local,
                    friend.friendship_signature_remote,
                    friend.verified,
                    false,
                    friend.created_at,
                    now,
                ],
            ).map_err(|e| format!("Insert error: {}", e))?;

            Ok(conn.last_insert_rowid())
        })?;
        self.invalidate_friends_cache();
        Ok(id)
    }

    /// Returns the local X25519 private key, decrypted from the most recent signed pre-key entry.
    ///
    /// # Errors
    ///
    /// Returns `Err` if no signed pre-key exists, if decryption fails, or if the key
    /// is not exactly 32 bytes.
    pub fn get_x25519_private(&self) -> Result<[u8; 32], String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT pre_key_private_encrypted FROM pre_keys
                 WHERE signed_pre_key_id IS NOT NULL
                 ORDER BY id DESC LIMIT 1"
            ).map_err(|e| format!("Prepare error: {}", e))?;

            let encrypted: Vec<u8> = stmt.query_row([], |row| row.get(0))
                .map_err(|_| "No signed pre-key found".to_string())?;

            let decrypted = crate::utils::security::cipher_key::decrypt_key_with_password(
                &encrypted,
                &self.password_hash,
                self.username.as_bytes(),
            ).map_err(|e| format!("Failed to decrypt X25519 key: {:?}", e))?;

            let key: [u8; 32] = decrypted.try_into()
                .map_err(|_| "Invalid X25519 key length".to_string())?;

            Ok(key)
        })
    }

    /// Opens and returns a [`UserDb`] wrapper for higher-level database operations.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the database cannot be opened.
    pub fn get_user_db(&self) -> Result<crate::db::user::UserDb, String> {
        crate::db::user::UserDb::open(&self.username, &self.password)
            .map_err(|e| format!("Failed to open user database: {:?}", e))
    }

    /// Returns the decrypted private key for the signed pre-key identified by `signed_pre_key_id`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the pre-key is not found, if decryption fails, or if the key
    /// is not exactly 32 bytes.
    pub fn get_signed_prekey(&self, signed_pre_key_id: u32) -> Result<[u8; 32], String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT pre_key_private_encrypted FROM pre_keys
                 WHERE signed_pre_key_id = ?1"
            ).map_err(|e| format!("Prepare error: {}", e))?;

            let encrypted: Vec<u8> = stmt.query_row([signed_pre_key_id], |row| row.get(0))
                .map_err(|_| format!("Signed pre-key {} not found", signed_pre_key_id))?;

            let decrypted = crate::utils::security::cipher_key::decrypt_key_with_password(
                &encrypted,
                &self.password_hash,
                self.username.as_bytes(),
            ).map_err(|e| format!("Failed to decrypt signed pre-key: {:?}", e))?;

            let key: [u8; 32] = decrypted.try_into()
                .map_err(|_| "Invalid signed pre-key length".to_string())?;

            Ok(key)
        })
    }

    /// Returns the decrypted private key for the one-time pre-key identified by `pre_key_id`,
    /// or `None` if the key has already been used or does not exist.
    ///
    /// # Errors
    ///
    /// Returns `Err` if decryption fails or if the key is not exactly 32 bytes.
    pub fn get_one_time_prekey(&self, pre_key_id: u32) -> Result<Option<(u32, [u8; 32])>, String> {
        self.with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT pre_key_id, pre_key_private_encrypted FROM pre_keys
                 WHERE pre_key_id = ?1 AND signed_pre_key_id IS NULL AND used = 0"
            ).map_err(|e| format!("Prepare error: {}", e))?;

            let result = stmt.query_row([pre_key_id], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, Vec<u8>>(1)?))
            });

            match result {
                Ok((id, encrypted)) => {
                    let decrypted = crate::utils::security::cipher_key::decrypt_key_with_password(
                        &encrypted,
                        &self.password_hash,
                        self.username.as_bytes(),
                    ).map_err(|e| format!("Failed to decrypt one-time pre-key: {:?}", e))?;

                    let key: [u8; 32] = decrypted.try_into()
                        .map_err(|_| "Invalid one-time pre-key length".to_string())?;

                    Ok(Some((id, key)))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query error: {}", e)),
            }
        })
    }
}
