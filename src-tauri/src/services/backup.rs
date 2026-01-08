use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::database::{repository, Database};

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid backup file: {0}")]
    InvalidBackup(String),
    #[error("Version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: String, got: String },
}

const BACKUP_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub version: String,
    pub created_at: String,
    pub app_version: String,
    pub account_count: usize,
    pub transaction_count: usize,
    pub category_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub metadata: BackupMetadata,
    pub accounts: Vec<serde_json::Value>,
    pub transactions: Vec<serde_json::Value>,
    pub categories: Vec<serde_json::Value>,
    pub budgets: Vec<serde_json::Value>,
    pub recurring: Vec<serde_json::Value>,
    pub goals: Vec<serde_json::Value>,
    pub category_rules: Vec<serde_json::Value>,
}

pub fn create_backup(db: &Database) -> Result<BackupData, BackupError> {
    db.with_connection(|conn| {
        let accounts = repository::get_all_accounts(conn)
            .map_err(|e| BackupError::Database(e.to_string()))?;
        let transactions = repository::get_transactions(
            conn,
            &crate::models::TransactionFilter::default(),
        )
        .map_err(|e| BackupError::Database(e.to_string()))?;
        let categories = repository::get_all_categories(conn)
            .map_err(|e| BackupError::Database(e.to_string()))?;
        let budgets = repository::get_all_budgets(conn)
            .map_err(|e| BackupError::Database(e.to_string()))?;
        let recurring = repository::get_all_recurring(conn)
            .map_err(|e| BackupError::Database(e.to_string()))?;
        let goals = repository::get_all_goals(conn)
            .map_err(|e| BackupError::Database(e.to_string()))?;
        let category_rules = repository::get_category_rules(conn)
            .map_err(|e| BackupError::Database(e.to_string()))?;

        let metadata = BackupMetadata {
            version: BACKUP_VERSION.to_string(),
            created_at: Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            account_count: accounts.len(),
            transaction_count: transactions.len(),
            category_count: categories.len(),
        };

        Ok(BackupData {
            metadata,
            accounts: accounts
                .iter()
                .map(|a| serde_json::to_value(a).unwrap())
                .collect(),
            transactions: transactions
                .iter()
                .map(|t| serde_json::to_value(t).unwrap())
                .collect(),
            categories: categories
                .iter()
                .map(|c| serde_json::to_value(c).unwrap())
                .collect(),
            budgets: budgets
                .iter()
                .map(|b| serde_json::to_value(b).unwrap())
                .collect(),
            recurring: recurring
                .iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect(),
            goals: goals
                .iter()
                .map(|g| serde_json::to_value(g).unwrap())
                .collect(),
            category_rules: category_rules
                .iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect(),
        })
    })
}

pub fn export_backup_to_file(db: &Database, path: &Path) -> Result<BackupMetadata, BackupError> {
    let backup = create_backup(db)?;
    let metadata = backup.metadata.clone();

    let json = serde_json::to_string_pretty(&backup)?;
    fs::write(path, json)?;

    Ok(metadata)
}

pub fn import_backup_from_file(db: &Database, path: &Path) -> Result<BackupMetadata, BackupError> {
    let content = fs::read_to_string(path)?;
    let backup: BackupData = serde_json::from_str(&content)?;

    // Validate version
    if backup.metadata.version != BACKUP_VERSION {
        return Err(BackupError::VersionMismatch {
            expected: BACKUP_VERSION.to_string(),
            got: backup.metadata.version,
        });
    }

    let metadata = backup.metadata.clone();

    db.with_connection(|conn| {
        // Import categories first (other data may reference them)
        for cat_json in &backup.categories {
            if let Ok(cat) = serde_json::from_value::<crate::models::Category>(cat_json.clone()) {
                let _ = repository::create_category(conn, &cat);
            }
        }

        // Import accounts
        for acc_json in &backup.accounts {
            if let Ok(acc) = serde_json::from_value::<crate::models::Account>(acc_json.clone()) {
                let _ = repository::create_account(conn, &acc);
            }
        }

        // Import budgets
        for budget_json in &backup.budgets {
            if let Ok(budget) = serde_json::from_value::<crate::models::Budget>(budget_json.clone())
            {
                let _ = repository::create_budget(conn, &budget);
            }
        }

        // Import recurring transactions
        for rec_json in &backup.recurring {
            if let Ok(rec) =
                serde_json::from_value::<crate::models::RecurringTransaction>(rec_json.clone())
            {
                let _ = repository::create_recurring(conn, &rec);
            }
        }

        // Import goals
        for goal_json in &backup.goals {
            if let Ok(goal) =
                serde_json::from_value::<crate::models::SavingsGoal>(goal_json.clone())
            {
                let _ = repository::create_goal(conn, &goal);
            }
        }

        // Import transactions
        for tx_json in &backup.transactions {
            if let Ok(tx) = serde_json::from_value::<crate::models::Transaction>(tx_json.clone()) {
                let _ = repository::create_transaction(conn, &tx);
            }
        }

        // Import category rules
        for rule_json in &backup.category_rules {
            if let Ok(rule) =
                serde_json::from_value::<crate::models::CategoryRule>(rule_json.clone())
            {
                let _ = repository::create_category_rule(conn, &rule);
            }
        }

        Ok(metadata)
    })
}

pub fn get_backup_info(path: &Path) -> Result<BackupMetadata, BackupError> {
    let content = fs::read_to_string(path)?;
    let backup: BackupData = serde_json::from_str(&content)?;
    Ok(backup.metadata)
}

pub fn get_default_backup_path() -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("indibudget_backup_{}.json", timestamp);

    directories::UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(filename)
}
