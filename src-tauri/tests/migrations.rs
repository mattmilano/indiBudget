//! Section 6 — Migration & upgrade safety (the achievable, in-process part).
//!
//! Full fidelity ("open a real old on-disk database, migrate, assert balances
//! reconcile") needs a committed fixture DB and is best run in CI / manual QA.
//! Here we assert the properties the migration runner must always hold:
//!   * migrations are version-gated and IDEMPOTENT (re-running is a no-op),
//!   * the latest additive column exists after migration,
//!   * seed data is present and the schema supports the repositories.

mod common;

use common::*;
use indibudget_lib::database::{migrations, repository};

#[test]
fn migrations_are_idempotent_when_rerun() {
    let db = db();

    // Re-running the full migration set on an already-migrated DB must succeed
    // and must not change the schema version.
    let before: i32 = db
        .with_connection(|conn| {
            Ok(conn
                .query_row("SELECT COALESCE(MAX(version),0) FROM schema_version", [], |r| r.get(0))?)
        })
        .expect("read version");

    db.with_connection(|conn| migrations::run_all(conn)).expect("re-run migrations");
    db.with_connection(|conn| migrations::run_all(conn)).expect("re-run migrations again");

    let after: i32 = db
        .with_connection(|conn| {
            Ok(conn
                .query_row("SELECT COALESCE(MAX(version),0) FROM schema_version", [], |r| r.get(0))?)
        })
        .expect("read version");

    assert_eq!(before, after, "re-running migrations must not advance the version");
    assert!(after >= 7, "expected at least 7 applied migrations, got {after}");
}

#[test]
fn latest_additive_column_exists_after_migration() {
    // Migration 007 adds category_rules.is_user_created via ALTER TABLE.
    // A SELECT referencing it must succeed (would error if the column is absent).
    let db = db();
    let ok = db
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM category_rules WHERE is_user_created = 0",
                [],
                |r| r.get::<_, i64>(0),
            )?;
            Ok(())
        })
        .is_ok();
    assert!(ok, "is_user_created column must exist after migrations");
}

#[test]
fn seed_data_and_repositories_are_available_after_migration() {
    let db = db();

    // Default system categories are seeded.
    let cats = db
        .with_connection(|conn| repository::get_all_categories(conn))
        .expect("categories");
    assert!(!cats.is_empty(), "default categories must be seeded");
    assert!(
        cats.iter().any(|c| c.is_system),
        "at least one system category must exist"
    );

    // Every core repository read works against the migrated schema.
    db.with_connection(|conn| repository::get_all_accounts(conn)).expect("accounts table");
    db.with_connection(|conn| repository::get_all_budgets(conn)).expect("budgets table");
    db.with_connection(|conn| repository::get_all_recurring(conn)).expect("recurring table");
    db.with_connection(|conn| repository::get_all_goals(conn)).expect("goals table");
    db.with_connection(|conn| repository::get_category_rules(conn)).expect("rules table");
    db.with_connection(|conn| repository::get_cancelled_subscriptions(conn)).expect("subs table");
}
