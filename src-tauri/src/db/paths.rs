use std::path::PathBuf;
use std::sync::OnceLock;
use std::fs;

/// Nom du dossier de base de données
const DB_FOLDER_NAME: &str = "database";

/// Cache du chemin de base de données (calculé une seule fois)
static DB_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Initialise le répertoire de base de données avec un chemin fourni (ex: app_data_dir de Tauri).
/// Doit être appelé avant tout accès à get_db_dir().
pub fn init_db_dir(base: PathBuf) {
    let db_path = base.join(DB_FOLDER_NAME);
    let _ = fs::create_dir_all(&db_path);
    let _ = DB_DIR.set(db_path);
}

/// Retourne le chemin absolu du répertoire de base de données.
///
/// En mode développement (cargo tauri dev), utilise le répertoire src-tauri/database/
/// En mode production, utilise le répertoire à côté de l'exécutable
pub fn get_db_dir() -> &'static PathBuf {
    DB_DIR.get_or_init(|| {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let exe_dir_str = exe_dir.to_string_lossy();
                if exe_dir_str.contains("target/debug") || exe_dir_str.contains("target/release") {
                    let mut dev_path = exe_dir.to_path_buf();
                    dev_path.pop();
                    dev_path.pop();
                    dev_path.push(DB_FOLDER_NAME);
                    if dev_path.exists() || fs::create_dir_all(&dev_path).is_ok() {
                        return dev_path;
                    }
                }
                let prod_path = exe_dir.join(DB_FOLDER_NAME);
                if prod_path.exists() || fs::create_dir_all(&prod_path).is_ok() {
                    return prod_path;
                }
            }
        }

        let fallback = PathBuf::from(DB_FOLDER_NAME);

        let _ = fs::create_dir_all(&fallback);

        fallback
    })
}

/// Retourne le chemin du fichier master.db
pub fn master_db_path() -> PathBuf {
    get_db_dir().join("master.db")
}

/// Retourne le chemin d'une base de données utilisateur
pub fn user_db_path(name_hash: &str) -> PathBuf {
    get_db_dir().join(format!("{}.db", name_hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_db_dir_not_empty() {
        let dir = get_db_dir();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_paths_end_with_db() {
        assert!(master_db_path().to_string_lossy().ends_with("master.db"));
        assert!(user_db_path("abc123").to_string_lossy().ends_with("abc123.db"));
    }
}
