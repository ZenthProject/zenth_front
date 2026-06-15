use rand::RngCore;
use zenth_crypto::{
    errors::error::{Error, Result},
    symmetric::{
        aes_gcm::{NONCE_SIZE, TAG_SIZE},
        Aes256GcmEncryption,
        Aes256GcmDecryption,
    }
};

pub fn encrypt_key_with_password(
    secret_key: &[u8],
    derived_key: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>> {
    if derived_key.len() != 32 {
        return Err(Error::InvalidKeySize);
    }

    let mut nonce = [0u8; NONCE_SIZE];
    rand::rng().fill_bytes(&mut nonce);

    let mut buf = secret_key.to_vec();

    let mut enc = Aes256GcmEncryption::new(derived_key, &nonce, associated_data)?;

    enc.encrypt(&mut buf);
    let tag = enc.compute_tag();

    let mut out = Vec::with_capacity(NONCE_SIZE + buf.len() + TAG_SIZE);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&buf);
    out.extend_from_slice(&tag);

    Ok(out)
}

#[allow(dead_code)]
pub fn decrypt_key_with_password(
    encrypted_data: &[u8],
    derived_key: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>> {
    if derived_key.len() != 32 {
        return Err(Error::InvalidKeySize);
    }

    if encrypted_data.len() < NONCE_SIZE + TAG_SIZE {
        return Err(Error::InvalidInputSize);
    }

    let nonce = &encrypted_data[..NONCE_SIZE];
    let tag_start = encrypted_data.len() - TAG_SIZE;
    let ciphertext = &encrypted_data[NONCE_SIZE..tag_start];
    let tag = &encrypted_data[tag_start..];

    let mut dec = Aes256GcmDecryption::new(derived_key, nonce, associated_data)?;

    let mut buf = ciphertext.to_vec();
    dec.decrypt(&mut buf);
    dec.verify_tag(tag)?;

    Ok(buf)
}
