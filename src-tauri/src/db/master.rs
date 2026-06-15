use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::fs;

use crate::db::crypto::{generate_salt, hash_username};
use crate::db::error::DbError;
use crate::db::paths::{get_db_dir, master_db_path, user_db_path};

#[derive(Debug, Clone)]
pub struct UserEntry {
    /// Hash du nom d'utilisateur (identifiant de la BDD utilisateur)
    pub name_hash: String,
    /// Salt pour la derivation de cle Argon2id
    pub salt: Vec<u8>,
}

pub struct MasterDb {
    conn: Connection,
}

impl MasterDb {
    pub fn open() -> Result<Self, DbError> {
        // S'assurer que le répertoire existe
        fs::create_dir_all(get_db_dir())?;
        let db_path = master_db_path();
        let conn = Connection::open(&db_path)?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    /// Initialise le schema de la base de donnees
    fn init(&self) -> Result<(), DbError> {
        let migrations = Migrations::new(vec![
            M::up(
                "CREATE TABLE IF NOT EXISTS users (
                    name_hash TEXT PRIMARY KEY,
                    salt BLOB NOT NULL
                );"
            ),
        ]);

        let mut conn = Connection::open(master_db_path())?;
        migrations.to_latest(&mut conn)?;

        Ok(())
    }

    /// Enregistre un nouvel utilisateur
    /// Genere automatiquement un salt aleatoire
    /// Retourne le salt genere
    pub fn register_user(&self, username: &str) -> Result<UserEntry, DbError> {
        let name_hash = hash_username(username);

        if self.user_exists(&name_hash)? {
            return Err(DbError::UserAlreadyExists(username.to_string()));
        }

        let salt = generate_salt();

        self.conn.execute(
            "INSERT INTO users (name_hash, salt) VALUES (?1, ?2)",
            rusqlite::params![name_hash, salt.as_slice()],
        )?;

        Ok(UserEntry {
            name_hash,
            salt: salt.to_vec(),
        })
    }

    pub fn get_user(&self, username: &str) -> Result<UserEntry, DbError> {
        let name_hash = hash_username(username);
        self.get_user_by_hash(&name_hash)
    }


    pub fn get_user_sync(&self, username: &str) -> Result<UserEntry, DbError> {
        let name_hash = hash_username(username);
        self.get_user_by_hash(&name_hash)
    }

    pub fn get_user_by_hash(&self, name_hash: &str) -> Result<UserEntry, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT name_hash, salt FROM users WHERE name_hash = ?1"
        )?;

        let entry = stmt.query_row([name_hash], |row| {
            Ok(UserEntry {
                name_hash: row.get(0)?,
                salt: row.get(1)?,
            })
        }).map_err(|_| DbError::UserNotFound(name_hash.to_string()))?;

        Ok(entry)
    }

    pub fn user_exists(&self, name_hash: &str) -> Result<bool, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*) FROM users WHERE name_hash = ?1"
        )?;

        let count: i64 = stmt.query_row([name_hash], |row| row.get(0))?;
        Ok(count > 0)
    }

    pub fn user_exists_by_name(&self, username: &str) -> Result<bool, DbError> {
        let name_hash = hash_username(username);
        self.user_exists(&name_hash)
    }

    pub fn delete_user(&self, username: &str) -> Result<(), DbError> {
        let name_hash = hash_username(username);

        let rows_affected = self.conn.execute(
            "DELETE FROM users WHERE name_hash = ?1",
            [&name_hash],
        )?;

        if rows_affected == 0 {
            return Err(DbError::UserNotFound(username.to_string()));
        }

        let db_path = user_db_path(&name_hash);
        if db_path.exists() {
            fs::remove_file(db_path)?;
        }

        Ok(())
    }

    /// Renomme un utilisateur : met à jour le master index et déplace le fichier DB.
    /// Utilisé après le jumelage d'appareils pour adopter le nom de l'appareil de confiance.
    pub fn rename_user(&self, old_username: &str, new_username: &str) -> Result<(), DbError> {
        let old_hash = hash_username(old_username);
        let new_hash = hash_username(new_username);

        if self.user_exists(&new_hash)? {
            return Err(DbError::UserAlreadyExists(new_username.to_string()));
        }

        let entry = self.get_user_by_hash(&old_hash)?;

        self.conn.execute(
            "INSERT INTO users (name_hash, salt) VALUES (?1, ?2)",
            rusqlite::params![new_hash, entry.salt],
        )?;

        let old_path = user_db_path(&old_hash);
        let new_path = user_db_path(&new_hash);
        if old_path.exists() {
            fs::rename(&old_path, &new_path)?;
        }

        self.conn.execute(
            "DELETE FROM users WHERE name_hash = ?1",
            [&old_hash],
        )?;

        Ok(())
    }

    /// Supprime uniquement l'entrée dans le master index, sans toucher aux fichiers.
    /// Utilisé par la commande wipe qui gère elle-même la suppression sécurisée des fichiers.
    pub fn remove_entry_by_hash(&self, name_hash: &str) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM users WHERE name_hash = ?1",
            [name_hash],
        )?;
        Ok(())
    }

    pub fn list_users(&self) -> Result<Vec<UserEntry>, DbError> {
        let mut stmt = self.conn.prepare("SELECT name_hash, salt FROM users")?;

        let users = stmt.query_map([], |row| {
            Ok(UserEntry {
                name_hash: row.get(0)?,
                salt: row.get(1)?,
            })
        })?;

        let mut result = Vec::new();
        for user in users {
            result.push(user?);
        }

        Ok(result)
    }

    pub fn db_path() -> std::path::PathBuf {
        master_db_path()
    }

    pub fn user_db_path(name_hash: &str) -> std::path::PathBuf {
        user_db_path(name_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use crate::db::crypto::SALT_SIZE;

    fn setup_test_db() -> MasterDb {
        let _ = fs::remove_file(MasterDb::db_path());
        MasterDb::open().unwrap()
    }

    #[test]
    fn test_register_and_get_user() {
        let db = setup_test_db();

        let entry = db.register_user("alice").unwrap();
        assert_eq!(entry.salt.len(), SALT_SIZE);

        let retrieved = db.get_user("alice").unwrap();
        assert_eq!(entry.name_hash, retrieved.name_hash);
        assert_eq!(entry.salt, retrieved.salt);
    }

    #[test]
    fn test_duplicate_user() {
        let db = setup_test_db();

        db.register_user("bob").unwrap();
        let result = db.register_user("bob");

        assert!(matches!(result, Err(DbError::UserAlreadyExists(_))));
    }

    #[test]
    fn test_user_exists() {
        let db = setup_test_db();

        assert!(!db.user_exists_by_name("charlie").unwrap());
        db.register_user("charlie").unwrap();
        assert!(db.user_exists_by_name("charlie").unwrap());
    }
}
