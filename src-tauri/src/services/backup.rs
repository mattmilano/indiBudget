use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::database::{repository, Database, DatabaseError};

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

impl From<DatabaseError> for BackupError {
    fn from(e: DatabaseError) -> Self {
        BackupError::Database(e.to_string())
    }
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
    let accounts = db.with_connection(|conn| repository::get_all_accounts(conn))?;
    let transactions = db.with_connection(|conn| {
        repository::get_transactions(conn, &crate::models::TransactionFilter::default())
    })?;
    let categories = db.with_connection(|conn| repository::get_all_categories(conn))?;
    let budgets = db.with_connection(|conn| repository::get_all_budgets(conn))?;
    let recurring = db.with_connection(|conn| repository::get_all_recurring(conn))?;
    let goals = db.with_connection(|conn| repository::get_all_goals(conn))?;
    let category_rules = db.with_connection(|conn| repository::get_category_rules(conn))?;

    let metadata = BackupMetadata {
        version: BACKUP_VERSION.to_string(),
        created_at: Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        account_count: accounts.len(),
        transaction_count: transactions.len(),
        category_count: categories.len(),
    };

    // Serialize each collection, propagating any serialization errors
    let accounts_json: Result<Vec<_>, _> = accounts.iter().map(serde_json::to_value).collect();
    let transactions_json: Result<Vec<_>, _> =
        transactions.iter().map(serde_json::to_value).collect();
    let categories_json: Result<Vec<_>, _> = categories.iter().map(serde_json::to_value).collect();
    let budgets_json: Result<Vec<_>, _> = budgets.iter().map(serde_json::to_value).collect();
    let recurring_json: Result<Vec<_>, _> = recurring.iter().map(serde_json::to_value).collect();
    let goals_json: Result<Vec<_>, _> = goals.iter().map(serde_json::to_value).collect();
    let category_rules_json: Result<Vec<_>, _> =
        category_rules.iter().map(serde_json::to_value).collect();

    Ok(BackupData {
        metadata,
        accounts: accounts_json?,
        transactions: transactions_json?,
        categories: categories_json?,
        budgets: budgets_json?,
        recurring: recurring_json?,
        goals: goals_json?,
        category_rules: category_rules_json?,
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

    let result = db.with_connection(|conn| {
        // Import categories first (other data may reference them)
        for cat_json in &backup.categories {
            if let Ok(cat) = serde_json::from_value::<crate::models::Category>(cat_json.clone()) {
                let _ = repository::create_category(conn, &cat);
            }
        }

        // Import accounts
        // Handle backwards compatibility: old backups have "balance" but not "starting_balance"
        for acc_json in &backup.accounts {
            if let Ok(mut acc) = serde_json::from_value::<crate::models::Account>(acc_json.clone()) {
                // If this is an old backup with balance but no starting_balance, use balance as starting_balance
                if acc.starting_balance == rust_decimal::Decimal::ZERO && acc.balance != rust_decimal::Decimal::ZERO {
                    acc.starting_balance = acc.balance;
                }
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

        Ok(())
    });

    result.map_err(BackupError::from)?;
    Ok(metadata)
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
