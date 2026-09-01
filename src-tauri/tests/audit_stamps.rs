//! Multi-user phase 2: authorship stamps.
//!
//! `created_by` / `updated_by` are written explicitly by the boundary rather
//! than by triggers, so that raw connections keep working. These tests hold
//! that helper to its contract, including its interaction with the
//! `row_version` trigger.

mod common;

use common::*;
use indibudget_lib::boundary::users::create_user;
use indibudget_lib::boundary::{stamp_write, Actor, Grants, Stamped};
use indibudget_lib::database::Database;
use indibudget_lib::models::*;

fn authorship(db: &Database, id: &str) -> (Option<String>, Option<String>) {
    db.with_connection(|conn| {
        let row = conn.query_row(
            "SELECT created_by, updated_by FROM accounts WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(row)
    })
    .expect("authorship should be readable")
}

fn version(db: &Database, id: &str) -> i64 {
    db.with_connection(|conn| {
        let v: i64 = conn.query_row(
            "SELECT row_version FROM accounts WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(v)
    })
    .unwrap()
}

fn actor(name: &str) -> Actor {
    Actor::new(format!("id-{name}"), name.to_string(), false, Grants::all())
}

#[test]
fn an_insert_stamp_records_both_author_columns() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");
    let sam = actor("Sam");

    db.with_connection(|conn| {
        stamp_write(conn, Stamped::Accounts, &id, &sam, true).unwrap();
        Ok(())
    })
    .unwrap();

    let (created_by, updated_by) = authorship(&db, &id);
    assert_eq!(created_by.as_deref(), Some("id-Sam"));
    assert_eq!(updated_by.as_deref(), Some("id-Sam"));
}

#[test]
fn an_update_stamp_preserves_who_created_the_row() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");

    db.with_connection(|conn| {
        stamp_write(conn, Stamped::Accounts, &id, &actor("Sam"), true).unwrap();
        stamp_write(conn, Stamped::Accounts, &id, &actor("Alex"), false).unwrap();
        Ok(())
    })
    .unwrap();

    let (created_by, updated_by) = authorship(&db, &id);
    assert_eq!(
        created_by.as_deref(),
        Some("id-Sam"),
        "an edit must not rewrite who created the row"
    );
    assert_eq!(updated_by.as_deref(), Some("id-Alex"));
}

/// Stamping is a real UPDATE, so it fires the `row_version` trigger like any
/// other. That is harmless and is pinned here so nobody has to rediscover it:
/// `row_version` is an opaque change-detector, not an edit counter. Optimistic
/// concurrency asks "is this row still the one I read?", and the answer stays
/// correct whether an edit advances the version by one or by two — because a
/// caller always reads the value *after* the write and its stamp have both
/// landed.
///
/// What would break is a stamp with no accompanying data write, which could
/// invalidate a version a caller is holding. The boundary never does that:
/// stamps only ever accompany a write.
#[test]
fn stamping_advances_the_version_and_that_is_harmless() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");
    let before = version(&db, &id);

    db.with_connection(|conn| {
        stamp_write(conn, Stamped::Accounts, &id, &actor("Sam"), true).unwrap();
        Ok(())
    })
    .unwrap();

    let after = version(&db, &id);
    assert!(
        after > before,
        "a stamp is an update and should advance the version"
    );

    // The property that actually matters: the version differs after a change,
    // so a caller holding the old one can be told the row moved.
    assert_ne!(before, after);
}

#[test]
fn stamps_are_readable_as_the_person_not_just_an_id() {
    // The stamp stores the user id; joining it back to a display name is what
    // makes "Alex changed this" possible in the UI.
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");

    db.with_connection(|conn| {
        let user = create_user(
            conn,
            "alex",
            "Alex Rivera",
            "Password1",
            false,
            &Grants::all(),
            None,
        )
        .unwrap();
        let alex = Actor::new(user.id.clone(), user.display_name.clone(), false, Grants::all());
        stamp_write(conn, Stamped::Accounts, &id, &alex, true).unwrap();

        let name: String = conn.query_row(
            "SELECT u.display_name FROM accounts a
             JOIN users u ON u.id = a.updated_by
             WHERE a.id = ?1",
            [&id],
            |row| row.get(0),
        )?;
        assert_eq!(name, "Alex Rivera");
        Ok(())
    })
    .unwrap();
}

#[test]
fn every_stamped_table_accepts_a_stamp() {
    // A cheap guard that the Stamped enum and the schema agree. If a table is
    // added to the enum without the migration adding its columns, this fails
    // rather than surfacing as a runtime error during a sweep.
    let db = db();
    let sam = actor("Sam");
    let tables = [
        Stamped::Accounts,
        Stamped::Transactions,
        Stamped::Categories,
        Stamped::Budgets,
        Stamped::SavingsGoals,
        Stamped::GoalContributions,
        Stamped::RecurringTransactions,
        Stamped::CategoryRules,
        Stamped::Users,
    ];

    db.with_connection(|conn| {
        for table in tables {
            // No row with this id exists; the statement must still be valid
            // SQL against the real schema, which is what is being checked.
            stamp_write(conn, table, "no-such-id", &sam, true)
                .unwrap_or_else(|e| panic!("{} rejected a stamp: {}", table.table(), e.sentence()));
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn rows_the_boundary_never_touched_keep_a_null_author() {
    let db = db();
    let id = new_account(&db, "Checking", AccountType::Checking, "100.00");
    let (created_by, updated_by) = authorship(&db, &id);
    assert!(created_by.is_none());
    assert!(updated_by.is_none());
}
