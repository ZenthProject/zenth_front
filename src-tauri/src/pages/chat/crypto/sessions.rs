//! Session management for Double Ratchet protocol
//!
//! This module handles:
//! - Creating new sessions via X3DH
//! - Storing/loading session state from database
//! - Encrypting/decrypting messages
//! - Validating prekey signatures (Dilithium)

use zenth_crypto::protocols::{
    SessionState, SessionCipher, EncryptedMessage, ExportedSessionState,
    IdentityKeyPair, SignedPreKey, OneTimePreKey, PreKeyBundle,
    x3dh_initiate, x3dh_bob, BobParameters,
};
use zenth_crypto::protocols;
use rand08::rngs::OsRng;  // Use rand 0.8 for compatibility with protocols
use serde::{Deserialize, Serialize};
use pqcrypto_dilithium::dilithium2::{
    PublicKey as DilithiumPublicKey,
    DetachedSignature as DilithiumSignature,
    verify_detached_signature,
};
use pqcrypto_traits::sign::{
    PublicKey as DilithiumPublicKeyTrait,
    DetachedSignature as DetachedSignatureTrait,
};
use crate::db::user::UserDb;
use crate::db::error::DbError;
use crate::pages::chat::database::queries::{
    self, Session as DbSession, NewSession, load_session, save_session, session_exists,
};

/// Error type for session operations
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found for user: {0}")]
    SessionNotFound(String),

    #[error("Failed to initialize session: {0}")]
    InitializationFailed(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Prekey bundle not found")]
    PrekeyBundleNotFound,

    #[error("API error: {0}")]
    ApiError(String),
}

impl From<DbError> for SessionError {
    fn from(err: DbError) -> Self {
        SessionError::DatabaseError(err.to_string())
    }
}

/// Pre-key bundle data for X3DH
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreKeyBundleData {
    pub registration_id: u32,
    pub device_id: u32,
    pub identity_key: Vec<u8>,
    pub signed_prekey_id: u32,
    pub signed_prekey: Vec<u8>,
    pub signed_prekey_signature: Vec<u8>,
    pub one_time_prekey_id: Option<u32>,
    pub one_time_prekey: Option<Vec<u8>>,
}

/// Result of X3DH initiation (Alice side)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X3DHInitResult {
    /// Ephemeral public key to send to Bob (hex)
    pub ephemeral_public: String,
    /// Which one-time prekey was used (if any)
    pub one_time_prekey_id: Option<u32>,
}

/// Data needed to complete X3DH (Bob side)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X3DHCompleteData {
    /// Alice's identity key (hex)
    pub their_identity_key: String,
    /// Alice's ephemeral key (hex)
    pub their_ephemeral_key: String,
    /// Which one-time prekey was used (if any)
    pub one_time_prekey_id: Option<u32>,
}

/// Session wrapper that owns the cipher
pub struct Session {
    cipher: SessionCipher,
}

impl Session {
    /// Create a new session from initialized state
    pub fn from_state(session_state: SessionState) -> Self {
        Session {
            cipher: SessionCipher::new(session_state),
        }
    }

    /// Create a new session as Alice (initiator)
    pub fn new_as_alice(
        our_identity_private: &[u8; 32],
        their_bundle: &PreKeyBundleData,
    ) -> Result<(Self, X3DHInitResult), SessionError> {
        let mut rng = OsRng;

        // Create our identity key pair
        let our_identity = IdentityKeyPair::from_private_key(*our_identity_private);

        // Parse their bundle
        let their_identity_bytes: [u8; 32] = their_bundle.identity_key
            .as_slice()
            .try_into()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid identity key length".into()))?;

        let their_signed_prekey_bytes: [u8; 32] = their_bundle.signed_prekey
            .as_slice()
            .try_into()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid signed prekey length".into()))?;

        let their_signature: [u8; 64] = their_bundle.signed_prekey_signature
            .as_slice()
            .try_into()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid signature length".into()))?;

        let their_one_time_prekey: Option<[u8; 32]> = their_bundle.one_time_prekey
            .as_ref()
            .map(|k| k.as_slice().try_into())
            .transpose()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid one-time prekey length".into()))?;

        // Build the PreKeyBundle
        let bundle = PreKeyBundle::new(
            their_bundle.registration_id,
            their_bundle.device_id,
            protocols::IdentityKey::from_bytes(their_identity_bytes),
            their_bundle.signed_prekey_id,
            their_signed_prekey_bytes,
            their_signature,
            their_bundle.one_time_prekey_id,
            their_one_time_prekey,
        );

        // Perform X3DH
        let (x3dh_result, _base_key) = x3dh_initiate(&our_identity, &bundle, &mut rng)
            .map_err(|e| SessionError::InitializationFailed(format!("{:?}", e)))?;

        // Initialize Alice's session state
        let session_state = SessionState::initialize_alice(
            x3dh_result.root_key.key(),
            &their_signed_prekey_bytes,
            &mut rng,
        ).map_err(|e| SessionError::InitializationFailed(format!("{:?}", e)))?;

        let init_result = X3DHInitResult {
            ephemeral_public: hex::encode(x3dh_result.ephemeral_public),
            one_time_prekey_id: x3dh_result.one_time_pre_key_id,
        };

        Ok((Session { cipher: SessionCipher::new(session_state) }, init_result))
    }

    /// Create a new session as Bob (responder)
    pub fn new_as_bob(
        our_identity_private: &[u8; 32],
        our_signed_prekey_id: u32,
        our_signed_prekey_private: &[u8; 32],
        our_one_time_prekey: Option<(u32, [u8; 32])>,
        x3dh_data: &X3DHCompleteData,
    ) -> Result<Self, SessionError> {
        let mut rng = OsRng;

        // Parse Alice's keys
        let their_identity_bytes: [u8; 32] = hex::decode(&x3dh_data.their_identity_key)
            .map_err(|e| SessionError::InvalidKeyFormat(e.to_string()))?
            .try_into()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid identity key length".into()))?;

        let their_ephemeral_bytes: [u8; 32] = hex::decode(&x3dh_data.their_ephemeral_key)
            .map_err(|e| SessionError::InvalidKeyFormat(e.to_string()))?
            .try_into()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid ephemeral key length".into()))?;

        // Create our key pairs
        let our_identity = IdentityKeyPair::from_private_key(*our_identity_private);

        // Create signed prekey
        let signed_prekey_keypair = protocols::KeyPair::from_private_key(*our_signed_prekey_private);
        let signature = our_identity.sign(&mut rng, &signed_prekey_keypair.public_key);
        let signed_prekey = SignedPreKey {
            id: our_signed_prekey_id,
            key_pair: signed_prekey_keypair,
            signature,
        };

        // Create one-time prekey if used
        let one_time_prekey = our_one_time_prekey.map(|(id, private)| {
            OneTimePreKey {
                id,
                key_pair: protocols::KeyPair::from_private_key(private),
            }
        });

        // Perform X3DH as Bob
        let bob_params = BobParameters {
            our_identity_key: &our_identity,
            our_signed_pre_key: &signed_prekey,
            our_one_time_pre_key: one_time_prekey.as_ref(),
            their_identity_key: &their_identity_bytes,
            their_base_key: &their_ephemeral_bytes,
        };

        let x3dh_result = x3dh_bob(&bob_params)
            .map_err(|e| SessionError::InitializationFailed(format!("{:?}", e)))?;

        // Initialize Bob's session state with his signed prekey
        // Bob uses his signed prekey as his initial ratchet key
        let session_state = SessionState::initialize_bob(
            x3dh_result.root_key.key(),
            signed_prekey.key_pair.private_key.clone(),
        ).map_err(|e| SessionError::InitializationFailed(format!("{:?}", e)))?;

        Ok(Session { cipher: SessionCipher::new(session_state) })
    }

    /// Encrypt a message
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<EncryptedMessage, SessionError> {
        self.cipher.encrypt(plaintext)
            .map_err(|e| SessionError::EncryptionFailed(format!("{:?}", e)))
    }

    /// Decrypt a message
    pub fn decrypt(&mut self, encrypted: &EncryptedMessage) -> Result<Vec<u8>, SessionError> {
        let mut rng = OsRng;
        self.cipher.decrypt(encrypted, &mut rng)
            .map_err(|e| SessionError::DecryptionFailed(format!("{:?}", e)))
    }
}

/// Encrypted message data for storage/transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMessageData {
    /// Protocol version
    pub version: u8,
    /// Ratchet public key (32 bytes, hex)
    pub public_key: String,
    /// Previous chain length
    pub previous_chain_length: u32,
    /// Message number
    pub message_number: u32,
    /// Ciphertext
    pub ciphertext: Vec<u8>,
    /// Authentication tag (16 bytes, hex)
    pub tag: String,
}

impl From<&EncryptedMessage> for EncryptedMessageData {
    fn from(msg: &EncryptedMessage) -> Self {
        Self {
            version: msg.version,
            public_key: hex::encode(&msg.header.public_key),
            previous_chain_length: msg.header.previous_chain_length,
            message_number: msg.header.message_number,
            ciphertext: msg.ciphertext.clone(),
            tag: hex::encode(&msg.tag),
        }
    }
}

impl TryFrom<&EncryptedMessageData> for EncryptedMessage {
    type Error = SessionError;

    fn try_from(data: &EncryptedMessageData) -> Result<Self, Self::Error> {
        let public_key: [u8; 32] = hex::decode(&data.public_key)
            .map_err(|e| SessionError::InvalidKeyFormat(e.to_string()))?
            .try_into()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid public key length".into()))?;

        let tag: [u8; 16] = hex::decode(&data.tag)
            .map_err(|e| SessionError::InvalidKeyFormat(e.to_string()))?
            .try_into()
            .map_err(|_| SessionError::InvalidKeyFormat("Invalid tag length".into()))?;

        Ok(EncryptedMessage {
            version: data.version,
            header: protocols::MessageHeader {
                public_key,
                previous_chain_length: data.previous_chain_length,
                message_number: data.message_number,
            },
            ciphertext: data.ciphertext.clone(),
            tag,
        })
    }
}

// ============================================================================
// Prekey Signature Validation (Dilithium)
// ============================================================================

/// Validate a signed prekey signature using Dilithium
///
/// # Arguments
/// * `identity_key_public` - The signer's Dilithium public key (1312 bytes for Dilithium2)
/// * `signed_prekey_public` - The X25519 prekey that was signed (32 bytes)
/// * `signature` - The Dilithium signature (2420 bytes for Dilithium2)
///
/// # Returns
/// Ok(true) if valid, Ok(false) if invalid, Err on format error
pub fn validate_prekey_signature(
    identity_key_public: &[u8],
    signed_prekey_public: &[u8],
    signature: &[u8],
) -> Result<bool, SessionError> {
    // Parse Dilithium public key from bytes
    let public_key = DilithiumPublicKey::from_bytes(identity_key_public)
        .map_err(|_| SessionError::InvalidKeyFormat(
            "Invalid Dilithium public key format".to_string()
        ))?;

    // Parse signature from bytes
    let sig = DilithiumSignature::from_bytes(signature)
        .map_err(|_| SessionError::InvalidSignature(
            "Invalid Dilithium signature format".to_string()
        ))?;

    // Verify the signature
    let is_valid = verify_detached_signature(&sig, signed_prekey_public, &public_key).is_ok();

    Ok(is_valid)
}

/// Validate a complete PreKeyBundle
///
/// Checks:
/// 1. Signed prekey signature is valid (Dilithium)
/// 2. Key lengths are correct
pub fn validate_prekey_bundle(bundle: &PreKeyBundleData) -> Result<(), SessionError> {
    // Check key lengths
    if bundle.identity_key.len() != 1312 {
        // Dilithium2 public key size
        return Err(SessionError::InvalidKeyFormat(format!(
            "Invalid Dilithium public key length: expected 1312, got {}",
            bundle.identity_key.len()
        )));
    }

    if bundle.signed_prekey.len() != 32 {
        return Err(SessionError::InvalidKeyFormat(format!(
            "Invalid X25519 prekey length: expected 32, got {}",
            bundle.signed_prekey.len()
        )));
    }

    if bundle.signed_prekey_signature.len() != 2420 {
        // Dilithium2 signature size
        return Err(SessionError::InvalidKeyFormat(format!(
            "Invalid Dilithium signature length: expected 2420, got {}",
            bundle.signed_prekey_signature.len()
        )));
    }

    // Validate the signature
    let is_valid = validate_prekey_signature(
        &bundle.identity_key,
        &bundle.signed_prekey,
        &bundle.signed_prekey_signature,
    )?;

    if !is_valid {
        return Err(SessionError::InvalidSignature(
            "Prekey signature verification failed".to_string(),
        ));
    }

    Ok(())
}

// ============================================================================
// Session Persistence
// ============================================================================

/// Result of getting or creating a session
pub enum SessionResult {
    /// Existing session was loaded
    Loaded(Session),
    /// New session was created (includes X3DH init result for Alice)
    Created(Session, X3DHInitResult),
}

/// Get an existing session or create a new one via X3DH
///
/// # Arguments
/// * `db` - User database connection
/// * `friend_id` - Friend's database ID
/// * `data_key` - Key for decrypting session data
/// * `our_identity_private` - Our X25519 identity private key (32 bytes)
/// * `their_bundle` - Their prekey bundle (if creating new session)
///
/// # Returns
/// SessionResult indicating whether session was loaded or created
pub fn get_or_create_session(
    db: &UserDb,
    friend_id: i64,
    data_key: &[u8],
    our_identity_private: &[u8; 32],
    their_bundle: Option<&PreKeyBundleData>,
) -> Result<SessionResult, SessionError> {
    // Try to load existing session
    if let Some(db_session) = load_session(db, friend_id, data_key)? {
        // Reconstruct Session from DB data
        let session = Session::from_db_session(&db_session, our_identity_private)?;
        return Ok(SessionResult::Loaded(session));
    }

    // No existing session - create new one via X3DH
    let bundle = their_bundle.ok_or(SessionError::PrekeyBundleNotFound)?;

    // Validate the bundle first
    validate_prekey_bundle(bundle)?;

    // Create new session as Alice (initiator)
    let (session, init_result) = Session::new_as_alice(our_identity_private, bundle)?;

    // Save to database
    let new_session = session.to_new_session(friend_id)?;
    save_session(db, &new_session, data_key)?;

    Ok(SessionResult::Created(session, init_result))
}

/// Check if a session exists for a friend
pub fn has_session(db: &UserDb, friend_id: i64) -> Result<bool, SessionError> {
    Ok(session_exists(db, friend_id)?)
}

impl Session {
    /// Convert database session to Session (public version)
    ///
    /// Uses the serialized session state bytes stored in the database.
    /// The `root_key` field in DbSession actually contains the full serialized state.
    pub fn from_db_session_raw(
        db_session: &DbSession,
        _our_identity_private: &[u8; 32],
    ) -> Result<Self, SessionError> {
        Self::from_db_session(db_session, _our_identity_private)
    }

    /// Convert database session to Session
    ///
    /// Deserializes the SessionState from the stored bytes.
    /// We store the full serialized state in `root_key` for backward compatibility
    /// with the existing database schema.
    fn from_db_session(
        db_session: &DbSession,
        _our_identity_private: &[u8; 32],
    ) -> Result<Self, SessionError> {

        if db_session.root_key.len() == 32 {
            return Err(SessionError::InitializationFailed(
                "Old session format detected, please recreate session".to_string()
            ));
        }

        let session_state = SessionState::from_bytes(&db_session.root_key)
            .map_err(|e| {
                SessionError::InitializationFailed(format!("Failed to deserialize session: {:?}", e))
            })?;

        Ok(Session {
            cipher: SessionCipher::new(session_state),
        })
    }

    /// Convert Session to NewSession for database storage
    ///
    /// Serializes the full SessionState into the `root_key` field.
    /// Other key fields are set to empty/default as they're not used anymore.
    fn to_new_session(&self, friend_id: i64) -> Result<NewSession, SessionError> {
        let state = self.cipher.state();

        // Serialize the entire session state
        let session_bytes = state.to_bytes()
            .map_err(|e| SessionError::SerializationError(format!("Failed to serialize session: {:?}", e)))?;



        let exported = state.export_state();

        Ok(NewSession {
            friend_id,
            // Store full serialized state in root_key field
            root_key: session_bytes,
            // These fields are kept for schema compatibility but not used for restoration
            sending_chain_key: exported.sending_chain
                .map(|(key, _)| key.to_vec())
                .unwrap_or_else(|| vec![0u8; 32]),
            receiving_chain_key: exported.receiving_chain.map(|(key, _)| key.to_vec()),
            sending_counter: exported.send_count,
            receiving_counter: exported.recv_count,
            dh_public: exported.dh_self_public.to_vec(),
            dh_private: exported.dh_self_private.to_vec(),
            remote_dh_public: exported.dh_remote.map(|k| k.to_vec()),
        })
    }
}

/// Save session state after encrypt/decrypt operations
pub fn save_session_state(
    db: &UserDb,
    friend_id: i64,
    session: &Session,
    data_key: &[u8],
) -> Result<(), SessionError> {
    let new_session = session.to_new_session(friend_id)?;
    save_session(db, &new_session, data_key)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_roundtrip() {
        let mut rng = OsRng;

        // Generate keys for Alice and Bob
        let alice_identity = IdentityKeyPair::generate(&mut rng);
        let bob_identity = IdentityKeyPair::generate(&mut rng);
        let bob_signed_prekey = SignedPreKey::generate(1, &bob_identity, &mut rng);
        let bob_one_time_prekey = OneTimePreKey::generate(1, &mut rng);

        // Create Bob's bundle
        let bob_bundle = PreKeyBundleData {
            registration_id: 12345,
            device_id: 1,
            identity_key: bob_identity.public_key_bytes().to_vec(),
            signed_prekey_id: bob_signed_prekey.id,
            signed_prekey: bob_signed_prekey.public_key().to_vec(),
            signed_prekey_signature: bob_signed_prekey.signature.to_vec(),
            one_time_prekey_id: Some(bob_one_time_prekey.id),
            one_time_prekey: Some(bob_one_time_prekey.public_key().to_vec()),
        };

        // Alice initiates session
        let (mut alice_session, init_result) = Session::new_as_alice(
            &alice_identity.private_key_bytes(),
            &bob_bundle,
        ).unwrap();

        // Bob completes session
        let mut bob_session = Session::new_as_bob(
            &bob_identity.private_key_bytes(),
            bob_signed_prekey.id,
            &bob_signed_prekey.key_pair.private_key.private_key_bytes(),
            Some((bob_one_time_prekey.id, bob_one_time_prekey.key_pair.private_key.private_key_bytes())),
            &X3DHCompleteData {
                their_identity_key: hex::encode(alice_identity.public_key_bytes()),
                their_ephemeral_key: init_result.ephemeral_public.clone(),
                one_time_prekey_id: init_result.one_time_prekey_id,
            },
        ).unwrap();

        // Alice sends message to Bob
        let plaintext = b"Hello, Bob!";
        let encrypted = alice_session.encrypt(plaintext).unwrap();
        let decrypted = bob_session.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());

        // Bob replies to Alice
        let reply = b"Hello, Alice!";
        let encrypted_reply = bob_session.encrypt(reply).unwrap();
        let decrypted_reply = alice_session.decrypt(&encrypted_reply).unwrap();
        assert_eq!(reply.as_slice(), decrypted_reply.as_slice());

        // Multiple messages
        for i in 0..5 {
            let msg = format!("Message {}", i);
            let enc = alice_session.encrypt(msg.as_bytes()).unwrap();
            let dec = bob_session.decrypt(&enc).unwrap();
            assert_eq!(msg.as_bytes(), dec.as_slice());
        }
    }
}
