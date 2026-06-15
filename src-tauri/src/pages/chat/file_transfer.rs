use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;
use prost::Message as ProstMessage;
use tauri::{AppHandle, Emitter};
use zenth_crypto::symmetric::{Aes256GcmEncryption, Aes256GcmDecryption};
use zenth_crypto::hashing::hash::CryptographicHash;
use zenth_dto::{
    DhtRequest, Method,
    WsFileAvailableRequest, WsFileChunkRequest, WsFileChunkResponse,
    WsCheckOnlineRequest,
};

use crate::utils::timestamp::plateform::current_timestamp;

const CHUNK_SIZE: usize = 65_536; // 64 KB par chunk
const TAG_SIZE: usize = 16;       // AES-256-GCM authentication tag
const NONCE_BASE_SIZE: usize = 8; // 8 bytes aléatoires + 4 bytes index = 12 bytes nonce

// Types
type TransferId = [u8; 16];

/// Entrée côté expéditeur.
/// Les chunks sont immutables après création → on peut lire en parallèle avec un read lock.
/// chunks_served est atomique → pas besoin de write lock pour incrémenter.
pub struct SenderEntry {
    pub chunks: Vec<Vec<u8>>,
    pub filename: String,
    pub chunks_served: AtomicU32,
}

pub struct ReceiverState {
    pub total_chunks: u32,
    pub chunks: Vec<Option<Vec<u8>>>,
    pub file_key: [u8; 32],
    pub file_iv: [u8; 12],
    pub filename: String,
    pub mime: String,
    pub file_hash: Vec<u8>,
    pub my_hash: Vec<u8>,
    pub sender_hash: Vec<u8>,
}

// Stores
// Read lock pour accéder aux chunks (plusieurs transferts en parallèle sans blocage).
// Write lock uniquement pour insert/remove d'un transfert.
static SENDER_STORE: OnceLock<Arc<RwLock<HashMap<TransferId, SenderEntry>>>> = OnceLock::new();
static RECEIVER_STORE: OnceLock<Arc<RwLock<HashMap<TransferId, ReceiverState>>>> = OnceLock::new();

fn sender_store() -> Arc<RwLock<HashMap<TransferId, SenderEntry>>> {
    SENDER_STORE.get().expect("file_transfer not initialised").clone()
}

fn receiver_store() -> Arc<RwLock<HashMap<TransferId, ReceiverState>>> {
    RECEIVER_STORE.get().expect("file_transfer not initialised").clone()
}

pub fn init() {
    SENDER_STORE.set(Arc::new(RwLock::new(HashMap::new()))).ok();
    RECEIVER_STORE.set(Arc::new(RwLock::new(HashMap::new()))).ok();
}

// Crypto
fn chunk_nonce(base_iv: &[u8; 12], chunk_index: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..NONCE_BASE_SIZE].copy_from_slice(&base_iv[..NONCE_BASE_SIZE]);
    nonce[NONCE_BASE_SIZE..].copy_from_slice(&chunk_index.to_le_bytes());
    nonce
}

pub fn encrypt_file_chunks(
    data: &[u8],
    key: &[u8; 32],
    base_iv: &[u8; 12],
) -> Result<Vec<Vec<u8>>, String> {
    data.chunks(CHUNK_SIZE)
        .enumerate()
        .map(|(i, chunk_data)| {
            let nonce = chunk_nonce(base_iv, i as u32);
            let mut enc = Aes256GcmEncryption::new(key, &nonce, b"ZENTH_FILE")
                .map_err(|e| format!("AES-GCM init chunk {}: {:?}", i, e))?;
            let mut buf = chunk_data.to_vec();
            enc.encrypt(&mut buf);
            let tag = enc.compute_tag();
            buf.extend_from_slice(&tag);
            Ok(buf)
        })
        .collect()
}

pub fn decrypt_file(
    chunks: &[Vec<u8>],
    key: &[u8; 32],
    base_iv: &[u8; 12],
    expected_hash: &[u8],
) -> Result<Vec<u8>, String> {
    let mut plaintext = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        if chunk.len() < TAG_SIZE {
            return Err(format!("Chunk {} trop court ({} bytes)", i, chunk.len()));
        }
        let (ciphertext, tag) = chunk.split_at(chunk.len() - TAG_SIZE);
        let nonce = chunk_nonce(base_iv, i as u32);

        let mut dec = Aes256GcmDecryption::new(key, &nonce, b"ZENTH_FILE")
            .map_err(|e| format!("AES-GCM init chunk {}: {:?}", i, e))?;
        let mut buf = ciphertext.to_vec();
        dec.decrypt(&mut buf);
        dec.verify_tag(tag)
            .map_err(|_| format!("Tag invalide pour chunk {}: fichier corrompu ou altéré", i))?;
        plaintext.extend_from_slice(&buf);
    }

    let mut hasher = CryptographicHash::new("SHA-256", 1)
        .map_err(|e| format!("CryptographicHash: {:?}", e))?;
    hasher.update(&plaintext);
    let hash = hasher.finalize();
    if hash.as_slice() != expected_hash {
        return Err("Hash du fichier invalide: données corrompues".to_string());
    }
    Ok(plaintext)
}

fn sha256_of(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut hasher = CryptographicHash::new("SHA-256", 1)
        .map_err(|e| format!("CryptographicHash: {:?}", e))?;
    hasher.update(data);
    Ok(hasher.finalize())
}

// WebSocket helpers
async fn ws_send_dht(method: Method, payload_bytes: Vec<u8>) -> Result<(), String> {
    let request = DhtRequest {
        method: method as i32,
        payload: payload_bytes,
        timestamp: current_timestamp(),
        request_id: vec![],
    };
    let mut buf = Vec::new();
    request.encode(&mut buf).map_err(|e| format!("encode DhtRequest: {}", e))?;
    crate::websocket::ws_send(buf).await
}

async fn send_chunk_request(transfer_id: Vec<u8>, chunk_index: u32, my_hash: Vec<u8>) -> Result<(), String> {
    let req = WsFileChunkRequest { transfer_id, chunk_index, requester_hash: my_hash };
    let mut payload = Vec::new();
    req.encode(&mut payload).map_err(|e| format!("encode WsFileChunkRequest: {}", e))?;
    ws_send_dht(Method::WsFileChunkRequest, payload).await
}

// Events Tauri (sender)
#[derive(Clone, serde::Serialize)]
pub struct FileSendStartedEvent {
    pub transfer_id: Vec<u8>,
    pub filename: String,
    pub total_chunks: u32,
}

#[derive(Clone, serde::Serialize)]
pub struct FileSendProgressEvent {
    pub transfer_id: Vec<u8>,
    pub chunks_served: u32,
    pub total_chunks: u32,
}

#[derive(Clone, serde::Serialize)]
pub struct FileSendCompleteEvent {
    pub transfer_id: Vec<u8>,
    pub filename: String,
}

// Events Tauri (receiver)
#[derive(Clone, serde::Serialize)]
pub struct FileCompleteEvent {
    pub transfer_id: Vec<u8>,
    pub filename: String,
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Clone, serde::Serialize)]
pub struct FileTransferErrorEvent {
    pub transfer_id: Vec<u8>,
    pub error: String,
}

// Gestion sender (appelé depuis websocket/mod.rs)
/// DHT a relayé une demande de chunk → l'expéditeur répond.
/// Utilise un read lock sur le store (non-bloquant pour les autres transferts simultanés).
/// Le compteur chunks_served est atomique (aucun write lock supplémentaire requis).
pub async fn handle_chunk_request(req: WsFileChunkRequest, app: &AppHandle) {
    let tid: TransferId = match req.transfer_id.as_slice().try_into() {
        Ok(t) => t,
        Err(_) => return,
    };

    // Read lock : plusieurs handle_chunk_request peuvent tourner en parallèle
    let arc = sender_store();
    let store = arc.read().await;

    let entry = match store.get(&tid) {
        Some(e) => e,
        None => return,
    };

    let idx = req.chunk_index as usize;
    let chunk_data = match entry.chunks.get(idx) {
        Some(c) => c.clone(),
        None => return,
    };
    let total = entry.chunks.len() as u32;
    let filename = entry.filename.clone();

    // Incrémenter atomiquement: pas de write lock
    let served_before = entry.chunks_served.fetch_add(1, Ordering::Relaxed);
    let served_after = served_before + 1;

    drop(store); // libère le read lock avant l'envoi WS

    // Notifier le JS au premier chunk : "garde la connexion"
    if served_before == 0 {
        let _ = app.emit("file-send-started", FileSendStartedEvent {
            transfer_id: req.transfer_id.clone(),
            filename: filename.clone(),
            total_chunks: total,
        });
    }

    // Envoyer le chunk au DHT (non-bloquant pour les autres transferts)
    let resp = WsFileChunkResponse {
        transfer_id: req.transfer_id.clone(),
        chunk_index: req.chunk_index,
        chunk_data,
        recipient_hash: req.requester_hash,
        total_chunks: total,
    };
    let mut payload = Vec::new();
    if resp.encode(&mut payload).is_err() {
        return;
    }
    let _ = ws_send_dht(Method::WsFileChunk, payload).await;

    // Notifier le JS quand tous les chunks ont été servis
    if served_after >= total {
        let _ = app.emit("file-send-complete", FileSendCompleteEvent {
            transfer_id: req.transfer_id,
            filename,
        });

        // Nettoyer le sender store (libère la RAM des chunks chiffrés)
        let arc2 = sender_store();
        let mut store2 = arc2.write().await;
        store2.remove(&tid);
    }
}

// Gestion receiver (appelé depuis websocket/mod.rs)
/// DHT a relayé un chunk → l'accumuler et demander le suivant.
pub async fn handle_incoming_chunk(resp: WsFileChunkResponse, app: &AppHandle) {
    let tid: TransferId = match resp.transfer_id.as_slice().try_into() {
        Ok(t) => t,
        Err(_) => return,
    };

    let arc = receiver_store();
    let mut store = arc.write().await;

    let state = match store.get_mut(&tid) {
        Some(s) => s,
        None => return,
    };

    let idx = resp.chunk_index as usize;
    if idx < state.chunks.len() && state.chunks[idx].is_none() {
        state.chunks[idx] = Some(resp.chunk_data);
    }

    let next_missing = state.chunks.iter().position(|c| c.is_none());
    let all_received = next_missing.is_none();

    if !all_received {
        let next_idx = next_missing.unwrap() as u32;
        let tid_vec = resp.transfer_id.clone();
        let my_hash = state.my_hash.clone();
        drop(store);
        let _ = send_chunk_request(tid_vec, next_idx, my_hash).await;
        return;
    }

    // Tous reçus: déchiffrer
    let chunks: Vec<Vec<u8>> = state.chunks.iter().filter_map(|c| c.clone()).collect();
    let key = state.file_key;
    let iv = state.file_iv;
    let filename = state.filename.clone();
    let mime = state.mime.clone();
    let file_hash = state.file_hash.clone();
    drop(store);

    match decrypt_file(&chunks, &key, &iv, &file_hash) {
        Ok(data) => {
            let _ = app.emit("file-transfer-complete", FileCompleteEvent {
                transfer_id: resp.transfer_id.clone(),
                filename,
                mime,
                data,
            });
        }
        Err(e) => {
            let _ = app.emit("file-transfer-error", FileTransferErrorEvent {
                transfer_id: resp.transfer_id.clone(),
                error: e,
            });
        }
    }

    let arc2 = receiver_store();
    let mut store2 = arc2.write().await;
    store2.remove(&tid);
}

// Commandes Tauri
#[derive(serde::Serialize)]
pub struct PreparedTransfer {
    pub transfer_id: Vec<u8>,
    pub file_key: Vec<u8>,
    pub file_iv: Vec<u8>,
    pub file_hash: Vec<u8>,
    pub chunk_count: u32,
    pub chunk_size: u32,
}

/// Chiffre le fichier en chunks, stocke dans le sender store, annonce au DHT.
/// Retourne les métadonnées à mettre dans l'InnerFileOffer.
#[tauri::command]
pub async fn prepare_file_transfer(
    file_data: Vec<u8>,
    filename: String,
    mime: String,
) -> Result<PreparedTransfer, String> {
    use rand::RngCore;

    let file_hash = sha256_of(&file_data)?;

    let mut key = [0u8; 32];
    let mut iv = [0u8; 12];
    rand::rng().fill_bytes(&mut key);
    rand::rng().fill_bytes(&mut iv[..NONCE_BASE_SIZE]);

    let chunks = encrypt_file_chunks(&file_data, &key, &iv)?;
    let chunk_count = chunks.len() as u32;

    let mut transfer_id = [0u8; 16];
    rand::rng().fill_bytes(&mut transfer_id);

    {
        let arc = sender_store();
        let mut store = arc.write().await;
        store.insert(transfer_id, SenderEntry {
            chunks,
            filename: filename.clone(),
            chunks_served: AtomicU32::new(0),
        });
    }

    let avail = WsFileAvailableRequest { transfer_id: transfer_id.to_vec() };
    let mut payload = Vec::new();
    avail.encode(&mut payload).map_err(|e| format!("encode WsFileAvailable: {}", e))?;
    let _ = ws_send_dht(Method::WsFileAvailable, payload).await;

    Ok(PreparedTransfer {
        transfer_id: transfer_id.to_vec(),
        file_key: key.to_vec(),
        file_iv: iv.to_vec(),
        file_hash,
        chunk_count,
        chunk_size: CHUNK_SIZE as u32,
    })
}

/// Annule un transfert sortant et libère la RAM.
#[tauri::command]
pub async fn cancel_file_transfer(transfer_id: Vec<u8>) -> Result<(), String> {
    let tid: TransferId = transfer_id.as_slice().try_into()
        .map_err(|_| "transfer_id invalide".to_string())?;
    let arc = sender_store();
    let mut store = arc.write().await;
    store.remove(&tid);
    Ok(())
}

/// Démarre le téléchargement d'un fichier reçu via InnerFileOffer.
#[tauri::command]
pub async fn start_file_download(
    transfer_id: Vec<u8>,
    sender_hash: Vec<u8>,
    my_hash: Vec<u8>,
    file_key: Vec<u8>,
    file_iv: Vec<u8>,
    filename: String,
    mime: String,
    total_chunks: u32,
    file_hash: Vec<u8>,
) -> Result<(), String> {
    let tid: TransferId = transfer_id.as_slice().try_into()
        .map_err(|_| "transfer_id invalide".to_string())?;
    let key: [u8; 32] = file_key.as_slice().try_into()
        .map_err(|_| "file_key doit faire 32 bytes".to_string())?;
    let iv: [u8; 12] = file_iv.as_slice().try_into()
        .map_err(|_| "file_iv doit faire 12 bytes".to_string())?;

    if total_chunks == 0 {
        return Err("total_chunks ne peut pas être zéro".to_string());
    }

    // Vérifier que l'expéditeur est connecté avant de commencer
    let check = WsCheckOnlineRequest { target_hash: sender_hash.clone() };
    let mut payload = Vec::new();
    check.encode(&mut payload).map_err(|e| format!("encode WsCheckOnline: {}", e))?;
    ws_send_dht(Method::WsCheckOnline, payload).await?;

    {
        let arc = receiver_store();
        let mut store = arc.write().await;
        store.insert(tid, ReceiverState {
            total_chunks,
            chunks: vec![None; total_chunks as usize],
            file_key: key,
            file_iv: iv,
            filename,
            mime,
            file_hash,
            my_hash: my_hash.clone(),
            sender_hash,
        });
    }

    send_chunk_request(transfer_id, 0, my_hash).await
}
