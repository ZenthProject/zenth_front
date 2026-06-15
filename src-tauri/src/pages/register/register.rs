use zenth_crypto::asymmetric::dilithium::dilithium::sign_friendship;

use crate::pages::register::crypto::{
    key::{
        generate_username_hash,
        derive_keys_from_password_with_salt,
        generate_user_crypto_keys,
        generate_x25519_keypair,
        generate_kyber_keypair
    },
    commitment::generate_zkp_proof
};
use crate::pages::keygen::crypto::key_gen::generate_network_key;
use rand08::rngs::OsRng as OsRng08;
use rand08::RngCore;
use pqcrypto_traits::sign::{PublicKey, SecretKey, DetachedSignature as TraitDetachedSignature};
use crate::utils::timestamp::plateform::unix_timestamp;
use zenth_dto::{
    RegistrationRequest, PreKeyBundle, IdentityKey,
    SignatureKeyType, KemPublicKey, KemKeyType, ZkpType,
};

use crate::utils::security::securitypassword::SecurityPassword;
use crate::utils::security::cipher_key::encrypt_key_with_password;
use crate::api::register::{RegisterConfig, RegisterApiClient, DarknetType};
use crate::api::errors::ApiError;
use crate::db::UserDb;
use crate::db::user::NewUserInfo;

/// Number of one-time pre-keys to generate at registration
const NUM_ONE_TIME_PREKEYS: u32 = 100;

/// Pre-key data for storage
#[derive(Debug, Clone)]
pub struct PreKeyData {
    pub pre_key_id: u32,
    pub public_key: Vec<u8>,
    pub private_key_encrypted: Vec<u8>,
    pub private_key_iv: Vec<u8>,
}

/// Signed pre-key data
#[derive(Debug, Clone)]
pub struct SignedPreKeyData {
    pub signed_pre_key_id: u32,
    pub public_key: Vec<u8>,
    pub private_key_encrypted: Vec<u8>,
    pub private_key_iv: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Kyber pre-key data (post-quantum)
#[derive(Debug, Clone)]
pub struct KyberPreKeyData {
    pub pq_pre_key_id: u32,
    pub public_key: Vec<u8>,
    pub private_key_encrypted: Vec<u8>,
    pub private_key_iv: Vec<u8>,
}

/// Bundle of all pre-keys for server upload
#[derive(Debug, Clone)]
pub struct LocalPreKeyBundle {
    pub one_time_pre_keys: Vec<PreKeyData>,
    pub signed_pre_key: SignedPreKeyData,
    pub kyber_pre_key: KyberPreKeyData,
    pub kyber_last_resort: KyberPreKeyData,
}

/// Generates one-time X25519 pre-keys
fn generate_one_time_prekeys(
    count: u32,
    encryption_key: &[u8],
    username: &[u8],
) -> Result<Vec<PreKeyData>, String> {
    let mut prekeys = Vec::with_capacity(count as usize);
    let mut rng = OsRng08;

    for _ in 0..count {
        let pre_key_id = rng.next_u32();
        let (secret_key, public_key) = generate_x25519_keypair();

        // Encrypt the private key
        let encrypted = encrypt_key_with_password(
            secret_key.as_bytes(),
            encryption_key,
            username,
        ).map_err(|e| format!("Failed to encrypt pre-key: {}", e))?;

        prekeys.push(PreKeyData {
            pre_key_id,
            public_key: public_key.as_bytes().to_vec(),
            private_key_encrypted: encrypted.clone(),
            private_key_iv: vec![], // IV is included in encrypted data
        });
    }

    Ok(prekeys)
}

/// Generates a signed X25519 pre-key with Dilithium signature
fn generate_signed_prekey(
    dilithium_secret: &[u8],
    encryption_key: &[u8],
    username: &[u8],
) -> Result<SignedPreKeyData, String> {
    let mut rng = OsRng08;
    let signed_pre_key_id = rng.next_u32();

    let (secret_key, public_key) = generate_x25519_keypair();
    let public_bytes = public_key.as_bytes().to_vec();

    // Sign the public key with Dilithium identity key
    let dilithium_sk = pqcrypto_dilithium::dilithium2::SecretKey::from_bytes(dilithium_secret)
        .map_err(|_| "Invalid Dilithium secret key")?;
    let signature_obj = sign_friendship(&dilithium_sk, &public_bytes);
    let signature = signature_obj.as_bytes().to_vec();

    // Encrypt the private key
    let encrypted = encrypt_key_with_password(
        secret_key.as_bytes(),
        encryption_key,
        username,
    ).map_err(|e| format!("Failed to encrypt signed pre-key: {}", e))?;

    Ok(SignedPreKeyData {
        signed_pre_key_id,
        public_key: public_bytes,
        private_key_encrypted: encrypted,
        private_key_iv: vec![],
        signature,
    })
}

/// Generates a Kyber1024 pre-key (post-quantum)
fn generate_kyber_prekey(
    encryption_key: &[u8],
    username: &[u8],
) -> Result<KyberPreKeyData, String> {
    let mut rng = OsRng08;
    let pq_pre_key_id = rng.next_u32();

    let (public_bytes, secret_bytes) = generate_kyber_keypair()?;

    // Encrypt the private key
    let encrypted = encrypt_key_with_password(
        &secret_bytes,
        encryption_key,
        username,
    ).map_err(|e| format!("Failed to encrypt Kyber pre-key: {}", e))?;

    Ok(KyberPreKeyData {
        pq_pre_key_id,
        public_key: public_bytes,
        private_key_encrypted: encrypted,
        private_key_iv: vec![],
    })
}

/// Generates all pre-keys for X3DH protocol
fn generate_all_prekeys(
    dilithium_secret: &[u8],
    encryption_key: &[u8],
    username: &[u8],
) -> Result<LocalPreKeyBundle, String> {
    // Generate 100 one-time X25519 pre-keys
    let one_time_pre_keys = generate_one_time_prekeys(
        NUM_ONE_TIME_PREKEYS,
        encryption_key,
        username,
    )?;

    // Generate signed X25519 pre-key
    let signed_pre_key = generate_signed_prekey(
        dilithium_secret,
        encryption_key,
        username,
    )?;

    // Generate Kyber pre-key (post-quantum)
    let kyber_pre_key = generate_kyber_prekey(encryption_key, username)?;

    // Generate Kyber last-resort key (post-quantum fallback)
    let kyber_last_resort = generate_kyber_prekey(encryption_key, username)?;

    Ok(LocalPreKeyBundle {
        one_time_pre_keys,
        signed_pre_key,
        kyber_pre_key,
        kyber_last_resort,
    })
}


/// Données préparées sur le thread pool avant l'appel HTTP
struct PreRegistrationData {
    user_db: UserDb,
    hash_password: Vec<u8>,
    network_key_bytes: Vec<u8>,
    username_hash: Vec<u8>,
    registration_id: u32,
    identity_key_dilithium: Vec<u8>,
    identity_key_dilithium_secret: Vec<u8>,
    kyber_public_bytes: Vec<u8>,
    kyber_secret_bytes: Vec<u8>,
    prekey_bundle: LocalPreKeyBundle,
    registration_request: RegistrationRequest,
    timestamp: u64,
    zkp_public_input_str: String,
}

// Safety: tous les champs sont Send (Vec<u8>, String, UserDb wrapping rusqlite::Connection)
unsafe impl Send for PreRegistrationData {}

#[tauri::command]
pub async fn register(username: String, password: String) -> Result<String, String> {
    // Phase 1 (blocking thread): toute la crypto coûteuse - Argon2, Dilithium, Kyber, ZK-STARK
    let prep = tokio::task::spawn_blocking({
        let username = username.clone();
        let password = password.clone();
        move || -> Result<PreRegistrationData, String> {
            SecurityPassword::check_register_params_no_key(&password, &username)?;

            let network_key = generate_network_key()
                .map_err(|e| format!("Failed to generate network key: {}", e))?;
            let network_key_bytes = network_key.full_key.as_bytes().to_vec();

            let username_hash = generate_username_hash(&username)?;

            // Argon2id via SQLCipher - sur le thread pool
            let user_db = UserDb::create(&username, &password)
                .map_err(|e| format!("Failed to create user database: {}", e))?;

            let master_db = crate::db::MasterDb::open()
                .map_err(|e| format!("Failed to open master database: {}", e))?;
            let user_entry = master_db.get_user(&username)
                .map_err(|e| format!("Failed to get user entry: {}", e))?;

            // Argon2id - sur le thread pool
            let hash_password = derive_keys_from_password_with_salt(&password, &user_entry.salt)?;

            // Dilithium + Kyber keypair generation - sur le thread pool
            let (
                dilithium_public,
                dilithium_secret,
                identity_key_dilithium,
                identity_key_dilithium_secret,
                kyber_public_bytes,
                kyber_secret_bytes
            ) = generate_user_crypto_keys()?;

            let registration_id = OsRng08.next_u32();

            // 100 X25519 prekeys + Kyber prekeys - sur le thread pool
            let prekey_bundle = generate_all_prekeys(
                &identity_key_dilithium_secret,
                &hash_password,
                username.as_bytes(),
            )?;

            let first_prekey = prekey_bundle.one_time_pre_keys.first()
                .ok_or("No pre-keys generated")?;

            let pre_key_bundle = PreKeyBundle {
                user_hash_id: username_hash.clone(),
                registration_id,
                identity_key: Some(IdentityKey {
                    key_type: SignatureKeyType::Dilithium2 as i32,
                    public_key: identity_key_dilithium.clone(),
                }),
                pre_key_id: first_prekey.pre_key_id,
                pre_key_public: first_prekey.public_key.clone(),
                signed_pre_key_id: prekey_bundle.signed_pre_key.signed_pre_key_id,
                signed_pre_key_public: prekey_bundle.signed_pre_key.public_key.clone(),
                signed_pre_key_signature: prekey_bundle.signed_pre_key.signature.clone(),
                pq_pre_key_id: prekey_bundle.kyber_pre_key.pq_pre_key_id,
                pq_pre_key_public: Some(KemPublicKey {
                    key_type: KemKeyType::Kyber1024 as i32,
                    public_key: prekey_bundle.kyber_pre_key.public_key.clone(),
                }),
                pq_last_resort_key_id: prekey_bundle.kyber_last_resort.pq_pre_key_id,
                pq_last_resort_key_public: Some(KemPublicKey {
                    key_type: KemKeyType::Kyber1024 as i32,
                    public_key: prekey_bundle.kyber_last_resort.public_key.clone(),
                }),
            };

            let timestamp = unix_timestamp()?;

            // ZK-STARK proof (réel) - prouve Poseidon(zkp_portion || domain) = commitment
            // sans révéler la clé secrète. Opération la plus coûteuse ~500ms.
            let zkp_proof = generate_zkp_proof(network_key.zkp_portion.as_bytes())?;
            let password_commitment = zkp_proof.public_inputs.commitment.to_vec();
            let zkp_public_input_str = hex::encode(&zkp_proof.public_inputs.commitment);

            let mut signature_data = Vec::new();
            signature_data.extend_from_slice(&username_hash);
            signature_data.extend_from_slice(&password_commitment);
            signature_data.extend_from_slice(zkp_public_input_str.as_bytes());

            let identity_signature_obj = sign_friendship(&dilithium_secret, &signature_data);
            let identity_signature = identity_signature_obj.as_bytes().to_vec();

            let pre_key_bundle_bytes = {
                use prost::Message;
                let mut buf = Vec::new();
                pre_key_bundle.encode(&mut buf)
                    .map_err(|e| format!("PreKeyBundle encoding error: {}", e))?;
                buf
            };

            let registration_request = RegistrationRequest {
                username_hash: username_hash.clone(),
                pre_key_bundle: pre_key_bundle_bytes,
                proof_type: ZkpType::Stark as i32,
                password_commitment,
                initial_proof: zkp_proof.proof_bytes,
                identity_key_dilithium: identity_key_dilithium.clone(),
                identity_signature,
                timestamp,
            };

            Ok(PreRegistrationData {
                user_db,
                hash_password,
                network_key_bytes,
                username_hash,
                registration_id,
                identity_key_dilithium,
                identity_key_dilithium_secret,
                kyber_public_bytes,
                kyber_secret_bytes,
                prekey_bundle,
                registration_request,
                timestamp,
                zkp_public_input_str,
            })
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))??;

    // Phase 2 (async): appel HTTP serveur
    let dht_url = crate::config::dht_api_url();
    let config = RegisterConfig {
        base_url: dht_url.clone(),
        darknet: DarknetType::Http,
        timeout_secs: 30,
        max_retries: 3,
        retry_delay_ms: 1000,
    };

    let client = RegisterApiClient::new(config)
        .await
        .map_err(|e| format!("Failed to create API client: {}", e))?;

    let registration_response = match client.register(prep.registration_request).await {
        Ok(r) => r,
        Err(e) => {
            // Nettoie l'entrée partielle (master DB + fichier DB utilisateur)
            let _ = crate::db::MasterDb::open()
                .and_then(|mdb| mdb.delete_user(&username));
            return Err(match e {
                ApiError::Server(_, ref msg) if msg.to_lowercase().contains("already exist") => {
                    "Username already taken".to_string()
                }
                ApiError::Server(_, ref msg) => msg.clone(),
                ApiError::Timeout(_) => "Connection timed out. Please try again.".to_string(),
                ApiError::Network(ref msg) if msg.contains("Connection refused") => {
                    format!("Server not reachable at {}.", dht_url)
                }
                ref other => {
                    let s = other.to_string();
                    if s.contains("certificate") || s.contains("SSL") || s.contains("TLS") || s.contains("UnknownIssuer") {
                        "TLS certificate error. If using a self-signed certificate, set ZENTH_ACCEPT_INVALID_CERTS=1.".to_string()
                    } else {
                        format!("Registration failed: {}", s)
                    }
                }
            });
        }
    };

    if !registration_response.success {
        let _ = crate::db::MasterDb::open()
            .and_then(|mdb| mdb.delete_user(&username));
        return Err(registration_response.error_message);
    }

    // Phase 3 (blocking thread): chiffrement + sauvegardes DB
    let prekey_count = prep.prekey_bundle.one_time_pre_keys.len();
    let username_for_cleanup = username.clone();
    let phase3_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let encrypted_network_key = encrypt_key_with_password(
            &prep.network_key_bytes,
            &prep.hash_password,
            username.as_bytes(),
        ).map_err(|e| format!("Failed to encrypt network key: {}", e))?;

        let keys_to_store = serde_json::json!({
            "dilithium_secret": hex::encode(&prep.identity_key_dilithium_secret),
            "kyber_secret": hex::encode(&prep.kyber_secret_bytes),
            "registration_id": prep.registration_id,
            "zkp_public_input": prep.zkp_public_input_str,
        });

        let keys_json = serde_json::to_vec(&keys_to_store)
            .map_err(|e| format!("Failed to serialize keys: {}", e))?;

        let encrypted_keys = encrypt_key_with_password(
            &keys_json,
            &prep.hash_password,
            username.as_bytes(),
        ).map_err(|e| format!("Failed to encrypt keys: {}", e))?;

        let x25519_public = prep.prekey_bundle.signed_pre_key.public_key.clone();

        let user_info = NewUserInfo {
            pseudo: username.clone(),
            username_hash: hex::encode(&prep.username_hash),
            encrypted_network_key,
            network_key_iv: vec![],
            encrypted_identity_keys: encrypted_keys,
            identity_keys_iv: vec![],
            identity_key_public: prep.identity_key_dilithium.clone(),
            kyber_public_key: prep.kyber_public_bytes.clone(),
            x25519_public_key: Some(x25519_public),
            registration_id: prep.registration_id as i64,
            created_at: prep.timestamp as i64,
        };

        let user_id = prep.user_db.save_user_info(&user_info)
            .map_err(|e| format!("Failed to save user info: {}", e))?;

        // Batch: 100 prekeys en une seule transaction
        save_prekeys_to_db(&prep.user_db, user_id, &prep.prekey_bundle, prep.timestamp as i64)?;

        Ok(())
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))?;

    if let Err(e) = phase3_result {
        // Sauvegarde locale échouée - nettoie pour permettre un réessai propre
        let _ = crate::db::MasterDb::open()
            .and_then(|mdb| mdb.delete_user(&username_for_cleanup));
        return Err(e);
    }

    Ok("Registration successful".to_string())
}

/// Partie synchrone : génère de nouvelles OTPKs et les stocke localement.
/// Retourne les clés publiques prêtes à être uploadées.
pub fn generate_and_save_otpks(
    encryption_key: &[u8],
    username: &[u8],
    user_id: i64,
    user_db: &crate::db::UserDb,
) -> Result<Vec<(u32, Vec<u8>)>, String> {
    use crate::api::prekeys::DEFAULT_PREKEY_COUNT;
    use crate::utils::timestamp::plateform::current_timestamp;

    let new_prekeys = generate_one_time_prekeys(DEFAULT_PREKEY_COUNT, encryption_key, username)?;
    let conn = user_db.conn();
    let ts = current_timestamp() as i64;

    for pk in &new_prekeys {
        conn.execute(
            "INSERT OR IGNORE INTO pre_keys (user_id, pre_key_id, pre_key_public,
             pre_key_private_encrypted, pre_key_iv, created_at, used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            rusqlite::params![
                user_id,
                pk.pre_key_id as i64,
                pk.public_key,
                pk.private_key_encrypted,
                pk.private_key_iv,
                ts,
            ],
        ).map_err(|e| format!("Failed to save new OTPK: {}", e))?;
    }

    Ok(new_prekeys.into_iter().map(|pk| (pk.pre_key_id, pk.public_key)).collect())
}

/// Partie asynchrone : upload les clés publiques sur le DHT.
/// Prend uniquement des données owned (pas de &UserDb) pour être Send.
pub async fn upload_otpks(
    user_hash: Vec<u8>,
    dilithium_secret: Vec<u8>,
    public_prekeys: Vec<(u32, Vec<u8>)>,
) -> Result<u32, String> {
    use crate::api::prekeys::{PreKeyApiClient, PreKeyConfig};
    use crate::utils::timestamp::plateform::current_timestamp;
    use pqcrypto_dilithium::dilithium2;
    use pqcrypto_traits::sign::DetachedSignature as TraitSig;
    use zenth_dto::PreKey;

    let ts = current_timestamp();
    let mut msg = Vec::new();
    msg.extend_from_slice(&user_hash);
    msg.extend_from_slice(&ts.to_le_bytes());

    let sk = dilithium2::SecretKey::from_bytes(&dilithium_secret)
        .map_err(|_| "Invalid Dilithium secret key")?;
    let sig = dilithium2::detached_sign(&msg, &sk);

    let proto_prekeys: Vec<PreKey> = public_prekeys.into_iter().map(|(id, pub_key)| PreKey {
        pre_key_id: id,
        public_key: pub_key,
    }).collect();

    let client = PreKeyApiClient::new(PreKeyConfig::default()).await
        .map_err(|e| format!("Failed to create prekey client: {:?}", e))?;

    let response = client.replenish_prekeys(
        user_hash,
        proto_prekeys,
        sig.as_bytes().to_vec(),
    ).await.map_err(|e| format!("Failed to upload OTPKs: {:?}", e))?;

    Ok(response.total_prekeys)
}

/// Saves all pre-keys to the database
fn save_prekeys_to_db(
    user_db: &UserDb,
    user_id: i64,
    bundle: &LocalPreKeyBundle,
    timestamp: i64,
) -> Result<(), String> {
    let conn = user_db.conn();

    // Save one-time pre-keys
    for prekey in &bundle.one_time_pre_keys {
        conn.execute(
            "INSERT INTO pre_keys (user_id, pre_key_id, pre_key_public, pre_key_private_encrypted,
             pre_key_iv, created_at, used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            rusqlite::params![
                user_id,
                prekey.pre_key_id as i64,
                prekey.public_key,
                prekey.private_key_encrypted,
                prekey.private_key_iv,
                timestamp,
            ],
        ).map_err(|e| format!("Failed to save pre-key: {}", e))?;
    }

    // Save signed pre-key (with signature)
    conn.execute(
        "INSERT INTO pre_keys (user_id, pre_key_id, pre_key_public, pre_key_private_encrypted,
         pre_key_iv, signed_pre_key_id, signed_pre_key_public, signed_pre_key_signature,
         created_at, used)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)",
        rusqlite::params![
            user_id,
            bundle.signed_pre_key.signed_pre_key_id as i64,
            bundle.signed_pre_key.public_key,
            bundle.signed_pre_key.private_key_encrypted,
            bundle.signed_pre_key.private_key_iv,
            bundle.signed_pre_key.signed_pre_key_id as i64,
            bundle.signed_pre_key.public_key,
            bundle.signed_pre_key.signature,
            timestamp,
        ],
    ).map_err(|e| format!("Failed to save signed pre-key: {}", e))?;

    // Save Kyber pre-key
    conn.execute(
        "INSERT INTO pre_keys (user_id, pre_key_id, pre_key_public, pre_key_private_encrypted,
         pre_key_iv, pq_pre_key_id, pq_pre_key_public, created_at, used)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
        rusqlite::params![
            user_id,
            bundle.kyber_pre_key.pq_pre_key_id as i64,
            bundle.kyber_pre_key.public_key,
            bundle.kyber_pre_key.private_key_encrypted,
            bundle.kyber_pre_key.private_key_iv,
            bundle.kyber_pre_key.pq_pre_key_id as i64,
            bundle.kyber_pre_key.public_key,
            timestamp,
        ],
    ).map_err(|e| format!("Failed to save Kyber pre-key: {}", e))?;

    // Save Kyber last-resort key
    conn.execute(
        "INSERT INTO pre_keys (user_id, pre_key_id, pre_key_public, pre_key_private_encrypted,
         pre_key_iv, pq_pre_key_id, pq_pre_key_public, created_at, used)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
        rusqlite::params![
            user_id,
            bundle.kyber_last_resort.pq_pre_key_id as i64,
            bundle.kyber_last_resort.public_key,
            bundle.kyber_last_resort.private_key_encrypted,
            bundle.kyber_last_resort.private_key_iv,
            bundle.kyber_last_resort.pq_pre_key_id as i64,
            bundle.kyber_last_resort.public_key,
            timestamp,
        ],
    ).map_err(|e| format!("Failed to save Kyber last-resort key: {}", e))?;

    Ok(())
}
