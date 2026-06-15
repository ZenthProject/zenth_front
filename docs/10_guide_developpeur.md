# Guide développeur - Comment continuer le projet

## Prérequis

### Frontend
```bash
bun install           # dépendances JS
```

### Backend Rust
```bash
rustup toolchain install stable
rustup target add x86_64-unknown-linux-gnu   # Linux
rustup target add x86_64-pc-windows-msvc     # Windows (sur Windows)
```

### DHT
```bash
cargo install diesel_cli --no-default-features --features postgres
export DATABASE_URL=postgres://user:pass@localhost/zenth_dht
diesel migration run
```

---

## Lancer en développement

```bash
# Terminal 1 - DHT server
cd zenth_dht
cargo run

# Terminal 2 - App Tauri
cd zenth_front
bun run tauri dev
```

Variables d'environnement nécessaires (`.env` dans `zenth_front/src-tauri/`) :
```
ZENTH_DHT_API_URL=http://localhost:3000
ZENTH_ACCEPT_INVALID_CERTS=1   # en dev uniquement
```

---

## Ajouter une commande Tauri

### 1. Créer la fonction Rust

```rust
// Dans src-tauri/src/pages/<module>/<fichier>.rs
#[tauri::command]
pub async fn ma_commande(
    session_token: String,
    mon_param: String,
) -> Result<MonRetour, String> {
    let session = get_session_by_token_async(session_token).await?;
    // ... logique
    Ok(MonRetour { ... })
}
```

### 2. Re-exporter depuis mod.rs

```rust
// src-tauri/src/pages/<module>/mod.rs
pub use mon_fichier::ma_commande;
```

### 3. Importer et enregistrer dans lib.rs

```rust
// src-tauri/src/lib.rs
use pages::mon_module::ma_commande;
// ...
tauri::generate_handler![
    // ... autres commandes
    ma_commande,
]
```

### 4. Appeler depuis le frontend

```typescript
const result = await invoke<MonRetour>("ma_commande", {
    sessionToken,
    monParam: "valeur",
});
```

---

## Ajouter une traduction

```python
# Modifier les 10 fichiers locales en Python :
import json
files = ["fr","en","de","es","it","pt","ru","zh","ja","hi"]
for lang in files:
    with open(f"src/locales/{lang}.json", "r+") as f:
        data = json.load(f)
        data["section"]["nouvelle_cle"] = "Traduction..."
        f.seek(0); json.dump(data, f, ensure_ascii=False, indent=2)
```

Puis dans le composant React :
```tsx
const { t } = useTranslation();
<p>{t("section.nouvelle_cle")}</p>
```

---

## Ajouter une méthode DHT

### 1. Créer le handler

```rust
// zenth_dht/src/handlers/method/ma_methode.rs
pub async fn ma_methode(req: MaRequest) -> Result<MaResponse, String> {
    // ... vérification signature Dilithium
    // ... logique métier
    // ... retour
}
```

### 2. Enregistrer

```rust
// mod.rs : pub mod ma_methode;
// decompose.rs :
const METHOD_MA_METHODE: i32 = 29; // prochain numéro disponible
// dans process_request() :
METHOD_MA_METHODE => handle_ma_methode(&dht_request.payload).await,
// + fn handle_ma_methode(payload) -> (bool, Vec<u8>, String)
```

### 3. Appeler depuis le client Rust

```rust
// Réutiliser un DTO existant si possible (évite de modifier le proto)
let req = ExistingRequest { ... };
let mut bytes = Vec::new();
req.encode(&mut bytes).unwrap();

let dht_req = DhtRequest {
    method: 29,
    payload: bytes,
    timestamp: current_timestamp(),
    request_id: rand::random::<[u8; 16]>().to_vec(),
};
// HTTP POST + decode DhtResponse
```

---

## Modifier le schéma DB client (SQLite)

Les migrations sont **idempotentes** dans `UserDb::open_with_entry()` :

```rust
// Pour une nouvelle table :
conn.execute_batch("
    CREATE TABLE IF NOT EXISTS ma_table (
        id INTEGER PRIMARY KEY,
        ...
    );
")?;

// Pour une nouvelle colonne :
match conn.execute("ALTER TABLE ma_table ADD COLUMN ma_colonne TEXT", []) {
    Ok(_) => {}
    Err(e) if e.to_string().contains("duplicate column") => {}
    Err(e) => return Err(DbError::from(e)),
}
```

Pas de numéros de version : l'idempotence suffit.

---

## Build release

### Linux
```bash
bash scripts/build.sh linux    # → releases/Zenth_x.x.x_linux_amd64.deb
bash scripts/build.sh          # → linux + apk
```

### Windows (sur machine Windows)
```powershell
.\scripts\build_windows.ps1              # NSIS + MSI
.\scripts\build_windows.ps1 -Bundle nsis # NSIS seulement
```

### Android
```bash
bash scripts/sign_apk.sh         # build + signe
bash scripts/sign_apk.sh --install  # + installe via adb
```

---

## Points d'attention importants

### Ordre d'initialisation au login

```
initSelfSpace()    ← DOIT être avant syncMessages()
syncMessages()     ← sinon "Mon espace" est raté définitivement
listFriends()
```

### Ratchet thread-safety

Les `send_message()` parallèles vers le même contact DOIVENT passer par le `send_lock(friend_id)`. Sans ça, l'état ratchet se désynchronise.

### OTPKs

Ne JAMAIS supprimer les OTPKs de la DB, même utilisées. Elles sont nécessaires pour rejouer l'historique sur un nouvel appareil.

### Clé `sequence_number` dans l'enveloppe

`ZenthSignalEnvelope.sequence_number` est **réutilisé** comme TTL en heures. Si tu réimplémentes la séquence de messages, utilise un autre champ ou étends le proto.

### Méthodes DHT 27 et 28

Non présentes dans l'enum `Method` du proto `dht.proto`. Utilisées comme constantes entières hardcodées. Si tu ajoutes des méthodes, continue à partir de 29.

---

## Structure des dépendances clés

```toml
# src-tauri/Cargo.toml (principales)
tauri = "2"
rusqlite + rusqlite-migration  # SQLite + SQLCipher
pqcrypto-dilithium             # Dilithium2
zenth_crypto                   # bibliothèque crypto interne (X3DH, ratchet)
zenth_dto                      # types protobuf partagés
zenth_requests                 # client HTTP avec support Tor
prost                          # sérialisation protobuf
x25519-dalek                   # X25519 (X3DH)
chacha20poly1305               # chiffrement symétrique
```
