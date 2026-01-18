pub mod migrations;
pub mod repository;

use rusqlite::Connection;
use std::path::PathBuf;
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
    #[error("{0}")]
    Other(String),
}

pub type DbResult<T> = Result<T, DatabaseError>;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn new(path: PathBuf) -> DbResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Self {
            connection: Mutex::new(conn),
        };

        db.run_migrations()?;
        db.seed_default_data()?;

        Ok(db)
    }

    pub fn in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Self {
            connection: Mutex::new(conn),
        };

        db.run_migrations()?;
        db.seed_default_data()?;

        Ok(db)
    }

    fn run_migrations(&self) -> DbResult<()> {
        let conn = self.connection.lock().unwrap();
        migrations::run_all(&conn)?;
        Ok(())
    }

    fn seed_default_data(&self) -> DbResult<()> {
        let conn = self.connection.lock().unwrap();

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
        let conn = self.connection.lock().unwrap();
        f(&conn)
    }

    pub fn with_connection_mut<F, T>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&mut Connection) -> DbResult<T>,
    {
        let mut conn = self.connection.lock().unwrap();
        f(&mut conn)
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
            PathBuf::from(data_home).join("indibudget").join("indibudget.db")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".local/share/indibudget").join("indibudget.db")
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
            PathBuf::from(appdata).join("indibudget").join("indibudget.db")
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
