# DHT - Serveur

## Technologies

- **Rust** + **Axum** (HTTP/WebSocket)
- **Diesel** (ORM) + **PostgreSQL**
- **TLS 1.3** (rustls)
- **Protobuf** (zenth_dto) pour la sérialisation

---

## Architecture de routage

Un seul endpoint HTTP `POST /` reçoit toutes les requêtes.

```
DhtRequest {
    method: i32,    // numéro de méthode
    payload: bytes, // message protobuf selon la méthode
    timestamp: u64,
    request_id: bytes,
}
```

Le `method` est invisible de l'extérieur car chiffré par TLS. Cela rend le traffic uniforme et difficile à analyser.

---

## Table des méthodes

| N° | Constante | Description |
|----|-----------|-------------|
| 1 | REGISTER | Inscription |
| 2 | LOGIN | Authentification challenge |
| 3 | DELETE | Suppression du compte |
| 6 | CONTACT | Envoyer une demande d'ami |
| 10 | FETCH_FRIEND_REQUESTS | Récupérer les demandes reçues |
| 11 | RESPOND_FRIEND_REQUEST | Accepter/Refuser une demande |
| 12 | SEND_MESSAGE | Envoyer un message |
| 13 | FETCH_MESSAGES | Récupérer les messages |
| 14 | FETCH_FRIEND_RESPONSES | Récupérer les réponses à nos demandes |
| 15 | UPLOAD_PREKEYS | Uploader des pre-keys X3DH |
| 16 | FETCH_PREKEY_BUNDLE | Récupérer le bundle PreKey d'un utilisateur |
| 17 | CHECK_PREKEY_COUNT | Vérifier le stock d'OTPKs |
| 18 | REPLENISH_PREKEYS | Recharger les OTPKs |
| 19 | GET_UPDATE_MANIFEST | Manifest de mise à jour |
| 20 | GET_UPDATE_CHUNK | Chunk de mise à jour |
| 21 | SYNC_PUSH_BLOB | Publier un blob de pairing |
| 22 | SYNC_FETCH_BLOB | Récupérer un blob de pairing |
| 23 | SYNC_DELETE_BLOB | Supprimer un blob de pairing |
| 24 | RELAY_PUSH | Envoyer un message relay |
| 25 | RELAY_FETCH | Récupérer les messages relay |
| 26 | RELAY_ACK | Accuser réception des messages relay |
| 27 | FETCH_MY_ACCEPTED | Contacts qu'on a acceptés (responder) |
| 28 | ACK_MESSAGE | Supprimer un message (ack réception) |

---

## Schéma PostgreSQL

### `users`
```sql
user_hash_id BYTEA PRIMARY KEY,   -- SHA256²(username)
password_commitment BYTEA,         -- hash du mot de passe (ZKP)
identity_key_dilithium BYTEA,      -- clé publique Dilithium2 (1312 bytes)
identity_signature BYTEA,
pre_key_bundle BYTEA,
proof_type INT,
created_at TIMESTAMP
```

### `messages`
```sql
id SERIAL PRIMARY KEY,
message_id BYTEA,
sender_hash_id BYTEA,
recipient_hash_id BYTEA,
content BYTEA,                     -- ZenthSignalEnvelope chiffré
dilithium_signature BYTEA,
timestamp BIGINT,
server_timestamp BIGINT,
delivered BOOL,
created_at TIMESTAMP,
expires_at TIMESTAMP               -- TTL (9999-12-31 si jamais)
```

### `friend_requests`
```sql
id SERIAL PRIMARY KEY,
requester_hash_id BYTEA,
target_hash_id BYTEA,
pre_key_bundle BYTEA,              -- clés publiques du demandeur
dilithium_signature BYTEA,
encrypted_message BYTEA,
timestamp BIGINT,
created_at TIMESTAMP
```

### `friend_responses`
```sql
id SERIAL PRIMARY KEY,
request_id INT → friend_requests,
responder_hash_id BYTEA,
requester_hash_id BYTEA,
accepted BOOL,
pre_key_bundle BYTEA,              -- clés du responder
dilithium_signature BYTEA,
delivered BOOL,
created_at TIMESTAMP
```

### `one_time_prekeys`
```sql
id SERIAL, user_hash_id BYTEA,
prekey_id INT, public_key BYTEA,
used BOOL, created_at TIMESTAMP
```

### `sync_blobs`
```sql
for_device_dilithium_pubkey BYTEA PRIMARY KEY,
ciphertext BYTEA, signature BYTEA,
expires_at TIMESTAMP               -- TTL max 1h (pairing éphémère)
```

### `relay_messages`
```sql
id BIGSERIAL PRIMARY KEY,
for_device_dilithium_pubkey BYTEA,
ciphertext BYTEA, nonce BYTEA,
expires_at TIMESTAMP               -- TTL max 24h
```

---

## Sécurité serveur

### Authentification des requêtes

Chaque requête sensible est signée avec Dilithium2 :

```
message_to_verify = données_de_la_requête (varie par méthode)
verify_dilithium_signature(user.identity_key_dilithium, message, signature)
```

Le serveur **ne peut pas** :
- Lire les messages (chiffrés avec X3DH+Ratchet)
- Lire les contacts (pseudos jamais transmis)
- Identifier les correspondants (seuls les hashes circulen)

### Validation de la suppression (METHOD 28)

```rust
// Seul le destinataire peut supprimer son message
DELETE FROM messages
WHERE message_id = ? AND recipient_hash_id = ?  -- double contrainte
// Signature Dilithium vérifiée avant la suppression
```

---

## Tâche de nettoyage (main.rs)

```rust
tokio::spawn(async {
    loop {
        sleep(3600s).await;
        DELETE FROM messages WHERE expires_at < NOW();
    }
});
```

Exécutée toutes les heures. Supprime les messages expirés selon le TTL configuré par l'expéditeur.

---

## WebSocket

Le serveur maintient des connexions WebSocket persistantes pour les notifications push.

```
GET / (WebSocket upgrade) → connexion identifiée par user_hash_id
POST / (méthode 12)       → stocke le message → push notification au destinataire
```

Notification poussée :
```protobuf
WsNotification {
    notification_type: MESSAGE_RECEIVED | FRIEND_REQUEST_ACCEPTED | ...,
    timestamp: uint64,
    payload: bytes,  // MessageNotification encodé
}
```

---

## Ajouter une nouvelle méthode

1. Créer `src/handlers/method/ma_methode.rs` avec la fonction handler
2. Ajouter `pub mod ma_methode;` dans `src/handlers/method/mod.rs`
3. Ajouter `const METHOD_MA_METHODE: i32 = N;` dans `decompose.rs`
4. Ajouter `METHOD_MA_METHODE => handle_ma_methode(&payload).await,` dans `process_request()`
5. Ajouter `async fn handle_ma_methode(payload) -> (bool, Vec<u8>, String)` dans `decompose.rs`
6. Si nécessaire : migration SQL dans `migrations/YYYY-MM-DD_description/up.sql`
7. Côté client : appel via `DhtRequest { method: N, payload: ... }`
