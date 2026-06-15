//! Commandes Tauri pour la mise à jour de l'application.
//!
//! Flux :
//!   1. check_update  → interroge le DHT (METHOD 19), retourne Some(version) si dispo
//!   2. download_update → télécharge via METHOD 20, vérifie SHA-256 + Ed25519, retourne chemin
//!   3. apply_update  → installe le binaire téléchargé

use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use tauri::AppHandle;

use crate::api::update::UpdateApiClient;

// Chemin du binaire téléchargé - jamais exposé au frontend
static PENDING_UPDATE_PATH: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

// Clé publique Ed25519 intégrée au binaire
// Remplace ces bytes par la clé publique générée via :
//   openssl genpkey -algorithm ed25519 -out update_private.pem
//   openssl pkey -in update_private.pem -pubout -outform DER | tail -c 32 | xxd -i
// La clé privée va dans les secrets GitLab CI - ne jamais committer.

const UPDATE_PUBKEY: &[u8; 32] = &[
    0xfb, 0x86, 0x0c, 0xd1, 0x91, 0x84, 0x31, 0xbd, 
    0x5c, 0x1d, 0xa7, 0xe6, 0xb8, 0x7a, 0x1f, 0xdc, 
    0x47, 0x46, 0x18, 0x8b, 0xb1, 0x0c, 0xd4, 0x78, 
    0xfa, 0x47, 0x29, 0x00, 0xda, 0x1b, 0xd5, 0xa5
];


/// Détecte la plateforme ET le format d'installation courant.
/// Linux : APPIMAGE env var présente → AppImage, sinon → deb
fn current_platform() -> String {
    #[cfg(all(target_os = "android", target_arch = "aarch64"))]
    return "android-arm64".to_string();

    #[cfg(all(target_os = "android", target_arch = "x86_64"))]
    return "android-x86_64".to_string();

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        if std::env::var("APPIMAGE").is_ok() {
            return "linux-x86_64-appimage".to_string();
        }
        return "linux-x86_64-deb".to_string();
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "windows-x86_64".to_string();

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "macos-x86_64".to_string();

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "macos-aarch64".to_string();

    #[allow(unreachable_code)]
    "linux-x86_64-deb".to_string()
}

/// Vérifie la signature Ed25519 du SHA-256 hex.
fn verify_signature(sha256_hex: &str, signature: &[u8]) -> Result<(), String> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    // Refuse de vérifier si la clé publique est encore le placeholder
    if UPDATE_PUBKEY == &[0u8; 32] {
        return Err("Clé de signature non configurée - mises à jour désactivées".to_string());
    }

    let key = VerifyingKey::from_bytes(UPDATE_PUBKEY)
        .map_err(|e| format!("Clé publique invalide : {}", e))?;

    let sig_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| "Signature invalide (taille incorrecte)".to_string())?;

    let sig = Signature::from_bytes(&sig_bytes);

    key.verify(sha256_hex.as_bytes(), &sig)
        .map_err(|_| "Signature Ed25519 invalide - binaire compromis ou falsifié".to_string())
}

/// Vérifie si une mise à jour est disponible.
/// Retourne `Some(latest_version)` si le serveur a une version plus récente,
/// `None` si l'app est à jour.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<String>, String> {
    let current = tauri::Manager::package_info(&app).version.to_string();
    let platform = current_platform();

    let client = UpdateApiClient::new().await
        .map_err(|e| format!("Connexion DHT impossible : {}", e))?;

    let manifest = client
        .get_manifest(&platform)
        .await
        .map_err(|e| format!("Manifest indisponible : {}", e))?;

    if manifest.latest_version != current {
        Ok(Some(manifest.latest_version))
    } else {
        Ok(None)
    }
}

/// Télécharge la mise à jour, vérifie SHA-256 + signature Ed25519.
/// Émet des événements `update-progress` { bytes: u64, total: u64 }.
/// Le chemin du binaire est stocké en mémoire Rust - jamais retourné au frontend.
#[tauri::command]
pub async fn download_update(app: AppHandle) -> Result<(), String> {
    use tauri::Emitter;

    let current = tauri::Manager::package_info(&app).version.to_string();
    let platform = current_platform();

    let client = UpdateApiClient::new().await
        .map_err(|e| format!("Connexion DHT impossible : {}", e))?;

    let manifest = client
        .get_manifest(&platform)
        .await
        .map_err(|e| format!("Manifest indisponible : {}", e))?;

    // Détermine l'extension selon la plateforme détectée
    let ext = if platform.ends_with("deb") { "deb" }
              else if platform.ends_with("appimage") { "AppImage" }
              else if platform.starts_with("windows") { "exe" }
              else if platform.starts_with("android") { "apk" }
              else { "bin" };

    let dest_path = std::env::temp_dir()
        .join(format!("zenth_update_{}.{}", manifest.latest_version, ext));

    let mut file = std::fs::File::create(&dest_path)
        .map_err(|e| format!("Impossible de créer le fichier temporaire : {}", e))?;

    let total = manifest.size;
    let app_clone = app.clone();

    client
        .download_binary(&platform, total, &mut file, &mut |received, total| {
            let _ = app_clone.emit("update-progress", serde_json::json!({
                "bytes": received,
                "total": total,
            }));
        })
        .await
        .map_err(|e| format!("Téléchargement échoué : {}", e))?;

    file.flush().map_err(|e| format!("Flush erreur : {}", e))?;
    drop(file);

    // Vérifie le SHA-256
    let data = std::fs::read(&dest_path)
        .map_err(|e| format!("Lecture fichier échouée : {}", e))?;

    let computed = format!("{:x}", Sha256::digest(&data));
    if computed != manifest.sha256 {
        let _ = std::fs::remove_file(&dest_path);
        return Err(format!(
            "SHA-256 invalide - fichier corrompu (attendu: {}, obtenu: {})",
            manifest.sha256, computed
        ));
    }

    // Vérifie la signature Ed25519
    verify_signature(&manifest.sha256, &manifest.signature)?;

    // Stocke le chemin côté Rust - ne jamais le retourner au frontend
    *PENDING_UPDATE_PATH.lock().unwrap() = Some(dest_path);
    Ok(())
}

/// Installe la mise à jour téléchargée.
/// Le chemin est lu depuis la mémoire Rust - aucun paramètre accepté depuis le frontend.
#[tauri::command]
pub async fn apply_update(app: AppHandle) -> Result<(), String> {
    let path = PENDING_UPDATE_PATH.lock().unwrap()
        .take()
        .ok_or("Aucune mise à jour téléchargée ou déjà installée")?;

    // Vérifier que le chemin est bien dans le dossier temporaire système
    let tmp = std::env::temp_dir();
    if !path.starts_with(&tmp) {
        return Err("Chemin de mise à jour invalide".to_string());
    }

    // Vérifier que le nom de fichier correspond au pattern attendu
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if !filename.starts_with("zenth_update_") {
        return Err("Fichier de mise à jour invalide".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let path_str = path.to_str().ok_or("Chemin invalide")?;

        // AppImage : remplace le fichier courant en place, redémarrage manuel requis
        if let Ok(appimage_path) = std::env::var("APPIMAGE") {
            std::fs::copy(&path, &appimage_path)
                .map_err(|e| format!("Impossible de remplacer l'AppImage : {}", e))?;
            std::fs::set_permissions(&appimage_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("chmod AppImage échoué : {}", e))?;
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }

        // .deb : installation système via pkexec dpkg
        let status = std::process::Command::new("pkexec")
            .args(["dpkg", "-i", path_str])
            .status()
            .map_err(|e| format!("Impossible de lancer dpkg : {}", e))?;

        if !status.success() {
            return Err(format!("dpkg a échoué (code {:?})", status.code()));
        }

        let _ = std::fs::remove_file(&path);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        // Installeur NSIS : auto-extractible, se lance directement
        let path_str = path.to_str().ok_or("Chemin invalide")?;
        std::process::Command::new(path_str)
            .spawn()
            .map_err(|e| format!("Impossible de lancer l'installeur : {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "android")]
    {
        use tauri_plugin_opener::OpenerExt;
        let path_str = path.to_str().ok_or("Chemin invalide")?;
        // opener gère le FileProvider Android automatiquement
        app.opener()
            .open_path(path_str, Some("application/vnd.android.package-archive"))
            .map_err(|e| format!("Impossible de lancer l'installeur APK : {}", e))?;
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "android")))]
    {
        let _ = app;
        Err("Plateforme non supportée pour l'installation automatique".to_string())
    }
}
