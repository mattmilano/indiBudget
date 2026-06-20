//! Tier 2 — Backup / restore round-trip (handoff Section 4 #7).
//!
//! export → fresh DB → import must reproduce accounts/transactions/budgets/
//! goals exactly. Version-mismatch backups must be rejected, not partially
//! restored.

mod common;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use common::*;
use indibudget_lib::models::*;
use indibudget_lib::services::backup::{export_backup_to_file, get_backup_info, import_backup_from_file};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_file(name: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("indibudget_backup_{nanos}_{n}"));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join(name)
}

#[test]
fn export_then_import_into_fresh_db_reproduces_data() {
    // Seed a source DB through the real repositories.
    let src = db();
    let groceries = new_category(&src, "Groceries", CategoryType::Expense, "#00aa00");
    let checking = new_account(&src, "Checking", AccountType::Checking, "1500.00");
    let savings = new_account(&src, "Savings", AccountType::Savings, "8000.00");
    add_expense(&src, &checking, "85.40", "2026-06-03", "Market", Some(&groceries));
    add_income(&src, &checking, "3000.00", "2026-06-01", "Salary", None);
    add_transfer(&src, &checking, &savings, "500.00", "2026-06-02", "To savings");

    let src_txns = all_transactions(&src);
    let src_accounts = src
        .with_connection(|conn| indibudget_lib::database::repository::get_all_accounts(conn))
        .expect("accounts");

    // Export to a file.
    let path = temp_file("backup.json");
    let meta = export_backup_to_file(&src, &path).expect("export");
    assert_eq!(meta.account_count, 2);
    assert_eq!(meta.transaction_count, src_txns.len());

    // Import into a brand-new DB.
    let dst = db();
    import_backup_from_file(&dst, &path).expect("import");

    // Accounts reproduced exactly (by id + balance).
    let dst_accounts = dst
        .with_connection(|conn| indibudget_lib::database::repository::get_all_accounts(conn))
        .expect("dst accounts");
    assert_eq!(dst_accounts.len(), src_accounts.len(), "account count preserved");
    for src_acc in &src_accounts {
        let found = dst_accounts
            .iter()
            .find(|a| a.id == src_acc.id)
            .unwrap_or_else(|| panic!("account {} missing after restore", src_acc.name));
        assert_eq!(found.balance, src_acc.balance, "balance preserved for {}", src_acc.name);
        assert_eq!(found.account_type, src_acc.account_type);
    }

    // Transactions reproduced exactly (by id, amount, type).
    let dst_txns = all_transactions(&dst);
    assert_eq!(dst_txns.len(), src_txns.len(), "transaction count preserved");
    for src_tx in &src_txns {
        let found = dst_txns
            .iter()
            .find(|t| t.id == src_tx.id)
            .expect("transaction missing after restore");
        assert_eq!(found.amount, src_tx.amount);
        assert_eq!(found.transaction_type, src_tx.transaction_type);
        assert_eq!(found.date, src_tx.date);
        assert_eq!(found.category_id, src_tx.category_id);
    }
}

#[test]
fn version_mismatch_backup_is_rejected() {
    // Hand-craft a backup file with a bogus version; import must fail cleanly.
    let path = temp_file("bad_version.json");
    let bogus = r#"{
        "metadata": {
            "version": "9.9",
            "created_at": "2026-06-01T00:00:00Z",
            "app_version": "1.0.0",
            "account_count": 0,
            "transaction_count": 0,
            "category_count": 0
        },
        "accounts": [], "transactions": [], "categories": [],
        "budgets": [], "recurring": [], "goals": [], "category_rules": []
    }"#;
    std::fs::write(&path, bogus).expect("write bogus backup");

    // get_backup_info should still read metadata.
    let info = get_backup_info(&path).expect("read metadata");
    assert_eq!(info.version, "9.9");

    // Import must be rejected (no partial restore).
    let dst = db();
    let before = all_transactions(&dst).len();
    assert!(
        import_backup_from_file(&dst, &path).is_err(),
        "version-mismatch backup must be rejected"
    );
    assert_eq!(all_transactions(&dst).len(), before, "no rows imported on rejected backup");
}
