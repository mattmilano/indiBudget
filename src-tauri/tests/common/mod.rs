//! Shared test harness for indiBudget integration tests.
//!
//! Philosophy (from the testing handoff):
//!   * Drive the REAL repositories/services against an in-memory SQLite DB.
//!   * Hand-derive every expected value; never read it back from the app.
//!   * Assert invariants after edits/deletes, not just the happy path.
//!
//! These helpers seed data through the same code paths the app uses
//! (`repository::*`), so integration bugs (bad SQL, wrong gating) surface here.

#![allow(dead_code)] // helpers are shared across multiple test files

use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::str::FromStr;

use indibudget_lib::database::{repository, Database};
use indibudget_lib::models::*;

/// A fresh, fully-migrated, seeded in-memory database.
pub fn db() -> Database {
    Database::in_memory().expect("in-memory db should initialize")
}

/// Parse a decimal from a string literal (e.g. `dec("12.34")`).
pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal")
}

/// Parse a `YYYY-MM-DD` date literal.
pub fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("valid YYYY-MM-DD date literal")
}

/// Create an account of the given type with a starting (stored) balance.
/// Returns the account id.
///
/// NOTE: indiBudget stores `balance` as a manually-maintained field; creating
/// transactions does NOT mutate it. Tests treat this stored value as-is.
pub fn new_account(db: &Database, name: &str, account_type: AccountType, balance: &str) -> String {
    let mut acct = Account::new(name.to_string(), account_type);
    acct.balance = dec(balance);
    let id = acct.id.clone();
    db.with_connection(|conn| repository::create_account(conn, &acct))
        .expect("create account");
    id
}

/// Create a custom (non-system) expense/income category. Returns its id.
pub fn new_category(db: &Database, name: &str, category_type: CategoryType, color: &str) -> String {
    let mut cat = Category::new(name.to_string(), category_type, color.to_string());
    let id = cat.id.clone();
    // Ensure it's treated as a user category, active.
    cat.is_system = false;
    cat.is_active = true;
    db.with_connection(|conn| repository::create_category(conn, &cat))
        .expect("create category");
    id
}

/// Insert a transaction with explicit type/category and return its id.
fn insert_tx(
    db: &Database,
    account_id: &str,
    tx_type: TransactionType,
    amount: &str,
    on: &str,
    description: &str,
    category_id: Option<&str>,
) -> String {
    let mut tx = Transaction::new(
        account_id.to_string(),
        tx_type,
        dec(amount),
        date(on),
        description.to_string(),
    );
    tx.category_id = category_id.map(|s| s.to_string());
    let id = tx.id.clone();
    db.with_connection(|conn| repository::create_transaction(conn, &tx))
        .expect("create transaction");
    id
}

/// Add an expense transaction. Returns its id.
pub fn add_expense(
    db: &Database,
    account_id: &str,
    amount: &str,
    on: &str,
    description: &str,
    category_id: Option<&str>,
) -> String {
    insert_tx(
        db,
        account_id,
        TransactionType::Expense,
        amount,
        on,
        description,
        category_id,
    )
}

/// Add an income transaction. Returns its id.
pub fn add_income(
    db: &Database,
    account_id: &str,
    amount: &str,
    on: &str,
    description: &str,
    category_id: Option<&str>,
) -> String {
    insert_tx(
        db,
        account_id,
        TransactionType::Income,
        amount,
        on,
        description,
        category_id,
    )
}

/// Add a transfer-typed transaction on a single account (indiBudget models a
/// transfer as one row per account; see harness notes). Returns its id.
pub fn add_transfer(
    db: &Database,
    account_id: &str,
    amount: &str,
    on: &str,
    description: &str,
) -> String {
    insert_tx(
        db,
        account_id,
        TransactionType::Transfer,
        amount,
        on,
        description,
        None,
    )
}

/// Fetch all transactions (default filter) for assertions.
pub fn all_transactions(db: &Database) -> Vec<Transaction> {
    db.with_connection(|conn| repository::get_transactions(conn, &TransactionFilter::default()))
        .expect("get transactions")
}
