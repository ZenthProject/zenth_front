use rand08::RngCore;
use rand08::rngs::OsRng;

/// Network key configuration
pub const NETWORK_KEY_LENGTH: usize = 20000;
pub const ZKP_PORTION_LENGTH: usize = 2000;
pub const AUTH_PORTION_LENGTH: usize = 18000;

/// Character set for network key generation (alphanumeric + safe symbols)
const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+[]{}|;:,.<>?";

/// Generated network key with its portions
#[derive(Debug, Clone)]
pub struct NetworkKey {
    /// Full 20000-char key
    pub full_key: String,
    /// First 2000 chars for ZK-STARK proof
    pub zkp_portion: String,
    /// Remaining 18000 chars for network auth + subkey derivation
    pub auth_portion: String,
}

impl NetworkKey {
    /// Derives a subkey from the auth portion using index
    /// Useful for deriving multiple keys for different purposes
    pub fn derive_subkey(&self, index: u32, length: usize) -> Vec<u8> {
        use sha3::{Sha3_256, Digest};

        let mut hasher = Sha3_256::new();
        hasher.update(self.auth_portion.as_bytes());
        hasher.update(&index.to_le_bytes());

        let hash = hasher.finalize();

        // If we need more bytes, chain multiple hashes
        if length <= 32 {
            hash[..length].to_vec()
        } else {
            let mut result = Vec::with_capacity(length);
            let mut current_hash = hash.to_vec();

            while result.len() < length {
                result.extend_from_slice(&current_hash);
                let mut hasher = Sha3_256::new();
                hasher.update(&current_hash);
                hasher.update(&(result.len() as u32).to_le_bytes());
                current_hash = hasher.finalize().to_vec();
            }

            result.truncate(length);
            result
        }
    }
}

/// Generates a cryptographically secure 20000-character network key
/// using the operating system's CSPRNG (Cryptographically Secure Pseudo-Random Number Generator)
pub fn generate_network_key() -> Result<NetworkKey, String> {
    let mut rng = OsRng;
    let mut key_chars = Vec::with_capacity(NETWORK_KEY_LENGTH);

    // Generate random indices and map to charset
    for _ in 0..NETWORK_KEY_LENGTH {
        let idx = (rng.next_u32() as usize) % CHARSET.len();
        key_chars.push(CHARSET[idx] as char);
    }

    let full_key: String = key_chars.into_iter().collect();

    // Split into portions
    let zkp_portion = full_key[..ZKP_PORTION_LENGTH].to_string();
    let auth_portion = full_key[ZKP_PORTION_LENGTH..].to_string();

    // Validate lengths
    if zkp_portion.len() != ZKP_PORTION_LENGTH {
        return Err(format!(
            "Invalid ZKP portion length: expected {}, got {}",
            ZKP_PORTION_LENGTH,
            zkp_portion.len()
        ));
    }

    if auth_portion.len() != AUTH_PORTION_LENGTH {
        return Err(format!(
            "Invalid auth portion length: expected {}, got {}",
            AUTH_PORTION_LENGTH,
            auth_portion.len()
        ));
    }

    Ok(NetworkKey {
        full_key,
        zkp_portion,
        auth_portion,
    })
}

/// Validates that a network key has the expected format and entropy
pub fn validate_network_key(key: &str) -> Result<(), String> {
    if key.len() != NETWORK_KEY_LENGTH {
        return Err(format!(
            "Invalid key length: expected {}, got {}",
            NETWORK_KEY_LENGTH,
            key.len()
        ));
    }

    // Check that all characters are in our charset
    for (i, c) in key.chars().enumerate() {
        if !CHARSET.contains(&(c as u8)) {
            return Err(format!(
                "Invalid character at position {}: '{}'",
                i, c
            ));
        }
    }

    // Basic entropy check: ensure we have variety
    // Count unique characters - should have at least 50 unique chars for good entropy
    let unique_chars: std::collections::HashSet<char> = key.chars().collect();
    if unique_chars.len() < 50 {
        return Err(format!(
            "Insufficient entropy: only {} unique characters",
            unique_chars.len()
        ));
    }

    Ok(())
}

/// Parses an existing network key into its portions
pub fn parse_network_key(key: &str) -> Result<NetworkKey, String> {
    validate_network_key(key)?;

    let zkp_portion = key[..ZKP_PORTION_LENGTH].to_string();
    let auth_portion = key[ZKP_PORTION_LENGTH..].to_string();

    Ok(NetworkKey {
        full_key: key.to_string(),
        zkp_portion,
        auth_portion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_network_key() {
        let key = generate_network_key().expect("Failed to generate key");

        assert_eq!(key.full_key.len(), NETWORK_KEY_LENGTH);
        assert_eq!(key.zkp_portion.len(), ZKP_PORTION_LENGTH);
        assert_eq!(key.auth_portion.len(), AUTH_PORTION_LENGTH);

        // Verify portions concatenate to full key
        let reconstructed = format!("{}{}", key.zkp_portion, key.auth_portion);
        assert_eq!(reconstructed, key.full_key);
    }

    #[test]
    fn test_validate_network_key() {
        let key = generate_network_key().expect("Failed to generate key");
        assert!(validate_network_key(&key.full_key).is_ok());
    }

    #[test]
    fn test_subkey_derivation() {
        let key = generate_network_key().expect("Failed to generate key");

        // Different indices should produce different subkeys
        let subkey1 = key.derive_subkey(0, 32);
        let subkey2 = key.derive_subkey(1, 32);

        assert_ne!(subkey1, subkey2);
        assert_eq!(subkey1.len(), 32);
        assert_eq!(subkey2.len(), 32);

        // Same index should produce same subkey
        let subkey1_again = key.derive_subkey(0, 32);
        assert_eq!(subkey1, subkey1_again);
    }

    #[test]
    fn test_parse_network_key() {
        let original = generate_network_key().expect("Failed to generate key");
        let parsed = parse_network_key(&original.full_key).expect("Failed to parse key");

        assert_eq!(original.full_key, parsed.full_key);
        assert_eq!(original.zkp_portion, parsed.zkp_portion);
        assert_eq!(original.auth_portion, parsed.auth_portion);
    }
}
