# Frontend - Services et appels Tauri

## Principe

Le frontend React ne touche jamais à la DB ni à la crypto. Il appelle des **commandes Tauri** via `invoke()`. Les services TypeScript encapsulent ces appels.

```typescript
// Pattern général
const result = await invoke<ReturnType>("command_name", { param1, param2 });
```

---

## ChatService (`src/services/chatService.ts`)

| Méthode | Commande Tauri | Description |
|---------|---------------|-------------|
| `sendMessage()` | `send_message` | Chiffre + envoie via DHT |
| `getMessages()` | `get_messages` | Charge l'historique local |
| `syncMessages()` | `sync_messages` | Récupère les nouveaux messages du DHT |
| `markMessageRead()` | `mark_message_read` | Met à jour le statut local |
| `deleteMessageSecure()` | `delete_message_secure` | Suppression sécurisée locale |

### sendMessage - paramètres

```typescript
{
  sessionToken: string,
  friendId: number,
  content: string,
  fileData?: number[],   // bytes bruts du fichier
  fileName?: string,
  fileMime?: string,
}
```

### syncMessages - retour

```typescript
{
  new_messages: number,
  errors: string[],
  updated_friend_ids: number[]  // ids des amis avec de nouveaux messages
}
```

---

## FriendService (`src/services/friendService.ts`)

| Méthode | Commande Tauri | Description |
|---------|---------------|-------------|
| `listFriends()` | `list_friends` | Liste tous les amis non bloqués |
| `sendFriendRequest()` | `send_friend_request` | Envoie une demande |
| `acceptFriendRequest()` | `accept_friend_request` | Accepte une demande entrante |
| `rejectFriendRequest()` | `reject_friend_request` | Refuse une demande |
| `syncFriendRequests()` | `sync_friend_requests` | Sync demandes reçues (METHOD 10) |
| `syncFriendResponses()` | `sync_friend_responses` | Sync réponses à nos demandes (METHOD 14) |
| `syncAcceptedContacts()` | `sync_accepted_contacts` | Sync contacts qu'on a acceptés (METHOD 27) |
| `initSelfSpace()` | `init_self_space` | Crée/retrouve "Mon espace" |
| `blockFriend()` | `block_friend` | Bloque un contact |
| `removeFriend()` | `remove_friend` | Supprime un contact |
| `renameFriend()` | `rename_friend` | Renomme un contact |
| `setMyAvatar()` | `set_my_avatar` | Définit son propre avatar |
| `setFriendAvatar()` | `set_friend_avatar` | Définit l'avatar d'un contact |
| `getMyPublicKey()` | `get_my_public_key` | Retourne notre username_hash (hex 64 chars) |
| `getFriendFingerprint()` | `get_friend_fingerprint` | Safety number (12 groupes de 5 chiffres) |
| `markFriendVerified()` | `mark_friend_verified` | Marque un contact comme vérifié |

---

## Commandes TTL / Chat settings

| Commande Tauri | Description |
|---------------|-------------|
| `get_chat_ttl` | Lit le TTL configuré pour une conv (heures, 0 = jamais) |
| `set_chat_ttl` | Définit le TTL (0/6/24/48/168/720) |

---

## Commandes Sync multi-appareils

| Commande Tauri | Description |
|---------------|-------------|
| `publish_pairing_keys` | Publie les clés du nouvel appareil sur DHT (TTL 5 min) |
| `generate_pairing_qr` | Scanne le QR du nouvel appareil, publie la réponse |
| `verify_pairing_qr` | Vérifie le QR retour, retourne la pubkey de confiance |
| `send_sync_key` | Encapsule la Sync Key Kyber pour le nouvel appareil |
| `fetch_sync_key` | Récupère et décapsule la Sync Key |
| `relay_push_all_contacts` | Pousse tous les contacts vers les appareils jumelés |
| `relay_pull_messages` | Récupère les événements relay depuis le DHT |
| `list_paired_devices` | Liste les appareils jumelés |
| `revoke_paired_device` | Révoque un appareil |

---

## Authentification (`src/hooks/use-auth.ts`)

Contexte React global exposant :
```typescript
{
  isAuthenticated: boolean,
  isLoading: boolean,
  username: string | null,
  sessionToken: string | null,
  login(username, token, password): void,
  logout(): Promise<void>,
}
```

Le `sessionToken` est un UUID généré par Rust à la connexion. Il est passé à chaque commande Tauri pour retrouver la session en cache.

---

## WebSocket (`src/contexts/WebSocketContext.tsx`)

Connexion temps réel pour les notifications push.

```typescript
const { isConnected, lastMessage, onFriendResponse } = useWebSocket();
```

Quand un message arrive via WS → déclenche `syncMessages()` → appelle `loadMessages()` pour les convs impactées.

Types de notifications :
- `MESSAGE_RECEIVED` → sync messages
- `FRIEND_REQUEST_ACCEPTED` → sync friend responses
- `FRIEND_REQUEST_REJECTED`
