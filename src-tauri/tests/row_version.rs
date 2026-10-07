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

// ------------------------------------------------- the models carry it

use indibudget_lib::boundary::commands::build_registry;
use indibudget_lib::boundary::registry::{dispatch, BoundaryCtx};
use indibudget_lib::boundary::{Actor, BoundaryError, Request, Response, SharedState};
use indibudget_lib::database::repository;
use serde_json::{json, Value};

/// One of each versioned model, written through the real repositories.
struct Seeded {
    account: String,
    category: String,
    transaction: String,
    budget: String,
    goal: String,
    recurring: String,
}

fn seed(db: &Database) -> Seeded {
    let account = new_account(db, "Checking", AccountType::Checking, "1000.00");
    let category = new_category(db, "Groceries", CategoryType::Expense, "#eab308");
    let transaction = add_expense(db, &account, "45.00", "2026-06-01", "Market", Some(&category));

    let budget = Budget::new(
        "Food".into(),
        category.clone(),
        dec("400.00"),
        BudgetPeriod::Monthly,
        date("2026-06-01"),
    );
    let goal = SavingsGoal::new("Holiday".into(), GoalType::Savings, dec("2000.00"));
    let recurring = RecurringTransaction::new(
        account.clone(),
        TransactionType::Expense,
        dec("12.99"),
        "Streaming".into(),
        RecurrenceFrequency::Monthly,
        date("2026-06-15"),
    );
    db.with_connection(|conn| {
        repository::create_budget(conn, &budget)?;
        repository::create_goal(conn, &goal)?;
        repository::create_recurring(conn, &recurring)
    })
    .unwrap();

    Seeded {
        account,
        category,
        transaction,
        budget: budget.id,
        goal: goal.id,
        recurring: recurring.id,
    }
}

/// The version each model reports when read back through the repository.
fn model_versions(db: &Database, s: &Seeded) -> [i64; 6] {
    db.with_connection(|conn| {
        Ok([
            repository::get_account(conn, &s.account)?.row_version,
            repository::get_transaction(conn, &s.transaction)?.row_version,
            repository::get_category(conn, &s.category)?.row_version,
            repository::get_budget(conn, &s.budget)?.row_version,
            repository::get_goal(conn, &s.goal)?.row_version,
            repository::get_recurring_by_id(conn, &s.recurring)?.row_version,
        ])
    })
    .unwrap()
}

#[test]
fn every_model_reads_its_row_version_from_the_database() {
    let db = db();
    let s = seed(&db);
    assert_eq!(model_versions(&db, &s), [1; 6]);

    // Move each row on through the column, not the model, so the only way
    // the model can report 2 is by having read it.
    for (table, id) in [
        ("accounts", &s.account),
        ("transactions", &s.transaction),
        ("categories", &s.category),
        ("budgets", &s.budget),
        ("savings_goals", &s.goal),
        ("recurring_transactions", &s.recurring),
    ] {
        db.with_connection(|conn| {
            conn.execute(
                &format!("UPDATE {table} SET updated_at = updated_at WHERE id = ?1"),
                [id],
            )?;
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(model_versions(&db, &s), [2; 6]);

    // And the list reads agree with the single-row reads.
    let listed = db
        .with_connection(|conn| {
            Ok((
                repository::get_all_accounts(conn)?,
                repository::get_all_categories(conn)?,
                repository::get_all_budgets(conn)?,
                repository::get_all_goals(conn)?,
                repository::get_all_recurring(conn)?,
                repository::get_transactions(conn, &TransactionFilter::default())?,
            ))
        })
        .unwrap();
    assert_eq!(listed.0.iter().find(|a| a.id == s.account).unwrap().row_version, 2);
    assert_eq!(listed.1.iter().find(|c| c.id == s.category).unwrap().row_version, 2);
    assert_eq!(listed.2.iter().find(|b| b.id == s.budget).unwrap().row_version, 2);
    assert_eq!(listed.3.iter().find(|g| g.id == s.goal).unwrap().row_version, 2);
    assert_eq!(listed.4.iter().find(|r| r.id == s.recurring).unwrap().row_version, 2);
    assert_eq!(listed.5.iter().find(|t| t.id == s.transaction).unwrap().row_version, 2);
}

#[test]
fn a_repository_update_advances_the_version_the_model_reports() {
    let db = db();
    let s = seed(&db);

    db.with_connection(|conn| {
        let mut account = repository::get_account(conn, &s.account)?;
        account.name = "Everyday".into();
        repository::update_account(conn, &account)?;

        let mut tx = repository::get_transaction(conn, &s.transaction)?;
        tx.description = "Farmers Market".into();
        repository::update_transaction(conn, &tx)?;

        let mut category = repository::get_category(conn, &s.category)?;
        category.color = "#000000".into();
        repository::update_category(conn, &category)?;

        let mut budget = repository::get_budget(conn, &s.budget)?;
        budget.amount = dec("450.00");
        repository::update_budget(conn, &budget)?;

        let mut goal = repository::get_goal(conn, &s.goal)?;
        goal.target_amount = dec("2500.00");
        repository::update_goal(conn, &goal)?;

        let mut recurring = repository::get_recurring_by_id(conn, &s.recurring)?;
        recurring.amount = dec("14.99");
        repository::update_recurring(conn, &recurring)
    })
    .unwrap();

    assert_eq!(model_versions(&db, &s), [2; 6]);
}

/// The triggers own the column. Whatever version a model happens to carry in
/// memory must never reach the database, or a stale copy could wind a row's
/// version backwards and let an old save through.
#[test]
fn the_version_a_model_carries_is_never_written() {
    let db = db();
    let mut account = Account::new("Checking".into(), AccountType::Checking);
    account.row_version = 42;
    db.with_connection(|conn| repository::create_account(conn, &account))
        .unwrap();
    assert_eq!(row_version(&db, "accounts", &account.id), 1);

    account.row_version = 99;
    account.name = "Renamed".into();
    db.with_connection(|conn| repository::update_account(conn, &account))
        .unwrap();
    assert_eq!(row_version(&db, "accounts", &account.id), 2);
}

#[test]
fn row_version_survives_a_json_round_trip_and_defaults_when_absent() {
    let mut account = Account::new("Checking".into(), AccountType::Checking);
    account.row_version = 7;
    let encoded = serde_json::to_value(&account).unwrap();
    assert_eq!(encoded["row_version"], json!(7));
    let decoded: Account = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded.row_version, 7);

    // A backup or a client written before the column existed simply leaves it
    // out, and that must still read as a row that has not been changed.
    let mut older = encoded;
    older.as_object_mut().unwrap().remove("row_version");
    let decoded: Account = serde_json::from_value(older).unwrap();
    assert_eq!(decoded.row_version, 1);

    for value in [
        serde_json::to_value(Transaction::new(
            "a".into(),
            TransactionType::Expense,
            dec("1"),
            date("2026-06-01"),
            "x".into(),
        ))
        .unwrap(),
        serde_json::to_value(Category::new("c".into(), CategoryType::Expense, "#fff".into()))
            .unwrap(),
        serde_json::to_value(Budget::new(
            "b".into(),
            "c".into(),
            dec("1"),
            BudgetPeriod::Monthly,
            date("2026-06-01"),
        ))
        .unwrap(),
        serde_json::to_value(SavingsGoal::new("g".into(), GoalType::Savings, dec("1"))).unwrap(),
        serde_json::to_value(RecurringTransaction::new(
            "a".into(),
            TransactionType::Expense,
            dec("1"),
            "r".into(),
            RecurrenceFrequency::Monthly,
            date("2026-06-01"),
        ))
        .unwrap(),
    ] {
        assert_eq!(value["row_version"], json!(1), "{value}");
    }
}

// --------------------------------------------- refused at the boundary

fn call(db: &Database, shared: &SharedState, command: &str, args: Value) -> Result<Value, BoundaryError> {
    let registry = build_registry();
    let actor = Actor::local_owner();
    let ctx = BoundaryCtx::new(db, &actor, shared);
    match dispatch(&registry, &ctx, Request::new(command, args)) {
        Response::Ok { value } => Ok(value),
        Response::Err { error, .. } => Err(error),
    }
}

/// What a save hands back must be the version the row is actually at, after
/// its authorship stamp too. Otherwise the person's very next save from the
/// same screen would be refused as someone else's change.
#[test]
fn a_boundary_save_hands_back_the_version_the_row_settled_at() {
    let db = db();
    let shared = SharedState::new();

    let created = call(
        &db,
        &shared,
        "create_account",
        json!({ "request": { "name": "Checking", "account_type": "checking" } }),
    )
    .unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["row_version"], json!(row_version(&db, "accounts", &id)));

    let updated = call(
        &db,
        &shared,
        "update_account",
        json!({
            "request": { "id": id, "name": "Everyday" },
            "expected_row_version": created["row_version"],
        }),
    )
    .unwrap();
    assert_eq!(updated["name"], json!("Everyday"));
    assert_eq!(updated["row_version"], json!(row_version(&db, "accounts", &id)));

    // So a second save from the same open form goes straight through.
    call(
        &db,
        &shared,
        "update_account",
        json!({
            "request": { "id": id, "name": "Everyday spending" },
            "expected_row_version": updated["row_version"],
        }),
    )
    .expect("a save built on the version just handed back should be accepted");
}

#[test]
fn a_boundary_save_built_on_an_old_version_is_refused_and_changes_nothing() {
    let db = db();
    let shared = SharedState::new();
    let s = seed(&db);

    // Someone else changes each record after our screen read it at version 1.
    for (table, id) in [
        ("accounts", &s.account),
        ("transactions", &s.transaction),
        ("categories", &s.category),
        ("budgets", &s.budget),
        ("savings_goals", &s.goal),
        ("recurring_transactions", &s.recurring),
    ] {
        db.with_connection(|conn| {
            conn.execute(
                &format!("UPDATE {table} SET updated_at = updated_at WHERE id = ?1"),
                [id],
            )?;
            Ok(())
        })
        .unwrap();
    }

    let cases = [
        ("update_account", json!({ "id": s.account, "name": "Mine" }), "account"),
        ("update_transaction", json!({ "id": s.transaction, "description": "Mine" }), "transaction"),
        ("update_category", json!({ "id": s.category, "name": "Mine" }), "category"),
        ("update_budget", json!({ "id": s.budget, "name": "Mine" }), "budget"),
        ("update_goal", json!({ "id": s.goal, "name": "Mine" }), "goal"),
        ("update_recurring", json!({ "id": s.recurring, "description": "Mine" }), "recurring payment"),
    ];

    for (command, request, label) in cases {
        let err = call(
            &db,
            &shared,
            command,
            json!({ "request": request, "expected_row_version": 1 }),
        )
        .unwrap_err();
        match &err {
            BoundaryError::Stale { record, expected, actual } => {
                assert_eq!(record, label, "{command}");
                assert_eq!((*expected, *actual), (1, 2), "{command}");
            }
            other => panic!("{command}: expected Stale, got {other:?}"),
        }
        assert!(err.sentence().contains("someone else"), "{}", err.sentence());
    }

    // Nothing was written by any of the refused saves.
    assert_eq!(model_versions(&db, &s), [2; 6]);
    let account = db
        .with_connection(|conn| repository::get_account(conn, &s.account))
        .unwrap();
    assert_eq!(account.name, "Checking");
}

#[test]
fn a_boundary_save_against_a_deleted_record_says_so() {
    let db = db();
    let shared = SharedState::new();

    let err = call(
        &db,
        &shared,
        "update_budget",
        json!({ "request": { "id": "gone", "name": "Mine" }, "expected_row_version": 1 }),
    )
    .unwrap_err();
    assert!(err.sentence().contains("has been deleted"), "{}", err.sentence());
}

/// A caller that sends no version keeps the older last-write-wins behaviour,
/// so nothing that has not been taught about versions breaks.
#[test]
fn a_boundary_save_without_a_version_is_not_checked() {
    let db = db();
    let shared = SharedState::new();
    let s = seed(&db);
    touch(&db, "budgets", &s.budget, "Moved on");

    let saved = call(
        &db,
        &shared,
        "update_budget",
        json!({ "request": { "id": s.budget, "name": "Mine" } }),
    )
    .unwrap();
    assert_eq!(saved["name"], json!("Mine"));
}
