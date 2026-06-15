use serde::{Deserialize, Serialize};
use crate::utils::sanitizer::parser::{FileParser, FileInfo};

// Structures pour la communication avec le frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub file_type: String,
    pub size: usize,
    pub signature_valid: bool,
    pub extension_matches_signature: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SanitizeResponse {
    pub success: bool,
    pub message: String,
    pub sanitized_data: Option<Vec<u8>>,
    pub original_size: usize,
    pub sanitized_size: usize,
}

// Convertir FileInfo en structure sérialisable
impl From<FileInfo> for FileAnalysis {
    fn from(info: FileInfo) -> Self {
        FileAnalysis {
            file_type: format!("{:?}", info.file_type),
            size: info.size,
            signature_valid: info.signature_valid,
            extension_matches_signature: info.extension_matches_signature,
            error: None,
        }
    }
}

// Commande pour analyser un fichier
#[tauri::command]
pub async fn analyze_file(file_path: String, file_data: Vec<u8>) -> Result<FileAnalysis, String> {
    tokio::task::spawn_blocking(move || {
        let path = std::path::PathBuf::from(&file_path);
        match FileParser::analyze(path.as_path(), &file_data) {
            Ok(info) => Ok(info.into()),
            Err(e) => Ok(FileAnalysis {
                file_type: "Unknown".to_string(),
                size: file_data.len(),
                signature_valid: false,
                extension_matches_signature: false,
                error: Some(e.to_string()),
            }),
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))?
}

// Commande pour sanitizer un fichier
#[tauri::command]
pub async fn sanitize_file(file_path: String, file_data: Vec<u8>) -> Result<SanitizeResponse, String> {
    tokio::task::spawn_blocking(move || {
        let path = std::path::PathBuf::from(&file_path);
        let original_size = file_data.len();

        match FileParser::parse(path.as_path(), &file_data) {
            Ok(sanitized) => {
                let sanitized_size = sanitized.len();
                Ok(SanitizeResponse {
                    success: true,
                    message: format!("File sanitized successfully. Reduced from {} to {} bytes",
                                    original_size, sanitized_size),
                    sanitized_data: Some(sanitized),
                    original_size,
                    sanitized_size,
                })
            }
            Err(e) => Ok(SanitizeResponse {
                success: false,
                message: e.to_string(),
                sanitized_data: None,
                original_size,
                sanitized_size: 0,
            }),
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))?
}

// Commande pour sanitizer par type de fichier spécifique
#[tauri::command]
pub async fn sanitize_by_type(
    file_type: String,
    file_data: Vec<u8>
) -> Result<SanitizeResponse, String> {
    let original_size = file_data.len();

    // Vérification du type avant de passer au thread pool
    let file_type_lower = file_type.to_lowercase();
    if !matches!(file_type_lower.as_str(), "jpeg" | "jpg" | "png" | "mp3" | "mp4" | "wav" | "pdf") {
        return Ok(SanitizeResponse {
            success: false,
            message: format!("Unsupported file type: {}", file_type),
            sanitized_data: None,
            original_size,
            sanitized_size: 0,
        });
    }

    tokio::task::spawn_blocking(move || {
        let result = match file_type_lower.as_str() {
            "jpeg" | "jpg" => zenth_protect::sanitize_jpeg(&file_data),
            "png" => zenth_protect::sanitize_png(&file_data),
            "mp3" => zenth_protect::sanitize_mp3(&file_data),
            "mp4" => zenth_protect::sanitize_mp4(&file_data),
            "wav" => zenth_protect::sanitize_wav(&file_data),
            "pdf" => zenth_protect::sanitize_pdf(&file_data),
            _ => unreachable!(),
        };

        match result {
            Ok(sanitized) => {
                let sanitized_size = sanitized.len();
                Ok(SanitizeResponse {
                    success: true,
                    message: "File sanitized successfully".to_string(),
                    sanitized_data: Some(sanitized),
                    original_size,
                    sanitized_size,
                })
            }
            Err(e) => Ok(SanitizeResponse {
                success: false,
                message: e.to_string(),
                sanitized_data: None,
                original_size,
                sanitized_size: 0,
            }),
        }
    })
    .await
    .map_err(|e| format!("Thread pool error: {}", e))?
}

// Commande pour obtenir les formats supportés
#[tauri::command]
pub async fn get_supported_formats() -> Result<Vec<String>, String> {
    Ok(vec![
        "jpeg".to_string(),
        "jpg".to_string(),
        "png".to_string(),
        "mp3".to_string(),
        "mp4".to_string(),
        "wav".to_string(),
        "pdf".to_string(),
    ])
}
