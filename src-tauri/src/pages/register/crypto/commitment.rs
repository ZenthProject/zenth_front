use zenth_crypto::sk_stark::{prove_secret_key, SecretKeyProof};

const ZKP_DOMAIN: &[u8; 16] = b"zenth_auth_v1\0\0\0";

/// Generate a real SK-STARK proof for the 2000-char ZKP portion of the network key.
///
/// Proves: Poseidon(zkp_secret || domain) = commitment
/// without revealing the secret key.
pub fn generate_zkp_proof(zkp_secret: &[u8]) -> Result<SecretKeyProof, String> {
    prove_secret_key(zkp_secret, ZKP_DOMAIN)
        .map_err(|e| format!("ZK proof generation failed: {}", e))
}
