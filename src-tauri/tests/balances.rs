//! Tier 1 — Derived balance invariants (I1, I2 from testing handoff).
//!
//! I1: Account balance = starting_balance + SUM(income) - SUM(expense)
//!     + transfer_in - transfer_out
//! I2: Transfers are zero-sum across accounts (total balance unchanged)

mod common;

use common::*;
use indibudget_lib::database::repository;
use indibudget_lib::models::*;

#[test]
fn account_balance_is_derived_from_transactions() {
    let db = db();
    let checking = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    // Before any transactions, balance equals starting_balance
    assert_eq!(get_balance(&db, &checking), dec("1000.00"));

    // Add income: 1000 + 500 = 1500
    add_income(&db, &checking, "500.00", "2026-06-01", "bonus", None);
    assert_eq!(get_balance(&db, &checking), dec("1500.00"));

    // Add expense: 1500 - 200 = 1300
    add_expense(&db, &checking, "200.00", "2026-06-02", "groceries", None);
    assert_eq!(get_balance(&db, &checking), dec("1300.00"));

    // Multiple transactions: 1300 + 100 - 50 = 1350
    add_income(&db, &checking, "100.00", "2026-06-03", "refund", None);
    add_expense(&db, &checking, "50.00", "2026-06-04", "coffee", None);
    assert_eq!(get_balance(&db, &checking), dec("1350.00"));
}

#[test]
fn transfer_moves_money_between_accounts() {
    let db = db();
    let checking = new_account(&db, "Checking", AccountType::Checking, "2000.00");
    let savings = new_account(&db, "Savings", AccountType::Savings, "500.00");

    // Before transfer: checking = 2000, savings = 500
    assert_eq!(get_balance(&db, &checking), dec("2000.00"));
    assert_eq!(get_balance(&db, &savings), dec("500.00"));

    // Transfer 300 from checking to savings
    add_transfer(&db, &checking, &savings, "300.00", "2026-06-01", "to savings");

    // After transfer: checking = 1700, savings = 800
    assert_eq!(get_balance(&db, &checking), dec("1700.00"));
    assert_eq!(get_balance(&db, &savings), dec("800.00"));

    // Total unchanged: 2500 before, 2500 after
    let total_before = dec("2000.00") + dec("500.00");
    let total_after = get_balance(&db, &checking) + get_balance(&db, &savings);
    assert_eq!(total_before, total_after, "transfers are zero-sum");
}

#[test]
fn deleting_transaction_updates_balance() {
    let db = db();
    let checking = new_account(&db, "Checking", AccountType::Checking, "1000.00");

    let income_id = add_income(&db, &checking, "200.00", "2026-06-01", "bonus", None);
    assert_eq!(get_balance(&db, &checking), dec("1200.00"));

    // Delete the income transaction
    db.with_connection(|conn| repository::delete_transaction(conn, &income_id))
        .expect("delete");

    // Balance should return to starting balance
    assert_eq!(get_balance(&db, &checking), dec("1000.00"));
}

#[test]
fn deleting_transfer_updates_both_accounts() {
    let db = db();
    let checking = new_account(&db, "Checking", AccountType::Checking, "1000.00");
    let savings = new_account(&db, "Savings", AccountType::Savings, "500.00");

    let (from_id, _to_id) = add_transfer(&db, &checking, &savings, "200.00", "2026-06-01", "transfer");

    // After transfer
    assert_eq!(get_balance(&db, &checking), dec("800.00"));
    assert_eq!(get_balance(&db, &savings), dec("700.00"));

    // Delete the transfer (should delete both sides)
    db.with_connection(|conn| repository::delete_transaction_with_pair(conn, &from_id))
        .expect("delete transfer");

    // Both balances restored
    assert_eq!(get_balance(&db, &checking), dec("1000.00"));
    assert_eq!(get_balance(&db, &savings), dec("500.00"));
}

#[test]
fn mixed_transactions_produce_correct_balance() {
    // Complex scenario with multiple account types and transaction types
    let db = db();
    let checking = new_account(&db, "Checking", AccountType::Checking, "5000.00");
    let savings = new_account(&db, "Savings", AccountType::Savings, "10000.00");
    let credit = new_account(&db, "Credit Card", AccountType::CreditCard, "0.00");

    // Checking: +3000 salary, -1200 rent, -85 groceries, -500 to savings
    add_income(&db, &checking, "3000.00", "2026-06-01", "Salary", None);
    add_expense(&db, &checking, "1200.00", "2026-06-05", "Rent", None);
    add_expense(&db, &checking, "85.00", "2026-06-10", "Groceries", None);
    add_transfer(&db, &checking, &savings, "500.00", "2026-06-15", "to savings");

    // Credit card: -50 coffee, -120 dining
    add_expense(&db, &credit, "50.00", "2026-06-08", "Coffee Shop", None);
    add_expense(&db, &credit, "120.00", "2026-06-12", "Restaurant", None);

    // Hand-derived balances:
    // Checking: 5000 + 3000 - 1200 - 85 - 500 = 6215
    // Savings: 10000 + 500 = 10500
    // Credit: 0 - 50 - 120 = -170
    assert_eq!(get_balance(&db, &checking), dec("6215.00"));
    assert_eq!(get_balance(&db, &savings), dec("10500.00"));
    assert_eq!(get_balance(&db, &credit), dec("-170.00"));

    // Total net worth: 6215 + 10500 - 170 = 16545
    let total = get_balance(&db, &checking) + get_balance(&db, &savings) + get_balance(&db, &credit);
    assert_eq!(total, dec("16545.00"));
}
