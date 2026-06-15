# Backend Rust - Contacts et synchronisation

## Fichier principal : `src/pages/friends/friends.rs`

---

## Ajout d'un contact (flux complet)

### Étape 1 - Envoi d'une demande

```
Utilisateur A entre le hash de B
→ send_friend_request(target_hash, pseudo, message)
   1. Vérifie que B n'est pas déjà ami / demande existante
   2. Crée un PreKeyBundle avec nos clés publiques
   3. Signe le bundle avec Dilithium2
   4. Sauvegarde en pending_friend_requests (direction='outgoing')
   5. Envoie METHOD 6 (CONTACT) au DHT
   6. relay_push_freq() → notifie les appareils jumelés
```

### Étape 2 - Réception de la demande (côté B)

```
sync_friend_requests() → METHOD 10
   → DHT retourne les demandes WHERE target_hash_id = notre_hash
   → Décode le PreKeyBundle (identity_key, kyber_key, x25519_key)
   → Sauvegarde en pending_friend_requests (direction='incoming')
   → relay_push_freq() → notifie les appareils jumelés de B
```

### Étape 3 - Acceptation

```
accept_friend_request(requester_hash, pseudo)
   1. Crée une signature d'amitié : sign("FRIENDSHIP:" || notre_hash || requester_hash)
   2. Met à jour pending_friend_requests → status='accepted'
   3. Crée l'entrée dans friends
   4. Envoie METHOD 11 (RESPOND_FRIEND_REQUEST) avec notre PreKeyBundle
   5. relay_push_friend() → notifie les appareils jumelés avec clés + pseudo
```

### Étape 4 - Finalisation côté A

```
sync_friend_responses() → METHOD 14
   → DHT retourne les réponses WHERE requester_hash_id = notre_hash
   → Décode le PreKeyBundle de B (ses clés publiques)
   → add_friend_from_accepted_request() → crée l'ami dans la DB
   → invalidate_friends_cache()
   → relay_push_friend() → notifie les appareils jumelés de A
```

---

## Cas non couvert avant : sync_accepted_contacts (METHOD 27)

### Problème

Quand Device B (nouveau) appelle `sync_friend_responses`, il ne récupère que les cas **"ils ont accepté notre demande"** (`WHERE requester_hash_id = notre_hash`).

Le cas **"on a accepté leur demande"** (on est `responder_hash_id`) n'était pas couvert.  
→ Device B voyait la demande encore pendante et la remettait en "pending" au lieu d'ajouter l'ami.

### Solution : METHOD 27

```sql
-- Requête DHT (fetch_my_accepted.rs) :
SELECT fr.requester_hash_id, fr.pre_key_bundle, resp.dilithium_signature
FROM friend_requests fr
JOIN friend_responses resp ON resp.request_id = fr.id
WHERE resp.responder_hash_id = notre_hash
  AND resp.accepted = true
```

Retourne les contacts qu'on a acceptés avec leur `pre_key_bundle` original → Device B peut reconstruire l'ami.

---

## Sync des pseudos multi-appareils

### Problème

Les pseudos ne sont jamais stockés sur le DHT (vie privée). Seul le relay les transporte.

### Solution : relay_push_all_contacts

Appelé lors de l'appairage (après `send_sync_key`) :

```rust
pub async fn relay_push_all_contacts(session_token) -> Result<u32> {
    let friends = session.list_friends();  // tous les contacts avec pseudos
    for friend in friends {
        relay_push_friend(username_hash, pseudo, identity_key, ...).await;
    }
}
```

Device B reçoit tous les contacts avec leurs pseudos via `relay_pull_messages`.

### Règle de priorité des pseudos

Dans `relay_pull_messages`, quand un contact arrive via relay :

```
pseudo local == préfixe de hash (jamais personnalisé) → prendre le pseudo du relay
pseudo local  ≠ préfixe de hash (personnalisé)        → garder le pseudo local
```

---

## Fingerprint d'amitié (Safety Number)

```rust
fn get_friend_fingerprint(our_hash, friend_hash) -> String {
    let (first, second) = si our_hash < friend_hash { (our, friend) } sinon { (friend, our) };
    let hash = SHA256("ZENTH_FINGERPRINT_V1" || first || second);
    // Affichage : 12 groupes de 5 chiffres
    // Ex: "12345 67890 23456 78901 34567 89012 45678 90123 56789 01234 67890 12345"
}
```

Les deux parties comparent ce code hors-bande (appel vocal, en personne) pour vérifier qu'il n'y a pas d'attaque MITM.

---

## Structure FriendInfo (retourné au frontend)

```typescript
{
  id: number,
  pseudo: string,
  username_hash: string,    // hex 64 chars
  identity_key_public: string,  // hex
  kyber_public_key: string | null,
  x25519_public_key: string | null,
  verified: boolean,
  blocked: boolean,
  created_at: number,       // timestamp Unix
  avatar: string | null,    // base64 JPEG
}
```
