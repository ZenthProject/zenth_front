use rand::RngCore;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::Deserialize;
use tauri::command;
use sha2::{Sha256, Digest};

#[derive(Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub t: i64,
}

#[command]
pub fn generate_random_string_chunk(
    points: Vec<Point>,
    chunk_size: usize,
    browser_entropy: Option<Vec<u8>>,
) -> Result<String, String> {
    if chunk_size == 0 {
        return Err("chunk_size must be > 0".into());
    }

    // Source 1 : entropie souris (x, y, timestamp)
    let mut mouse_bytes = Vec::with_capacity(points.len() * 16);
    for point in &points {
        mouse_bytes.extend(&point.x.to_le_bytes());
        mouse_bytes.extend(&point.y.to_le_bytes());
        mouse_bytes.extend(&point.t.to_le_bytes());
    }

    // Source 2 : entropie OS (getrandom/dev/urandom, processus natif)
    let mut os_entropy = [0u8; 32];
    rand::rng().fill_bytes(&mut os_entropy);

    // Source 3 : entropie browser (crypto.getRandomValues, processus WebView)
    // Les 3 sources sont indépendantes: compromise d'une seule ne suffit pas

    let mut hasher = Sha256::new();
    hasher.update(&mouse_bytes);
    hasher.update(&os_entropy);
    if let Some(ref extra) = browser_entropy {
        hasher.update(extra);
    }
    let seed: [u8; 32] = hasher.finalize().into();

    // CSPRNG seedé par les 3 sources combinées
    let mut rng = ChaCha20Rng::from_seed(seed);

    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|;:,.<>?";

    let mut result = String::with_capacity(chunk_size);
    for _ in 0..chunk_size {
        let idx = (rng.next_u32() as usize) % CHARSET.len();
        result.push(CHARSET[idx] as char);
    }

    Ok(result)
}






