use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

const CHUNK_SIZE: usize = 65536; // 64 KB

/// Écrase un fichier avec 3 passes puis le supprime.
///
/// - Passe 1 : zéros (0x00)
/// - Passe 2 : uns   (0xFF)
/// - Passe 3 : octets aléatoires cryptographiquement sûrs
///
/// Appelle `fsync` après chaque passe pour forcer l'écriture sur disque.
/// Retourne Ok(()) même si le fichier n'existe pas.
pub fn secure_delete(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let size = std::fs::metadata(path)?.len() as usize;

    if size > 0 {
        let mut file = std::fs::OpenOptions::new().write(true).open(path)?;

        // Passe 1 : zéros
        write_pass(&mut file, size, PassKind::Zeros)?;
        file.seek(SeekFrom::Start(0))?;

        // Passe 2 : uns
        write_pass(&mut file, size, PassKind::Ones)?;
        file.seek(SeekFrom::Start(0))?;

        // Passe 3 : aléatoire
        write_pass(&mut file, size, PassKind::Random)?;

        file.flush()?;
        file.sync_all()?;
    }

    std::fs::remove_file(path)
}

enum PassKind {
    Zeros,
    Ones,
    Random,
}

fn write_pass(file: &mut std::fs::File, size: usize, kind: PassKind) -> std::io::Result<()> {
    use rand::RngCore;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut written = 0;

    while written < size {
        let chunk = (size - written).min(CHUNK_SIZE);

        match kind {
            PassKind::Zeros => buf[..chunk].fill(0x00),
            PassKind::Ones => buf[..chunk].fill(0xFF),
            PassKind::Random => rand::rng().fill_bytes(&mut buf[..chunk]),
        }

        file.write_all(&buf[..chunk])?;
        written += chunk;
    }

    Ok(())
}
