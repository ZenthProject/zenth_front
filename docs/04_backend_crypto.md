# Backend Rust - Cryptographie

## Vue d'ensemble des algorithmes

| Usage | Algorithme | Type |
|-------|-----------|------|
| Échange de clés initial | X3DH | Classique (X25519) |
| Forward secrecy messages | Double Ratchet | Classique (X25519) |
| Authentification réseau | Dilithium2 | Post-quantique |
| Pairing multi-appareils | Kyber1024 | Post-quantique |
| Chiffrement relay | ChaCha20-Poly1305 | Symétrique |
| DB locale | SQLCipher (AES-256) | Symétrique |
| Chiffrement messages local | ChaCha20-Poly1305 | Symétrique |

---

## X3DH (Extended Triple Diffie-Hellman)

Protocole d'échange de clés initial entre deux parties. Utilisé pour établir la première session.

### Clés en jeu

```
Alice (expéditeur)          Bob (destinataire)
─────────────────           ─────────────────
IK_A (identity key)         IK_B (identity key)     ← stocké dans users DHT
EK_A (ephemeral key)        SPK_B (signed prekey)   ← stocké dans pre_keys DHT
                            OPK_B (one-time prekey)  ← stocké dans pre_keys DHT (optionnel)
```

### Calcul du secret partagé

```
DH1 = DH(IK_A,  SPK_B)
DH2 = DH(EK_A,  IK_B)
DH3 = DH(EK_A,  SPK_B)
DH4 = DH(EK_A,  OPK_B)   # si one-time prekey disponible

IKM = DH1 || DH2 || DH3 || DH4
SK  = SHA256("ZenthX3DH" || IKM)
```

### Spécificité Zenth

Dans Zenth, `IK_B = SPK_B` (la clé d'identité X25519 EST la signed prekey). Les deux utilisent `bundle.signed_pre_key_public`.

### "Mon espace" (self-session X3DH)

Quand sender = recipient, X3DH fonctionne parce que les DH sont symétriques :
- Alice utilise ses propres clés publiques comme clés de Bob
- Le secret partagé est dérivable des deux côtés depuis les mêmes clés privées
- Les messages suivants utilisent le Double Ratchet normalement

---

## Double Ratchet

Après l'X3DH, chaque message fait avancer le ratchet pour garantir la **forward secrecy**.

### État de session (stocké chiffré dans `sessions`)

```
root_key              → dérivé depuis l'X3DH
sending_chain_key     → avance à chaque message envoyé
receiving_chain_key   → avance à chaque message reçu
sending_counter       → numéro de message sortant
receiving_counter     → numéro de message entrant
dh_public/private     → clé DH courante du ratchet
remote_dh_public      → clé DH publique du correspondant
```

### Sérialisation dans l'enveloppe

```protobuf
RatchetHeader {
    sender_ratchet_key  → clé DH publique courante
    previous_counter    → pour out-of-order delivery
    counter             → position dans la chaîne
}
```

---

## Dilithium2 (Post-quantique)

Algorithme de signature numérique résistant aux ordinateurs quantiques (basé sur les réseaux).

### Usages dans Zenth

1. **Authentification DHT** : chaque requête est signée avec `dilithium_secret`
2. **Vérification côté serveur** : le DHT vérifie la signature avec `identity_key_dilithium` (stocké à l'inscription)
3. **Amitié** : signature `"FRIENDSHIP:" || hash_A || hash_B` lors de l'acceptation d'un contact
4. **Ack de message** : `message_id || recipient_hash || timestamp`

### Taille des clés

- Clé publique : **1312 bytes**
- Clé secrète : **2528 bytes**
- Signature : **2420 bytes**

---

## Kyber1024 (Post-quantique)

Algorithme KEM (Key Encapsulation Mechanism) résistant aux ordinateurs quantiques.

### Usage : pairing multi-appareils

```
Device A (confiance)                  Device B (nouveau)
────────────────────                  ──────────────────
                                      publie kyber_pub sur DHT

récupère kyber_pub
(shared_secret, ciphertext) = kyber_pub.encapsulate()
publie ciphertext sur DHT

                                      récupère ciphertext
                                      shared_secret = kyber_priv.decapsulate(ciphertext)
```

La `shared_secret` devient la **Sync Key** pour chiffrer les messages relay.

---

## Chiffrement local des messages (DB)

Les messages ne sont jamais stockés en clair dans SQLite.

```
enc_key    = password_hash (dérivé via Argon2id)
nonce      = SHA256(message_id)[..12]  // déterministe → pas besoin de stocker le nonce
ciphertext = ChaCha20-Poly1305(enc_key, nonce, inner_message_bytes)
```

`inner_message_bytes` est le protobuf `InnerMessage` sérialisé.

---

## InnerMessage (format interne)

Payload chiffré par le Double Ratchet. **Jamais vu par le serveur**.

```protobuf
message InnerMessage {
    InnerMessageType type = 1;  // INNER_TEXT ou INNER_FILE
    string text      = 2;
    bytes  file_data = 3;
    string file_name = 4;
    string file_mime = 5;
}
```

---

## Sanitisation des fichiers entrants

Avant stockage, chaque fichier reçu est vérifié :

1. **Magic bytes** (signature du format réel, pas le MIME déclaré)
2. **Sanitisation** via `zenth_protect` (lib interne)
3. Exception : audio/vidéo WebM/OGG/MP4 passent bruts si magic bytes cohérents avec le MIME

```rust
fn sanitize_incoming_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    // parse InnerMessage
    // si fichier → FileParser::parse_by_signature()
    // si audio/vidéo non sanitisable → vérifie magic bytes vs MIME
    // sinon → Err (rejeté)
}
```

---

## Identifiant utilisateur (username_hash)

```
username_hash = SHA256(SHA256(username))
```

- Double hash pour se protéger des rainbow tables
- Déterministe : identique sur tous les appareils du même compte
- 32 bytes = 64 chars hex
- Utilisé comme clé primaire dans le DHT (`users.user_hash_id`)
