use zenth_crypto::{
    kdf::argon2id::Argon2idHasher,
    hashing::hash::CryptographicHash,
    asymmetric::dilithium::dilithium::{
        generate_user_key,
        sign_friendship,
    },
    kem::{
        KeyPair,
        KeyType
    }
};
use rand08::rngs::OsRng as OsRng08;
use rand08::RngCore;
use argon2::password_hash::SaltString;
use pqcrypto_traits::sign::{PublicKey as PublicKeyTrait, SecretKey as SecretKeyTrait};
use pqcrypto_dilithium::dilithium2::{PublicKey, SecretKey};
use x25519_dalek::{StaticSecret, PublicKey as X25519PublicKey};


pub fn generate_username_hash(username: &str) -> Result<Vec<u8>, String> {
    let mut hash = CryptographicHash::new("SHA256", 2)
        .map_err(|e| format!("Hash initialization error: {}", e))?;
    hash.update(username.as_bytes());
    Ok(hash.finalize())
}

pub fn derive_keys_from_password(password: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    let salt = SaltString::generate(&mut OsRng08);
    let salt_bytes = salt.as_str().as_bytes();
    let hash_password = match Argon2idHasher::new() {
        Ok(hasher) => hasher
            .derive_key(&password, salt_bytes, 32)
            .map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }?;
    Ok((hash_password, salt_bytes.to_vec()))
}

/// Derive password hash using a specific salt (for login)
pub fn derive_keys_from_password_with_salt(password: &str, salt_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let hash_password = match Argon2idHasher::new() {
        Ok(hasher) => hasher
            .derive_key(&password, salt_bytes, 32)
            .map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }?;
    Ok(hash_password)
}

pub fn generate_user_crypto_keys() -> Result<(
    PublicKey,
    SecretKey,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>
), String> {
    let (dilithium_public, dilithium_secret) = generate_user_key();
    let identity_key_dilithium = dilithium_public.as_bytes().to_vec();
    let identity_key_dilithium_secret = dilithium_secret.as_bytes().to_vec();
    let mut rng = rand::rng();
    let kyber_keypair = KeyPair::generate(KeyType::Kyber1024, &mut rng);
    let kyber_public_bytes = kyber_keypair.public_key.serialize();
    let kyber_secret_bytes = kyber_keypair.secret_key.serialize();

    Ok((
        dilithium_public,
        dilithium_secret,
        identity_key_dilithium,
        identity_key_dilithium_secret,
        kyber_public_bytes,
        kyber_secret_bytes,
    ))
}

/// Generate a X25519 keypair for ECDH key exchange
///
/// Returns a tuple of (secret_key, public_key) where:
/// - secret_key: StaticSecret (32 bytes)
/// - public_key: X25519PublicKey (32 bytes)
pub fn generate_x25519_keypair() -> (StaticSecret, X25519PublicKey) {
    let secret = StaticSecret::random_from_rng(OsRng08);
    let public = X25519PublicKey::from(&secret);
    (secret, public)
}

/// Generate a Kyber1024 keypair for post-quantum KEM
///
/// Returns a tuple of (public_key_bytes, secret_key_bytes) where:
/// - public_key_bytes: Vec<u8> (1568 bytes for Kyber1024)
/// - secret_key_bytes: Vec<u8> (3168 bytes for Kyber1024)
pub fn generate_kyber_keypair() -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut rng = rand::rng();
    let kyber_keypair = KeyPair::generate(KeyType::Kyber1024, &mut rng);
    let public_bytes = kyber_keypair.public_key.serialize();
    let secret_bytes = kyber_keypair.secret_key.serialize();
    Ok((public_bytes, secret_bytes))
}