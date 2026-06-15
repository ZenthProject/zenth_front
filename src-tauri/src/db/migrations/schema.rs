use rusqlite::{Connection, Result};
use rusqlite_migration::{Migrations, M};

pub fn database_migration(db_name: String, sqlcipher_key: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = Connection::open(db_name)?;
    conn.pragma_update(None, "key", sqlcipher_key)?;

    let migrations = Migrations::new(vec![
        M::up(
            "
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                password TEXT NOT NULL,
            );
            "
        ),
        M::up(
            "
            CREATE TABLE IF NOT EXISTS friends (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                username TEXT NOT NULL,
                hashid TEXT NOT NULL,
                hashmac TEXT NOT NULL,
                no_logs_local BOOLEAN DEFAULT 0,
                no_logs_remote BOOLEAN DEFAULT 0,
                FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
            );
            "
        ),
        M::up(
            "
            CREATE TABLE IF NOT EXISTS message (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                friend_id INTEGER NOT NULL,
                sender TEXT NOT NULL,
                datetime TEXT NOT NULL,
                message_type TEXT NOT NULL,
                message BLOB NOT NULL,
                filename TEXT,
                mime_type TEXT,
                FOREIGN KEY(friend_id) REFERENCES friends(id) ON DELETE CASCADE
            );
            "
        ),
    ]);

    migrations.to_latest(&mut conn)?;

    println!("Migration terminée avec succès.");
    Ok(())
}
