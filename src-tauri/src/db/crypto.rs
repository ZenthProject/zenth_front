use argon2::{Argon2, Algorithm, Version, Params};
use sha2::{Sha256, Digest};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::db::error::DbError;

/// Taille du salt en bytes (32 bytes = 256 bits)
pub const SALT_SIZE: usize = 32;

/// Taille de la cle SQLCipher derivee (32 bytes = 256 bits)
pub const KEY_SIZE: usize = 32;

/// Parametres Argon2id pour la derivation de cle
/// - Memoire: 64 MB
/// - Iterations: 3
/// - Parallelisme: 4
const ARGON2_M_COST: u32 = 65536; // 64 MB
const ARGON2_T_COST: u32 = 3;     // 3 iterations
const ARGON2_P_COST: u32 = 4;     // 4 threads

/// Genere un salt aleatoire cryptographiquement sur
pub fn generate_salt() -> [u8; SALT_SIZE] {
    let mut salt = [0u8; SALT_SIZE];
    rand::rng().fill_bytes(&mut salt);
    salt
}

/// Derive une cle SQLCipher a partir du mot de passe et du salt
/// Utilise Argon2id avec des parametres securises
pub fn derive_sqlcipher_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_SIZE]>, DbError> {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_SIZE))
        .map_err(|e| DbError::Encryption(format!("Argon2 params error: {}", e)))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; KEY_SIZE]);
    argon2
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| DbError::Encryption(format!("Argon2 hash error: {}", e)))?;

    Ok(key)
}

/// Convertit une cle binaire en chaine hexadecimale pour SQLCipher
/// Format: "x'<hex>'"
pub fn key_to_sqlcipher_pragma(key: &[u8; KEY_SIZE]) -> Zeroizing<String> {
    let hex_key = hex::encode(key);
    Zeroizing::new(format!("x'{}'", hex_key))
}

/// Hash le nom d'utilisateur avec SHA256 pour creer un identifiant de BDD
/// Retourne les 16 premiers caracteres hex (8 bytes)
pub fn hash_username(username: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // 16 caracteres hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_salt() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();
        assert_ne!(salt1, salt2);
        assert_eq!(salt1.len(), SALT_SIZE);
    }

    #[test]
    fn test_derive_key() {
        let salt = generate_salt();
        let key1 = derive_sqlcipher_key("password123", &salt).unwrap();
        let key2 = derive_sqlcipher_key("password123", &salt).unwrap();
        let key3 = derive_sqlcipher_key("different", &salt).unwrap();

        assert_eq!(*key1, *key2);
        assert_ne!(*key1, *key3);
    }

    #[test]
    fn test_hash_username() {
        let hash1 = hash_username("alice");
        let hash2 = hash_username("alice");
        let hash3 = hash_username("bob");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16);
    }
}
