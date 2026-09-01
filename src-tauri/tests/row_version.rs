//! Multi-user phase 1: the optimistic-concurrency backstop.
//!
//! `row_version` is maintained by an AFTER UPDATE trigger per table rather than
//! by the repositories, so it cannot be forgotten by code written later. These
//! tests hold that mechanism to its contract — including the property the whole
//! design rests on: that a trigger updating its own table does not re-fire.

mod common;

use common::*;
use indibudget_lib::database::Database;
use indibudget_lib::models::*;

/// Every table carrying user-editable rows that a second person could be
/// editing at the same time.
const VERSIONED_TABLES: [&str; 8] = [
    "accounts",
    "transactions",
    "categories",
    "budgets",
    "savings_goals",
    "goal_contributions",
    "recurring_transactions",
    "category_rules",
];

fn row_version(db: &Database, table: &str, id: &str) -> i64 {
    db.with_connection(|conn| {
        let sql = format!("SELECT row_version FROM {table} WHERE id = ?1");
        let v: i64 = conn.query_row(&sql, [id], |row| row.get(0))?;
        Ok(v)
    })
    .expect("row_version should be readable")
}

fn touch(db: &Database, table: &str, id: &str, name: &str) {
    db.with_connection(|conn| {
        let sql = format!("UPDATE {table} SET name = ?1 WHERE id = ?2");
        conn.execute(&sql, rusqlite::params![name, id])?;
        Ok(())
    })
    .expect("update should succeed");
}

#[test]
fn every_versioned_table_has_a_row_version_trigger() {
    let db = db();
    for table in VERSIONED_TABLES {
        let exists: i64 = db
            .with_connection(|conn| {
                let v: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'trigger' AND name = ?1",
                    [format!("trg_{table}_row_version")],
                    |row| row.get(0),
                )?;
                Ok(v)
            })
            .unwrap();
        assert_eq!(exists, 1, "{table} is missing its row_version trigger");
    }
}

#[test]
fn every_versioned_table_has_the_boundary_columns() {
    let db = db();
    for table in VERSIONED_TABLES {
        let columns: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
                let rows = stmt
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .unwrap();

        for expected in ["row_version", "created_by", "updated_by"] {
            assert!(
                columns.iter().any(|c| c == expected),
                "{table} is missing {expected}"
            );
        }
    }
}

#[test]
fn a_new_row_starts_at_version_one() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");
    assert_eq!(row_version(&db, "accounts", &id), 1);
}

/// The property the trigger design depends on. SQLite's `recursive_triggers`
/// is off by default and this codebase never enables it, so the trigger's own
/// UPDATE cannot re-fire it. If recursion were ever switched on, this would
/// jump past 2 and fail here rather than silently corrupting every version
/// comparison in the app.
#[test]
fn one_update_advances_the_version_by_exactly_one() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");

    touch(&db, "accounts", &id, "Renamed");
    assert_eq!(
        row_version(&db, "accounts", &id),
        2,
        "a single update should advance the version by exactly one — \
         a jump past 2 means the trigger re-fired"
    );
}

#[test]
fn repeated_updates_advance_one_at_a_time() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");

    for expected in 2..=6 {
        touch(&db, "accounts", &id, &format!("Rename {expected}"));
        assert_eq!(row_version(&db, "accounts", &id), expected);
    }
}

#[test]
fn versions_are_per_row_not_per_table() {
    let db = db();
    let first = new_account(&db, "Checking", AccountType::Checking, "100.00");
    let second = new_account(&db, "Savings", AccountType::Savings, "500.00");

    touch(&db, "accounts", &first, "Renamed");
    touch(&db, "accounts", &first, "Renamed again");

    assert_eq!(row_version(&db, "accounts", &first), 3);
    assert_eq!(
        row_version(&db, "accounts", &second),
        1,
        "editing one row should not advance another row's version"
    );
}

#[test]
fn transactions_are_versioned_too() {
    let db = db();
    let account = new_account(&db, "Checking", AccountType::Checking, "1000.00");
    let category = new_category(&db, "Groceries", CategoryType::Expense, "#eab308");
    let txn = add_expense(
        &db,
        &account,
        "45.00",
        "2026-06-01",
        "Market",
        Some(&category),
    );

    assert_eq!(row_version(&db, "transactions", &txn), 1);

    db.with_connection(|conn| {
        conn.execute(
            "UPDATE transactions SET description = ?1 WHERE id = ?2",
            rusqlite::params!["Farmers Market", &txn],
        )?;
        Ok(())
    })
    .unwrap();

    assert_eq!(row_version(&db, "transactions", &txn), 2);
}

/// The audit stamps are deliberately not maintained by triggers, so that raw
/// connections — tests, backup and restore, and any future CLI or salvage path
/// — keep working without per-session actor state on the connection. A row
/// written outside the boundary simply has no author, and that must not be an
/// error.
#[test]
fn rows_written_outside_the_boundary_have_no_author_and_still_work() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");

    let (created_by, updated_by): (Option<String>, Option<String>) = db
        .with_connection(|conn| {
            let row = conn.query_row(
                "SELECT created_by, updated_by FROM accounts WHERE id = ?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            Ok(row)
        })
        .unwrap();

    assert!(created_by.is_none(), "raw writes should leave no author");
    assert!(updated_by.is_none(), "raw writes should leave no author");

    // And the row is still fully usable through the normal read path.
    touch(&db, "accounts", &id, "Renamed");
    assert_eq!(row_version(&db, "accounts", &id), 2);
}

#[test]
fn balances_still_derive_correctly_after_the_migration() {
    // Guards against the migration having disturbed the derived-balance query,
    // which reads columns the new ones now sit beside.
    let db = db();
    let account = new_account(&db, "Checking", AccountType::Checking, "1000.00");
    let category = new_category(&db, "Groceries", CategoryType::Expense, "#eab308");

    add_expense(&db, &account, "45.00", "2026-06-01", "Market", Some(&category));
    add_expense(&db, &account, "55.00", "2026-06-02", "Market", Some(&category));

    assert_eq!(get_balance(&db, &account), dec("900.00"));
}
