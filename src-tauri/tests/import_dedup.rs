//! Tier 2 — Import idempotency (invariant I7) and duplicate false-positives (F7).
//!
//! The import command dedups by `imported_id` via
//! `repository::check_duplicate_transaction`. These tests drive that exact
//! primitive plus `importer::parse_transaction` (the real id generator) so the
//! behavior under test is the shipping behavior.
//!
//! I7: re-importing the same row set creates ZERO new rows.
//! F7: two GENUINELY DISTINCT transactions that happen to share
//!     date+description+amount must BOTH survive an import.

mod common;

use common::*;
use indibudget_lib::database::{repository, Database};
use indibudget_lib::models::*;
use indibudget_lib::services::importer::{
    disambiguate_import_ids, parse_transaction, RawTransaction,
};

fn raw(date: &str, description: &str, amount: &str) -> RawTransaction {
    RawTransaction {
        date: date.to_string(),
        description: description.to_string(),
        amount: amount.to_string(),
        debit: None,
        credit: None,
        category: None,
    }
}

/// Mirror the dedup pipeline in the `import_transactions` command against a
/// real DB, calling the SAME shared `disambiguate_import_ids` the command uses.
/// Returns (imported_count, skipped_count).
fn import_rows(db: &Database, account_id: &str, rows: &[RawTransaction]) -> (usize, usize) {
    db.with_connection(|conn| {
        // Parse the whole batch, then disambiguate identical-looking rows
        // exactly as the command does.
        let mut parsed: Vec<_> = rows
            .iter()
            .map(|r| parse_transaction(r, account_id, "%Y-%m-%d").expect("parse row"))
            .collect();
        disambiguate_import_ids(&mut parsed);

        let mut imported = 0;
        let mut skipped = 0;
        for tx in parsed {
            if let Some(ref imported_id) = tx.imported_id {
                if repository::check_duplicate_transaction(conn, imported_id)? {
                    skipped += 1;
                    continue;
                }
            }
            repository::create_transaction(conn, &tx)?;
            imported += 1;
        }
        Ok((imported, skipped))
    })
    .expect("import rows")
}

#[test]
fn reimporting_the_same_file_creates_zero_new_rows() {
    // I7: idempotency by imported_id.
    let db = db();
    let acct = new_account(&db, "Checking", AccountType::Checking, "0.00");

    let rows = vec![
        raw("2026-06-01", "Paycheck", "1500.00"),
        raw("2026-06-02", "Rent", "-1200.00"),
        raw("2026-06-03", "Groceries", "-85.40"),
    ];

    let (imported1, skipped1) = import_rows(&db, &acct, &rows);
    assert_eq!(imported1, 3, "first import inserts all rows");
    assert_eq!(skipped1, 0);

    let (imported2, skipped2) = import_rows(&db, &acct, &rows);
    assert_eq!(imported2, 0, "re-importing the same file must insert nothing");
    assert_eq!(skipped2, 3, "all three rows recognized as duplicates");

    // Ledger truth: still exactly 3 transactions.
    assert_eq!(all_transactions(&db).len(), 3, "no duplicate rows created");
}

#[test]
fn distinct_rows_are_not_treated_as_duplicates() {
    // Sanity: rows that differ in any of date/description/amount survive.
    let db = db();
    let acct = new_account(&db, "Checking", AccountType::Checking, "0.00");

    let rows = vec![
        raw("2026-06-04", "Coffee", "-4.50"),
        raw("2026-06-04", "Coffee", "-5.25"),   // different amount
        raw("2026-06-05", "Coffee", "-4.50"),   // different date
        raw("2026-06-04", "Lunch", "-4.50"),    // different description
    ];

    let (imported, skipped) = import_rows(&db, &acct, &rows);
    assert_eq!(imported, 4, "all genuinely distinct rows must import");
    assert_eq!(skipped, 0);
}

#[test]
fn two_identical_looking_but_distinct_transactions_both_survive() {
    // F7 (reproduction of a real defect): two separate $4.50 coffees on the
    // same day with the same description are DISTINCT real-world purchases and
    // must BOTH import. The current id scheme hashes only
    // (date, description, amount), so they collide and the second is wrongly
    // skipped. This test pins the CORRECT behavior; it fails until the import
    // id incorporates per-row position. See findings write-up.
    let db = db();
    let acct = new_account(&db, "Checking", AccountType::Checking, "0.00");

    let rows = vec![
        raw("2026-06-04", "Coffee Shop", "-4.50"),
        raw("2026-06-04", "Coffee Shop", "-4.50"), // a second, real coffee
    ];

    let (imported, skipped) = import_rows(&db, &acct, &rows);
    assert_eq!(
        imported, 2,
        "two distinct same-day/same-amount purchases must both import (F7)"
    );
    assert_eq!(skipped, 0, "neither should be dropped as a false-positive duplicate");
    assert_eq!(all_transactions(&db).len(), 2);
}
