use std::path::Path;
use zenth_protect::{
    sanitize_jpeg, sanitize_mp4, sanitize_mp3, sanitize_pdf, sanitize_png, sanitize_wav,
    Error, Result
};

/// Enum représentant les types de fichiers supportés
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Jpeg,
    Png,
    Mp3,
    Mp4,
    Wav,
    Pdf,
}

impl FileType {
    /// Détecte le type de fichier à partir de l'extension
    pub fn from_extension(path: &Path) -> Result<Self> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or(Error::UnsupportedFormat("No extension found"))?
            .to_lowercase();

        match extension.as_str() {
            "jpg" | "jpeg" => Ok(FileType::Jpeg),
            "png" => Ok(FileType::Png),
            "mp3" => Ok(FileType::Mp3),
            "mp4" => Ok(FileType::Mp4),
            "wav" => Ok(FileType::Wav),
            "pdf" => Ok(FileType::Pdf),
            _ => Err(Error::UnsupportedFormat("Unknown file extension")),
        }
    }

    /// Détecte le type de fichier à partir des magic bytes (signature)
    pub fn from_signature(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(Error::InvalidSignature("File too small"));
        }

        // JPEG: FF D8 FF
        if data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
            return Ok(FileType::Jpeg);
        }

        // PNG: 89 50 4E 47 0D 0A 1A 0A
        if data.len() >= 8
            && data[0] == 0x89
            && &data[1..4] == b"PNG"
            && data[4] == 0x0D
            && data[5] == 0x0A
            && data[6] == 0x1A
            && data[7] == 0x0A
        {
            return Ok(FileType::Png);
        }

        // MP3: ID3 ou FF FB/FF FA (MPEG frame sync)
        if (data.len() >= 3 && &data[0..3] == b"ID3")
            || (data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xE0 == 0xE0))
        {
            return Ok(FileType::Mp3);
        }

        // MP4: ftyp signature (offset 4)
        if data.len() >= 8 && &data[4..8] == b"ftyp" {
            return Ok(FileType::Mp4);
        }

        // WAV: RIFF....WAVE
        if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
            return Ok(FileType::Wav);
        }

        // PDF: %PDF
        if data.len() >= 4 && &data[0..4] == b"%PDF" {
            return Ok(FileType::Pdf);
        }

        Err(Error::InvalidSignature("Unknown file signature"))
    }
}

/// Parser principal pour analyser et sanitizer les fichiers
pub struct FileParser;

impl FileParser {
    /// Parse et sanitize un fichier en détectant son type par l'extension
    pub fn parse_by_extension(path: &Path, data: &[u8]) -> Result<Vec<u8>> {
        let file_type = FileType::from_extension(path)?;
        Self::sanitize_by_type(file_type, data)
    }

    /// Parse et sanitize un fichier en détectant son type par la signature
    pub fn parse_by_signature(data: &[u8]) -> Result<Vec<u8>> {
        let file_type = FileType::from_signature(data)?;
        Self::sanitize_by_type(file_type, data)
    }

    /// Parse et sanitize un fichier (essaie d'abord l'extension, puis la signature)
    pub fn parse(path: &Path, data: &[u8]) -> Result<Vec<u8>> {
        // Essaie d'abord par extension
        if let Ok(file_type) = FileType::from_extension(path) {
            // Vérifie que la signature correspond à l'extension
            if let Ok(sig_type) = FileType::from_signature(data) {
                if file_type != sig_type {
                    return Err(Error::InvalidSignature(
                        "File signature doesn't match extension",
                    ));
                }
            }
            return Self::sanitize_by_type(file_type, data);
        }

        // Sinon, essaie par signature
        Self::parse_by_signature(data)
    }

    /// Sanitize les données selon le type de fichier
    /// UTILISE LES FONCTIONS DE ZENTH_PROTECT
    fn sanitize_by_type(file_type: FileType, data: &[u8]) -> Result<Vec<u8>> {
        match file_type {
            FileType::Jpeg => sanitize_jpeg(data),
            FileType::Png => sanitize_png(data),
            FileType::Mp3 => sanitize_mp3(data),
            FileType::Mp4 => sanitize_mp4(data),
            FileType::Wav => sanitize_wav(data),
            FileType::Pdf => sanitize_pdf(data),
        }
    }

    /// Analyse un fichier et retourne des informations sans le sanitizer
    pub fn analyze(path: &Path, data: &[u8]) -> Result<FileInfo> {
        let file_type = FileType::from_extension(path)
            .or_else(|_| FileType::from_signature(data))?;

        let signature_valid = FileType::from_signature(data)
            .map(|sig| sig == file_type)
            .unwrap_or(false);

        Ok(FileInfo {
            file_type,
            size: data.len(),
            signature_valid,
            extension_matches_signature: signature_valid,
        })
    }
}

/// Informations sur un fichier analysé
#[derive(Debug)]
pub struct FileInfo {
    pub file_type: FileType,
    pub size: usize,
    pub signature_valid: bool,
    pub extension_matches_signature: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_file_type_from_extension() {
        let path = PathBuf::from("test.mp3");
        assert_eq!(FileType::from_extension(&path).unwrap(), FileType::Mp3);

        let path = PathBuf::from("test.jpg");
        assert_eq!(FileType::from_extension(&path).unwrap(), FileType::Jpeg);

        let path = PathBuf::from("test.JPEG");
        assert_eq!(FileType::from_extension(&path).unwrap(), FileType::Jpeg);
    }

    #[test]
    fn test_file_type_from_signature_mp3() {
        let data_id3 = b"ID3\x04\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            FileType::from_signature(data_id3).unwrap(),
            FileType::Mp3
        );

        let data_frame = [0xFF, 0xFB, 0x90, 0x00];
        assert_eq!(
            FileType::from_signature(&data_frame).unwrap(),
            FileType::Mp3
        );
    }

    #[test]
    fn test_file_type_from_signature_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(FileType::from_signature(&data).unwrap(), FileType::Jpeg);
    }

    #[test]
    fn test_file_type_from_signature_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(FileType::from_signature(&data).unwrap(), FileType::Png);
    }

    #[test]
    fn test_file_type_from_signature_pdf() {
        let data = b"%PDF-1.4\n";
        assert_eq!(FileType::from_signature(data).unwrap(), FileType::Pdf);
    }

    #[test]
    fn test_parse_validates_signature() {
        let path = PathBuf::from("test.mp3");
        let invalid_data = b"NOT AN MP3";
        assert!(FileParser::parse(&path, invalid_data).is_err());
    }

    #[test]
    fn test_analyze() {
        let path = PathBuf::from("test.mp3");
        let data = [0xFF, 0xFB, 0x90, 0x00];
        let info = FileParser::analyze(&path, &data).unwrap();
        assert_eq!(info.file_type, FileType::Mp3);
        assert_eq!(info.size, 4);
        assert!(info.signature_valid);
    }
}
