/// Chiffrement vault pour "Mon espace" - couche AES-256-GCM optionnelle par-dessus
/// le chiffrement existant ChaCha20-Poly1305.
///
/// Quand le vault est actif, les messages stockés sont :
///   encrypted_content = ChaCha20(password_hash, AES-GCM(vault_key, inner_bytes))
///
/// Dérivation de la clé vault :
///   vault_key = Argon2id(vault_password, salt=user_hash[..16], 32 bytes)
///
/// Migration : set/change/remove vault_password re-chiffre tous les messages Mon espace.

use crate::session::get_session_by_token_async;
use crate::pages::chat::database::encryption::{encrypt_message, decrypt_message};
use zenth_crypto::symmetric::{Aes256GcmEncryption, Aes256GcmDecryption};
use zenth_crypto::kdf::argon2id::Argon2idHasher;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;

/// Cache des clés vault en mémoire, keyed par session_token.
/// Vidé quand la session expire ou que l'utilisateur se déconnecte.
static VAULT_KEYS: Lazy<RwLock<HashMap<String, [u8; 32]>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Retourne la clé vault en cache pour ce session_token (si déverrouillé).
pub fn get_cached_vault_key(session_token: &str) -> Option<[u8; 32]> {
    VAULT_KEYS.read().ok()?.get(session_token).copied()
}

/// Vide la clé vault du cache à la déconnexion.
pub fn clear_vault_key(session_token: &str) {
    if let Ok(mut cache) = VAULT_KEYS.write() {
        cache.remove(session_token);
    }
}

// API publique
/// Chiffre `plaintext` avec AES-256-GCM.
/// Format de sortie : nonce(12) || ciphertext || tag(16)
pub fn vault_encrypt(key: &[u8; 32], associated: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let nonce: [u8; 12] = rand::random();
    let mut enc = Aes256GcmEncryption::new(key, &nonce, associated)
        .map_err(|e| format!("AES-GCM init: {:?}", e))?;
    let mut buf = plaintext.to_vec();
    enc.encrypt(&mut buf);
    let tag = enc.compute_tag();

    let mut out = Vec::with_capacity(12 + buf.len() + 16);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&buf);
    out.extend_from_slice(&tag);
    Ok(out)
}

/// Déchiffre un blob produit par `vault_encrypt`.
pub fn vault_decrypt(key: &[u8; 32], associated: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 12 + 16 {
        return Err("Données vault trop courtes".to_string());
    }
    let nonce      = &data[..12];
    let ciphertext = &data[12..data.len() - 16];
    let tag        = &data[data.len() - 16..];

    let mut dec = Aes256GcmDecryption::new(key, nonce, associated)
        .map_err(|e| format!("AES-GCM init: {:?}", e))?;
    let mut buf = ciphertext.to_vec();
    dec.decrypt(&mut buf);
    dec.verify_tag(tag).map_err(|_| "Mot de passe vault incorrect ou données corrompues".to_string())?;
    Ok(buf)
}

/// Dérive une clé vault 32 bytes depuis le mot de passe utilisateur.
/// Salt = user_hash[..16] (déterministe - même résultat sur tous les appareils).
pub fn derive_vault_key(password: &str, user_hash: &[u8]) -> Result<[u8; 32], String> {
    let salt = &user_hash[..16.min(user_hash.len())];
    if salt.len() < 8 {
        return Err("user_hash trop court pour dériver le salt".to_string());
    }
    let hasher = Argon2idHasher::new()
        .map_err(|e| format!("Argon2id init: {}", e))?;
    let key_vec = hasher.derive_key(password, salt, 32)
        .map_err(|e| format!("Argon2id derive: {}", e))?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_vec);
    Ok(key)
}

// Commandes Tauri
#[derive(Debug, Serialize, Deserialize)]
pub struct VaultStatus {
    pub enabled: bool,
    pub messages_count: u32,   // nombre de messages Mon espace
    pub encrypted_count: u32,  // combien sont vault-chiffrés
}

/// Retourne l'état du vault pour Mon espace.
#[tauri::command]
pub async fn get_vault_status(session_token: String) -> Result<VaultStatus, String> {
    let session = get_session_by_token_async(session_token).await?;
    let enabled_setting = session.get_setting("vault_enabled")
        .unwrap_or(None)
        .map(|v| v == "1")
        .unwrap_or(false);

    let self_hash = session.user_hash_hex.clone();

    session.with_db(|conn| {
        let total: u32 = conn.query_row(
            "SELECT COUNT(*) FROM messages m
             JOIN friends f ON f.id = m.friend_id
             WHERE f.username_hash = ?1",
            [&self_hash],
            |row| row.get::<_, u32>(0),
        ).unwrap_or(0);

        let encrypted: u32 = conn.query_row(
            "SELECT COUNT(*) FROM messages m
             JOIN friends f ON f.id = m.friend_id
             WHERE f.username_hash = ?1 AND m.vault_encrypted = 1",
            [&self_hash],
            |row| row.get::<_, u32>(0),
        ).unwrap_or(0);

        // Si des messages sont vault-chiffrés mais que le setting n'est pas encore
        // synchronisé (Device B), on considère quand même le vault comme actif.
        let enabled = enabled_setting || encrypted > 0;

        Ok(VaultStatus { enabled, messages_count: total, encrypted_count: encrypted })
    })
}

/// Définit ou change le mot de passe vault.
/// Re-chiffre tous les messages Mon espace avec la nouvelle clé.
/// Si `old_password` est fourni, vérifie d'abord que l'ancienne clé est correcte.
#[tauri::command]
pub async fn set_vault_password(
    session_token: String,
    new_password: String,
    old_password: Option<String>,
) -> Result<u32, String> {
    let session = get_session_by_token_async(session_token).await?;
    let user_hash = session.user_hash.clone();
    let self_hash_hex = session.user_hash_hex.clone();
    let data_key = session.password_hash.clone();

    let new_key = derive_vault_key(&new_password, &user_hash)?;

    // Dérive l'ancienne clé si vault était déjà actif
    let old_key: Option<[u8; 32]> = if let Some(ref old_pwd) = old_password {
        Some(derive_vault_key(old_pwd, &user_hash)?)
    } else {
        None
    };

    let user_db = session.get_user_db()
        .map_err(|e| format!("DB: {}", e))?;

    // Récupère tous les messages Mon espace
    let rows: Vec<(i64, String, Vec<u8>, i32)> = user_db.conn()
        .prepare(
            "SELECT m.id, m.message_id, m.encrypted_content, m.vault_encrypted
             FROM messages m
             JOIN friends f ON f.id = m.friend_id
             WHERE f.username_hash = ?1
             ORDER BY m.timestamp ASC"
        )
        .and_then(|mut stmt| {
            let r: rusqlite::Result<Vec<_>> = stmt
                .query_map([&self_hash_hex], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, i32>(3)?,
                    ))
                })?
                .collect();
            r
        })
        .map_err(|e| format!("Query: {}", e))?;

    let mut migrated = 0u32;

    for (id, mid, enc_content, is_vault_enc) in rows {
        // Étape 1 : déchiffrer la couche ChaCha20 (password_hash)
        let layer1 = match decrypt_message(&enc_content, &data_key, &mid) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };

        // Étape 2 : déchiffrer la couche vault si elle existait
        let inner_bytes = if is_vault_enc == 1 {
            let key = old_key.as_ref()
                .ok_or("Ancien mot de passe requis pour la migration")?;
            match vault_decrypt(key, mid.as_bytes(), &layer1) {
                Ok(bytes) => bytes,
                Err(_) => continue, // clé incorrecte - on skip
            }
        } else {
            layer1
        };

        // Étape 3 : chiffrer avec la nouvelle clé vault
        let vault_layer = vault_encrypt(&new_key, mid.as_bytes(), &inner_bytes)?;

        // Étape 4 : re-chiffrer avec ChaCha20 (password_hash)
        let new_enc = encrypt_message(&vault_layer, &data_key, &mid)
            .map_err(|e| format!("Re-chiffrement ChaCha20: {:?}", e))?;

        user_db.conn().execute(
            "UPDATE messages SET encrypted_content = ?1, vault_encrypted = 1 WHERE id = ?2",
            rusqlite::params![new_enc, id],
        ).map_err(|e| format!("UPDATE: {}", e))?;

        migrated += 1;
    }

    // Sauvegarde le hash de vérification (pour vérifier le mot de passe sur un autre appareil)
    let verification = vault_encrypt(&new_key, b"vault_verify", b"ZENTH_VAULT_V1")?;
    let _ = session.set_setting("vault_enabled", "1");
    let _ = session.set_setting("vault_verification", &hex::encode(&verification));

    Ok(migrated)
}

/// Supprime le chiffrement vault : re-chiffre tout en vault_encrypted = 0.
#[tauri::command]
pub async fn remove_vault_password(
    session_token: String,
    current_password: String,
) -> Result<u32, String> {
    let session = get_session_by_token_async(session_token).await?;
    let user_hash = session.user_hash.clone();
    let self_hash_hex = session.user_hash_hex.clone();
    let data_key = session.password_hash.clone();

    let vault_key = derive_vault_key(&current_password, &user_hash)?;

    // Vérifie le mot de passe avec le hash de vérification
    if let Ok(Some(verif_hex)) = session.get_setting("vault_verification") {
        if let Ok(verif) = hex::decode(&verif_hex) {
            vault_decrypt(&vault_key, b"vault_verify", &verif)
                .map_err(|_| "Mot de passe vault incorrect".to_string())?;
        }
    }

    let user_db = session.get_user_db()
        .map_err(|e| format!("DB: {}", e))?;

    let rows: Vec<(i64, String, Vec<u8>)> = user_db.conn()
        .prepare(
            "SELECT m.id, m.message_id, m.encrypted_content
             FROM messages m
             JOIN friends f ON f.id = m.friend_id
             WHERE f.username_hash = ?1 AND m.vault_encrypted = 1"
        )
        .and_then(|mut stmt| {
            let r: rusqlite::Result<Vec<_>> = stmt
                .query_map([&self_hash_hex], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                })?
                .collect();
            r
        })
        .map_err(|e| format!("Query: {}", e))?;

    let mut migrated = 0u32;

    for (id, mid, enc_content) in rows {
        let layer1 = match decrypt_message(&enc_content, &data_key, &mid) {
            Ok(b) => b, Err(_) => continue,
        };
        let inner_bytes = match vault_decrypt(&vault_key, mid.as_bytes(), &layer1) {
            Ok(b) => b, Err(_) => continue,
        };
        let new_enc = encrypt_message(&inner_bytes, &data_key, &mid)
            .map_err(|e| format!("Re-chiffrement: {:?}", e))?;

        user_db.conn().execute(
            "UPDATE messages SET encrypted_content = ?1, vault_encrypted = 0 WHERE id = ?2",
            rusqlite::params![new_enc, id],
        ).map_err(|e| format!("UPDATE: {}", e))?;

        migrated += 1;
    }

    let _ = session.set_setting("vault_enabled", "0");
    let _ = session.set_setting("vault_verification", "");

    Ok(migrated)
}

/// Reverrouille le vault : supprime la clé du cache mémoire.
/// Les messages re-deviennent illisibles jusqu'au prochain unlock.
#[tauri::command]
pub async fn lock_vault(session_token: String) -> Result<(), String> {
    get_session_by_token_async(session_token.clone()).await?;
    clear_vault_key(&session_token);
    Ok(())
}

/// Déverrouille le vault pour la session courante.
/// La clé est gardée en mémoire jusqu'à la déconnexion.
#[tauri::command]
pub async fn unlock_vault(
    session_token: String,
    password: String,
) -> Result<bool, String> {
    let session = get_session_by_token_async(session_token.clone()).await?;
    let user_hash = session.user_hash.clone();

    let vault_key = match derive_vault_key(&password, &user_hash) {
        Ok(k) => k,
        Err(_) => return Ok(false),
    };

    // Vérifie avec le hash de vérification
    let verif_hex = match session.get_setting("vault_verification") {
        Ok(Some(v)) if !v.is_empty() => v,
        _ => return Ok(false),
    };
    let verif = match hex::decode(&verif_hex) {
        Ok(v) => v, Err(_) => return Ok(false),
    };

    if vault_decrypt(&vault_key, b"vault_verify", &verif).is_err() {
        return Ok(false);
    }

    // Stocke la clé en cache pour cette session
    VAULT_KEYS.write().unwrap_or_else(|e| e.into_inner()).insert(session_token.clone(), vault_key);

    // Répare les messages Mon espace dont vault_encrypted=0 par erreur (ancienne version de
    // session.save_message qui omettait la colonne — le contenu est vault-chiffré mais le flag ne l'est pas).
    // On réutilise with_db (connexion déjà ouverte) pour éviter d'ouvrir une 2e connexion SQLCipher.
    let data_key = session.password_hash.clone();
    let self_hash_hex = session.user_hash_hex.clone();
    let _ = session.with_db(|conn| {
        let rows: Vec<(i64, String, Vec<u8>)> = conn
            .prepare(
                "SELECT m.id, m.message_id, m.encrypted_content
                 FROM messages m
                 JOIN friends f ON f.id = m.friend_id
                 WHERE f.username_hash = ?1 AND m.vault_encrypted = 0"
            )
            .and_then(|mut stmt| {
                stmt.query_map([&self_hash_hex], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Vec<u8>>(2)?))
                })
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            })
            .unwrap_or_default();

        for (id, mid, enc_content) in rows {
            if let Ok(layer1) = decrypt_message(&enc_content, &data_key, &mid) {
                if vault_decrypt(&vault_key, mid.as_bytes(), &layer1).is_ok() {
                    let _ = conn.execute(
                        "UPDATE messages SET vault_encrypted = 1 WHERE id = ?1",
                        [id],
                    );
                }
            }
        }
        Ok(())
    });

    Ok(true)
}

/// Déchiffre `encrypted_content` en appliquant la couche vault si nécessaire.
/// Utilisé par get_messages et sync_messages.
pub fn decrypt_with_vault_layer(
    encrypted: &[u8],
    data_key: &[u8],
    message_id: &str,
    vault_encrypted: bool,
    session_token: &str,
) -> Result<Vec<u8>, String> {
    use zenth_crypto::errors::error::Error;

    // Couche 1 : ChaCha20-Poly1305(password_hash)
    let layer1 = decrypt_message(encrypted, data_key, message_id)
        .map_err(|e| format!("Déchiffrement ChaCha20: {:?}", e))?;

    if !vault_encrypted {
        return Ok(layer1);
    }

    // Couche 2 : AES-256-GCM(vault_key)
    let key = get_cached_vault_key(session_token)
        .ok_or("[Mon espace verrouillé - entrez votre mot de passe vault]".to_string())?;

    vault_decrypt(&key, message_id.as_bytes(), &layer1)
        .map_err(|e| format!("Déchiffrement vault: {}", e))
}

/// Vérifie si un mot de passe vault est correct (sans rien modifier).
#[tauri::command]
pub async fn verify_vault_password(
    session_token: String,
    password: String,
) -> Result<bool, String> {
    let session = get_session_by_token_async(session_token).await?;
    let user_hash = session.user_hash.clone();

    let vault_key = match derive_vault_key(&password, &user_hash) {
        Ok(k) => k,
        Err(_) => return Ok(false),
    };

    let verif_hex = match session.get_setting("vault_verification") {
        Ok(Some(v)) if !v.is_empty() => v,
        _ => return Ok(false),
    };

    let verif = match hex::decode(&verif_hex) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };

    Ok(vault_decrypt(&vault_key, b"vault_verify", &verif).is_ok())
}
