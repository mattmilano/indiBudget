pub mod migrations;
pub mod repository;

use rusqlite::Connection;
use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database not initialized")]
    NotInitialized,
    #[error("Record not found")]
    NotFound,
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("indiBudget is already open in another window on this computer. Switch to that window, or close it before opening indiBudget again.")]
    AlreadyOpen,
    #[error("{0}")]
    Other(String),
}

pub type DbResult<T> = Result<T, DatabaseError>;

pub struct Database {
    connection: Mutex<Connection>,
    /// Held for as long as the database is open. See [`lock_beside`].
    _lock: Option<File>,
}

/// Claim the database for this window, through a lock file beside it.
///
/// Two windows on one file would each keep their own edit holds, news and
/// hosting state, so neither would know what the other was doing — the very
/// thing sharing exists to prevent. The lock is the operating system's, so it
/// lets go by itself when the program exits, however it exits: a crash never
/// leaves a stale lock that has to be cleared by hand.
fn lock_beside(path: &Path) -> DbResult<File> {
    let mut lock_path = path.as_os_str().to_owned();
    lock_path.push(".lock");
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(PathBuf::from(lock_path))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(DatabaseError::AlreadyOpen),
        Err(TryLockError::Error(e)) => Err(DatabaseError::Io(e)),
    }
}

impl Database {
    pub fn new(path: PathBuf) -> DbResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Before opening, so a second window never touches the file at all.
        let lock = lock_beside(&path)?;

        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;

        let db = Self {
            connection: Mutex::new(conn),
            _lock: Some(lock),
        };

        db.run_migrations()?;
        db.seed_default_data()?;

        Ok(db)
    }

    pub fn in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;

        let db = Self {
            connection: Mutex::new(conn),
            _lock: None,
        };

        db.run_migrations()?;
        db.seed_default_data()?;

        Ok(db)
    }

    fn run_migrations(&self) -> DbResult<()> {
        let conn = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        migrations::run_all(&conn)?;
        Ok(())
    }

    fn seed_default_data(&self) -> DbResult<()> {
        let conn = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // Ensure all default categories exist (use INSERT OR IGNORE to handle existing ones)
        // This allows new categories to be added in updates without breaking existing databases
        let categories = crate::models::category::get_default_categories();
        for cat in categories {
            conn.execute(
                "INSERT OR IGNORE INTO categories (id, name, category_type, color, icon, parent_id, is_system, is_active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    cat.id,
                    cat.name,
                    cat.category_type.as_str(),
                    cat.color,
                    cat.icon,
                    cat.parent_id,
                    cat.is_system,
                    cat.is_active,
                    cat.created_at.to_rfc3339(),
                    cat.updated_at.to_rfc3339(),
                ],
            )?;
        }

        Ok(())
    }

    pub fn with_connection<F, T>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&Connection) -> DbResult<T>,
    {
        let conn = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&conn)
    }

    /// Carry whatever sits in the write-ahead log into the database file.
    ///
    /// The budget is a plain SQLite file, which invites people to copy it by
    /// hand. A copy taken between checkpoints is missing whatever was still in
    /// the `-wal` file beside it, with nothing to say so. Called after every
    /// successful write and on close. PASSIVE never waits on a reader, and is
    /// free when there is nothing to carry; a checkpoint that cannot finish
    /// now simply finishes on a later one, so failure is not worth reporting.
    pub fn checkpoint(&self) {
        let _ = self.with_connection(|conn| {
            conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |_| Ok(()))?;
            Ok(())
        });
    }

    pub fn with_connection_mut<F, T>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&mut Connection) -> DbResult<T>,
    {
        let mut conn = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut conn)
    }
}

impl Drop for Database {
    /// Leave everything in the database file itself, so a copy taken after
    /// closing is complete.
    fn drop(&mut self) {
        self.checkpoint();
    }
}

pub fn get_database_path() -> PathBuf {
    // Use simple "indibudget" naming scheme across all platforms
    // On Linux: ~/.local/share/indibudget/
    // On macOS: ~/Library/Application Support/indibudget/
    // On Windows: C:\Users\<User>\AppData\Roaming\indibudget\

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
            PathBuf::from(data_home)
                .join("indibudget")
                .join("indibudget.db")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join(".local/share/indibudget")
                .join("indibudget.db")
        } else {
            PathBuf::from("indibudget.db")
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join("Library/Application Support/indibudget")
                .join("indibudget.db")
        } else {
            PathBuf::from("indibudget.db")
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            PathBuf::from(appdata)
                .join("indibudget")
                .join("indibudget.db")
        } else {
            PathBuf::from("indibudget.db")
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Fallback for other platforms
        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "indibudget") {
            proj_dirs.data_dir().join("indibudget.db")
        } else {
            PathBuf::from("indibudget.db")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("indibudget-db-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("indibudget.db")
    }

    #[test]
    fn a_second_window_on_the_same_file_is_refused_in_words() {
        let path = scratch("lock");
        let first = Database::new(path.clone()).unwrap();

        let err = Database::new(path.clone()).err().expect("the second open must be refused");
        assert!(matches!(err, DatabaseError::AlreadyOpen));
        assert!(err.to_string().contains("already open in another window"), "{err}");

        // Closing the first lets the next window in.
        drop(first);
        Database::new(path.clone()).expect("free once the first window closed");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_hand_copy_of_the_file_has_the_latest_write() {
        let path = scratch("checkpoint");
        let db = Database::new(path.clone()).unwrap();
        db.with_connection(|conn| {
            crate::database::repository::set_setting(conn, "probe", "written")
        })
        .unwrap();
        db.checkpoint();

        // Copy only the main file, as someone copying it by hand would, and
        // leave the write-ahead log behind.
        let copy = path.with_file_name("copy.db");
        std::fs::copy(&path, &copy).unwrap();
        let conn = Connection::open(&copy).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM app_settings WHERE key = 'probe'", [], |r| r.get(0))
            .expect("the copy is missing the write");
        assert_eq!(value, "written");

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
