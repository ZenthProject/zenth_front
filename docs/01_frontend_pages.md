# Frontend - Pages et navigation

## Routes

Définies dans `src/lib/routes.ts` :

| Route | Composant | Accès |
|-------|-----------|-------|
| `/login` | `Login.tsx` | Public |
| `/register` | `Register.tsx` | Public |
| `/keygen` | `KeyGen.tsx` | Public |
| `/` | `Chat.tsx` | Authentifié |
| `/chat` | `Chat.tsx` | Authentifié |
| `/dashboard` | `Chat.tsx` (redirect) | Authentifié |
| `/friends` | `Friends.tsx` | Authentifié |
| `/settings` | `Settings.tsx` | Authentifié |

Les routes authentifiées sont enveloppées dans `Layout.tsx` qui inclut la sidebar.

---

## Chat.tsx

Page principale. Gère la liste des conversations + l'interface de chat.

### États importants

```typescript
const [friends, setFriends]               // liste complète des contacts
const [selfFriendId, setSelfFriendId]     // id de "Mon espace"
const [conversations, setConversations]   // Map<friend_id, Conversation>
const [selectedFriendId, setSelectedFriendId]
const [chatTtl, setChatTtl]              // TTL DHT de la conv sélectionnée (heures, 0 = jamais)
```

### Initialisation (useEffect au login)

```
initSelfSpace()          → crée l'entrée "Mon espace" si absente
syncMessages()           → récupère les messages DHT
listFriends()            → charge les contacts
getMessages(friend_id)   → charge l'historique local pour chaque contact
```

> **Important** : `initSelfSpace` doit s'exécuter AVANT `syncMessages`.
> Sinon les messages auto-envoyés sont sautés et `last_message_sync` avance sans les traiter.

### "Mon espace" (self-messaging)

- `selfFriendId` : id du contact dont `username_hash == notre propre hash`
- Affiché épinglé en premier dans la liste avec une icône Bookmark violet
- Aucun menu Bloquer/Supprimer pour ce contact
- Le header affiche "Notes chiffrées, synchronisées sur tous vos appareils"

### TTL par conversation

- Chargé via `get_chat_ttl(friend_id)` quand on change de conversation
- Options : Jamais / 6h / 24h / 48h / 7j / 30j
- Stocké dans `chat_settings` (SQLite local) et transmis au DHT via `sequence_number` de l'enveloppe

### Sync périodique

- Sans WebSocket : toutes les 10 secondes
- Avec WebSocket connecté : toutes les 30 secondes
- Déclenché aussi à chaque changement de conversation sélectionnée

---

## Friends.tsx

Gestion des contacts. Trois sections :

1. **Demandes entrantes** - Accept/Refuser
2. **Demandes sortantes** - Cancel
3. **Liste des amis** - Rename, Avatar, Vérifier identité, Bloquer, Supprimer

### Synchronisation des contacts

Le bouton "Sync" déclenche en parallèle :

```typescript
syncFriendRequests()     // METHOD 10 - demandes reçues
syncFriendResponses()    // METHOD 14 - réponses à nos demandes envoyées
syncAcceptedContacts()   // METHOD 27 - contacts qu'on a acceptés (on est responder)
relay_pull_messages()    // relay - événements multi-appareils
```

> **Pourquoi 3 syncs ?**
> - METHOD 14 : couvre le cas "ils ont accepté MA demande"
> - METHOD 27 : couvre le cas "j'ai accepté LEUR demande" (cas non couvert avant)
> - Sans METHOD 27, un nouvel appareil remettait les amis acceptés en "pending"

---

## Settings.tsx

Paramètres organisés en sections :
- Apparence (thème, langue, taille de police, bulles)
- Sécurité (verrouillage auto, wipe sur échec, persistance session)
- Notifications
- Synchronisation multi-appareils (QR pairing)
- Identité

### SyncronizationSection.tsx

Flux d'appairage en 5 étapes :

```
Nouvel appareil (Device B)          Appareil de confiance (Device A)
─────────────────────────           ─────────────────────────────────
publish_pairing_keys()              scan QR de Device B
→ publie clés sur DHT               generate_pairing_qr()
→ affiche QR (pid, hash, v)         → vérifie engagement SHA256
                                    → publie réponse + clés sur DHT
                                    send_sync_key()
                                    → encapsule Sync Key Kyber
                                    → publie sur DHT
                                    relay_push_all_contacts()  ← IMPORTANT
                                    → pousse TOUS les amis (avec pseudos)
verify_pairing_qr()
→ récupère réponse DHT
→ vérifie signature Dilithium
fetch_sync_key()
→ décapsule Sync Key
relay_pull_messages()               ← reçoit tous les amis de Device A
```

---

## Login.tsx / Register.tsx

- Login → `invoke("login")` → session Rust créée → `navigate("/chat")`
- Register → génère entropie dans `KeyGen.tsx` → `invoke("register")`
- Première connexion : affichage des CGU, acceptation stockée dans `localStorage`

---

## Localisation (i18n)

- Bibliothèque : `react-i18next`
- 10 langues : `fr`, `en`, `de`, `es`, `it`, `pt`, `ru`, `zh`, `ja`, `hi`
- Fichiers dans `src/locales/*.json`
- Clés organisées par section : `chat.*`, `friends.*`, `settings.*`, `login.*`, etc.
- Ajout d'une traduction : modifier les 10 fichiers JSON (clé identique, valeur traduite)
