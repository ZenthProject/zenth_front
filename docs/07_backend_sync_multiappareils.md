# Backend Rust - Synchronisation multi-appareils

## Fichier principal : `src/pages/sync/mod.rs`

---

## Vue d'ensemble

Deux mécanismes complémentaires :

| Mécanisme | Canal | Usage | Sécurité |
|-----------|-------|-------|----------|
| **Relay** | DHT relay_messages | Messages et contacts entre appareils jumelés | ChaCha20-Poly1305 + Kyber |
| **DHT standard** | Messages/contacts DHT | Récupération sur un nouvel appareil non jumelé | X3DH + Dilithium |

---

## Protocole d'appairage (5 étapes)

### Étape 1 - Nouvel appareil publie ses clés

```rust
publish_pairing_keys(session_token, password)
```
- Génère un `pairing_id` aléatoire (16 bytes)
- Calcule `h = SHA256(dilithium_pub || kyber_pub)` (engagement anti-substitution)
- Publie `dilithium_pub || kyber_pub` sur le DHT (clé = pairing_id, TTL 5 min)
- Retourne le JSON du QR : `{"pid": "hex", "h": "base64", "v": "1"}`

### Étape 2 - Appareil de confiance scanne le QR

```rust
generate_pairing_qr(session_token, password, scanned_qr_json)
```
- Récupère les clés depuis le DHT (clé = pairing_id)
- Vérifie `SHA256(dil_pub || kyber_pub) == h` (anti-substitution)
- Signe `dil_pub_pc || kyber_pub_pc || timestamp` avec notre Dilithium
- Publie la réponse sur le DHT (clé = pairing_id + [0xFF], TTL 5 min)
- Retourne le QR retour + les clés du nouvel appareil

### Étape 3 - Nouvel appareil scanne le QR retour

```rust
verify_pairing_qr(session_token, qr_json)
```
- Récupère la réponse depuis le DHT
- Vérifie la signature Dilithium de l'appareil de confiance
- Vérifie que le QR n'est pas expiré (TTL 2 min)
- Retourne `dilithium_pubkey_tel` (base64) pour l'étape suivante

### Étape 4 - Appareil de confiance envoie la Sync Key

```rust
send_sync_key(session_token, kyber_pubkey_pc, dilithium_pubkey_pc)
```
- Encapsule une Sync Key aléatoire via Kyber : `(shared_secret, ciphertext) = kyber_pub.encapsulate()`
- Signe le ciphertext avec Dilithium
- Publie sur le DHT (clé = dilithium_pubkey_pc)
- Sauvegarde `(dilithium_pubkey_pc, shared_secret)` dans `paired_devices`
- **Appelle `relay_push_all_contacts()`** → envoie tous les amis avec pseudos

### Étape 5 - Nouvel appareil récupère la Sync Key

```rust
fetch_sync_key(session_token, dilithium_pubkey_tel_base64)
```
- Récupère le blob depuis le DHT (clé = notre dilithium_pub)
- Vérifie la signature Dilithium de l'appareil de confiance
- Décapsule via Kyber : `shared_secret = kyber_priv.decapsulate(ciphertext)`
- Supprime le blob du DHT
- Sauvegarde `(dilithium_pubkey_tel, shared_secret)` dans `paired_devices`
- **Appelle `relay_pull_messages()`** → reçoit les amis et messages poussés

---

## Relay - Types d'événements

```rust
enum RelayEvent {
    Msg {
        mid: String,   // message_id hex
        fh: String,    // friend_username_hash
        fp: String,    // friend_pseudo
        out: bool,     // is_outgoing
        mt: String,    // message_type (text/image/audio/video/file)
        ct: String,    // base64(inner_message_bytes) - PLAINTEXT
        ts: i64,       // timestamp
    },
    Friend {
        fh: String,           // username_hash
        fp: String,           // pseudo
        ik: String,           // identity_key base64
        kk: Option<String>,   // kyber_public_key base64
        xk: Option<String>,   // x25519_public_key base64
        sl: Option<String>,   // friendship_signature_local
        sr: Option<String>,   // friendship_signature_remote
    },
    FriendRequest {
        fh: String,
        fp: Option<String>,
        ik: Option<String>,
        dir: String,          // "incoming" | "outgoing"
        sig: String,
        msg: Option<String>,
    },
}
```

**Note sécurité** : `Msg.ct` contient le plaintext du message (base64). Il est chiffré avec la Sync Key Kyber (ChaCha20-Poly1305) lors du transit relay. Le serveur DHT ne le voit qu'en chiffré.

---

## Chiffrement du relay

```rust
// Envoi
let key32 = sync_key[..32];
let (ciphertext, nonce) = chacha_encrypt(key32, &payload_json)?;
client.relay_push(peer_pubkey, ciphertext, nonce).await?;

// Réception
for (_, sync_key) in paired_devices {
    if let Ok(pt) = chacha_decrypt(key32, &nonce, &ciphertext) {
        // décoder RelayEvent JSON
        break;
    }
}
```

Chaque appareil essaie toutes ses sync keys jusqu'à déchiffrement réussi (supporte plusieurs appareils jumelés avec des clés différentes).

---

## relay_pull_messages - Traitement des événements

### RelayEvent::Msg

```
1. Retrouver le contact par friend_hash
   → si absent : créer un placeholder (pseudo=fp, identity_key vide)
2. Chiffrer inner_bytes localement
3. INSERT OR IGNORE INTO messages (dédup automatique)
4. Dispatcher vers le frontend via invalidate_friends_cache
```

### RelayEvent::Friend

```
1. get_friend_by_hash(fh)
   ├── Ami réel (identity_key non vide) :
   │   → Règle pseudos : garder local si personnalisé, sinon prendre relay
   │   → return false (pas de mise à jour des clés)
   └── Placeholder (identity_key vide) :
       → UPDATE avec les vraies clés + pseudo (selon règle)
       → DELETE pending_friend_requests pour ce hash
   └── Absent :
       → INSERT dans friends avec toutes les données
```

### Curseur relay

```sql
relay_cursors: last_relay_id
```
Après `relay_pull_messages()`, ACK envoyé au DHT avec `max_id`. Le prochain fetch ne repart que depuis ce curseur.

---

## Gestion des messages relay expirés

Le DHT relay a un TTL max de **24h** (`relay_messages.expires_at`).  
Si un appareil ne pull pas dans les 24h, les événements sont perdus.  
→ Mitigé par le fait que les messages DHT (voie principale) restent disponibles selon le TTL configuré par l'expéditeur.
