# Backend Rust - Session et base de données

## Session en mémoire (`src/session/mod.rs`)

Chaque utilisateur connecté a une `CachedSession` en mémoire (hashmap globale).

### Pourquoi ce cache ?

La dérivation de clé PBKDF2 (Argon2id + SQLCipher) prend **2-5 secondes**. Sans cache, chaque commande Tauri rouvrirait la DB et re-dériverait les clés. Avec le cache, la session est chaude et les commandes s'exécutent en quelques ms.

### Structure CachedSession

```rust
pub struct CachedSession {
    pub username: String,
    pub password: String,          // conservé pour rouvrir la DB si connexion dropped
    pub session_token: String,     // UUID - clé du cache
    pub password_hash: Vec<u8>,    // dérivé via Argon2id, clé de chiffrement local
    pub dilithium_secret: Vec<u8>, // clé privée Dilithium2 (post-quantique)
    pub kyber_secret: Vec<u8>,     // clé privée Kyber1024
    pub user_hash: Vec<u8>,        // SHA256²(username) - identifiant réseau
    pub user_hash_hex: String,     // idem en hex (64 chars)
    pub registration_id: i64,
    pub identity_key_public: Vec<u8>,
    pub kyber_public_key: Vec<u8>,
    pub x25519_public_key: Option<Vec<u8>>,
    db_conn: Mutex<Option<Connection>>,       // connexion SQLCipher lazily ouverte
    friends_cache: Mutex<Option<Vec<Friend>>>, // cache des contacts
    send_locks: Mutex<HashMap<i64, Arc<Mutex<()>>>>, // locks par conversation
    pub sync_lock: tokio::sync::Mutex<()>,    // empêche les syncs parallèles
}
```

### Accès à la session depuis une commande

```rust
// Dans une commande Tauri :
let session = get_session_by_token_async(session_token).await?;
// → retrouve la CachedSession dans le hashmap global

// Exécuter une requête DB :
session.with_db(|conn| {
    conn.query_row("SELECT ...", [], |row| Ok(row.get(0)?))
        .map_err(|e| format!("Erreur: {}", e))
})?;
```

---

## Base de données utilisateur (`src/db/user.rs`)

Chaque utilisateur a sa propre DB SQLite chiffrée avec SQLCipher.

**Chemin** : `{app_data_dir}/{name_hash}.db`

### Dérivation de la clé SQLCipher

```
clé = PBKDF2-SHA256(password, salt_de_l_utilisateur, iterations)
pragma key = "x'<clé_hex>'"
```

Le `salt` est stocké dans la `MasterDb` (base non chiffrée qui liste les utilisateurs).

### Tables principales

#### `user`
Informations de l'utilisateur local.
```sql
id, pseudo, username_hash, encrypted_network_key, encrypted_identity_keys,
identity_key_dilithium_public, kyber_public_key, x25519_public_key,
registration_id, created_at, avatar
```

#### `friends`
Contacts confirmés.
```sql
id, pseudo, username_hash, identity_key_public, kyber_public_key,
x25519_public_key, friendship_signature_local, friendship_signature_remote,
verified, blocked, created_at, updated_at, avatar
```
> `username_hash` est UNIQUE → "Mon espace" y est stocké avec `username_hash = notre propre hash`.

#### `messages`
Messages chiffrés localement.
```sql
id, friend_id, message_id (UNIQUE), is_outgoing, message_type,
encrypted_content, content_iv, filename, file_size, mime_type,
timestamp, status, delivered_at, read_at
```
> `message_type` : `text` | `image` | `audio` | `video` | `file`
> `status` : `pending` | `sent` | `delivered` | `read` | `failed`

#### `sessions`
États des sessions Double Ratchet (une par conversation).
```sql
friend_id (UNIQUE), root_key_encrypted, sending_chain_key_encrypted,
receiving_chain_key_encrypted, sending_counter, receiving_counter,
dh_public, dh_private_encrypted, remote_dh_public, created_at, last_used_at
```

#### `pre_keys`
Clés pré-calculées pour X3DH.
```sql
pre_key_id (UNIQUE), pre_key_public, pre_key_private_encrypted,
signed_pre_key_id, signed_pre_key_public, used, used_at
```
> Les OTPKs sont marqués `used = 1` mais **pas supprimés** → permet la replay de l'historique sur un nouvel appareil.

#### `pending_friend_requests`
Demandes d'ami en cours.
```sql
direction ('incoming'|'outgoing'), remote_username_hash (UNIQUE),
remote_pseudo, remote_identity_key, remote_kyber_public_key,
remote_x25519_public_key, dilithium_signature, status, message,
created_at, expires_at
```

#### `settings`
Paires clé/valeur pour les paramètres.
```sql
key TEXT PRIMARY KEY, value TEXT
```
> Clés importantes : `last_message_sync`, `last_response_sync_ts`, `last_accepted_sync_ts`

#### `chat_settings`
TTL DHT par conversation.
```sql
friend_id (PK), ttl_hours (0 = jamais)
```

#### `paired_devices`
Appareils jumelés pour le relay.
```sql
peer_dilithium_pubkey (PK), sync_key, added_at
```

#### `relay_cursors`
Curseur pour le relay multi-appareils.
```sql
id = 1, last_relay_id
```

---

## MasterDb (`src/db/master.rs`)

Base SQLite **non chiffrée** qui liste les utilisateurs.

```sql
users: name_hash, salt, created_at
```

Permet d'ouvrir la DB utilisateur correcte depuis le `name_hash` (dérivé du username).

---

## Migrations idempotentes

Les extensions de schéma (colonnes ajoutées après la création initiale) sont dans `UserDb::open_with_entry()` :

```rust
conn.execute_batch("
    CREATE TABLE IF NOT EXISTS paired_devices (...);
    CREATE TABLE IF NOT EXISTS chat_settings (...);
    ...
");
// ALTER TABLE avec gestion de l'erreur "duplicate column"
```

Pas besoin de numéros de migration : toutes les modifications sont idempotentes.
