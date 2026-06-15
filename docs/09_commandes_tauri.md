# Commandes Tauri - Référence complète

Toutes les commandes sont déclarées dans `src-tauri/src/lib.rs` via `tauri::generate_handler![]`.

---

## Authentification

| Commande | Paramètres | Retour | Description |
|----------|-----------|--------|-------------|
| `login` | `username, password` | `String` (session_token) | Crée la session en cache |
| `logout` | `session_token` | `()` | Vide la session du cache |
| `check_session` | `session_token` | `bool` | Vérifie si la session est active |
| `get_ws_auth` | `session_token` | `String` | Token JWT pour la connexion WebSocket |
| `configure_wipe` | `session_token, max_attempts` | `()` | Configure le wipe après N échecs |
| `register` | `username, password, entropy_hex` | `String` | Crée un compte + génère les clés |

---

## Chat

| Commande | Paramètres | Retour | Description |
|----------|-----------|--------|-------------|
| `send_message` | `session_token, friend_id, content, file_data?, file_name?, file_mime?` | `MessageInfo` | Envoie un message |
| `get_messages` | `session_token, friend_id, limit?, offset?` | `Vec<MessageInfo>` | Historique local |
| `sync_messages` | `session_token` | `MessageSyncResult` | Sync depuis DHT |
| `mark_message_read` | `session_token, message_id` | `()` | Met à jour le statut |
| `delete_message_secure` | `session_token, message_id` | `()` | Suppression sécurisée (overwrite) |
| `clear_all_sessions` | `session_token` | `u32` | Supprime toutes les sessions ratchet |
| `init_self_space` | `session_token` | `i64` (friend_id) | Crée/retrouve "Mon espace" |
| `get_chat_ttl` | `session_token, friend_id` | `u32` | TTL DHT de la conversation |
| `set_chat_ttl` | `session_token, friend_id, ttl_hours` | `()` | Définit le TTL |

### MessageInfo

```typescript
{
  id: number,
  message_id: string,
  friend_id: number,
  content: string,
  is_outgoing: boolean,
  timestamp: number,
  status: "pending" | "sent" | "delivered" | "read" | "failed",
  delivered_at: number | null,
  read_at: number | null,
  message_type: "text" | "image" | "audio" | "video" | "file",
  file_name: string | null,
  file_mime: string | null,
  file_data: string | null,  // base64
}
```

---

## Contacts

| Commande | Paramètres | Retour | Description |
|----------|-----------|--------|-------------|
| `list_friends` | `session_token` | `Vec<FriendInfo>` | Tous les amis non bloqués |
| `send_friend_request` | `session_token, target_hash, target_pseudo?, message?` | `String` | Envoie une demande |
| `accept_friend_request` | `session_token, requester_hash, pseudo?` | `String` | Accepte |
| `reject_friend_request` | `session_token, requester_hash` | `String` | Refuse |
| `cancel_friend_request` | `session_token, target_hash` | `String` | Annule une demande sortante |
| `retry_friend_request` | `session_token, target_hash` | `String` | Réessaie l'envoi |
| `list_pending_requests` | `session_token` | `Vec<PendingRequestInfo>` | Demandes en attente |
| `sync_friend_requests` | `session_token` | `SyncResult` | Sync demandes reçues (METHOD 10) |
| `sync_friend_responses` | `session_token` | `SyncResult` | Sync réponses (METHOD 14) |
| `sync_accepted_contacts` | `session_token` | `SyncResult` | Sync contacts acceptés (METHOD 27) |
| `remove_friend` | `session_token, friend_id` | `String` | Supprime un ami |
| `block_friend` | `session_token, friend_id` | `()` | Bloque |
| `unblock_friend` | `session_token, friend_id` | `()` | Débloque |
| `list_blocked_friends` | `session_token` | `Vec<FriendInfo>` | Amis bloqués |
| `rename_friend` | `session_token, friend_id, new_pseudo` | `()` | Renomme |
| `get_my_public_key` | `session_token` | `String` | Notre username_hash (hex) |
| `set_my_avatar` | `session_token, avatar_b64` | `()` | Définit notre avatar |
| `get_my_avatar` | `session_token` | `String?` | Récupère notre avatar (base64) |
| `set_friend_avatar` | `session_token, friend_id, avatar_b64` | `()` | Définit l'avatar d'un ami |
| `get_friend_fingerprint` | `session_token, friend_id` | `String` | Safety number |
| `mark_friend_verified` | `session_token, friend_id` | `()` | Marque comme vérifié |

---

## Sync multi-appareils

| Commande | Paramètres | Retour | Description |
|----------|-----------|--------|-------------|
| `sync_accounts_user` | `session_token, password` | `DeviceSyncKeys` | Clés publiques de cet appareil |
| `publish_pairing_keys` | `session_token, password` | `String` (JSON QR) | Étape 1 pairing |
| `generate_pairing_qr` | `session_token, password, scanned_qr_json` | `PairingQrResult` | Étape 2 |
| `verify_pairing_qr` | `session_token, qr_json` | `String` (pubkey tel) | Étape 3 |
| `send_sync_key` | `session_token, kyber_pubkey_pc_b64, dilithium_pubkey_pc_b64` | `String` | Étape 4 |
| `fetch_sync_key` | `session_token, dilithium_pubkey_tel_b64` | `String` | Étape 5 |
| `relay_push_all_contacts` | `session_token` | `u32` (count) | Pousse tous les contacts |
| `relay_pull_messages` | `session_token` | `u32` (count) | Pull événements relay |
| `get_relay_status` | `session_token` | JSON | Nb appareils + curseur |
| `list_paired_devices` | `session_token` | `Vec<JSON>` | Appareils jumelés |
| `revoke_paired_device` | `session_token, pubkey_hex` | `()` | Révoque un appareil |

---

## Paramètres

| Commande | Paramètres | Retour | Description |
|----------|-----------|--------|-------------|
| `save_setting` | `session_token, key, value` | `()` | Sauvegarde un paramètre |
| `load_settings` | `session_token` | JSON | Charge tous les paramètres |
| `save_all_settings` | `session_token, settings` | `()` | Sauvegarde en masse |
| `get_setting` | `session_token, key` | `String?` | Lit un paramètre |
| `delete_setting` | `session_token, key` | `()` | Supprime un paramètre |
| `reset_settings` | `session_token` | `()` | Remet à défaut |

---

## Fichiers / Sécurité

| Commande | Paramètres | Retour | Description |
|----------|-----------|--------|-------------|
| `analyze_file` | `file_path, file_data` | JSON | Analyse le format |
| `sanitize_file` | `file_path, file_data` | JSON | Nettoie le fichier |
| `sanitize_by_type` | `file_type, file_data` | JSON | Nettoie par type MIME |
| `get_supported_formats` | - | `Vec<String>` | Formats supportés |
| `wipe_user_data` | `session_token` | `()` | Efface toutes les données locales |
| `delete_account` | `session_token, password` | `()` | Supprime le compte + données |
