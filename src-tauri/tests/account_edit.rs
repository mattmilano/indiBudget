//! Editing an account must not move its balance by itself.
//!
//! The edit screen shows the *current* balance — the opening balance plus
//! every transaction — and sends back whatever is in that box. It used to be
//! stored as the opening balance, so saving any edit, even a new name, counted
//! every existing transaction a second time.

use indibudget_lib::boundary::commands::build_registry;
use indibudget_lib::boundary::registry::{dispatch, BoundaryCtx};
use indibudget_lib::boundary::{Actor, Request, Response, SharedState};
use indibudget_lib::database::Database;
use serde_json::{json, Value};

fn call(db: &Database, command: &str, args: Value) -> Value {
    let registry = build_registry();
    let actor = Actor::local_owner();
    let shared = SharedState::new();
    let ctx = BoundaryCtx::new(db, &actor, &shared);
    match dispatch(&registry, &ctx, Request::new(command, args)) {
        Response::Ok { value } => value,
        Response::Err { sentence, .. } => panic!("{command} refused: {sentence}"),
    }
}

fn balance(db: &Database, id: &str) -> rust_decimal::Decimal {
    call(db, "get_account", json!({ "id": id }))["balance"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn renaming_an_account_leaves_its_balance_alone_and_an_adjustment_lands_exactly() {
    let db = Database::in_memory().unwrap();
    let account = call(
        &db,
        "create_account",
        json!({ "request": { "name": "Checking", "account_type": "checking", "balance": "100.00" } }),
    );
    let id = account["id"].as_str().unwrap().to_string();
    call(
        &db,
        "create_transaction",
        json!({ "request": {
            "account_id": id, "transaction_type": "expense", "amount": "30.00",
            "date": "2026-10-01", "description": "Groceries"
        } }),
    );
    assert_eq!(balance(&db, &id), "70.00".parse().unwrap());

    // What the edit screen sends: a new name, and the balance box unchanged.
    call(
        &db,
        "update_account",
        json!({ "request": { "id": id, "name": "Joint Checking", "balance": "70.00" } }),
    );
    assert_eq!(balance(&db, &id), "70.00".parse().unwrap(), "a rename moved the balance");

    // Correcting the balance to match the bank lands exactly on that figure.
    call(&db, "update_account", json!({ "request": { "id": id, "balance": "82.50" } }));
    assert_eq!(balance(&db, &id), "82.50".parse().unwrap());
}
