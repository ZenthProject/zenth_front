use rand::rngs::OsRng;
use rand::RngCore;
use zenth_crypto_service::{base64decode, hashs::{base64_vecdecode, base64encode, HashSecure, HasherImpl}};
use hex;

pub fn key_gen_database(username: &str, password: &str, pin: &str) -> (String, String) {
    let sha3_result = HasherImpl::sha512_fun(&format!("{}{}", password, pin), 5);
    let sha3_bytes = base64_vecdecode(&sha3_result).expect("Base64 decode failed");
    let keccak_result = HasherImpl::keccak_fun(&format!("{}{}", username, password), 5);
    let keccak_bytes = base64_vecdecode(&keccak_result).expect("Base64 decode failed");
    let crc_result = HasherImpl::crc_fun(&format!("{}{}", username, pin), 3);
    let crc_bytes = base64_vecdecode(&crc_result).expect("Base64 decode failed");
    let mut combined = Vec::new();
    for (a, b) in sha3_bytes.iter().zip(keccak_bytes.iter()) {
        combined.push(a ^ b);
    }
    for (i, byte) in crc_bytes.iter().enumerate() {
        if i < combined.len() {
            combined[i] ^= byte;
        }
    }
    let final_hash = HasherImpl::sha512_fun(&base64encode(&combined), 1);
    let final_bytes = base64_vecdecode(&final_hash).expect("Base64 decode failed");
    let mut final_key = final_bytes[..32].to_vec();
    let leftover_bytes = &final_bytes[32..];

    let mut crc_val = 0u32;
    for byte in &crc_bytes {
        crc_val = crc_val.wrapping_add(*byte as u32);
    }
    apply_permutation(&mut final_key, crc_val);
    let final_key_b64 = base64encode(&final_key);
    let db_name_raw = base64encode(leftover_bytes);
    let db_name_clean = db_name_raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>();

    let db_name = format!("db_{}.sqlite", db_name_clean);

    (final_key_b64, db_name)
}





fn apply_permutation(data: &mut Vec<u8>, seed: u32) {
    let len = data.len();
    let mut index = (seed % len as u32) as usize;

    for i in 0..len {
        let swap_index = (index + (i * 7) % len) % len;
        data.swap(i, swap_index);
        index = (index.wrapping_mul(31) + seed as usize) % len;
    }
}




pub fn generate_random_pepper(size: usize) -> String {
    let mut pepper_bytes = vec![0u8; size];
    OsRng.fill_bytes(&mut pepper_bytes);
    base64encode(&pepper_bytes)
}
