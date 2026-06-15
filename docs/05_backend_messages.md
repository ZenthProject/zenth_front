# Backend Rust - Messages

## Fichier principal : `src/pages/chat/messages.rs`

---

## Envoi d'un message (`send_message`)

### Flux complet

```
1. Récupérer la session (cache mémoire)
2. Récupérer le contact (friend_id)
3. Acquérir le send_lock(friend_id) → sérialise les envois parallèles
4. Vérifier si une session Double Ratchet existe pour ce contact
   ├── OUI → charger depuis DB
   └── NON → fetch_friend_prekey_bundle(DHT) → X3DH → nouvelle session
5. Sérialiser le contenu en InnerMessage protobuf
6. Chiffrer avec Double Ratchet → EncryptedMessageData
7. Construire ZenthSignalEnvelope
   ├── Si nouvelle session → PreKeyMessage (inclut base_key, identity_key)
   └── Si session existante → RegularMessage
8. Lire chat_settings.ttl_hours → mettre dans envelope.sequence_number
9. Signer l'enveloppe avec Dilithium2
10. Sauvegarder le message localement (chiffré avec password_hash)
11. Sauvegarder l'état du ratchet en DB
12. Libérer le send_lock
13. relay_push_message() → envoyer aux appareils jumelés (best-effort, spawné)
14. send_message_to_server() → HTTP POST vers DHT (METHOD 12)
15. Mettre à jour le statut ("sent" ou "failed")
```

### Gestion de l'échec du ratchet

Si le chiffrement échoue (session corrompue) :
- Supprime la session en DB
- Fetch un nouveau bundle PreKey depuis le DHT
- Crée une nouvelle session X3DH
- Réessaie

### Cas "Mon espace" (sender = recipient)

- `friend.username_hash == session.user_hash_hex`
- Le DHT accepte `sender_hash == recipient_hash` (pas de validation côté serveur)
- Le message est stocké dans notre propre boîte de réception DHT

---

## Réception des messages (`sync_messages`)

### Flux complet

```
1. Récupérer la session
2. try_lock(sync_lock) → si sync déjà en cours, abandon immédiat
3. Vérifier le stock d'OTPKs → renouveler si < MIN_PREKEY_COUNT
4. fetch_messages_from_server(since_timestamp, limit=100) → METHOD 13
5. Pour chaque message reçu :
   a. Décoder sender_hash depuis l'enveloppe
   b. get_friend_by_hash(sender_hash) → retrouver le contact local
      → "Mon espace" : sender_hash == notre hash → trouvé si init_self_space() fait
   c. Déchiffrer selon le type :
      ├── RegularMessage avec message_type=PreKey → decrypt_prekey_message()
      ├── RegularMessage standard → decrypt_regular_message()
      └── PrekeyMessage → decrypt_prekey_message()
   d. sanitize_incoming_bytes() → vérification magic bytes
   e. Chiffrer localement et sauvegarder en DB
   f. is_outgoing = (sender_hash == notre hash) ← pour "Mon espace"
   g. spawn → send_message_ack() → METHOD 28 (suppression DHT immédiate)
   h. spawn → relay_push_message() → relay multi-appareils
6. Mettre à jour last_message_sync avec le timestamp du serveur
```

### Déchiffrement PreKey message

```rust
fn decrypt_prekey_message(prekey_msg, user_db, our_x25519_private, session) {
    // 1. Récupérer notre signed prekey depuis la DB
    // 2. Récupérer notre one-time prekey si utilisée (marquée used mais non supprimée)
    // 3. create_bob_session_manual() → recrée le côté Bob de l'X3DH
    // 4. Déchiffrer le message avec le ratchet
    // 5. Marquer l'OTPKs comme used
}
```

### Déchiffrement Regular message

```rust
fn decrypt_regular_message(msg, user_db, friend_id, our_x25519_private) {
    // 1. Charger l'état du ratchet depuis DB
    // 2. Session::from_db_session_raw() → reconstruit la session
    // 3. session.decrypt() → déchiffre
}
```

---

## Ack de message (`send_message_ack`)

Envoyé après chaque message stocké avec succès. Supprime immédiatement le message du DHT.

```rust
async fn send_message_ack(message_id, recipient_hash, dilithium_secret, timestamp) {
    // Signe : message_id || recipient_hash || timestamp
    // Construit MessageAck protobuf
    // Envoie METHOD 28 au DHT
    // Best-effort : erreur réseau ignorée
}
```

---

## Gestion des TTL (durée de vie sur le DHT)

Le TTL est transporté dans `ZenthSignalEnvelope.sequence_number` :

| `sequence_number` | Comportement DHT |
|-------------------|-----------------|
| `0` | `expires_at = 9999-12-31` (jamais) |
| `N > 0` | `expires_at = NOW() + N heures` |

Côté client, lu depuis `chat_settings.ttl_hours` avant l'envoi.

---

## "Mon espace" (`init_self_space`)

```rust
// Insère dans friends si absent :
INSERT OR IGNORE INTO friends
  (pseudo, username_hash, identity_key_public, kyber_public_key, x25519_public_key,
   verified, blocked, created_at, updated_at)
  VALUES ('Mon espace', notre_hash, notre_identity_key, notre_kyber, notre_x25519,
          1, 0, now, now)
```

- `username_hash` = `session.user_hash_hex` (notre propre hash)
- `verified = 1` par défaut
- `x25519_public_key` = notre propre clé → permet au ratchet Bob de déchiffrer nos propres messages

---

## Commandes TTL

```rust
// Lire le TTL d'une conversation
pub async fn get_chat_ttl(session_token, friend_id) -> Result<u32>
// → SELECT ttl_hours FROM chat_settings WHERE friend_id = ?

// Définir le TTL
pub async fn set_chat_ttl(session_token, friend_id, ttl_hours) -> Result<()>
// → INSERT OR REPLACE INTO chat_settings (friend_id, ttl_hours) VALUES (?, ?)
```

---

## Lock de conversation

Le `send_lock` par `friend_id` sérialise les envois parallèles vers un même contact.

**Problème sans lock** : deux `send_message()` parallèles liraient le même état ratchet, produiraient des messages avec le même compteur, et le destinataire ne pourrait déchiffrer que le premier.

**Avec lock** : la section critique (lecture ratchet → chiffrement → sauvegarde) est atomique par conversation.
