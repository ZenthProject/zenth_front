use rusqlite::Connection;

use crate::db::crypto::{derive_sqlcipher_key, key_to_sqlcipher_pragma};
use crate::db::master::{MasterDb, UserEntry};
use crate::db::error::DbError;
use crate::utils::timestamp::plateform::current_timestamp;

/// Base de donnees utilisateur chiffree avec SQLCipher
pub struct UserDb {
    conn: Connection,
    name_hash: String,
}

impl UserDb {
    /// Ouvre une base de donnees utilisateur existante
    /// Le mot de passe est utilise pour deriver la cle de chiffrement
    pub fn open(username: &str, password: &str) -> Result<Self, DbError> {
        let master = MasterDb::open()?;
        let entry = master.get_user(username)?;
        Self::open_with_entry(&entry, password)
    }

    pub fn open_with_entry(entry: &UserEntry, password: &str) -> Result<Self, DbError> {
        let db_path = MasterDb::user_db_path(&entry.name_hash);
        let key = derive_sqlcipher_key(password, &entry.salt)?;
        let pragma_key = key_to_sqlcipher_pragma(&key);
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "key", &*pragma_key)?;        
        
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
            .map_err(|_| DbError::InvalidPassword)?;

        // WAL mode: allows concurrent reads + 1 writer; busy_timeout retries on SQLITE_BUSY
        let _ = conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;");

        conn.pragma_update(None, "secure_delete", "ON")?;

        // Schema extensions (idempotent)
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS paired_devices (
                peer_dilithium_pubkey BLOB PRIMARY KEY,
                sync_key              BLOB NOT NULL,
                added_at              INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS relay_cursors (
                id            INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
                last_relay_id INTEGER NOT NULL DEFAULT 0
            );
            INSERT OR IGNORE INTO relay_cursors (id, last_relay_id) VALUES (1, 0);
            CREATE TABLE IF NOT EXISTS chat_settings (
                friend_id  INTEGER NOT NULL PRIMARY KEY REFERENCES friends(id) ON DELETE CASCADE,
                ttl_hours  INTEGER NOT NULL DEFAULT 0
            );"
        );

        for sql in &[
            "ALTER TABLE friends ADD COLUMN avatar BLOB",
            "ALTER TABLE user ADD COLUMN avatar BLOB",
            "ALTER TABLE messages ADD COLUMN vault_encrypted INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE friends ADD COLUMN pin_hash TEXT",
        ] {
            match conn.execute(sql, []) {
                Ok(_) => {}
                Err(e) => {
                    let msg = e.to_string();
                    if !msg.contains("duplicate column") {
                        return Err(DbError::InvalidOperation(format!("Migration failed: {}", e)));
                    }
                }
            }
        }

        let db = Self {
            conn,
            name_hash: entry.name_hash.clone(),
        };

        Ok(db)
    }

    pub fn create(username: &str, password: &str) -> Result<Self, DbError> {
        let master = MasterDb::open()?;
        let entry = master.register_user(username)?;
        let key = derive_sqlcipher_key(password, &entry.salt)?;
        let pragma_key = key_to_sqlcipher_pragma(&key);
        let db_path = MasterDb::user_db_path(&entry.name_hash);
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "key", &*pragma_key)?;

        let _ = conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;");
        conn.pragma_update(None, "secure_delete", "ON")?;

        let db = Self {
            conn,
            name_hash: entry.name_hash.clone(),
        };

        db.init_schema()?;

        Ok(db)
    }

    /// Initialise le schema de la base de donnees utilisateur
    fn init_schema(&self) -> Result<(), DbError> {
        // Note: On ne peut pas utiliser rusqlite_migration avec SQLCipher
        // car il ouvre une nouvelle connexion sans la cle
        // On execute les migrations manuellement

        self.conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            -- Table utilisateur principal
            CREATE TABLE IF NOT EXISTS user (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pseudo TEXT NOT NULL,
                username_hash TEXT NOT NULL UNIQUE,
                encrypted_network_key BLOB NOT NULL,
                network_key_iv BLOB NOT NULL,
                encrypted_identity_keys BLOB NOT NULL,
                identity_keys_iv BLOB NOT NULL,
                identity_key_dilithium_public BLOB NOT NULL,
                kyber_public_key BLOB NOT NULL,
                x25519_public_key BLOB,
                registration_id INTEGER NOT NULL UNIQUE,
                created_at INTEGER NOT NULL,
                avatar BLOB
            );

            -- Pre-keys pour X3DH
            CREATE TABLE IF NOT EXISTS pre_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                pre_key_id INTEGER NOT NULL UNIQUE,
                pre_key_public BLOB NOT NULL,
                pre_key_private_encrypted BLOB NOT NULL,
                pre_key_iv BLOB NOT NULL,
                signed_pre_key_id INTEGER,
                signed_pre_key_public BLOB,
                signed_pre_key_signature BLOB,
                pq_pre_key_id INTEGER,
                pq_pre_key_public BLOB,
                created_at INTEGER NOT NULL,
                used INTEGER NOT NULL DEFAULT 0,
                used_at INTEGER,
                FOREIGN KEY(user_id) REFERENCES user(id) ON DELETE CASCADE
            );

            -- Contacts/Amis
            CREATE TABLE IF NOT EXISTS friends (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pseudo TEXT NOT NULL,
                username_hash TEXT NOT NULL UNIQUE,
                identity_key_public BLOB NOT NULL,
                kyber_public_key BLOB,
                x25519_public_key BLOB,
                shared_secret_encrypted BLOB,
                shared_secret_iv BLOB,
                -- Signatures d'amitie Dilithium (preuve cryptographique d'amitie mutuelle)
                friendship_signature_local BLOB,   -- Notre signature de l'amitie
                friendship_signature_remote BLOB,  -- Signature de l'ami
                verified INTEGER NOT NULL DEFAULT 0,
                blocked INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                avatar BLOB
            );

            -- Messages chiffres
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                friend_id INTEGER NOT NULL,
                message_id TEXT NOT NULL UNIQUE,
                is_outgoing INTEGER NOT NULL,
                message_type TEXT NOT NULL DEFAULT 'text',
                encrypted_content BLOB NOT NULL,
                content_iv BLOB NOT NULL,
                filename TEXT,
                file_size INTEGER,
                mime_type TEXT,
                file_hash BLOB,
                thumbnail_encrypted BLOB,
                thumbnail_iv BLOB,
                timestamp INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                delivered_at INTEGER,
                read_at INTEGER,
                reply_to_id TEXT,
                edited INTEGER NOT NULL DEFAULT 0,
                edited_at INTEGER,
                deleted INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER,
                vault_encrypted INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(friend_id) REFERENCES friends(id) ON DELETE CASCADE,
                CHECK (message_type IN ('text', 'image', 'file', 'audio', 'video')),
                CHECK (status IN ('pending', 'sent', 'delivered', 'read', 'failed'))
            );

            -- Sessions Double Ratchet
            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                friend_id INTEGER NOT NULL UNIQUE,
                root_key_encrypted BLOB NOT NULL,
                root_key_iv BLOB NOT NULL,
                sending_chain_key_encrypted BLOB NOT NULL,
                sending_chain_iv BLOB NOT NULL,
                receiving_chain_key_encrypted BLOB,
                receiving_chain_iv BLOB,
                sending_counter INTEGER NOT NULL DEFAULT 0,
                receiving_counter INTEGER NOT NULL DEFAULT 0,
                dh_public BLOB NOT NULL,
                dh_private_encrypted BLOB NOT NULL,
                dh_private_iv BLOB NOT NULL,
                remote_dh_public BLOB,
                created_at INTEGER NOT NULL,
                last_used_at INTEGER NOT NULL,
                FOREIGN KEY(friend_id) REFERENCES friends(id) ON DELETE CASCADE
            );

            -- Noeuds de relais (Tor/I2P/Lokinet)
            CREATE TABLE IF NOT EXISTS relay_nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_address TEXT NOT NULL UNIQUE,
                node_port INTEGER NOT NULL,
                node_type TEXT NOT NULL,
                public_key BLOB,
                reliability_score REAL NOT NULL DEFAULT 0.5,
                latency_ms INTEGER,
                last_used_at INTEGER,
                last_checked_at INTEGER,
                is_trusted INTEGER NOT NULL DEFAULT 0,
                is_blacklisted INTEGER NOT NULL DEFAULT 0,
                added_at INTEGER NOT NULL,
                CHECK (node_type IN ('tor', 'i2p', 'lokinet')),
                CHECK (reliability_score >= 0.0 AND reliability_score <= 1.0)
            );

            -- Parametres utilisateur
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- Demandes d'ami en attente
            CREATE TABLE IF NOT EXISTS pending_friend_requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                direction TEXT NOT NULL CHECK (direction IN ('incoming', 'outgoing')),
                remote_username_hash TEXT NOT NULL UNIQUE,
                remote_pseudo TEXT,
                remote_identity_key BLOB NOT NULL,
                remote_kyber_public_key BLOB,
                remote_x25519_public_key BLOB,
                dilithium_signature BLOB NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected', 'expired')),
                message TEXT,
                created_at INTEGER NOT NULL,
                expires_at INTEGER
            );

            -- Paramètres par conversation (TTL DHT, etc.)
            CREATE TABLE IF NOT EXISTS chat_settings (
                friend_id  INTEGER NOT NULL PRIMARY KEY REFERENCES friends(id) ON DELETE CASCADE,
                ttl_hours  INTEGER NOT NULL DEFAULT 0
            );

            -- Index pour les performances
            CREATE INDEX IF NOT EXISTS idx_messages_friend ON messages(friend_id);
            CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);
            CREATE INDEX IF NOT EXISTS idx_friends_username ON friends(username_hash);
            CREATE INDEX IF NOT EXISTS idx_pre_keys_user ON pre_keys(user_id);
            CREATE INDEX IF NOT EXISTS idx_pending_requests_status ON pending_friend_requests(status);
            CREATE INDEX IF NOT EXISTS idx_pending_requests_direction ON pending_friend_requests(direction);
            "
        )?;

        Ok(())
    }

    /// Retourne une reference a la connexion
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Retourne une reference mutable a la connexion
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Retourne le hash du nom d'utilisateur
    pub fn name_hash(&self) -> &str {
        &self.name_hash
    }

    /// Force le checkpoint WAL et tronque le journal (anti-forensic).
    /// À appeler avant de fermer la connexion dans des contextes sensibles.
    pub fn checkpoint_wal(&self) -> Result<(), DbError> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// Verifie si la base de donnees est valide
    pub fn is_valid(&self) -> bool {
        self.conn.execute("SELECT 1", []).is_ok()
    }

    /// Change le mot de passe de la base de donnees
    pub fn change_password(&self, old_password: &str, new_password: &str) -> Result<(), DbError> {
        let master = MasterDb::open()?;
        let entry = master.get_user_by_hash(&self.name_hash)?;

        // Verifier l'ancien mot de passe
        let old_key = derive_sqlcipher_key(old_password, &entry.salt)?;
        let old_pragma = key_to_sqlcipher_pragma(&old_key);

        // Generer une nouvelle cle
        let new_key = derive_sqlcipher_key(new_password, &entry.salt)?;
        let new_pragma = key_to_sqlcipher_pragma(&new_key);

        // Changer la cle SQLCipher
        self.conn.pragma_update(None, "rekey", &*new_pragma)?;

        Ok(())
    }
}

// ============================================================================
// QUERIES CRUD
// ============================================================================

impl UserDb {
    // === USER ===

    /// Recupere la publics key de l'utilisateur pour syncronisation des deux comptes.
    pub fn get_user_sync(&self) -> Result<UserInfoSync, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pseudo, username_hash, identity_key_dilithium_public,
                    kyber_public_key, x25519_public_key
             FROM user LIMIT 1"
        )?;

        let info = stmt.query_row([], |row| {
            Ok(UserInfoSync {
                id: row.get(0)?,
                pseudo: row.get(1)?,
                username_hash: row.get(2)?,
                identity_key_public: row.get(3)?,
                kyber_public_key: row.get(4)?,
                x25519_public_key: row.get(5)?
            })
        }).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound("Account data not found. Please re-register.".to_string()),
            other => DbError::Sqlite(other),
        })?;

        Ok(info)
    }


    /// Recupere les informations de l'utilisateur
    pub fn get_user_info(&self) -> Result<UserInfo, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pseudo, username_hash, identity_key_dilithium_public,
                    kyber_public_key, x25519_public_key, registration_id, created_at,
                    encrypted_identity_keys, identity_keys_iv
             FROM user LIMIT 1"
        )?;

        let info = stmt.query_row([], |row| {
            Ok(UserInfo {
                id: row.get(0)?,
                pseudo: row.get(1)?,
                username_hash: row.get(2)?,
                identity_key_public: row.get(3)?,
                kyber_public_key: row.get(4)?,
                x25519_public_key: row.get(5)?,
                registration_id: row.get(6)?,
                created_at: row.get(7)?,
                encrypted_identity_keys: row.get(8)?,
                identity_keys_iv: row.get(9)?,
            })
        }).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound("Account data not found. Please re-register.".to_string()),
            other => DbError::Sqlite(other),
        })?;

        Ok(info)
    }

    /// Remplace le username_hash local (après adoption de l'identité d'un appareil de confiance)
    pub fn update_username_hash(&self, new_hash_hex: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE user SET username_hash = ?1",
            [new_hash_hex],
        )?;
        Ok(())
    }

    /// Sauvegarde les informations utilisateur
    pub fn save_user_info(&self, info: &NewUserInfo) -> Result<i64, DbError> {
        self.conn.execute(
            "INSERT INTO user (pseudo, username_hash, encrypted_network_key, network_key_iv,
                encrypted_identity_keys, identity_keys_iv, identity_key_dilithium_public,
                kyber_public_key, x25519_public_key, registration_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                info.pseudo,
                info.username_hash,
                info.encrypted_network_key,
                info.network_key_iv,
                info.encrypted_identity_keys,
                info.identity_keys_iv,
                info.identity_key_public,
                info.kyber_public_key,
                info.x25519_public_key,
                info.registration_id,
                info.created_at,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    // === FRIENDS ===

    /// Ajoute un ami
    pub fn add_friend(&self, friend: &NewFriend) -> Result<i64, DbError> {
        self.conn.execute(
            "INSERT INTO friends (pseudo, username_hash, identity_key_public, kyber_public_key,
                x25519_public_key, friendship_signature_local, friendship_signature_remote,
                verified, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            rusqlite::params![
                friend.pseudo,
                friend.username_hash,
                friend.identity_key_public,
                friend.kyber_public_key,
                friend.x25519_public_key,
                friend.friendship_signature_local,
                friend.friendship_signature_remote,
                friend.verified,
                friend.created_at,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Liste tous les amis non-bloqués et non-verrouillés
    pub fn list_friends(&self) -> Result<Vec<Friend>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                    x25519_public_key, friendship_signature_local, friendship_signature_remote,
                    verified, blocked, created_at, updated_at, avatar
             FROM friends WHERE blocked = 0 AND pin_hash IS NULL ORDER BY pseudo"
        )?;

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
        })?;

        let mut result = Vec::new();
        for friend in friends {
            result.push(friend?);
        }

        Ok(result)
    }

    /// Retourne les (id, pin_hash) de toutes les convs verrouillées
    pub fn list_locked_friend_hashes(&self) -> Result<Vec<(i64, String)>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pin_hash FROM friends WHERE pin_hash IS NOT NULL AND blocked = 0"
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// Récupère un ami verrouillé par id (pour l'afficher après unlock)
    pub fn get_locked_friend(&self, id: i64) -> Result<Friend, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                    x25519_public_key, friendship_signature_local, friendship_signature_remote,
                    verified, blocked, created_at, updated_at, avatar
             FROM friends WHERE id = ?1 AND pin_hash IS NOT NULL"
        )?;
        let friend = stmt.query_row([id], |row| {
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
        })?;
        Ok(friend)
    }

    /// Pose un verrou sur une conversation (pin_hash = argon2 du PIN)
    pub fn set_friend_pin(&self, friend_id: i64, pin_hash: &str) -> Result<(), DbError> {
        let updated = self.conn.execute(
            "UPDATE friends SET pin_hash = ?1 WHERE id = ?2 AND blocked = 0",
            rusqlite::params![pin_hash, friend_id],
        )?;
        if updated == 0 {
            return Err(DbError::NotFound(format!("Friend {} not found", friend_id)));
        }
        Ok(())
    }

    /// Retire le verrou d'une conversation
    pub fn clear_friend_pin(&self, friend_id: i64) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE friends SET pin_hash = NULL WHERE id = ?1",
            rusqlite::params![friend_id],
        )?;
        Ok(())
    }

    /// Recupere un ami par son ID
    pub fn get_friend(&self, id: i64) -> Result<Friend, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                    x25519_public_key, friendship_signature_local, friendship_signature_remote,
                    verified, blocked, created_at, updated_at, avatar
             FROM friends WHERE id = ?1"
        )?;

        let friend = stmt.query_row([id], |row| {
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
        })?;

        Ok(friend)
    }

    // === MESSAGES ===

    /// Sauvegarde un message
    pub fn save_message(&self, msg: &NewMessage) -> Result<i64, DbError> {
        self.conn.execute(
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
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Liste les messages d'une conversation
    pub fn list_messages(&self, friend_id: i64, limit: i64, offset: i64) -> Result<Vec<Message>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, friend_id, message_id, is_outgoing, message_type,
                    encrypted_content, content_iv, filename, file_size, mime_type,
                    timestamp, status, delivered_at, read_at, reply_to_id, edited, deleted,
                    vault_encrypted
             FROM messages
             WHERE friend_id = ?1 AND deleted = 0
             ORDER BY timestamp DESC
             LIMIT ?2 OFFSET ?3"
        )?;

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
        })?;

        let mut result = Vec::new();
        for msg in messages {
            result.push(msg?);
        }

        Ok(result)
    }

    /// Met a jour le statut d'un message
    pub fn update_message_status(&self, message_id: &str, status: &str) -> Result<(), DbError> {
        let now = current_timestamp() as i64;

        let (delivered, read) = match status {
            "delivered" => (Some(now), None),
            "read" => (None, Some(now)),
            _ => (None, None),
        };

        self.conn.execute(
            "UPDATE messages SET status = ?1, delivered_at = COALESCE(?2, delivered_at),
             read_at = COALESCE(?3, read_at) WHERE message_id = ?4",
            rusqlite::params![status, delivered, read, message_id],
        )?;

        Ok(())
    }

    // === SUPPRESSION SÉCURISÉE ===

    /// Supprime un message de façon définitive :
    /// 1. PRAGMA secure_delete ON  → SQLite écrase les pages libérées avec des zéros
    /// 2. Écrase manuellement encrypted_content avec des zéros (double protection)
    /// 3. DELETE du row
    pub fn delete_message_secure(&self, message_id: &str) -> Result<(), DbError> {
        // Active l'effacement sécurisé pour cette connexion
        self.conn.execute_batch("PRAGMA secure_delete = ON;")?;

        // Récupère la taille du blob pour générer un vecteur de zéros de la bonne taille
        let blob_size: Option<i64> = self.conn.query_row(
            "SELECT length(encrypted_content) FROM messages WHERE message_id = ?1",
            [message_id],
            |row| row.get(0),
        ).ok();

        if let Some(size) = blob_size {
            let zeros = vec![0u8; size as usize];
            // Écrase le contenu chiffré avec des zéros avant suppression
            self.conn.execute(
                "UPDATE messages SET encrypted_content = ?1, filename = NULL,
                 mime_type = NULL, file_size = NULL WHERE message_id = ?2",
                rusqlite::params![zeros, message_id],
            )?;
        }

        // Suppression définitive (secure_delete ON écrase à nouveau les pages)
        self.conn.execute(
            "DELETE FROM messages WHERE message_id = ?1",
            [message_id],
        )?;

        Ok(())
    }

    // === SETTINGS ===

    /// Recupere un parametre
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;

        let result = stmt.query_row([key], |row| row.get(0));

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    /// Sauvegarde un parametre
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;

        Ok(())
    }

    // Paired devices
    pub fn save_paired_device(&self, peer_pubkey: &[u8], sync_key: &[u8]) -> Result<(), DbError> {
        let now = crate::utils::timestamp::plateform::current_timestamp() as i64;
        self.conn.execute(
            "INSERT INTO paired_devices (peer_dilithium_pubkey, sync_key, added_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(peer_dilithium_pubkey) DO UPDATE SET sync_key = excluded.sync_key, added_at = excluded.added_at",
            rusqlite::params![peer_pubkey, sync_key, now],
        )?;
        Ok(())
    }

    pub fn get_all_paired_devices(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT peer_dilithium_pubkey, sync_key FROM paired_devices"
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_paired_devices_full(&self) -> Result<Vec<PairedDeviceInfo>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT peer_dilithium_pubkey, added_at FROM paired_devices ORDER BY added_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PairedDeviceInfo {
                peer_dilithium_pubkey: row.get(0)?,
                added_at: row.get(1)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn remove_paired_device(&self, peer_pubkey: &[u8]) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM paired_devices WHERE peer_dilithium_pubkey = ?1",
            rusqlite::params![peer_pubkey],
        )?;
        Ok(())
    }

    // Relay cursor
    pub fn get_relay_cursor(&self) -> Result<i64, DbError> {
        self.conn.query_row(
            "SELECT last_relay_id FROM relay_cursors WHERE id = 1",
            [],
            |row| row.get(0),
        ).map_err(DbError::Sqlite)
    }

    pub fn set_relay_cursor(&self, id: i64) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE relay_cursors SET last_relay_id = ?1 WHERE id = 1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    /// Recupere tous les parametres
    pub fn get_all_settings(&self) -> Result<Vec<(String, String)>, DbError> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;

        let settings = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut result = Vec::new();
        for setting in settings {
            result.push(setting?);
        }

        Ok(result)
    }

    // === PENDING FRIEND REQUESTS ===

    /// Ajoute une demande d'ami en attente (upsert - écrase les lignes stales)
    pub fn add_pending_request(&self, request: &NewPendingRequest) -> Result<i64, DbError> {
        self.conn.execute(
            "INSERT INTO pending_friend_requests
             (direction, remote_username_hash, remote_pseudo, remote_identity_key,
              remote_kyber_public_key, remote_x25519_public_key, dilithium_signature,
              status, message, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(remote_username_hash) DO UPDATE SET
               direction          = excluded.direction,
               remote_pseudo      = excluded.remote_pseudo,
               remote_identity_key = excluded.remote_identity_key,
               remote_kyber_public_key  = excluded.remote_kyber_public_key,
               remote_x25519_public_key = excluded.remote_x25519_public_key,
               dilithium_signature = excluded.dilithium_signature,
               status      = excluded.status,
               message     = excluded.message,
               created_at  = excluded.created_at,
               expires_at  = excluded.expires_at",
            rusqlite::params![
                request.direction,
                request.remote_username_hash,
                request.remote_pseudo,
                request.remote_identity_key,
                request.remote_kyber_public_key,
                request.remote_x25519_public_key,
                request.dilithium_signature,
                request.status,
                request.message,
                request.created_at,
                request.expires_at,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Liste les demandes d'ami en attente
    pub fn list_pending_requests(&self, direction: Option<&str>) -> Result<Vec<PendingRequest>, DbError> {
        let query = match direction {
            Some(_) => {
                "SELECT id, direction, remote_username_hash, remote_pseudo, remote_identity_key,
                        remote_kyber_public_key, remote_x25519_public_key, dilithium_signature,
                        status, message, created_at, expires_at
                 FROM pending_friend_requests
                 WHERE status = 'pending' AND direction = ?1
                 ORDER BY created_at DESC"
            }
            None => {
                "SELECT id, direction, remote_username_hash, remote_pseudo, remote_identity_key,
                        remote_kyber_public_key, remote_x25519_public_key, dilithium_signature,
                        status, message, created_at, expires_at
                 FROM pending_friend_requests
                 WHERE status = 'pending'
                 ORDER BY created_at DESC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let requests = if let Some(dir) = direction {
            stmt.query_map([dir], Self::map_pending_request)?
        } else {
            stmt.query_map([], Self::map_pending_request)?
        };

        let mut result = Vec::new();
        for req in requests {
            result.push(req?);
        }

        Ok(result)
    }

    /// Mapper pour PendingRequest
    fn map_pending_request(row: &rusqlite::Row) -> rusqlite::Result<PendingRequest> {
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
    }

    /// Recupere une demande d'ami par hash
    pub fn get_pending_request_by_hash(&self, remote_hash: &str) -> Result<Option<PendingRequest>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, direction, remote_username_hash, remote_pseudo, remote_identity_key,
                    remote_kyber_public_key, remote_x25519_public_key, dilithium_signature,
                    status, message, created_at, expires_at
             FROM pending_friend_requests
             WHERE remote_username_hash = ?1"
        )?;

        let result = stmt.query_row([remote_hash], Self::map_pending_request);

        match result {
            Ok(req) => Ok(Some(req)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    /// Met a jour le statut d'une demande d'ami
    pub fn update_pending_request_status(&self, remote_hash: &str, status: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE pending_friend_requests SET status = ?1 WHERE remote_username_hash = ?2",
            [status, remote_hash],
        )?;

        Ok(())
    }

    /// Supprime une demande d'ami
    pub fn delete_pending_request(&self, remote_hash: &str) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM pending_friend_requests WHERE remote_username_hash = ?1",
            [remote_hash],
        )?;

        Ok(())
    }

    /// Supprime les demandes expirees
    pub fn cleanup_expired_requests(&self) -> Result<usize, DbError> {
        let now = current_timestamp() as i64;

        let count = self.conn.execute(
            "DELETE FROM pending_friend_requests WHERE expires_at IS NOT NULL AND expires_at < ?1",
            [now],
        )?;

        Ok(count)
    }

    /// Recupere un ami par son hash
    pub fn get_friend_by_hash(&self, username_hash: &str) -> Result<Option<Friend>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pseudo, username_hash, identity_key_public, kyber_public_key,
                    x25519_public_key, friendship_signature_local, friendship_signature_remote,
                    verified, blocked, created_at, updated_at, avatar
             FROM friends WHERE username_hash = ?1"
        )?;

        let result = stmt.query_row([username_hash], |row| {
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
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    /// Met a jour la signature d'amitie distante d'un ami
    /// Appelee quand on recoit une reponse d'acceptation du serveur
    pub fn update_friend_remote_signature(
        &self,
        username_hash: &str,
        remote_signature: &[u8],
    ) -> Result<(), DbError> {
        let now = current_timestamp() as i64;

        self.conn.execute(
            "UPDATE friends SET friendship_signature_remote = ?1, updated_at = ?2 WHERE username_hash = ?3",
            rusqlite::params![remote_signature, now, username_hash],
        )?;
        Ok(())
    }

    /// Supprime un ami
    pub fn remove_friend(&self, friend_id: i64) -> Result<(), DbError> {
        self.conn.execute("DELETE FROM friends WHERE id = ?1", [friend_id])?;
        Ok(())
    }

    /// Met a jour l'avatar d'un ami (BLOB)
    pub fn set_friend_avatar(&self, friend_id: i64, avatar: &[u8]) -> Result<(), DbError> {
        let now = current_timestamp() as i64;
        self.conn.execute(
            "UPDATE friends SET avatar = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![avatar, now, friend_id],
        )?;
        Ok(())
    }

    /// Recupere l'avatar de l'utilisateur (BLOB)
    pub fn get_my_avatar(&self) -> Result<Option<Vec<u8>>, DbError> {
        let result = self.conn.query_row(
            "SELECT avatar FROM user LIMIT 1",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(avatar) => Ok(avatar),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sqlite(e)),
        }
    }

    /// Met a jour l'avatar de l'utilisateur (BLOB)
    pub fn set_my_avatar(&self, avatar: &[u8]) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE user SET avatar = ?1",
            rusqlite::params![avatar],
        )?;
        Ok(())
    }
}

// ============================================================================
// TYPES
// ============================================================================

#[derive(Debug, Clone)]
pub struct PairedDeviceInfo {
    pub peer_dilithium_pubkey: Vec<u8>,
    pub added_at: i64,
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub id: i64,
    pub pseudo: String,
    pub username_hash: String,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Vec<u8>,
    pub x25519_public_key: Option<Vec<u8>>,
    pub registration_id: i64,
    pub created_at: i64,
    pub encrypted_identity_keys: Vec<u8>,
    pub identity_keys_iv: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct UserInfoSync {
    pub id: i64,
    pub pseudo: String,
    pub username_hash: String,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Vec<u8>,
    pub x25519_public_key: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct NewUserInfo {
    pub pseudo: String,
    pub username_hash: String,
    pub encrypted_network_key: Vec<u8>,
    pub network_key_iv: Vec<u8>,
    pub encrypted_identity_keys: Vec<u8>,
    pub identity_keys_iv: Vec<u8>,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Vec<u8>,
    pub x25519_public_key: Option<Vec<u8>>,
    pub registration_id: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct Friend {
    pub id: i64,
    pub pseudo: String,
    pub username_hash: String,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Option<Vec<u8>>,
    pub x25519_public_key: Option<Vec<u8>>,
    pub friendship_signature_local: Option<Vec<u8>>,  // Notre signature Dilithium de l'amitie
    pub friendship_signature_remote: Option<Vec<u8>>, // Signature Dilithium de l'ami
    pub verified: bool,
    pub blocked: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub avatar: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct NewFriend {
    pub pseudo: String,
    pub username_hash: String,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Option<Vec<u8>>,
    pub x25519_public_key: Option<Vec<u8>>,
    pub friendship_signature_local: Option<Vec<u8>>,  // Notre signature de l'amitie
    pub friendship_signature_remote: Option<Vec<u8>>, // Signature de l'ami (reçue lors de l'acceptation)
    pub verified: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: i64,
    pub friend_id: i64,
    pub message_id: String,
    pub is_outgoing: bool,
    pub message_type: String,
    pub encrypted_content: Vec<u8>,
    pub content_iv: Vec<u8>,
    pub filename: Option<String>,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub timestamp: i64,
    pub status: String,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub reply_to_id: Option<String>,
    pub edited: bool,
    pub deleted: bool,
    pub vault_encrypted: bool,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub friend_id: i64,
    pub message_id: String,
    pub is_outgoing: bool,
    pub message_type: String,
    pub encrypted_content: Vec<u8>,
    pub content_iv: Vec<u8>,
    pub filename: Option<String>,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub timestamp: i64,
    pub status: String,
    pub vault_encrypted: bool,
    pub reply_to_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub id: i64,
    pub direction: String,
    pub remote_username_hash: String,
    pub remote_pseudo: Option<String>,
    pub remote_identity_key: Vec<u8>,
    pub remote_kyber_public_key: Option<Vec<u8>>,
    pub remote_x25519_public_key: Option<Vec<u8>>,
    pub dilithium_signature: Vec<u8>,
    pub status: String,
    pub message: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewPendingRequest {
    pub direction: String,
    pub remote_username_hash: String,
    pub remote_pseudo: Option<String>,
    pub remote_identity_key: Vec<u8>,
    pub remote_kyber_public_key: Option<Vec<u8>>,
    pub remote_x25519_public_key: Option<Vec<u8>>,
    pub dilithium_signature: Vec<u8>,
    pub status: String,
    pub message: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}
