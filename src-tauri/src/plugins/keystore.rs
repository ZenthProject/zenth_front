use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "android")]
use tauri::plugin::PluginHandle;

#[cfg(target_os = "android")]
pub struct KeystoreHandle<R: Runtime>(pub PluginHandle<R>);

const KEYRING_SERVICE: &str = "zenth";
const KEYRING_ACTIVE_USER_KEY: &str = "__active_user__";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("keystore")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin(
                    "com.zenth_project.app",
                    "KeystorePlugin",
                )?;
                app.manage(KeystoreHandle(handle));
            }
            Ok(())
        })
        .build()
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Credential {
    pub username: String,
    pub password: String,
}

#[derive(serde::Deserialize, Default)]
struct VoidResult {}

// Helpers desktop
#[cfg(not(target_os = "android"))]
fn is_backend_unavailable(err: &keyring::Error) -> bool {
    matches!(
        err,
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_)
    )
}

// Fallback : fichier chiffré (AES-256-GCM)
//
// Utilisé quand aucun keyring OS n'est disponible (Linux sans Secret Service,
// Linux Mint MATE/XFCE, etc.). Le fichier est chiffré avec une clé dérivée du
// machine-id: il ne contient jamais de données en clair sur le disque.
//
// Format du fichier : [nonce : 12 o][ciphertext][tag GCM : 16 o]
// Plaintext          : JSON {"u":"<username>","p":"<password>"}

#[cfg(not(target_os = "android"))]
mod file_store {
    use std::path::PathBuf;
    use hkdf::Hkdf;
    use sha2::Sha256;
    use zenth_crypto::symmetric::{
        Aes256GcmEncryption, Aes256GcmDecryption,
        AES_GCM_NONCE_SIZE, AES_GCM_TAG_SIZE,
    };

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Stored { u: String, p: String }

    fn creds_path(app_data_dir: &PathBuf) -> PathBuf {
        app_data_dir.join("zenth_creds.dat")
    }

    /// Dérive une clé AES-256 à partir du machine-id de la machine.
    /// Sur Linux : /etc/machine-id: présent sur tous les systèmes systemd/udev modernes.
    /// Si la lecture échoue, on utilise une chaîne vide (dégradation minimale).
    fn derive_key() -> [u8; 32] {
        let machine_id = std::fs::read_to_string("/etc/machine-id")
            .unwrap_or_default();
        let machine_id = machine_id.trim();

        let hk = Hkdf::<Sha256>::new(Some(b"zenth-creds-v1"), machine_id.as_bytes());
        let mut key = [0u8; 32];
        // expand ne peut échouer que si len > 255 * HashLen, impossible ici
        let _ = hk.expand(b"file-encryption-key", &mut key);
        key
    }

    pub fn store(app_data_dir: &PathBuf, username: &str, password: &str) -> Result<(), String> {
        let payload = serde_json::to_vec(&Stored {
            u: username.to_string(),
            p: password.to_string(),
        }).map_err(|e| e.to_string())?;

        let key = derive_key();
        let mut nonce = [0u8; AES_GCM_NONCE_SIZE];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;

        let mut ciphertext = payload;
        let mut enc = Aes256GcmEncryption::new(&key, &nonce, b"")
            .map_err(|e| format!("AES-GCM init: {:?}", e))?;
        enc.encrypt(&mut ciphertext);
        let tag = enc.compute_tag();

        let mut file_data = Vec::with_capacity(AES_GCM_NONCE_SIZE + ciphertext.len() + AES_GCM_TAG_SIZE);
        file_data.extend_from_slice(&nonce);
        file_data.extend_from_slice(&ciphertext);
        file_data.extend_from_slice(&tag);

        let _ = std::fs::create_dir_all(app_data_dir);
        std::fs::write(creds_path(app_data_dir), &file_data)
            .map_err(|e| e.to_string())
    }

    pub fn retrieve(app_data_dir: &PathBuf) -> Result<Option<super::Credential>, String> {
        let path = creds_path(app_data_dir);
        if !path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&path).map_err(|e| e.to_string())?;
        let min_len = AES_GCM_NONCE_SIZE + AES_GCM_TAG_SIZE;
        if data.len() < min_len {
            return Ok(None);
        }

        let nonce = &data[..AES_GCM_NONCE_SIZE];
        let tag   = &data[data.len() - AES_GCM_TAG_SIZE..];
        let mut ct = data[AES_GCM_NONCE_SIZE..data.len() - AES_GCM_TAG_SIZE].to_vec();

        let key = derive_key();
        let mut dec = Aes256GcmDecryption::new(&key, nonce, b"")
            .map_err(|e| format!("AES-GCM init: {:?}", e))?;
        dec.decrypt(&mut ct);
        if let Err(_) = dec.verify_tag(tag) {
            // Tag invalide → fichier corrompu ou machine-id modifié ; on supprime
            let _ = std::fs::remove_file(&path);
            return Ok(None);
        }

        let stored: Stored = serde_json::from_slice(&ct).map_err(|e| e.to_string())?;
        if stored.u.is_empty() {
            return Ok(None);
        }

        Ok(Some(super::Credential { username: stored.u, password: stored.p }))
    }

    pub fn delete(app_data_dir: &PathBuf) -> Result<(), String> {
        let path = creds_path(app_data_dir);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

// Commands
#[tauri::command]
pub async fn store_credential<R: Runtime>(
    app: tauri::AppHandle<R>,
    username: String,
    password: String,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        app.state::<KeystoreHandle<R>>()
            .0
            .run_mobile_plugin::<VoidResult>(
                "store",
                serde_json::json!({ "username": username, "password": password }),
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let store_pw = keyring::Entry::new(KEYRING_SERVICE, &username)
            .map_err(|e| e.to_string())?
            .set_password(&password);

        match store_pw {
            Ok(()) => {
                keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACTIVE_USER_KEY)
                    .map_err(|e| e.to_string())?
                    .set_password(&username)
                    .map_err(|e| e.to_string())
            }
            Err(ref e) if is_backend_unavailable(e) => {
                // Pas de keyring OS → fichier chiffré dans l'app data dir
                let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                file_store::store(&dir, &username, &password)
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

#[tauri::command]
pub async fn retrieve_credential<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<Credential>, String> {
    #[cfg(target_os = "android")]
    {
        #[derive(serde::Deserialize, Default)]
        struct MaybeCredential {
            username: Option<String>,
            password: Option<String>,
        }

        let c: MaybeCredential = app
            .state::<KeystoreHandle<R>>()
            .0
            .run_mobile_plugin("retrieve", serde_json::json!({}))
            .map_err(|e| e.to_string())?;

        match (c.username, c.password) {
            (Some(u), Some(p)) if !u.is_empty() => Ok(Some(Credential { username: u, password: p })),
            _ => Ok(None),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let username = match keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACTIVE_USER_KEY)
            .map_err(|e| e.to_string())?
            .get_password()
        {
            Ok(u) if !u.is_empty() => u,
            Err(keyring::Error::NoEntry) => {
                // Rien dans le keyring → peut-être dans le fichier chiffré
                let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                return file_store::retrieve(&dir);
            }
            Err(e) if is_backend_unavailable(&e) => {
                let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                return file_store::retrieve(&dir);
            }
            Err(e) => return Err(e.to_string()),
            _ => return Ok(None),
        };

        match keyring::Entry::new(KEYRING_SERVICE, &username)
            .map_err(|e| e.to_string())?
            .get_password()
        {
            Ok(p) => Ok(Some(Credential { username, password: p })),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) if is_backend_unavailable(&e) => {
                let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                file_store::retrieve(&dir)
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

#[tauri::command]
pub async fn delete_credential<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        app.state::<KeystoreHandle<R>>()
            .0
            .run_mobile_plugin::<VoidResult>("delete", serde_json::json!({}))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        // Supprime du keyring OS si présent
        let username = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACTIVE_USER_KEY)
            .map_err(|e| e.to_string())?
            .get_password();

        match username {
            Ok(u) if !u.is_empty() => {
                for key in [u.as_str(), KEYRING_ACTIVE_USER_KEY] {
                    match keyring::Entry::new(KEYRING_SERVICE, key)
                        .map_err(|e| e.to_string())?
                        .delete_credential()
                    {
                        Ok(()) | Err(keyring::Error::NoEntry) => {}
                        Err(e) if is_backend_unavailable(&e) => {}
                        Err(e) => return Err(e.to_string()),
                    }
                }
            }
            Err(e) if is_backend_unavailable(&e) => {}
            _ => {}
        }

        // Supprime aussi le fichier chiffré si existant (les deux peuvent coexister
        // si l'utilisateur a migré d'un système avec keyring vers un sans)
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        file_store::delete(&dir)
    }
}
