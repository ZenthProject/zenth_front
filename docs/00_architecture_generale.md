# Architecture générale de Zenth

## Vue d'ensemble

Zenth est une application de messagerie **décentralisée, chiffrée de bout en bout**, construite avec :

| Couche | Technologie |
|--------|-------------|
| Interface | React 18 + TypeScript + TailwindCSS |
| Desktop/Mobile | Tauri 2 (Rust) |
| Chiffrement | X3DH + Double Ratchet + Dilithium2 + Kyber1024 |
| Transport | Protobuf sur HTTPS (ou Tor) |
| Réseau P2P | DHT custom (serveur Rust/Axum + PostgreSQL) |
| Base de données | SQLite chiffrée (SQLCipher) |

---

## Schéma général

```
┌─────────────────────────────────────────────────────┐
│  APPAREIL (Tauri app)                                │
│                                                      │
│  ┌──────────┐    invoke()    ┌────────────────────┐ │
│  │  React   │ ─────────────▶ │   Rust (Tauri)     │ │
│  │  (UI)    │ ◀─────────────  │   Commandes        │ │
│  └──────────┘                │   Session cache     │ │
│                              │   Crypto (X3DH etc) │ │
│                              │   SQLCipher (DB)    │ │
│                              └────────┬───────────┘ │
└───────────────────────────────────────┼─────────────┘
                                        │ HTTPS + Protobuf
                           ┌────────────▼────────────┐
                           │   DHT Server             │
                           │   (Rust/Axum + PgSQL)    │
                           │                          │
                           │   METHOD 1-28 :          │
                           │   register, login,       │
                           │   send_message,          │
                           │   fetch_messages,        │
                           │   sync_blob, relay,      │
                           │   ack_message, ...       │
                           └─────────────────────────┘
```

---

## Flux de démarrage

```
1. Lancement app
2. Login.tsx → invoke("login") → session créée en mémoire Rust
3. navigate("/chat") → Chat.tsx
4. initSelfSpace() → crée entrée "Mon espace" si absente
5. listFriends() → charge les contacts
6. syncMessages() → récupère les messages en attente sur le DHT
7. Sync périodique toutes les 10-30s
```

---

## Sécurité

- **Chiffrement des messages** : X3DH (échange de clés initial) + Double Ratchet (forward secrecy)
- **Authentification réseau** : Signature Dilithium2 (post-quantique) sur chaque requête DHT
- **Échange de clés** : Kyber1024 (post-quantique) pour le pairing multi-appareils
- **Base de données** : SQLCipher (AES-256 PBKDF2) - dérivée du mot de passe utilisateur
- **Transit** : Protobuf sérialisé sur TLS 1.3

---

## Structure des dossiers

```
zenth_front/
├── src/                    # Frontend React/TypeScript
│   ├── components/
│   │   ├── pages/          # Pages principales (Chat, Friends, Settings…)
│   │   ├── modules/        # Composants réutilisables (ChatInterface, QRScanner…)
│   │   ├── SideBar/        # Sidebar et navigation
│   │   └── ui/             # Composants UI (shadcn)
│   ├── services/           # Appels Tauri (chatService, friendService…)
│   ├── hooks/              # Hooks React (use-auth, useVoiceRecorder…)
│   ├── locales/            # Traductions i18n (fr, en, de, es, it, pt, ru, zh, ja, hi)
│   ├── lib/                # Utilitaires (routes, emotes…)
│   └── types/              # Types TypeScript
│
└── src-tauri/              # Backend Rust (Tauri)
    └── src/
        ├── pages/          # Modules par fonctionnalité
        │   ├── chat/       # Messages, sessions Double Ratchet
        │   ├── friends/    # Contacts, demandes d'ami
        │   ├── login/      # Authentification
        │   ├── register/   # Inscription, génération de clés
        │   ├── settings/   # Paramètres
        │   ├── sync/       # Multi-appareils, relay
        │   └── keygen/     # Générateur d'entropie
        ├── db/             # Base de données (UserDb, MasterDb, SQLCipher)
        ├── api/            # Clients HTTP vers le DHT
        ├── session/        # Cache de session en mémoire
        ├── utils/          # Crypto utilitaires, timestamps, sanitizer
        └── websocket/      # Connexion WebSocket temps réel

zenth_dht/                  # Serveur DHT (Rust/Axum)
    └── src/
        ├── handlers/       # Routage et handlers par méthode
        ├── models.rs       # Structs Diesel (ORM)
        ├── schema.rs       # Schéma PostgreSQL généré par Diesel
        ├── crypto.rs       # Vérification Dilithium
        └── websocket/      # Push WebSocket vers les clients

zenth_dto/                  # Définitions Protobuf partagées
    └── proto/
        ├── dht.proto       # DhtRequest / DhtResponse / Method enum
        ├── message.proto   # ZenthSignalEnvelope, MessageAck, InnerMessage…
        ├── friend.proto    # FriendRequest, FriendResponse, PreKeyBundle…
        └── sync.proto      # SyncPushBlobRequest, RelayPush/Fetch…
```
