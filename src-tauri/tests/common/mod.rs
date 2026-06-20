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

/// Create an account of the given type with a starting balance.
/// Returns the account id.
///
/// The balance is now DERIVED from transactions (starting_balance + transactions).
/// The `starting_balance` parameter sets the opening balance before any transactions.
pub fn new_account(db: &Database, name: &str, account_type: AccountType, starting_balance: &str) -> String {
    let acct = Account::with_starting_balance(name.to_string(), account_type, dec(starting_balance));
    let id = acct.id.clone();
    db.with_connection(|conn| repository::create_account(conn, &acct))
        .expect("create account");
    id
}

/// Get the computed balance for an account (starting_balance + all transactions).
pub fn get_balance(db: &Database, account_id: &str) -> Decimal {
    db.with_connection(|conn| {
        let account = repository::get_account(conn, account_id)?;
        Ok(account.balance)
    })
    .expect("get account balance")
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

/// Add a linked transfer between two accounts. Creates two transactions
/// (outgoing from source, incoming to destination) with the same transfer_pair_id.
/// Returns the (from_tx_id, to_tx_id) tuple.
pub fn add_transfer(
    db: &Database,
    from_account_id: &str,
    to_account_id: &str,
    amount: &str,
    on: &str,
    description: &str,
) -> (String, String) {
    use uuid::Uuid;

    let transfer_pair_id = Uuid::new_v4().to_string();
    let amt = dec(amount);
    let dt = date(on);

    // Outgoing transaction (from source account)
    // Description starts with "Transfer to" for balance computation
    let mut from_tx = Transaction::new(
        from_account_id.to_string(),
        TransactionType::Transfer,
        amt,
        dt,
        format!("Transfer to: {}", description),
    );
    from_tx.transfer_account_id = Some(to_account_id.to_string());
    from_tx.transfer_pair_id = Some(transfer_pair_id.clone());
    let from_id = from_tx.id.clone();

    // Incoming transaction (to destination account)
    // Description starts with "Transfer from" for balance computation
    let mut to_tx = Transaction::new(
        to_account_id.to_string(),
        TransactionType::Transfer,
        amt,
        dt,
        format!("Transfer from: {}", description),
    );
    to_tx.transfer_account_id = Some(from_account_id.to_string());
    to_tx.transfer_pair_id = Some(transfer_pair_id);
    let to_id = to_tx.id.clone();

    db.with_connection(|conn| {
        repository::create_transaction(conn, &from_tx)?;
        repository::create_transaction(conn, &to_tx)?;
        Ok(())
    })
    .expect("create transfer");

    (from_id, to_id)
}

/// Fetch all transactions (default filter) for assertions.
pub fn all_transactions(db: &Database) -> Vec<Transaction> {
    db.with_connection(|conn| repository::get_transactions(conn, &TransactionFilter::default()))
        .expect("get transactions")
}
