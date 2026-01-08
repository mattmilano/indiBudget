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

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE is_system = 1",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            let categories = crate::models::category::get_default_categories();
            for cat in categories {
                conn.execute(
                    "INSERT INTO categories (id, name, category_type, color, icon, parent_id, is_system, is_active, created_at, updated_at)
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
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "indibudget", "indiBudget") {
        proj_dirs.data_dir().join("indibudget.db")
    } else {
        PathBuf::from("indibudget.db")
    }
}
