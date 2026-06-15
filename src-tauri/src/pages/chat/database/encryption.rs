//! Database-level encryption for messages and files
//!
//! Uses AES-256-GCM via the existing cipher_key functions.
//! Format: [nonce: 12 bytes][ciphertext][tag: 16 bytes]

use crate::utils::security::cipher_key::{encrypt_key_with_password, decrypt_key_with_password};
use zenth_crypto::errors::error::{Error, Result};

/// Encrypt message content before storage
///
/// # Arguments
/// * `content` - Plaintext message content
/// * `data_key` - 32-byte derived key
/// * `message_id` - Unique message ID (used as AAD)
///
/// # Returns
/// Encrypted data: nonce || ciphertext || tag
pub fn encrypt_message(content: &[u8], data_key: &[u8], message_id: &str) -> Result<Vec<u8>> {
    encrypt_key_with_password(content, data_key, message_id.as_bytes())
}

/// Decrypt message content after retrieval
///
/// # Arguments
/// * `encrypted` - Encrypted data (nonce || ciphertext || tag)
/// * `data_key` - 32-byte derived key
/// * `message_id` - Unique message ID (used as AAD)
///
/// # Returns
/// Decrypted plaintext
pub fn decrypt_message(encrypted: &[u8], data_key: &[u8], message_id: &str) -> Result<Vec<u8>> {
    decrypt_key_with_password(encrypted, data_key, message_id.as_bytes())
}

/// Encrypt file/blob before storage
///
/// # Arguments
/// * `data` - File data
/// * `data_key` - 32-byte derived key
/// * `file_id` - Unique file ID or message_id (used as AAD)
///
/// # Returns
/// Encrypted data: nonce || ciphertext || tag
pub fn encrypt_file(data: &[u8], data_key: &[u8], file_id: &str) -> Result<Vec<u8>> {
    encrypt_key_with_password(data, data_key, file_id.as_bytes())
}

/// Decrypt file/blob after retrieval
///
/// # Arguments
/// * `encrypted` - Encrypted data (nonce || ciphertext || tag)
/// * `data_key` - 32-byte derived key
/// * `file_id` - Unique file ID or message_id (used as AAD)
///
/// # Returns
/// Decrypted file data
pub fn decrypt_file(encrypted: &[u8], data_key: &[u8], file_id: &str) -> Result<Vec<u8>> {
    decrypt_key_with_password(encrypted, data_key, file_id.as_bytes())
}

/// Encrypt thumbnail before storage
///
/// # Arguments
/// * `thumbnail` - Thumbnail image data
/// * `data_key` - 32-byte derived key
/// * `file_id` - Parent file ID (used as AAD)
///
/// # Returns
/// Encrypted data: nonce || ciphertext || tag
pub fn encrypt_thumbnail(thumbnail: &[u8], data_key: &[u8], file_id: &str) -> Result<Vec<u8>> {
    let aad = format!("thumb:{}", file_id);
    encrypt_key_with_password(thumbnail, data_key, aad.as_bytes())
}

/// Decrypt thumbnail after retrieval
///
/// # Arguments
/// * `encrypted` - Encrypted thumbnail data
/// * `data_key` - 32-byte derived key
/// * `file_id` - Parent file ID (used as AAD)
///
/// # Returns
/// Decrypted thumbnail data
pub fn decrypt_thumbnail(encrypted: &[u8], data_key: &[u8], file_id: &str) -> Result<Vec<u8>> {
    let aad = format!("thumb:{}", file_id);
    decrypt_key_with_password(encrypted, data_key, aad.as_bytes())
}

/// Encrypt session state before storage
///
/// # Arguments
/// * `session_data` - Serialized session state
/// * `data_key` - 32-byte derived key
/// * `friend_id` - Friend's hash ID (used as AAD)
///
/// # Returns
/// Encrypted data: nonce || ciphertext || tag
pub fn encrypt_session(session_data: &[u8], data_key: &[u8], friend_id: &str) -> Result<Vec<u8>> {
    let aad = format!("session:{}", friend_id);
    encrypt_key_with_password(session_data, data_key, aad.as_bytes())
}

/// Decrypt session state after retrieval
///
/// # Arguments
/// * `encrypted` - Encrypted session data
/// * `data_key` - 32-byte derived key
/// * `friend_id` - Friend's hash ID (used as AAD)
///
/// # Returns
/// Decrypted session data
pub fn decrypt_session(encrypted: &[u8], data_key: &[u8], friend_id: &str) -> Result<Vec<u8>> {
    let aad = format!("session:{}", friend_id);
    decrypt_key_with_password(encrypted, data_key, aad.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_encryption_roundtrip() {
        let data_key = [0x42u8; 32];
        let message_id = "msg_123456";
        let plaintext = b"Hello, World!";

        let encrypted = encrypt_message(plaintext, &data_key, message_id).unwrap();
        let decrypted = decrypt_message(&encrypted, &data_key, message_id).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_file_encryption_roundtrip() {
        let data_key = [0x42u8; 32];
        let file_id = "file_789";
        let file_data = [0xDE_u8, 0xAD, 0xBE, 0xEF].repeat(256); // 256 × 4 = 1024 bytes

        let encrypted = encrypt_file(&file_data, &data_key, file_id).unwrap();
        let decrypted = decrypt_file(&encrypted, &data_key, file_id).unwrap();

        assert_eq!(decrypted, file_data);
    }

    #[test]
    fn test_thumbnail_encryption_roundtrip() {
        let data_key = [0x42u8; 32];
        let file_id = "file_789";
        let thumbnail = vec![0xFF; 256];

        let encrypted = encrypt_thumbnail(&thumbnail, &data_key, file_id).unwrap();
        let decrypted = decrypt_thumbnail(&encrypted, &data_key, file_id).unwrap();

        assert_eq!(decrypted, thumbnail);
    }

    #[test]
    fn test_wrong_aad_fails() {
        let data_key = [0x42u8; 32];
        let plaintext = b"Secret message";

        let encrypted = encrypt_message(plaintext, &data_key, "msg_1").unwrap();
        let result = decrypt_message(&encrypted, &data_key, "msg_2");

        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_key_fails() {
        let data_key1 = [0x42u8; 32];
        let data_key2 = [0x43u8; 32];
        let message_id = "msg_123";
        let plaintext = b"Secret message";

        let encrypted = encrypt_message(plaintext, &data_key1, message_id).unwrap();
        let result = decrypt_message(&encrypted, &data_key2, message_id);

        assert!(result.is_err());
    }
}
